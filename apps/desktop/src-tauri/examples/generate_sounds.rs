// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-Muster-app-store-exception

//! Writes the built-in sounds to `sounds/` (run through `s/sounds`).

use muster_desktop::audio::{Clip, wav_bytes};

fn main() -> std::io::Result<()> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../sounds");
    std::fs::create_dir_all(&dir)?;
    for c in Clip::ALL {
        let p = dir.join(c.file_name());
        std::fs::write(&p, wav_bytes(&c.render()))?;
        println!("wrote {}", p.display());
    }
    Ok(())
}
