# oubli-bridge

UniFFI bridge exposing the Oubli wallet to iOS (Swift) and Android (Kotlin).

## Overview

Wraps `WalletCore` and all internal crates into a single FFI-safe `OubliWallet` object. Uses [UniFFI](https://mozilla.github.io/uniffi-rs/) to generate native bindings from `oubli.udl`.

## Generated Bindings

```bash
# iOS (Swift)
make build-ios-sim && make generate-swift
# Output: ios/Generated/oubli.swift + oubliFFI headers

# Android (Kotlin)
make build-android && make generate-kotlin
# Output: android/app/src/main/java/.../oubli.kt
```

## FFI Surface

### Wallet Operations
- `handle_complete_onboarding()` — Create or restore wallet
- `handle_unlock_biometric()` / `handle_lock()` — Auth
- `handle_fund()` / `handle_send()` / `handle_transfer()` — Transactions
- `handle_withdraw()` / `handle_ragequit()` — Exit operations
- `handle_refresh_balance()` — Sync balance + auto-rollover

### Swaps
- `pay_lightning(bolt11)` — Pay a Lightning invoice (WBTC → BTCLN)
- `swap_btc_to_wbtc()` / `swap_wbtc_to_btc()` — On-chain swaps
- `swap_ln_to_wbtc()` — Lightning on-ramp
- `swap_execute()` / `swap_status()` / `swap_list()` / `swap_limits()`

### Seed Backup
- `get_mnemonic()` — Retrieve seed phrase (requires T3 auth)
- `generate_mnemonic()` / `validate_mnemonic()`

### Platform Storage
Native platforms implement `PlatformStorageCallback` to bridge iOS Keychain / Android Keystore into Rust.

## Build Notes

- Crate type: `["lib", "cdylib", "staticlib"]`
- iOS links `liboubli_bridge.a` — do NOT rename the lib
- Two iOS headers must stay in sync: `ios/Generated/oubliFFI.h` and `ios/Generated/oubliFFI/oubliFFI.h`
- Always regenerate bindings from the compiled library (not from `.udl` alone)
