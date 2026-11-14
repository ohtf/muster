// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-Muster-app-store-exception

//! The agent runtime: one task owns the [`Core`], feeds it inputs from the
//! network drivers, the control API and timers, and performs its outputs
//! (`spec/02-architecture.md`, Runtime).

use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use muster_core::bootstrap::{enrol_with_token_state, new_device_keys};
use muster_core::io::{AlarmView, AudioCmd, Input, Output, UiEvent, UserAction};
use muster_core::persist::PersistedState;
use muster_core::rng::CoreRng;
use muster_core::{Core, Identity, Mono};
use muster_net::{Net, NetConfig};
use muster_proto::{AudioStatus, DeviceId, MsgId, SessionState, Source, Token};
use muster_store::Store;
use serde_json::{Value, json};
use tokio::sync::{broadcast, mpsc};

use crate::config::ConfigFile;
use crate::control::{self, Request, RpcError};

#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("store: {0}")]
    Store(#[from] muster_store::StoreError),
    #[error("network: {0}")]
    Net(#[from] muster_net::NetError),
    #[error("enrolment token: {0}")]
    Token(String),
    #[error("I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("random number generator unavailable")]
    Rng,
}

#[derive(Debug, Clone)]
pub struct AgentOptions {
    pub data_dir: PathBuf,
    pub control_socket: PathBuf,
    pub config: ConfigFile,
    /// Allow `raise` over the control API (test builds, headless, or policy
    /// `allow_cli_raise`).
    pub allow_raise: bool,
    /// `--label`. Otherwise the label in this device's cert, else the host
    /// name.
    pub label: Option<String>,
    /// No audio device or interactive session: report working audio and an
    /// active session once at start. The desktop app reports real values.
    pub headless: bool,
}

/// A running agent, for the process that embeds it (the desktop app, or
/// the headless binary).
#[derive(Debug, Clone)]
pub struct AgentHandle {
    requests: mpsc::Sender<Request>,
    inputs: mpsc::Sender<Input>,
    events: broadcast::Sender<UiEvent>,
    audio: broadcast::Sender<AudioCmd>,
}

impl AgentHandle {
    /// A control API call in-process (same methods as the socket).
    pub async fn call(&self, method: &str, params: Value) -> Result<Value, RpcError> {
        let (reply, rx) = tokio::sync::oneshot::channel();
        self.requests
            .send(Request {
                method: method.to_owned(),
                params,
                reply,
            })
            .await
            .map_err(|_| RpcError::refused("agent stopped"))?;
        rx.await
            .unwrap_or_else(|_| Err(RpcError::refused("agent stopped")))
    }

    /// Platform inputs: audio status, session state, user display name.
    pub async fn input(&self, i: Input) {
        let _ = self.inputs.send(i).await;
    }

    pub fn events(&self) -> broadcast::Receiver<UiEvent> {
        self.events.subscribe()
    }

    pub fn audio(&self) -> broadcast::Receiver<AudioCmd> {
        self.audio.subscribe()
    }
}

pub fn host_name() -> String {
    std::env::var("HOSTNAME")
        .ok()
        .or_else(|| std::fs::read_to_string("/etc/hostname").ok())
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "muster-agent".into())
}

/// The label in this device's own cert, if enrolled.
fn cert_label(state: &PersistedState) -> Option<String> {
    let own = state.own_serial?;
    state
        .certs
        .iter()
        .filter_map(|raw| {
            <muster_proto::Cert as muster_proto::Cbor>::from_cbor(raw)
                .ok()?
                .body()
                .ok()
        })
        .find(|b| b.serial == own)
        .map(|b| b.label)
        .filter(|l| !l.is_empty())
}

/// The device label shown to others.
fn effective_label(opts: &AgentOptions, state: &PersistedState) -> String {
    opts.label
        .clone()
        .or_else(|| cert_label(state))
        .unwrap_or_else(host_name)
}

pub fn os_seed() -> Result<[u8; 32], AgentError> {
    let mut s = [0u8; 32];
    getrandom::fill(&mut s).map_err(|_| AgentError::Rng)?;
    Ok(s)
}

