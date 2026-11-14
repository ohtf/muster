// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-Muster-app-store-exception

//! Drill reports (FR-DRL-004). RTT is measured locally at the originator,
//! from send to Receipt arrival, so it does not depend on synchronised
//! clocks.

use muster_proto::{AudioStatus, DeviceId, MsgId, PathKind};

use crate::time::WallMs;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrillRow {
    pub device: DeviceId,
    pub label: String,
    pub room: String,
    pub received: bool,
    pub rtt_ms: Option<u64>,
    pub path: Option<PathKind>,
    pub audio: Option<AudioStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrillReport {
    pub alarm_id: MsgId,
    pub started_at: WallMs,
    pub rows: Vec<DrillRow>,
}

impl DrillReport {
    pub fn received(&self) -> usize {
        self.rows.iter().filter(|r| r.received).count()
    }

    /// CSV export (FR-DRL-007).
    pub fn to_csv(&self) -> String {
        let mut s = String::from(
            "device,label,room,received,rtt_ms,path,audio_present,audio_muted,volume_pct\n",
        );
        for r in &self.rows {
            let path = r
                .path
                .map(|p| format!("{p:?}").to_lowercase())
                .unwrap_or_default();
            let (p, m, v) = r
                .audio
                .map(|a| {
                    (
                        a.output_present.to_string(),
                        a.muted.to_string(),
                        a.volume_pct.to_string(),
                    )
                })
                .unwrap_or_default();
            s.push_str(&format!(
                "{},{},{},{},{},{},{},{},{}\n",
                r.device.short(),
                csv_field(&r.label),
                csv_field(&r.room),
                r.received,
                r.rtt_ms.map(|x| x.to_string()).unwrap_or_default(),
                path,
                p,
                m,
                v
            ));
        }
        s
    }
}

/// Quotes a CSV field and neutralises spreadsheet formula injection.
pub fn csv_field(s: &str) -> String {
    let mut v = s.replace('"', "\"\"");
    if v.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        v.insert(0, '\'');
    }
    format!("\"{v}\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_escapes() {
        assert_eq!(csv_field("a\"b"), "\"a\"\"b\"");
        assert_eq!(csv_field("=SUM(A1)"), "\"'=SUM(A1)\"");
    }
}
