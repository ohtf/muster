# 01 Requirements

Priority uses MoSCoW: **M** must (v1 blocker), **S** should (v1 if possible), **C** could, **W** won't in v1.

## Functional requirements

### ACT: Activation

| ID | Pri | Requirement |
|---|---|---|
| FR-ACT-001 | M | A floating on-screen button is shown on every desktop device while the agent is running and enrolled. |
| FR-ACT-002 | M | Pressing and holding the button for `hold_ms` (default 1500, configurable 1000-3000 by Practice Admin) raises an alarm. Releasing earlier does not. |
| FR-ACT-003 | M | During the hold, a progress ring fills on the button over `hold_ms`. |
| FR-ACT-004 | M | During the hold, a soft rising audio cue plays on the sender's device when `cue_mode = soft` (default). Cue modes: `silent`, `soft`, `alarm_ramp`. The cue stops immediately on release. |
| FR-ACT-005 | M | Moving the pointer while holding does not cancel the hold. The hold ends only on button release, loss of pointer capture, or the window being destroyed. |
| FR-ACT-006 | M | A configurable global keyboard shortcut (default `Ctrl+Shift+F12`) raises an alarm when held for `hold_ms`. Key auto-repeat is ignored. |
| FR-ACT-007 | S | Where the platform cannot report key release (some Wayland setups), the shortcut instead requires two presses within 1 s, and the UI says so. |
| FR-ACT-008 | M | The trigger timing decision is made in the Rust core, not the UI, so button, shortcut and future HID sources behave identically. |
| FR-ACT-009 | M | A hold released between 300 ms and `hold_ms` is logged locally as a near miss (time, source, duration). Counts are visible to the Practice Admin. |
| FR-ACT-010 | M | The button can be repositioned only via the tray menu "Move button" mode (30 s) or Shift+drag. Its position is remembered per monitor layout. |
| FR-ACT-011 | M | If the button's monitor is disconnected, the button moves to the primary monitor. |
| FR-ACT-012 | S | The button can be hidden only if policy `floating_button.allow_hide = true` (default false). The shortcut still works. |
| FR-ACT-013 | M | The button is reachable by keyboard and has an accessible name ("Press and hold to call for help"). |

### ALM: Alarm lifecycle

| ID | Pri | Requirement |
|---|---|---|
| FR-ALM-001 | M | There is one alarm kind, `help`. The protocol carries a `kind` field so others can be added later without breaking compatibility. |
| FR-ALM-002 | M | Once triggered, the alarm is sent immediately on every available transport (see `03-protocol.md`). There is no countdown. |
| FR-ALM-003 | M | The sender's device makes no alarm sound after triggering. It shows a small, neutral confirmation panel next to the button. |
| FR-ALM-004 | M | The sender's panel shows, updated live: "Help requested", then "Delivered to N devices", then "Coming: <names>". |
| FR-ALM-005 | M | The sender can cancel their own open alarm as "sent in error" (with a confirmation step). All devices then show it as cancelled. |
| FR-ALM-006 | M | Any device's user can mark an open alarm Resolved (with a confirmation step). |
| FR-ALM-007 | M | Alarm state is derived from an order-independent set of signed events. All devices that have the same events show the same state. |
| FR-ALM-008 | M | An open alarm older than `ALARM_MAX_LIFE` (60 min) no longer triggers an alert on devices that have not yet alerted for it. It appears in history as "Expired - not resolved", and the Practice Admin is notified. |
| FR-ALM-009 | M | A device that receives an alarm it already knows does not alert again. |
| FR-ALM-010 | M | Alarm ids already processed are persisted across restarts, so replays are not re-alerted. |
| FR-ALM-011 | M | The `source` of activation (button, hotkey, HID, mobile, CLI) is recorded in the alarm. |

### RCV: Receiving

| ID | Pri | Requirement |
|---|---|---|
| FR-RCV-001 | M | A receiving device shows a topmost alert window with "HELP NEEDED", the room, the user's display name, the device label and the time raised (including "N minutes ago" if more than 1 minute has passed). |
| FR-RCV-002 | M | The alert window offers "I'm coming" and "Seen - can't come". Neither is a default button: Enter and Space do nothing until the user tabs to or clicks a button, so typing into another application cannot acknowledge by accident. |
| FR-RCV-003 | M | Closing the window (for example Alt+F4) counts as "Seen". |
| FR-RCV-004 | M | The receiving device plays the alarm sound, starting at 50% of alarm volume and rising to 100% over 8 s, looping until the local user acknowledges or the alarm closes. |
| FR-RCV-005 | M | The alert window lists who is coming, live. |
| FR-RCV-006 | M | When an alarm closes, the window shows "Resolved by <name> at HH:MM" or "Cancelled - sent in error", stops the sound and closes itself after 15 s. |
| FR-RCV-007 | M | On receipt, the device automatically sends a Receipt (machine-level acknowledgement) including its audio status. |
| FR-RCV-008 | M | The alert window does not take keyboard focus from the active application where the OS allows this, but it is always visible above other windows. The taskbar or dock entry flashes. |
| FR-RCV-009 | S | When the policy `override_mute = true` (default true in the practice profile, Windows only in v1), the device unmutes and raises master volume to at least 60% for the duration of the alarm, then restores the previous settings. |
| FR-RCV-010 | M | Several simultaneous alarms are shown stacked in one window, newest first. |
| FR-RCV-011 | M | The alarm sound must be distinct from common UK fire alarm sounders (a rhythmic two-tone pattern with gaps, not continuous). Practices can choose from at least 3 patterns. |
| FR-RCV-012 | M | When the screen is locked, the sound still plays. The window appears on unlock if the alarm is still open. |