fn wall_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Opens the store, loads or creates the identity, and enrols with a token
/// if one is configured and the device is not yet enrolled.
pub fn prepare(opts: &AgentOptions) -> Result<(Store, Identity, PersistedState), AgentError> {
    std::fs::create_dir_all(&opts.data_dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&opts.data_dir, std::fs::Permissions::from_mode(0o700))?;
    }
    let mut store = Store::open(&opts.data_dir.join("muster.db"))?;
    let mut rng = CoreRng::from_seed(os_seed()?);
    let keys = match store.load_identity()? {
        Some(k) => k,
        None => {
            let k = new_device_keys(&mut rng);
            store.save_identity(&k)?;
            tracing::info!("created device identity {}", k.device_id());
            k
        }
    };
    let mut state = store.load()?;
    if state.practice.is_none() {
        let token_text = match (
            &opts.config.enrolment.token,
            &opts.config.enrolment.token_file,
        ) {
            (Some(t), _) => Some(t.clone()),
            (None, Some(path)) => match std::fs::read_to_string(path) {
                Ok(t) => Some(t),
                Err(e) => {
                    tracing::warn!("cannot read token file {}: {e}", path.display());
                    None
                }
            },
            (None, None) => None,
        };
        if let Some(text) = token_text {
            let token = Token::parse(&text).map_err(|e| AgentError::Token(e.to_string()))?;
            let label = opts.label.clone().unwrap_or_else(host_name);
            let enrolled = enrol_with_token_state(&mut rng, &keys, &token, &label, wall_now())
                .map_err(|e| AgentError::Token(e.to_string()))?;
            store.apply_all(enrolled.to_ops().iter())?;
            state = store.load()?;
            tracing::info!("enrolled in {} with token", token.practice_name);
            // Blank the token where we can (`spec/04-security.md`, flow B).
            if let Some(path) = &opts.config.enrolment.token_file
                && std::fs::write(path, b"").is_err()
            {
                tracing::info!(
                    "could not blank {}; it remains valid until it expires",
                    path.display()
                );
            }
        }
    }
    Ok((store, Identity { keys }, state))
}

struct Runtime {
    core: Core,
    store: Store,
    net: Net,
    relays: muster_net::relay::RelayClients,
    start: Instant,
    events: broadcast::Sender<UiEvent>,
    audio: broadcast::Sender<AudioCmd>,
    allow_raise: bool,
    audit_failures: u64,
    local: muster_core::config::LocalConfig,
}

impl Runtime {
    fn now(&self) -> Mono {
        Mono(self.start.elapsed().as_millis() as u64)
    }

    fn handle(&mut self, input: Input) {
        let now = self.now();
        let outs = self.core.handle(now, wall_now(), input);
        for o in outs {
            self.output(o);
        }
        if self.core.needs_reseed()
            && let Ok(s) = os_seed()
        {
            self.core.reseed(s);
        }
        self.net.set_mdns_tag(self.core.practice_tag().map(|t| t.0));
    }

    fn output(&mut self, o: Output) {
        match &o {
            Output::SendDatagram { .. } | Output::SendStream { .. } => self.net.send(&o),
            Output::SendRelay { relay, bytes } => self.relays.send(*relay, bytes),
            Output::Persist(op) => {
                if let Err(e) = self.store.apply(op) {
                    tracing::error!("persist failed: {e}");
                }
            }
            Output::Ui(e) => {
                tracing::debug!("ui: {e:?}");
                if let UiEvent::DrillReport(r) = e
                    && let Err(err) = self.store.save_drill_report(r)
                {
                    tracing::error!("saving drill report: {err}");
                }
                let _ = self.events.send(e.clone());
            }
            Output::Audio(a) => {
                tracing::debug!("audio: {a:?}");
                let _ = self.audio.send(*a);
            }
            Output::SetSystemVolumeOverride(on) => tracing::info!("volume override: {on}"),
            Output::Log(e) => {
                if self.store.audit(wall_now(), e).is_err() {
                    self.audit_failures += 1;
                }
            }
        }
    }

    fn control(&mut self, req: Request) {
        let r = self.dispatch(&req.method, &req.params);
        let _ = req.reply.send(r);
    }

