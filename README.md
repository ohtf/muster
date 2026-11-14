# Muster (working name)

An open-source, cross-platform panic alarm for GP practices and other small workplaces.

A member of staff presses and holds an on-screen button (or, later, a USB button). Every other enrolled device in the practice shows an alert and sounds an alarm. The person who raised the alarm sees, discreetly, that the alarm was delivered and who is coming to help.

Stewarded by the [Open Health Technology Foundation](https://openhealthtechnology.org) (OHTF).

**Status:** pre-alpha, under active development. Not yet suitable for use. See [spec/09-roadmap.md](spec/09-roadmap.md) for progress.

## What it is not

- Not a monitored alarm service, and not a route to 999. It alerts colleagues nearby only. Your practice's emergency procedures remain primary.
- Not a patient-facing or clinical system. It never handles patient data.
- Not supported inside Citrix/RDS sessions or on thin clients (detected and reported).

## Why

Commercial on-screen panic alarms are expensive for what they are, Windows-only, and opaque about whether they are actually working. In 2023 EMIS removed its built-in panic button because local network configurations stopped it working reliably. Muster is designed around that failure: it uses several delivery paths at once, and it continuously and visibly proves that it can reach other devices.

## Design principles

1. **LAN-first, server optional.** Devices on the practice network find each other and deliver alarms directly. An optional, self-hostable relay bridges VLANs, Wi-Fi client isolation and remote staff.
2. **Trust comes from cryptography, not the network.** Every device is enrolled into a practice group. Messages are signed and encrypted. Sharing a LAN with another practice or a guest network is safe.
3. **Prove it works.** Every device shows an honest status light. Scheduled drills produce coverage reports. A device must never claim to be connected when it cannot deliver an alarm.
4. **Silent to the room.** Raising an alarm makes no sound the aggressor would recognise. There is only a soft cue during the 1.5 second hold so accidental presses are obvious to the user.
5. **Nothing leaves the practice.** No telemetry and no third-party services. The practice owns its keys, relay and audit log.
6. **One alarm: "come and help".** Responders judge what is needed when they arrive.

## Quick start (developers)

Requires Rust (the pinned toolchain in `rust-toolchain.toml` is installed automatically by `rustup`).

```sh
git clone https://github.com/ohtf/muster.git
cd muster
s/test        # full test suite, including simulator properties
s/lint        # fmt, clippy, cargo-deny
```

See [s/README.md](s/README.md) for all scripts.

## Reading order

| File | Contents |
|---|---|
| [agent-instructions.md](agent-instructions.md) | Instructions for coding agents (also read by `AGENTS.md` / `CLAUDE.md`) |
| [spec/README.md](spec/README.md) | Specification index: overview, requirements, architecture, protocol, security, desktop, relay, testing, deployment, roadmap |
| [spec/adr/](spec/adr/) | Architecture decision records |
| [SAFETY.md](SAFETY.md) | Clinical safety entry point and hazard log |
| [CONTRIBUTING.md](CONTRIBUTING.md) | How to contribute (DCO sign-off) |
| [SECURITY.md](SECURITY.md) | Reporting vulnerabilities |

## Licensing

- Code: AGPL-3.0-or-later with an additional permission for app store distribution (draft, pending legal review). See [LICENSE](LICENSE) and [LICENSE-EXCEPTIONS.md](LICENSE-EXCEPTIONS.md).
- Documentation and specification: CC-BY-SA-4.0.
- Generated alarm sounds: CC0-1.0.
- Copyright: Open Health Technology Foundation contributors. Contributions use the [Developer Certificate of Origin](https://developercertificate.org/).
- Licensing metadata is machine-checkable with `reuse lint`.

## Working name

"Muster" is a placeholder. Check trade marks before any public launch. Keep the name confined to a small number of constants (crate prefix, mDNS service name, file paths, registry keys, UI strings) so it can be changed before the first release.
