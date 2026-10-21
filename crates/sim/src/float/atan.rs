//! V8's fdlibm `atan2` and its private `atan` reduction.
//!
//! Source: Node v22.17.0, `deps/v8/src/base/ieee754.cc:1069–1267`.
//! Identical route-goal operands at Solomon tick 940 produce a different last bit with
//! macOS libm; that heading difference later changes the independently evolved path.
//! The pinned ARM64 Node binary fuses the polynomial's multiply-adds and final subtraction.
//! Explicit `mul_add` calls preserve those witnessed instructions on native and WASM;
//! the source expression alone, evaluated without fusion, differs by one ulp as well.
//! `float_atan2_goldens` covers the exact argument and signed boundary cases.
//!
//! Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
//! Developed at SunSoft, a Sun Microsystems, Inc. business.
//! Permission to use, copy, modify, and distribute this software is freely granted,
//! provided that this notice is preserved.
//! Modified significantly by Google Inc. Copyright 2016 the V8 project authors.
//! All rights reserved. See `docs/third-party/v8-fdlibm-LICENSE` for redistribution terms.

const ATAN_HIGH: [f64; 4] = [
    f64::from_bits(0x3fdd_ac67_0561_bb4f),
    f64::from_bits(0x3fe9_21fb_5444_2d18),
    f64::from_bits(0x3fef_730b_d281_f69b),
    f64::from_bits(0x3ff9_21fb_5444_2d18),
];
const ATAN_LOW: [f64; 4] = [
    f64::from_bits(0x3c7a_2b7f_222f_65e2),
    f64::from_bits(0x3c81_a626_3314_5c07),
    f64::from_bits(0x3c70_0788_7af0_cbbd),
    f64::from_bits(0x3c91_a626_3314_5c07),
];
const COEFFICIENTS: [f64; 11] = [
    f64::from_bits(0x3fd5_5555_5555_550d),
    f64::from_bits(0xbfc9_9999_9998_ebc4),
    f64::from_bits(0x3fc2_4924_9200_83ff),
    f64::from_bits(0xbfbc_71c6_fe23_1671),
    f64::from_bits(0x3fb7_45cd_c54c_206e),
    f64::from_bits(0xbfb3_b0f2_af74_9a6d),
    f64::from_bits(0x3fb1_0d66_a0d0_3d51),
    f64::from_bits(0xbfad_de2d_52de_fd9a),
    f64::from_bits(0x3fa9_7b4b_2476_0deb),
    f64::from_bits(0xbfa2_b444_2c6a_6c2f),
    f64::from_bits(0x3f90_ad3a_e322_da11),
];
const PI: f64 = f64::from_bits(0x4009_21fb_5444_2d18);
const PI_LOW: f64 = f64::from_bits(0x3ca1_a626_3314_5c07);
const HALF_PI: f64 = f64::from_bits(0x3ff9_21fb_5444_2d18);
const QUARTER_PI: f64 = f64::from_bits(0x3fe9_21fb_5444_2d18);
const TINY: f64 = 1.0e-300;

