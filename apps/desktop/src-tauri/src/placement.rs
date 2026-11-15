// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-PanicButton-app-store-exception

//! Where the floating button goes. Positions are remembered per monitor
//! layout (FR-ACT-010), so a laptop's button returns to its docked place when
//! it is docked again. A saved position is pulled back onto the monitor that
//! holds its centre; if no monitor does, the button goes to the default
//! corner. Either way it can never be lost off screen.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// One monitor, in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Monitor {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub scale: f64,
}

impl Monitor {
    /// The button's side on this monitor, from its logical size.
    fn side(&self, logical: f64) -> i32 {
        (logical * self.scale).round() as i32
    }

    fn holds(&self, x: i32, y: i32) -> bool {
        let (x, y) = (i64::from(x), i64::from(y));
        let (x0, y0) = (i64::from(self.x), i64::from(self.y));
        x >= x0 && y >= y0 && x < x0 + i64::from(self.width) && y < y0 + i64::from(self.height)
    }

    fn contains(&self, x: i32, y: i32, logical: f64) -> bool {
        let s = self.side(logical);
        self.holds(x, y) && self.holds(x + s - 1, y + s - 1)
    }

    /// `(x, y)` moved the least distance needed to put the whole button on
    /// this monitor.
    fn clamp(&self, x: i32, y: i32, logical: f64) -> (i32, i32) {
        let s = self.side(logical);
        let max_x =
            (i64::from(self.x) + i64::from(self.width) - i64::from(s)).max(i64::from(self.x));
        let max_y =
            (i64::from(self.y) + i64::from(self.height) - i64::from(s)).max(i64::from(self.y));
        (
            i64::from(x).clamp(i64::from(self.x), max_x) as i32,
            i64::from(y).clamp(i64::from(self.y), max_y) as i32,
        )
    }
}

/// A stable name for a set of monitors: their sizes, positions and scales,
/// in order. Monitor names are left out because some systems change them
/// between sessions.
pub fn layout_key(monitors: &[Monitor]) -> String {
    let mut m: Vec<String> = monitors
        .iter()
        .map(|m| {
            format!(
                "{}x{}+{}+{}@{}",
                m.width,
                m.height,
                m.x,
                m.y,
                (m.scale * 100.0).round() as i64
            )
        })
        .collect();
    m.sort();
    m.join(",")
}

/// Where to put the button (`logical` pixels square before scaling): the
/// saved position, pulled wholly onto the monitor holding its centre, or
/// `default` if there is no saved position or no such monitor.
pub fn choose(
    saved: Option<(i32, i32)>,
    monitors: &[Monitor],
    logical: f64,
    default: (i32, i32),
) -> (i32, i32) {
    let Some((x, y)) = saved else {
        return default;
    };
    monitors
        .iter()
        .find(|m| {
            let h = m.side(logical) / 2;
            m.holds(x.saturating_add(h), y.saturating_add(h))
        })
        .map_or(default, |m| m.clamp(x, y, logical))
}

/// Whether the button at `(x, y)` is wholly on a monitor.
pub fn visible(pos: (i32, i32), monitors: &[Monitor], logical: f64) -> bool {
    monitors.iter().any(|m| m.contains(pos.0, pos.1, logical))
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    /// Layout key to physical position.
    positions: BTreeMap<String, (i32, i32)>,
}

/// Saved positions, in `<data dir>/button-position.json`.
#[derive(Debug)]
pub struct Store {
    path: PathBuf,
    file: File,
}

/// At most this many layouts are kept; the file never grows without bound.
const MAX_LAYOUTS: usize = 16;

impl Store {
    pub fn load(data_dir: &Path) -> Self {
        let path = data_dir.join("button-position.json");
        let file = std::fs::read(&path)
            .ok()
            .and_then(|b| match serde_json::from_slice(&b) {
                Ok(f) => Some(f),
                Err(e) => {
                    tracing::warn!("ignoring {}: {e}", path.display());
                    None
                }
            })
            .unwrap_or_default();
        Self { path, file }
    }

    pub fn get(&self, layout: &str) -> Option<(i32, i32)> {
        self.file.positions.get(layout).copied()
    }

