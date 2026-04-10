# AGENTS.md — Oubli

This is the operator source of truth for LLMs and other coding agents.

Start here:
- [`README.md`](README.md) for the repo map and common commands
- [`docs/change-map.md`](docs/change-map.md) before editing cross-cutting code
- [`docs/env.md`](docs/env.md) before running networked workflows
- [`docs/architecture-deep-dive.md`](docs/architecture-deep-dive.md) for long-form design context

## Project Overview

Oubli is a privacy-preserving mobile wallet built on the Tongo protocol. The Rust workspace owns wallet logic, auth, storage, backup, swaps, and the UniFFI bridge. iOS and Android are thin native shells over that Rust core.

## Architecture

```
Mobile UI (Swift / Kotlin)
    ↓ UniFFI
oubli-bridge        — FFI surface, async→sync adapter, Tokio runtime
    ↓
oubli-wallet        — Wallet state machine, tx flows, RPC, paymaster, swaps
    ↓                    ↓
oubli-swap           oubli-auth    — Auth tiers, session state, KEK derivation
  ↓ (QuickJS)        oubli-store   — Encrypted blobs + platform storage callbacks
oubli-swap-js/       oubli-backup  — Seed display, verification, cloud backup
```

## Safe Defaults

- `make` auto-loads `.sepolia.env` when present.
- Mainnet is opt-in. Use `OUBLI_ENV_FILE=.mainnet.env` and set `OUBLI_ALLOW_MAINNET=1` for mainnet-only test flows.
- Prefer `make test-offline`, `make test-smoke`, `make check-rust`, and `make check-swap` before any networked command.

## Common Commands

```sh
make env-status
make test-offline
make test-smoke
make check-rust
make check-swap
make build-swap-js
make regen-swift
make regen-kotlin
make regen-bindings
make regen-all
```

## High-Risk Paths

- `crates/oubli-bridge/src/oubli.udl`: changing the FFI surface means regenerating Swift and Kotlin bindings from the compiled library.
- `oubli-swap-js/src/`: changing TypeScript means rebuilding `crates/oubli-swap/js/bundle.js`.
- `crates/oubli-wallet/src/core.rs`: most wallet state, auth gating, and swap lifecycle changes land here.
- `crates/oubli-store/`: storage changes must preserve the `BlobManager` boundary. Do not bypass platform storage through ad hoc file access.
- `ios/Generated/` and `android/app/src/main/java/uniffi/oubli/oubli.kt`: generated artifacts. Do not hand-edit them.

## Change Impact Rules

- Bridge / UDL changes: inspect [`docs/change-map.md`](docs/change-map.md), then run `make regen-bindings`.
- Swap JS changes: run `make check-swap` and `make build-swap-js`.
- Wallet / auth / store changes: run `make test-offline` and `make test-smoke` at minimum.
- CI-facing generated outputs must stay current. Use `make verify-swap-bundle`, `make verify-swift-bindings`, or `make verify-kotlin-bindings` as appropriate.

## Repo Constraints

1. Regenerate UniFFI bindings from the compiled library, never from `.udl` alone.
2. The bridge crate must keep `[lib] name = "oubli_bridge"`.
3. Swap signing stays in Rust. JS must never hold private keys.
4. `BlobManager` owns secret-at-rest encryption. Platform code should implement callbacks, not alternate storage layers.
5. Do not assume mainnet in local automation, tests, or docs.

## Testing

- Offline unit tests: `make test-offline`
- Wallet smoke tests: `make test-smoke`
- Sepolia integration tests: `make test-sepolia`
- Mainnet integration tests: `make OUBLI_ENV_FILE=.mainnet.env OUBLI_ALLOW_MAINNET=1 test-mainnet`
- Devnet tests: `make test-devnet`

## Platform Notes

- iOS-specific guidance lives in [`ios/CLAUDE.md`](ios/CLAUDE.md).
- Android-specific guidance lives in [`android/CLAUDE.md`](android/CLAUDE.md).
- Rust workspace notes live in [`crates/CLAUDE.md`](crates/CLAUDE.md).
