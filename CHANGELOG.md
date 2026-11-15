# Changelog

All notable changes to Panic Button are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Releases are grouped from conventional-commit messages by [git-cliff](https://git-cliff.org).

## [0.1.0] - 2026-11-15

### Bug fixes

- **netns**: Always run the scenarios in a private network and mount namespace ([9ab75f4](https://github.com/ohtf/panic-button/commit/9ab75f4623ec2a28bc66a41b3217f5d23b3e6653))

- **agent**: Keep the enrolled label unless --label is given; add the headless try-it guide ([ede3be3](https://github.com/ohtf/panic-button/commit/ede3be3a2cb62c0f6b2b55aafe8a978843be8072))

- **core**: Duplicate copies update path tracking; TCP trust is not sticky ([cd4302e](https://github.com/ohtf/panic-button/commit/cd4302e4693a294cdc9a3837c03fa636c8c4b78d))

### CI

- Quick Linux checks on push; slow checks local (s/check-deep) or by hand ([8e4644b](https://github.com/ohtf/panic-button/commit/8e4644b95a19d7dad71dfd26264152bae132b729))

- **netns**: Build dev-profile binaries with a resource monitor; release builds wedged the runner ([eea1887](https://github.com/ohtf/panic-button/commit/eea1887201959d26e14aacd72a1c8ec6725f2ad6))

- Longer desktop timeout for cold caches; a step timeout so the netns build hang leaves logs ([62a7cc2](https://github.com/ohtf/panic-button/commit/62a7cc238c56e11b6ba7c55ffa16d2da56ea61fd))

- **nightly**: Scale property runs to 1,000 cases and fuzzing to 10 min until the nightly is reliable ([6f97787](https://github.com/ohtf/panic-button/commit/6f977878bbf40cab89b70a9185842785005708f7))

- Fix golden vectors on Windows and install cargo-fuzz from source ([098c0e8](https://github.com/ohtf/panic-button/commit/098c0e84e5c083d001446ff9bdaa559b45ef70ca))

### Chores

- Scaffold workspace, CI, licensing and scripts (R1, R2, R4) ([8c820f6](https://github.com/ohtf/panic-button/commit/8c820f658d5468d1a6207af69de897e1d62e3f62))

### Documentation

- Practice Admin guide (R58, draft); AppImage tested on Arch ([7375696](https://github.com/ohtf/panic-button/commit/7375696e6da5733d81102e50fb930a2d0ab903d9))

- Staff quick guide with screenshots (R57, draft) ([70d786c](https://github.com/ohtf/panic-button/commit/70d786cfd8b40ae229eee8c38463c21f18d33b60))

- Home test guide, writing checklist, plainer README (R55, R56) ([6387279](https://github.com/ohtf/panic-button/commit/6387279fee55dbf73dfe6e5852798878d6219985))

- **roadmap**: Home-test installers (R52-R55) and user documentation stages (R56-R60) ([f0e6589](https://github.com/ohtf/panic-button/commit/f0e6589fbf7103ac145567da13fad4829942a029))

- Resolve Q5-Q8; not a medical device, big red button palette ([197b796](https://github.com/ohtf/panic-button/commit/197b796708f5809a165a49a5ef225b64b555dad4))

- **agents**: Record standing permission to push to main ([7ed0963](https://github.com/ohtf/panic-button/commit/7ed09633ec67091b5e0a70a079432254253ad5d0))

- **spec**: Import Muster v1 specification ([a01cc39](https://github.com/ohtf/panic-button/commit/a01cc39abc899d922cc610dc401402dafb3a44c1))

### Features

- **release**: Release pipeline for every channel (R62-R65) ([3211889](https://github.com/ohtf/panic-button/commit/3211889af10b33176403d8fb38109131334a4063))

- **packaging**: Windows MSI with firewall rules and autostart, Linux deb and AppImage, built in CI (R52-R54) ([6c5d381](https://github.com/ohtf/panic-button/commit/6c5d3816854256ff03965427e617902f02043202))

- **admin**: Recover admin access with the recovery kit (spec/04-security.md) ([bbc581d](https://github.com/ohtf/panic-button/commit/bbc581d9718c554edc58df8724cf723a65e44f6f))

- **drills**: Weekly test alarms from a coordinator; skipped drills recorded (FR-DRL-001, FR-DRL-006) ([4917dca](https://github.com/ohtf/panic-button/commit/4917dca651c1d7a1f5af4886bb01dca1cab250bf))

- **admin**: Export the audit log as CSV from the app and CLI (FR-LOG-001) ([ef2f6e2](https://github.com/ohtf/panic-button/commit/ef2f6e264bc07dcaad9502aba72fed60497f65ce))

- **admin**: Edit practice settings from the app and CLI (FR-ADM-003) ([1a44408](https://github.com/ohtf/panic-button/commit/1a44408975de0238768d0fdfd42e95c484a63ba6))

- **core**: Admins set a device's label and room (DeviceSettings 0x09, ADR 0014) ([7a1c77d](https://github.com/ohtf/panic-button/commit/7a1c77d77d2efe3a2efb066be80a332c40c02ec5))

- **agent**: Control API over a named pipe on Windows (Q7, R32) ([dd48e54](https://github.com/ohtf/panic-button/commit/dd48e54bb610d26629094eb663040ae767d4c294))

- **admin**: Promote a member to Practice Admin from the app and CLI (FR-ADM-002, FR-ADM-004) ([58c580d](https://github.com/ohtf/panic-button/commit/58c580d77c9e79d6faaa54a7e775f723fa55eade))

- **desktop**: Admin tab with devices, enrolment codes and test alarm reports (R34) ([bb9fc3d](https://github.com/ohtf/panic-button/commit/bb9fc3d0092366c5c2f2d7cec8e753d8835602c8))

- **cli**: Token list/revoke, device list, drill reports kept and exported; core_handle_datagram fuzz target (R8, R27, R39) ([b759d3a](https://github.com/ohtf/panic-button/commit/b759d3ac7934beb746cd4d00a00339dbccde9acd))

- **desktop**: Alarm sounds as replaceable WAV files (R31) ([b3878a5](https://github.com/ohtf/panic-button/commit/b3878a5bbc58f0cebd5af7cf4ef10a4a601ec17d))

- **desktop**: Tauri 2 + Svelte 5 app embedding the agent (R30, R31) ([77d326e](https://github.com/ohtf/panic-button/commit/77d326e58447d32a4ab6a4a10eda83c619e14b95))

- **relay**: Docker image, compose with Caddy, systemd unit and operator guide (R38, FR-RLY-007) ([1a1ed4f](https://github.com/ohtf/panic-button/commit/1a1ed4fc17483cb3e85b33783c7405131eb9b282))

- **relay**: Long-poll fallback, proxies, persistence and relay-only S14 (R36, R37) ([0e3b58c](https://github.com/ohtf/panic-button/commit/0e3b58c50ca9076df6f63e579122b5bc765aed0a))

- **relay**: Muster-relay server, WebSocket relay client, relay netns tests (R36-R38) ([08a0873](https://github.com/ohtf/panic-button/commit/08a08737f06a13ad665fdddda4075db854934a6c))

- **agent**: Network drivers, headless agent, control API and CLI (R25-R29) ([9c72e4d](https://github.com/ohtf/panic-button/commit/9c72e4d5e820e4e3c05903057b705ee8ea8c8125))

- **store**: SQLite persistence for core state, identity and audit (R24) ([9fc23d3](https://github.com/ohtf/panic-button/commit/9fc23d3b23b393df1925c979c64e5c1c73bcc331))

- **sim**: Properties P1-P12 over random worlds, and the core fixes they found (R10-R22) ([2f524a3](https://github.com/ohtf/panic-button/commit/2f524a330a05322958c9067fdc13baf937c83d42))

- **sim**: Deterministic network simulator and scenarios S1-S17 (R20, R21) ([dd9b2d6](https://github.com/ohtf/panic-button/commit/dd9b2d6f742cc339d9df6591246de97af07f68f8))

- **core**: Sans-IO core engine (R10-R19 in progress) ([4079952](https://github.com/ohtf/panic-button/commit/40799520d078c9b9b92f632ef9183b7d131a6bc4))

- **crypto**: Keys, certificates, sealing, key shares, recovery kit and tokens (R7) ([8d73150](https://github.com/ohtf/panic-button/commit/8d73150280f32e92a80dbb5d9a15becd1450c682))

- **proto**: Wire protocol types and strict deterministic CBOR (R6) ([0c45dc1](https://github.com/ohtf/panic-button/commit/0c45dc1b482de827d4fc70b6f6415ae9673d3314))

### Refactor

- Rename Muster to Panic Button (ADR 0015) **[breaking]** ([e45ecf0](https://github.com/ohtf/panic-button/commit/e45ecf0c607c5e79571cb5c73590b2d46dc98924))

- **agent**: Shared setup module for practice creation and enrolment tokens (CLI and desktop) ([0db67ae](https://github.com/ohtf/panic-button/commit/0db67aea39f5752c17aff7c782fbd9069e5a674b))

- **agent**: Embeddable start() with an AgentHandle; hold and UI-event views for the desktop app ([de07eca](https://github.com/ohtf/panic-button/commit/de07ecad51819647c4d1067ffc5b942980da9b17))


