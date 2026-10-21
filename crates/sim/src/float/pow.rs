//! V8's fdlibm power function, used by the native Fire presentation envelope.
//!
//! Source: Node v22.17.0 `deps/v8/src/base/ieee754.cc:2645–2903`. The native
//! platform power differs from the JavaScript witness `0.95 ** 10` by one bit.
//! Keep the source's split logarithm and exponential evaluation order on both
//! native and WASM targets. Explicit fused operations reproduce the ARM64 V8
//! build used by the oracle: compiling the retained source with contraction on
//! matches all 17,431 cases; contraction off differs in four cases.
//! Constants retain their published IEEE words.
//!
//! Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
//! See `docs/third-party/v8-fdlibm-LICENSE` for redistribution terms.

const BP: [f64; 2] = [1.0, 1.5];
const DP_H: [f64; 2] = [0.0, f64::from_bits(0x3fe2_b803_4000_0000)];
const DP_L: [f64; 2] = [0.0, f64::from_bits(0x3e4c_fdeb_43cf_d006)];
const L1: f64 = f64::from_bits(0x3fe3_3333_3333_3303);
const L2: f64 = f64::from_bits(0x3fdb_6db6_db6f_abff);
const L3: f64 = f64::from_bits(0x3fd5_5555_518f_264d);
const L4: f64 = f64::from_bits(0x3fd1_7460_a91d_4101);
const L5: f64 = f64::from_bits(0x3fcd_864a_93c9_db65);
const L6: f64 = f64::from_bits(0x3fca_7e28_4a45_4eef);
const P1: f64 = f64::from_bits(0x3fc5_5555_5555_553e);
const P2: f64 = f64::from_bits(0xbf66_c16c_16be_bd93);
const P3: f64 = f64::from_bits(0x3f11_566a_af25_de2c);
const P4: f64 = f64::from_bits(0xbebb_bd41_c5d2_6bf1);
const P5: f64 = f64::from_bits(0x3e66_3769_72be_a4d0);
const LG2: f64 = f64::from_bits(0x3fe6_2e42_fefa_39ef);
const LG2_H: f64 = f64::from_bits(0x3fe6_2e43_0000_0000);
const LG2_L: f64 = f64::from_bits(0xbe20_5c61_0ca8_6c39);
const OVT: f64 = 8.008_566_259_537_294e-17;
const CP: f64 = f64::from_bits(0x3fee_c709_dc3a_03fd);
const CP_H: f64 = f64::from_bits(0x3fee_c709_e000_0000);
const CP_L: f64 = f64::from_bits(0xbe3e_2fe0_145b_01f5);
const IVLN2: f64 = f64::from_bits(0x3ff7_1547_652b_82fe);
const IVLN2_H: f64 = f64::from_bits(0x3ff7_1547_6000_0000);
const IVLN2_L: f64 = f64::from_bits(0x3e54_ae0b_f85d_df44);

