// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-PanicButton-app-store-exception

//! `relay.toml` (`spec/06-relay.md`, Configuration). Strict: unknown keys
//! are errors.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use panic_button_proto::PracticeId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    #[serde(default = "d_conns")]
    pub max_connections_per_tenant: usize,
    #[serde(default = "d_frame")]
    pub max_frame_bytes: usize,
    #[serde(default = "d_rate")]
    pub msgs_per_sec_per_conn: u32,
    #[serde(default = "d_burst")]
    pub burst_per_conn: u32,
    #[serde(default = "d_hours")]
    pub buffer_hours: u64,
    #[serde(default = "d_envs")]
    pub buffer_max_envelopes: usize,
}

fn d_conns() -> usize {
    500
}
fn d_frame() -> usize {
    65_536
}
fn d_rate() -> u32 {
    50
}
fn d_burst() -> u32 {
    200
}
fn d_hours() -> u64 {
    24
}
fn d_envs() -> usize {
    10_000
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            max_connections_per_tenant: d_conns(),
            max_frame_bytes: d_frame(),
            msgs_per_sec_per_conn: d_rate(),
            burst_per_conn: d_burst(),
            buffer_hours: d_hours(),
            buffer_max_envelopes: d_envs(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TenantConfig {
    /// 32 hex characters.
    pub practice_id: String,
    /// 64 hex characters.
    pub root_pub: String,
    /// Operator-facing label only.
    #[serde(default)]
    pub name: String,
}

impl TenantConfig {
    pub fn parsed(&self) -> Result<(PracticeId, [u8; 32]), String> {
        let pid = PracticeId::from_hex(&self.practice_id).ok_or_else(|| {
            format!(
                "tenant {:?}: practice_id must be 32 hex characters",
                self.name
            )
        })?;
        let root = panic_button_proto::DeviceId::from_hex(&self.root_pub)
            .ok_or_else(|| format!("tenant {:?}: root_pub must be 64 hex characters", self.name))?;
        Ok((pid, root.0))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RelayConfig {
    #[serde(default = "d_listen")]
    pub listen: SocketAddr,
    #[serde(default)]
    pub tls_cert: String,
    #[serde(default)]
    pub tls_key: String,
    #[serde(default = "d_source")]
    pub public_source_url: String,
    #[serde(default = "d_data")]
    pub data_dir: PathBuf,
    #[serde(default)]
    pub persist: bool,
    #[serde(default)]
    pub limits: Limits,
    #[serde(default, rename = "tenant")]
    pub tenants: Vec<TenantConfig>,
}

fn d_listen() -> SocketAddr {
    SocketAddr::from(([0, 0, 0, 0], 8443))
}
fn d_source() -> String {
    "https://github.com/ohtf/panic-button/tree/v{version}".into()
}
fn d_data() -> PathBuf {
    PathBuf::from("/var/lib/panic-button-relay")
}

impl Default for RelayConfig {
    fn default() -> Self {
        RelayConfig {
            listen: d_listen(),
            tls_cert: String::new(),
            tls_key: String::new(),
            public_source_url: d_source(),
            data_dir: d_data(),
            persist: false,
            limits: Limits::default(),
            tenants: Vec::new(),
        }
    }
}

impl RelayConfig {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let text = std::fs::read_to_string(path)?;
        let c: RelayConfig = toml::from_str(&text)?;
        for t in &c.tenants {
            t.parsed().map_err(anyhow::Error::msg)?;
        }
        if !c.tls_cert.is_empty() || !c.tls_key.is_empty() {
            anyhow::bail!(
                "built-in TLS is not available yet; terminate TLS in a reverse proxy (see packaging/relay)"
            );
        }
        Ok(c)
    }

    /// The source URL for `GET /source` (AGPL section 13).
    pub fn source_url(&self) -> String {
        self.public_source_url
            .replace("{version}", env!("CARGO_PKG_VERSION"))
    }

    /// Tenants file managed by `panic-button-relay tenant ...`.
    pub fn tenants_file(&self) -> PathBuf {
        self.data_dir.join("tenants.toml")
    }
}

/// The tenants file: `[[tenant]]` tables only.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(deny_unknown_fields)]
pub struct TenantsFile {
    #[serde(default, rename = "tenant")]
    pub tenants: Vec<TenantConfig>,
}

impl TenantsFile {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let t: TenantsFile = toml::from_str(&std::fs::read_to_string(path)?)?;
        for x in &t.tenants {
            x.parsed().map_err(anyhow::Error::msg)?;
        }
        Ok(t)
    }

    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(d) = path.parent() {
            std::fs::create_dir_all(d)?;
        }
        std::fs::write(path, toml::to_string(self)?)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_example_parses() {
        let c: RelayConfig = toml::from_str(
            r#"
listen = "0.0.0.0:8443"
tls_cert = ""
tls_key  = ""
public_source_url = "https://github.com/<org>/panic-button/tree/v{version}"
data_dir = "/var/lib/panic-button-relay"
persist = true

[limits]
max_connections_per_tenant = 500
max_frame_bytes = 65536
msgs_per_sec_per_conn = 50
burst_per_conn = 200
buffer_hours = 24
buffer_max_envelopes = 10000

[[tenant]]
practice_id = "00112233445566778899aabbccddeeff"
root_pub    = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff"
name        = "Riverside Surgery"
"#,
        )
        .expect("parses");
        assert_eq!(c.tenants.len(), 1);
        assert!(c.tenants[0].parsed().is_ok());
        assert!(c.source_url().contains("/tree/v"));
    }

    #[test]
    fn unknown_keys_rejected() {
        assert!(toml::from_str::<RelayConfig>("lissen = \"0.0.0.0:1\"").is_err());
    }
}