    fn dispatch(&mut self, method: &str, p: &Value) -> Result<Value, RpcError> {
        let alarm = |p: &Value| -> Result<MsgId, RpcError> {
            p.get("alarm_id")
                .and_then(Value::as_str)
                .and_then(MsgId::from_hex)
                .ok_or_else(|| RpcError::invalid_params("alarm_id (32 hex characters) required"))
        };
        let device = |p: &Value| -> Result<DeviceId, RpcError> {
            p.get("device")
                .and_then(Value::as_str)
                .and_then(DeviceId::from_hex)
                .ok_or_else(|| RpcError::invalid_params("device (64 hex characters) required"))
        };
        let wall = wall_now();
        // A display string param: at most 64 bytes (`spec/03-protocol.md`).
        let text = |p: &Value, key: &str| -> Result<Option<String>, RpcError> {
            match p.get(key) {
                None | Some(Value::Null) => Ok(None),
                Some(Value::String(s)) if s.trim().len() <= 64 => Ok(Some(s.trim().to_owned())),
                Some(Value::String(_)) => Err(RpcError::invalid_params(format!(
                    "{key} is at most 64 bytes"
                ))),
                _ => Err(RpcError::invalid_params(format!("{key} must be text"))),
            }
        };
        let source = |p: &Value| -> Result<Source, RpcError> {
            match p.get("source").and_then(Value::as_str).unwrap_or("button") {
                "button" => Ok(Source::Button),
                "hotkey" => Ok(Source::Hotkey),
                s => Err(RpcError::invalid_params(format!("unknown source {s}"))),
            }
        };
        match method {
            "status" => Ok(crate::views::status(&self.core, self.now(), wall)),
            "hold.start" => self.user(UserAction::HoldStart {
                source: source(p)?,
                practice: p.get("practice").and_then(Value::as_bool).unwrap_or(false),
            }),
            "hold.end" => self.user(UserAction::HoldEnd { source: source(p)? }),
            "config" => {
                let c = self.core.config();
                Ok(json!({
                    "label": c.label,
                    "room": c.room,
                    "hold_ms": c.hold_ms,
                    "cue_mode": format!("{:?}", c.cue_mode).to_lowercase(),
                    "sound_pattern": format!("{:?}", c.sound_pattern),
                    "volume_pct": c.volume_pct,
                    "relay_urls": c.relay_urls,
                    "label_locked": self.local.label_locked,
                    "room_locked": self.local.room_locked,
                }))
            }
            "device.set_room" | "device.set_label" => {
                let (key, locked) = if method == "device.set_room" {
                    ("room", self.local.room_locked)
                } else {
                    ("label", self.local.label_locked)
                };
                let v = Some(
                    text(p, key)?
                        .ok_or_else(|| RpcError::invalid_params(format!("{key} required")))?,
                );
                if locked {
                    return Err(RpcError::refused(format!(
                        "the {key} is set by this computer's configuration"
                    )));
                }
                let (label, room) = if key == "room" { (None, v) } else { (v, None) };
                self.user(UserAction::SetOwnSettings { label, room })
            }
            "admin.device_settings" => {
                let d = device(p)?;
                let label = text(p, "label").ok().flatten();
                let room = text(p, "room").ok().flatten();
                if label.is_none() && room.is_none() {
                    return Err(RpcError::invalid_params("label or room required"));
                }
                self.user(UserAction::AssignDeviceSettings {
                    device: d,
                    label,
                    room,
                })
            }
            "peers" => Ok(crate::views::peers(&self.core, self.now(), wall)),
            "alarms" => Ok(Value::Array(
                self.core
                    .alarm_views(wall)
                    .iter()
                    .map(crate::views::alarm)
                    .collect(),
            )),
            "diagnostics" => Ok(crate::views::diagnostics(&self.core, self.audit_failures)),
            "raise" => {
                if !self.allow_raise {
                    return Err(RpcError::refused(
                        "raising from the control API is disabled (policy allow_cli_raise)",
                    ));
                }
                self.handle(Input::User(UserAction::RaiseNow {
                    source: Source::Cli,
                }));
                let own = self
                    .core
                    .alarm_views(wall)
                    .into_iter()
                    .filter(|v| v.own && !v.drill)
                    .max_by_key(|v| v.raised_at);
                Ok(own.map(|v| crate::views::alarm(&v)).unwrap_or(Value::Null))
            }
            "respond" => self.user(UserAction::Respond {
                alarm_id: alarm(p)?,
            }),
            "seen" => self.user(UserAction::Seen {
                alarm_id: alarm(p)?,
            }),
            "resolve" => self.user(UserAction::Resolve {
                alarm_id: alarm(p)?,
            }),
            "cancel" => self.user(UserAction::Cancel {
                alarm_id: alarm(p)?,
            }),
            "drill.run" => self.user(UserAction::RunDrill),
            "drill.reports" => {
                let rs = self
                    .store
                    .drill_reports()
                    .map_err(|e| RpcError::refused(e.to_string()))?;
                Ok(Value::Array(
                    rs.iter()
                        .map(|r| {
                            json!({
                                "alarm_id": r.alarm_id.to_hex(),
                                "started_at": r.started_at,
                                "received": r.received,
                                "expected": r.expected,
                            })
                        })
                        .collect(),
                ))
            }
            "drill.report" => {
                let id = alarm(p)?;
                let rs = self
                    .store
                    .drill_reports()
                    .map_err(|e| RpcError::refused(e.to_string()))?;
                let r = rs
                    .iter()
                    .find(|r| r.alarm_id == id)
                    .ok_or_else(|| RpcError::refused("no report for that drill on this device"))?;
                Ok(json!({
                    "alarm_id": r.alarm_id.to_hex(),
                    "started_at": r.started_at,
                    "received": r.received,
                    "expected": r.expected,
                    "csv": r.csv,
                }))
            }
            "devices" => Ok(crate::views::devices(&self.core, self.now(), wall)),
            "tokens" => Ok(crate::views::tokens(&self.core, wall)),
            "token.revoke" => {
                let id = p
                    .get("token_id")
                    .and_then(Value::as_str)
                    .and_then(muster_proto::TokenId::from_hex)
                    .ok_or_else(|| {
                        RpcError::invalid_params("token_id (32 hex characters) required")
                    })?;
                let enroller = self
                    .core
                    .trust()
                    .and_then(|t| {
                        t.certs()
                            .find(|(_, b)| {
                                b.role == muster_proto::Role::Enroller && b.token_id == Some(id)
                            })
                            .map(|(_, b)| b.subject)
                    })
                    .ok_or_else(|| RpcError::refused("no token with that id"))?;
                // Revoking the enroller stops new enrolments; devices it
                // already enrolled stay members (ADR 0012).
                self.user(UserAction::Revoke {
                    device: enroller,
                    cascade: false,
                    reason: "Enrolment token revoked".into(),
                })
            }
            "admin.revoke" => self.user(UserAction::Revoke {
                device: device(p)?,
                cascade: p.get("cascade").and_then(Value::as_bool).unwrap_or(false),
                reason: p
                    .get("reason")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_owned(),
            }),
            "admin.approve_join" => self.user(UserAction::ApproveJoin { device: device(p)? }),
            "admin.reject_join" => self.user(UserAction::RejectJoin { device: device(p)? }),
            "admin.promote" => self.user(UserAction::Promote { device: device(p)? }),
            _ => Err(RpcError::not_found(method)),
        }
    }

