//! Source draw arithmetic shared by the presentation families.

use crate::float::fround;

pub(super) const WHITE: u32 = 0xffffff;
pub(super) fn degrees_to_radians(degrees: f64) -> f64 {
    degrees * std::f64::consts::PI / 180.0
}
pub(super) fn hash_unit(first: u64, second: u64) -> f64 {
    let mut value = (first as u32 ^ 0x9e3779b9).wrapping_mul(0x85ebca6b)
        ^ (second as u32).wrapping_mul(0xc2b2ae35);
    value ^= value >> 16;
    f64::from(value) / 4294967296.0
}
pub(crate) fn repeated_float_decay(initial: f64, decay: f64, ticks: u64) -> f64 {
    let mut value = fround(initial);
    let decay = fround(decay);
    for _ in 0..ticks {
        value = fround(value - decay).max(0.0);
        if value == 0.0 {
            break;
        }
    }
    value
}
pub fn repeated_float_multiply(initial: f64, factor: f64, ticks: u64) -> f64 {
    let mut value = fround(initial);
    let factor = fround(factor);
    for _ in 0..ticks {
        value = fround(value * factor);
    }
    value
}
pub(crate) fn pack_normalized_rgb(red: f64, green: f64, blue: f64) -> u32 {
    let byte = |value: f64| (value.clamp(0.0, 1.0) * 255.0 + 0.5).floor() as u32;
    byte(red) << 16 | byte(green) << 8 | byte(blue)
}
pub(super) fn pack_gray(value: f64) -> u32 {
    let channel = (value * 255.0 + 0.5).floor().clamp(0.0, 255.0) as u32;
    channel << 16 | channel << 8 | channel
}
pub(super) fn clamp_entry(value: f64, first: usize, last: usize) -> usize {
    (value + 0.5).floor().clamp(first as f64, last as f64) as usize
}
