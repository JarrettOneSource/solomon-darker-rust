//! Generated scenery pixels. The shared scenery compiler owns geometry, stroke
//! order, tile origins and alpha cropping; only path rasterization is platform I/O.

use crate::assets::Image;
use solomon_sim::Vec2;

#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
pub use browser::Raster;
#[cfg(not(target_arch = "wasm32"))]
pub use native::Raster;

pub enum Paint<'a> {
    Color { rgb: [u8; 3], alpha: f64 },
    RepeatingImage { image: &'a Image, origin: Vec2 },
}
