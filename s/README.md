# `s/`

The `s/` directory contains some simple convenience scripts to speed up and standardise working with this repository.

## `s/test`

Run the full test suite, including simulator scenarios and properties.

- `s/test` - everything
- `s/test -p muster-core` - one crate
- `PROPTEST_CASES=256 s/test --release -p muster-sim --test properties` - CI-scale property run
- `PROPTEST_CASES=1000 s/test --release -p muster-sim --test properties` - long property run (about an hour)

## `s/check-deep`

The slow checks that GitHub Actions no longer runs on every push: simulator properties and scenarios, network-namespace scenarios, the desktop app, and a short fuzz of every target. Run it before pushing changes to `muster-core`, `muster-net` or the agent.

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
- `MUSTER_DATA_DIR=/tmp/a MUSTER_CONFIG=/tmp/a.toml s/gui-dev` - a throwaway practice (see `docs/gui/`)

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
