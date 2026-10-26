//! Asset I/O and decoded images for the same native and browser stock inputs.

#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg(target_arch = "wasm32")]
pub use browser::{ResourceUrl, fetch_bytes, load_image};
#[cfg(target_arch = "wasm32")]
pub type Image = web_sys::HtmlImageElement;

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(not(target_arch = "wasm32"))]
pub use native::{
    Image, NativeAssets, ResourceUrl, configure, decode_image, fetch_bytes, load_image,
};
