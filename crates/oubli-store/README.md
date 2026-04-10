# oubli-store

Encrypted blob storage for the Oubli wallet.

## Overview

Provides a secure key-value storage abstraction using AES-GCM authenticated encryption. Platform-specific backends (iOS Keychain, Android Keystore) implement the `PlatformStorage` trait.

## Modules

- **`blob.rs`** — `BlobManager` for encrypting/decrypting secrets with a KEK
- **`platform.rs`** — `PlatformStorage` trait for platform backends
- **`mock.rs`** — In-memory mock storage for testing

## PlatformStorage Contract

- Rust owns encryption at rest through `BlobManager`. Platform code stores opaque encrypted bytes only.
- Callers should use stable keys and treat `None` as "missing", not as an error.
- New persistence for wallet or swap state should go through this trait, not through ad hoc files or SQLite side paths.
- iOS and Android callback implementations are part of the storage boundary and should be reviewed alongside any trait or blob format change.

## Usage

```rust
use oubli_store::{BlobManager, PlatformStorage};

let storage = MyPlatformStorage::new();
let manager = BlobManager::new(storage);

// Store encrypted
manager.store("mnemonic", secret_bytes, &kek)?;

// Retrieve
let secret = manager.load("mnemonic", &kek)?;
```
