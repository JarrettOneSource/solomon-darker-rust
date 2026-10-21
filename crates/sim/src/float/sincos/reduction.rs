//! fdlibm's split pi/2 subtraction and 24-bit Payne–Hanek reduction.

const TWO_OVER_PI: [i32; 66] = [
    0xa2f983, 0x6e4e44, 0x1529fc, 0x2757d1, 0xf534dd, 0xc0db62, 0x95993c, 0x439041, 0xfe5163,
    0xabdebb, 0xc561b7, 0x246e3a, 0x424dd2, 0xe00649, 0x2eea09, 0xd1921c, 0xfe1deb, 0x1cb129,
    0xa73ee8, 0x8235f5, 0x2ebb44, 0x84e99c, 0x7026b4, 0x5f7e41, 0x3991d6, 0x398353, 0x39f49c,
    0x845f8b, 0xbdf928, 0x3b1ff8, 0x97ffde, 0x05980f, 0xef2f11, 0x8b5a0a, 0x6d1f6d, 0x367ecf,
    0x27cb09, 0xb74f46, 0x3f669e, 0x5fea2d, 0x7527ba, 0xc7ebe5, 0xf17b3d, 0x0739f7, 0x8a5292,
    0xea6bfb, 0x5fb11f, 0x8d5d08, 0x560330, 0x46fc7b, 0x6babf0, 0xcfbc20, 0x9af436, 0x1da9e3,
    0x91615e, 0xe61b08, 0x659985, 0x5f14a0, 0x68408d, 0xffd880, 0x4d7327, 0x310606, 0x1556ca,
    0x73a8c9, 0x60e27b, 0xc08c6b,
];
const MULTIPLE_HIGH: [u32; 32] = [
    0x3ff921fb, 0x400921fb, 0x4012d97c, 0x401921fb, 0x401f6a7a, 0x4022d97c, 0x4025fdbb, 0x402921fb,
    0x402c463a, 0x402f6a7a, 0x4031475c, 0x4032d97c, 0x40346b9c, 0x4035fdbb, 0x40378fdb, 0x403921fb,
    0x403ab41b, 0x403c463a, 0x403dd85a, 0x403f6a7a, 0x40407e4c, 0x4041475c, 0x4042106c, 0x4042d97c,
    0x4043a28c, 0x40446b9c, 0x404534ac, 0x4045fdbb, 0x4046c6cb, 0x40478fdb, 0x404858eb, 0x404921fb,
];
const PI_OVER_TWO_CHUNKS: [f64; 5] = [
    f64::from_bits(0x3ff9_21fb_4000_0000),
    f64::from_bits(0x3e74_442d_0000_0000),
    f64::from_bits(0x3cf8_4698_8000_0000),
    f64::from_bits(0x3b78_cc51_6000_0000),
    f64::from_bits(0x39f0_1b83_8000_0000),
];
const INVERSE_PIO2: f64 = f64::from_bits(0x3fe4_5f30_6dc9_c883);
const PIO2_1: f64 = f64::from_bits(0x3ff9_21fb_5440_0000);
const PIO2_1_TAIL: f64 = f64::from_bits(0x3dd0_b461_1a62_6331);
const PIO2_2: f64 = f64::from_bits(0x3dd0_b461_1a60_0000);
const PIO2_2_TAIL: f64 = f64::from_bits(0x3ba3_198a_2e03_7073);
const PIO2_3: f64 = f64::from_bits(0x3ba3_198a_2e00_0000);
const PIO2_3_TAIL: f64 = f64::from_bits(0x397b_839a_2520_49c1);
const TWO24: f64 = 16_777_216.0;
const INVERSE_TWO24: f64 = 1.0 / TWO24;

