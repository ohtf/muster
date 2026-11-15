// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-PanicButton-app-store-exception

//! mDNS discovery: service `_panic-button._udp.local.` with TXT `t=<practice
//! tag hex>`, `p=<tcp port>`, `v=1`. Used to find peers where multicast to
//! the Panic Button group is filtered but mDNS is allowed. Peers whose tag matches
//! ours become [`Input::Discovered`].

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddrV4};
use std::sync::{Arc, Mutex};

use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use panic_button_core::io::Input;
use panic_button_proto::limits::MDNS_SERVICE;
use tokio::sync::mpsc;

pub struct Mdns {
    daemon: ServiceDaemon,
    tcp_port: u16,
    udp_port: u16,
    instance: String,
    tag: Arc<Mutex<Option<String>>>,
}

impl std::fmt::Debug for Mdns {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Mdns")
            .field("instance", &self.instance)
            .finish()
    }
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

impl Mdns {
    pub fn start(
        tcp_port: u16,
        udp_port: u16,
        inputs: mpsc::Sender<Input>,
    ) -> Result<Self, mdns_sd::Error> {
        let daemon = ServiceDaemon::new()?;
        let rx = daemon.browse(MDNS_SERVICE)?;
        let tag: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let t2 = tag.clone();
        // Instance names are random so two devices never collide; they carry
        // no identifying information.
        let instance = format!(
            "panic-button-{:08x}",
            std::process::id() ^ (udp_port as u32) << 16
        );
        std::thread::spawn(move || {
            while let Ok(ev) = rx.recv() {
                let ServiceEvent::ServiceResolved(info) = ev else {
                    continue;
                };
                let want = t2.lock().map(|g| g.clone()).unwrap_or(None);
                let theirs = info.get_property_val_str("t").map(str::to_owned);
                if want.is_none() || theirs != want {
                    continue;
                }
                let port = info.get_port();
                for ip in info.get_addresses() {
                    if let IpAddr::V4(v4) = ip.to_ip_addr() {
                        let _ = inputs.blocking_send(Input::Discovered {
                            addr: SocketAddrV4::new(v4, port),
                        });
                    }
                }
            }
        });
        Ok(Mdns {
            daemon,
            tcp_port,
            udp_port,
            instance,
            tag,
        })
    }

    /// Advertises the current practice tag (it changes with the epoch).
    pub fn set_tag(&mut self, tag: Option<[u8; 8]>) {
        let t = tag.map(|t| hex(&t));
        {
            let mut g = self.tag.lock().unwrap_or_else(|e| e.into_inner());
            if *g == t {
                return;
            }
            g.clone_from(&t);
        }
        let Some(t) = t else { return };
        let host = format!("{}.local.", self.instance);
        let props: HashMap<String, String> = [
            ("t".to_string(), t),
            ("p".to_string(), self.tcp_port.to_string()),
            ("v".to_string(), "1".to_string()),
        ]
        .into_iter()
        .collect();
        match ServiceInfo::new(
            MDNS_SERVICE,
            &self.instance,
            &host,
            "",
            self.udp_port,
            props,
        ) {
            Ok(info) => {
                if let Err(e) = self.daemon.register(info.enable_addr_auto()) {
                    tracing::warn!("mDNS register: {e}");
                }
            }
            Err(e) => tracing::warn!("mDNS service info: {e}"),
        }
    }
}

impl Drop for Mdns {
    fn drop(&mut self) {
        let _ = self.daemon.shutdown();
    }
}
