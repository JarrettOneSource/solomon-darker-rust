//! Player death and level-up world presentation from the Website renderer's
//! The articulated character, Enchant Staff, death and level-up presentation programs.
//! These cosmetic samples do not consume gameplay randomness or advance combat.

mod attachments;
mod death;
mod enchant_staff;
mod level_up;

pub use attachments::*;
pub use death::*;
pub use enchant_staff::*;
pub use level_up::*;

fn stable_hash(value: &str) -> u32 {
    let mut hash = 0x811c_9dc5_u32;
    for code_unit in value.encode_utf16() {
        hash ^= u32::from(code_unit);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    hash
}

fn mix(seed: u32, salt: u32) -> u32 {
    let mut value = seed ^ salt.wrapping_mul(0x9e37_79b1);
    value ^= value >> 16;
    value = value.wrapping_mul(0x7feb_352d);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846c_a68b);
    value ^ (value >> 16)
}

fn unit(value: u32) -> f64 {
    f64::from(value) / 4_294_967_296.0
}
