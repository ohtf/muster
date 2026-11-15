// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-PanicButton-app-store-exception

//! Peers and per-peer path tracking (`spec/03-protocol.md`, Per-peer path
//! tracking).
//!
//! Paths are recorded only from messages that are not re-flooded
//! (heartbeats, presence summaries, sync traffic), because for a flooded
//! message the signer is not necessarily the device that transmitted it.

use std::collections::BTreeMap;
use std::net::{Ipv4Addr, SocketAddrV4};

use panic_button_proto::{AudioStatus, DeviceId, Heartbeat, PathKind, SessionState, StatusLevel};

use crate::time::{ADDRESS_FALLBACK_AFTER, Mono, PEER_FORGET, PEER_STALE, WallMs};

/// Path indices.
const MCAST: usize = 0;
const BCAST: usize = 1;
const UCAST: usize = 2;
const TCP: usize = 3;
const RELAY: usize = 4;

fn idx(p: PathKind) -> Option<usize> {
    match p {
        PathKind::Multicast => Some(MCAST),
        PathKind::Broadcast => Some(BCAST),
        PathKind::UdpUnicast => Some(UCAST),
        PathKind::Tcp => Some(TCP),
        PathKind::Relay => Some(RELAY),
        _ => None,
    }
}

/// How a peer is reachable right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerClass {
    /// Heard via multicast or broadcast.
    LanGroup,
    /// Heard only by unicast UDP or TCP.
    UnicastOnly,
    /// Heard only via a relay (or present in a relay's presence list).
    RelayOnly,
    /// Known only from another peer's PresenceSummary.
    Indirect,
    /// Not heard within PEER_STALE.
    Stale,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Peer {
    pub device_id: DeviceId,
    pub heard: [Option<Mono>; 5],
    /// Last time listed by a direct peer's PresenceSummary.
    pub indirect: Option<Mono>,
    /// Last time present in a connected relay's presence list.
    pub relay_presence: Option<Mono>,
    pub last_seen_wall: WallMs,
    /// Source address of the last UDP datagram (the peer's bound port).
    pub udp_addr: Option<SocketAddrV4>,
    pub addrs: Vec<Ipv4Addr>,
    pub tcp_port: u16,
    /// The address its TCP connections came from: TCP to it is known to be
    /// routable, unlike its other addresses.
    pub tcp_ip: Option<Ipv4Addr>,
    /// The time of the last table update, for staleness checks.
    pub last_check: Mono,
    pub heartbeat: Option<Heartbeat>,
}

impl Peer {
    fn new(device_id: DeviceId) -> Self {
        Peer {
            device_id,
            heard: [None; 5],
            indirect: None,
            relay_presence: None,
            last_seen_wall: 0,
            udp_addr: None,
            addrs: Vec::new(),
            tcp_port: 0,
            tcp_ip: None,
            last_check: Mono::ZERO,
            heartbeat: None,
        }
    }

    fn fresh(t: Option<Mono>, now: Mono) -> bool {
        t.is_some_and(|t| now.since(t) <= PEER_STALE)
    }

    pub fn class(&self, now: Mono) -> PeerClass {
        let f = |i: usize| Self::fresh(self.heard[i], now);
        if f(MCAST) || f(BCAST) {
            PeerClass::LanGroup
        } else if f(UCAST) || f(TCP) {
            PeerClass::UnicastOnly
        } else if f(RELAY) || Self::fresh(self.relay_presence, now) {
            PeerClass::RelayOnly
        } else if Self::fresh(self.indirect, now) {
            PeerClass::Indirect
        } else {
            PeerClass::Stale
        }
    }

    /// TCP to this peer is likely routable: it is on our LAN segment, or
    /// has reached us by TCP recently. Not sticky, since a network change
    /// (client isolation, a moved device) can break a TCP path that once
    /// worked while UDP by another address still does.
    pub fn tcp_trusted(&self, now: Mono) -> bool {
        Self::fresh(self.heard[TCP], now) || self.class(now) == PeerClass::LanGroup
    }

    /// Reachable for status purposes: direct or via relay (not indirect).
    pub fn reachable(&self, now: Mono) -> bool {
        matches!(
            self.class(now),
            PeerClass::LanGroup | PeerClass::UnicastOnly | PeerClass::RelayOnly
        )
    }

    pub fn last_heard(&self) -> Option<Mono> {
        self.heard.iter().flatten().max().copied()
    }

    /// The best path for a PresenceSummary entry.
    pub fn best_path(&self, now: Mono) -> PathKind {
        let f = |i: usize| Self::fresh(self.heard[i], now);
        if f(MCAST) {
            PathKind::Multicast
        } else if f(BCAST) {
            PathKind::Broadcast
        } else if f(UCAST) {
            PathKind::UdpUnicast
        } else if f(TCP) {
            PathKind::Tcp
        } else {
            PathKind::Relay
        }
    }

    /// Unicast UDP destinations: the observed source address while it is
    /// working; once the peer goes quiet, every advertised address too, in
    /// case the path through the observed one has broken.
    pub fn unicast_targets(&self, port: u16) -> Vec<SocketAddrV4> {
        let mut v: Vec<SocketAddrV4> = self.udp_addr.into_iter().collect();
        let quiet = self
            .heard
            .iter()
            .take(4)
            .flatten()
            .max()
            .is_none_or(|t| self.last_check.since(*t) > ADDRESS_FALLBACK_AFTER);
        if v.is_empty() || quiet {
            for ip in &self.addrs {
                let a = SocketAddrV4::new(*ip, port);
                if !v.contains(&a) {
                    v.push(a);
                }
            }
        }
        v
    }