pub(super) fn reduce_pio2(value: f64) -> (i32, [f64; 2]) {
    let high = super::magnitude_high(value);
    if high <= 0x3fe9_21fb {
        return (0, [value, 0.0]);
    }
    if high < 0x4002_d97c {
        let sign = if value.is_sign_negative() { -1.0 } else { 1.0 };
        let mut remainder = value - sign * PIO2_1;
        let tail = if high == 0x3ff9_21fb {
            remainder -= sign * PIO2_2;
            sign * PIO2_2_TAIL
        } else {
            sign * PIO2_1_TAIL
        };
        let head = remainder - tail;
        return (sign as i32, [head, (remainder - head) - tail]);
    }
    if high <= 0x4139_21fb {
        return reduce_medium(value, high);
    }
    // Public sin/cos remove nonfinite inputs before reaching this private reducer.
    let exponent = (high >> 20) as i32 - 1046;
    let scaled_high = high.wrapping_sub((exponent as u32).wrapping_shl(20));
    let mut scaled =
        f64::from_bits((u64::from(scaled_high) << 32) | (value.to_bits() & 0xffff_ffff));
    let mut chunks = [0.0; 3];
    for chunk in &mut chunks[..2] {
        *chunk = f64::from(scaled as i32);
        scaled = (scaled - *chunk) * TWO24;
    }
    chunks[2] = scaled;
    let mut count = 3;
    while chunks[count - 1] == 0.0 {
        count -= 1;
    }
    let (quadrant, [head, tail]) = reduce_large(&chunks[..count], exponent);
    if value.is_sign_negative() { (-quadrant, [-head, -tail]) } else { (quadrant, [head, tail]) }
}

fn reduce_medium(value: f64, high: u32) -> (i32, [f64; 2]) {
    let magnitude = value.abs();
    // The pinned reducer uses fmadd at 0x10169c364 and 0x10169c380.
    let count = magnitude.mul_add(INVERSE_PIO2, 0.5) as i32;
    let multiple = f64::from(count);
    let mut remainder = multiple.mul_add(-PIO2_1, magnitude);
    let mut tail = multiple * PIO2_1_TAIL;
    let mut head = remainder - tail;
    if count >= 32 || high == MULTIPLE_HIGH[count as usize - 1] {
        let exponent = (high >> 20) as i32;
        let lost_bits = exponent - ((head.to_bits() >> 52) & 0x7ff) as i32;
        if lost_bits > 16 {
            let previous = remainder;
            tail = multiple * PIO2_2;
            remainder = previous - tail;
            tail = multiple.mul_add(PIO2_2_TAIL, -((previous - remainder) - tail));
            head = remainder - tail;
            let lost_bits = exponent - ((head.to_bits() >> 52) & 0x7ff) as i32;
            if lost_bits > 49 {
                let previous = remainder;
                tail = multiple * PIO2_3;
                remainder = previous - tail;
                tail = multiple.mul_add(PIO2_3_TAIL, -((previous - remainder) - tail));
                head = remainder - tail;
            }
        }
    }
    let tail = (remainder - head) - tail;
    if value.is_sign_negative() { (-count, [-head, -tail]) } else { (count, [head, tail]) }
}

