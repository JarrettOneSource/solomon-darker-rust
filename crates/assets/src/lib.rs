//! Readers for the stock Solomon Dark asset containers.
//!
//! The retail `images/*.bundle` files are flat arrays of sprite records that index into the
//! PNG atlas of the same name. The layout (little-endian) is recorded in the Website's
//! `tools/native_bundle_art.py`:
//!
//! | offset | type        | field                                      |
//! |--------|-------------|--------------------------------------------|
//! | `0x00` | 4 × f32     | atlas `x`, `y`, `width`, `height`          |
//! | `0x10` | i32, u32    | `logical_width`, `logical_height`          |
//! | `0x18` | 2 × f32     | content width/height (equal to the crop)   |
//! | `0x20` | 2 × f32     | `center_x`, `center_y` registration offset |
//! | `0x28` | u8          | rotated flag (always zero in stock data)   |
//! | `0x29` | u32         | point count                                |
//! | `0x2D` | n × 2 × f32 | attachment points, relative to the centre  |

pub mod boneyard;
pub mod browser_hall;
pub mod browser_settings;
pub mod bundle;
pub mod json_value;
pub mod memorial;
pub mod ml_bot_checkpoint;
pub mod native_save;
pub mod native_save_source;
pub mod navigation;
pub mod prepared_mod_save;

pub use bundle::{BundleError, SpriteRecord, parse_bundle};

pub mod save_document;
pub mod website_save;
