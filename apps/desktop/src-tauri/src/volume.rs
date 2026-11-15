// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-PanicButton-app-store-exception

//! Whether the computer's sound is muted or turned right down, so a
//! computer that would ring silently shows amber instead of green
//! (`StatusReason::AudioMuted`) and colleagues' test alarm reports show it.
//!
//! Read from the system's own tools: `pactl` (PulseAudio and PipeWire),
//! then `wpctl` (PipeWire), then `amixer` (ALSA) on Linux, and `osascript`
//! on macOS, and the audio endpoint API on Windows
//! (`platform::windows_audio`), which can also override the mute while an
//! alarm sounds. When nothing can be read the level is unknown: an unknown
//! level is never reported as muted.

// The command-line readers are not used on Windows.
#![cfg_attr(windows, allow(dead_code))]

use std::time::Duration;

/// The default output's level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Level {
    pub muted: bool,
    pub volume_pct: u8,
}

/// Runs a command with `LC_ALL=C` (some tools translate their output),
/// giving up after 3 s.
async fn run(cmd: &str, args: &[&str]) -> Option<String> {
    let child = tokio::process::Command::new(cmd)
        .args(args)
        .env("LC_ALL", "C")
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .output();
    match tokio::time::timeout(Duration::from_secs(3), child).await {
        Ok(Ok(o)) if o.status.success() => String::from_utf8(o.stdout).ok(),
        _ => None,
    }
}

/// Reads the level, and on Windows overrides the mute.
#[derive(Debug, Clone)]
pub struct Volume {
    #[cfg(windows)]
    endpoint: Option<crate::platform::windows_audio::Endpoint>,
}

impl Volume {
    pub fn new() -> Self {
        Self {
            #[cfg(windows)]
            endpoint: crate::platform::windows_audio::Endpoint::start(),
        }
    }

    /// Whether `set_override` can unmute and raise the volume.
    #[cfg(windows)]
    pub fn override_available(&self) -> bool {
        self.endpoint.is_some()
    }

    #[cfg(not(windows))]
    pub fn override_available(&self) -> bool {
        false
    }

    #[cfg(windows)]
    pub fn set_override(&self, on: bool) {
        if let Some(e) = &self.endpoint {
            e.set_override(on);
        }
    }

    #[cfg(not(windows))]
    pub fn set_override(&self, _on: bool) {}

    /// The current level, or `None` if it cannot be read on this computer.
    #[cfg(windows)]
    pub async fn read(&self) -> Option<Level> {
        let e = self.endpoint.clone()?;
        tokio::task::spawn_blocking(move || e.read())
            .await
            .ok()
            .flatten()
    }

    #[cfg(not(windows))]
    pub async fn read(&self) -> Option<Level> {
        read_with_tools().await
    }
}

async fn read_with_tools() -> Option<Level> {
    if cfg!(target_os = "macos") {
        return parse_osascript(&run("osascript", &["-e", "get volume settings"]).await?);
    }
    if !cfg!(target_os = "linux") {
        return None;
    }
    if let (Some(m), Some(v)) = (
        run("pactl", &["get-sink-mute", "@DEFAULT_SINK@"]).await,
        run("pactl", &["get-sink-volume", "@DEFAULT_SINK@"]).await,
    ) && let (Some(muted), Some(volume_pct)) = (parse_pactl_mute(&m), parse_pactl_volume(&v))
    {
        return Some(Level { muted, volume_pct });
    }
    if let Some(l) = run("wpctl", &["get-volume", "@DEFAULT_AUDIO_SINK@"])
        .await
        .as_deref()
        .and_then(parse_wpctl)
    {
        return Some(l);
    }
    parse_amixer(&run("amixer", &["get", "Master"]).await?)
}

fn pct(v: f64) -> u8 {
    v.round().clamp(0.0, 100.0) as u8
}

/// `Mute: no`
fn parse_pactl_mute(s: &str) -> Option<bool> {
    match s.trim().strip_prefix("Mute:")?.trim() {
        "yes" => Some(true),
        "no" => Some(false),
        _ => None,
    }
}

/// `Volume: front-left: 45220 /  69% / -9.67 dB,   front-right: ...`; the
/// loudest channel.
fn parse_pactl_volume(s: &str) -> Option<u8> {
    let line = s.lines().find(|l| l.trim_start().starts_with("Volume:"))?;
    line.split('/')
        .filter_map(|p| p.trim().strip_suffix('%')?.trim().parse::<f64>().ok())
        .reduce(f64::max)
        .map(pct)
}

