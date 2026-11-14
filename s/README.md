# `s/`

The `s/` directory contains some simple convenience scripts to speed up and standardise working with this repository.

## `s/test`

Run the full test suite, including simulator scenarios and properties.

- `s/test` - everything
- `s/test -p muster-core` - one crate
- `PROPTEST_CASES=256 s/test --release -p muster-sim --test properties` - CI-scale property run
- `PROPTEST_CASES=10000 s/test --release -p muster-sim --test properties` - nightly-scale property run

## `s/lint`

Run rustfmt, clippy, cargo-deny and `reuse lint`, as CI does.

- `s/lint`

## `s/fuzz`

Run a cargo-fuzz target (needs nightly and cargo-fuzz).

- `s/fuzz envelope_decode -- -max_total_time=600`

## `s/install-hooks`

Use the tracked hooks in `.githooks/` (pre-commit runs `s/lint`).

- `s/install-hooks`
