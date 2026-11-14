// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-Muster-app-store-exception

//! HTTP and WebSocket endpoints (`spec/06-relay.md`, Endpoints).
//!
//! Long-poll: `POST /v1/lp/auth` returns a challenge (CBOR); `POST
//! /v1/lp/auth/complete` takes the RelayAuth envelope and returns the auth
//! result, whose `session_id` (hex) is the bearer token; `POST /v1/lp/send`
//! takes a CBOR array of envelopes; `GET /v1/lp/recv?cursor=N&timeout=25`
//! returns a CBOR array and the newest cursor in `X-Muster-Cursor`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::Router;
use axum::body::Bytes;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use muster_proto::Cbor;
use muster_proto::relay::{Frame, decode_batch, encode_batch};
use serde::Deserialize;
use tokio::sync::mpsc;

use crate::state::{AuthError, CLOSE_AUTH_FAILED, Outgoing, Relay, SessionId};

pub const PING_INTERVAL: Duration = Duration::from_secs(20);
const AUTH_TIMEOUT: Duration = Duration::from_secs(10);
const NONCE_TTL_MS: u64 = 60_000;
const LP_MAX_WAIT: u64 = 25;

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn random<const N: usize>() -> [u8; N] {
    let mut b = [0u8; N];
    // An unusable RNG is fatal for a security service.
    if getrandom::fill(&mut b).is_err() {
        tracing::error!("OS random number generator unavailable");
        std::process::abort();
    }
    b
}

#[derive(Debug, Clone)]
pub struct App {
    pub relay: Arc<Mutex<Relay>>,
    pub source_url: String,
    /// Long-poll challenges issued: nonce -> expiry.
    nonces: Arc<Mutex<HashMap<[u8; 32], u64>>>,
}

impl App {
    pub fn new(relay: Relay, source_url: String) -> Self {
        App {
            relay: Arc::new(Mutex::new(relay)),
            source_url,
            nonces: Default::default(),
        }
    }

    fn relay(&self) -> std::sync::MutexGuard<'_, Relay> {
        self.relay.lock().unwrap_or_else(|e| e.into_inner())
    }
}

pub fn router(app: App) -> Router {
    Router::new()
        .route("/v1/ws", get(ws))
        .route("/v1/lp/auth", post(lp_auth))
        .route("/v1/lp/auth/complete", post(lp_complete))
        .route("/v1/lp/send", post(lp_send))
        .route("/v1/lp/recv", get(lp_recv))
        .route("/healthz", get(|| async { "ok" }))
        .route("/metrics", get(metrics))
        .route("/source", get(source))
        .with_state(app)
}

async fn source(State(app): State<App>) -> Redirect {
    Redirect::to(&app.source_url)
}

async fn metrics(State(app): State<App>) -> impl IntoResponse {
    let r = app.relay();
    let m = &r.metrics;
    let body = format!(
        "# TYPE muster_relay_sessions gauge\nmuster_relay_sessions {}\n\
         # TYPE muster_relay_tenants gauge\nmuster_relay_tenants {}\n\
         # TYPE muster_relay_auth_ok_total counter\nmuster_relay_auth_ok_total {}\n\
         # TYPE muster_relay_auth_failed_total counter\nmuster_relay_auth_failed_total {}\n\
         # TYPE muster_relay_envelopes_in_total counter\nmuster_relay_envelopes_in_total {}\n\
         # TYPE muster_relay_envelopes_forwarded_total counter\nmuster_relay_envelopes_forwarded_total {}\n\
         # TYPE muster_relay_dropped_rate_limited_total counter\nmuster_relay_dropped_rate_limited_total {}\n\
         # TYPE muster_relay_dropped_invalid_total counter\nmuster_relay_dropped_invalid_total {}\n\
         # TYPE muster_relay_revocations_total counter\nmuster_relay_revocations_total {}\n",
        r.session_count(),
        r.tenant_count(),
        m.auth_ok,
        m.auth_failed,
        m.envelopes_in,
        m.envelopes_forwarded,
        m.dropped_rate_limited,
        m.dropped_invalid,
        m.revocations
    );
    ([(header::CONTENT_TYPE, "text/plain; version=0.0.4")], body)
}

async fn ws(State(app): State<App>, up: WebSocketUpgrade) -> Response {
    up.max_message_size(muster_proto::limits::RELAY_WS_FRAME_MAX + 1)
        .on_upgrade(move |s| async move {
            if let Err(e) = ws_session(app, s).await {
                tracing::debug!("ws session ended: {e}");
            }
        })
}

async fn close(s: &mut WebSocket, code: u16) {
    let _ = s
        .send(Message::Close(Some(axum::extract::ws::CloseFrame {
            code,
            reason: "".into(),
        })))
        .await;
}