    fn user(&mut self, a: UserAction) -> Result<Value, RpcError> {
        self.handle(Input::User(a));
        Ok(json!({"ok": true}))
    }
}

/// Waits for the next alert (`wait_alarm`), answered outside the core task.
async fn wait_alarm(
    mut rx: broadcast::Receiver<UiEvent>,
    timeout: Duration,
) -> Result<Value, RpcError> {
    let r = tokio::time::timeout(timeout, async move {
        loop {
            match rx.recv().await {
                Ok(UiEvent::AlarmShown { alarm, .. }) => return Ok(alarm),
                Ok(_) => {}
                Err(broadcast::error::RecvError::Lagged(_)) => {}
                Err(broadcast::error::RecvError::Closed) => return Err(()),
            }
        }
    })
    .await;
    match r {
        Ok(Ok(a)) => Ok(crate::views::alarm(&a as &AlarmView)),
        Ok(Err(())) => Err(RpcError::refused("agent stopping")),
        Err(_) => Err(RpcError::refused("timed out waiting for an alarm")),
    }
}

/// Runs the agent until the process is stopped (Ctrl-C) or the agent
/// task ends.
pub async fn run(opts: AgentOptions) -> Result<(), AgentError> {
    let (handle, task) = start(opts).await?;
    let _keep = handle;
    tokio::select! {
        _ = task => Ok(()),
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("stopping");
            Ok(())
        }
    }
}

