# iOS Notes

Repo-wide workflow lives in [`README.md`](../README.md) and [`AGENTS.md`](../AGENTS.md). This file only adds iOS-specific guidance.

## Architecture
- Follow Apple HIG, accessibility, privacy, and App Review expectations.
- Prefer the repo's existing SwiftUI/UIKit pattern; keep model/data separate from views.
- Minimize Info.plist permissions and ask only when needed.
- For UI changes, verify Dynamic Type, VoiceOver labels, contrast, safe areas, and loading/empty/error states.
- Flag any change that may affect tracking, purchases, sign-in, account deletion, or review risk.

# Native bridge (UniFFI)
- **Always source `.mainnet.env` before building**: `set -a && source .mainnet.env && set +a` (from repo root). `build.rs` XOR-encodes RPC URLs, paymaster keys, and fee config at compile time. Without this, secrets are empty and the wallet fails with "invalid rpc url" on launch.
- Rebuild for simulator: `make regen-swift` (from repo root)
- Rebuild for device: `make build-ios && make generate-swift` (from repo root)
- Full setup (includes xcodegen): `make setup-ios`
- Generated header lives at `Generated/oubliFFI/oubliFFI.h` (moved there by `make generate-swift`)
- Links `-loubli_bridge` — do not change `[lib] name` in Cargo.toml

# Simulator
- Biometric auto-succeeds via `#if targetEnvironment(simulator)` in KeychainStorage
