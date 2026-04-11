#![deny(unsafe_code)]

pub mod blob;
pub mod error;
pub mod mock;
pub mod platform;

pub use blob::{BlobManager, EncryptedBlob};
pub use error::StoreError;
pub use mock::MockPlatformStorage;
pub use platform::PlatformStorage;
