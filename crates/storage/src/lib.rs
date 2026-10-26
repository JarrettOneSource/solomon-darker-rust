//! Durable native documents shared by host checkpoints and client persistence.
//! Gameplay validation stays in the existing save codec.

#[cfg(not(target_arch = "wasm32"))]
mod document;
#[cfg(not(target_arch = "wasm32"))]
pub use document::{DocumentFile, LockedDocument};