/// Source `__kernel_rem_pio2` with prec=2. Each limb contains 24 significant bits.
/// The fixed arrays are the source's workspace, bounded for IEEE binary64 inputs.
fn reduce_large(chunks: &[f64], exponent: i32) -> (i32, [f64; 2]) {
    const INITIAL_TERMS: usize = 4;
    let last_chunk = chunks.len() - 1;
    let table_start = ((exponent - 3) / 24).max(0) as usize;
    let mut leading_exponent = exponent - 24 * (table_start as i32 + 1);
    let mut factors = [0.0; 20];
    let mut products = [0.0; 20];
    let mut integer_limbs = [0_i32; 20];
    let mut compressed = [0.0; 20];
    for (index, factor) in factors.iter_mut().enumerate().take(last_chunk + INITIAL_TERMS + 1) {
        let table_index = table_start as i32 - last_chunk as i32 + index as i32;
        if table_index >= 0 {
            *factor = f64::from(TWO_OVER_PI[table_index as usize]);
        }
    }
    for (index, product) in products.iter_mut().enumerate().take(INITIAL_TERMS + 1) {
        for (chunk_index, chunk) in chunks.iter().enumerate() {
            *product = chunk.mul_add(factors[last_chunk + index - chunk_index], *product);
        }
    }
    let mut last = INITIAL_TERMS;
    let (count, complement, mut fractional) = loop {
        let mut carry = products[last];
        for (limb, index) in (1..=last).rev().enumerate() {
            let upper = f64::from((INVERSE_TWO24 * carry) as i32);
            integer_limbs[limb] = upper.mul_add(-TWO24, carry) as i32;
            carry = products[index - 1] + upper;
        }
        let mut fraction = scale_pow2(carry, leading_exponent);
        fraction = (fraction * 0.125).floor().mul_add(-8.0, fraction);
        let mut count = fraction as i32;
        fraction -= f64::from(count);
        let mut complement = 0;
        if leading_exponent > 0 {
            let upper = integer_limbs[last - 1] >> (24 - leading_exponent);
            count += upper;
            integer_limbs[last - 1] -= upper << (24 - leading_exponent);
            complement = integer_limbs[last - 1] >> (23 - leading_exponent);
        } else if leading_exponent == 0 {
            complement = integer_limbs[last - 1] >> 23;
        } else if fraction >= 0.5 {
            complement = 2;
        }
        if complement > 0 {
            count += 1;
            let mut borrow = false;
            for limb in &mut integer_limbs[..last] {
                if borrow {
                    *limb = 0xff_ffff - *limb;
                } else if *limb != 0 {
                    borrow = true;
                    *limb = 0x100_0000 - *limb;
                }
            }
            if leading_exponent == 1 {
                integer_limbs[last - 1] &= 0x7f_ffff;
            } else if leading_exponent == 2 {
                integer_limbs[last - 1] &= 0x3f_ffff;
            }
            if complement == 2 {
                fraction = 1.0 - fraction;
                if borrow {
                    fraction -= scale_pow2(1.0, leading_exponent);
                }
            }
        }
        if fraction == 0.0 && integer_limbs[INITIAL_TERMS..last].iter().all(|limb| *limb == 0) {
            // Cancellation can require more 2/pi limbs. Recompute only the added
            // products, exactly as fdlibm's recompute label does.
            let mut additional = 1;
            while additional <= INITIAL_TERMS && integer_limbs[INITIAL_TERMS - additional] == 0 {
                additional += 1;
            }
            for index in last + 1..=last + additional {
                factors[last_chunk + index] = f64::from(TWO_OVER_PI[table_start + index]);
                products[index] = 0.0;
                for (chunk_index, chunk) in chunks.iter().enumerate() {
                    products[index] =
                        chunk.mul_add(factors[last_chunk + index - chunk_index], products[index]);
                }
            }
            last += additional;
            continue;
        }
        break (count, complement, fraction);
    };
    if fractional == 0.0 {
        last -= 1;
        leading_exponent -= 24;
        while integer_limbs[last] == 0 {
            last -= 1;
            leading_exponent -= 24;
        }
    } else {
        fractional = scale_pow2(fractional, -leading_exponent);
        if fractional >= TWO24 {
            let upper = f64::from((INVERSE_TWO24 * fractional) as i32);
            integer_limbs[last] = upper.mul_add(-TWO24, fractional) as i32;
            last += 1;
            leading_exponent += 24;
            integer_limbs[last] = upper as i32;
        } else {
            integer_limbs[last] = fractional as i32;
        }
    }
    let mut scale = scale_pow2(1.0, leading_exponent);
    for index in (0..=last).rev() {
        products[index] = scale * f64::from(integer_limbs[index]);
        scale *= INVERSE_TWO24;
    }
    for index in (0..=last).rev() {
        let mut sum = 0.0;
        for coefficient in 0..=INITIAL_TERMS.min(last - index) {
            sum = products[index + coefficient].mul_add(PI_OVER_TWO_CHUNKS[coefficient], sum);
        }
        compressed[last - index] = sum;
    }
    let mut head = 0.0;
    for index in (0..=last).rev() {
        head += compressed[index];
    }
    let mut tail = compressed[0] - head;
    for chunk in &compressed[1..=last] {
        tail += chunk;
    }
    if complement == 0 { (count & 7, [head, tail]) } else { (count & 7, [-head, -tail]) }
}

fn scale_pow2(value: f64, exponent: i32) -> f64 {
    // Reduction exponents remain normal binary64 powers of two, even for the
    // largest finite input: its large exponent has already selected table_start.
    value * f64::from_bits(((exponent + 1023) as u64) << 52)
}
