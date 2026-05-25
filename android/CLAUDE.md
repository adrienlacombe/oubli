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

# Local QA / Emulator smoke testing

Verifying the release APK on an emulator before publishing:

1. **Set a device PIN first.** The wallet uses `BIOMETRIC_STRONG | DEVICE_CREDENTIAL`. With neither enrolled, auth fails silently with `CredentialAvailable: false` and the app dismisses to launcher (looks like a crash; isn't — unlike iOS, Android has no simulator biometric bypass).
   ```
   adb -s emulator-5554 shell locksettings set-pin 1234
   ```

2. **Install + launch.** API 36 needs `pm unstop` before `am start`:
   ```
   adb -s emulator-5554 install -r android/app/build/outputs/apk/release/app-release.apk
   adb -s emulator-5554 shell pm unstop com.oubli.wallet
   adb -s emulator-5554 shell am start -n com.oubli.wallet/.MainActivity
   ```

3. **`adb screencap` returns a black PNG.** `MainActivity` sets `FLAG_SECURE` — correct wallet behavior (blocks screenshots of balances and recovery phrases). To inspect UI use `uiautomator dump` instead, which reads the accessibility tree and bypasses `FLAG_SECURE`:
   ```
   adb -s emulator-5554 shell uiautomator dump /sdcard/ui.xml
   adb -s emulator-5554 shell cat /sdcard/ui.xml
   ```
   A healthy home screen shows `text="0"`, `text="sats"`, `Receive/Scan/Send` buttons, and `"No transactions yet"`.

4. **Type the PIN (1234) on the BiometricPrompt** to advance — it falls back to PIN entry because no biometric is enrolled.

5. **Smoke-test signals** (logcat):
   - `Displayed com.oubli.wallet/.MainActivity` within ~200ms
   - `nativeloader ... liboubli_bridge.so ... ok` and `libjnidispatch.so ... ok` — UniFFI/JNA loaded
   - No `FATAL`, `AndroidRuntime`, `UnsatisfiedLinkError`, or `dlopen` errors
   - `BiometricService ... Status: 0` after PIN is set means auth succeeded

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
