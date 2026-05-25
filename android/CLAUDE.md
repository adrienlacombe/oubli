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

# Release
**Order matters: GitHub release with the APK comes first, then Zapstore.** GitHub release is the canonical, stable APK URL and changelog; it's easy to edit/delete and gives a safety net before the externally-visible Zapstore publish.

1. **Decide semver** from `git log <prev-tag>..HEAD` — patch for dep hygiene, minor for features, major for breaking changes.
2. **Bump version** — `versionCode` and `versionName` in `android/app/build.gradle.kts`.
3. **Source mainnet env** — `set -a && source .mainnet.env && set +a` (from repo root).
4. **Build native** — `make build-android` (cross-compiles `.so`, copies to `jniLibs/arm64-v8a/`).
5. **Regenerate Kotlin** — `make generate-kotlin` (uses `--library` flag on the compiled `.so`).
6. **Build signed APK** — `cd android && OUBLI_KEYSTORE_PASSWORD="oubli-release-2024" ./gradlew assembleRelease`. Output: `android/app/build/outputs/apk/release/app-release.apk`.
7. **Commit + tag** — commit as `"Release vX.Y.Z"`, `git tag -a vX.Y.Z -m "..."`, push branch + tag.
8. **GitHub release** (first):
   ```
   gh release create vX.Y.Z "android/app/build/outputs/apk/release/app-release.apk#oubli-vX.Y.Z.apk" \
     --title "vX.Y.Z - ..." --notes "..."
   ```
9. **Zapstore publish** (after):
   ```
   cd android && SIGN_WITH="<bunker_uri>" ~/go/bin/zsp publish \
     -quiet --skip-preview --skip-certificate-linking zapstore.yaml
   ```
   Requires Amber tap-to-sign approval on the signing device. `-quiet` is the auto-confirm flag (was `-y`; renamed in zsp v0.4.10 — `-y` now errors).

Never invoke `zsp publish` before the GitHub release exists.
