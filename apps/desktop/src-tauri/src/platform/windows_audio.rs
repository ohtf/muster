// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-PanicButton-app-store-exception

//! The Windows audio endpoint (`IAudioEndpointVolume`) for the default
//! output: its mute and volume for the audio status, and the mute override
//! (`alarm.override_mute`, `spec/05-desktop-agent.md`): while an alarm
//! sounds, unmute and raise the volume to at least 60%, then put both back
//! exactly as they were.
//!
//! COM objects live on one thread of their own, initialised for COM there;
//! the rest of the app talks to it over a channel.

#![allow(unsafe_code)]

use std::sync::mpsc;
use std::time::Duration;

use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{IMMDeviceEnumerator, MMDeviceEnumerator, eConsole, eRender};
use windows::Win32::System::Com::{
    CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
};

use crate::volume::Level;

/// The lowest volume while the override is on.
const OVERRIDE_MIN: f32 = 0.6;

enum Req {
    Read(mpsc::Sender<Option<Level>>),
    Override(bool),
}

/// Handle to the endpoint thread.
#[derive(Debug, Clone)]
pub struct Endpoint {
    tx: mpsc::Sender<Req>,
}

impl Endpoint {
    /// Starts the thread. `None` if COM or the device enumerator is not
    /// available.
    pub fn start() -> Option<Self> {
        let (tx, rx) = mpsc::channel::<Req>();
        let (ready_tx, ready_rx) = mpsc::channel::<bool>();
        std::thread::Builder::new()
            .name("audio-endpoint".into())
            .spawn(move || {
                // SAFETY: called once, first, on this thread, which then owns
                // every COM object it creates; all COM calls below are made on
                // this thread.
                let init = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
                if init.is_err() {
                    tracing::warn!("audio endpoint: COM: {init:?}");
                    let _ = ready_tx.send(false);
                    return;
                }
                // SAFETY: COM is initialised on this thread; the CLSID and the
                // interface type match (MMDeviceEnumerator / IMMDeviceEnumerator).
                let enumerator: windows::core::Result<IMMDeviceEnumerator> =
                    unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) };
                let enumerator = match enumerator {
                    Ok(e) => e,
                    Err(e) => {
                        tracing::warn!("audio endpoint: device enumerator: {e}");
                        let _ = ready_tx.send(false);
                        return;
                    }
                };
                let _ = ready_tx.send(true);
                serve(&enumerator, &rx);
            })
            .ok()?;
        ready_rx
            .recv_timeout(Duration::from_secs(5))
            .unwrap_or(false)
            .then_some(Self { tx })
    }

    /// The default output's level, or `None` if there is none.
    pub fn read(&self) -> Option<Level> {
        let (tx, rx) = mpsc::channel();
        self.tx.send(Req::Read(tx)).ok()?;
        rx.recv_timeout(Duration::from_secs(2)).ok().flatten()
    }

    pub fn set_override(&self, on: bool) {
        let _ = self.tx.send(Req::Override(on));
    }
}

/// What the override changed, to put back.
struct Saved {
    endpoint: IAudioEndpointVolume,
    muted: bool,
    level: f32,
}

fn serve(enumerator: &IMMDeviceEnumerator, rx: &mpsc::Receiver<Req>) {
    let mut saved: Option<Saved> = None;
    while let Ok(req) = rx.recv() {
        match req {
            Req::Read(reply) => {
                let level = endpoint(enumerator).and_then(|v| read(&v)).ok();
                let _ = reply.send(level.map(|(muted, level)| Level {
                    muted,
                    volume_pct: (level * 100.0).round().clamp(0.0, 100.0) as u8,
                }));
            }
            Req::Override(true) if saved.is_none() => {
                // Saved before anything changes, so a partial override is
                // still put back.
                match endpoint(enumerator).and_then(|v| read(&v).map(|r| (v, r))) {
                    Ok((v, (muted, level))) => {
                        tracing::info!(
                            was_muted = muted,
                            was_pct = (level * 100.0).round(),
                            "volume override on"
                        );
                        let s = saved.insert(Saved {
                            endpoint: v,
                            muted,
                            level,
                        });
                        if let Err(e) = override_on(s) {
                            tracing::warn!("volume override: {e}");
                        }
                    }
                    Err(e) => tracing::warn!("volume override: {e}"),
                }
            }
            Req::Override(false) => {
                if let Some(s) = saved.take() {
                    // SAFETY: `s.endpoint` was activated on this thread; a
                    // null event context is allowed.
                    let r = unsafe {
                        s.endpoint
                            .SetMasterVolumeLevelScalar(s.level, std::ptr::null())
                            .and_then(|()| s.endpoint.SetMute(s.muted, std::ptr::null()))
                    };
                    match r {
                        Ok(()) => tracing::info!("volume override off, restored"),
                        Err(e) => tracing::warn!("restoring the volume: {e}"),
                    }
                }
            }
            Req::Override(true) => {}
        }
    }
}

fn endpoint(enumerator: &IMMDeviceEnumerator) -> windows::core::Result<IAudioEndpointVolume> {
    // SAFETY: the enumerator was created on this thread; Activate is asked
    // for the interface type it returns, with no activation parameters.
    unsafe {
        enumerator
            .GetDefaultAudioEndpoint(eRender, eConsole)?
            .Activate(CLSCTX_ALL, None)
    }
}

/// `(muted, level 0.0 to 1.0)`.
fn read(v: &IAudioEndpointVolume) -> windows::core::Result<(bool, f32)> {
    // SAFETY: `v` was activated on this thread; both calls only write their
    // result.
    unsafe { Ok((v.GetMute()?.as_bool(), v.GetMasterVolumeLevelScalar()?)) }
}

fn override_on(s: &Saved) -> windows::core::Result<()> {
    // SAFETY: as in `read`; a null event context is allowed.
    unsafe {
        s.endpoint.SetMute(false, std::ptr::null())?;
        if s.level < OVERRIDE_MIN {
            s.endpoint
                .SetMasterVolumeLevelScalar(OVERRIDE_MIN, std::ptr::null())?;
        }
    }
    Ok(())
}
