# AGENTS.md — Oubli

## Project Overview

Oubli is a privacy-preserving mobile wallet built on the Tongo protocol (ElGamal-encrypted ERC20 on Starknet). Rust backend with UniFFI bindings to native iOS (SwiftUI) and Android (Jetpack Compose) apps.

## Architecture

```
Mobile UI (Swift / Kotlin)
    ↓ UniFFI
oubli-bridge        — FFI surface, async→sync adapter, Tokio runtime
    ↓
oubli-wallet        — State machine, transactions, RPC, paymaster
    ↓                    ↓
oubli-swap           oubli-auth    — Argon2 key derivation, session tiers
  ↓ (QuickJS)        oubli-store   — AES-GCM encrypted blob storage (platform callbacks)
oubli-swap-js/       oubli-backup  — Mnemonic display/verify, cloud backup via KMS
  (Atomiq SDK)           ↓
                     krusty-kms-*  — Cryptographic signing (external crates)
```

## Crate Guide

| Crate | Path | Purpose |
|-------|------|---------|
| `oubli-auth` | `crates/oubli-auth/` | Key encryption key (KEK) derivation with Argon2, authentication tiers, session management |
| `oubli-store` | `crates/oubli-store/` | Platform-agnostic encrypted storage; iOS/Android implement `PlatformStorage` trait via callbacks |
| `oubli-backup` | `crates/oubli-backup/` | Seed phrase display, verification flow, cloud backup encryption |
| `oubli-wallet` | `crates/oubli-wallet/` | Core wallet: state machine (`WalletState`), operations (fund/send/transfer/withdraw/rollover/ragequit), RPC, paymaster, tx queue |
| `oubli-swap` | `crates/oubli-swap/` | BTC ↔ WBTC atomic swaps via embedded QuickJS runtime running the Atomiq SDK. Delegates signing to wallet, HTTP to `reqwest` |
| `oubli-bridge` | `crates/oubli-bridge/` | UniFFI FFI layer — the only crate mobile apps link against. Defines `oubli.udl` interface |
| *(JS)* | `oubli-swap-js/` | TypeScript source for the Atomiq SDK bundle. Builds to `crates/oubli-swap/js/bundle.js` (~1.4 MB) |

## Key Files

- **Wallet state machine**: `crates/oubli-wallet/src/core.rs` — `WalletCore` with states: Onboarding, Locked, Ready, Processing, Error, SeedBackup, Wiped
- **Operations**: `crates/oubli-wallet/src/operations.rs` — fund, send, transfer, withdraw, rollover, ragequit
- **Network config**: `crates/oubli-wallet/src/config.rs` — defaults to mainnet; per-network values in `src/networks/`
- **FFI interface**: `crates/oubli-bridge/src/oubli.udl` — UniFFI definition consumed by Swift and Kotlin
- **Bridge impl**: `crates/oubli-bridge/src/lib.rs` — `OubliWallet` FFI object, error flattening, platform storage adapter
- **Swap engine**: `crates/oubli-swap/src/lib.rs` — `SwapEngine` high-level API (create/execute/query swaps)
- **Swap runtime**: `crates/oubli-swap/src/runtime.rs` — QuickJS setup, host functions (fetch, signing, storage, crypto), polyfills
- **Swap integration**: `crates/oubli-wallet/src/swap.rs` — `StarknetSignerCallback` impl, `create_swap_engine()`
- **Swap JS source**: `oubli-swap-js/src/index.ts` — Atomiq SDK init, swap creation/execution, LP discovery
- **Swap JS signer**: `oubli-swap-js/src/signer.ts` — `OubliStarknetAccount` delegates signing to Rust host function
- **iOS entry**: `ios/Oubli/OubliApp.swift`, ViewModel: `ios/Oubli/ViewModels/WalletViewModel.swift`
- **Android entry**: `android/app/src/main/java/com/oubli/wallet/MainActivity.kt`

## Coding Conventions

- **Language**: Rust 2021 edition for all backend crates; Swift for iOS; Kotlin for Android
- **Error handling**: `thiserror` enums per crate, flattened to `OubliError` unit variants at the FFI boundary with thread-local message string
- **Secrets**: `zeroize` derive on all secret types; `SecretFelt` and `Drop`-based cleanup. Internal default PIN `"147258"` for key derivation (user never sees it, biometric only)
- **Async**: Tokio runtime embedded in the bridge; wallet operations are async internally, sync at FFI surface
- **Storage**: Never access platform storage directly — go through `BlobManager` (AES-GCM encrypted). Platform provides `PlatformStorageCallback` (Keychain on iOS, Keystore on Android)

## Building

```sh
make test                  # Unit tests
make test-integration      # Integration tests (needs network)
make test-devnet           # Devnet tests (1 thread, needs local devnet)

make build-ios-sim         # iOS simulator binary
make generate-swift        # Swift + C header from compiled .a
make setup-ios             # Build + generate + xcodegen

make build-android         # Android arm64 .so (needs ANDROID_NDK_HOME)
make generate-kotlin       # Kotlin bindings from compiled .so
make setup-android         # Build + generate

make build-swap-js         # Bundle oubli-swap-js/ → crates/oubli-swap/js/bundle.js
```

## Critical Rules

1. **Always regenerate bindings from the compiled library** (`.a` or `.so`), never from `.udl` alone — the `.udl`-only path produces `libuniffi_oubli` instead of `liboubli_bridge`, causing runtime dlopen crash
2. **Two iOS headers must stay in sync**: `ios/Generated/oubliFFI.h` and `ios/Generated/oubliFFI/oubliFFI.h`
3. **Do not change `[lib] name`** in `oubli-bridge/Cargo.toml` — iOS links against `-loubli_bridge`
4. **`ANDROID_NDK_HOME`** must be set explicitly (shell default is wrong): `~/Library/Android/sdk/ndk/28.2.13676358`
5. **Network default is mainnet** — test flows that spend real STRK. Use env files (`sepolia.env`, `mainnet.env`) to switch networks in tests
6. **`sync_balance()` before proof ops** — `handle_refresh_balance()` triggers auto-fund which interferes; call `sync_balance()` directly
7. **Public key hex padding** — `owner_public_key_hex` may need zero-padding to 128 chars for transfers
8. **Rebuild JS bundle after TS changes** — run `make build-swap-js` after editing `oubli-swap-js/src/`; the Rust crate embeds `js/bundle.js` at compile time
9. **Swap signing stays in Rust** — JS never touches private keys; all Starknet signing goes through the `__oubli_starknet_sign` host function callback

## Testing

- **Unit**: `cargo test --workspace`
- **Sepolia integration**: `crates/oubli-wallet/tests/sepolia_full_flow.rs` — needs `OUBLI_TEST_MNEMONIC_A` env var
- **Mainnet integration**: `crates/oubli-wallet/tests/mainnet_full_flow.rs` — ~14 min, uses ~6 STRK
- **Recovery test**: `test_recover_strk_mainnet` with `OUBLI_RECOVER_MNEMONICS` env var (semicolon-separated)
- **Devnet**: `make test-devnet` — needs `devnet` feature flag and local Starknet devnet running

## Publishing (Android → Zapstore)

1. Bump `versionCode`/`versionName` in `android/app/build.gradle.kts`
2. `make build-android && make generate-kotlin`
3. `cd android && ./gradlew assembleRelease`
4. `cd android && SIGN_WITH="<bunker_uri>" ~/go/bin/zsp publish -y --skip-preview zapstore.yaml`
