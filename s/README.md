# `s/`

The `s/` directory contains some simple convenience scripts to speed up and standardise working with this repository.

## `s/test`

Run the full test suite, including simulator scenarios and properties.

- `s/test` - everything
- `s/test -p panic-button-core` - one crate
- `PROPTEST_CASES=256 s/test --release -p panic-button-sim --test properties` - CI-scale property run
- `PROPTEST_CASES=1000 s/test --release -p panic-button-sim --test properties` - long property run (about an hour)

## `s/install`

Install from this checkout. The command-line programs go into `~/.cargo/bin`. With `--desktop`, the desktop app is also built and installed as a system package: `panic-button-git` on Arch (via the AUR PKGBUILD), the deb on Debian and Ubuntu, the rpm on Fedora. Asks for your sudo password at the end.

- `s/install` - the command-line programs
- `s/install --desktop` - and the desktop app

## `s/version++`

Release: bump the version (`patch`, `minor` or `major`), regenerate `CHANGELOG.md` and push the release commit. CI tags it and publishes to GitHub Releases, Homebrew, the AUR and crates.io.

- `s/version++ minor`

## `s/check-deep`

The slow checks that GitHub Actions no longer runs on every push: simulator properties and scenarios, network-namespace scenarios, the desktop app, and a short fuzz of every target. Run it before pushing changes to `panic-button-core`, `panic-button-net` or the agent.

- `s/check-deep` - about 25 minutes
- `s/check-deep --long` - 1,000 property cases, S13 at 250 devices, 10 minutes per fuzz target (a few hours)

## `s/lint`

Run rustfmt, clippy, cargo-deny and `reuse lint`, as CI does.

- `s/lint`

## `s/netns-test`

Build the release binaries (agent, CLI, relay) and run the network-namespace scenarios (Linux). Runs rootless through `unshare` when not root.

- `s/netns-test` - all scenarios
- `KEEP=1 s/netns-test s7` - one scenario, keeping logs

## `s/gui-dev`

Run the desktop app in development (Vite hot reload plus the Tauri shell).

- `s/gui-dev`
- `PANIC_BUTTON_DATA_DIR=/tmp/a PANIC_BUTTON_CONFIG=/tmp/a.toml s/gui-dev` - a throwaway practice (see `docs/gui/`)

## `s/desktop-check`

Type-check and build the frontend, then clippy and test the Tauri crate. `s/test` and `s/lint` leave the desktop crate out because it needs WebKit and a built frontend.

- `s/desktop-check`

## `s/sounds`

Regenerate the built-in sounds in `sounds/` from the synthesiser, then commit them. A test checks the files match.

- `s/sounds`

## `s/fuzz`

Run a cargo-fuzz target (needs nightly and cargo-fuzz).

- `s/fuzz envelope_decode -- -max_total_time=600`

## `s/install-hooks`

Use the tracked hooks in `.githooks/` (pre-commit runs `s/lint`).

- `s/install-hooks`
