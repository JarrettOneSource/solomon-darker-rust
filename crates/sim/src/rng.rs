//! The native 55-word lagged-Fibonacci generator.
//!
//! Native addresses: constructor `0x00401110`, unit float `0x004011F0`, scaled float
//! `0x00401310`. The state is 232 bytes: two ring indices, 55 words, and the float divisor at
//! `+0xE4`. Every draw is reproduced bit-exactly by `tests/native_goldens.rs`.

use crate::float::fround;

pub const MASK: u32 = 0x3fff_ffff;
pub const WORD_COUNT: usize = 55;
pub const INITIAL_INDEX_A: usize = 0;
pub const INITIAL_INDEX_B: usize = 31;
pub const STOCK_DIVISOR: u32 = 100_000;
pub const SEED_TICK_MULTIPLIER: u32 = 0x0ef3;

/// The Website's FNV-1a over UTF-16 code units, used for separately owned gate, encounter
/// and wave random streams. Zero uses the same nonzero escape seed as those source owners.
#[must_use]
pub fn seed_from_text(seed: &str) -> u32 {
    let mut state = 0x811c_9dc5_u32;
    for code_unit in seed.encode_utf16() {
        state ^= u32::from(code_unit);
        state = state.wrapping_mul(0x0100_0193);
    }
    if state == 0 { 0x6d2b_79f5 } else { state }
}

/// The Website's private actor/gate/wave xorshift stream (`nextBoneyardWaveRandom`).
/// Its zero escape is also the returned word, unlike the courtyard ambient sampler.
pub fn next_xorshift_word(state: &mut u32) -> u32 {
    let mut next = *state;
    next ^= next << 13;
    next ^= next >> 17;
    next ^= next << 5;
    *state = if next == 0 { 0x6d2b_79f5 } else { next };
    *state
}

pub fn next_xorshift_random(state: &mut u32) -> f64 {
    f64::from(next_xorshift_word(state)) / 4_294_967_296.0
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(try_from = "saved::NativeRngDocument"))]
pub struct NativeRng {
    pub index_a: usize,
    pub index_b: usize,
    #[cfg_attr(feature = "persistence", serde(with = "crate::persistence::fixed_array"))]
    pub words: [u32; WORD_COUNT],
    pub divisor: u32,
}

#[cfg(feature = "persistence")]
mod saved {
    use super::{MASK, NativeRng, WORD_COUNT};

    // Deserialization must establish the ring invariants before any kernel indexes it.
    #[derive(serde::Deserialize)]
    pub(super) struct NativeRngDocument {
        index_a: usize,
        index_b: usize,
        #[serde(with = "crate::persistence::fixed_array")]
        words: [u32; WORD_COUNT],
        divisor: u32,
    }

    impl TryFrom<NativeRngDocument> for NativeRng {
        type Error = &'static str;

        fn try_from(value: NativeRngDocument) -> Result<Self, Self::Error> {
            if value.index_a >= WORD_COUNT || value.index_b >= WORD_COUNT {
                return Err("random ring index is outside its 55 words");
            }
            if value.index_b != (value.index_a + 31) % WORD_COUNT {
                return Err("random ring cursors have lost their native spacing");
            }
            if value.divisor == 0 || value.divisor >= i32::MAX as u32 {
                return Err("random float divisor is outside its native range");
            }
            if value.words.iter().any(|word| *word > MASK) {
                return Err("random ring contains a word outside its 30-bit range");
            }
            Ok(Self {
                index_a: value.index_a,
                index_b: value.index_b,
                words: value.words,
                divisor: value.divisor,
            })
        }
    }
}

impl NativeRng {
    /// `drawNativeUnitVector` preserves the native float32 angle stores before trig.
    pub fn draw_unit_vector(&mut self) -> crate::Vec2 {
        let angle = self.draw_scaled_float(360.0, false);
        let radians =
            crate::float::fround(crate::float::fround(std::f64::consts::PI) * angle / 180.0);
        crate::Vec2::new(
            crate::float::fround(crate::float::sin(radians)),
            crate::float::fround(-crate::float::cos(radians)),
        )
    }

    /// Seeds the ladder: `words[0] = seed & MASK`, `words[1] = 1`, then pairwise sums.
    ///
    /// Like the reference, the seed is reduced to its low 32 bits before masking.
    #[must_use]
    pub fn new(seed: i64) -> Self {
        let mut words = [0u32; WORD_COUNT];
        words[0] = (seed as u32) & MASK;
        words[1] = 1;
        for index in 2..WORD_COUNT {
            words[index] = words[index - 1].wrapping_add(words[index - 2]) & MASK;
        }
        Self { index_a: INITIAL_INDEX_A, index_b: INITIAL_INDEX_B, words, divisor: STOCK_DIVISOR }
    }

