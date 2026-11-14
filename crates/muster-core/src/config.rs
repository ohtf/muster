// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-Muster-app-store-exception

//! Effective configuration (`spec/08-deployment.md`, Configuration sources
//! and precedence).
//!
//! The runtime reads machine policy (1) and the machine config file (2) and
//! user preferences (4), and passes them in as [`LocalConfig`]. The core
//! merges them with the signed practice config (3) and profile defaults (5).

use std::net::{Ipv4Addr, SocketAddrV4};

use muster_proto::{CueMode, DeviceId, DrillMode, PracticeSettings, SoundPattern};

use crate::time::{HOLD_MS_DEFAULT, HOLD_MS_MAX, HOLD_MS_MIN};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Profile {
    #[default]
    Practice,
    Home,
}

/// Settings that sources 1 and 2 (machine policy and config file) can set.
/// `None` means not set at that level.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MachineSettings {
    pub hold_ms: Option<u32>,
    pub cue_mode: Option<CueMode>,
    pub sound_pattern: Option<SoundPattern>,
    pub override_mute: Option<bool>,
    pub volume_pct: Option<u8>,
    pub seed_peers: Vec<String>,
    pub relay_urls: Vec<String>,
}

/// User preferences (source 4). Only applied where sources 1 to 3 are silent.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UserPrefs {
    pub volume_pct: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalConfig {
    pub profile: Profile,
    pub label: String,
    pub room: String,
    /// Sources 1 and 2, already merged (1 wins) by the runtime.
    pub machine: MachineSettings,
    pub user: UserPrefs,
    pub port: u16,
    pub allow_remote_session: bool,
    /// Seed peers resolved to socket addresses by the runtime.
    pub seed_addrs: Vec<SocketAddrV4>,
    /// Number of relay URLs configured (relays are identified by index).
    pub relay_count: u16,
}

impl Default for LocalConfig {
    fn default() -> Self {
        LocalConfig {
            profile: Profile::Practice,
            label: String::new(),
            room: String::new(),
            machine: MachineSettings::default(),
            user: UserPrefs::default(),
            port: muster_proto::limits::DEFAULT_PORT,
            allow_remote_session: false,
            seed_addrs: Vec::new(),
            relay_count: 0,
        }
    }
}

/// The resolved values the core acts on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveConfig {
    pub profile: Profile,
    pub label: String,
    pub room: String,
    pub hold_ms: u32,
    pub cue_mode: CueMode,
    pub sound_pattern: SoundPattern,
    pub override_mute: bool,
    pub volume_pct: u8,
    pub drill_mode: DrillMode,
    pub drill_coordinator: Option<DeviceId>,
    pub port: u16,
    pub allow_remote_session: bool,
    pub seed_addrs: Vec<SocketAddrV4>,
    /// Union of relay URLs from sources 1 to 3.
    pub relay_urls: Vec<String>,
    pub relay_count: u16,
}

impl EffectiveConfig {
    pub fn resolve(local: &LocalConfig, practice: &PracticeSettings) -> EffectiveConfig {
        let home = local.profile == Profile::Home;
        let m = &local.machine;
        let hold_ms = m
            .hold_ms
            .or(practice.hold_ms)
            .unwrap_or(HOLD_MS_DEFAULT)
            .clamp(HOLD_MS_MIN, HOLD_MS_MAX);
        let cue_mode = m
            .cue_mode
            .or(practice.cue_mode)
            .filter(|c| !matches!(c, CueMode::Other(_)))
            .unwrap_or(if home {
                CueMode::AlarmRamp
            } else {
                CueMode::Soft
            });
        let sound_pattern = m
            .sound_pattern
            .or(practice.sound_pattern)
            .filter(|p| !matches!(p, SoundPattern::Other(_)))
            .unwrap_or(SoundPattern::A);
        let override_mute = m.override_mute.or(practice.override_mute).unwrap_or(!home);
        let volume_pct = m
            .volume_pct
            .or(local.user.volume_pct)
            .unwrap_or(100)
            .min(100);
        let drill_mode = practice
            .drill_mode
            .filter(|d| !matches!(d, DrillMode::Other(_)))
            .unwrap_or(if home {
                DrillMode::Audible
            } else {
                DrillMode::Silent
            });
        let mut relay_urls = m.relay_urls.clone();
        for u in practice.relay_urls.iter().flatten() {
            if !relay_urls.contains(u) {
                relay_urls.push(u.clone());
            }
        }
        EffectiveConfig {
            profile: local.profile,
            label: local.label.clone(),
            room: local.room.clone(),
            hold_ms,
            cue_mode,
            sound_pattern,
            override_mute,
            volume_pct,
            drill_mode,
            drill_coordinator: practice.drill_coordinator,
            port: local.port,
            allow_remote_session: local.allow_remote_session,
            seed_addrs: local.seed_addrs.clone(),
            relay_urls,
            relay_count: local.relay_count,
        }
    }
}

