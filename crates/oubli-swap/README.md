# oubli-swap

BTC ↔ WBTC cross-chain swaps via the embedded [Atomiq SDK](https://github.com/atomiqlabs).

## Overview

Runs the Atomiq TypeScript SDK inside a QuickJS JavaScript runtime (via [rquickjs](https://crates.io/crates/rquickjs)). Starknet signing is delegated back to Rust, keeping private keys secure. HTTP requests use `reqwest` from Rust.

## Architecture

```
┌─────────────────────────────────┐
│  SwapEngine (Rust API)          │
│  ├─ create_wbtc_to_btc_ln()    │
│  ├─ execute_swap()              │
│  └─ ...                         │
├─────────────────────────────────┤
│  JsRuntime (QuickJS)            │
│  ├─ Atomiq SDK bundle (~1.9MB)  │
│  ├─ Polyfills (fetch, crypto,   │
│  │   AbortSignal, IndexedDB...) │
│  └─ OubliStarknetAccount        │
├─────────────────────────────────┤
│  Rust Host Functions            │
│  ├─ __oubli_fetch (reqwest)     │
│  ├─ __oubli_starknet_sign       │
│  ├─ __oubli_storage_{get,set}   │
│  └─ __oubli_log                 │
└─────────────────────────────────┘
```

## Swap Types

| Direction | Method |
|-----------|--------|
| BTC → WBTC | `create_btc_to_wbtc()` |
| WBTC → BTC | `create_wbtc_to_btc()` |
| Lightning → WBTC | `create_ln_to_wbtc()` |
| WBTC → Lightning | `create_wbtc_to_btc_ln()` |

## JS Bundle

The TypeScript source lives in `oubli-swap-js/` and builds into `crates/oubli-swap/js/bundle.js`:

```bash
make build-swap-js  # or: cd oubli-swap-js && npm run build
```

## Runtime Boundary

- Rust remains the source of truth for signing. JS requests signatures through `__oubli_starknet_sign`.
- Rust also owns durable storage. JS should treat in-memory maps as caches and recover swap state through the storage-backed runtime.
- The host-function contract lives in `src/runtime.rs`. If you add or rename a host function, inspect the matching call sites in `oubli-swap-js/src/`.
- After any TypeScript change, rebuild `js/bundle.js` and verify the checked-in artifact changed intentionally.

## Key Implementation Details

- **Binary fetch**: Response bodies are base64-encoded to preserve binary data (LP wire protocol uses length-prefixed frames)
- **Block tag patch**: `pre_confirmed` → `pending` for starknet.js v6 compatibility with older RPC endpoints
- **Swap lifecycle** (ToBTCLN): `create()` → `commit()` (escrow) → `waitForPayment()` (LP pays invoice)
- **Amount format**: SDK returns BTC decimals (e.g. `"0.00003037"`), converted to sats in the wallet layer

## Usage

```rust
use oubli_swap::{SwapEngine, RuntimeConfig};

let engine = SwapEngine::new(config, signer, storage).await?;
let quote = engine.create_wbtc_to_btc_ln(bolt11).await?;
engine.execute_swap(&quote.swap_id).await?;
```