/// `Volume: 0.69` or `Volume: 0.69 [MUTED]`
fn parse_wpctl(s: &str) -> Option<Level> {
    let rest = s.trim().strip_prefix("Volume:")?.trim();
    let v: f64 = rest.split_whitespace().next()?.parse().ok()?;
    Some(Level {
        muted: rest.contains("[MUTED]"),
        volume_pct: pct(v * 100.0),
    })
}

/// `  Front Left: Playback 45220 [69%] [on]` lines; muted only if every
/// channel is off.
fn parse_amixer(s: &str) -> Option<Level> {
    let mut found = None::<Level>;
    for line in s
        .lines()
        .filter(|l| l.contains("Playback") && l.contains('%'))
    {
        let fields: Vec<&str> = line
            .split('[')
            .skip(1)
            .filter_map(|f| f.split(']').next())
            .collect();
        let Some(v) = fields
            .iter()
            .find_map(|f| f.strip_suffix('%')?.parse::<f64>().ok())
        else {
            continue;
        };
        let off = fields.contains(&"off");
        found = Some(match found {
            None => Level {
                muted: off,
                volume_pct: pct(v),
            },
            Some(l) => Level {
                muted: l.muted && off,
                volume_pct: l.volume_pct.max(pct(v)),
            },
        });
    }
    found
}

/// `output volume:50, input volume:75, alert volume:100, output muted:false`
fn parse_osascript(s: &str) -> Option<Level> {
    let mut volume = None;
    let mut muted = None;
    for part in s.trim().split(',') {
        match part.trim().split_once(':')? {
            ("output volume", v) => volume = v.trim().parse::<f64>().ok(),
            ("output muted", m) => muted = m.trim().parse::<bool>().ok(),
            _ => {}
        }
    }
    Some(Level {
        muted: muted?,
        volume_pct: pct(volume?),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn l(muted: bool, volume_pct: u8) -> Option<Level> {
        Some(Level { muted, volume_pct })
    }

    #[test]
    fn pactl() {
        assert_eq!(parse_pactl_mute("Mute: no\n"), Some(false));
        assert_eq!(parse_pactl_mute("Mute: yes\n"), Some(true));
        assert_eq!(parse_pactl_mute("Stumm: nein\n"), None);
        let v = "Volume: front-left: 45220 /  69% / -9.67 dB,   front-right: 45220 /  71% / -9.67 dB\n        balance 0.00\n";
        assert_eq!(parse_pactl_volume(v), Some(71));
        assert_eq!(
            parse_pactl_volume("Volume: mono: 98304 / 150% / 10.57 dB\n"),
            Some(100)
        );
        assert_eq!(
            parse_pactl_volume("Volume: mono: 0 /   0% / -inf dB\n"),
            Some(0)
        );
        assert_eq!(parse_pactl_volume("nothing\n"), None);
    }

    #[test]
    fn wpctl() {
        assert_eq!(parse_wpctl("Volume: 0.69\n"), l(false, 69));
        assert_eq!(parse_wpctl("Volume: 0.40 [MUTED]\n"), l(true, 40));
        assert_eq!(parse_wpctl("Volume: 1.20\n"), l(false, 100));
        assert_eq!(parse_wpctl("Translate this\n"), None);
    }

    #[test]
    fn amixer() {
        let on = "Simple mixer control 'Master',0\n  Limits: Playback 0 - 65536\n  Mono:\n  Front Left: Playback 45220 [69%] [on]\n  Front Right: Playback 45220 [69%] [on]\n";
        assert_eq!(parse_amixer(on), l(false, 69));
        let off = on.replace("[on]", "[off]");
        assert_eq!(parse_amixer(&off), l(true, 69));
        let one_off = on.replacen("[on]", "[off]", 1);
        assert_eq!(parse_amixer(&one_off), l(false, 69));
        let mono = "  Mono: Playback 87 [100%] [0.00dB] [on]\n";
        assert_eq!(parse_amixer(mono), l(false, 100));
        assert_eq!(
            parse_amixer("amixer: Unable to find simple control 'Master',0\n"),
            None
        );
    }

    #[test]
    fn osascript() {
        assert_eq!(
            parse_osascript(
                "output volume:50, input volume:75, alert volume:100, output muted:false\n"
            ),
            l(false, 50)
        );
        assert_eq!(
            parse_osascript(
                "output volume:missing value, input volume:missing value, alert volume:100, output muted:true"
            ),
            None
        );
        assert_eq!(
            parse_osascript(
                "output volume:0, input volume:75, alert volume:100, output muted:true"
            ),
            l(true, 0)
        );
    }
}
