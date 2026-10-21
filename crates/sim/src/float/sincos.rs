//! Pinned V8 fdlibm sine/cosine kernels and argument reduction.
//!
//! Source: Node v22.17.0 `deps/v8/src/base/ieee754.cc:113–710,1354–1383,2450–2478`.
//! The ARM64 binary's polynomial chains, compensated tails and reduction products use
//! fused instructions. Explicit `mul_add` preserves those witnessed rounding points on
//! native and WASM rather than delegating to platform libm. The large reduction below is
//! fdlibm's precision-two path, the sole path used by both scalar functions.
//!
//! Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
//! Developed at SunSoft, a Sun Microsystems, Inc. business.
//! Permission to use, copy, modify, and distribute this software is freely granted,
//! provided that this notice is preserved.
//! Modified significantly by Google Inc. Copyright 2016 the V8 project authors.
//! All rights reserved. See `docs/third-party/v8-fdlibm-LICENSE`.

mod reduction;

use reduction::reduce_pio2;

const SIN_COEFFICIENTS: [f64; 6] = [
    f64::from_bits(0xbfc5_5555_5555_5549),
    f64::from_bits(0x3f81_1111_1110_f8a6),
    f64::from_bits(0xbf2a_01a0_19c1_61d5),
    f64::from_bits(0x3ec7_1de3_57b1_fe7d),
    f64::from_bits(0xbe5a_e5e6_8a2b_9ceb),
    f64::from_bits(0x3de5_d93a_5acf_d57c),
];
const COS_COEFFICIENTS: [f64; 6] = [
    f64::from_bits(0x3fa5_5555_5555_554c),
    f64::from_bits(0xbf56_c16c_16c1_5177),
    f64::from_bits(0x3efa_01a0_19cb_1590),
    f64::from_bits(0xbe92_7e4f_809c_52ad),
    f64::from_bits(0x3e21_ee9e_bdb4_b1c4),
    f64::from_bits(0xbda8_fae9_be88_38d4),
];

/// JavaScript `Math.sin`, retaining the pinned V8 reduction and fused polynomial.
#[must_use]
pub fn sin(value: f64) -> f64 {
    let high = magnitude_high(value);
    if high <= 0x3fe9_21fb {
        return kernel_sin(value, 0.0, false);
    }
    if !value.is_finite() {
        return f64::NAN;
    }
    let (quadrant, [head, tail]) = reduce_pio2(value);
    match quadrant & 3 {
        0 => kernel_sin(head, tail, true),
        1 => kernel_cos(head, tail),
        2 => -kernel_sin(head, tail, true),
        _ => -kernel_cos(head, tail),
    }
}

/// JavaScript `Math.cos`, sharing the exact same argument reduction as `sin`.
#[must_use]
pub fn cos(value: f64) -> f64 {
    let high = magnitude_high(value);
    if high <= 0x3fe9_21fb {
        return kernel_cos(value, 0.0);
    }
    if !value.is_finite() {
        return f64::NAN;
    }
    let (quadrant, [head, tail]) = reduce_pio2(value);
    match quadrant & 3 {
        0 => kernel_cos(head, tail),
        1 => -kernel_sin(head, tail, true),
        2 => -kernel_cos(head, tail),
        _ => kernel_sin(head, tail, true),
    }
}

fn magnitude_high(value: f64) -> u32 {
    ((value.to_bits() >> 32) as u32) & 0x7fff_ffff
}

fn kernel_sin(value: f64, tail: f64, has_tail: bool) -> f64 {
    if magnitude_high(value) < 0x3e40_0000 {
        return value;
    }
    let square = value * value;
    let cube = square * value;
    let mut polynomial = square.mul_add(SIN_COEFFICIENTS[5], SIN_COEFFICIENTS[4]);
    polynomial = square.mul_add(polynomial, SIN_COEFFICIENTS[3]);
    polynomial = square.mul_add(polynomial, SIN_COEFFICIENTS[2]);
    polynomial = square.mul_add(polynomial, SIN_COEFFICIENTS[1]);
    if !has_tail {
        return cube.mul_add(square.mul_add(polynomial, SIN_COEFFICIENTS[0]), value);
    }
    // sin instructions 0x10169e1b4..0x10169e1dc: the cube*polynomial is
    // rounded separately, then both compensated subtraction stages are fused.
    let correction = tail.mul_add(0.5, -cube * polynomial);
    let correction = square.mul_add(correction, -tail);
    let correction = (-cube).mul_add(SIN_COEFFICIENTS[0], correction);
    value - correction
}

fn kernel_cos(value: f64, tail: f64) -> f64 {
    let high = magnitude_high(value);
    if high < 0x3e40_0000 {
        return 1.0;
    }
    let square = value * value;
    let mut polynomial = square.mul_add(COS_COEFFICIENTS[5], COS_COEFFICIENTS[4]);
    polynomial = square.mul_add(polynomial, COS_COEFFICIENTS[3]);
    polynomial = square.mul_add(polynomial, COS_COEFFICIENTS[2]);
    polynomial = square.mul_add(polynomial, COS_COEFFICIENTS[1]);
    polynomial = square.mul_add(polynomial, COS_COEFFICIENTS[0]);
    polynomial *= square;
    let correction = square.mul_add(polynomial, -value * tail);
    if high < 0x3fd3_3333 {
        return 1.0 - square.mul_add(0.5, -correction);
    }
    let quarter = if high > 0x3fe9_0000 {
        0.28125
    } else {
        f64::from_bits(u64::from(high - 0x0020_0000) << 32)
    };
    let half_square = square.mul_add(0.5, -quarter);
    let baseline = 1.0 - quarter;
    baseline + (correction - half_square)
}
