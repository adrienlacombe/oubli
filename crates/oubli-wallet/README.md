# oubli-wallet

Core wallet logic for the Oubli privacy wallet.

## Overview

Manages Starknet accounts, privacy-preserving transactions via [krusty-kms](https://crates.io/crates/krusty-kms) (Tongo protocol), balance tracking, and cross-chain swaps via the Atomiq SDK.

## Architecture

`WalletCore` is the central state machine:

```
Onboarding → Locked → Ready ↔ Processing
                ↑                  ↓
                └──── Error ◄──────┘
```

All operations go through `WalletCore` methods which manage state transitions, proof generation, and transaction submission via the AVNU paymaster (gasless transactions).

## Key Operations

| Operation | Description |
|-----------|-------------|
| `handle_fund()` | Deposit WBTC into the privacy pool |
| `handle_transfer_op()` | Private transfer to another Oubli user |
| `handle_withdraw_op()` | Withdraw from privacy pool to a Starknet address |
| `handle_ragequit_op()` | Emergency exit (bypass normal flow) |
| `handle_pay_lightning()` | Pay a Lightning invoice via Atomiq swap |

## Modules

- **`core.rs`** — `WalletCore` state machine and operation handlers
- **`operations.rs`** — krusty-kms proof generation (fund, transfer, withdraw, ragequit)
- **`rpc.rs`** — Starknet JSON-RPC client
- **`paymaster.rs`** — AVNU paymaster for gasless transactions
- **`submitter.rs`** — Transaction submission abstraction
- **`swap.rs`** — Swap operation wrappers
- **`denomination.rs`** — SAT/Tongo unit conversion
- **`config.rs`** — Network configuration (mainnet/Sepolia)
- **`signing.rs`** — Starknet message signing

## Networks

Default: **Sepolia** for local development. Mainnet should be selected explicitly. Network configs use compile-time env vars for baked defaults and runtime env vars for overrides.

```bash
# Safe local setup
cp .sepolia.env.example .sepolia.env
make env-status
make test-offline
```

For cross-cutting edits, start with the repo-level [`docs/change-map.md`](../../docs/change-map.md).
