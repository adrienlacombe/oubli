# Change Map

Use this before editing any cross-cutting area.

| If you edit... | Also inspect... | Run... |
|----------------|-----------------|--------|
| `crates/oubli-bridge/src/oubli.udl` or `crates/oubli-bridge/src/lib.rs` | `ios/Generated/`, `android/app/src/main/java/uniffi/oubli/oubli.kt`, mobile call sites | `make regen-bindings` |
| `oubli-swap-js/src/**` | `crates/oubli-swap/js/bundle.js`, `crates/oubli-swap/src/runtime.rs`, `crates/oubli-wallet/src/swap.rs` | `make check-swap && make build-swap-js` |
| `crates/oubli-wallet/src/core.rs` | `crates/oubli-wallet/src/operations.rs`, `crates/oubli-wallet/src/state.rs`, bridge methods that expose the changed behavior | `make test-offline && make test-smoke` |
| `crates/oubli-auth/**` or `crates/oubli-store/**` | `crates/oubli-wallet/src/core.rs`, platform storage implementations in `ios/` and `android/` | `make test-offline` |
| `ios/Generated/**` or `android/app/src/main/java/uniffi/oubli/oubli.kt` | The Rust bridge and `.udl` source of truth | Regenerate instead of hand-editing |
| `Makefile` or `.github/workflows/ci.yml` | `README.md`, `AGENTS.md`, `docs/env.md` | `make env-status`, then run the affected checks |

## Generated Outputs

These files are derived artifacts and should change only via their source workflows:

- `crates/oubli-swap/js/bundle.js` from `oubli-swap-js/`
- `ios/Generated/oubli.swift`
- `ios/Generated/oubliFFI/oubliFFI.h`
- `ios/Generated/oubliFFI/module.modulemap`
- `android/app/src/main/java/uniffi/oubli/oubli.kt`

## Good Agent Pattern

1. Read the source file and this map.
2. Edit the true source, not the generated output.
3. Run the smallest relevant verification target.
4. Regenerate checked-in artifacts before finishing.
