//! Shared scalar operations used by the Source policy's observation modules.

use crate::Vec2;
use crate::float::{hypot, sin_cos};

pub(super) fn signed(value: f64, scale: f64) -> f64 {
    (value / scale).clamp(-1.0, 1.0)
}

pub(super) fn unsigned(value: f64, scale: f64) -> f64 {
    (value / scale).clamp(0.0, 1.0)
}

pub(super) fn ratio(value: f64, maximum: f64) -> f64 {
    if maximum > 0.0 { (value / maximum).clamp(0.0, 1.0) } else { 0.0 }
}

pub(super) fn count_scaled(value: f64) -> f64 {
    let maximum = super::spec::scales::INVENTORY_COUNT_SATURATION;
    value.clamp(0.0, maximum).ln_1p() / (maximum + 1.0).ln()
}

pub(super) fn normalized(x: f64, y: f64) -> Vec2 {
    let length = hypot(x, y);
    if length > 1e-9 { Vec2::new(x / length, y / length) } else { Vec2::ZERO }
}

pub(super) fn heading_vector(degrees: f64) -> Vec2 {
    let radians = degrees * std::f64::consts::PI / 180.0;
    let (sin, cos) = sin_cos(radians);
    Vec2::new(sin, -cos)
}

pub(super) fn directions() -> [Vec2; 8] {
    // This Source expression rounds one bit below FRAC_1_SQRT_2.
    let diagonal = 1.0 / 2.0_f64.sqrt();
    [
        Vec2::new(1.0, 0.0),
        Vec2::new(diagonal, diagonal),
        Vec2::new(0.0, 1.0),
        Vec2::new(-diagonal, diagonal),
        Vec2::new(-1.0, 0.0),
        Vec2::new(-diagonal, -diagonal),
        Vec2::new(0.0, -1.0),
        Vec2::new(diagonal, -diagonal),
    ]
}

pub(super) fn distance_squared(left: Vec2, right: Vec2) -> f64 {
    let dx = left.x - right.x;
    let dy = left.y - right.y;
    dx * dx + dy * dy
}

pub(super) fn write(block: &mut [f32], start: usize, values: &[f64]) {
    for (offset, value) in values.iter().enumerate() {
        block[start + offset] = *value as f32;
    }
}
