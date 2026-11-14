# Contributing

Thank you for helping. Muster is safety-relevant software, so contributions are held to a high standard of testing and review.

## Licence of contributions

By contributing you agree that your contribution is licensed under AGPL-3.0-or-later **together with the additional permission in `LICENSE-EXCEPTIONS.md`**. The exception lets the project distribute mobile apps through app stores. Without it, iOS distribution is not possible. Hardware designs (when added under `hardware/`) are licensed CERN-OHL-S-2.0.

## Developer Certificate of Origin

We use the Developer Certificate of Origin (https://developercertificate.org/) rather than a CLA. Sign off every commit:

```
git commit -s -m "feat(core): add alarm expiry"
```

This adds a `Signed-off-by:` line certifying that you have the right to submit the work under the project licence.

## Code of conduct

The project adopts the Contributor Covenant, version 2.1. The text will be added as `CODE_OF_CONDUCT.md` (fetch it from https://www.contributor-covenant.org/; do not paraphrase it).

## Before opening a PR

- [ ] `cargo fmt --all --check`
- [ ] `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] `cargo test --workspace` (includes simulator property tests)
- [ ] `cargo deny check`
- [ ] Frontend: `npm run lint`, `npm run format:check`, `npm run check`, `npm test` in `apps/desktop`
- [ ] New functionality has tests that reference requirement IDs
- [ ] If behaviour that affects safety changed, `safety/hazard-log.md` has been updated
- [ ] Docs updated

## Reporting security issues

Do not open public issues for vulnerabilities. See `SECURITY.md` (to be written in M0: a private reporting address owned by OHTF and a 90-day disclosure policy).
