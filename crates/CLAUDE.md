# Rust Workspace Notes

Repo-wide workflow lives in [`README.md`](../README.md) and [`AGENTS.md`](../AGENTS.md). This file only adds Rust-specific constraints.

## Build

- Prefer repo-root `make` targets over ad hoc cargo commands.
- Safe default env is `.sepolia.env`.
- If you edit `oubli-swap-js/src/`, rebuild `crates/oubli-swap/js/bundle.js`.
- If you edit the bridge API or `.udl`, regenerate mobile bindings from the compiled library.

## Dependencies

- All krusty-kms deps are pinned at `v0.3.0`.
- `v0.3.0` removed `fee_to_sender` from operation params. Do not reintroduce it.

## Conventions

- JS/Rust boundary structs use `#[serde(rename_all = "camelCase")]`.
- `oubli-bridge` must keep `[lib] name = "oubli_bridge"`.

## Testing

- Offline Rust tests: `make test-offline`
- Wallet smoke tests: `make test-smoke`
- Devnet tests: `make test-devnet`
