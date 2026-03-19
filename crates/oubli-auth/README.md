# oubli-auth

Authentication and session management for the Oubli wallet.

## Overview

Manages user authentication tiers, biometric/PIN challenges, session lifecycle, and Key Encryption Key (KEK) derivation using Argon2.

## Auth Tiers

| Tier | Access Level | Unlocked By |
|------|-------------|-------------|
| `T0None` | No access | — |
| `T1ViewOnly` | View balance | PIN or biometric |
| `T2Transact` | Send/receive | Biometric |
| `T3Critical` | Seed phrase, wipe | Biometric + confirmation |

## Modules

- **`kek.rs`** — KEK derivation via Argon2 password hashing
- **`session.rs`** — Session configuration and timeout management
- **`tier.rs`** — Auth tier state transitions

## Usage

```rust
use oubli_auth::{AuthState, AuthTier, AuthAction};

let mut state = AuthState::default(); // T0None
let result = state.transition(AuthAction::UnlockBiometric)?;
assert_eq!(state.tier, AuthTier::T2Transact);
```
