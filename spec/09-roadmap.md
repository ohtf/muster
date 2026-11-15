# 09 Roadmap

Legend: `[x]` done, `[~]` in progress, `[ ]` not started. Item codes (`R1`, `R2`, ...) are stable: never renumbered or reused (see [house-style specs.md](https://github.com/pacharanero/house-style/blob/main/specs.md#stable-roadmap-item-codes)). Exit criteria are tasks too.

## v1 = phase 1 (desktop + LAN mesh) + phase 2 (optional relay)

Build in this order. Each milestone ends with green CI and a short demo note in the PR (or, while the repo is committing direct to `main`, in the commit body).

### M0 Repository and CI

- [x] **R1 - Workspace skeleton** per `02-architecture.md`.
- [x] **R2 - Licence, security and conduct files**: fetch the AGPL licence text; write `SECURITY.md`; add `CODE_OF_CONDUCT.md` (fetched text).
- [x] **R3 - CI**: fmt, clippy, test, cargo-deny, frontend lint placeholders, build matrix (Linux, Windows, macOS).
- [x] **R4 - Commit the ADRs** in `spec/adr/`.
- [x] **R5 - M0 exit**: empty crates build on all three OSes. CI green.

### M1 Protocol and crypto

- [x] **R6 - `muster-proto`**: all message types, deterministic CBOR, limits, golden vectors.
- [x] **R7 - `muster-crypto`**: keys, certs, chain validation, seal and open, KeyShare, recovery kit, tokens.
- [x] **R8 - Fuzz targets** for all decoders, plus `core_handle_datagram` (random bytes into an enrolled core by every transport).
- [~] **R9 - M1 exit**: unit tests for every rule in `03-protocol.md` and `04-security.md`. Fuzzers run clean for 10 minutes each.

### M2 Core and simulator

- [x] **R10 - Core: peers and path tracking, heartbeats.**
- [x] **R11 - Core: alarm raise and receive pipeline**, re-flood with suppression, receipts.
- [x] **R12 - Core: alarm state merge and expiry.**
- [x] **R13 - Core: hold machine** (FR-ACT-002, FR-ACT-005, FR-ACT-008, FR-ACT-009).
- [x] **R14 - Core: status computation** (FR-STS-003).
- [x] **R15 - Core: anti-entropy** (digests, SyncRequest, SyncResponse).
- [x] **R16 - Core: enrolment flows** (token and interactive).
- [x] **R17 - Core: revocation and rotation.**
- [x] **R18 - Core: drills.**
- [x] **R19 - Core: practice config.**
- [x] **R20 - `muster-sim` world model**: segments, routing, relays, schedules.
- [x] **R21 - Simulator scenarios S1 to S17.**
- [x] **R22 - Simulator properties P1 to P12.**
- [x] **R23 - M2 exit**: all properties and scenarios pass at 256 cases in CI and 1,000 cases in one nightly run (scaled down from 10,000 on 2026-11-14 by Marcus Baw, to raise again once the nightly is reliable). **Do not start M4 until this passes.** Met 2026-11-14: CI run 43621103526 (256 cases) and nightly run 43642345393 (1,000 cases per property, S13 at 250 devices, 10 minutes of fuzzing per target).

### M3 Runtime, store, headless agent, CLI

- [x] **R24 - `muster-store`**: SQLite schema and migrations.
- [x] **R25 - `muster-net`**: UDP, TCP, mDNS, interface watch.
- [x] **R26 - `muster-agent`**: runtime, control API, headless binary.
- [x] **R27 - `muster-cli`**: `practice init`, `token create/revoke/list`, `device list/revoke/rename/room`, `drill run/report`, `status`, `peers`, `ctl` commands. Also `device promote`. Rename and room use the DeviceSettings message (ADR 0014); drill reports are kept in the store (FR-DRL-007).
- [x] **R28 - netns integration tests** (S1, S2, S4, S7 without relay).
- [x] **R29 - M3 exit**: three headless agents in netns raise and receive alarms. The CLI can set up a practice end to end.

### M4 Desktop app

- [~] **R30 - Tauri 2 + Svelte 5 app** embedding the agent: floating button, hold ring and cue, sender panel, receiver alert window, tray, status, main window (status, history, settings), first run, practice run. Prototype working end to end on Linux (`docs/gui/`). Still to do: join by approval code, global shortcut, button position memory, eslint and prettier checks.
- [~] **R31 - Generated sounds** (CC0). Patterns A to C and the drill chime are generated WAV files in `sounds/` (`s/sounds`), replaceable per device; the hold cue is synthesised. Needs a listening check on real speakers.
- [~] **R32 - Platform modules**: Windows audio endpoint (status and override), session detection, DPAPI; macOS Local Network detection; Linux Wayland fallbacks. Done: the Windows control API named pipe (Q7). Still to do: the rest.
- [ ] **R33 - M4 exit**: manual run-through of every screen on all three OSes. Accessibility check (keyboard only, screen reader labels, contrast).

### M5 Admin and drills in the app

- [~] **R34 - Admin section**: coverage, devices (approve, revoke, rename, rooms, promote), tokens, drills (schedule, run, reports, CSV), practice settings, admins and recovery, audit export. Done: devices with a needs-attention filter, rename, room, promote and remove; join approvals; enrolment codes (create, copy, revoke, devices per code); test alarms with reports and CSV copy; a warning when there is only one admin; practice settings (hold time, cue, alarm pattern, drill mode, opening hours, relay URLs; also `muster practice settings`). Also audit log export as CSV (`muster ctl audit-export`). Also weekly test alarms run from a chosen coordinator, with skips recorded on admin devices (FR-DRL-001, FR-DRL-006), and "Recover admin access" with the recovery kit on an enrolled device (Settings, or `muster practice recover`). Not yet: recovery on a device that is not enrolled, and "Create a new recovery kit".
- [ ] **R35 - M5 exit**: a practice can be set up, enrolled, drilled and administered entirely from the GUI.

### M6 Relay

- [x] **R36 - `muster-relay`**: WebSocket and long-poll, tenants, auth, forwarding, buffer and resume, presence, revocation enforcement, `/source`, metrics.
- [x] **R37 - Relay client** in `muster-net`, including proxy support and long-poll fallback. NTLM and Kerberos proxy auth are not supported (S for v1, see `06-relay.md`).
- [x] **R38 - Relay netns tests** S5, S6, S12, S14 with a real relay. Docker image and compose example. Also S5 over long-poll; `packaging/relay/` has the image, compose with Caddy, and a hardened systemd unit.
- [x] **R39 - M6 exit**: L3, L5 and L8 equivalents pass in netns. L3 is S5 (two VLANs, relay; delivery within 5 s, amber where relay-only), L5 is S6 (Wi-Fi isolation plus relay; laptops receive and raise within 5 s), L8 is S12 (relay down mid-alarm; LAN unaffected, relay-only device catches up).

### M7 Packaging

- [ ] **R40 - Windows MSI** with properties, firewall, Run key, WebView2, signing (with a placeholder certificate in CI), Scheduled Task watchdog registration on first run.
- [ ] **R41 - macOS pkg** (signing and notarisation on tags), LaunchAgent.
- [ ] **R42 - Linux** deb, AppImage and systemd units.
- [ ] **R43 - M7 exit**: silent MSI install smoke test in CI. L9 passes in the lab.

### M7a Home test (Windows and Linux installers first)

Priority set by Marcus Baw on 2026-11-15: a Windows installer and Linux packages good enough for a limited test on several Windows and Linux machines at home, ahead of the rest of M7. The full R40 to R43 items still apply afterwards.

- [~] **R52 - Installer builds in CI**: a workflow that builds the Windows MSI and the Linux deb and AppImage from any commit (manual run) and on tags, and keeps them as downloadable artifacts. Unsigned for now: Windows SmartScreen will warn. Workflow written (`.github/workflows/installers.yml`); its first run was refused by GitHub billing.
- [~] **R53 - Windows MSI basics**: per-machine install to Program Files, Windows Firewall rules for UDP and TCP 47474 (so the first alarm is not blocked behind a firewall prompt), starts at sign-in for every user, a clean uninstall that removes the rules. Signing, policies, the WebView2 bootstrapper check and the watchdog stay in R40. `packaging/windows/muster.wxs`; not yet built or installed on Windows.
- [~] **R54 - Linux packages basics**: a deb and an AppImage that start the app at sign-in (XDG autostart), with the same sounds and icons. Deb built and inspected locally; AppImage built in an Ubuntu 22.04 container.
- [~] **R55 - Home test guide**: a short guide for installing on a few machines, setting up a practice on one, adding the others with an enrolment code, and the checks to run (alarm both ways, a computer switched off, a test alarm). `docs/home-test.md`.

### Documentation

The guides for people who use Muster, in plain English for practice staff. Every page is checked against the "Signs of AI writing" patterns (Wikipedia; also the humanizer skill) before it is committed, and screenshots come from the real app.

- [x] **R56 - Writing checklist**: the AI-writing patterns to avoid, as a short checklist in `docs/` that every page is checked against. Fix the patterns already in `README.md`. `docs/writing-checklist.md`; README fixed.
- [ ] **R57 - Staff quick guide**: one printable page. How to call for help, what the status shapes mean, answering an alert, cancelling a mistake.
- [ ] **R58 - Practice Admin guide**: setting up the practice, the recovery kit, adding and removing computers, rooms and names, test alarms and their reports, what to do when a computer shows amber or red.
- [ ] **R59 - Docs site** with Zensical (house style `docs.md`), published to GitHub Pages, with a check that every page is in the navigation.
- [ ] **R60 - Install guides**, written with the installers and tested against them: for practice staff installing on one computer (Windows first), and for IT deploying the MSI silently with policy settings. They become part of R46.

### M8 Hardening and release candidate

- [ ] **R44 - Hardware lab protocol** L1 to L14 executed and recorded.
- [ ] **R45 - Hazard log** updated with evidence and residual risks.
- [ ] **R46 - Documentation set** from `08-deployment.md`.
- [ ] **R47 - CSO review and sign-off.**
- [ ] **R48 - M8 exit**: v1.0.0-rc.1 tagged and pilot started.

### Pre-release checks (from the original spec notes)

- [ ] **R49 - Legal review** of the app store additional permission before the first external contribution (ADR 0001).
- [ ] **R50 - Check port 47474** against the IANA service name and port registry.
- [ ] **R51 - Trade mark check** on the working name "Muster".

## Post-v1

### 1.1 Integration node and home profile

- Headless agent "integration outputs": ntfy, Gotify, Matrix, generic webhook, MQTT (Home Assistant discovery). These give mobile *receiving* via existing apps before native apps exist, and let self-hosters drive lights and sirens.
- Inputs: a Home Assistant or MQTT trigger can raise an alarm (`source = integration`; disabled by default; requires an explicit admin setting).
- Home profile defaults tested. Docker image for "relay + integration node" in one compose file.

### 1.2 OHTF USB button (hardware)

- Open hardware under `hardware/` (CERN-OHL-S-2.0). A low-cost microcontroller (for example RP2040 or CH552) with a large momentary switch.
- Presents a **vendor-defined HID interface** (read via `hidapi`; no driver on Windows or macOS; udev rule on Linux) with reports for press, release and periodic presence. An optional fallback keyboard interface sends F24, a key almost nothing uses, only if configured.
- The agent applies the same hold machine (`source = hid`). Button presence is reported in the heartbeat and coverage ("button connected").
- Compliance for sale: UKCA/CE (EMC), RoHS, WEEE producer registration. OHTF-branded units are sold to fund development. The trade mark policy controls the branding.

### 2.0 Mobile

- Android first: FCM high-priority messages via the push forwarder, plus an optional foreground-service mode that listens directly on practice Wi-Fi. UnifiedPush support so a practice can avoid Google and OHTF entirely.
- iOS: APNs via the OHTF push forwarder. Apply for the Critical Alerts entitlement early. Raising alarms from widgets, the Action Button and Apple Watch.
- **Push forwarder:** run by OHTF. Content-blind: it receives `{device push token, opaque wake id}` from the practice relay and sends a wake-only push. The app then fetches the envelopes from the practice relay. No alarm content passes through OHTF.
- PWA with Web Push via the relay as an interim, no-app-store option.
- Mobile enrolment by QR code from any enrolled desktop.

### Later

- ADMX/ADML Group Policy templates.
- Welsh language (cy).
- IPv6.
- Citrix and AVD investigation: a virtual channel or endpoint helper so thin-client users are covered.
- Multi-site: practices with branch surgeries (several practice ids, or sites within one practice, with site-scoped alerting).
- An alternative independent implementation of the protocol from the spec and golden vectors, to validate the spec.
