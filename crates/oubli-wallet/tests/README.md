# oubli-wallet integration tests

## Test files

| File | Network | Duration | What it tests |
|------|---------|----------|---------------|
| `integration.rs` | Mock + Sepolia | seconds (mock) / minutes (Sepolia) | Wallet lifecycle, fund, transfer+rollover, lazy deploy, ERC-20 balance, auto-fund, withdraw, activity |
| `sepolia_full_flow.rs` | Sepolia | ~14 min | Full 5-wallet flow: distribute STRK, auto-fund, explicit fund, transfers, rollovers, withdraw, ragequit, activity, STRK recovery |
| `mainnet_full_flow.rs` | Mainnet | ~14 min | Same as Sepolia flow but with **real WBTC** (~10,000 sats budget). Also: withdraw-to-starknet, auto-fund-on-login, recovery |
| `devnet_integration.rs` | Local devnet | seconds | Fund, transfer, rollover, withdraw, ragequit, full lifecycle — all against a local `starknet-devnet` with a freshly deployed Tongo contract |

## Running tests

### Unit tests (no network)

```sh
cargo test --workspace
```

### Mock-only integration tests

```sh
cargo test -p oubli-wallet --test integration test_full_lifecycle_mock
cargo test -p oubli-wallet --test integration test_fund_requires_t2
```

### Sepolia tests

```sh
set -a && . crates/oubli-wallet/tests/sepolia.env && set +a
cargo test -p oubli-wallet --test integration -- --ignored --nocapture
cargo test -p oubli-wallet --test sepolia_full_flow -- --ignored --nocapture
```

### Mainnet tests

**WARNING: Uses real WBTC.**

```sh
set -a && . crates/oubli-wallet/tests/mainnet.env && set +a
cargo test -p oubli-wallet --test mainnet_full_flow -- --ignored --nocapture
```

### Devnet tests

Requires `starknet-devnet` installed and available on PATH.

```sh
cargo test -p oubli-wallet --features devnet --test devnet_integration -- --nocapture --test-threads=1
```

Or via Makefile:

```sh
make test-devnet
```

## Environment setup

Each network has an `.env` file that must be sourced before running tests:

| File | Network | Token |
|------|---------|-------|
| `sepolia.env` | Sepolia testnet | STRK |
| `mainnet.env` | Starknet mainnet | WBTC |
| `sepolia.env.example` | Template | — |

Required env vars:

- `OUBLI_RPC_URL` — Starknet JSON-RPC endpoint
- `OUBLI_CHAIN_ID` — `SN_SEPOLIA` or `SN_MAIN`
- `OUBLI_TONGO_CONTRACT` — Tongo privacy pool contract address
- `OUBLI_TOKEN_CONTRACT` — Underlying ERC-20 (STRK on Sepolia, WBTC on mainnet)
- `OUBLI_ACCOUNT_CLASS_HASH` — OZ account class hash (v0.4.0)
- `OUBLI_PAYMASTER_URL` — AVNU paymaster endpoint
- `OUBLI_AVNU_PAYMASTER_API_KEY` — AVNU API key
- `OUBLI_TEST_MNEMONIC_A` — Faucet mnemonic (pre-funded, distributes tokens to test wallets)

## Test architecture

All Sepolia and mainnet tests follow the same pattern:

1. **Generate fresh mnemonics** at runtime (no hardcoded test accounts)
2. **Faucet (S0)** distributes tokens from `OUBLI_TEST_MNEMONIC_A` via raw ERC-20 transfers
3. Tests exercise wallet operations (fund, transfer, rollover, withdraw, ragequit)
4. **Cleanup** recovers tokens back to the faucet via ragequit + send_token

Mainnet tests print recovery mnemonics at the start so funds can be manually recovered if the test fails mid-run. Use `test_recover_strk_mainnet` with `OUBLI_RECOVER_MNEMONICS` (semicolon-separated) to automate recovery.

## Faucet architecture (SingleOwnerAccount vs WalletCore)

The faucet account (`OUBLI_TEST_MNEMONIC_A`) holds public ERC-20 tokens (STRK on Sepolia, WBTC on mainnet) and distributes them to test wallets. It is intentionally used in two different ways:

### `faucet_strk()` — Raw ERC-20 transfers (safe)

The `faucet_strk()` helper uses `SingleOwnerAccount` from starknet-rs directly. This is a plain Starknet account that signs and submits ERC-20 `transfer()` calls. **No WalletCore, no auto-fund, no Tongo involvement.** The faucet's public token balance is preserved.

This is the correct way to interact with the faucet during tests.

### `WalletCore` — Full Oubli wallet (dangerous for faucet)

`WalletCore` is the full Oubli wallet with auto-fund, auto-rollover, and Tongo privacy pool integration. When `handle_refresh_balance()` runs and detects a public token balance, it **automatically sweeps everything into the Tongo pool** (auto-fund).

**Do NOT create a `WalletCore` from the faucet mnemonic** unless:
- Deploying the faucet account for the first time (`ensure_faucet_deployed`)
- Running the recovery test (`test_recover_strk_mainnet`) to ragequit funds back

If auto-fund accidentally sweeps the faucet's tokens into Tongo, use the recovery test to ragequit them back to the public balance:

```sh
set -a && . crates/oubli-wallet/tests/mainnet.env && set +a
OUBLI_RECOVER_MNEMONICS="$OUBLI_TEST_MNEMONIC_A" \
  cargo test -p oubli-wallet --test mainnet_full_flow test_recover_strk_mainnet -- --ignored --nocapture
```

Note: The recovery test itself creates a WalletCore (triggering auto-fund), then ragequits back. This is a safe round-trip but costs gas.

## Paymaster retry logic

The AVNU paymaster intermittently rejects transactions with `"execution call was rejected"` when on-chain state hasn't fully settled from a previous operation. Tests use a `retry_paymaster_op()` helper that retries with increasing delays (10s, 20s, 30s) up to 3 attempts.

## Key test pitfalls

- **Never call `handle_refresh_balance()` before `handle_fund()`** — auto-fund sweeps public tokens, causing `u256_sub Overflow` when the explicit fund tries to use them. `handle_fund()` uses `sync_balance_for_proof()` internally (no auto-fund).
- **Ragequit to faucet, not self** — ragequit to the wallet's own address causes an infinite cycle: ragequit sends tokens to self → auto-fund immediately re-sweeps them into Tongo.
- **Use auto-rollover, not manual `handle_rollover_op()`** — `handle_refresh_balance()` triggers auto-rollover when pending > 0. Calling manual rollover after a refresh races with auto-rollover and causes "Proof Of Ownership failed".
- **Source both `.env` and network env** — `.env` provides compile-time RPC URLs; `sepolia.env`/`mainnet.env` provides runtime test config. Use `bash -c 'set -a && source .env && source <network>.env && set +a && cargo test ...'`.

## Devnet architecture

Devnet tests spawn a local `starknet-devnet` process, declare and deploy a fresh Tongo contract via UDC, then run operations using `DirectSubmitter` (no paymaster). The devnet process is killed on drop. Requires the `devnet` feature flag.
