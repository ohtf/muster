# 00 Overview

## Problem

Staff in GP practices are sometimes threatened or attacked, or face a medical emergency in a room on their own. They need a fast, discreet way to call colleagues nearby. Existing on-screen panic alarms are proprietary, priced per seat or per site, Windows-only, and give no evidence that they will work on a given practice network. Networks have also changed: laptops and tablets on Wi-Fi with client isolation, VLAN segmentation and remote sessions all break naive LAN broadcast designs.

## Goals for v1

1. Desktop parity with commercial on-screen panic alarms: an always-available on-screen button and a keyboard shortcut that raise an alarm on every other enrolled device in the practice.
2. Run on Windows 10/11, macOS and Linux.
3. Zero-touch deployment on managed NHS estates (MSI with properties, Group Policy/Intune) and simple manual setup elsewhere.
4. Work on awkward networks: multicast blocked, multiple VLANs and Wi-Fi isolation (the last with the optional relay).
5. Prove it works: honest per-device status, a coverage view for the Practice Admin, and scheduled drills with reports.
6. Resist false alarms (accidental, stale or spoofed) without delaying genuine ones.
7. Ship with a protocol specification, a deterministic network simulator and a DCB0129-style hazard log.

## Non-goals for v1

- Mobile apps (phase 3). The protocol must not preclude them.
- Hardware buttons (post-v1). The protocol carries a `source` field ready for them.
- Monitored alarm services, calling 999, or alerting anyone outside the practice.
- Multiple alarm types. There is one alarm, meaning "come and help".
- Integrations (Home Assistant, ntfy, webhooks): post-v1, through a headless integration node.
- Running inside Citrix/RDS sessions or on thin clients (detected and reported, not supported).
- The household profile beyond configuration defaults.

## Roles

| Role | Who | Can do |
|---|---|---|
| Staff user | Anyone working at a device | Raise an alarm; respond to alarms; cancel their own alarm as sent in error; resolve alarms |
| Responder | Any staff user receiving an alarm | "I'm coming" or "Seen - can't come"; resolve |
| Practice Admin | Practice manager, lead partner, IT manager or similar. At least two per practice are recommended | Create the practice; approve and revoke devices; create enrolment tokens; set rooms; configure drills and practice settings; view coverage and history; hold the recovery kit |
| IT deployer | ICB/CSU/practice IT | Deploy the MSI with an enrolment token and policies |
| Relay operator | The practice, a PCN or federation, or OHTF | Run an optional relay. Cannot read alarm content |

## Glossary

- **Practice**: a trust group of devices, identified by a 16-byte practice id and a root public key. One building may host several practices. They are isolated from each other.
- **Device / node**: one enrolled installation, identified by its Ed25519 public key (the device id).
- **Peer**: another device this device has recently heard from.
- **Alarm**: a request for help. It has an id, an origin device, a room, a user display name and a lifecycle.
- **Receipt**: an automatic machine-level acknowledgement that a device received and displayed an alarm.
- **Seen / Responding**: human acknowledgements from a receiving user.
- **Resolved / Cancelled**: terminal alarm states. Cancelled means "sent in error".
- **Drill**: a test alarm, flagged as such, used to measure coverage.
- **Relay**: an optional server that forwards encrypted envelopes between devices of a practice.
- **Epoch**: the generation of the practice group encryption key. It increments when a device is revoked.
- **Certificate (cert)**: a signed statement that a device key belongs to a practice, with a role.
- **Enrolment token**: a secret string IT can deploy so devices enrol without manual approval.
- **Profile**: a set of defaults: `practice` (v1) or `home` (later).
- **Near miss**: a hold that was released after 300 ms but before the trigger threshold.

## Decisions already made (with Marcus, 2026-11-14)

- Licence AGPL-3.0-or-later with an app store exception and DCO (ADR 0001).
- The practice owns its deployment: Practice Admin role, practice-run relay, practice-held keys. For mobile (later), OHTF runs only a content-blind push forwarder.
- Activation: press and hold for 1.5 s on the floating button (or the shortcut). A soft rising cue plays only during the hold. The sender's device makes no alarm sound (ADR 0008).
- A single alarm type: "come and help" (ADR 0007).
- MVP = phase 1 (desktop + LAN mesh) + phase 2 (optional relay).
- OHTF-branded USB HID buttons are a planned optional purchase that helps fund the project (post-v1).
- A household or self-hosted profile is a planned second market (post-v1).

## v1 success criteria

- All simulator properties in `07-testing.md` pass in CI.
- The hardware lab protocol passes in full, including Wi-Fi client isolation with a relay.
- Silent MSI deployment is demonstrated with an enrolment token on a managed Windows 11 machine.
- Alarm shown on all directly reachable LAN devices within 2 s (p99), and via relay within 5 s (p99).
- The hazard log has been reviewed and signed off by a Clinical Safety Officer.
- A pilot in at least one practice for 4 weeks, with weekly drills and no unexplained missed deliveries.