/// Starts the agent on the current tokio runtime. Returns a handle and the
/// agent task; dropping every handle does not stop the agent.
pub async fn start(
    opts: AgentOptions,
) -> Result<(AgentHandle, tokio::task::JoinHandle<()>), AgentError> {
    let (store, identity, state) = prepare(&opts)?;
    let label = effective_label(&opts, &state);
    let local = opts.config.local_config(&label);
    let (in_tx, in_rx) = mpsc::channel::<Input>(4096);
    let net = Net::start(
        NetConfig {
            port: opts.config.port(),
            multicast_group: opts
                .config
                .network
                .multicast_group
                .unwrap_or_else(|| muster_proto::limits::DEFAULT_MCAST_GROUP.into()),
            multicast_ttl: u32::from(opts.config.network.multicast_ttl.unwrap_or(1)),
            mdns: opts.config.network.mdns.unwrap_or(true),
        },
        in_tx.clone(),
    )
    .await?;
    if net.tcp_port() != opts.config.port() {
        tracing::warn!(
            "TCP listening on {} rather than {}; peers reach us by UDP",
            net.tcp_port(),
            opts.config.port()
        );
    }
    let core = Core::new(identity, state, local.clone(), os_seed()?);
    let (auth_tx, auth_rx) = mpsc::channel::<muster_net::relay::AuthRequest>(16);
    let relays = muster_net::relay::RelayClients::start(
        &opts.config.network.relay_urls,
        opts.config.network.relay_ca_pem.as_deref(),
        in_tx.clone(),
        auth_tx,
    );
    tracing::info!("device {} starting", core.device_id());
    let (events, _) = broadcast::channel(1024);
    let (audio, _) = broadcast::channel(64);
    let mut rt = Runtime {
        core,
        store,
        net,
        relays,
        start: Instant::now(),
        events: events.clone(),
        audio: audio.clone(),
        allow_raise: opts.allow_raise || opts.config.policy.allow_cli_raise.unwrap_or(false),
        audit_failures: 0,
        local,
    };
    if opts.headless {
        rt.handle(Input::AudioStatus {
            status: AudioStatus {
                output_present: true,
                muted: false,
                volume_pct: 100,
            },
            override_available: false,
        });
        rt.handle(Input::SessionState(SessionState::Active));
    }
    rt.handle(Input::UserDisplay(label));

    let (ctl_tx, ctl_rx) = mpsc::channel::<Request>(64);
    let path = opts.control_socket.clone();
    let socket_tx = ctl_tx.clone();
    tokio::spawn(async move {
        if let Err(e) = control::serve(&path, socket_tx).await {
            tracing::error!("control API: {e}");
        }
    });
    let handle = AgentHandle {
        requests: ctl_tx,
        inputs: in_tx,
        events,
        audio,
    };
    // Drill reports are kept for 400 days (spec/02-architecture.md).
    let cutoff = wall_now().saturating_sub(400 * 24 * 60 * 60 * 1000);
    if let Err(e) = rt.store.prune_drill_reports(cutoff) {
        tracing::warn!("pruning drill reports: {e}");
    }
    let task = tokio::spawn(agent_loop(rt, in_rx, ctl_rx, auth_rx));
    Ok((handle, task))
}

async fn agent_loop(
    mut rt: Runtime,
    mut in_rx: mpsc::Receiver<Input>,
    mut ctl_rx: mpsc::Receiver<Request>,
    mut auth_rx: mpsc::Receiver<muster_net::relay::AuthRequest>,
) {
    let mut ifaces = tokio::time::interval(Duration::from_secs(5));
    loop {
        let deadline = rt
            .core
            .next_deadline()
            .map(|d| rt.start + Duration::from_millis(d.0))
            .unwrap_or_else(|| Instant::now() + Duration::from_secs(1));
        tokio::select! {
            Some(input) = in_rx.recv() => rt.handle(input),
            Some(req) = ctl_rx.recv() => {
                if req.method == "wait_alarm" {
                    let ms = req.params.get("timeout_ms").and_then(Value::as_u64).unwrap_or(10_000);
                    let rx = rt.events.subscribe();
                    tokio::spawn(async move {
                        let r = wait_alarm(rx, Duration::from_millis(ms)).await;
                        let _ = req.reply.send(r);
                    });
                } else {
                    rt.control(req);
                }
            }
            Some(a) = auth_rx.recv() => {
                let env = rt.core.relay_auth(a.nonce, a.resume_cursor, wall_now());
                let _ = a.reply.send(env);
            }
            _ = tokio::time::sleep_until(deadline.into()) => rt.handle(Input::Tick),
            _ = ifaces.tick() => {
                if let Some(i) = rt.net.poll_interfaces() {
                    rt.handle(i);
                }
            }
        }
    }
}