/// JavaScript's `Math.pow` / exponentiation primitive, including signed zero.
#[must_use]
pub fn pow(x: f64, y: f64) -> f64 {
    let hx = high(x);
    let hy = high(y);
    let mut ix = hx & 0x7fff_ffff;
    let iy = hy & 0x7fff_ffff;
    let lx = x.to_bits() as u32;
    let ly = y.to_bits() as u32;
    if iy == 0 && ly == 0 {
        return 1.0;
    }
    if x.is_nan() || y.is_nan() {
        return x + y;
    }

    // Negative bases distinguish odd integers, even integers and nonintegers.
    let mut y_is_integer = 0;
    if hx < 0 {
        if iy >= 0x4340_0000 {
            y_is_integer = 2;
        } else if iy >= 0x3ff0_0000 {
            let exponent = (iy >> 20) - 0x3ff;
            if exponent > 20 {
                let integer = ly >> (52 - exponent);
                if (integer << (52 - exponent)) == ly {
                    y_is_integer = 2 - (integer & 1) as i32;
                }
            } else if ly == 0 {
                let integer = iy >> (20 - exponent);
                if (integer << (20 - exponent)) == iy {
                    y_is_integer = 2 - (integer & 1);
                }
            }
        }
    }
    if ly == 0 {
        if iy == 0x7ff0_0000 {
            if ix == 0x3ff0_0000 && lx == 0 {
                return f64::NAN;
            }
            if ix >= 0x3ff0_0000 {
                return if hy >= 0 { y } else { 0.0 };
            }
            return if hy < 0 { -y } else { 0.0 };
        }
        if iy == 0x3ff0_0000 {
            return if hy < 0 { 1.0 / x } else { x };
        }
        if hy == 0x4000_0000 {
            return x * x;
        }
        if hy == 0x3fe0_0000 && hx >= 0 {
            return x.sqrt();
        }
    }

    let mut ax = x.abs();
    if lx == 0 && matches!(ix, 0 | 0x3ff0_0000 | 0x7ff0_0000) {
        let mut z = if hy < 0 { 1.0 / ax } else { ax };
        if hx < 0 {
            if ix == 0x3ff0_0000 && y_is_integer == 0 {
                z = f64::NAN;
            } else if y_is_integer == 1 {
                z = -z;
            }
        }
        return z;
    }
    if hx < 0 && y_is_integer == 0 {
        return f64::NAN;
    }
    let sign = if hx < 0 && y_is_integer == 1 { -1.0 } else { 1.0 };

    let (t1, t2) = if iy > 0x41e0_0000 {
        // Only bases within 2^-20 of one need the logarithm for huge powers.
        if iy > 0x43f0_0000 {
            if ix <= 0x3fef_ffff {
                return if hy < 0 { f64::INFINITY } else { 0.0 };
            }
            if ix >= 0x3ff0_0000 {
                return if hy > 0 { f64::INFINITY } else { 0.0 };
            }
        }
        if ix < 0x3fef_ffff {
            return sign * if hy < 0 { f64::INFINITY } else { 0.0 };
        }
        if ix > 0x3ff0_0000 {
            return sign * if hy > 0 { f64::INFINITY } else { 0.0 };
        }
        let t = ax - 1.0;
        let w = (t * t) * (-t).mul_add((-t).mul_add(0.25, 1.0 / 3.0), 0.5);
        let u = IVLN2_H * t;
        let v = t.mul_add(IVLN2_L, -(w * IVLN2));
        let t1 = clear_low(u + v);
        (t1, v - (t1 - u))
    } else {
        let mut exponent = 0;
        if ix < 0x0010_0000 {
            ax *= 9_007_199_254_740_992.0;
            exponent -= 53;
            ix = high(ax);
        }
        exponent += (ix >> 20) - 0x3ff;
        let fraction = ix & 0x000f_ffff;
        ix = fraction | 0x3ff0_0000;
        let interval = if fraction <= 0x3988e {
            0
        } else if fraction < 0xbb67a {
            1
        } else {
            exponent += 1;
            ix -= 0x0010_0000;
            0
        };
        ax = with_high(ax, ix);
        let u = ax - BP[interval];
        let v = 1.0 / (ax + BP[interval]);
        let ss = u * v;
        let s_h = clear_low(ss);
        let t_h =
            with_high(0.0, ((ix >> 1) | 0x2000_0000) + 0x0008_0000 + ((interval as i32) << 18));
        let t_l = ax - (t_h - BP[interval]);
        let s_l = v * (-s_h).mul_add(t_l, (-s_h).mul_add(t_h, u));
        let s2 = ss * ss;
        let polynomial =
            s2.mul_add(s2.mul_add(s2.mul_add(s2.mul_add(s2.mul_add(L6, L5), L4), L3), L2), L1);
        let r = s_l.mul_add(s_h + ss, s2 * s2 * polynomial);
        let s2 = s_h * s_h;
        let t_h = clear_low(3.0 + s2 + r);
        let t_l = r - ((t_h - 3.0) - s2);
        let u = s_h * t_h;
        let v = s_l.mul_add(t_h, t_l * ss);
        let p_h = clear_low(u + v);
        let p_l = v - (p_h - u);
        let z_h = CP_H * p_h;
        let z_l = p_h.mul_add(CP_L, p_l * CP) + DP_L[interval];
        let t = f64::from(exponent);
        let t1 = clear_low(((z_h + z_l) + DP_H[interval]) + t);
        (t1, z_l - (((t1 - t) - DP_H[interval]) - z_h))
    };

    let y1 = clear_low(y);
    let p_l = (y - y1).mul_add(t1, y * t2);
    let mut p_h = y1 * t1;
    let z = p_l + p_h;
    let j = high(z);
    let low = z.to_bits() as u32;
    if j >= 0x4090_0000 {
        if j != 0x4090_0000 || low != 0 || p_l + OVT > z - p_h {
            return sign * f64::INFINITY;
        }
    } else if (j & 0x7fff_ffff) >= 0x4090_cc00
        && (j as u32 != 0xc090_cc00 || low != 0 || p_l <= z - p_h)
    {
        return sign * 0.0;
    }

    let magnitude = j & 0x7fff_ffff;
    let mut exponent = (magnitude >> 20) - 0x3ff;
    let mut n = 0;
    if magnitude > 0x3fe0_0000 {
        n = j + (0x0010_0000 >> (exponent + 1));
        exponent = ((n & 0x7fff_ffff) >> 20) - 0x3ff;
        let t = with_high(0.0, n & !(0x000f_ffff >> exponent));
        n = ((n & 0x000f_ffff) | 0x0010_0000) >> (20 - exponent);
        if j < 0 {
            n = -n;
        }
        p_h -= t;
    }
    let t = clear_low(p_l + p_h);
    let u = t * LG2_H;
    let v = (p_l - (t - p_h)).mul_add(LG2, t * LG2_L);
    let z = u + v;
    let w = v - (z - u);
    let t = z * z;
    let t1 = (-t).mul_add(t.mul_add(t.mul_add(t.mul_add(t.mul_add(P5, P4), P3), P2), P1), z);
    let r = (z * t1) / ((t1 - 2.0) - z.mul_add(w, w));
    let z = 1.0 - (r - z);
    let scaled_high = high(z).wrapping_add(n.wrapping_shl(20));
    let result = if scaled_high >> 20 <= 0 {
        // fdlibm scalbn's subnormal path. The exponential above is always a
        // positive normal mantissa; shift through a normal value, rounding once.
        let exponent = (high(z) >> 20) + n;
        if exponent <= -54 {
            0.0
        } else {
            with_high(z, (high(z) & 0x000f_ffff) | ((exponent + 54) << 20))
                * f64::from_bits(0x3c90_0000_0000_0000)
        }
    } else {
        with_high(z, scaled_high)
    };
    sign * result
}

fn high(value: f64) -> i32 {
    (value.to_bits() >> 32) as i32
}

fn with_high(value: f64, high: i32) -> f64 {
    f64::from_bits((u64::from(high as u32) << 32) | (value.to_bits() & 0xffff_ffff))
}

fn clear_low(value: f64) -> f64 {
    f64::from_bits(value.to_bits() & 0xffff_ffff_0000_0000)
}