### STS: Status and coverage

| ID | Pri | Requirement |
|---|---|---|
| FR-STS-001 | M | Every device shows a status light: a small dot on the floating button plus the tray icon. It is green, amber or red as defined in `03-protocol.md` (Status computation). |
| FR-STS-002 | M | Hovering over or opening the tray shows status text, for example "Connected to 14 devices (12 direct, 2 via relay)". |
| FR-STS-003 | M | **Honesty:** green is never shown unless, in the last `PEER_STALE` seconds, the device has had a working path to at least one other enrolled device and its audio output is present and not muted. |
| FR-STS-004 | M | Amber reasons are listed in plain English (for example "Sound is muted on this computer", "Relay unreachable", "Only reachable via relay"). |
| FR-STS-005 | M | The admin coverage view lists every device seen in the last 7 days: label, room, current user, status, last seen, path (direct, relay, indirect), audio status, version and near-miss count. |
| FR-STS-006 | S | Amber is shown with "Fewer devices than usual" when currently reachable peers fall below 50% of the 7-day median during configured opening hours. |

### DRL: Drills

| ID | Pri | Requirement |
|---|---|---|
| FR-DRL-001 | M | A Practice Admin can run a drill now, or schedule a weekly drill (day and time). |
| FR-DRL-002 | M | Drill alarms are flagged `drill = true`. Receivers auto-send a Receipt with diagnostics and do not show the full alert. |
| FR-DRL-003 | M | Drill display mode: `silent` (default: a small toast "Test alarm received - no action needed" for 10 s) or `audible` (a 2 s chime plus the toast). |
| FR-DRL-004 | M | The drill report lists expected devices (seen in the last 7 days) and, for each, whether a Receipt arrived, the round-trip time measured at the originator, the path, and the audio status. |
| FR-DRL-005 | M | The drill auto-closes 60 s after start. |
| FR-DRL-006 | M | Scheduled drills run from the configured drill coordinator device. If it is offline at the scheduled time, the drill is recorded as skipped and shown to admins. |
| FR-DRL-007 | M | Drill reports are kept locally on the coordinator and on admin devices, and can be exported as CSV. |

### ENR: Enrolment

| ID | Pri | Requirement |
|---|---|---|
| FR-ENR-001 | M | First run with no config offers "Set up a new practice" and "Join an existing practice". |
| FR-ENR-002 | M | Setting up a new practice generates the practice keys, makes this device the first Practice Admin, and requires the user to save a recovery kit (passphrase-protected file plus printable page) before continuing. |
| FR-ENR-003 | M | Token enrolment: if an enrolment token is present (MSI property, config file or pasted), the device enrols with no human approval. |
| FR-ENR-004 | M | Interactive enrolment on the LAN: the joining device shows a 6-digit verification code. Admin devices show the join request with the same code, and the admin approves or rejects it. |
| FR-ENR-005 | M | After enrolment the device asks for its room unless one is configured by policy or config. |
| FR-ENR-006 | M | Tokens have an expiry (default 30 days, max 365) and optional maximum uses (advisory, see `04-security.md`). Admins can revoke tokens. |
| FR-ENR-007 | M | Devices enrolled by a token are labelled with the token id in the admin view for review. |

### ADM: Administration

| ID | Pri | Requirement |
|---|---|---|
| FR-ADM-001 | M | Admin functions are available in the desktop app on admin devices only, and in the `panic-button` CLI. |
| FR-ADM-002 | M | Admins can promote a device to admin, rename devices, set rooms, revoke devices (with or without cascade) and create or revoke tokens. |
| FR-ADM-003 | M | Admins can edit practice settings (`cue_mode`, `hold_ms`, alarm sound pattern, drill schedule and mode, drill coordinator, relay URLs, opening hours). Changes are signed and distributed. |
| FR-ADM-004 | M | The app warns when the practice has fewer than 2 admins. |
| FR-ADM-005 | M | Recovery: with the recovery kit and passphrase, a new device can be made admin even if all admin devices are lost. |
| FR-ADM-006 | M | Alarm history and the audit log can be exported as CSV for incident reporting. |

### NET: Networking

