// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-PanicButton-app-store-exception

//! The UDP driver (`spec/03-protocol.md`, Transports and addressing).
//!
//! One socket bound to `0.0.0.0:PORT`, joined to the multicast group on
//! every interface. On Linux and macOS each datagram's destination address
//! (`IP_PKTINFO`) tells multicast, broadcast and unicast apart. Elsewhere
//! everything is reported as unicast: that is the safe mistake, because it
//! makes the core send more unicast, never less.

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::sync::{Arc, Mutex};

use panic_button_core::io::{Input, InterfaceInfo, UdpDest, UdpKind};
use socket2::{Domain, Protocol, SockRef, Socket, Type};
use tokio::net::UdpSocket;
use tokio::sync::mpsc;

use crate::{NetConfig, NetError};

/// Largest datagram we accept (the protocol limit is 1200; allow slack so
/// oversize messages are counted as decode errors, not truncated).
const RECV_BUF: usize = 2048;

#[derive(Debug)]
pub struct UdpDriver {
    tx: mpsc::UnboundedSender<(UdpDest, Vec<u8>)>,
    shared: Arc<Shared>,
}

#[derive(Debug)]
struct Shared {
    sock: Arc<UdpSocket>,
    group: Ipv4Addr,
    port: u16,
    interfaces: Mutex<Vec<InterfaceInfo>>,
}

fn bind(cfg: &NetConfig) -> std::io::Result<std::net::UdpSocket> {
    let s = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    s.set_reuse_address(true)?;
    #[cfg(all(unix, not(target_os = "solaris"), not(target_os = "illumos")))]
    s.set_reuse_port(true)?;
    s.set_broadcast(true)?;
    s.set_multicast_loop_v4(true)?;
    s.set_multicast_ttl_v4(cfg.multicast_ttl)?;
    s.set_nonblocking(true)?;
    s.bind(&SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, cfg.port).into())?;
    #[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
    {
        use nix::sys::socket::{setsockopt, sockopt};
        setsockopt(&s, sockopt::Ipv4PacketInfo, &true).map_err(std::io::Error::from)?;
    }
    Ok(s.into())
}

impl UdpDriver {
    pub fn start(
        cfg: &NetConfig,
        interfaces: &[InterfaceInfo],
        inputs: mpsc::Sender<Input>,
    ) -> Result<Self, NetError> {
        let sock = Arc::new(UdpSocket::from_std(bind(cfg)?)?);
        let shared = Arc::new(Shared {
            sock: sock.clone(),
            group: cfg.multicast_group,
            port: cfg.port,
            interfaces: Mutex::new(Vec::new()),
        });
        let driver = {
            let (tx, rx) = mpsc::unbounded_channel();
            tokio::spawn(send_loop(shared.clone(), rx));
            UdpDriver { tx, shared }
        };
        driver.set_interfaces(interfaces);
        tokio::spawn(recv_loop(driver.shared.clone(), inputs));
        Ok(driver)
    }

    /// Joins the group on new interfaces (and leaves old ones).
    pub fn set_interfaces(&self, now: &[InterfaceInfo]) {
        let mut cur = self
            .shared
            .interfaces
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        for old in cur.iter() {
            if !now.contains(old) {
                let _ = self
                    .shared
                    .sock
                    .leave_multicast_v4(self.shared.group, old.addr);
            }
        }
        for new in now {
            if !cur.contains(new)
                && let Err(e) = self
                    .shared
                    .sock
                    .join_multicast_v4(self.shared.group, new.addr)
            {
                tracing::warn!("cannot join multicast on {}: {e}", new.addr);
            }
        }
        *cur = now.to_vec();
    }

    pub fn send(&self, dest: UdpDest, bytes: Vec<u8>) {
        let _ = self.tx.send((dest, bytes));
    }
}

async fn send_loop(sh: Arc<Shared>, mut rx: mpsc::UnboundedReceiver<(UdpDest, Vec<u8>)>) {
    while let Some((dest, bytes)) = rx.recv().await {
        let ifs = sh.interfaces.lock().map(|g| g.clone()).unwrap_or_default();
        match dest {
            UdpDest::Unicast(a) => {
                if let Err(e) = sh.sock.send_to(&bytes, a).await {
                    tracing::debug!("udp send to {a}: {e}");
                }
            }
            UdpDest::Multicast => {
                // Out of every interface (sends are sequential in this task,
                // so setting the outgoing interface per send is safe).
                for i in &ifs {
                    if SockRef::from(&*sh.sock)
                        .set_multicast_if_v4(&i.addr)
                        .is_ok()
                    {
                        let _ = sh
                            .sock
                            .send_to(&bytes, SocketAddrV4::new(sh.group, sh.port))
                            .await;
                    }
                }
            }
            UdpDest::Broadcast => {
                let _ = sh
                    .sock
                    .send_to(&bytes, SocketAddrV4::new(Ipv4Addr::BROADCAST, sh.port))
                    .await;
                for i in &ifs {
                    if let Some(b) = i.broadcast {
                        let _ = sh.sock.send_to(&bytes, SocketAddrV4::new(b, sh.port)).await;
                    }
                }
            }
        }
    }
}

