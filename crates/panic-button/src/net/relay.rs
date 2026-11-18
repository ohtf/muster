// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-PanicButton-app-store-exception

//! The relay client (`spec/06-relay.md`, Client behaviour): one task per
//! configured relay URL, reconnecting with full-jitter back-off from 1 s
//! doubling to 30 s. Each challenge is signed by the runtime, which owns
//! the keys.
//!
//! WebSocket is preferred. If the upgrade fails (an HTTP error from a
//! proxy, a handshake a middlebox broke, or silence after the upgrade), the
//! client uses HTTP long-poll for that relay for an hour, then tries
//! WebSocket again.
//!
//! Proxies come from `HTTPS_PROXY` / `HTTP_PROXY` / `ALL_PROXY` /
//! `NO_PROXY`, or the system settings on Windows and macOS. WebSocket goes
//! through HTTP CONNECT (Basic auth from the proxy URL); long-poll uses
//! reqwest's proxy support. TLS trusts the system roots plus the optional
//! `relay_ca_pem`.
//!
//! Diagnostics: `PANIC_BUTTON_RELAY_TRANSPORT=long-poll` skips WebSocket
//! entirely (used by the netns tests, and to rule WebSocket out when a
//! proxy misbehaves).

mod lp;
mod proxy;
mod ws;

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use crate::core::io::{Input, RelayId, RelayLinkState};
use tokio::sync::{mpsc, oneshot};
use tokio::time::Instant;

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
/// The relay pings every 20 s: silence for a minute means the path is gone
/// even if TCP has not noticed.
const READ_TIMEOUT: Duration = Duration::from_secs(60);
const LONG_POLL_FOR: Duration = Duration::from_secs(60 * 60);

/// How a session ended.
#[derive(Debug)]
enum End {
    /// The runtime has gone: stop.
    Closed,
    /// WebSocket could not be established through this path: use long-poll.
    Upgrade(String),
    /// The long-poll hour is over: try WebSocket again.
    Rotate,
    /// Anything else: report disconnected and back off.
    Lost { why: String, was_connected: bool },
}

impl End {
    fn lost(why: impl Into<String>) -> Self {
        End::Lost {
            why: why.into(),
            was_connected: false,
        }
    }
}

/// What a session needs.
struct Ctx {
    id: RelayId,
    url: String,
    inputs: mpsc::Sender<Input>,
    auth: mpsc::Sender<AuthRequest>,
    tls: Arc<rustls::ClientConfig>,
    proxy: hyper_util::client::proxy::matcher::Matcher,
    force_long_poll: bool,
}

impl Ctx {
    async fn sign(&self, nonce: [u8; 32], resume_cursor: Option<u64>) -> Result<Vec<u8>, End> {
        let (reply, rx) = oneshot::channel();
        self.auth
            .send(AuthRequest {
                relay: self.id,
                nonce,
                resume_cursor,
                reply,
            })
            .await
            .map_err(|_| End::Closed)?;
        rx.await
            .ok()
            .flatten()
            .ok_or_else(|| End::lost("not enrolled; cannot authenticate"))
    }

    async fn state(&self, state: RelayLinkState, presence: Option<Vec<crate::proto::DeviceId>>) {
        let _ = self
            .inputs
            .send(Input::RelayState {
                relay: self.id,
                state,
                presence,
            })
            .await;
    }

    async fn frame(&self, bytes: Vec<u8>) -> Result<(), End> {
        self.inputs
            .send(Input::RelayFrame {
                relay: self.id,
                bytes,
            })
            .await
            .map_err(|_| End::Closed)
    }
}

