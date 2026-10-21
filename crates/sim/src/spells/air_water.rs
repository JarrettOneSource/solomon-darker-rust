//! Shared Air/Water draw domains from `core-kernels/air-water-spell-actors.ts`.

mod actors;
mod host;
mod presentation;
pub use actors::*;
pub use host::*;
pub use presentation::*;

use crate::NativeRng;
use crate::float::fround;

/// Unlike the ordered-endpoint range helper, an equal damage range still draws a word.
pub fn draw_native_spell_damage(rng: &mut NativeRng, minimum: f64, maximum: f64) -> f64 {
    assert!(
        minimum.is_finite() && maximum.is_finite() && maximum >= minimum,
        "native spell damage range is invalid"
    );
    fround(minimum + rng.draw_scaled_float(maximum - minimum, false))
}

pub fn draw_native_disintegrate_percentile(rng: &mut NativeRng, chance: f64) -> bool {
    assert!(
        chance.fract() == 0.0 && (0.0..=100.0).contains(&chance),
        "native Disintegrate chance must be an integer in 0..100"
    );
    f64::from(rng.draw_integer(100)) >= 100.0 - chance
}