    /// TCP destinations.
    pub fn tcp_targets(&self) -> Vec<SocketAddrV4> {
        if self.tcp_port == 0 {
            return Vec::new();
        }
        let mut ips: Vec<Ipv4Addr> = self.tcp_ip.into_iter().collect();
        if let Some(a) = self.udp_addr
            && !ips.contains(a.ip())
        {
            ips.push(*a.ip());
        }
        for ip in &self.addrs {
            if !ips.contains(ip) {
                ips.push(*ip);
            }
        }
        ips.truncate(2);
        ips.into_iter()
            .map(|ip| SocketAddrV4::new(ip, self.tcp_port))
            .collect()
    }

    pub fn audio(&self) -> Option<AudioStatus> {
        self.heartbeat.as_ref().map(|h| h.audio)
    }

    pub fn session(&self) -> Option<SessionState> {
        self.heartbeat.as_ref().map(|h| h.session)
    }

    pub fn status(&self) -> Option<StatusLevel> {
        self.heartbeat.as_ref().map(|h| h.status)
    }
}

#[derive(Debug, Clone, Default)]
pub struct PeerTable {
    peers: BTreeMap<DeviceId, Peer>,
}

impl PeerTable {
    pub fn get(&self, d: &DeviceId) -> Option<&Peer> {
        self.peers.get(d)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Peer> {
        self.peers.values()
    }

    /// Advances the table's notion of now (used by address selection).
    pub fn set_now(&mut self, now: Mono) {
        for p in self.peers.values_mut() {
            p.last_check = now;
        }
    }

    fn entry(&mut self, d: DeviceId) -> &mut Peer {
        self.peers.entry(d).or_insert_with(|| Peer::new(d))
    }

    /// Records that `d` was heard directly on `path`.
    pub fn heard(
        &mut self,
        d: DeviceId,
        path: PathKind,
        from: Option<SocketAddrV4>,
        now: Mono,
        wall: WallMs,
    ) {
        let p = self.entry(d);
        if let Some(i) = idx(path) {
            p.heard[i] = Some(now);
        }
        p.last_seen_wall = wall;
        if path == PathKind::Tcp {
            p.tcp_ip = from.map(|a| *a.ip());
        }
        if let (Some(a), PathKind::Multicast | PathKind::Broadcast | PathKind::UdpUnicast) =
            (from, path)
        {
            p.udp_addr = Some(a);
        }
    }

    pub fn heartbeat(&mut self, d: DeviceId, hb: Heartbeat) {
        let p = self.entry(d);
        p.addrs = hb
            .addrs
            .iter()
            .map(|a| Ipv4Addr::from(*a))
            .filter(|ip| !ip.is_loopback() && !ip.is_unspecified())
            .collect();
        p.tcp_port = hb.tcp_port;
        p.heartbeat = Some(hb);
    }

    pub fn indirect(&mut self, d: DeviceId, now: Mono) {
        self.entry(d).indirect = Some(now);
    }

    /// A relay this device was present on disconnected.
    pub fn relay_gone(&mut self, d: &DeviceId) {
        if let Some(p) = self.peers.get_mut(d) {
            p.relay_presence = None;
            p.heard[RELAY] = None;
        }
    }

    pub fn relay_present(&mut self, d: DeviceId, now: Mono) {
        self.entry(d).relay_presence = Some(now);
    }

    /// Finds the peer whose last UDP source address is `a`.
    pub fn by_udp_addr(&self, a: &SocketAddrV4) -> Option<DeviceId> {
        self.peers
            .values()
            .find(|p| p.udp_addr.as_ref() == Some(a))
            .map(|p| p.device_id)
    }

    pub fn by_ip(&self, ip: &Ipv4Addr) -> Option<DeviceId> {
        self.peers
            .values()
            .find(|p| p.udp_addr.is_some_and(|a| a.ip() == ip) || p.addrs.contains(ip))
            .map(|p| p.device_id)
    }

    pub fn remove(&mut self, d: &DeviceId) {
        self.peers.remove(d);
    }

    /// Forgets peers not seen for PEER_FORGET.
    pub fn prune(&mut self, wall: WallMs) {
        self.peers
            .retain(|_, p| wall.saturating_sub(p.last_seen_wall) <= PEER_FORGET);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classes() {
        let mut t = PeerTable::default();
        let d = DeviceId([1; 32]);
        t.heard(d, PathKind::Relay, None, Mono(0), 0);
        assert_eq!(
            t.get(&d).map(|p| p.class(Mono(1))),
            Some(PeerClass::RelayOnly)
        );
        t.heard(d, PathKind::Tcp, None, Mono(10), 0);
        assert_eq!(
            t.get(&d).map(|p| p.class(Mono(11))),
            Some(PeerClass::UnicastOnly)
        );
        t.heard(d, PathKind::Broadcast, None, Mono(20), 0);
        assert_eq!(
            t.get(&d).map(|p| p.class(Mono(21))),
            Some(PeerClass::LanGroup)
        );
        assert_eq!(
            t.get(&d).map(|p| p.class(Mono(20 + PEER_STALE + 1))),
            Some(PeerClass::Stale)
        );
    }
}
