//! Native random reductions applied to a caller-supplied word.
//!
//! Stock draws these values from its one shared generator. Replicated presentation cannot
//! share that draw stream, so the reference hashes a stable semantic word instead and keeps
//! the native integer reduction, endpoint domain, and 32-bit float stores exact. Reference:
//! `Website/frontend/src/game/core-kernels/native-random-domain.ts`.

use crate::float::fround;
use crate::rng::MASK;

pub const NATIVE_RANDOM_FLOAT_DENOMINATOR: u32 = 100_000;
/// Unit floats are drawn as integers in `0..=100_000`, so both endpoints are reachable.
pub const NATIVE_RANDOM_FLOAT_INTEGER_BOUND: u32 = NATIVE_RANDOM_FLOAT_DENOMINATOR + 1;
const WORD_SHIFT: u32 = 6;

/// The native integer reduction of `word` into `0..exclusive_bound`.
///
/// The word is masked to the generator's 30 bits, dropped by six, masked to the smallest
/// power of two that covers the bound, and reduced modulo the bound.
#[must_use]
pub fn native_random_int_from_word(word: u32, exclusive_bound: u32) -> u32 {
    let bound = u64::from(exclusive_bound);
    let mut reduction_width: u64 = 2;
    while reduction_width < bound {
        reduction_width *= 2;
    }
    let native_word = u64::from((word & MASK) >> WORD_SHIFT);
    ((native_word & (reduction_width - 1)) % bound) as u32
}

/// A float in `[0, maximum]` drawn the native way: an integer sample scaled through two
/// single-precision stores.
#[must_use]
pub fn native_random_float_from_word(word: u32, maximum: f64) -> f64 {
    let integer_sample =
        fround(f64::from(native_random_int_from_word(word, NATIVE_RANDOM_FLOAT_INTEGER_BOUND)));
    let unit_sample = fround(integer_sample / f64::from(NATIVE_RANDOM_FLOAT_DENOMINATOR));
    fround(unit_sample * fround(maximum))
}

/// `nativeSignedRandomFloatFromSemanticWords`: magnitude and sign own separate semantic words.
#[must_use]
pub fn native_signed_random_float_from_words(
    magnitude_word: u32,
    sign_word: u32,
    maximum: f64,
) -> f64 {
    let magnitude = native_random_float_from_word(magnitude_word, maximum);
    if native_random_int_from_word(sign_word, 2) == 1 { fround(-magnitude) } else { magnitude }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integers_use_the_bits_above_the_shift_and_cover_the_bound() {
        assert_eq!(native_random_int_from_word(0, 2), 0);
        assert_eq!(native_random_int_from_word(1 << WORD_SHIFT, 2), 1);
        assert_eq!(native_random_int_from_word(0b11 << WORD_SHIFT, 2), 1);
        assert_eq!(native_random_int_from_word(5 << WORD_SHIFT, 3), 1);
        let samples = (0..4096u32).map(|word| native_random_int_from_word(word << WORD_SHIFT, 7));
        assert!(samples.clone().all(|sample| sample < 7));
        assert!(samples.max() == Some(6));
    }

    #[test]
    fn floats_reach_both_endpoints_of_the_native_domain() {
        assert_eq!(native_random_float_from_word(0, 0.5), 0.0);
        let endpoint = NATIVE_RANDOM_FLOAT_DENOMINATOR << WORD_SHIFT;
        assert_eq!(native_random_float_from_word(endpoint, 1.0), 1.0);
        assert_eq!(native_random_float_from_word(endpoint, 0.1), fround(0.1));
    }
}
