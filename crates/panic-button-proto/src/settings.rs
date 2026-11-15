// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-PanicButton-app-store-exception

//! Practice-wide settings carried in PracticeConfig (source 3 in
//! `spec/08-deployment.md`). Every field is optional: absent means "not set
//! by the practice", so lower-precedence sources apply.
//!
//! Integer keys (documented in `spec/03-protocol.md`, PracticeConfig):
//! 0 hold_ms, 1 cue_mode, 2 sound_pattern, 3 override_mute, 4 drill_schedule,
//! 5 drill_mode, 6 drill_coordinator, 7 opening_hours, 8 relay_urls,
//! 9 seed_peers.

use crate::cbor::{Cbor, MapBuilder, Reader, Writer};
use crate::error::DecodeError;
use crate::ids::DeviceId;
use crate::limits;
use crate::message::open_enum;

open_enum!(
    CueMode {
        Silent = 0,
        Soft = 1,
        AlarmRamp = 2,
    }
);

open_enum!(
    SoundPattern {
        A = 0,
        B = 1,
        C = 2,
    }
);

open_enum!(
    DrillMode {
        Silent = 0,
        Audible = 1,
    }
);

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PracticeSettings {
    pub hold_ms: Option<u32>,
    pub cue_mode: Option<CueMode>,
    pub sound_pattern: Option<SoundPattern>,
    pub override_mute: Option<bool>,
    pub drill_schedule: Option<String>,
    pub drill_mode: Option<DrillMode>,
    pub drill_coordinator: Option<DeviceId>,
    pub opening_hours: Option<String>,
    pub relay_urls: Option<Vec<String>>,
    pub seed_peers: Option<Vec<String>>,
}

fn text_list(w: &mut Writer, v: &[String]) {
    w.array(v.len());
    for s in v {
        w.text_full(s);
    }
}

impl Cbor for PracticeSettings {
    fn encode(&self, w: &mut Writer) {
        let mut m = MapBuilder::new();
        if let Some(v) = self.hold_ms {
            m.entry(0, |w| {
                w.u64(u64::from(v));
            });
        }
        if let Some(v) = self.cue_mode {
            m.entry(1, |w| {
                w.u64(v.code());
            });
        }
        if let Some(v) = self.sound_pattern {
            m.entry(2, |w| {
                w.u64(v.code());
            });
        }
        if let Some(v) = self.override_mute {
            m.entry(3, |w| {
                w.bool(v);
            });
        }
        if let Some(v) = &self.drill_schedule {
            m.entry(4, |w| {
                w.text_full(v);
            });
        }
        if let Some(v) = self.drill_mode {
            m.entry(5, |w| {
                w.u64(v.code());
            });
        }
        if let Some(v) = &self.drill_coordinator {
            m.entry(6, |w| v.encode(w));
        }
        if let Some(v) = &self.opening_hours {
            m.entry(7, |w| {
                w.text_full(v);
            });
        }
        if let Some(v) = &self.relay_urls {
            m.entry(8, |w| text_list(w, v));
        }
        if let Some(v) = &self.seed_peers {
            m.entry(9, |w| text_list(w, v));
        }
        m.write(w);
    }

    fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        let mut s = PracticeSettings::default();
        let list = |r: &mut Reader<'_>| r.array_of(64, |r| r.text_max(limits::CONFIG_STRING_MAX));
        r.map(|k, r| {
            match k {
                0 => {
                    s.hold_ms =
                        Some(u32::try_from(r.u64()?).map_err(|_| DecodeError::InvalidValue)?)
                }
                1 => s.cue_mode = Some(CueMode::from_code(r.u64()?)),
                2 => s.sound_pattern = Some(SoundPattern::from_code(r.u64()?)),
                3 => s.override_mute = Some(r.bool()?),
                4 => s.drill_schedule = Some(r.text_max(limits::CONFIG_STRING_MAX)?),
                5 => s.drill_mode = Some(DrillMode::from_code(r.u64()?)),
                6 => s.drill_coordinator = Some(DeviceId::decode(r)?),
                7 => s.opening_hours = Some(r.text_max(limits::CONFIG_STRING_MAX)?),
                8 => s.relay_urls = Some(list(r)?),
                9 => s.seed_peers = Some(list(r)?),
                _ => return Ok(false),
            }
            Ok(true)
        })?;
        Ok(s)
    }
}
