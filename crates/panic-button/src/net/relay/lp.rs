// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-PanicButton-app-store-exception

//! HTTP long-poll sessions (`spec/06-relay.md`, Endpoints). The bearer
//! token lasts an hour; on a 401 the client authenticates again without
//! reporting a disconnect.

use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use crate::core::io::RelayLinkState;
use crate::proto::relay::{AuthResult, Challenge, decode_batch, encode_batch};
use crate::proto::{Cbor, DeviceId};
use reqwest::StatusCode;
use tokio::sync::mpsc;
use tokio::time::Instant;

use super::{AUTH_TIMEOUT, Ctx, End};

/// The server holds a poll for up to 25 s.
const POLL_WAIT_S: u64 = 25;
const POLL_TIMEOUT: Duration = Duration::from_secs(POLL_WAIT_S + 15);

pub(super) fn http_client(ctx: &Ctx) -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .tls_backend_preconfigured((*ctx.tls).clone())
        .connect_timeout(AUTH_TIMEOUT)
        .build()
        .map_err(|e| e.to_string())
}

/// `ws://h:p/v1/ws` -> `http://h:p`, `wss://...` -> `https://...`.
fn base_url(ws_url: &str) -> Result<String, String> {
    let uri: http::Uri = ws_url.parse().map_err(|e| format!("bad relay URL: {e}"))?;
    let scheme = match uri.scheme_str() {
        Some("wss") => "https",
        Some("ws") => "http",
        _ => return Err("relay URL must start with ws:// or wss://".into()),
    };
    let auth = uri.authority().ok_or("relay URL has no host")?;
    let path = uri.path().trim_end_matches('/');
    let prefix = path.strip_suffix("/v1/ws").unwrap_or("");
    Ok(format!("{scheme}://{auth}{prefix}"))
}

fn presence(h: &reqwest::header::HeaderMap) -> Option<Vec<DeviceId>> {
    let v = h.get("x-panic-button-presence")?.to_str().ok()?;
    Some(
        v.split(',')
            .filter(|s| !s.is_empty())
            .filter_map(DeviceId::from_hex)
            .collect(),
    )
}

struct Poll {
    envelopes: Vec<Vec<u8>>,
    cursor: u64,
    presence: Option<Vec<DeviceId>>,
}

enum Fail {
    Unauthorized,
    Other(String),
}

type PollFuture = Pin<Box<dyn Future<Output = Result<Poll, Fail>> + Send>>;

fn poll(http: reqwest::Client, base: String, token: String, cursor: u64) -> PollFuture {
    Box::pin(async move {
        let r = http
            .get(format!(
                "{base}/v1/lp/recv?cursor={cursor}&timeout={POLL_WAIT_S}"
            ))
            .bearer_auth(token)
            .timeout(POLL_TIMEOUT)
            .send()
            .await
            .map_err(|e| Fail::Other(e.to_string()))?;
        if r.status() == StatusCode::UNAUTHORIZED {
            return Err(Fail::Unauthorized);
        }
        if !r.status().is_success() {
            return Err(Fail::Other(format!("poll: HTTP {}", r.status())));
        }
        let next = r
            .headers()
            .get("x-panic-button-cursor")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse().ok())
            .unwrap_or(cursor);
        let presence = presence(r.headers());
        let body = r.bytes().await.map_err(|e| Fail::Other(e.to_string()))?;
        let envelopes = decode_batch(&body).map_err(|e| Fail::Other(format!("poll: {e}")))?;
        Ok(Poll {
            envelopes,
            cursor: next,
            presence,
        })
    })
}

async fn send(
    http: &reqwest::Client,
    base: &str,
    token: &str,
    batch: &[Vec<u8>],
) -> Result<(), Fail> {
    let r = http
        .post(format!("{base}/v1/lp/send"))
        .bearer_auth(token)
        .timeout(AUTH_TIMEOUT)
        .body(encode_batch(batch))
        .send()
        .await
        .map_err(|e| Fail::Other(e.to_string()))?;
    match r.status() {
        StatusCode::UNAUTHORIZED => Err(Fail::Unauthorized),
        s if s.is_success() => Ok(()),
        s => Err(Fail::Other(format!("send: HTTP {s}"))),
    }
}

