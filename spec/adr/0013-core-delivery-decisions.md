# ADR 0013: Delivery and honesty decisions made while building the core

- Status: Accepted (2026-11-14), made under the "choose the safer option" rule while implementing M2. Each item is flagged for review by Marcus and the CSO.

## Context

Building `muster-core` and the simulator exposed gaps and tensions in `spec/03-protocol.md`. Each decision below picks the option more likely to deliver an alarm and less likely to mislead a user, or (for traffic) the option that keeps the network within NFR-RES-002 without weakening delivery.

## Decisions

1. **Red means nobody is reachable, even with a relay connected.** The spec's red rule says "zero peers reachable ... and no relay connected". A connected relay with no other device on it delivers nothing, so P8 (status honesty) requires red. Reachable means heard directly within `PEER_STALE`, or listed in a connected relay's presence. A device whose practice has no other members is amber "No other devices found", as the spec intends.
2. **Dual-epoch sending during key rotation.** After rotation, a device that has not yet received the new key cannot read messages sealed only in the new epoch. For `EPOCH_GRACE` after adopting an epoch, senders also seal a copy in any older epoch still advertised in a fresh heartbeat by a valid peer. Revoked devices are not valid peers, but could read such copies if they are still on the LAN; this is accepted (asset 1, availability, outranks asset 3, confidentiality) and stops once peers catch up.
3. **Any held epoch is accepted from a valid, unrevoked signer.** Signatures, not the group key, carry authenticity, so accepting an older epoch from a valid member adds no forgery risk. This lets a token-enrolled device on a stale epoch be heard, and given the current key (flow B step 3).
4. **Interrupted alerts resume after a restart.** If the agent restarts while an alarm is alerting, open, unacknowledged and not expired, the alert is shown again (`AlarmShown { resumed: true }`). P3 ("no node alerts twice") is read as excluding resumed alerts: a crash must not silence an alarm nobody has answered.
5. **An alert continues past `ALARM_MAX_LIFE` once started**, until the local user acknowledges it or it closes. Expiry only stops alarms alerting on devices that had not yet alerted (FR-ALM-008).
6. **Receipts are re-sent when the origin retransmits by unicast or TCP.** The origin unicasts retransmissions only to peers it has no Receipt from, so such a duplicate means our receipt was lost. We re-send it directly, at most once a second. This protects the sender's "Delivered to N" (FR-ALM-004).
7. **Alarm events are retransmitted** on the LAN group at 250 ms, 1 s and 3 s after sending (Seen, Responding, Resolved, Cancelled). The spec only schedules retransmits for the raise, but "Coming" and "Resolved" matter to people too.
8. **Anti-entropy waits for a persistent mismatch.** Digests differ transiently while messages are in flight, so a SyncRequest is sent only when a peer's digest differs in two consecutive heartbeats. Unknown certs are requested after 2 s (at once for alarm messages), because certs normally arrive by flooding. Without this, start-up and every alarm caused a sync storm of order N².
9. **The alarm digest covers every message.** The spec's digest used only the latest event per alarm, so two devices with the same latest event but different earlier receipts or responses would never repair. The digest now covers `(alarm_id || msg_id)` for the raise and every event (`spec/03-protocol.md`, Clarifications).
10. **Path tracking uses only messages that are not re-flooded** (heartbeats, presence, sync). For a re-flooded message the signer is not the transmitter.
11. **LAN re-flooding is probabilistic on single-interface devices.** A device with one interface that received a message by multicast or broadcast shares that segment with the transmitter, so its LAN re-flood only guards against loss (which the origin's retransmissions cover). It re-floods on the LAN with probability about 3 / (LAN-group peers). Multi-homed devices, and copies that arrived by unicast, TCP or relay, always re-flood, and unicast-only and relay paths are always covered. Without this, simultaneous re-floods inside the 150 ms jitter window grew with N (about 8 per message at 250 devices) and broke P7.

## Consequences

- All named scenarios S1 to S17 pass, including S13 at 250 devices within P7's 10 x N first transmissions.
- Items 2 and 3 change the security analysis in `spec/04-security.md` slightly; the threat model table should be reviewed with them in mind.