async fn ws_session(app: App, mut sock: WebSocket) -> Result<(), axum::Error> {
    let nonce: [u8; 32] = random();
    let ch = Relay::challenge(nonce, now_ms());
    sock.send(Message::Binary(Frame::Challenge(ch).to_bytes().into()))
        .await?;
    let first = match tokio::time::timeout(AUTH_TIMEOUT, sock.recv()).await {
        Ok(Some(Ok(Message::Binary(b)))) => b,
        _ => {
            close(&mut sock, CLOSE_AUTH_FAILED).await;
            return Ok(());
        }
    };
    let Ok(Frame::Envelope(env)) = Frame::from_bytes(&first) else {
        close(&mut sock, CLOSE_AUTH_FAILED).await;
        return Ok(());
    };
    let session: SessionId = random();
    let (tx, mut rx) = mpsc::unbounded_channel();
    let auth = app
        .relay()
        .authenticate(session, &nonce, &env, Some(tx), now_ms());
    let (result, replay) = match auth {
        Ok(x) => x,
        Err(AuthError(code)) => {
            close(&mut sock, code).await;
            return Ok(());
        }
    };
    sock.send(Message::Binary(Frame::AuthResult(result).to_bytes().into()))
        .await?;
    for e in replay {
        sock.send(Message::Binary(Frame::Envelope(e).to_bytes().into()))
            .await?;
    }
    let mut ping = tokio::time::interval(PING_INTERVAL);
    ping.tick().await;
    let mut missed = 0u32;
    let r = loop {
        tokio::select! {
            m = sock.recv() => match m {
                Some(Ok(Message::Binary(b))) => {
                    match Frame::from_bytes(&b) {
                        Ok(Frame::Envelope(e)) => {
                            if !app.relay().receive(&session, &e, now_ms()) {
                                break Ok(());
                            }
                        }
                        _ => app.relay().metrics.dropped_invalid += 1,
                    }
                }
                Some(Ok(Message::Pong(_))) => missed = 0,
                Some(Ok(Message::Close(_))) | None => break Ok(()),
                Some(Ok(_)) => {}
                Some(Err(e)) => break Err(e),
            },
            o = rx.recv() => match o {
                Some(Outgoing::Frame(f)) => {
                    if let Err(e) = sock.send(Message::Binary(f.to_bytes().into())).await {
                        break Err(e);
                    }
                }
                Some(Outgoing::Close(code)) => {
                    close(&mut sock, code).await;
                    break Ok(());
                }
                None => break Ok(()),
            },
            _ = ping.tick() => {
                missed += 1;
                if missed > 2 {
                    break Ok(());
                }
                if let Err(e) = sock.send(Message::Ping(Bytes::new())).await {
                    break Err(e);
                }
            }
        }
    };
    app.relay().disconnect(&session);
    r
}

async fn lp_auth(State(app): State<App>) -> impl IntoResponse {
    let nonce: [u8; 32] = random();
    let now = now_ms();
    {
        let mut n = app.nonces.lock().unwrap_or_else(|e| e.into_inner());
        n.retain(|_, exp| *exp > now);
        n.insert(nonce, now + NONCE_TTL_MS);
    }
    Relay::challenge(nonce, now).to_cbor()
}

async fn lp_complete(State(app): State<App>, body: Bytes) -> Response {
    // Find which issued nonce this envelope answers: try each live one.
    let now = now_ms();
    let live: Vec<[u8; 32]> = {
        let n = app.nonces.lock().unwrap_or_else(|e| e.into_inner());
        n.iter()
            .filter(|(_, e)| **e > now)
            .map(|(k, _)| *k)
            .collect()
    };
    let nonce = match muster_proto::Envelope::from_cbor(&body)
        .ok()
        .and_then(|e| match e {
            muster_proto::Envelope::Open(o) => {
                muster_proto::SignedMessage::from_cbor(&o.payload).ok()
            }
            _ => None,
        })
        .and_then(|s| muster_proto::Body::from_cbor(&s.body).ok())
        .and_then(|b| match b.payload {
            muster_proto::Payload::RelayAuth(a) => Some(a.nonce),
            _ => None,
        }) {
        Some(n) if live.contains(&n) => n,
        _ => return StatusCode::UNAUTHORIZED.into_response(),
    };
    app.nonces
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&nonce);
    let session: SessionId = random();
    match app.relay().authenticate(session, &nonce, &body, None, now) {
        Ok((result, _)) => result.to_cbor().into_response(),
        Err(AuthError(code)) => (StatusCode::UNAUTHORIZED, code.to_string()).into_response(),
    }
}

fn bearer(h: &HeaderMap) -> Option<SessionId> {
    let v = h.get(header::AUTHORIZATION)?.to_str().ok()?;
    let hex = v.strip_prefix("Bearer ")?;
    let id = muster_proto::MsgId::from_hex(hex)?;
    Some(id.0)
}

async fn lp_send(State(app): State<App>, h: HeaderMap, body: Bytes) -> StatusCode {
    let Some(session) = bearer(&h) else {
        return StatusCode::UNAUTHORIZED;
    };
    let Ok(envs) = decode_batch(&body) else {
        return StatusCode::BAD_REQUEST;
    };
    let now = now_ms();
    let mut r = app.relay();
    for e in envs {
        if !r.receive(&session, &e, now) {
            return StatusCode::UNAUTHORIZED;
        }
    }
    StatusCode::NO_CONTENT
}

#[derive(Debug, Deserialize)]
struct RecvQuery {
    cursor: u64,
    timeout: Option<u64>,
}

async fn lp_recv(State(app): State<App>, h: HeaderMap, Query(q): Query<RecvQuery>) -> Response {
    let Some(session) = bearer(&h) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let wait = Duration::from_secs(q.timeout.unwrap_or(LP_MAX_WAIT).min(LP_MAX_WAIT));
    let deadline = tokio::time::Instant::now() + wait;
    loop {
        let polled = app.relay().poll(&session, q.cursor, now_ms());
        let Some((envs, cursor, notify)) = polled else {
            return StatusCode::UNAUTHORIZED.into_response();
        };
        if !envs.is_empty() || tokio::time::Instant::now() >= deadline {
            return (
                [("x-muster-cursor", cursor.to_string())],
                encode_batch(&envs),
            )
                .into_response();
        }
        // Register for the wake-up before re-checking, so an envelope that
        // arrives in between is not missed.
        let notified = notify.notified();
        tokio::pin!(notified);
        notified.as_mut().enable();
        let again = app.relay().poll(&session, q.cursor, now_ms());
        if again.is_some_and(|(e, _, _)| !e.is_empty()) {
            continue;
        }
        let _ = tokio::time::timeout_at(deadline, notified).await;
    }
}
