// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-Muster-app-store-exception

//! Default file locations (`spec/08-deployment.md`), with deterministic
//! precedence: explicit argument, then environment, then the platform
//! default. A leading `~` in a path argument is expanded here, not left to
//! the shell (house-style rust-cli.md).

use std::path::{Path, PathBuf};

/// Expands a leading `~` or `~/`.
pub fn expand(p: &Path) -> PathBuf {
    let s = p.to_string_lossy();
    if (s == "~" || s.starts_with("~/"))
        && let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))
    {
        return PathBuf::from(home).join(s.trim_start_matches('~').trim_start_matches('/'));
    }
    p.to_owned()
}

/// [`expand`] for a string.
pub fn expand_str(p: &str) -> PathBuf {
    expand(Path::new(p))
}

/// Headless service data directory, or the per-user one.
pub fn default_data_dir(headless: bool) -> PathBuf {
    if headless && cfg!(unix) {
        return PathBuf::from("/var/lib/muster");
    }
    if cfg!(windows) {
        return std::env::var_os("ProgramData")
            .map(|p| PathBuf::from(p).join("Muster"))
            .unwrap_or_else(|| PathBuf::from(r"C:\ProgramData\Muster"));
    }
    if let Some(x) = std::env::var_os("XDG_DATA_HOME") {
        return PathBuf::from(x).join("muster");
    }
    expand(Path::new("~/.local/share/muster"))
}

/// The machine config file.
pub fn default_config_file() -> PathBuf {
    if cfg!(windows) {
        return default_data_dir(false).join("config.toml");
    }
    if cfg!(target_os = "macos") {
        return PathBuf::from("/Library/Application Support/Muster/config.toml");
    }
    PathBuf::from("/etc/muster/config.toml")
}

/// The machine policy file (Linux; Windows and macOS use the registry and
/// managed preferences, read by the platform layer).
pub fn default_policy_file() -> PathBuf {
    PathBuf::from("/etc/muster/policy.toml")
}

/// The control socket: `$XDG_RUNTIME_DIR/muster/control.sock`, else inside
/// the data directory. On Windows, the named pipe
/// `\\.\pipe\muster-<domain>-<user>` (one per signed-in user).
pub fn default_control_socket(data_dir: &Path) -> PathBuf {
    if cfg!(windows) {
        let part = |v: &str| {
            std::env::var(v)
                .unwrap_or_default()
                .chars()
                .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
                .collect::<String>()
        };
        let _ = data_dir;
        return PathBuf::from(format!(
            r"\\.\pipe\muster-{}-{}",
            part("USERDOMAIN"),
            part("USERNAME")
        ));
    }
    match std::env::var_os("XDG_RUNTIME_DIR") {
        Some(r) => PathBuf::from(r).join("muster").join("control.sock"),
        None => data_dir.join("control.sock"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tilde_expands() {
        let p = expand(Path::new("~/x"));
        assert!(!p.to_string_lossy().starts_with('~'));
        assert_eq!(expand(Path::new("/a/~b")), PathBuf::from("/a/~b"));
    }
}