/// JavaScript `Math.atan2(y, x)`, with the pinned V8 argument reduction and polynomial.
#[must_use]
pub fn atan2(y: f64, x: f64) -> f64 {
    if x.is_nan() || y.is_nan() {
        return x + y;
    }
    if x == 1.0 {
        return atan(y);
    }
    let x_high = (x.to_bits() >> 32) as u32;
    let y_high = (y.to_bits() >> 32) as u32;
    let x_magnitude = x_high & 0x7fff_ffff;
    let y_magnitude = y_high & 0x7fff_ffff;
    let mut quadrant = ((y_high >> 31) & 1) | ((x_high >> 30) & 2);
    if y == 0.0 {
        return match quadrant {
            0 | 1 => y,
            2 => PI + TINY,
            _ => -PI - TINY,
        };
    }
    if x == 0.0 {
        return if y.is_sign_negative() { -HALF_PI - TINY } else { HALF_PI + TINY };
    }
    if x.is_infinite() {
        return if y.is_infinite() {
            match quadrant {
                0 => QUARTER_PI + TINY,
                1 => -QUARTER_PI - TINY,
                2 => 3.0 * QUARTER_PI + TINY,
                _ => -3.0 * QUARTER_PI - TINY,
            }
        } else {
            match quadrant {
                0 => 0.0,
                1 => -0.0,
                2 => PI + TINY,
                _ => -PI - TINY,
            }
        };
    }
    if y.is_infinite() {
        return if y.is_sign_negative() { -HALF_PI - TINY } else { HALF_PI + TINY };
    }
    // Exponent comparison avoids overflow or underflow before dividing y by x.
    let exponent_difference = (y_magnitude as i32 - x_magnitude as i32) >> 20;
    let angle = if exponent_difference > 60 {
        quadrant &= 1;
        PI_LOW.mul_add(0.5, HALF_PI)
    } else if x.is_sign_negative() && exponent_difference < -60 {
        0.0
    } else {
        atan((y / x).abs())
    };
    match quadrant {
        0 => angle,
        1 => -angle,
        2 => PI - (angle - PI_LOW),
        _ => (angle - PI_LOW) - PI,
    }
}

fn atan(mut value: f64) -> f64 {
    let negative = value.is_sign_negative();
    let high = ((value.to_bits() >> 32) as u32) & 0x7fff_ffff;
    if high >= 0x4410_0000 {
        if value.is_nan() {
            return value + value;
        }
        return if negative { -ATAN_HIGH[3] - ATAN_LOW[3] } else { ATAN_HIGH[3] + ATAN_LOW[3] };
    }
    let reduction = if high < 0x3fdc_0000 {
        if high < 0x3e40_0000 {
            // V8 also raises the floating-point inexact flag here; JS cannot observe it.
            return value;
        }
        None
    } else {
        value = value.abs();
        if high < 0x3ff3_0000 {
            if high < 0x3fe6_0000 {
                value = value.mul_add(2.0, -1.0) / (2.0 + value);
                Some(0)
            } else {
                value = (value - 1.0) / (value + 1.0);
                Some(1)
            }
        } else if high < 0x4003_8000 {
            value = (value - 1.5) / value.mul_add(1.5, 1.0);
            Some(2)
        } else {
            value = -1.0 / value;
            Some(3)
        }
    };
    let square = value * value;
    let fourth = square * square;
    // These are the two independent fdlibm polynomial chains. The ARM64 oracle
    // uses fmadd for each inner stage and ordinary multiplication for the outer
    // square/fourth factors (atan instructions 0x10169b8ac..0x10169b984).
    let mut odd = fourth.mul_add(COEFFICIENTS[10], COEFFICIENTS[8]);
    odd = fourth.mul_add(odd, COEFFICIENTS[6]);
    odd = fourth.mul_add(odd, COEFFICIENTS[4]);
    odd = fourth.mul_add(odd, COEFFICIENTS[2]);
    odd = fourth.mul_add(odd, COEFFICIENTS[0]);
    odd *= square;
    let mut even = fourth.mul_add(COEFFICIENTS[9], COEFFICIENTS[7]);
    even = fourth.mul_add(even, COEFFICIENTS[5]);
    even = fourth.mul_add(even, COEFFICIENTS[3]);
    even = fourth.mul_add(even, COEFFICIENTS[1]);
    even *= fourth;
    let Some(index) = reduction else {
        // fmsub at 0x10169b994 rounds the product and subtraction only once.
        return (-value).mul_add(odd + even, value);
    };
    // fnmsub at 0x10169b9bc fuses only the product minus the low part; both
    // following subtractions remain separate operations in the oracle.
    let low_difference = value.mul_add(odd + even, -ATAN_LOW[index]);
    let angle = ATAN_HIGH[index] - (low_difference - value);
    if negative { -angle } else { angle }
}
