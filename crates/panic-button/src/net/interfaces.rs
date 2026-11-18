// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-PanicButton-app-store-exception

//! IPv4 interface discovery and change detection (polled every 5 s by the
//! agent; FR-NET-003).

use crate::core::io::InterfaceInfo;

#[derive(Debug, Default)]
pub struct Interfaces {
    last: Vec<InterfaceInfo>,
}

impl Interfaces {
    pub fn new() -> Self {
        let mut s = Interfaces::default();
        s.last = s.read();
        s
    }

    fn read(&self) -> Vec<InterfaceInfo> {
        let mut v: Vec<InterfaceInfo> = if_addrs::get_if_addrs()
            .unwrap_or_default()
            .into_iter()
            .filter(|i| !i.is_loopback())
            .filter_map(|i| match i.addr {
                if_addrs::IfAddr::V4(a) => Some(InterfaceInfo {
                    addr: a.ip,
                    broadcast: a.broadcast,
                }),
                _ => None,
            })
            .filter(|i| !i.addr.is_link_local() && !i.addr.is_unspecified())
            .collect();
        v.sort_by_key(|i| i.addr);
        v.dedup();
        v
    }

    pub fn list(&self) -> Vec<InterfaceInfo> {
        self.last.clone()
    }

    /// Returns the new list if it changed.
    pub fn refresh(&mut self) -> Option<Vec<InterfaceInfo>> {
        let now = self.read();
        if now == self.last {
            None
        } else {
            self.last = now.clone();
            Some(now)
        }
    }
}
