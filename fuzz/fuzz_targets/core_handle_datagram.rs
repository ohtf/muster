// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-PanicButton-app-store-exception

//! Random bytes into an enrolled core, by every transport
//! (`spec/07-testing.md`, Fuzzing). The core must never panic, and must not
//! treat junk as an alarm.

#![no_main]

use std::net::{Ipv4Addr, SocketAddrV4};
use std::sync::OnceLock;

use libfuzzer_sys::fuzz_target;
use panic_button::core::bootstrap::{new_device_keys, new_practice};
use panic_button::core::config::LocalConfig;
use panic_button::core::io::{Input, Output, UdpKind, UiEvent};
use panic_button::core::persist::PersistedState;
use panic_button::core::rng::CoreRng;
use panic_button::core::{Core, Identity, Mono};
use panic_button::crypto::DeviceKeys;

const NOW: u64 = 1_763_100_000_000;

fn setup() -> &'static (DeviceKeys, PersistedState) {
    static S: OnceLock<(DeviceKeys, PersistedState)> = OnceLock::new();
    S.get_or_init(|| {
        let mut rng = CoreRng::from_seed([7; 32]);
        let keys = new_device_keys(&mut rng);
        let np = new_practice(&mut rng, &keys, "Fuzz Surgery", "fuzz", NOW);
        (keys, np.state)
    })
}

fuzz_target!(|data: &[u8]| {
    let Some((&sel, bytes)) = data.split_first() else {
        return;
    };
    let (keys, state) = setup();
    let mut core = Core::new(
        Identity { keys: keys.clone() },
        state.clone(),
        LocalConfig::default(),
        [9; 32],
    );
    let from = SocketAddrV4::new(Ipv4Addr::new(10, 0, 0, 2), 47474);
    let input = match sel % 5 {
        0 => Input::Datagram {
            kind: UdpKind::Multicast,
            from,
            bytes: bytes.to_vec(),
        },
        1 => Input::Datagram {
            kind: UdpKind::Broadcast,
            from,
            bytes: bytes.to_vec(),
        },
        2 => Input::Datagram {
            kind: UdpKind::Unicast,
            from,
            bytes: bytes.to_vec(),
        },
        3 => Input::StreamFrame {
            from,
            bytes: bytes.to_vec(),
        },
        _ => Input::RelayFrame {
            relay: 0,
            bytes: bytes.to_vec(),
        },
    };
    for o in core.handle(Mono(1_000), NOW, input) {
        if let Output::Ui(UiEvent::AlarmShown { .. }) = o {
            panic!("unsigned bytes raised an alarm");
        }
    }
});
