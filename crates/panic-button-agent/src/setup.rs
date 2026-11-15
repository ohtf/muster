// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-PanicButton-app-store-exception

//! Offline setup on a data directory, shared by the CLI and the desktop
//! app: create a practice (flow A in `spec/04-security.md`), create an
//! enrolment token (flow B), and make a new recovery kit.

use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use panic_button_core::bootstrap::{new_device_keys, new_practice};
use panic_button_core::rng::CoreRng;
use panic_button_crypto::TrustStore;
use panic_button_crypto::recovery::{
    KdfParams, RecoveryContents, check_passphrase, open_kit, seal_kit,
};
use panic_button_crypto::token::{TokenRequest, create_token};
use panic_button_proto::{Cbor, Cert, CertSerial, DeviceId, TokenId};
use panic_button_store::Store;

const DAY_MS: u64 = 24 * 60 * 60 * 1000;

fn wall_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Writes a new file readable only by its owner; fails if it exists.
pub fn write_private(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts
        .open(path)
        .with_context(|| format!("creating {} (it must not already exist)", path.display()))?;
    f.write_all(bytes)?;
    f.sync_all()?;
    Ok(())
}

fn open_store(dir: &Path) -> anyhow::Result<Store> {
    std::fs::create_dir_all(dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(Store::open(&dir.join("panic-button.db"))?)
}

/// What a new practice needs to be registered elsewhere (for example with
/// a relay operator).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewPractice {
    pub name: String,
    pub practice_id: String,
    pub root_pub: String,
    pub admin_device: String,
    pub recovery_kit: PathBuf,
}

/// Is this data directory already enrolled in a practice?
pub fn is_enrolled(dir: &Path) -> anyhow::Result<bool> {
    if !dir.join("panic-button.db").exists() {
        return Ok(false);
    }
    Ok(Store::open(&dir.join("panic-button.db"))?
        .load()?
        .practice
        .is_some())
}

/// Creates a practice on `dir`; this device becomes its first admin. The
/// recovery kit is sealed with `passphrase` (checked first: at least 12
/// characters) and written to `kit_path`, which must not exist. Deriving
/// the kit key takes a few seconds.
pub fn init_practice(
    dir: &Path,
    name: &str,
    label: &str,
    passphrase: &str,
    kit_path: &Path,
) -> anyhow::Result<NewPractice> {
    check_passphrase(passphrase)?;
    if name.trim().is_empty() {
        bail!("the practice needs a name");
    }
    let mut store = open_store(dir)?;
    if store.load()?.practice.is_some() {
        bail!("{} already belongs to a practice", dir.display());
    }
    let mut rng = CoreRng::from_seed(crate::runtime::os_seed()?);
    let keys = match store.load_identity()? {
        Some(k) => k,
        None => {
            let k = new_device_keys(&mut rng);
            store.save_identity(&k)?;
            k
        }
    };
    let now = wall_now();
    let np = new_practice(&mut rng, &keys, name.trim(), label, now);
    let practice = np.state.practice.clone().context("practice")?;
    let contents = RecoveryContents {
        practice_id: practice.practice_id,
        practice_name: practice.name.clone(),
        root: np.root,
        created_at: now,
    };
    let kit = seal_kit(
        &contents,
        passphrase,
        rng.bytes(),
        rng.bytes(),
        KdfParams::V1,
    )?;
    // Prove the passphrase opens it before relying on it.
    let check = open_kit(&kit, passphrase)?;
    if check.root.public() != practice.root_pub {
        bail!("recovery kit check failed");
    }
    drop(contents);
    write_private(kit_path, &kit.to_cbor())?;
    store.apply_all(np.state.to_ops().iter())?;
    Ok(NewPractice {
        name: practice.name,
        practice_id: practice.practice_id.to_hex(),
        root_pub: DeviceId(practice.root_pub).to_hex(),
        admin_device: keys.device_id().to_hex(),
        recovery_kit: kit_path.to_path_buf(),
    })
}

/// A recovery kit written by [`new_recovery_kit`].
#[derive(Debug, Clone)]
pub struct NewKit {
    pub practice: String,
    pub path: PathBuf,
}

