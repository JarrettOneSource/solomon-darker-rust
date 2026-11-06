//! The game's deterministic math: single-precision narrowing, `hypot`, and fdlibm's `sin`,
//! `cos`, `atan2` and `pow`. Platform libm can differ in the last bit; these give the same
//! bits on native and WebAssembly, which recorded goldens and saved trajectories depend on.

mod atan;
mod pow;
mod sincos;

pub use atan::atan2;
pub use pow::pow;
pub use sincos::{cos, sin};

/// The scalar `sin` and `cos` of one operand, rather than a combined platform routine.
#[must_use]
pub fn sin_cos(value: f64) -> (f64, f64) {
    (sin(value), cos(value))
}

/// Narrows a double to the nearest `f32` and widens it back.
///
/// This is the original game's x87 store to a 32-bit float. Each call is a single-precision
/// boundary: kernels otherwise compute in `f64`, and adding or removing a boundary changes
/// saved and recorded trajectories.
#[inline]
#[must_use]
pub const fn fround(value: f64) -> f64 {
    value as f32 as f64
}

/// Euclidean length of `(x, y)`.
///
/// Both operands are scaled by the larger magnitude, and their squares are summed with Kahan
/// compensation before the square root. Plain `sqrt(x² + y²)` can differ by an ulp and
/// overflows sooner; the recordings in `tests/website_goldens.rs` and saved trajectories
/// depend on these exact bits.
#[inline]
#[must_use]
pub fn hypot(x: f64, y: f64) -> f64 {
    hypot_components([x, y])
}

/// The same scaled, compensated length for three components, such as a boulder's 3D
/// orientation axis.
#[inline]
#[must_use]
pub fn hypot3(x: f64, y: f64, z: f64) -> f64 {
    hypot_components([x, y, z])
}

#[inline]
fn hypot_components<const N: usize>(values: [f64; N]) -> f64 {
    if values.iter().any(|value| value.is_infinite()) {
        return f64::INFINITY;
    }
    if values.iter().any(|value| value.is_nan()) {
        return f64::NAN;
    }
    let mut max: f64 = 0.0;
    for value in values {
        max = max.max(value.abs());
    }
    if max == 0.0 {
        return 0.0;
    }
    let mut sum = 0.0;
    let mut compensation = 0.0;
    for value in values {
        let n = value.abs() / max;
        let summand = n * n - compensation;
        let preliminary = sum + summand;
        compensation = (preliminary - sum) - summand;
        sum = preliminary;
    }
    sum.sqrt() * max
}

/// IEEE-754 bits of the `f32` nearest to `value`, formatted the way the goldens record them.
#[must_use]
pub fn float32_bits(value: f64) -> String {
    format!("0x{:08X}", (value as f32).to_bits())
}
