# ADR 0004: LAN-first, relay optional, all paths at once

- Status: Accepted (2026-11-14)

## Context

EMIS withdrew its panic button in 2023 because some practice network configurations stopped it working. Practice networks commonly block multicast, split VLANs and isolate Wi-Fi clients. A central server would solve reachability but adds a single point of failure and an operator, and goes against the project's local-first values.

## Decision

- Devices discover each other and deliver alarms directly using UDP multicast, broadcast and unicast, TCP, mDNS and configured seed peers.
- Every device re-floods each alarm and alarm event once, with suppression, which bridges partial connectivity without configuration.
- An optional relay (self-hostable, multi-tenant, content-blind) adds an outbound-443 path. Devices use it **in addition to**, never instead of, the LAN.
- The LAN is a transport, not a trust boundary (see ADR 0005).

## Consequences

- No single failure (relay, multicast, one node) prevents delivery between devices that have another path.
- Traffic per alarm is higher than a single-path design. Suppression keeps it within 10 x N packets (P7).
- Practices with Wi-Fi client isolation need a relay for their Wi-Fi devices. The status light makes this visible (red without one).