| ID | Pri | Requirement |
|---|---|---|
| FR-NET-001 | M | Discovery and delivery use UDP multicast, UDP broadcast, UDP unicast, TCP unicast, mDNS, configured seed peers and the relay, as specified in `03-protocol.md`. |
| FR-NET-002 | M | Devices re-flood alarms and alarm events once, so partial connectivity is bridged without configuration. |
| FR-NET-003 | M | Network changes (interface up or down, address change, wake from sleep) trigger an immediate heartbeat and sync. |
| FR-NET-004 | M | Messages from other practices are ignored cheaply (practice tag mismatch) and never shown. |
| FR-NET-005 | M | IPv4 is required. IPv6 is W for v1. |

### RLY: Relay

| ID | Pri | Requirement |
|---|---|---|
| FR-RLY-001 | M | A single-binary relay forwards envelopes between authenticated devices of the same practice and cannot decrypt them. |
| FR-RLY-002 | M | Transports: WebSocket over TLS, plus HTTP long-poll as a fallback for proxies that break WebSockets. |
| FR-RLY-003 | M | The relay buffers the last 24 h (or 10,000 envelopes) per practice and serves them on reconnect. |
| FR-RLY-004 | M | The relay is multi-tenant: an operator can host many practices, each registered with its practice id and root public key. |
| FR-RLY-005 | M | The relay provides a presence list of connected devices to authenticated clients of the same practice. |
| FR-RLY-006 | M | The relay honours admin-signed revocations by dropping and refusing revoked devices. |
| FR-RLY-007 | M | Docker image, systemd unit and example reverse proxy configuration are provided. |
| FR-RLY-008 | M | `GET /source` satisfies AGPL section 13. |

### CFG: Configuration

| ID | Pri | Requirement |
|---|---|---|
| FR-CFG-001 | M | Configuration sources and precedence are as defined in `08-deployment.md`. |
| FR-CFG-002 | M | The `profile` setting (`practice` or `home`) selects defaults. Only `practice` is tested for v1. |

### LOG: Audit and diagnostics

| ID | Pri | Requirement |
|---|---|---|
| FR-LOG-001 | M | Each device keeps a local audit log of alarm events, near misses, enrolment, revocation, configuration changes and drills. |
| FR-LOG-002 | M | Retention: alarm events 400 days, near misses and diagnostics 30 days (configurable by admin). |
| FR-LOG-003 | M | Diagnostic logs rotate daily, keep 7 days, and never contain secrets. |

### PLT: Platform

| ID | Pri | Requirement |
|---|---|---|
| FR-PLT-001 | M | Windows 10 22H2+ and Windows 11 (x64 must; arm64 should), macOS 13+ (universal), Ubuntu 22.04+/Debian 12+ (x64, arm64). |
| FR-PLT-002 | M | The agent starts automatically at login and is restarted if it crashes (watchdog, see `05-desktop-agent.md`). |
| FR-PLT-003 | M | In a remote desktop session (RDS/Citrix) the agent detects this, shows an explanatory message and does not participate, unless policy `allow_remote_session = true`. |
| FR-PLT-004 | M | On Windows, only the agent in the active console session participates (fast user switching). |
| FR-PLT-005 | M | A headless agent (`panic-button-agent --headless`) runs on Linux for relay boxes, integration nodes (later) and tests. |

## Non-functional requirements

| ID | Pri | Requirement |
|---|---|---|
| NFR-LAT-001 | M | Alarm shown on all directly reachable LAN peers within 2 s of trigger, p99, at up to 250 devices. |
| NFR-LAT-002 | M | Alarm shown via relay within 5 s p99 (relay on the same continent, healthy network). |
| NFR-LAT-003 | M | Sender sees "Delivered to N" within 3 s for directly reachable peers. |
| NFR-REL-001 | M | No single component failure (relay down, multicast blocked, one node offline) prevents delivery between devices that still have another path. |
| NFR-REL-002 | M | Agent crash recovery within 5 minutes (watchdog). Target is under 10 s where the OS supports it (macOS, Linux). |
| NFR-RES-001 | M | Idle footprint per device: RAM under 120 MB including the webview, CPU under 0.5% average, network under 100 bytes/s average. |
| NFR-RES-002 | M | Message volume per alarm at 250 fully connected devices: no more than 10 x N packets in total. |
| NFR-SEC-001 | M | All non-enrolment traffic is authenticated and encrypted per `04-security.md`. |
| NFR-SEC-002 | M | Decoders are fuzzed, with no crashes after 1 hour per target in CI nightly. |
| NFR-PRV-001 | M | Only device labels, room names and user display names are processed. No patient data. No data leaves the practice except via a relay the practice chose. |
| NFR-ACC-001 | M | Alert and admin windows meet WCAG 2.2 AA (contrast, keyboard access, screen reader labels). The alert is understandable without colour or sound. |
| NFR-I18N-001 | S | All strings are externalised. en-GB ships in v1. Welsh (cy) is planned. |
| NFR-COMP-001 | M | Devices on protocol version N interoperate with N-1 (unknown fields ignored). |
| NFR-OFF-001 | M | The LAN mesh needs no internet access at all. |
| NFR-START-001 | M | Agent participates within 5 s of login on a typical practice PC. |