fn classify(dst: Option<Ipv4Addr>, group: Ipv4Addr, ifs: &[InterfaceInfo]) -> UdpKind {
    match dst {
        Some(d) if d == group || d.is_multicast() => UdpKind::Multicast,
        Some(d) if d.is_broadcast() || ifs.iter().any(|i| i.broadcast == Some(d)) => {
            UdpKind::Broadcast
        }
        _ => UdpKind::Unicast,
    }
}

async fn recv_loop(sh: Arc<Shared>, inputs: mpsc::Sender<Input>) {
    let mut buf = vec![0u8; RECV_BUF];
    loop {
        let r = recv(&sh.sock, &mut buf).await;
        let (n, from, dst) = match r {
            Ok(x) => x,
            Err(e) => {
                tracing::debug!("udp recv: {e}");
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                continue;
            }
        };
        let SocketAddr::V4(from) = from else { continue };
        let kind = {
            let ifs = sh.interfaces.lock().map(|g| g.clone()).unwrap_or_default();
            classify(dst, sh.group, &ifs)
        };
        let input = Input::Datagram {
            kind,
            from,
            bytes: buf[..n].to_vec(),
        };
        if inputs.send(input).await.is_err() {
            return;
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
async fn recv(
    sock: &UdpSocket,
    buf: &mut [u8],
) -> std::io::Result<(usize, SocketAddr, Option<Ipv4Addr>)> {
    use std::io::IoSliceMut;
    use std::os::fd::AsRawFd;

    use nix::sys::socket::{ControlMessageOwned, MsgFlags, SockaddrIn, recvmsg};
    use tokio::io::Interest;
    loop {
        sock.readable().await?;
        let r = sock.try_io(Interest::READABLE, || {
            let mut iov = [IoSliceMut::new(buf)];
            let mut cmsg = nix::cmsg_space!(nix::libc::in_pktinfo);
            let msg = recvmsg::<SockaddrIn>(
                sock.as_raw_fd(),
                &mut iov,
                Some(&mut cmsg),
                MsgFlags::empty(),
            )
            .map_err(std::io::Error::from)?;
            let mut dst = None;
            if let Ok(cms) = msg.cmsgs() {
                for c in cms {
                    if let ControlMessageOwned::Ipv4PacketInfo(info) = c {
                        dst = Some(Ipv4Addr::from(u32::from_be(info.ipi_addr.s_addr)));
                    }
                }
            }
            let from = msg
                .address
                .map(|a| SocketAddr::V4(SocketAddrV4::new(a.ip(), a.port())))
                .ok_or_else(|| std::io::Error::other("no source address"))?;
            Ok((msg.bytes, from, dst))
        });
        match r {
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
            other => return other,
        }
    }
}

#[cfg(not(any(target_os = "linux", target_os = "android", target_vendor = "apple")))]
async fn recv(
    sock: &UdpSocket,
    buf: &mut [u8],
) -> std::io::Result<(usize, SocketAddr, Option<Ipv4Addr>)> {
    let (n, from) = sock.recv_from(buf).await?;
    Ok((n, from, None))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classification() {
        let g = Ipv4Addr::new(239, 255, 71, 71);
        let ifs = [InterfaceInfo {
            addr: Ipv4Addr::new(10, 0, 0, 5),
            broadcast: Some(Ipv4Addr::new(10, 0, 0, 255)),
        }];
        assert_eq!(classify(Some(g), g, &ifs), UdpKind::Multicast);
        assert_eq!(
            classify(Some(Ipv4Addr::BROADCAST), g, &ifs),
            UdpKind::Broadcast
        );
        assert_eq!(
            classify(Some(Ipv4Addr::new(10, 0, 0, 255)), g, &ifs),
            UdpKind::Broadcast
        );
        assert_eq!(
            classify(Some(Ipv4Addr::new(10, 0, 0, 5)), g, &ifs),
            UdpKind::Unicast
        );
        assert_eq!(classify(None, g, &ifs), UdpKind::Unicast);
    }
}