/// TLS settings: the system roots plus `ca_pem` if given. A CA file that
/// cannot be read is logged and skipped rather than disabling the relay.
pub fn tls_config(ca_pem: Option<&Path>) -> Arc<rustls::ClientConfig> {
    use rustls_pki_types::CertificateDer;
    use rustls_pki_types::pem::PemObject;
    let mut roots = rustls::RootCertStore::empty();
    let native = rustls_native_certs::load_native_certs();
    for e in &native.errors {
        tracing::warn!("system root certificates: {e}");
    }
    let (_, ignored) = roots.add_parsable_certificates(native.certs);
    if ignored > 0 {
        tracing::debug!("{ignored} system root certificates ignored");
    }
    if let Some(p) = ca_pem {
        match CertificateDer::pem_file_iter(p) {
            Ok(it) => {
                let certs: Vec<_> = it.filter_map(Result::ok).collect();
                let (added, _) = roots.add_parsable_certificates(certs);
                if added == 0 {
                    tracing::warn!("relay_ca_pem {}: no certificates found", p.display());
                }
            }
            Err(e) => tracing::warn!("relay_ca_pem {}: {e}", p.display()),
        }
    }
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let cfg = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map(|b| b.with_root_certificates(roots).with_no_client_auth())
        .unwrap_or_else(|e| {
            // The ring provider supports the default versions; this is
            // unreachable in practice.
            tracing::error!("TLS configuration: {e}");
            rustls::ClientConfig::builder()
                .with_root_certificates(rustls::RootCertStore::empty())
                .with_no_client_auth()
        });
    Arc::new(cfg)
}

impl RelayClients {
    pub fn start(
        urls: &[String],
        ca_pem: Option<&Path>,
        inputs: mpsc::Sender<Input>,
        auth: mpsc::Sender<AuthRequest>,
    ) -> Self {
        let mut senders = Vec::new();
        if urls.is_empty() {
            return RelayClients { senders };
        }
        let tls = tls_config(ca_pem);
        let force_long_poll =
            std::env::var("PANIC_BUTTON_RELAY_TRANSPORT").is_ok_and(|v| v == "long-poll");
        for (i, url) in urls.iter().enumerate() {
            let (tx, rx) = mpsc::unbounded_channel();
            senders.push(tx);
            let ctx = Ctx {
                id: i as RelayId,
                url: url.clone(),
                inputs: inputs.clone(),
                auth: auth.clone(),
                tls: tls.clone(),
                proxy: hyper_util::client::proxy::matcher::Matcher::from_system(),
                force_long_poll,
            };
            tokio::spawn(client(ctx, rx));
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

async fn client(ctx: Ctx, mut out: mpsc::UnboundedReceiver<Vec<u8>>) {
    let mut backoff = BACKOFF_MIN;
    let mut cursor: Option<u64> = None;
    let mut long_poll_until: Option<Instant> = ctx
        .force_long_poll
        .then(|| Instant::now() + Duration::from_secs(365 * 24 * 3600));
    let http = match lp::http_client(&ctx) {
        Ok(h) => Some(h),
        Err(e) => {
            tracing::warn!("relay {}: long-poll unavailable: {e}", ctx.url);
            None
        }
    };
    loop {
        let lp_active = long_poll_until.filter(|t| Instant::now() < *t);
        let end = match (lp_active, &http) {
            (Some(until), Some(h)) => lp::session(&ctx, h, &mut out, &mut cursor, until).await,
            _ => ws::session(&ctx, &mut out, &mut cursor).await,
        };
        match end {
            End::Closed => return,
            End::Upgrade(why) if http.is_some() && lp_active.is_none() => {
                tracing::info!(
                    "relay {}: WebSocket unavailable ({why}); using long-poll for an hour",
                    ctx.url
                );
                long_poll_until = Some(Instant::now() + LONG_POLL_FOR);
                continue;
            }
            End::Rotate => {
                long_poll_until = None;
                continue;
            }
            End::Upgrade(why) => {
                tracing::info!("relay {}: {why}", ctx.url);
            }
            End::Lost { why, was_connected } => {
                tracing::info!("relay {}: {why}", ctx.url);
                if was_connected {
                    backoff = BACKOFF_MIN;
                }
            }
        }
        ctx.state(RelayLinkState::Disconnected, None).await;
        // Drop what was queued while down: the core resends what matters,
        // and the relay replays what we missed.
        while out.try_recv().is_ok() {}
        tokio::time::sleep(Duration::from_millis(jitter(backoff))).await;
        backoff = (backoff * 2).min(BACKOFF_MAX);
    }
}
