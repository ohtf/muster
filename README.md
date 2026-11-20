# Panic Button

An open-source, cross-platform panic alarm for GP practices and other small workplaces.

A member of staff presses and holds an on-screen button (or, later, a USB button). Every other enrolled device in the practice shows an alert and sounds an alarm. The person who raised the alarm sees, discreetly, that the alarm was delivered and who is coming to help.

Stewarded by the [Open Health Technology Foundation](https://openhealthtechnology.org) (OHTF).

Status: pre-alpha. Not yet suitable for use. See [spec/09-roadmap.md](spec/09-roadmap.md) for progress.

Documentation for practice staff, Practice Admins and IT: https://ohtf.github.io/panic-button/

<img src="docs/assets/panic-button-logo.svg" width="144" height="144" alt="Panic Button logo: a big red button with a white hand printed on its face">

## What it is not

- Not a monitored alarm service, and not a route to 999. It alerts colleagues nearby only. Your practice's emergency procedures remain primary.
- Not a patient-facing or clinical system. It never handles patient data.
- Not supported inside Citrix or Remote Desktop sessions or on thin clients. Detecting these and saying so is planned.

## Why

Commercial on-screen panic alarms are expensive for what they are, Windows-only, and opaque about whether they are working. In 2023 EMIS removed its built-in panic button because local network configurations stopped it working reliably. Panic Button sends each alarm several ways at once, and every computer shows whether it can currently reach the others.

## Design principles

1. Devices on the practice network find each other and deliver alarms directly. A relay server is optional; it bridges separate networks, Wi-Fi client isolation and remote staff, and a practice can run its own.
2. Every device is enrolled into its practice, and messages are signed and encrypted. Another practice or a guest Wi-Fi on the same network cannot read or fake alarms.
3. Every device shows an honest status light, and scheduled test alarms produce coverage reports. A device never claims to be connected when it cannot deliver an alarm.
4. Once the alarm is raised, nothing sounds on the computer that raised it, so the person causing trouble is not alerted. A soft tone plays during the 1.5 second hold, so an accidental press is obvious to the person pressing.
5. Nothing leaves the practice: no telemetry and no third-party services. The practice holds its own keys, relay and audit log.
6. There is one kind of alarm, "come and help". Responders judge what is needed when they arrive.

## Installing

See [Installing](https://ohtf.github.io/panic-button/install/) on the docs site ([docs/install.md](docs/install.md) in the repository). Panic Button is not yet ready for real practices.

## Quick start (developers)

Requires Rust (the pinned toolchain in `rust-toolchain.toml` is installed automatically by `rustup`).

```sh
git clone https://github.com/ohtf/panic-button.git
cd panic-button
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

## Name

The project was called Muster until 15 November 2026, when it was renamed because other projects already used that name on crates.io and the AUR (ADR 0015). Packages are named `panic-button`.
