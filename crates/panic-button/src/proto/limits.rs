// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-PanicButton-app-store-exception

//! Size limits and protocol constants (`spec/03-protocol.md`).

/// Envelope version understood by this implementation.
pub const ENVELOPE_VERSION: u64 = 1;
/// Protocol version advertised in heartbeats.
pub const PROTO_VERSION: u64 = 1;

/// Maximum UDP datagram payload. Never fragment.
pub const UDP_MAX: usize = 1200;
/// Maximum TCP frame (excluding the 4-byte length prefix).
pub const TCP_FRAME_MAX: usize = 256 * 1024;
/// Maximum relay WebSocket frame.
pub const RELAY_WS_FRAME_MAX: usize = 64 * 1024;
/// Maximum relay long-poll batch.
pub const RELAY_LP_BATCH_MAX: usize = 256 * 1024;
/// Display strings are truncated to this many bytes on send.
pub const STRING_SEND_MAX: usize = 64;
/// Display strings over this many bytes are rejected on receive.
pub const STRING_RECV_MAX: usize = 256;
/// Longer configuration strings (URLs, host:port, schedules).
pub const CONFIG_STRING_MAX: usize = 2048;
/// Maximum interface addresses in a heartbeat.
pub const HEARTBEAT_ADDRS_MAX: usize = 8;
/// Maximum entries in one PresenceSummary.
pub const PRESENCE_ENTRIES_MAX: usize = 20;
/// Maximum certificate chain depth.
pub const CERT_CHAIN_MAX: usize = 8;
/// Upper bound on list lengths not otherwise limited (envelopes in a sync
/// response, certs in an announce, ids in a request, URLs).
pub const LIST_MAX: usize = 4096;
/// Default UDP and TCP port (to be checked against IANA before release).
pub const DEFAULT_PORT: u16 = 47474;
/// Default multicast group.
pub const DEFAULT_MCAST_GROUP: [u8; 4] = [239, 255, 71, 71];
/// mDNS service type.
pub const MDNS_SERVICE: &str = "_panic-button._udp.local.";