    /// The seed the retail game derives from its elapsed application tick counter.
    #[must_use]
    pub fn seed_from_elapsed_ticks(elapsed_app_ticks: u32) -> i64 {
        i64::from(elapsed_app_ticks.wrapping_mul(SEED_TICK_MULTIPLIER) as i32)
    }

    /// Advances the ring once and returns the new word.
    pub fn draw_word(&mut self) -> u32 {
        let word = self.words[self.index_a].wrapping_add(self.words[self.index_b]) & MASK;
        self.words[self.index_a] = word;
        self.index_a = (self.index_a + 1) % WORD_COUNT;
        self.index_b = (self.index_b + 1) % WORD_COUNT;
        word
    }

    fn positive_integer(&mut self, bound: u32) -> u32 {
        let mut power_of_two: u64 = 2;
        while power_of_two < u64::from(bound) {
            power_of_two *= 2;
        }
        let word = self.draw_word();
        ((word >> 6) & ((power_of_two - 1) as u32)) % bound
    }

    /// An integer in `[0, bound)`. A zero bound draws nothing; a negative bound draws a
    /// magnitude below `-bound` and then a sign word.
    pub fn draw_integer(&mut self, bound: i32) -> i32 {
        if bound == 0 {
            return 0;
        }
        if bound > 0 {
            return self.positive_integer(bound as u32) as i32;
        }
        let magnitude = self.positive_integer(bound.unsigned_abs()) as i32;
        if self.positive_integer(2) == 0 { magnitude } else { -magnitude }
    }

    fn unit_magnitude(&mut self) -> f64 {
        let integer = self.draw_integer(self.divisor as i32 + 1);
        fround(fround(f64::from(integer)) / f64::from(self.divisor))
    }

    fn apply_sign(&mut self, value: f64) -> f64 {
        if self.draw_integer(2) == 0 { value } else { -value }
    }

    /// Native `0x004011F0`: a float in `[0, 1]` with two float32 rounding points. A signed
    /// draw costs a second word.
    pub fn draw_unit_float(&mut self, signed: bool) -> f64 {
        let magnitude = self.unit_magnitude();
        if signed { self.apply_sign(magnitude) } else { magnitude }
    }

    /// Native `0x00401310`: a unit draw scaled by `magnitude`, three float32 rounding points.
    pub fn draw_scaled_float(&mut self, magnitude: f64, signed: bool) -> f64 {
        let scaled = fround(self.unit_magnitude() * fround(magnitude));
        if signed { self.apply_sign(scaled) } else { scaled }
    }

    /// Native `0x004012C0` (`drawNativeSign`): zero selects negative, the opposite
    /// polarity from the optional sign word of `drawNativeFloat`.
    pub fn draw_sign(&mut self, magnitude: f64) -> f64 {
        if self.draw_integer(2) == 0 { -magnitude } else { magnitude }
    }

    /// Native `0x00448480`: ordered-endpoint interpolation. Equal float32 endpoints
    /// return immediately without consuming a random word.
    pub fn draw_float_range(&mut self, first: f64, second: f64) -> f64 {
        let first = fround(first);
        let second = fround(second);
        if first == second {
            return first;
        }
        fround(first + self.draw_scaled_float(fround(second - first), false))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_xorshift_streams_return_the_nonzero_escape_word() {
        let mut state = 0;
        assert_eq!(next_xorshift_word(&mut state), 0x6d2b_79f5);
        assert_eq!(state, 0x6d2b_79f5);
        state = 1;
        assert_eq!(next_xorshift_word(&mut state), 270_369);
    }

    #[test]
    fn seed_ladder_matches_the_documented_fibonacci_prefix() {
        let rng = NativeRng::new(1);
        assert_eq!(&rng.words[..8], &[1, 1, 2, 3, 5, 8, 13, 21]);
        assert_eq!(rng.index_a, 0);
        assert_eq!(rng.index_b, 31);
    }

    #[test]
    fn zero_bound_consumes_no_words() {
        let mut rng = NativeRng::new(7);
        let before = rng.clone();
        assert_eq!(rng.draw_integer(0), 0);
        assert_eq!(rng, before);
    }

    #[test]
    fn negative_bound_draws_magnitude_then_sign() {
        let mut rng = NativeRng::new(7);
        let mut mirror = rng.clone();
        let value = rng.draw_integer(-16);
        let magnitude = mirror.draw_integer(16);
        let sign = mirror.draw_integer(2);
        assert_eq!(value, if sign == 0 { magnitude } else { -magnitude });
        assert_eq!(rng, mirror);
    }
}
