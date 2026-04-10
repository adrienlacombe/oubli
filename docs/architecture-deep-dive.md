# Oubli - Privacy-Preserving Mobile Wallet

## 1. Overview

Oubli is a privacy-preserving mobile wallet built on the [Tongo protocol](https://docs.tongo.cash/), which wraps ERC20 tokens with ElGamal encryption on Starknet. The wallet provides confidential transfers, encrypted balances, and regulatory auditability -- all from a mobile device.

This document is the **implementation plan** covering architecture, key management, security, testing, and phased delivery. Each subsystem defines explicit **quality gates**, **security gates**, and **smoke tests**.

### 1.1 Architecture Decision: `kms` Rust Crates + Oubli Hardening Layer + UniFFI

The cryptographic and protocol layer is provided by the `kms` monorepo's **Rust crates** (`crates/core/*`), consumed as normal Cargo dependencies by Oubli's Rust workspace. Oubli adds the mobile hardening layer (auth, storage, backup, session) in Rust on top, then exposes a single FFI surface to native UI shells via UniFFI.

**Why Rust crates (not the Zig C ABI):**

1. **No FFI indirection for the hardening layer.** Oubli's auth/storage/backup logic is written in Rust and calls `kms` crates as normal `use` imports -- full `Result<T, E>` ergonomics, no C error code marshaling, no opaque pointers.
2. **`SecretFelt` and `Zeroize` work natively.** Through the C ABI, zeroization is the caller's responsibility. With Rust crates, `Drop`-based zeroization is automatic and compiler-enforced.
3. **Type safety across the boundary.** `ProjectivePoint`, `Felt`, `TongoAccount` are real Rust types with compile-time guarantees, not `void*`.
4. **Single FFI hop.** One Rust binary (`liboubli`) exposes one UniFFI surface to Swift/Kotlin. No double-FFI (Swift -> C ABI -> Zig, or Swift -> JNI -> Rust).
5. **`kms` parity vectors still apply.** `kms`'s `fixtures/vectors/parity/` test vectors validate Oubli's output since it calls the same Rust code.

> The `kms` Zig C ABI (`zig/include/kms.h`) and pre-built wrappers (`kms-swift`, `kms-jvm`) remain available as a fallback or for non-Rust consumers, but Oubli does not use them.

### 1.2 Layer Diagram

```
┌──────────────────────┐  ┌──────────────────────┐
│   SwiftUI (iOS)      │  │  Compose (Android)    │
│   Renders WalletState│  │  Renders WalletState  │
│   Emits UserAction   │  │  Emits UserAction     │
└─────────┬────────────┘  └─────────┬────────────┘
          │                          │
          │  UniFFI (generated Swift/Kotlin bindings)
          │                          │
┌─────────┴──────────────────────────┴─────────────┐
│              oubli-core (Rust workspace)           │
│                                                    │
│  ┌──────────────────────────────────────────────┐  │
│  │ oubli-wallet   (orchestration + state)       │  │
│  │ - WalletState enum (Locked|ViewOnly|Ready)   │  │
│  │ - handle_action(UserAction) -> WalletState   │  │
│  └──────────────────────────────────────────────┘  │
│                         │                          │
│         ┌───────────────┼───────────────┐          │
│         │               │               │          │
│  ┌──────┴─────┐  ┌──────┴─────┐  ┌──────┴─────┐   │
│  │ oubli-auth │  │oubli-store │  │oubli-backup│   │
│  │ - T0-T3    │  │ - KEK wrap │  │ - seed     │   │
│  │ - PIN      │  │ - mlock    │  │ - cloud    │   │
│  │ - Argon2id │  │ - platform │  │ - shamir   │   │
│  │ - session  │  │   callback │  │            │   │
│  └────────────┘  └────────────┘  └────────────┘   │
│                         │                          │
│         ┌───────────────┼───────────────┐          │
│         │               │               │          │
│  ┌──────┴─────┐  ┌──────┴─────┐  ┌──────┴───────┐ │
│  │ kms        │  │ she-core   │  │ tongo-sdk    │ │
│  │ (Cargo dep)│  │ (Cargo dep)│  │ (Cargo dep)  │ │
│  │ - BIP-39   │  │ - ElGamal  │  │ - fund       │ │
│  │ - BIP-44   │  │ - PoE/PoE2 │  │ - transfer   │ │
│  │ - derive   │  │ - range    │  │ - rollover   │ │
│  │ - grind    │  │ - audit    │  │ - withdraw   │ │
│  │ - dual-key │  │ - bit      │  │ - ragequit   │ │
│  └────────────┘  └────────────┘  └──────────────┘ │
└────────────────────────────────────────────────────┘
```

### 1.3 What `kms` Provides (Cargo dependencies, do NOT reimplement)

| Capability | `kms` Crate | Notes |
|---|---|---|
| BIP-39 mnemonic (12-24 words) | `ghoul-kms` (`crates/core/kms`) | `generate_mnemonic()`, `validate_mnemonic()`, `mnemonic_to_seed()` |
| HD key derivation | `ghoul-kms` | `derive_private_key()`, `derive_keypair()`, `derive_view_keypair()` |
| Stark curve key grinding | `ghoul-kms` | secp256k1 -> Stark curve rejection sampling |
| ElGamal encrypt/decrypt | `she-core` (`crates/core/she-core`) | `ElGamal::encrypt()`, `ElGamal::decrypt()` |
| ZK proofs (PoE, PoE2, Range, Bit, Audit) | `she-core` | Fiat-Shamir non-interactive, `Poe::prove()`, `Range::prove()`, etc. |
| Tongo operations (all 5) | `tongo-sdk` (`crates/core/tongo-sdk`) | `TongoAccount::fund()`, `transfer()`, `rollover()`, `withdraw()`, `ragequit()` |
| Starknet RPC + address derivation | `starknet-client` (`crates/core/starknet-client`) | Counterfactual account address derivation (Argent-style on Sepolia/mainnet; devnet custom class) |
| Dual-key model | `ghoul-kms` | Owner (coin type 5454) + View (coin type 5353) |
| `SecretFelt` with `Zeroize` on drop | `ghoul-common` (`crates/core/common`) | Redacted `Debug`, volatile zeroing, explicit `.expose_secret()` access, no `Deref`/`Serialize` |
| Proof intermediate zeroization | `she-core` | All blinding factors (`r`, `r_b`, `r_r`, `k1`, `k2`) wrapped in `SecretFelt`; scalar intermediate bytes zeroized |
| Deterministic RNG gated | `she-core` | `set_deterministic_rng()` behind `#[cfg(feature = "test-utils")]` -- cannot compile in production |
| Nostr messaging | `nostr-messaging` (`crates/core/nostr-messaging`) | NIP-44: ECDH + HKDF + ChaCha20 |
| Cross-language test vectors | `fixtures/vectors/parity/` | Rust-Zig equivalence verified |

### 1.4 What Oubli Builds (Rust crates in `oubli-core` workspace)

| Crate | Purpose | Key Types |
|---|---|---|
| `oubli-auth` | Auth tier state machine (T0-T3), PIN validation, Argon2id KEK derivation, session timeout | `AuthTier`, `SessionState`, `PinPolicy` |
| `oubli-store` | KEK wrapping, encrypted blob management, `mlock` on secret buffers, `PlatformStorage` trait (callbacks to Keychain/Keystore) | `EncryptedSeed`, `PlatformStorage` trait |
| `oubli-backup` | Seed phrase display/verify flow, AES-256-GCM cloud backup, Shamir (future) | `CloudBackup`, `BackupVerification` |
| `oubli-wallet` | Top-level orchestration: `WalletState` enum, `handle_action()`, wires auth + store + `kms` together | `WalletState`, `UserAction` |
| `oubli-bridge` | UniFFI `.udl` interface, FFI-safe types, `PlatformStorage` callback registration | Generated Swift + Kotlin bindings |

### 1.5 FFI Strategy

**Single FFI boundary**: `oubli-bridge` generates Swift and Kotlin bindings via UniFFI. All `kms` types are consumed inside Rust and never cross FFI.

```
Native UI  ←─── UniFFI ───→  oubli-core (Rust)  ─── Cargo dep ───→  kms crates (Rust)
           (public keys,                          (SecretFelt, Felt,
            balance u64,                           ProjectivePoint,
            WalletState enum,                      TongoAccount —
            error strings)                         all stay in Rust)
```

**FFI boundary rules:**

1. **No secret types cross FFI.** `SecretFelt`, seeds, and scalars stay in Rust. Swift/Kotlin receive: public keys (hex strings), balance amounts (`u64`), `WalletState` enum variants, error strings.
2. **Platform callbacks cross FFI inward.** `PlatformStorage` trait methods (`secure_store`, `secure_load`, `request_biometric`) are implemented in Swift/Kotlin and registered with `oubli-bridge` at startup. Rust calls them; they return results.
3. **UI is a pure function of `WalletState`.** Native code calls `handle_action(UserAction)` and renders the returned `WalletState`. No wallet logic in Swift/Kotlin.
4. **Async via UniFFI.** ZK proof generation and RPC calls run on Rust-side `tokio`. Native receives callbacks/futures via UniFFI async support.

#### Quality Gate: Integration with `kms`
- [ ] `kms` crates pinned in `Cargo.toml` (git dependency with exact commit hash or tag)
- [ ] `kms` parity test vectors (`fixtures/vectors/parity/`) pass in `oubli-core` CI
- [ ] Oubli produces identical Tongo address as `kms` Rust tests for same mnemonic
- [ ] `cargo build --target aarch64-apple-ios` and `--target aarch64-linux-android` compile clean
- [ ] UniFFI-generated Swift and Kotlin bindings compile without manual edits

#### Security Gate: Integration with `kms`
- [x] `SecretFelt` zeroization via volatile write + SeqCst fence on drop (commit 631e440)
- [x] `SecretFelt` requires explicit `.expose_secret()` -- no `Deref`, no accidental coercion (commit 434ad4d)
- [x] All proof blinding factors wrapped in `SecretFelt` with automatic drop zeroization (commit 434ad4d)
- [x] Scalar intermediate byte arrays zeroized in `scalar_add`, `scalar_mul`, `reduce_scalar` (commit 434ad4d)
- [x] Deterministic RNG gated behind `#[cfg(feature = "test-utils")]` -- cannot compile in production (commit 434ad4d)
- [ ] No `kms` function callable without prior auth tier check in `oubli-auth`
- [ ] `kms` CSPRNG delegates to platform hardware (`getrandom` -> `SecRandomCopyBytes` / `SecureRandom`)
- [ ] `cargo audit` passes on full `oubli-core` + `kms` dependency tree
- [ ] `#[deny(unsafe_code)]` on all Oubli crates except `oubli-bridge` UniFFI scaffolding
- [ ] Secret types from `kms` (`SecretFelt`) never implement `Serialize` or cross UniFFI boundary
- [ ] Timing attack caveat documented: `kms` scalar mul is NOT constant-time (inherited from `starknet-types-core`)

#### Smoke Test: Integration with `kms`
1. `cargo test -p oubli-wallet` -> all unit tests pass (mock `PlatformStorage`)
2. Build `liboubli` for `aarch64-apple-ios` -> link into Xcode project -> compiles clean
3. Build `liboubli` for `aarch64-linux-android` -> link into Gradle project -> compiles clean
4. Call `generate_wallet()` from Swift test -> valid `WalletState` with Tongo address
5. Call `generate_wallet()` from Kotlin test -> identical address for same seed
6. Pass invalid UTF-8 as Tongo address in `transfer()` -> `Result::Err`, no panic

---

## 2. Key Inventory

The wallet manages the following distinct key types:

| Key | Type | Curve / Algo | Purpose | Sensitivity |
|-----|------|-------------|---------|-------------|
| **Tongo Private Key** (`sk`) | Scalar | Stark curve | Decrypt balances, generate ZK proofs, sign Tongo operations | CRITICAL - loss = total fund loss |
| **Tongo Public Key** (`pk = g^sk`) | EC Point | Stark curve | Account identifier (Tongo address), encryption target | PUBLIC |
| **Starknet Signing Key** | Scalar | Stark curve (ECDSA) | Sign Starknet transactions, pay gas | CRITICAL - loss = inability to transact |
| **Hint Key** | Symmetric | XChaCha12 (derived from `sk`) | Fast balance recovery without brute-force DLOG | HIGH - leaks balance |
| **Seed Phrase** | Entropy | BIP-39 (24 words) | Root entropy for deterministic key derivation | CRITICAL - master secret |
| **Biometric Auth Token** | Platform | Secure Enclave / StrongBox | Gate access to encrypted key material | HIGH - access control |

### 2.1 Key Relationships

All derivation below is handled by `kms` (`crates/core/kms/src/derivation.rs`). Oubli calls these functions; it does not reimplement them.

```
Seed Phrase (BIP-39, 256-bit entropy)
    |
    +-- PBKDF2-HMAC-SHA512 (BIP-39) --> 512-bit seed
         |
         +-- BIP-32 (secp256k1) + Stark grinding
              |
              +-- m/44'/9004'/0'/0/0 --> Starknet Signing Key
              |
              +-- m/44'/5454'/0'/0/0 --> Tongo Owner Key (sk_owner)
              |    |
              |    +-- pk_owner = g^sk_owner (Tongo Address)
              |
              +-- m/44'/5353'/0'/0/0 --> Tongo View Key (sk_view)
              |    |
              |    +-- pk_view = g^sk_view
              |
              +-- m/44'/1237'/0'/0/0 --> Nostr Key (secp256k1, for messaging)
              |
              +-- HKDF(sk_owner, "hint") --> Hint Key (XChaCha12)
```

> **Design Decision (from `kms`)**: The dual-key model separates spending authority (owner key, coin type 5454) from read-only decryption (view key, coin type 5353). A user can share the view key with an auditor or portfolio tracker without risking fund loss. The Stark grinding step (rejection sampling of `SHA256(seed||counter) mod stark_order`) bridges BIP-32's secp256k1 derivation to the Stark curve.

---

## 3. Key Generation

### 3.1 Entropy Requirements

| Requirement | Specification |
|-------------|--------------|
| Entropy source | `kms` uses `getrandom` crate -> delegates to `SecRandomCopyBytes` (iOS) / `SecureRandom` (Android) |
| Minimum entropy | 256 bits for seed generation (24-word mnemonic) |
| Entropy health check | NIST SP 800-90B on-line health tests (repetition count + adaptive proportion) |
| Fallback | Abort key generation if CSPRNG health check fails; never fall back to weaker source |
| `kms` enforcement | `kms` uses `rand` + `getrandom` internally; deterministic RNG gated behind `#[cfg(feature = "test-utils")]` and cannot compile in production builds |

### 3.2 Seed Generation (via `kms`)

1. Call `ghoul_kms::mnemonic::generate_mnemonic(24)` -> `kms` generates 256 bits via platform CSPRNG, returns 24-word BIP-39 mnemonic.
2. Call `ghoul_kms::mnemonic::mnemonic_to_seed(mnemonic, passphrase)` -> PBKDF2-HMAC-SHA512 (2048 rounds) -> 512-bit seed.
3. Oubli's `oubli-store` encrypts the seed with KEK immediately (see Section 4). The mnemonic string is passed to the UI via UniFFI only during backup display, then zeroized.
4. `kms` handles internal zeroization of raw entropy via `SecretFelt` / `Zeroize`.

### 3.3 Key Derivation (via `kms`)

All derivation uses BIP-32 over secp256k1, then Stark grinding for Stark curve keys.

| Derived Key | `kms` Rust Function | Coin Type | Algorithm |
|-------------|---------------------|-----------|-----------|
| Tongo Owner Key (`sk_owner`) | `ghoul_kms::derivation::derive_keypair(seed, 5454, 0, 0)` | 5454 | BIP-44 + Stark grind |
| Tongo View Key (`sk_view`) | `ghoul_kms::derivation::derive_view_keypair(seed, 0, 0)` | 5353 | BIP-44 + Stark grind |
| Starknet Signing Key | `ghoul_kms::derivation::derive_keypair(seed, 9004, 0, 0)` | 9004 | BIP-44 + Stark grind |
| Nostr Key | `ghoul_kms::derivation::derive_nostr_keypair(seed, 0, 0)` | 1237 | BIP-44 (secp256k1, no grind) |
| Hint Key | `HKDF-SHA256(ikm=sk_owner, salt="oubli-hint", info="xchacha12")` | N/A | HKDF (RFC 5869) |

### 3.4 Tongo Public Key Computation

```
pk = g^sk  (scalar multiplication on Stark curve generator)
```

Export in two formats:
- **Affine**: `{ x: bigint, y: bigint }` -- used for ZK proof generation
- **Base58**: Tongo address string -- used for display and sharing

#### Quality Gate: Key Generation
- [ ] `ghoul_kms::mnemonic::generate_mnemonic(24)` returns valid BIP-39 mnemonic (checksum verified by `validate_mnemonic`)
- [ ] `ghoul_kms::derivation::derive_keypair` for coin types 5454, 5353, 9004 all produce valid Stark curve points
- [ ] Hint key is 256 bits and deterministically reproducible from `sk_owner`
- [ ] Key derivation is deterministic: same seed always produces same keys (verified against `kms` parity vectors in `fixtures/vectors/parity/`)
- [ ] Dual-key separation: owner key (5454) differs from view key (5353) for same seed

#### Security Gate: Key Generation
- [ ] No mnemonic or key material logged, serialized to disk unencrypted, or transmitted
- [ ] Entropy health check fails gracefully (abort, not fallback)
- [ ] `kms` uses hardened BIP-44 paths only (prevents child key -> parent key attacks)
- [ ] `getrandom` confirmed to use hardware CSPRNG on target platforms
- [ ] No key material passed through IPC, clipboard, or shared memory
- [ ] Mnemonic is encrypted with KEK immediately after display to user

#### Smoke Test: Key Generation
1. Generate a new wallet -> seed phrase is 24 words, valid BIP-39 English wordlist
2. Derive keys from seed -> Tongo address matches `kms` Rust oracle for same seed
3. Restore from same seed phrase -> identical Tongo address, Starknet address, and view key produced
4. Corrupt 1 bit of entropy -> completely different mnemonic (avalanche property)
5. Kill app mid-generation, restart -> no partial key material persisted to disk
6. Derive owner key and view key from same seed -> different keys, different public keys

---

## 4. Key Storage

### 4.1 At-Rest Encryption Architecture

```
+------------------------------------------------------+
|  Platform Secure Element (SE / StrongBox / Keychain)  |
|  +--------------------------------------------------+ |
|  |  Key Encryption Key (KEK)                         | |
|  |  - Generated inside SE, never exported            | |
|  |  - AES-256-GCM                                    | |
|  |  - Bound to biometric + device                    | |
|  +--------------------------------------------------+ |
+------------------------------------------------------+
          |  Encrypts / Decrypts
          v
+------------------------------------------------------+
|  Encrypted Key Blob (App Sandbox / Keychain)          |
|  +--------------------------------------------------+ |
|  |  AES-256-GCM(KEK, nonce, seed_bytes)              | |
|  |  + AAD: app_id | device_id | version              | |
|  +--------------------------------------------------+ |
+------------------------------------------------------+
```

### 4.2 Platform-Specific Implementation

| Platform | Secure Element | Key Storage API | Biometric API |
|----------|---------------|----------------|---------------|
| iOS | Secure Enclave (SEP) | Keychain Services (`kSecAttrAccessibleWhenUnlockedThisDeviceOnly`) | LocalAuthentication (Face ID / Touch ID) |
| Android | StrongBox / TEE (Keymaster HAL) | Android Keystore (`setUserAuthenticationRequired(true)`) | BiometricPrompt (`BIOMETRIC_STRONG`) |

### 4.3 Storage Rules

1. **Seed phrase**: Encrypted with KEK in platform secure storage. Never stored in plaintext. Never stored in app sandbox files, SharedPreferences, UserDefaults, or SQLite.
2. **Derived private keys** (`sk`, Starknet key): Re-derived from seed on unlock. Held in memory only while wallet is active. Zeroized on lock/background.
3. **Public keys** (`pk`, Starknet address): May be stored in app sandbox (non-sensitive). Cached for UI display.
4. **Hint key**: Re-derived from `sk` on unlock. Never persisted independently.
5. **Session tokens**: Ephemeral, in-memory only.

### 4.4 Memory Protection

`kms` now provides comprehensive in-library zeroization (commits 631e440, 434ad4d):
- `SecretFelt`: volatile write + SeqCst fence on drop, redacted `Debug`, explicit `.expose_secret()` access
- All proof blinding factors wrapped in `SecretFelt` (automatic zeroization on scope exit, including early returns)
- Scalar intermediate byte arrays (`scalar_add`, `scalar_mul`, `reduce_scalar`) explicitly zeroized
- Key derivation intermediate values (`seed`, HMAC outputs, chain codes) wrapped in `Zeroizing<[u8; 32]>`

Oubli adds platform-level protections on top:

| Mechanism | `kms` (library) | iOS (Oubli shell) | Android (Oubli shell) |
|-----------|-----------------|--------------------|-----------------------|
| Zeroization on drop | `SecretFelt` + `Zeroize` crate | N/A (secrets stay in `liboubli` Rust) | N/A (secrets stay in `liboubli` Rust) |
| Prevent paging to disk | N/A (caller responsibility) | `mlock()` on buffers during `kms` calls | `mlock()` via NDK during `kms` calls |
| Prevent screenshots | N/A | `UIScreen` notification overlay | `FLAG_SECURE` on sensitive screens |
| Debug protection | N/A | `ptrace(PT_DENY_ATTACH)` in release | `android:debuggable=false`, tamper detection |
| No GC interference | Guaranteed (Zig/Rust, no GC) | N/A (secrets never in Swift heap) | N/A (secrets never in Kotlin heap) |

> **Note**: The mnemonic string passed via UniFFI for backup display is the one value that briefly exists in Swift/Kotlin memory. Oubli must zeroize this native string immediately after the user dismisses the backup screen.

#### Quality Gate: Key Storage
- [ ] KEK is generated inside Secure Enclave / StrongBox (attestation verified)
- [ ] Encrypted key blob uses AES-256-GCM with unique nonce per encryption
- [ ] AAD includes app identifier and schema version (detects blob transplant attacks)
- [ ] Key material is only in memory while wallet is unlocked
- [ ] Public keys load correctly from cache after app restart
- [ ] Storage migration path exists for schema version upgrades

#### Security Gate: Key Storage
- [ ] No plaintext key material in: filesystem, logs, crash reports, analytics, backups
- [ ] `kSecAttrAccessibleWhenUnlockedThisDeviceOnly` (iOS) prevents iCloud Keychain sync of secrets
- [ ] Android backup exclusion rules applied (`android:allowBackup="false"` or encrypted backup only)
- [ ] Rooted / jailbroken device detection prevents key access (with user-facing warning)
- [ ] Key blob authenticated with AAD prevents copy to different app or device
- [ ] Failed biometric attempts trigger exponential backoff (platform-enforced)

#### Smoke Test: Key Storage
1. Lock wallet -> attempt to read key blob without biometric -> decryption fails
2. Kill app process -> inspect memory dump -> no key material found
3. Copy app sandbox to another device -> key blob decryption fails (device-bound KEK)
4. Uninstall and reinstall app -> previous key blob inaccessible (Keychain: `kSecAttrAccessibleWhenUnlockedThisDeviceOnly`)
5. Background app for 30 seconds -> return -> biometric re-auth required to transact
6. Screenshot attempt on seed display screen -> screen capture blocked or blurred

---

## 5. Key Access Control

### 5.1 Authentication Tiers

| Tier | Auth Required | Operations Permitted |
|------|--------------|---------------------|
| **T0 - Locked** | None | View app icon, receive push notifications |
| **T1 - View Only** | Biometric OR PIN | View balances (uses hint key for fast decryption), view Tongo address, view transaction history |
| **T2 - Transact** | Biometric AND PIN | Fund, Transfer, Rollover, Withdraw |
| **T3 - Critical** | Biometric AND PIN AND confirmation dialog | Ragequit, Export seed phrase, Change PIN |

### 5.2 Session Management

| Parameter | Value | Rationale |
|-----------|-------|-----------|
| T1 session timeout | 5 minutes idle | Balance between UX and security |
| T2 session timeout | 2 minutes idle OR app backgrounded | ZK proof generation happens in-session |
| T3 session timeout | Single-use (re-auth per operation) | Irreversible operations |
| Max failed biometric | 5 (platform-enforced lockout) | Brute-force prevention |
| Max failed PIN | 10 -> wipe keys from device | Anti-extraction |

### 5.3 PIN Requirements

| Requirement | Specification |
|-------------|--------------|
| Minimum length | 6 digits |
| Complexity | Not a repeating pattern (111111) or sequential (123456) |
| Storage | PIN-derived KEK via Argon2id (memory=64MB, iterations=3, parallelism=4) |
| Purpose | Secondary auth factor; backup when biometric unavailable |

#### Quality Gate: Key Access Control
- [ ] Each auth tier is enforced independently (T2 operations fail with only T1 auth)
- [ ] Session timeout triggers zeroization of in-memory key material
- [ ] PIN lockout counter persists across app restarts
- [ ] Biometric enrollment change (new fingerprint added) invalidates existing KEK

#### Security Gate: Key Access Control
- [ ] PIN is never stored; only the Argon2id-derived KEK is used
- [ ] Auth state is not persisted to disk (no "remember me" for T2/T3)
- [ ] Downgrade attack prevention: T2 operation cannot be replayed at T1
- [ ] Rate limiting on PIN attempts is enforced at storage layer, not app logic

#### Smoke Test: Key Access Control
1. Open app cold -> only T0 state visible
2. Biometric auth -> balances visible (T1)
3. Attempt transfer without PIN -> blocked, PIN prompt shown
4. Enter correct PIN -> transfer executes (T2)
5. Background app for 3 minutes -> return -> session expired, biometric required
6. Enter wrong PIN 10 times -> key material wiped, recovery prompt shown
7. Add new fingerprint to device -> next app open requires PIN (biometric invalidated)

---

## 6. Key Backup & Recovery

### 6.1 Seed Phrase Backup (Primary)

The seed phrase is the single recovery artifact. All keys are re-derivable.

| Requirement | Specification |
|-------------|--------------|
| Display | Show 24 words one-at-a-time or in groups of 4, never all on one copyable screen |
| Verification | User must re-enter words 3, 8, 17 (random subset) to confirm backup |
| Clipboard | Clipboard access blocked during seed display |
| Screen capture | Blocked via platform API during seed display |
| Re-display | Requires T3 auth (biometric + PIN + confirmation) |
| Format | BIP-39 English wordlist, compatible with standard wallet recovery |

### 6.2 Social Recovery (Optional, Future)

| Component | Design |
|-----------|--------|
| Scheme | Shamir's Secret Sharing (SSS) with t-of-n threshold |
| Default | 3-of-5 |
| Share format | Encrypted with recipient's public key before distribution |
| Storage | Shares distributed to trusted contacts via end-to-end encrypted channel |
| Reconstruction | Requires t shares + biometric auth on new device |

### 6.3 Cloud Backup (Optional, Encrypted)

| Component | Design |
|-----------|--------|
| Payload | AES-256-GCM encrypted seed, key derived from user password via Argon2id |
| Storage | iCloud Keychain (iOS) / Google Drive (Android), app-specific scoped |
| Key derivation | Argon2id(password, salt=device_id, memory=128MB, iterations=4, parallelism=4) |
| Metadata | No plaintext metadata about wallet contents (no balance, no address) |
| Versioning | Schema version in AAD to support future format changes |

### 6.4 Recovery Flow

```
User enters 24-word seed
         |
         v
BIP-39 checksum validation
         |
         v
PBKDF2-HMAC-SHA512 --> 512-bit seed
         |
         v
HD derivation --> Starknet key + Tongo key (sk)
         |
         v
Compute pk = g^sk
         |
         v
Query Tongo contract(s) for account state by pk
         |
         v
Decrypt balances using sk
         |
         v
Wallet restored
```

#### Quality Gate: Key Backup & Recovery
- [ ] Seed phrase round-trip: generate -> backup -> restore -> identical keys
- [ ] Recovery works on different device, different OS version
- [ ] Recovery works offline (key derivation is local-only)
- [ ] Cloud backup decrypt/encrypt round-trip with known test vectors
- [ ] Invalid seed phrase (bad checksum) is rejected with clear error

#### Security Gate: Key Backup & Recovery
- [ ] Seed phrase never transmitted over network
- [ ] Seed display screen blocks screenshots, screen recording, accessibility readers
- [ ] Cloud backup password strength enforced (min 12 chars, zxcvbn score >= 3)
- [ ] No server-side component has access to plaintext seed or derived keys
- [ ] Shamir shares are never reconstructed on a server

#### Smoke Test: Key Backup & Recovery
1. Create wallet -> write down seed -> delete app -> reinstall -> recover with seed -> same balances
2. Create wallet -> enable cloud backup -> factory reset device -> recover from cloud -> same wallet
3. Enter 23 correct words + 1 wrong word -> recovery rejected (checksum fail)
4. Screenshot during seed display -> blocked (platform API verified)
5. Recover on Android from seed generated on iOS -> identical wallet

---

## 7. Key Usage in Tongo Operations

### 7.1 Operation-Key Matrix

| Operation | Tongo SK | Tongo PK | Starknet Key | Hint Key | Auditor PK |
|-----------|----------|----------|-------------|----------|------------|
| **Fund** | ZK proof of ownership | Encryption target | Tx signing + gas | Balance hint update | Audit encryption |
| **Transfer** | ZK proof of ownership + amount proof + balance proof | Sender encryption | Tx signing + gas | Balance hint update | Amount + balance audit |
| **Rollover** | ZK proof of ownership | Balance update target | Tx signing + gas | Balance hint update | Audit encryption |
| **Withdraw** | ZK proof of ownership + balance proof | Encryption target | Tx signing + gas | Balance hint update | Audit encryption |
| **Ragequit** | ZK proof of ownership + full balance disclosure | Encryption target | Tx signing + gas | N/A (balance zeroed) | Audit encryption |
| **View Balance** | Decrypt ciphertext (`L/R^sk = g^b`) | N/A | N/A | Fast decrypt shortcut | N/A |

### 7.2 ZK Proof Generation Security

| Requirement | Specification |
|-------------|--------------|
| Randomness for proofs | `kms` `she-core` uses `getrandom` -> platform CSPRNG per proof; deterministic RNG impossible in production (`#[cfg(feature = "test-utils")]` gated) |
| Proof binding | `chain_id + contract_address + nonce` included in proof to prevent replay |
| Proof generation location | On-device, inside `kms` `she-core` / `tongo-sdk` Rust crates (private key never crosses UniFFI boundary) |
| Computation timeout | Oubli wraps `kms` proof calls with a 30s timeout; abort on expiry |
| Memory ceiling | ZK proof generation capped at 512MB memory |
| Implementation | Native Rust via `starknet-types-core` (not WASM, not JS runtime) |
| Performance | `kms` benchmarks: PoE <1ms, PoE2 <2ms, ElGamal <5ms, Range proofs scale with bit_size |
| Timing caveat | `kms` scalar mul is NOT constant-time (inherited from `starknet-types-core`); documented risk |

### 7.3 Transfer-Specific Key Handling

Transfers require encryption under **two different public keys** (sender and receiver):
1. Encrypt amount under sender's `pk` (for balance subtraction)
2. Encrypt same amount under receiver's `pk` (for pending balance addition)
3. ZK proof of same-encryption (both ciphertexts encode identical amount)

The receiver's `pk` must be validated before use:
- Verify it lies on the Stark curve
- Verify it is not the point at infinity
- Verify it is not the sender's own `pk` (optional: allow self-transfer for rollover-like behavior)

#### Quality Gate: Key Usage
- [ ] Each operation type produces valid ZK proofs that pass on-chain verification
- [ ] **Devnet "Proof Of Ownership failed":** The Tongo contract ([fatlabsxyz/tongo](https://github.com/fatlabsxyz/tongo/tree/master/packages/contracts/src)) builds the fund prefix as `poseidon(chain_id, tongo_address, **sender_address**, fund_selector, y.x, y.y, amount, nonce)` — see `structs/operations/fund.cairo` and `structs/traits.cairo` (`GeneralPrefixData`). The client proof must use the **same** prefix including **sender_address** (the Starknet account that will call `fund`, i.e. `get_caller_address()`). The kms/tongo-sdk currently does not include `sender_address` in the fund prefix; add it to `FundParams` and to the prefix hashed in proof generation so it matches the contract. Same check for rollover/withdraw/ragequit/transfer if they use `GeneralPrefixData`. Contract `Fund` struct order is `to, amount, hint, proof, auditPart` (see `structs/operations/fund.cairo`); calldata must match.
- [ ] Proof generation uses fresh randomness (no nonce reuse across operations)
- [ ] Balance decryption matches on-chain state for known test accounts
- [ ] Transfer receiver `pk` validation rejects invalid curve points
- [ ] Proof binding fields match live chain configuration

#### Security Gate: Key Usage
- [ ] Private key `sk` never serialized into proof output (only used as witness)
- [x] Blinding factors (`r`, `r_b`, `r_r`, `k1`, `k2`) wrapped in `SecretFelt`, zeroized on drop (commit 434ad4d)
- [x] Scalar intermediate bytes in proof computation zeroized after use (commit 434ad4d)
- [ ] Receiver `pk` is validated on-curve before encryption (invalid curve attack prevention)
- [ ] Nonce is fetched from chain immediately before proof generation (stale nonce = rejected tx)
- [x] Failed proof generation zeroizes intermediate values (via `SecretFelt` drop on early return / `?` propagation)

#### Smoke Test: Key Usage
1. Fund 100 tokens -> balance decrypts to 100
2. Transfer 50 tokens to known account -> sender balance = 50, receiver pending = 50
3. Rollover on receiver -> pending = 0, balance = 50
4. Withdraw 25 -> balance = 25, ERC20 received
5. Ragequit remaining -> balance = 0, all ERC20 returned
6. Attempt transfer with stale nonce -> rejected on-chain (replay protection works)
7. Attempt transfer to invalid public key (not on curve) -> rejected client-side

---

## 8. Key Lifecycle

### 8.1 State Machine

```
              generate
  [Empty] ─────────────> [Active]
                            |
                +-----------+-----------+
                |           |           |
             lock/bg    export seed   destroy
                |           |           |
                v           v           v
           [Locked]    [Displayed]  [Destroyed]
                |           |
             unlock      dismiss
                |           |
                v           v
            [Active]    [Active]
```

### 8.2 Lifecycle Events

| Event | Action | Key Material State |
|-------|--------|-------------------|
| **Wallet creation** | Generate seed, derive all keys, encrypt seed with KEK | Seed encrypted at rest, derived keys in memory |
| **App unlock** | Decrypt seed, re-derive keys | Derived keys in memory |
| **App lock / background** | Zeroize all derived keys in memory | Only encrypted seed at rest |
| **Transaction** | Use `sk` for ZK proof, Starknet key for signing | Keys in memory during operation |
| **Seed export** | Decrypt seed, display mnemonic, re-encrypt | Seed in display buffer, zeroized on dismiss |
| **PIN change** | Re-derive KEK with new PIN, re-encrypt seed | Brief window: seed in memory for re-encryption |
| **Device wipe / uninstall** | Platform deletes Keychain/Keystore entries | All key material destroyed |
| **Remote wipe (future)** | Push notification triggers key zeroization | Emergency: zeroize seed blob, derived keys |

### 8.3 Key Rotation

Tongo does not support on-chain key rotation (the public key **is** the account identifier). Rotation requires:

1. Create new Tongo account (new `sk'`, new `pk'`)
2. Transfer all balance from old account to new account
3. Destroy old key material

> **Implication**: Key compromise requires full account migration, not just rotation. The wallet should make this flow as frictionless as possible.

#### Quality Gate: Key Lifecycle
- [ ] State machine transitions are exhaustively tested (all valid transitions + all invalid ones rejected)
- [ ] Zeroization on lock is verified via memory inspection in debug builds
- [ ] Key rotation flow preserves total balance (no loss during migration)
- [ ] Uninstall leaves no key material recoverable from device

#### Security Gate: Key Lifecycle
- [ ] No state transition bypasses authentication
- [ ] Background transition triggers zeroization within 100ms
- [ ] Seed export increments a counter visible in security audit log
- [ ] Key destruction is cryptographic (zeroize + delete blob), not just file deletion

#### Smoke Test: Key Lifecycle
1. Create wallet -> lock -> unlock -> same keys available
2. Background app -> memory dump -> no `sk` in memory
3. Rotate key -> old account balance = 0, new account balance = previous total
4. Uninstall app -> forensic scan of app sandbox -> no key material

---

## 9. Threat Model

### 9.1 Threats & Mitigations

| Threat | Impact | Mitigation |
|--------|--------|------------|
| **Device theft (locked)** | Attacker has encrypted key blob | Biometric + PIN required; 10-fail wipe; device-bound KEK |
| **Device theft (unlocked)** | Keys in memory | Auto-lock on background; T2/T3 require re-auth; app-level screen lock |
| **Malware on device** | Keylogger, memory scraping | Secure Enclave operations; mlock'd buffers; custom keyboard for PIN; anti-debug |
| **Supply chain (compromised `kms` or dependency)** | Malicious code in `kms` or its dependency tree | Pin `kms` git dep to exact commit hash in `Cargo.toml`; `cargo audit` + `cargo vet` on full dep tree; verify `kms` parity vectors on each version bump |
| **Network MITM** | Intercept RPC calls | HTTPS + certificate pinning to Starknet RPC; no key material over network |
| **Backup theft** | Attacker gets cloud backup | Argon2id-encrypted; password required; no metadata leaks wallet info |
| **Side-channel on ZK proof** | Timing/power analysis leaks `sk` | `kms` scalar mul is NOT constant-time (known caveat); mitigate with proof generation in isolated thread + noise; track `starknet-types-core` for CT support |
| **Nonce reuse in ElGamal** | Breaks ciphertext indistinguishability | Fresh CSPRNG `r` per encryption; never cache or reuse blinding factors |
| **Invalid curve attack** | Receiver sends malicious `pk` | Validate all external public keys on-curve before use |
| **Stale nonce replay** | Old proof replayed after state change | Fetch nonce from chain immediately before proof; proofs include nonce binding |

### 9.2 Out of Scope (v1)

- Hardware wallet integration (Ledger, Trezor)
- Multi-signature Tongo accounts
- Threshold signing for Starknet transactions
- Post-quantum key encapsulation
- Multi-token support (STRK, ETH, USDC, etc.) -- Oubli v1 is wBTC-only

---

## 10. Product Scope

### 10.1 Single-Token: wBTC Only

Oubli is a **Bitcoin wallet on Starknet**. It supports exactly one Tongo instance: **wBTC**.

| Parameter | Value |
|---|---|
| Token | wBTC (Wrapped Bitcoin) |
| Tongo instance (mainnet) | wBTC Tongo contract (rate: 10) |
| Tongo instance (testnet) | Sepolia wBTC Tongo contract |
| Denomination | Display in BTC (8 decimals), transact in Tongo units (rate-adjusted) |
| Other tokens | Not supported in v1. No STRK, ETH, USDC, or multi-token UI. |

The rate value (10) means 1 Tongo unit = 10 wBTC base units. The wallet must handle this conversion transparently -- the user sees BTC amounts, never Tongo units.

### 10.2 Auto-Rollover

Tongo's dual-balance model (current balance + pending balance) requires an explicit rollover operation to move received funds into the spendable balance. **Oubli auto-rolls over transparently:**

1. On every app unlock (T1+), query account state via `kms` `starknet-client`.
2. If `pending_balance > 0`, automatically generate rollover proof and submit.
3. UI shows a single "Balance" number (current + pending combined for display). A subtle indicator shows if a rollover is in-flight.
4. User never sees "pending" as a separate concept. Incoming transfers appear as "confirming..." until rollover completes.

**Edge case**: if the user tries to spend while rollover is in-flight, queue the spend after rollover confirmation (sequential nonce).

#### Quality Gate: Auto-Rollover
- [ ] Rollover triggers automatically when pending > 0 on unlock
- [ ] Combined balance display matches `current + pending` before rollover, matches `new_current` after
- [ ] UI shows "confirming..." state during rollover in-flight
- [ ] Spend-after-rollover queuing works (sequential execution)

#### Smoke Test: Auto-Rollover
1. Receive transfer while app is closed -> open app -> rollover fires automatically -> balance updates
2. Receive transfer -> open app -> immediately attempt spend -> spend queues behind rollover -> both succeed
3. Rollover fails (RPC down) -> user sees error, balance shows pending separately as fallback

### 10.3 Gas & Fee Management via AVNU Paymaster

Oubli uses the [AVNU Paymaster](https://docs.avnu.fi/docs/paymaster/index) so users never need to hold STRK or ETH for gas. All Tongo operations are submitted as **gasfree (sponsored)** transactions via SNIP-9 outside execution.

**Flow (per SNIP-29):**

```
oubli-core generates Tongo calldata (fund/transfer/rollover/withdraw/ragequit)
    |
    v
POST to AVNU paymaster: buildTypedData(calls, gasToken=wBTC)
    |
    v
Paymaster returns SNIP-12 TypedData (OutsideExecution wrapping calls + fee)
    |
    v
oubli-core signs TypedData with Starknet signing key
    |
    v
POST to AVNU paymaster: execute(typedData, signature)
    |
    v
Paymaster relayer submits on-chain, pays gas in STRK from sponsor credits
    |
    v
Transaction confirmed, oubli-core polls for receipt
```

| Parameter | Value |
|---|---|
| Paymaster endpoint (mainnet) | `https://starknet.paymaster.avnu.fi` |
| Paymaster endpoint (testnet) | `https://sepolia.paymaster.avnu.fi` |
| Auth | `x-paymaster-api-key` header (stored as env var / build config, NOT user-facing) |
| Gas token | Gasfree mode (sponsored) for v1; gasless (user pays in wBTC) as fallback |
| SNIP version | SNIP-9 v2 outside execution + SNIP-29 paymaster API |

**Implication for key usage**: The Starknet signing key signs the SNIP-12 TypedData (not a raw transaction). The `kms` `starknet-client` crate must support TypedData signing, or Oubli adds this in `oubli-wallet`.

#### Quality Gate: Paymaster Integration
- [ ] `buildTypedData` returns valid SNIP-12 TypedData for each Tongo operation type
- [ ] TypedData signature verifies against Starknet account public key
- [ ] `execute` returns transaction hash; transaction appears on-chain
- [ ] Fallback to gasless mode (user pays wBTC fee) if sponsor credits exhausted

#### Security Gate: Paymaster Integration
- [ ] Paymaster API key is NOT embedded in app binary (injected at build time or fetched from secure config)
- [ ] TypedData call array validated: must match Oubli's submitted calls (no extra calls injected by paymaster)
- [ ] HTTPS enforced for all paymaster communication
- [ ] Transaction hash verified on-chain after paymaster submission (don't trust paymaster receipt alone)

#### Smoke Test: Paymaster Integration
1. Fund operation -> paymaster sponsors gas -> user pays zero STRK -> transaction confirmed
2. Transfer operation -> paymaster sponsors -> recipient sees pending balance
3. Paymaster down -> operation fails gracefully with user-facing error, no state corruption
4. Paymaster returns TypedData with extra call -> Oubli rejects (call array mismatch)

---

## 11. Network & RPC

### 11.1 Configuration

| Parameter | Value |
|---|---|
| RPC URL | Environment variable `OUBLI_RPC_URL` (no hardcoded default) |
| Protocol | JSON-RPC over HTTPS |
| Spec version | Starknet JSON-RPC v0.10.0 |
| Certificate pinning | Optional for v1; recommended for production |
| Timeout | 10s per RPC call; 30s for proof submission |

### 11.2 RPC Usage

| Operation | RPC Calls |
|---|---|
| View balance | `starknet_call` (read Tongo contract state for account `pk`) |
| Fetch nonce | `starknet_call` (read Tongo account nonce) |
| Fetch auditor key | `starknet_call` (read Tongo instance auditor `pk`) |
| Submit operation | Paymaster `buildTypedData` + `execute` (not direct `starknet_addInvokeTransaction`) |
| Poll confirmation | `starknet_getTransactionReceipt` until confirmed or timeout |

### 11.3 Nonce Management

Tongo accounts have an on-chain nonce that increments with each operation. Nonce is bound into ZK proofs -- a proof generated with nonce N is only valid when the on-chain nonce is N.

**Strategy: sequential execution with queue.**

```
                 ┌─────────────────┐
  User action -> │  Operation Queue │ -> Process one at a time
                 └─────────────────┘
                          |
            1. Fetch nonce from chain
            2. Generate ZK proof with nonce
            3. Submit via paymaster
            4. Poll for confirmation
            5. If confirmed: dequeue, process next
               If failed: retry once with fresh nonce, then surface error
```

- **No concurrent operations.** One Tongo operation at a time per account. Operations queue in FIFO order.
- **Auto-rollover has priority.** If pending > 0, rollover is inserted at head of queue before any user-initiated operation.
- **Stale nonce recovery.** If a submitted proof is rejected (nonce mismatch), fetch fresh nonce and regenerate proof once. If it fails again, surface error to user.
- **Optimistic UI.** Balance updates optimistically on submission, reverts on failure.

#### Quality Gate: Nonce Management
- [ ] Sequential queue processes operations one at a time
- [ ] Nonce fetched from chain immediately before proof generation (not cached)
- [ ] Auto-rollover inserts at head of queue
- [ ] Stale nonce triggers one automatic retry with fresh nonce

#### Smoke Test: Nonce Management
1. Submit fund -> immediately submit transfer -> transfer queues behind fund -> both succeed sequentially
2. Two rapid transfers -> second waits for first to confirm -> both succeed with correct nonces
3. Simulate nonce mismatch (external state change) -> auto-retry with fresh nonce -> succeeds
4. Paymaster submission fails -> proof is discarded, nonce not incremented -> next attempt works

---

## 12. Target Platforms

| Parameter | iOS | Android |
|---|---|---|
| Minimum OS | iOS 17.0 | Android 14 (API 34) |
| Architecture | arm64 only | arm64-v8a only (no x86) |
| Secure Element | Secure Enclave (required) | StrongBox preferred, TEE fallback |
| Biometric | Face ID / Touch ID | Class 3 (BIOMETRIC_STRONG) |
| UI Framework | SwiftUI | Jetpack Compose |
| Language | Swift 5.9+ | Kotlin 1.9+ |
| Rust target | `aarch64-apple-ios` | `aarch64-linux-android` |
| Rust toolchain | Stable 1.75+ | Stable 1.75+ |
| UniFFI version | 0.27+ | 0.27+ |
| NDK | N/A | r26+ |

> **Rationale for recent-only:** iOS 17 and Android 14 guarantee Secure Enclave/StrongBox APIs, modern biometric support, and Swift/Kotlin language features needed for UniFFI interop. Eliminates a large surface of compatibility edge cases.

---

## 13. Testing Strategy

### 13.1 Test Layers

Tests are split between `kms` (upstream, crypto correctness) and Oubli (mobile hardening):

**Layer 1: `kms` tests (run on `kms` version bump, not every Oubli PR)**

| Test Area | `kms` Location | Coverage | Invariant |
|-----------|---------------|----------|-----------|
| Key derivation | `crates/core/kms/tests/key_derivation_vectors.rs` | 100% | Deterministic: same seed -> same keys |
| Swift parity | `crates/core/kms/tests/swift_parity_vectors.rs` | 100% | Cross-platform parity verified |
| ElGamal encrypt/decrypt | `crates/core/she-core/tests/test_vectors.rs` | 100% | `Dec(sk, Enc(pk, m, r)) == m` |
| ZK proof generation | `crates/core/tongo-sdk/tests/prover_vectors.rs` | 100% | Proofs match TypeScript reference |
| Rust-Zig parity | `tools/equivalence-harness/` | 100% | Zig output matches Rust oracle |
| Sepolia integration | `crates/core/starknet-client/tests/` | On-chain | Live contract interaction |

**Layer 2: Oubli tests (every PR)**

| Test Area | Location | Coverage | Invariant |
|-----------|----------|----------|-----------|
| Auth tier state machine | Oubli shared logic | 100% | All valid transitions pass, invalid rejected |
| PIN validation | Oubli shared logic | 100% | Reject sequential, repeating; accept valid |
| KEK wrap/unwrap | Oubli platform tests | 100% | Round-trip: encrypt seed -> decrypt -> identical |
| Session timeout | Oubli platform tests | 100% | Background triggers zeroization |
| Cloud backup encrypt/decrypt | Oubli shared logic | 100% | Argon2id round-trip with test vectors |
| `kms` integration | Oubli CI | Smoke | `kms` calls succeed on both platforms |

### 13.2 Integration Tests

| Test Scenario | Environment | Runner | Validation |
|--------------|-------------|--------|------------|
| Full wallet lifecycle | Testnet (Sepolia) | Device farm | Create -> fund -> transfer -> rollover -> withdraw -> ragequit |
| Cross-platform recovery | iOS seed -> Android restore | Device farm (Firebase Test Lab + Xcode Cloud) | Identical addresses and balances |
| Concurrent operations | Two devices, same wallet | Device farm | Nonce conflicts detected and handled |
| `kms` version upgrade | Old seed + new `kms` crate version | CI with stored test vectors | All operations succeed, no re-key needed |
| Dual-key separation | View key shared, owner key retained | CI | View key decrypts balance; view key cannot sign operations |

### 13.3 Security Tests

| Test | Method | Pass Criteria |
|------|--------|--------------|
| Memory forensics | Dump process memory after lock | Zero occurrences of key material patterns (both `liboubli` Rust heap and native heap) |
| Backup extraction | Analyze iCloud/GDrive backup | No plaintext key material or wallet metadata |
| Root/jailbreak detection | Run on rooted device | Warning displayed, optional key wipe |
| Binary analysis | `nm` / `strings` on release `liboubli.a` / `liboubli.so` + app binary | No hardcoded keys, no key material in strings |
| Timing analysis | Measure ZK proof generation time vs amount | Document variance (known non-CT in `kms`); flag if exploitable |
| `kms` dependency audit | `cargo audit` + `cargo vet` on pinned `kms` commit | Zero known vulnerabilities |

### 13.4 Build & CI Pipeline

```
PR opened (Oubli repo)
  │
  ├── Lint & format (SwiftLint, ktlint, swiftformat)
  ├── Oubli unit tests (auth, PIN, backup, session)
  │
  ├── cargo build --target aarch64-apple-ios       (build liboubli.a)
  ├── cargo build --target aarch64-linux-android   (build liboubli.so)
  ├── uniffi-bindgen generate                       (Swift + Kotlin bindings)
  │
  ├── xcodebuild test (iOS simulator: UniFFI smoke + Oubli auth tests)
  ├── ./gradlew connectedAndroidTest (emulator: UniFFI smoke + Oubli auth tests)
  │
  └── Sepolia integration (nightly only, device farm)

kms version bump (triggered manually or by kms release)
  │
  ├── Run kms parity vectors (fixtures/vectors/parity/)
  ├── Run kms Rust test suite (cargo test in kms repo)
  ├── cargo build oubli-core for all targets with new kms
  ├── Run Oubli smoke tests against new kms
  └── Update pinned kms commit in oubli-core Cargo.toml
```

### 13.5 Smoke Test Suite (CI/CD)

Every Oubli PR must pass:

```
SMOKE-KMS-001: generate_wallet() -> valid 24-word BIP-39 mnemonic + valid Stark curve point
SMOKE-KMS-002: derive_keypair(seed, coin_type=5454) -> valid Stark curve point (via kms)
SMOKE-KMS-003: Generate wallet -> export seed -> restore -> identical pk (via kms)
SMOKE-KMS-004: Lock wallet -> attempt T2 operation -> rejected by auth tier
SMOKE-KMS-005: Fund 100 -> decrypt balance via kms -> equals 100
SMOKE-KMS-006: Transfer 50 -> sender balance = 50, receiver pending = 50
SMOKE-KMS-007: Enter wrong PIN 10x -> keys wiped from secure storage
SMOKE-KMS-008: Background app -> foreground -> biometric required (session expired)
SMOKE-KMS-009: Invalid recipient pk (off-curve) -> transfer rejected client-side
SMOKE-KMS-010: Seed phrase with bad checksum -> restore returns Err(InvalidChecksum)
SMOKE-KMS-011: Cloud backup encrypt -> decrypt -> identical seed bytes
SMOKE-KMS-012: liboubli.a builds for aarch64-apple-ios + UniFFI Swift bindings compile in Xcode
SMOKE-KMS-013: liboubli.so builds for aarch64-linux-android + UniFFI Kotlin bindings compile in Gradle
SMOKE-KMS-014: Derive owner key (5454) != view key (5353) for same seed
SMOKE-KMS-015: View key can decrypt balance but cannot generate transfer proof
SMOKE-OPS-001: Fund via paymaster -> balance increases, user pays zero STRK
SMOKE-OPS-002: Receive transfer -> reopen app -> auto-rollover fires -> balance updates
SMOKE-OPS-003: Two rapid operations -> second queues behind first -> both succeed
SMOKE-OPS-004: Nonce mismatch -> auto-retry with fresh nonce -> succeeds
SMOKE-OPS-005: Paymaster returns mismatched TypedData calls -> Oubli rejects
SMOKE-OPS-006: Balance displays in BTC (8 decimals), not Tongo units
```

---

## 14. Compliance & Audit Considerations

### 14.1 Tongo Auditor Integration

The Tongo protocol supports a global auditor with public key `y_a`. The wallet must:

1. **Encrypt balance updates for auditor** during every state-changing operation (Fund, Transfer, Rollover, Withdraw).
2. **Generate audit ZK proofs** confirming auditor-encrypted amounts match actual balance changes.
3. **Compute audit hints** using a shared key derived from `sk` and auditor `pk` (ECDH-like).
4. **Support selective disclosure** to third parties via viewing keys without revealing `sk`.

### 14.2 Auditor Key Handling

| Requirement | Specification |
|-------------|--------------|
| Auditor `pk` source | Fetched from on-chain Tongo contract |
| Validation | Verify auditor `pk` on-curve before use |
| Caching | Cache auditor `pk` with TTL; re-fetch before critical operations |
| Multi-auditor | Support distributed auditor keys (`y_a = g^(a1 + a2)`) |

### 14.3 Ex-Post Proving (Selective Disclosure)

Users can prove transaction details to a third party without revealing `sk`:
- Provide viewing key for specific transaction
- ZK proof of same-encryption between user ciphertext and disclosure ciphertext
- Wallet must support generating these proofs on-demand

---

## 15. Implementation Priorities

### Phase 1 - `kms` Integration + Secure Storage (MVP)
- [ ] `oubli-core` Cargo workspace setup with `kms` crates as git dependencies (pinned commit)
- [ ] `oubli-wallet`, `oubli-auth`, `oubli-store`, `oubli-backup`, `oubli-bridge` crate scaffolding
- [ ] `OUBLI_RPC_URL` env var configuration, Starknet RPC client setup
- [ ] wBTC Tongo instance configuration (mainnet + Sepolia contract addresses, rate=10)
- [ ] BTC denomination display layer (Tongo units <-> BTC with 8 decimal formatting)
- [ ] Implement KEK wrapping (Keychain on iOS, Android Keystore on Android)
- [ ] Implement biometric gating (LocalAuthentication on iOS, BiometricPrompt on Android)
- [ ] Implement auth tier state machine (T0-T3) in `oubli-auth`
- [ ] Implement PIN + Argon2id KEK derivation
- [ ] AVNU paymaster integration: `buildTypedData` + SNIP-12 TypedData signing + `execute`
- [ ] Operation queue with sequential nonce management
- [ ] Fund + View Balance operations (via `kms` `tongo-sdk`)
- [ ] Auto-rollover on unlock (detect pending > 0, queue rollover at head)
- [ ] Seed backup display + verification flow (24 words, clipboard blocked, screenshots blocked)
- [ ] Seed restore flow (enter mnemonic -> `ghoul_kms::validate_mnemonic` -> derive keys -> query chain)
- [ ] iOS SwiftUI shell: renders `WalletState`, emits user actions
- [ ] Android Compose shell: renders `WalletState`, emits user actions
- [ ] CI pipeline: `cargo test` + cross-compile `liboubli` + UniFFI bindgen + platform smoke tests

### Phase 2 - Full Operations
- [ ] Transfer with recipient `pk` validation (call `kms` `tongo-sdk`)
- [ ] Withdraw
- [ ] Ragequit
- [ ] Transaction history with decryption (via view key)
- [ ] Auditor integration (audit proofs generated by `kms` `she-core`)
- [ ] Session management (timeout, background zeroization, re-auth)
- [ ] Gasless fallback (user pays wBTC fee if sponsor credits exhausted)
- [ ] Stale nonce auto-retry (one retry with fresh nonce on mismatch)

### Phase 3 - Hardening
- [ ] Cloud backup: AES-256-GCM + Argon2id encrypted seed to iCloud / GDrive
- [ ] Root/jailbreak detection (native shells)
- [ ] Memory forensics testing in CI (process memory scan post-lock, both `liboubli` Rust heap and native heap)
- [ ] Timing analysis for ZK proofs (document `kms` non-CT scalar mul risk)
- [ ] Certificate pinning for Starknet RPC (native shells)
- [ ] Key rotation / account migration flow (new `kms` account -> transfer -> destroy old)
- [ ] Selective disclosure / ex-post proving (via `kms` `she-core` SameEncryption proofs)
- [ ] `mlock()` on buffers during `kms` calls (native wrapper)

### Phase 4 - Advanced (Future)
- [ ] Social recovery (Shamir's Secret Sharing)
- [ ] Multi-device sync
- [ ] Hardware wallet integration (Ledger Starknet app)
- [ ] Remote wipe capability
- [ ] Nostr-based encrypted messaging between wallet users (via `kms` `nostr-messaging`)
