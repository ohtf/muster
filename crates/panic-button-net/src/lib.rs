// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-PanicButton-app-store-exception

//! Panic Button network I/O drivers: UDP, TCP, mDNS, relay client and interface watch.
//!
//! The drivers turn sockets into core [`Input`]s and perform the core's
//! send [`Output`]s (`spec/02-architecture.md`, Runtime). They make no
//! protocol decisions.

pub mod interfaces;
pub mod mdns;
pub mod relay;
pub mod tcp;
pub mod udp;

use std::net::Ipv4Addr;

use panic_button_core::io::{Input, Output};
use tokio::sync::mpsc;

pub use interfaces::Interfaces;

#[derive(Debug, Clone)]
pub struct NetConfig {
    pub port: u16,
    pub multicast_group: Ipv4Addr,
    pub multicast_ttl: u32,
    /// Advertise and browse with mDNS.
    pub mdns: bool,
}

impl Default for NetConfig {
    fn default() -> Self {
        NetConfig {
            port: panic_button_proto::limits::DEFAULT_PORT,
            multicast_group: Ipv4Addr::from(panic_button_proto::limits::DEFAULT_MCAST_GROUP),
            multicast_ttl: 1,
            mdns: true,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum NetError {
    #[error("network I/O: {0}")]
    Io(#[from] std::io::Error),
}

/// The running drivers.
#[derive(Debug)]
pub struct Net {
    udp: udp::UdpDriver,
    tcp: tcp::TcpDriver,
    mdns: Option<mdns::Mdns>,
    interfaces: Interfaces,
}

impl Net {
    /// Binds sockets and starts receiving. Inputs go to `inputs`.
    pub async fn start(cfg: NetConfig, inputs: mpsc::Sender<Input>) -> Result<Net, NetError> {
        let interfaces = Interfaces::new();
        let current = interfaces.list();
        let tcp = tcp::TcpDriver::start(cfg.port, inputs.clone()).await?;
        let udp = udp::UdpDriver::start(&cfg, &current, inputs.clone())?;
        let mdns = if cfg.mdns {
            match mdns::Mdns::start(tcp.port(), cfg.port, inputs.clone()) {
                Ok(m) => Some(m),
                Err(e) => {
                    tracing::warn!("mDNS unavailable: {e}");
                    None
                }
            }
        } else {
            None
        };
        let _ = inputs.send(Input::InterfacesChanged(current)).await;
        Ok(Net {
            udp,
            tcp,
            mdns,
            interfaces,
        })
    }

    /// The TCP port actually bound (advertised in heartbeats).
    pub fn tcp_port(&self) -> u16 {
        self.tcp.port()
    }

    /// Performs a send output. Other outputs are ignored.
    pub fn send(&self, o: &Output) {
        match o {
            Output::SendDatagram { dest, bytes } => self.udp.send(*dest, bytes.clone()),
            Output::SendStream { to, bytes } => self.tcp.send(*to, bytes.clone()),
            _ => {}
        }
    }

    /// Re-reads interfaces; returns an input if they changed.
    pub fn poll_interfaces(&mut self) -> Option<Input> {
        let changed = self.interfaces.refresh()?;
        self.udp.set_interfaces(&changed);
        Some(Input::InterfacesChanged(changed))
    }

    /// Updates the practice tag advertised by mDNS.
    pub fn set_mdns_tag(&mut self, tag: Option<[u8; 8]>) {
        if let Some(m) = self.mdns.as_mut() {
            m.set_tag(tag);
        }
    }
}
