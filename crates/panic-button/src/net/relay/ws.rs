// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-PanicButton-app-store-exception

//! WebSocket sessions, directly or through an HTTP CONNECT proxy.

use crate::core::io::RelayLinkState;
use crate::proto::relay::Frame;
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::tungstenite::{Error as WsError, Message};
use tokio_tungstenite::{Connector, MaybeTlsStream, WebSocketStream};

use super::{AUTH_TIMEOUT, Ctx, End, READ_TIMEOUT, proxy};

type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// The target of a `ws://` or `wss://` URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Target {
    pub tls: bool,
    pub host: String,
    pub port: u16,
}

pub(super) fn target(url: &str) -> Result<Target, String> {
    let uri: http::Uri = url.parse().map_err(|e| format!("bad relay URL: {e}"))?;
    let tls = match uri.scheme_str() {
        Some("wss") => true,
        Some("ws") => false,
        _ => return Err("relay URL must start with ws:// or wss://".into()),
    };
    let host = uri.host().ok_or("relay URL has no host")?.to_owned();
    let port = uri.port_u16().unwrap_or(if tls { 443 } else { 80 });
    Ok(Target { tls, host, port })
}

async fn connect(ctx: &Ctx) -> Result<Ws, End> {
    let t = target(&ctx.url).map_err(End::lost)?;
    let req = ctx
        .url
        .as_str()
        .into_client_request()
        .map_err(|e| End::lost(e.to_string()))?;
    let tcp = tokio::time::timeout(AUTH_TIMEOUT, proxy::tcp_to(&ctx.proxy, &t))
        .await
        .map_err(|_| End::lost("connect timeout"))??;
    let _ = tcp.set_nodelay(true);
    let connector = if t.tls {
        Connector::Rustls(ctx.tls.clone())
    } else {
        Connector::Plain
    };
    let cfg = WebSocketConfig::default()
        .max_message_size(Some(crate::proto::limits::RELAY_WS_FRAME_MAX + 1))
        .max_frame_size(Some(crate::proto::limits::RELAY_WS_FRAME_MAX + 1));
    let hs = tokio_tungstenite::client_async_tls_with_config(req, tcp, Some(cfg), Some(connector));
    match tokio::time::timeout(AUTH_TIMEOUT, hs).await {
        Err(_) => Err(End::Upgrade("no WebSocket handshake".into())),
        Ok(Ok((ws, _))) => Ok(ws),
        // An HTTP answer other than 101, or a handshake a middlebox broke.
        Ok(Err(WsError::Http(r))) => Err(End::Upgrade(format!(
            "upgrade refused (HTTP {})",
            r.status()
        ))),
        Ok(Err(WsError::Protocol(e))) => Err(End::Upgrade(format!("handshake failed: {e}"))),
        Ok(Err(e)) => Err(End::lost(e.to_string())),
    }
}

pub(super) async fn session(
    ctx: &Ctx,
    out: &mut mpsc::UnboundedReceiver<Vec<u8>>,
    cursor: &mut Option<u64>,
) -> End {
    match run(ctx, out, cursor).await {
        Ok(never) => match never {},
        Err(e) => e,
    }
}

async fn run(
    ctx: &Ctx,
    out: &mut mpsc::UnboundedReceiver<Vec<u8>>,
    cursor: &mut Option<u64>,
) -> Result<std::convert::Infallible, End> {
    let mut ws = connect(ctx).await?;
    // Upgraded, but nothing comes through: a proxy that accepts the upgrade
    // and then buffers or drops frames.
    let first = tokio::time::timeout(AUTH_TIMEOUT, ws.next())
        .await
        .map_err(|_| End::Upgrade("no challenge after the upgrade".into()))?;
    let Some(Ok(Message::Binary(b))) = first else {
        return Err(End::lost("no challenge"));
    };
    let Ok(Frame::Challenge(ch)) = Frame::from_bytes(&b) else {
        return Err(End::lost("bad challenge"));
    };
    let env = ctx.sign(ch.nonce, *cursor).await?;
    ws.send(Message::Binary(Frame::Envelope(env).to_bytes().into()))
        .await
        .map_err(|e| End::lost(e.to_string()))?;
    let r = tokio::time::timeout(AUTH_TIMEOUT, ws.next())
        .await
        .map_err(|_| End::lost("no auth result"))?;
    let result = match r {
        Some(Ok(Message::Binary(b))) => match Frame::from_bytes(&b) {
            Ok(Frame::AuthResult(a)) if a.ok => a,
            _ => return Err(End::lost("authentication refused")),
        },
        Some(Ok(Message::Close(c))) => {
            return Err(End::lost(format!(
                "authentication refused ({})",
                c.map(|c| u16::from(c.code)).unwrap_or(0)
            )));
        }
        _ => return Err(End::lost("authentication failed")),
    };
    *cursor = Some(result.cursor);
    tracing::info!("relay {}: connected (WebSocket)", ctx.url);
    ctx.state(RelayLinkState::Connected, Some(result.presence))
        .await;
    let lost = |why: String| End::Lost {
        why,
        was_connected: true,
    };
    loop {
        tokio::select! {
            m = tokio::time::timeout(READ_TIMEOUT, ws.next()) => match m.map_err(|_| lost("relay silent".into()))? {
                Some(Ok(Message::Binary(b))) => match Frame::from_bytes(&b) {
                    Ok(Frame::Envelope(e)) => ctx.frame(e).await?,
                    Ok(Frame::Presence(p)) => {
                        ctx.state(RelayLinkState::Connected, Some(p.connected)).await;
                    }
                    _ => {}
                },
                Some(Ok(Message::Ping(p))) => {
                    ws.send(Message::Pong(p)).await.map_err(|e| lost(e.to_string()))?;
                }
                Some(Ok(Message::Close(c))) => {
                    return Err(lost(format!("closed by relay ({})", c.map(|c| u16::from(c.code)).unwrap_or(0))));
                }
                Some(Ok(_)) => {}
                Some(Err(e)) => return Err(lost(e.to_string())),
                None => return Err(lost("connection lost".into())),
            },
            o = out.recv() => match o {
                Some(bytes) => {
                    ws.send(Message::Binary(Frame::Envelope(bytes).to_bytes().into()))
                        .await
                        .map_err(|e| lost(e.to_string()))?;
                }
                None => return Err(End::Closed),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn targets() {
        assert_eq!(
            target("wss://relay.example/v1/ws"),
            Ok(Target {
                tls: true,
                host: "relay.example".into(),
                port: 443
            })
        );
        assert_eq!(
            target("ws://10.0.0.1:8443/v1/ws").map(|t| (t.tls, t.port)),
            Ok((false, 8443))
        );
        assert!(target("https://relay.example/v1/ws").is_err());
    }
}
