# oubli-backup

Seed phrase backup and recovery for the Oubli wallet.

## Overview

Handles seed phrase display, user verification prompts, and cloud backup encryption. Supports both manual (write-down) and encrypted cloud recovery flows. The current mobile shells generate 12-word BIP-39 mnemonics through the bridge, while `SeedDisplayFlow` itself can chunk any non-empty mnemonic.

## Modules

- **`seed_display.rs`** — `SeedDisplayFlow` chunks 12/24 words into groups for UI display
- **`restore.rs`** — Recovery from cloud backup or manual seed entry
- **`cloud.rs`** — `CloudBackup` with encrypted payloads for iCloud/Google Drive

## Usage

```rust
use oubli_backup::{SeedDisplayFlow, RestoreFlow};

// Display seed phrase in groups of 4
let flow = SeedDisplayFlow::new(&mnemonic);
for group in flow.word_groups() {
    // show group to user
}

// Restore from mnemonic
let restore = RestoreFlow::from_mnemonic(phrase)?;
```
