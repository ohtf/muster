// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-PanicButton-app-store-exception

//! The module boundaries that separate crates used to enforce (ADR 0016,
//! `spec/02-architecture.md`, Module dependency direction):
//!
//! - `proto`, `crypto` and `core` do no I/O and read no clock or system
//!   randomness, so the simulator stays deterministic;
//! - each module uses only the modules below it;
//! - the relay uses only `proto` and `crypto`, so it can check certificates
//!   but never handles alarm state or the alarm logic.
//!
//! The check reads the source text, so a forbidden name in a comment fails
//! too: reword the comment.

use std::path::{Path, PathBuf};

/// Names a module that must stay free of I/O may not mention. Address types
/// from `std::net` are allowed; sockets are not.
const IO: &[&str] = &[
    "tokio",
    "std::fs",
    "std::io",
    "std::thread",
    "std::env",
    "std::process",
    "UdpSocket",
    "TcpStream",
    "TcpListener",
    "SystemTime",
    "Instant",
    "getrandom",
    "OsRng",
    "thread_rng",
    "rusqlite",
    "reqwest",
    "axum",
];

/// Each module, the modules it may use, and whether it must be free of I/O.
const RULES: &[(&str, &[&str], bool)] = &[
    ("proto", &[], true),
    ("crypto", &["proto"], true),
    ("core", &["proto", "crypto"], true),
    ("store", &["proto", "crypto", "core"], false),
    ("net", &["proto", "core"], false),
    ("agent", &["proto", "crypto", "core", "store", "net"], false),
    ("relay", &["proto", "crypto"], false),
    (
        "cli",
        &["proto", "crypto", "core", "store", "net", "agent"],
        false,
    ),
];

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).expect("read_dir") {
        let p = e.expect("entry").path();
        if p.is_dir() {
            rust_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

/// Whether `word` appears in `text` with no identifier character either side.
fn mentions(text: &str, word: &str) -> bool {
    let ident = |c: char| c.is_alphanumeric() || c == '_';
    text.match_indices(word).any(|(i, _)| {
        !text[..i].chars().next_back().is_some_and(ident)
            && !text[i + word.len()..].chars().next().is_some_and(ident)
    })
}

#[test]
fn modules_keep_their_boundaries() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let all: Vec<&str> = RULES.iter().map(|(m, _, _)| *m).collect();
    let mut problems = Vec::new();
    for (module, allowed, pure) in RULES {
        let mut files = Vec::new();
        rust_files(&src.join(module), &mut files);
        assert!(!files.is_empty(), "no source for module {module}");
        for f in files {
            let text = std::fs::read_to_string(&f).expect("read");
            let name = f.strip_prefix(&src).unwrap_or(&f).display().to_string();
            for other in all.iter().filter(|o| *o != module && !allowed.contains(o)) {
                if mentions(&text, &format!("crate::{other}")) {
                    problems.push(format!("{name} uses crate::{other}"));
                }
            }
            if *pure {
                for word in IO {
                    if mentions(&text, word) {
                        problems.push(format!("{name} mentions {word}, but {module} does no I/O"));
                    }
                }
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn the_check_catches_a_violation() {
    assert!(mentions("use tokio::time;", "tokio"));
    assert!(!mentions("use my_tokio_shim;", "tokio"));
    assert!(mentions("use crate::store::Store;", "crate::store"));
    assert!(!mentions("use crate::storage;", "crate::store"));
}
