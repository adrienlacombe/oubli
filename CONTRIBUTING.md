# Contributing

Thanks for contributing to Oubli.

This repository contains wallet code that handles seed phrases, signing, transaction construction, and secure mobile storage.
Treat all changes with a security-first mindset.

## Before You Start

- Prefer small, focused pull requests.
- For larger changes, open an issue or draft PR first so the approach can be discussed before implementation.
- If your change affects wallet security, persistence, signing, recovery, or swap execution, call that out explicitly in the PR description.

## Development Setup

Core tooling:

- Rust stable
- Node.js and npm for `oubli-swap-js/`
- Xcode for iOS work
- Android Studio or the Android command-line toolchain for Android work

Android native builds require `ANDROID_NDK_HOME`.
The project expects:

- `~/Library/Android/sdk/ndk/28.2.13676358`

## Common Commands

Run the workspace tests:

```sh
make test-offline
```

Run the wallet smoke tests:

```sh
make test-smoke
```

Run Sepolia integration tests:

```sh
make OUBLI_ENV_FILE=.sepolia.env test-sepolia
```

Run mainnet integration tests:

```sh
make OUBLI_ENV_FILE=.mainnet.env OUBLI_ALLOW_MAINNET=1 test-mainnet
```

Run devnet tests:

```sh
make test-devnet
```

Rebuild the embedded swap bundle after TypeScript changes:

```sh
make build-swap-js
```

Prepare iOS bindings:

```sh
make setup-ios
```

Prepare Android bindings:

```sh
make setup-android
```

## Repository-Specific Rules

### UniFFI bindings

- Always regenerate bindings from the compiled library, not from `.udl` alone.
- Do not change the `[lib] name` in `crates/oubli-bridge/Cargo.toml`.
- Treat `ios/Generated/` and `android/app/src/main/java/uniffi/oubli/oubli.kt` as generated outputs.
- Use `make regen-swift`, `make regen-kotlin`, or `make regen-bindings`.

### Swap runtime

- After editing files in `oubli-swap-js/src/`, run `make build-swap-js`.
- The Rust crate embeds `crates/oubli-swap/js/bundle.js` at compile time.
- Private keys must remain in Rust. Do not move Starknet signing into JavaScript.

### Networks and test safety

- The default local network is Sepolia when `.sepolia.env` is present.
- Be careful with commands that can spend real funds.
- Mainnet validation is explicit and should use `OUBLI_ALLOW_MAINNET=1`.
- Use the provided env files and test configurations when working on integration flows.

### Sensitive data and generated files

Do not commit:

- `.env` files or funded mnemonic files
- keystores, signing keys, or provisioning material
- local build output such as `target/`, `android/build/`, `android/app/build/`, or `ios/build/`
- regenerated native binaries unless the change specifically requires updated checked-in generated code

Do not add logs that print:

- seed phrases
- private keys
- decrypted secret material
- authentication secrets

## Testing Expectations

Before opening a PR, run the smallest relevant set of checks for your change.

Examples:

- Rust-only change: `make test-offline`
- wallet flow or auth/storage change: `make test-offline && make test-smoke`
- swap TypeScript change: `make check-swap && make build-swap-js`
- Android UI or ViewModel change: relevant Gradle tests
- iOS UI or view model change: relevant Xcode tests

If you cannot run a required check locally, say so in the PR.

## Pull Request Checklist

- The change is scoped to a clear problem.
- Generated artifacts were rebuilt when required.
- Tests or checks relevant to the change were run.
- Security-sensitive behavior changes are called out clearly.
- New configuration, secrets, or operational steps are documented.
