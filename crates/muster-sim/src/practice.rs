// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-Muster-app-store-exception

//! Practice set-up for simulated worlds: the first node of each practice
//! creates it (flow A); every other node enrols with a token (flow B).

use std::collections::BTreeMap;

use muster_core::bootstrap::{enrol_with_token_state, new_device_keys, new_practice};
use muster_core::persist::PersistedState;
use muster_core::rng::CoreRng;
use muster_crypto::token::{TokenRequest, create_token};
use muster_crypto::{DeviceKeys, GroupKey, SigningSecret};
use muster_proto::{Cbor, CertSerial, PracticeId, Token, TokenId};

use crate::world::{WALL_BASE, WorldSpec};

pub struct PracticeSetup {
    pub practice_id: PracticeId,
    pub admin_node: usize,
    pub root: SigningSecret,
    pub group_key: GroupKey,
    pub token: Token,
    states: BTreeMap<usize, (DeviceKeys, PersistedState)>,
}

impl std::fmt::Debug for PracticeSetup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PracticeSetup({:?})", self.practice_id)
    }
}

impl PracticeSetup {
    pub fn node_state(&self, node: usize) -> (DeviceKeys, PersistedState) {
        self.states.get(&node).cloned().unwrap_or_else(|| {
            (
                DeviceKeys::from_secrets([0; 32], [0; 32]),
                PersistedState::default(),
            )
        })
    }
}

pub fn setup(rng: &mut CoreRng, spec: &WorldSpec) -> Vec<PracticeSetup> {
    let mut out = Vec::new();
    for p in 0..spec.practices.max(1) {
        let members: Vec<usize> = spec
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.practice == p)
            .map(|(i, _)| i)
            .collect();
        let Some(&admin_node) = members.first() else {
            continue;
        };
        let admin = new_device_keys(rng);
        let np = new_practice(
            rng,
            &admin,
            &format!("Practice {p}"),
            &format!("N{admin_node}"),
            WALL_BASE - 1000,
        );
        let practice = np.state.practice.clone().expect("practice");
        let group_key = np.state.group_keys[0].key.clone();
        let issuer_chain = np.state.certs.clone();
        let (token, _enroller_cert) = create_token(
            &admin,
            rng.bytes(),
            &group_key,
            TokenRequest {
                practice_id: practice.practice_id,
                practice_name: practice.name.clone(),
                root_pub: practice.root_pub,
                label: "Simulator rollout".into(),
                token_id: TokenId(rng.bytes()),
                enroller_serial: CertSerial(rng.bytes()),
                issued_at: WALL_BASE - 1000,
                expires_at: WALL_BASE + 30 * 24 * 3_600_000,
                epoch: 0,
                relay_urls: vec![],
                seed_peers: vec![],
                network: Default::default(),
                issuer_chain,
            },
        )
        .expect("token");
        let mut states = BTreeMap::new();
        states.insert(admin_node, (admin, np.state));
        let admin_chain = states
            .get(&admin_node)
            .map(|(_, s)| s.certs.clone())
            .unwrap_or_default();
        for &m in &members[1..] {
            let keys = new_device_keys(rng);
            let mut st =
                enrol_with_token_state(rng, &keys, &token, &format!("N{m}"), WALL_BASE - 500)
                    .expect("enrol");
            if spec.admins.contains(&m) {
                // Promoted by the first admin.
                let serial = CertSerial(rng.bytes());
                let cert = muster_crypto::issue_cert(
                    &states[&admin_node].0,
                    muster_proto::CertBody {
                        version: muster_proto::cert::CERT_VERSION,
                        practice_id: practice.practice_id,
                        serial,
                        subject: keys.device_id(),
                        subject_kx: keys.kx_pub(),
                        role: muster_proto::Role::Admin,
                        label: format!("N{m}"),
                        issued_at: WALL_BASE - 400,
                        expires_at: 0,
                        issuer: [0; 32],
                        token_id: None,
                    },
                );
                st.certs.extend(admin_chain.iter().cloned());
                st.certs.push(cert.to_cbor());
            }
            states.insert(m, (keys, st));
        }
        let _ = token.to_cbor();
        out.push(PracticeSetup {
            practice_id: practice.practice_id,
            admin_node,
            root: np.root,
            group_key,
            token,
            states,
        });
    }
    out
}