/// Parses `host:port` where host is a dotted IPv4 address. Name resolution
/// is the runtime's job.
pub fn parse_ipv4_peer(s: &str, default_port: u16) -> Option<SocketAddrV4> {
    if let Ok(a) = s.parse::<SocketAddrV4>() {
        return Some(a);
    }
    s.parse::<Ipv4Addr>()
        .ok()
        .map(|ip| SocketAddrV4::new(ip, default_port))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn precedence_policy_over_practice_over_defaults() {
        let mut local = LocalConfig::default();
        let practice = PracticeSettings {
            hold_ms: Some(2000),
            cue_mode: Some(CueMode::Silent),
            relay_urls: Some(vec!["wss://b".into()]),
            ..Default::default()
        };
        let c = EffectiveConfig::resolve(&local, &practice);
        assert_eq!(c.hold_ms, 2000);
        assert_eq!(c.cue_mode, CueMode::Silent);
        local.machine.hold_ms = Some(2500);
        local.machine.relay_urls = vec!["wss://a".into()];
        let c = EffectiveConfig::resolve(&local, &practice);
        assert_eq!(c.hold_ms, 2500);
        // Relay URLs are a union.
        assert_eq!(
            c.relay_urls,
            vec!["wss://a".to_string(), "wss://b".to_string()]
        );
    }

    #[test]
    fn hold_ms_is_clamped() {
        let mut local = LocalConfig::default();
        local.machine.hold_ms = Some(100);
        assert_eq!(
            EffectiveConfig::resolve(&local, &PracticeSettings::default()).hold_ms,
            HOLD_MS_MIN
        );
        local.machine.hold_ms = Some(10_000);
        assert_eq!(
            EffectiveConfig::resolve(&local, &PracticeSettings::default()).hold_ms,
            HOLD_MS_MAX
        );
    }

    #[test]
    fn profile_defaults() {
        let c = EffectiveConfig::resolve(&LocalConfig::default(), &PracticeSettings::default());
        assert_eq!(c.hold_ms, 1500);
        assert_eq!(c.cue_mode, CueMode::Soft);
        assert!(c.override_mute);
        assert_eq!(c.drill_mode, DrillMode::Silent);
        let home = LocalConfig {
            profile: Profile::Home,
            ..Default::default()
        };
        let c = EffectiveConfig::resolve(&home, &PracticeSettings::default());
        assert_eq!(c.cue_mode, CueMode::AlarmRamp);
        assert!(!c.override_mute);
    }

    #[test]
    fn parses_peers() {
        assert_eq!(
            parse_ipv4_peer("10.0.0.1:1", 5),
            Some(SocketAddrV4::new(Ipv4Addr::new(10, 0, 0, 1), 1))
        );
        assert_eq!(
            parse_ipv4_peer("10.0.0.1", 5),
            Some(SocketAddrV4::new(Ipv4Addr::new(10, 0, 0, 1), 5))
        );
        assert_eq!(parse_ipv4_peer("host:1", 5), None);
    }
}