    /// Returns true if the position changed.
    pub fn set(&mut self, layout: &str, pos: (i32, i32)) -> bool {
        if self.get(layout) == Some(pos) {
            return false;
        }
        if !self.file.positions.contains_key(layout) && self.file.positions.len() >= MAX_LAYOUTS {
            let first = self.file.positions.keys().next().cloned();
            if let Some(k) = first {
                self.file.positions.remove(&k);
            }
        }
        self.file.positions.insert(layout.to_owned(), pos);
        true
    }

    pub fn save(&self) {
        let r = serde_json::to_vec_pretty(&self.file)
            .map_err(std::io::Error::other)
            .and_then(|b| {
                let tmp = self.path.with_extension("json.tmp");
                std::fs::write(&tmp, b)?;
                std::fs::rename(&tmp, &self.path)
            });
        if let Err(e) = r {
            tracing::warn!("could not save the button position: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LAPTOP: Monitor = Monitor {
        x: 0,
        y: 0,
        width: 1920,
        height: 1200,
        scale: 1.25,
    };
    const DESK: Monitor = Monitor {
        x: 1920,
        y: 0,
        width: 2560,
        height: 1440,
        scale: 1.0,
    };

    #[test]
    fn layout_key_ignores_order() {
        assert_eq!(layout_key(&[LAPTOP, DESK]), layout_key(&[DESK, LAPTOP]));
        assert_ne!(layout_key(&[LAPTOP]), layout_key(&[LAPTOP, DESK]));
        assert_eq!(layout_key(&[LAPTOP]), "1920x1200+0+0@125");
    }

    // The button is 76 logical pixels: 95 physical on LAPTOP, 76 on DESK.
    const B: f64 = 76.0;

    #[test]
    fn saved_position_is_kept_or_pulled_back_on_screen() {
        let d = (1800, 1080);
        // On the second monitor.
        assert_eq!(
            choose(Some((3000, 500)), &[LAPTOP, DESK], B, d),
            (3000, 500)
        );
        // That monitor is gone.
        assert_eq!(choose(Some((3000, 500)), &[LAPTOP], B, d), d);
        // Centre on the laptop but partly off its edge: pulled back on.
        assert_eq!(choose(Some((1850, 500)), &[LAPTOP], B, d), (1825, 500));
        assert_eq!(choose(Some((-10, -5)), &[LAPTOP], B, d), (0, 0));
        // Centre past the laptop's edge with nothing beyond.
        assert_eq!(choose(Some((1900, 500)), &[LAPTOP], B, d), d);
        // Straddling two monitors: goes wholly onto the one with its centre.
        assert_eq!(
            choose(Some((1900, 500)), &[LAPTOP, DESK], B, d),
            (1920, 500)
        );
        // Exactly in the corner.
        assert_eq!(choose(Some((1825, 1105)), &[LAPTOP], B, d), (1825, 1105));
        assert_eq!(choose(None, &[LAPTOP], B, d), d);
        // No monitors reported: never trust a saved position.
        assert_eq!(choose(Some((10, 10)), &[], B, d), d);
        // Extreme values do not overflow.
        assert_eq!(choose(Some((i32::MAX, i32::MAX)), &[LAPTOP], B, d), d);
    }

    #[test]
    fn visibility() {
        assert!(visible((1825, 1105), &[LAPTOP], B));
        assert!(!visible((1826, 1105), &[LAPTOP], B));
        assert!(!visible((1900, 500), &[LAPTOP, DESK], B));
        assert!(visible((1920, 500), &[LAPTOP, DESK], B));
    }

    #[test]
    fn store_round_trips_and_is_bounded() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut s = Store::load(dir.path());
        assert!(s.set("a", (1, 2)));
        assert!(!s.set("a", (1, 2)));
        s.save();
        let s2 = Store::load(dir.path());
        assert_eq!(s2.get("a"), Some((1, 2)));
        let mut s3 = Store::load(dir.path());
        for i in 0..40 {
            s3.set(&format!("layout{i:02}"), (i, i));
        }
        assert_eq!(s3.file.positions.len(), MAX_LAYOUTS);
        assert_eq!(s3.get("layout39"), Some((39, 39)));
    }

    #[test]
    fn a_damaged_file_is_ignored() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("button-position.json"), b"{not json").expect("write");
        assert_eq!(Store::load(dir.path()).get("a"), None);
    }
}