/// Opens a kit file, with errors in words for staff.
fn open_kit_file(path: &Path, pass: &str) -> anyhow::Result<RecoveryContents> {
    let bytes = std::fs::read(path).with_context(|| format!("cannot read {}", path.display()))?;
    let kit = panic_button_crypto::recovery::RecoveryKit::from_cbor(&bytes)
        .map_err(|_| anyhow::anyhow!("that file is not a Panic Button recovery kit"))?;
    open_kit(&kit, pass)
        .map_err(|_| anyhow::anyhow!("the passphrase does not open this recovery kit"))
}

/// Writes a new recovery kit for the same practice with a new passphrase
/// ("Create a new recovery kit", `spec/05-desktop-agent.md`). Nothing in the
/// practice changes, so the old kit keeps working: destroy it once the new
/// one is stored safely. If `dir` belongs to a practice, the kit must be
/// for that practice. Opening and sealing take a few seconds each.
pub fn new_recovery_kit(
    dir: Option<&Path>,
    old_kit: &Path,
    old_pass: &str,
    new_kit: &Path,
    new_pass: &str,
) -> anyhow::Result<NewKit> {
    check_passphrase(new_pass)?;
    if new_kit.exists() {
        bail!("{} already exists; choose another name", new_kit.display());
    }
    let old = open_kit_file(old_kit, old_pass)?;
    if let Some(dir) = dir
        && dir.join("panic-button.db").exists()
        && let Some(p) = open_store(dir)?.load()?.practice
        && p.practice_id != old.practice_id
    {
        bail!(
            "this recovery kit is for {}, not {}",
            old.practice_name,
            p.name
        );
    }
    let root_pub = old.root.public();
    let contents = RecoveryContents {
        practice_id: old.practice_id,
        practice_name: old.practice_name.clone(),
        root: old.root,
        created_at: wall_now(),
    };
    let mut rng = CoreRng::from_seed(crate::runtime::os_seed()?);
    let kit = seal_kit(&contents, new_pass, rng.bytes(), rng.bytes(), KdfParams::V1)?;
    if open_kit(&kit, new_pass)?.root.public() != root_pub {
        bail!("recovery kit check failed");
    }
    write_private(new_kit, &kit.to_cbor())?;
    Ok(NewKit {
        practice: contents.practice_name.clone(),
        path: new_kit.to_path_buf(),
    })
}

/// A new enrolment token, as text to hand over.
#[derive(Debug, Clone)]
pub struct NewToken {
    pub token_id: String,
    pub expires_at: u64,
    pub text: String,
}

/// Options for [`create_enrolment_token`].
#[derive(Debug, Clone, Default)]
pub struct TokenOptions {
    pub days: u64,
    pub label: String,
    pub relay_urls: Vec<String>,
    pub seed_peers: Vec<String>,
}

