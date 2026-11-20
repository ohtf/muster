# Changelog

All notable changes to Panic Button are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Releases are grouped from conventional-commit messages by [git-cliff](https://git-cliff.org).

## [0.2.1] - 2026-11-20

### Bug fixes

- **desktop**: The home profile allows Quit, as the spec says; home test explains allow_quit ([53d5a1f](https://github.com/ohtf/panic-button/commit/53d5a1f750779697c5d4cbb54ad0bd54dc7a4ec4))

- **scripts**: S/install replaces an existing install ([4238b96](https://github.com/ohtf/panic-button/commit/4238b965f4db2c1ab8ba56c3cac6a1b8120a4f66))

### CI

- **release**: Publish to crates.io by trusted publishing only (R65) ([b33dc0d](https://github.com/ohtf/panic-button/commit/b33dc0dae04543a069a21695b1862b16ba04f620))

- **release**: Publish to crates.io only from Auto-tag ([8737225](https://github.com/ohtf/panic-button/commit/8737225df9c34a2a50b8e7e833e24071bab971f6))

### Documentation

- **install**: The Windows .zip is the command-line programs, not the app ([fd17704](https://github.com/ohtf/panic-button/commit/fd17704ec62c24a7a2b98bc1fa9245127351e64e))

- **gui**: Fuller niri rules for the button, panel and alert, and their limits ([55e0578](https://github.com/ohtf/panic-button/commit/55e0578a3ce8342a6a89399a4eb9dc6c70ac98f4))

- Add Panic Button logo to the README ([05ce98f](https://github.com/ohtf/panic-button/commit/05ce98f8777ddea5f160956bc3b6e9d2945bcb34))

- **roadmap**: R66, a basic send-only mobile app with Tauri ([4139df0](https://github.com/ohtf/panic-button/commit/4139df005c44d741522419d19779e8460e5ea0e5))

- **home-test**: Wording ([a1d1fad](https://github.com/ohtf/panic-button/commit/a1d1fad36e0611660fc71803fa4e8c32d804a372))

- **home-test**: V0.2.0 downloads; try joining by approval, a locked screen and Remote Desktop ([3b7fc2b](https://github.com/ohtf/panic-button/commit/3b7fc2b686ba8e23e480c10b87618498e7b07a36))

- **roadmap**: The former library crates are yanked (R65) ([c992f5d](https://github.com/ohtf/panic-button/commit/c992f5d94f9870e406ad9a65bbcfd4113697e244))

- **roadmap**: V0.2.0 published to crates.io by trusted publishing (R65) ([f9a2024](https://github.com/ohtf/panic-button/commit/f9a20249753d11012ca0ad3735a6df61b2239743))

### Features

- First run finds Panic Button on the network and offers to join; fix the practice hold ([23a5ce2](https://github.com/ohtf/panic-button/commit/23a5ce2a208366c77058651fcaa20300415f694f))

- **scripts**: S/install --user installs the desktop app for one account ([e4647e5](https://github.com/ohtf/panic-button/commit/e4647e534beb3cfbbf5921a590a5b602223b4988))

## [0.2.0] - 2026-11-18

### Bug fixes

- **scripts**: Sign off the release commit ([2791a9c](https://github.com/ohtf/panic-button/commit/2791a9c5b4dbca238eb6a6c4355380365aae9097))

- **net**: Retry a busy UDP port at start; port 47474 is unassigned (R50) ([e4ae02d](https://github.com/ohtf/panic-button/commit/e4ae02d17e3f1dbbed3b0068180f8953da4a1624))

- **reuse**: Licence header on apps/desktop/.prettierignore ([66c654c](https://github.com/ohtf/panic-button/commit/66c654cabd00ea616ae75e9a154856eea06b9e9b))

- **packaging**: Autostart entry in the rpm; clean cask header; home test guide uses release downloads ([87e7c1a](https://github.com/ohtf/panic-button/commit/87e7c1a948988ae8e2a4ad0a0802d024c13e373a))

### Build

- **desktop**: ESLint and Prettier for the frontend (R30) ([8c11fde](https://github.com/ohtf/panic-button/commit/8c11fde4846bdc1e2a6a0073e92b553cc75e20e4))

### CI

- **release**: Publish to crates.io from the crates-io environment ([cdb0231](https://github.com/ohtf/panic-button/commit/cdb0231575485feaf2aa2d000e8ae5a7e4666687))

### Documentation

- **roadmap**: The former crates are not yanked yet ([e1a4c0d](https://github.com/ohtf/panic-button/commit/e1a4c0d8efd136a960b725915cedb570f2aadebc))

- The staff guide prints on one A4 page (R57) ([355f3e8](https://github.com/ohtf/panic-button/commit/355f3e8dce6a44f358561189b3320011135f58d8))

- **spec**: Q9, admin recovery on a computer outside the practice ([ed6f943](https://github.com/ohtf/panic-button/commit/ed6f943decf1ca305ffbdd111c429b405b6eba1f))

- Documentation site with Zensical (R59) ([b62e056](https://github.com/ohtf/panic-button/commit/b62e056cead81f12f4c6f560f35226c9bca91afb))

- V0.1.0 crates are on crates.io ([a19273e](https://github.com/ohtf/panic-button/commit/a19273e09caca7f2bfc16259f9cce5022fb9639f))

- **agents**: Record publicity channels ([4fe5790](https://github.com/ohtf/panic-button/commit/4fe5790b5605edf12b978464a5b535568e2830d0))

- **roadmap**: Panic-button-bin is on the AUR ([32dac3c](https://github.com/ohtf/panic-button/commit/32dac3c1e1524792855b9720f2341bec5539097d))

- **install**: Checksums under releases; crates.io not yet published ([9489e19](https://github.com/ohtf/panic-button/commit/9489e1923ba266de90cb105a9d2708b802326f7d))

### Features

- **desktop**: Session detection, locked and remote (R32) ([c362a6d](https://github.com/ohtf/panic-button/commit/c362a6d2b7a6280ec7ed7b2ff5ac07a636084344))

- Join by approval code (flow C, R30) ([5072fb7](https://github.com/ohtf/panic-button/commit/5072fb7b0f57b9a5d020b996dd4fcc21ebe42fc8))

- **admin**: Create a new recovery kit with a new passphrase (R34) ([5d1fd34](https://github.com/ohtf/panic-button/commit/5d1fd34ab7a6100055ab57a503fe34f848490867))

- **desktop**: Windows audio endpoint and mute override (R32) ([c4076d2](https://github.com/ohtf/panic-button/commit/c4076d21befcded1291d597fddfa6cac757944ee))

- **desktop**: A muted computer shows amber on Linux and macOS (R32) ([46ccaa7](https://github.com/ohtf/panic-button/commit/46ccaa76294613d7cfce85304c3f5c41d0821bae))

- **desktop**: Keyboard shortcut and remembered button position (R30) ([7cbc3cd](https://github.com/ohtf/panic-button/commit/7cbc3cd8aa9aecd028cc48a3eed086cc31b01236))

- **scripts**: S/install for the command-line programs and, with --desktop, the desktop app as a system package ([bd612f8](https://github.com/ohtf/panic-button/commit/bd612f84ee41c338bc2a854d59f12cddd05a2c2e))

### Refactor

- One published crate, panic-button (ADR 0016) **[breaking]** ([0f7c947](https://github.com/ohtf/panic-button/commit/0f7c9471b215976ca566d807fb751a74883f8af1))

## [0.1.0] - 2026-11-15

### Bug fixes

- **release**: Build only the three programs with cargo-dist (precise builds) ([b9d8bfe](https://github.com/ohtf/panic-button/commit/b9d8bfe4d603d2ee1ce67dd2ae856c14c7556c21))

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


