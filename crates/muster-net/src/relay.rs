// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-Muster-app-store-exception

//! The relay client (`spec/06-relay.md`, Client behaviour): one task per
//! configured relay URL, over WebSocket (TLS with the system roots for
//! `wss://`), reconnecting with full-jitter back-off from 1 s doubling to
//! 30 s. Each challenge is signed by the runtime, which owns the keys.
//!
//! Long-poll fallback and HTTP proxies are not yet implemented (R37).

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use muster_core::io::{Input, RelayId, RelayLinkState};
use muster_proto::relay::Frame;
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::tungstenite::Message;

/// A request to sign a relay challenge.
#[derive(Debug)]
pub struct AuthRequest {
    pub relay: RelayId,
    pub nonce: [u8; 32],
    pub resume_cursor: Option<u64>,
    pub reply: oneshot::Sender<Option<Vec<u8>>>,
}

/// Handle to the running relay clients.
#[derive(Debug, Default)]
pub struct RelayClients {
    senders: Vec<mpsc::UnboundedSender<Vec<u8>>>,
}

const BACKOFF_MIN: u64 = 1_000;
const BACKOFF_MAX: u64 = 30_000;
const AUTH_TIMEOUT: Duration = Duration::from_secs(10);
const READ_TIMEOUT: Duration = Duration::from_secs(60);

impl RelayClients {
    pub fn start(
        urls: &[String],
        inputs: mpsc::Sender<Input>,
        auth: mpsc::Sender<AuthRequest>,
    ) -> Self {
        let mut senders = Vec::new();
        for (i, url) in urls.iter().enumerate() {
            let (tx, rx) = mpsc::unbounded_channel();
            senders.push(tx);
            let url = url.clone();
            let inputs = inputs.clone();
            let auth = auth.clone();
            tokio::spawn(client(i as RelayId, url, rx, inputs, auth));
        }
        RelayClients { senders }
    }

    /// Sends an envelope to one relay, or all (`None`).
    pub fn send(&self, relay: Option<RelayId>, bytes: &[u8]) {
        for (i, s) in self.senders.iter().enumerate() {
            if relay.is_none_or(|r| r as usize == i) {
                let _ = s.send(bytes.to_vec());
            }
        }
    }
}

fn jitter(max: u64) -> u64 {
    let mut b = [0u8; 8];
    let _ = getrandom::fill(&mut b);
    u64::from_le_bytes(b) % max.max(1)
}

async fn client(
    id: RelayId,
    url: String,
    mut out: mpsc::UnboundedReceiver<Vec<u8>>,
    inputs: mpsc::Sender<Input>,
    auth: mpsc::Sender<AuthRequest>,
) {
    let mut backoff = BACKOFF_MIN;
    let mut cursor: Option<u64> = None;
    loop {
        match session(id, &url, &mut out, &inputs, &auth, &mut cursor).await {
            Ok(()) => backoff = BACKOFF_MIN,
            Err(e) => tracing::info!("relay {url}: {e}"),
        }
        let _ = inputs
            .send(Input::RelayState {
                relay: id,
                state: RelayLinkState::Disconnected,
                presence: None,
            })
            .await;
        // Drop what was queued while down: the core resends what matters,
        // and the relay replays what we missed.
        while out.try_recv().is_ok() {}
        tokio::time::sleep(Duration::from_millis(jitter(backoff))).await;
        backoff = (backoff * 2).min(BACKOFF_MAX);
    }
}

async fn session(
    id: RelayId,
    url: &str,
    out: &mut mpsc::UnboundedReceiver<Vec<u8>>,
    inputs: &mpsc::Sender<Input>,
    auth: &mpsc::Sender<AuthRequest>,
    cursor: &mut Option<u64>,
) -> Result<(), String> {
    let (mut ws, _) = tokio::time::timeout(AUTH_TIMEOUT, tokio_tungstenite::connect_async(url))
        .await
        .map_err(|_| "connect timeout".to_string())?
        .map_err(|e| e.to_string())?;
    let first = tokio::time::timeout(AUTH_TIMEOUT, ws.next())
        .await
        .map_err(|_| "no challenge".to_string())?;
    let Some(Ok(Message::Binary(b))) = first else {
        return Err("no challenge".into());
    };
    let Ok(Frame::Challenge(ch)) = Frame::from_bytes(&b) else {
        return Err("bad challenge".into());
    };
    let (reply, rx) = oneshot::channel();
    auth.send(AuthRequest {
        relay: id,
        nonce: ch.nonce,
        resume_cursor: *cursor,
        reply,
    })
    .await
    .map_err(|_| "runtime gone".to_string())?;
    let env = rx
        .await
        .ok()
        .flatten()
        .ok_or_else(|| "not enrolled; cannot authenticate".to_string())?;
    ws.send(Message::Binary(Frame::Envelope(env).to_bytes().into()))
        .await
        .map_err(|e| e.to_string())?;
    let r = tokio::time::timeout(AUTH_TIMEOUT, ws.next())
        .await
        .map_err(|_| "no auth result".to_string())?;
    let result = match r {
        Some(Ok(Message::Binary(b))) => match Frame::from_bytes(&b) {
            Ok(Frame::AuthResult(a)) if a.ok => a,
            _ => return Err("authentication refused".into()),
        },
        Some(Ok(Message::Close(c))) => {
            return Err(format!(
                "authentication refused ({})",
                c.map(|c| u16::from(c.code)).unwrap_or(0)
            ));
        }
        _ => return Err("authentication failed".into()),
    };
    *cursor = Some(result.cursor);
    tracing::info!("relay {url}: connected");
    let _ = inputs
        .send(Input::RelayState {
            relay: id,
            state: RelayLinkState::Connected,
            presence: Some(result.presence),
        })
        .await;
    loop {
        tokio::select! {
            // The relay pings every 20 s: silence for a minute means the
            // path is gone even if TCP has not noticed.
            m = tokio::time::timeout(READ_TIMEOUT, ws.next()) => match m.map_err(|_| "relay silent".to_string())? {
                Some(Ok(Message::Binary(b))) => match Frame::from_bytes(&b) {
                    Ok(Frame::Envelope(e)) => {
                        if inputs.send(Input::RelayFrame { relay: id, bytes: e }).await.is_err() {
                            return Ok(());
                        }
                    }
                    Ok(Frame::Presence(p)) => {
                        let _ = inputs
                            .send(Input::RelayState {
                                relay: id,
                                state: RelayLinkState::Connected,
                                presence: Some(p.connected),
                            })
                            .await;
                    }
                    _ => {}
                },
                Some(Ok(Message::Ping(p))) => {
                    ws.send(Message::Pong(p)).await.map_err(|e| e.to_string())?;
                }
                Some(Ok(Message::Close(c))) => {
                    return Err(format!("closed by relay ({})", c.map(|c| u16::from(c.code)).unwrap_or(0)));
                }
                Some(Ok(_)) => {}
                Some(Err(e)) => return Err(e.to_string()),
                None => return Err("connection lost".into()),
            },
            o = out.recv() => match o {
                Some(bytes) => {
                    ws.send(Message::Binary(Frame::Envelope(bytes).to_bytes().into()))
                        .await
                        .map_err(|e| e.to_string())?;
                }
                None => return Ok(()),
            },
        }
    }
}
