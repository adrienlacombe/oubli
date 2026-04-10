# Oubli

Oubli is a privacy-preserving mobile wallet built on Starknet. The native iOS and Android apps are thin shells over a Rust workspace that owns auth, encrypted storage, wallet state, swaps, and the UniFFI bridge.

## Start Here

- [`AGENTS.md`](AGENTS.md): operator guide and repo constraints
- [`docs/change-map.md`](docs/change-map.md): what else to touch when you edit a given subsystem
- [`docs/env.md`](docs/env.md): env files, safe defaults, and networked workflows
- [`docs/architecture-deep-dive.md`](docs/architecture-deep-dive.md): long-form architecture and implementation notes

## Repo Map

- `crates/oubli-wallet/`: wallet state machine, operations, RPC, paymaster, swap orchestration
- `crates/oubli-bridge/`: UniFFI FFI layer consumed by Swift and Kotlin
- `crates/oubli-swap/`: QuickJS runtime embedding the Atomiq SDK bundle
- `oubli-swap-js/`: TypeScript source that builds into `crates/oubli-swap/js/bundle.js`
- `crates/oubli-store/`: encrypted blob storage and platform storage callbacks
- `ios/`: SwiftUI app
- `android/`: Jetpack Compose app

## Safe Quickstart

```sh
cp .sepolia.env.example .sepolia.env
make env-status
make test-offline
make test-smoke
make check-rust
make check-swap
```

`make` defaults to `.sepolia.env` when it exists. Mainnet workflows are explicit and opt-in.

## Common Task Commands

```sh
make build-swap-js
make regen-swift
make regen-kotlin
make regen-bindings
make regen-all
make coverage-rust
make coverage-android-unit
make test-sepolia
make OUBLI_ENV_FILE=.mainnet.env OUBLI_ALLOW_MAINNET=1 test-mainnet
```

## Coverage

Start with Rust coverage first, then add Android JVM coverage:

```sh
make coverage-rust
make coverage-android-unit
```

This writes:

- `target/coverage/rust/summary.txt`
- `target/coverage/rust/lcov.info`
- `target/coverage/rust/html/index.html`
- `target/coverage/android-unit/summary.txt`
- `target/coverage/android-unit/report.xml`
- `target/coverage/android-unit/html/index.html`

See [`docs/testing/coverage.md`](docs/testing/coverage.md) for the reporting workflow and release checklist.

## Generated Artifacts

These files are checked in and must be regenerated, not hand-edited:

- `crates/oubli-swap/js/bundle.js`
- `ios/Generated/oubli.swift`
- `ios/Generated/oubliFFI/oubliFFI.h`
- `ios/Generated/oubliFFI/module.modulemap`
- `android/app/src/main/java/uniffi/oubli/oubli.kt`

Use:

```sh
make verify-swap-bundle
make verify-swift-bindings
make verify-kotlin-bindings
```

## Change Impact

Before editing any of these areas, check [`docs/change-map.md`](docs/change-map.md):

- Bridge / UDL changes
- Swap TypeScript and runtime changes
- Wallet core, auth, and storage changes
- Generated bindings and bundle outputs

## Release & Publish

### 1. Bump version

Edit `android/app/build.gradle.kts` — increment `versionCode` and `versionName`.

### 2. Tag a release

```sh
git tag v0.1.64
git push origin v0.1.64
```

The **Release** workflow builds a signed APK and creates a GitHub Release with the APK attached.

### 3. Publish to Zapstore

Go to **Actions > Publish to Zapstore > Run workflow**, enter the tag (e.g. `v0.1.64`), and click **Run**. This downloads the release APK and publishes it to Zapstore.

### Required GitHub secrets

| Secret | Description |
|---|---|
| `OUBLI_KEYSTORE_BASE64` | Base64-encoded `android/oubli-release.jks` |
| `OUBLI_KEYSTORE_PASSWORD` | Keystore password |
| `OUBLI_MAINNET_RPC_URL` | Mainnet RPC URL (for `build.rs` compile-time encoding) |
| `OUBLI_MAINNET_PAYMASTER_API_KEY` | Mainnet paymaster API key |
| `OUBLI_FEE_COLLECTOR_PUBKEY` | Fee collector public key |
| `OUBLI_FEE_PERCENT` | Fee percentage |
| `NOSTR_NSEC` | Nostr private key (nsec) for Zapstore signing |

## Safety

- Do not default local automation to mainnet.
- Do not hand-edit generated bindings or bundle outputs.
- Keep signing in Rust. JS should never touch private keys.
- Go through `BlobManager` and platform storage callbacks for secrets at rest.