/// Creates an enrolment token on an admin device's data directory. The
/// token text is a secret: treat it like a password.
pub fn create_enrolment_token(dir: &Path, o: &TokenOptions) -> anyhow::Result<NewToken> {
    if o.days == 0 || o.days > 365 {
        bail!("a token lasts 1 to 365 days");
    }
    let store = Store::open(&dir.join("panic-button.db"))?;
    let keys = store
        .load_identity()?
        .context("this data directory has no identity")?;
    let state = store.load()?;
    let practice = state
        .practice
        .clone()
        .context("this device is not enrolled")?;
    let now = wall_now();
    let mut trust = TrustStore::new(practice.practice_id, practice.root_pub);
    for raw in &state.certs {
        if let Ok(c) = Cert::from_cbor(raw) {
            let _ = trust.add_cert(c);
        }
    }
    if !trust.is_admin(&keys.device_id(), now) {
        bail!("this device is not a Practice Admin");
    }
    let key = state
        .group_keys
        .iter()
        .max_by_key(|k| (k.epoch, k.key.rotation_rank()))
        .context("no group key")?;
    let chain: Vec<Vec<u8>> = trust
        .chain_for(&keys.device_id(), now)
        .context("no valid admin chain")?
        .iter()
        .map(|c| c.to_cbor())
        .collect();
    let mut rng = CoreRng::from_seed(crate::runtime::os_seed()?);
    let token_id = TokenId(rng.bytes());
    let (token, enroller_cert) = create_token(
        &keys,
        rng.bytes(),
        &key.key,
        TokenRequest {
            practice_id: practice.practice_id,
            practice_name: practice.name.clone(),
            root_pub: practice.root_pub,
            label: o.label.clone(),
            token_id,
            enroller_serial: CertSerial(rng.bytes()),
            issued_at: now,
            expires_at: now + o.days * DAY_MS,
            epoch: key.epoch,
            relay_urls: o.relay_urls.clone(),
            seed_peers: o.seed_peers.clone(),
            network: Default::default(),
            issuer_chain: chain,
        },
    )?;
    store.apply(&panic_button_core::persist::PersistOp::AddCert(
        enroller_cert.to_cbor(),
    ))?;
    Ok(NewToken {
        token_id: token_id.to_hex(),
        expires_at: token.expires_at,
        text: token.to_token_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_recovery_kit_opens_with_the_new_passphrase_only() {
        let d = tempfile::tempdir().expect("tempdir");
        let dir = d.path().join("state");
        let old = d.path().join("old.kit");
        let p = init_practice(
            &dir,
            "Test Surgery",
            "Reception",
            "the first passphrase",
            &old,
        )
        .expect("init");
        let new = d.path().join("new.kit");
        let k = new_recovery_kit(
            Some(&dir),
            &old,
            "the first passphrase",
            &new,
            "the second passphrase",
        )
        .expect("new kit");
        assert_eq!(k.practice, "Test Surgery");
        let c = open_kit_file(&new, "the second passphrase").expect("open new");
        assert_eq!(DeviceId(c.root.public()).to_hex(), p.root_pub);
        assert!(open_kit_file(&new, "the first passphrase").is_err());
        // The old kit still works.
        assert!(open_kit_file(&old, "the first passphrase").is_ok());

        let e = |r: anyhow::Result<NewKit>| r.expect_err("should fail").to_string();
        let other = d.path().join("other.kit");
        assert!(
            e(new_recovery_kit(
                None,
                &old,
                "the wrong passphrase",
                &other,
                "the second passphrase"
            ))
            .contains("does not open")
        );
        assert!(
            e(new_recovery_kit(
                None,
                &old,
                "the first passphrase",
                &new,
                "the second passphrase"
            ))
            .contains("already exists")
        );
        assert!(
            e(new_recovery_kit(
                None,
                &old,
                "the first passphrase",
                &other,
                "short"
            ))
            .contains("12")
        );
        assert!(!other.exists());

        // A kit for another practice is refused on this device.
        let dir2 = d.path().join("state2");
        let old2 = d.path().join("old2.kit");
        init_practice(
            &dir2,
            "Other Surgery",
            "Desk",
            "the other passphrase",
            &old2,
        )
        .expect("init 2");
        assert!(
            e(new_recovery_kit(
                Some(&dir),
                &old2,
                "the other passphrase",
                &other,
                "the second passphrase"
            ))
            .contains("Other Surgery")
        );
    }

    #[test]
    fn practice_then_token() {
        let d = tempfile::tempdir().expect("tempdir");
        let dir = d.path().join("state");
        assert!(!is_enrolled(&dir).expect("check"));
        assert!(init_practice(&dir, "T", "Reception", "short", &d.path().join("kit")).is_err());
        let p = init_practice(
            &dir,
            "Test Surgery",
            "Reception",
            "a long enough passphrase",
            &d.path().join("kit"),
        )
        .expect("init");
        assert_eq!(p.name, "Test Surgery");
        assert!(is_enrolled(&dir).expect("check"));
        assert!(
            init_practice(
                &dir,
                "Again",
                "x",
                "a long enough passphrase",
                &d.path().join("k2")
            )
            .is_err()
        );
        let t = create_enrolment_token(
            &dir,
            &TokenOptions {
                days: 1,
                label: "test".into(),
                ..Default::default()
            },
        )
        .expect("token");
        assert!(panic_button_proto::Token::parse(&t.text).is_ok());
        assert!(
            create_enrolment_token(
                &dir,
                &TokenOptions {
                    days: 0,
                    ..Default::default()
                }
            )
            .is_err()
        );
    }
}
