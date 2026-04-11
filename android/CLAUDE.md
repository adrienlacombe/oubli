# Android Notes

Repo-wide workflow lives in [`README.md`](../README.md) and [`AGENTS.md`](../AGENTS.md). This file only adds Android-specific guidance.

## Architecture
- Follow Android recommended architecture: layered design, single source of truth, unidirectional data flow, ViewModel/state holders.
- Prefer the repo's existing Compose/View approach; state down, events up.
- Verify adaptive behavior for phones, tablets, foldables, and multi-window where relevant.
- Minimize permissions and data collection.
- Check content descriptions/semantics, TalkBack flow, and loading/empty/error/offline states.

# Native bridge (UniFFI)
- Native lib: `app/src/main/jniLibs/arm64-v8a/liboubli_bridge.so`
- Generated bindings: `app/src/main/java/uniffi/oubli/oubli.kt`
- Rebuild .so when Rust bridge code changes: `make build-android` (from repo root)
- Regenerate Kotlin bindings from the compiled .so: `make regen-kotlin` (from repo root)
- **Never regenerate bindings from .udl alone** — produces wrong library name, causing dlopen crash
- Full setup: `make setup-android`

# Build
- **Always source `.mainnet.env` before building**: `set -a && source .mainnet.env && set +a` (from repo root). `build.rs` XOR-encodes RPC URLs, paymaster keys, and fee config at compile time. Without this, secrets are empty and the wallet fails with "invalid rpc url" on launch.
- Debug: `cd android && ./gradlew assembleDebug`
- Release: `cd android && OUBLI_KEYSTORE_PASSWORD="..." ./gradlew assembleRelease`
- ABI: arm64-v8a only, minSdk=35, targetSdk=36, compileSdk=36
- ProGuard enabled for release (isMinifyEnabled=true, isShrinkResources=true)
