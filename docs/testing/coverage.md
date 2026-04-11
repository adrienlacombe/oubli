# Coverage Guide

Coverage is a release signal, not the release goal. Use it to answer two questions:

1. Which critical behaviors are actually exercised by tests?
2. Which high-risk paths still lack automated protection?

For Oubli, start with Rust coverage because the wallet logic, auth, storage, and bridge behavior live there. Then add Android JVM coverage so ViewModel and repository behavior are part of the release picture.

## Rust Baseline

Generate the local Rust coverage baseline:

```sh
make coverage-rust
```

This target:

- runs workspace library tests under `cargo-llvm-cov`
- writes a console-style summary to `target/coverage/rust/summary.txt`
- exports `lcov` data to `target/coverage/rust/lcov.info`
- generates an HTML report at `target/coverage/rust/html/index.html`

Install requirements if your machine does not already have them:

```sh
cargo install cargo-llvm-cov --locked
rustup component add llvm-tools-preview
```

## CI

CI publishes the Rust coverage artifact from the `rust-coverage` job. Download the artifact to inspect:

- `summary.txt` for the quick snapshot
- `lcov.info` for external tooling
- `html/index.html` for per-file drill-down

## Android JVM Baseline

Generate Android JVM coverage from the existing debug unit tests:

```sh
make coverage-android-unit
```

This target:

- runs `jacocoDebugUnitTestReport` for the Android app module
- writes a console-style summary to `target/coverage/android-unit/summary.txt`
- exports JaCoCo XML to `target/coverage/android-unit/report.xml`
- generates an HTML report at `target/coverage/android-unit/html/index.html`

CI publishes the Android artifact from the `android-unit-tests` job. Download the artifact to inspect:

- `summary.txt` for the quick snapshot
- `report.xml` for XML-based tooling
- `html/index.html` for per-file drill-down

## What To Report

When you summarize coverage for a release candidate, report:

- the command(s) you ran
- the current Rust and Android coverage snapshots
- which critical flows are covered
- the known gaps that still need tests

Coverage percentage comes after the flow map. A concise release note should answer:

- Auth and lock flows covered?
- Onboarding and restore covered?
- Storage and migration behavior covered?
- Send, receive, fund, rollover, and withdraw behavior covered?
- Failure paths and invalid-state handling covered?

## Suggested Release Template

Use this structure when reporting coverage:

| Area | Evidence | Coverage Source | Known Gaps |
|------|----------|-----------------|-----------|
| Auth / biometrics | Unit tests passing | Rust summary + test names | Mobile fallback / device-specific edge cases |
| Onboarding / restore | Unit or UI tests passing | Rust summary + iOS / Android tests | Real-network restore latency |
| Transfers / activity | Unit + integration tests | Rust summary + smoke/integration runs | Cross-platform UI edge states |
| Android ViewModel / state | JVM unit tests passing | Android summary + JaCoCo HTML | Compose rendering and device-specific behavior |
| Storage / backup | Unit tests passing | Rust summary | Migration and recovery scenarios |

## Next Layers

After the Rust baseline is stable, add:

1. iOS build/test coverage via `xcodebuild` + `xccov`.
2. A release-facing combined coverage summary that includes behavioral gaps, not just percentages.
3. Integration-flow coverage notes that tie smoke/network tests back to release-critical user journeys.
