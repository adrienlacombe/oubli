# Environment Guide

## Defaults

- `make` auto-loads `.sepolia.env` when it exists.
- An already-exported shell environment wins over the Makefile-selected file.
- Mainnet is never the implicit local default.

## Example Files

```sh
cp .sepolia.env.example .sepolia.env
cp .mainnet.env.example .mainnet.env
```

Use `.sepolia.env` for normal development. Use `.mainnet.env` only for explicit production validation and real-money tests.

## Variable Reference

| Variable | Purpose |
|----------|---------|
| `OUBLI_NETWORK` | Optional network selector. Use `sepolia` or `mainnet`. |
| `OUBLI_RPC_URL` | Runtime JSON-RPC URL used by the wallet. |
| `OUBLI_CHAIN_ID` | `SN_SEPOLIA` or `SN_MAIN`. |
| `OUBLI_TONGO_CONTRACT` | Tongo privacy pool contract address. |
| `OUBLI_TOKEN_CONTRACT` | ERC-20 token contract address. |
| `OUBLI_ACCOUNT_CLASS_HASH` | Counterfactual Starknet account class hash used by the wallet and paymaster flows. Sepolia/mainnet use the ArgentX v0.4 class hash; devnet uses its built-in custom account class. |
| `OUBLI_PAYMASTER_URL` | AVNU paymaster endpoint. |
| `OUBLI_AVNU_PAYMASTER_API_KEY` | Generic paymaster API key used at runtime. |
| `OUBLI_SEPOLIA_RPC_URL` | Compile-time Sepolia RPC URL baked into the binary when present. |
| `OUBLI_SEPOLIA_PAYMASTER_API_KEY` | Compile-time Sepolia paymaster API key. |
| `OUBLI_MAINNET_RPC_URL` | Compile-time mainnet RPC URL baked into the binary when present. |
| `OUBLI_MAINNET_PAYMASTER_API_KEY` | Compile-time mainnet paymaster API key. |
| `OUBLI_TEST_MNEMONIC_A` | Funded mnemonic used by networked tests. |
| `OUBLI_FEE_PERCENT` | Optional fee percentage override. |
| `OUBLI_FEE_COLLECTOR_PUBKEY` | Optional fee collector public key. |

## Common Workflows

Safe local loop:

```sh
cp .sepolia.env.example .sepolia.env
make env-status
make test-offline
make test-smoke
```

Sepolia integration:

```sh
make OUBLI_ENV_FILE=.sepolia.env test-sepolia
```

Mainnet validation:

```sh
make OUBLI_ENV_FILE=.mainnet.env OUBLI_ALLOW_MAINNET=1 test-mainnet
```

If you source env vars manually, they override `OUBLI_ENV_FILE`:

```sh
set -a && source .sepolia.env && set +a
make check-rust
```