/// Challenge, sign, complete: the bearer token and the auth result.
async fn authenticate(
    ctx: &Ctx,
    http: &reqwest::Client,
    base: &str,
    cursor: Option<u64>,
) -> Result<(String, AuthResult), End> {
    let r = http
        .post(format!("{base}/v1/lp/auth"))
        .timeout(AUTH_TIMEOUT)
        .send()
        .await
        .map_err(|e| End::lost(e.to_string()))?;
    if !r.status().is_success() {
        return Err(End::lost(format!("long-poll auth: HTTP {}", r.status())));
    }
    let body = r.bytes().await.map_err(|e| End::lost(e.to_string()))?;
    let ch = Challenge::from_cbor(&body).map_err(|_| End::lost("bad challenge"))?;
    let env = ctx.sign(ch.nonce, cursor).await?;
    let r = http
        .post(format!("{base}/v1/lp/auth/complete"))
        .timeout(AUTH_TIMEOUT)
        .body(env)
        .send()
        .await
        .map_err(|e| End::lost(e.to_string()))?;
    if !r.status().is_success() {
        let why = r.text().await.unwrap_or_default();
        return Err(End::lost(format!("authentication refused ({why})")));
    }
    let body = r.bytes().await.map_err(|e| End::lost(e.to_string()))?;
    let a = AuthResult::from_cbor(&body).map_err(|_| End::lost("bad auth result"))?;
    if !a.ok {
        return Err(End::lost("authentication refused"));
    }
    let token = a.session_id.iter().map(|b| format!("{b:02x}")).collect();
    Ok((token, a))
}

/// Envelopes queued now, up to one batch.
fn batch(first: Vec<u8>, out: &mut mpsc::UnboundedReceiver<Vec<u8>>) -> Vec<Vec<u8>> {
    let mut size = first.len();
    let mut v = vec![first];
    while size < crate::proto::limits::RELAY_LP_BATCH_MAX / 2 {
        match out.try_recv() {
            Ok(e) => {
                size += e.len();
                v.push(e);
            }
            Err(_) => break,
        }
    }
    v
}

pub(super) async fn session(
    ctx: &Ctx,
    http: &reqwest::Client,
    out: &mut mpsc::UnboundedReceiver<Vec<u8>>,
    cursor: &mut Option<u64>,
    until: Instant,
) -> End {
    let base = match base_url(&ctx.url) {
        Ok(b) => b,
        Err(e) => return End::lost(e),
    };
    let (mut token, first) = match authenticate(ctx, http, &base, *cursor).await {
        Ok(x) => x,
        Err(e) => return e,
    };
    tracing::info!("relay {}: connected (long-poll)", ctx.url);
    let mut shown = first.presence.clone();
    ctx.state(RelayLinkState::Connected, Some(first.presence))
        .await;
    let lost = |why: String| End::Lost {
        why,
        was_connected: true,
    };
    // Without a cursor, read the whole buffer, as a fresh WebSocket
    // session is replayed the last `buffer_hours`.
    let mut at = cursor.unwrap_or(0);
    let mut pending = poll(http.clone(), base.clone(), token.clone(), at);
    loop {
        tokio::select! {
            r = &mut pending => {
                match r {
                    Ok(p) => {
                        for e in p.envelopes {
                            if ctx.frame(e).await.is_err() {
                                return End::Closed;
                            }
                        }
                        at = p.cursor;
                        *cursor = Some(at);
                        if let Some(now) = p.presence
                            && now != shown
                        {
                            shown = now.clone();
                            ctx.state(RelayLinkState::Connected, Some(now)).await;
                        }
                    }
                    Err(Fail::Unauthorized) => match authenticate(ctx, http, &base, *cursor).await {
                        Ok((t, _)) => token = t,
                        Err(End::Lost { why, .. }) => return lost(why),
                        Err(e) => return e,
                    },
                    Err(Fail::Other(e)) => return lost(e),
                }
                pending = poll(http.clone(), base.clone(), token.clone(), at);
            }
            o = out.recv() => {
                let Some(first) = o else { return End::Closed };
                let b = batch(first, out);
                match send(http, &base, &token, &b).await {
                    Ok(()) => {}
                    Err(Fail::Unauthorized) => {
                        match authenticate(ctx, http, &base, *cursor).await {
                            Ok((t, _)) => token = t,
                            Err(End::Lost { why, .. }) => return lost(why),
                            Err(e) => return e,
                        }
                        match send(http, &base, &token, &b).await {
                            Ok(()) => {}
                            Err(Fail::Unauthorized) => {
                                return lost("send refused after re-authenticating".into());
                            }
                            Err(Fail::Other(e)) => return lost(e),
                        }
                    }
                    Err(Fail::Other(e)) => return lost(e),
                }
            }
            _ = tokio::time::sleep_until(until) => return End::Rotate,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_urls() {
        assert_eq!(
            base_url("wss://relay.example/v1/ws").as_deref(),
            Ok("https://relay.example")
        );
        assert_eq!(
            base_url("ws://10.0.0.1:8443/v1/ws").as_deref(),
            Ok("http://10.0.0.1:8443")
        );
        assert_eq!(
            base_url("wss://pcn.example/panic-button/v1/ws").as_deref(),
            Ok("https://pcn.example/panic-button")
        );
        assert!(base_url("http://x/v1/ws").is_err());
    }
}
