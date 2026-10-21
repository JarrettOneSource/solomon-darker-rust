//! Float narrowing helpers shared by every kernel.

mod atan;
mod pow;
mod sincos;

pub use atan::atan2;
pub use pow::pow;
pub use sincos::{cos, sin};

/// Source pair of `Math.sin` and `Math.cos`, evaluated from the same scalar operand.
#[must_use]
pub fn sin_cos(value: f64) -> (f64, f64) {
    (sin(value), cos(value))
}

/// Narrows a double to the nearest `f32` and widens it back.
///
/// This is JavaScript's `Math.fround` and the native x87 32-bit store. The reference kernels
/// round at exactly these points, so the port rounds there too and nowhere else.
#[inline]
#[must_use]
pub const fn fround(value: f64) -> f64 {
    value as f32 as f64
}

/// Euclidean length, computed the way V8 computes `Math.hypot`.
///
/// The reference kernels call `Math.hypot`, and V8 scales both operands by the larger one and
/// sums the squares with Kahan compensation before the square root. Reproducing that algorithm
/// (rather than `sqrt(x² + y²)`, which can differ by an ulp) keeps the port bit-identical to
/// the Website kernels, which the differential goldens in `tests/website_goldens.rs` require.
#[inline]
#[must_use]
pub fn hypot(x: f64, y: f64) -> f64 {
    hypot_components([x, y])
}

/// The same V8 scaled sum for a native three-dimensional boulder orientation axis.
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
