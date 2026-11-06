//! The ID and salt hash behind cosmetic Earth, Water and Ether randomness. Effects that
//! sample it never advance gameplay RNG.

pub fn visual_hash(id: u64, salt: u32) -> u32 {
    let mut value = id as u32 ^ salt.wrapping_add(1).wrapping_mul(0x9e37_79b1);
    value ^= value >> 16;
    value = value.wrapping_mul(0x7feb_352d);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846c_a68b);
    value ^ (value >> 16)
}

pub fn visual_unit_random(id: u64, salt: u32) -> f64 {
    f64::from(visual_hash(id, salt)) / 4_294_967_296.0
}
