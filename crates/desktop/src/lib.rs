//! Native platform ownership; gameplay and presentation use the shared Rust client.

#[cfg(not(target_arch = "wasm32"))]
pub mod window;

#[cfg(not(target_arch = "wasm32"))]
pub mod authority;

#[cfg(not(target_arch = "wasm32"))]
pub mod peer;

#[cfg(not(target_arch = "wasm32"))]
pub mod config;
#[cfg(not(target_arch = "wasm32"))]
pub mod profile;
#[cfg(not(target_arch = "wasm32"))]
pub mod session;
#[cfg(not(target_arch = "wasm32"))]
pub mod updates;

#[cfg(not(target_arch = "wasm32"))]
pub mod app;
