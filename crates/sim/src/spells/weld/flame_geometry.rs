//! Current Website `native-weld-flame-ribbon.ts` (fa08ae24): retained record-25
//! Flame factory and ribbon. Every named native float store remains explicit.
//! This spline has its own stored tangents; the general natural spline does not.

use crate::Vec2;
use crate::float::{fround, sin};

pub const NATIVE_FLAME_MAXIMUM_PATH_POINTS: usize = 1_024;
pub const NATIVE_FLAME_MAXIMUM_RIBBON_PAIRS: usize = 4_096;

// Recovered Math+4 bytes da 0f 49 40, rather than fround(Math.PI).
const NATIVE_PI: f64 = 3.141592502593994;
const NORMAL_DELTA: f64 = 0.001_f32 as f64;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeFlameLashFactoryInput {
    pub start: Vec2,
    pub control: Vec2,
    pub end: Vec2,
    pub offset: Vec2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeFlameLashGeometryKind {
    Short,
    Long,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeFlameLashBounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeFlameLashFactoryGeometry {
    pub kind: NativeFlameLashGeometryKind,
    pub start: Vec2,
    pub control: Vec2,
    pub extended_end: Vec2,
    pub body_end: Vec2,
    pub span: f64,
    pub split_bounds: NativeFlameLashBounds,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeFlameLashRibbonInput {
    pub geometry: NativeFlameLashFactoryInput,
    pub width: f64,
    pub weak: bool,
    pub enhanced_effects: bool,
    pub phase_degrees: f64,
    pub short_width_factor: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeFlameLashRibbon {
    pub kind: NativeFlameLashGeometryKind,
    pub vertices: Vec<Vec2>,
    pub indices: Vec<u16>,
    /// Absolute UVs on the original 2048-square BadGuys atlas.
    pub uvs: Vec<Vec2>,
    /// Native ARGB, including the long ribbon's transparent caps.
    pub colors: Vec<u32>,
    pub parameters: Vec<f64>,
}

/// Integer multiplication wraps before the native phase is stored as a float.
#[must_use]
pub fn native_flame_lash_birth_phase(birth_tick: u64) -> f64 {
    fround((birth_tick as i32).wrapping_mul(-3) as f64)
}

pub fn native_flame_lash_factory_geometry(
    input: NativeFlameLashFactoryInput,
) -> Result<NativeFlameLashFactoryGeometry, String> {
    let start = stored_point(input.start)?;
    let control = stored_point(input.control)?;
    let end = stored_point(input.end)?;
    let direction = normalized(difference(end, control));
    let extended_end = added(end, scaled(direction, 15.0));
    let body_end = added(extended_end, stored_point(input.offset)?);
    let span = length(difference(body_end, start));
    // Native Rect(S.x,S.y,1,1), then union M and extended E and pad 100.
    // The body offset and displaced mesh extrema do not enlarge this owner.
    let left = start.x.min(control.x).min(extended_end.x);
    let top = start.y.min(control.y).min(extended_end.y);
    let right = fround(start.x + 1.0).max(control.x).max(extended_end.x);
    let bottom = fround(start.y + 1.0).max(control.y).max(extended_end.y);
    Ok(NativeFlameLashFactoryGeometry {
        kind: if span < 75.0 {
            NativeFlameLashGeometryKind::Short
        } else {
            NativeFlameLashGeometryKind::Long
        },
        start,
        control,
        extended_end,
        body_end,
        span,
        split_bounds: NativeFlameLashBounds {
            x: fround(left - 100.0),
            y: fround(top - 100.0),
            width: fround(fround(right - left) + 200.0),
            height: fround(fround(bottom - top) + 200.0),
        },
    })
}

/// Birth-only membership. Each admitted point consumes its own radius word.
/// The control point can occur twice; Source deliberately does not deduplicate it.
pub fn native_flame_lash_path_points(
    geometry: &NativeFlameLashFactoryGeometry,
    maximum_points: usize,
) -> Result<Vec<Vec2>, String> {
    require_budget(maximum_points)?;
    let mut points = Vec::new();
    let mut consider = |candidate: Vec2| -> Result<(), String> {
        let delta = difference(candidate, geometry.start);
        if fround(delta.x * delta.x + delta.y * delta.y) >= 48_400.0 {
            if points.len() >= maximum_points {
                return Err("Flame path exceeds its point budget".into());
            }
            points.push(Vec2::new(candidate.x, fround(candidate.y + 35.0)));
        }
        Ok(())
    };
    for (start, end) in
        [(geometry.start, geometry.control), (geometry.control, geometry.extended_end)]
    {
        let delta = difference(end, start);
        let mut remaining = length(delta);
        if !remaining.is_finite() {
            return Err("Invalid Flame path length".into());
        }
        let step = scaled(normalized(delta), 100.0);
        let mut current = start;
        while remaining > 50.0 {
            consider(current)?;
            let next = added(current, step);
            let next_remaining = fround(remaining - 100.0);
            if next_remaining == remaining || next == current {
                return Err("Flame path cannot advance at float32 precision".into());
            }
            current = next;
            remaining = next_remaining;
        }
        consider(end)?;
    }
    Ok(points)
}

pub fn native_flame_lash_ribbon_pair_count(
    geometry: &NativeFlameLashFactoryGeometry,
    enhanced_effects: bool,
    maximum_pairs: usize,
) -> Result<usize, String> {
    require_budget(maximum_pairs)?;
    if geometry.kind == NativeFlameLashGeometryKind::Short {
        if maximum_pairs < 3 {
            return Err("Flame ribbon exceeds its pair budget".into());
        }
        return Ok(3);
    }
    Ok(parameter_samples(
        geometry.start,
        geometry.control,
        if enhanced_effects { 7.0 } else { 30.0 },
        maximum_pairs,
    )?
    .len())
}

#[must_use]
pub fn native_flame_lash_spline_point(points: [Vec2; 3], parameter: f64) -> Vec2 {
    spline_point(&spline_axes(points), fround(parameter))
}

#[must_use]
pub fn native_flame_lash_spline_normal(points: [Vec2; 3], parameter: f64) -> Vec2 {
    spline_normal(&spline_axes(points), fround(parameter))
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeFlameLashPrivateSample {
    pub angle_degrees: f64,
    pub radius: f64,
    pub next_state: i32,
}

/// Private builder stream. Signed remainders and INT_MIN survive the abs step.
#[must_use]
pub fn native_flame_lash_private_sample(state: i32) -> NativeFlameLashPrivateSample {
    let angle_magnitude = fround((state % 360_000) as f64 / 360_000.0 * 65.0);
    let sign_state = mix32(state).wrapping_abs();
    let radius_state = mix32(sign_state).wrapping_abs();
    NativeFlameLashPrivateSample {
        angle_degrees: if sign_state % 2 == 1 { fround(-angle_magnitude) } else { angle_magnitude },
        radius: fround((radius_state % 360_000) as f64 / 360_000.0 * 5.0),
        next_state: mix32(radius_state).wrapping_abs(),
    }
}

pub fn build_native_flame_lash_ribbon(
    input: NativeFlameLashRibbonInput,
) -> Result<NativeFlameLashRibbon, String> {
    let geometry = native_flame_lash_factory_geometry(input.geometry)?;
    let short = geometry.kind == NativeFlameLashGeometryKind::Short;
    if short && input.short_width_factor.is_none_or(|factor| !factor.is_finite()) {
        return Err("Short Flame geometry requires its retained birth width factor".into());
    }
    let parameters = if short {
        vec![0.0, 1.0, 2.0]
    } else {
        parameter_samples(
            geometry.start,
            geometry.control,
            if input.enhanced_effects { 7.0 } else { 30.0 },
            NATIVE_FLAME_MAXIMUM_RIBBON_PAIRS,
        )?
    };
    let pair_count = parameters.len();
    if pair_count * 2 > 65_536 {
        return Err("Flame ribbon exceeds native uint16 vertex indices".into());
    }
    let mut ribbon = NativeFlameLashRibbon {
        kind: geometry.kind,
        vertices: vec![Vec2::ZERO; pair_count * 2],
        indices: Vec::with_capacity((pair_count - 1) * 6),
        uvs: vec![Vec2::ZERO; pair_count * 2],
        colors: vec![0; pair_count * 2],
        parameters,
    };
    let body_color = if input.weak { 0x7fff_ffff } else { 0xffff_ffff };
    let width = fround(input.width);
    if short {
        let delta = difference(geometry.body_end, geometry.start);
        let normal = normalized(Vec2::new(delta.y, fround(-delta.x)));
        let half_width = fround(
            fround(fround(width * 65.0) * 0.5)
                * fround(input.short_width_factor.expect("short width was admitted")),
        );
        let middle = scaled(added(geometry.start, geometry.body_end), 0.5);
        for (pair, center) in [geometry.start, middle, geometry.body_end].into_iter().enumerate() {
            write_pair(&mut ribbon, pair, center, normal, half_width, None);
            write_style(&mut ribbon, pair, body_color, pair == 1);
        }
    } else {
        let axes = spline_axes([geometry.start, geometry.control, geometry.body_end]);
        let phase = fround(input.phase_degrees);
        // Source trunc(phase)|0 wraps; f32 rounding can reach +2^31.
        let mut private_state = phase.trunc().rem_euclid(4_294_967_296.0) as u32 as i32;
        let base_half_width = fround(fround(width * 100.0) * 0.5);
        let amplitude =
            if geometry.span < 250.0 { fround(geometry.span / 250.0 * 16.0) } else { 16.0 };
        for pair in 0..pair_count {
            let parameter = ribbon.parameters[pair];
            let normal = spline_normal(&axes, parameter);
            let first = pair == 0;
            let last = pair == pair_count - 1;
            if first || last {
                let center = if first { geometry.start } else { geometry.body_end };
                let shift = first.then(|| scaled(Vec2::new(normal.y, fround(-normal.x)), 25.0));
                write_pair(&mut ribbon, pair, center, normal, base_half_width, shift);
                write_style(&mut ribbon, pair, 0, false);
                continue;
            }
            // Separate native vector stores, rather than a combined displacement.
            let envelope = sine_degrees(fround(parameter / 2.0 * 180.0));
            let mut center = spline_point(&axes, parameter);
            let primary_sine = sine_degrees(fround(parameter * 360.0 + phase));
            center =
                added(center, scaled(scaled(scaled(normal, primary_sine), amplitude), envelope));
            let secondary_sine = sine_degrees(fround(phase * 2.5 - parameter * 90.0));
            center = added(center, scaled(scaled(scaled(normal, secondary_sine), 8.0), envelope));
            let sample = native_flame_lash_private_sample(private_state);
            private_state = sample.next_state;
            let jitter =
                scaled(scaled(direction_degrees(sample.angle_degrees), sample.radius), envelope);
            center = added(center, jitter);
            let half_width = fround(((1.0 - envelope) * 0.75 + 0.5) * base_half_width);
            write_pair(&mut ribbon, pair, center, normal, half_width, None);
            write_style(&mut ribbon, pair, body_color, true);
        }
    }
    for pair in 0..pair_count - 1 {
        let left = (pair * 2) as u16;
        ribbon.indices.extend([left, left + 1, left + 2, left + 2, left + 1, left + 3]);
    }
    Ok(ribbon)
}

fn stored_point(point: Vec2) -> Result<Vec2, String> {
    let point = Vec2::new(fround(point.x), fround(point.y));
    if !point.x.is_finite() || !point.y.is_finite() {
        return Err("Flame coordinates must be finite float32 values".into());
    }
    Ok(point)
}

fn added(left: Vec2, right: Vec2) -> Vec2 {
    Vec2::new(fround(left.x + right.x), fround(left.y + right.y))
}

fn difference(left: Vec2, right: Vec2) -> Vec2 {
    Vec2::new(fround(left.x - right.x), fround(left.y - right.y))
}

fn scaled(point: Vec2, scale: f64) -> Vec2 {
    Vec2::new(fround(point.x * scale), fround(point.y * scale))
}

fn length(point: Vec2) -> f64 {
    fround(fround(point.x * point.x + point.y * point.y).sqrt())
}

fn normalized(point: Vec2) -> Vec2 {
    let magnitude = length(point);
    if magnitude == 0.0 { Vec2::ZERO } else { scaled(point, fround(1.0 / magnitude)) }
}

type AxisCoefficients = [[f64; 4]; 2];

fn spline_axes(points: [Vec2; 3]) -> [AxisCoefficients; 2] {
    [
        spline_axis(fround(points[0].x), fround(points[1].x), fround(points[2].x)),
        spline_axis(fround(points[0].y), fround(points[1].y), fround(points[2].y)),
    ]
}

fn spline_axis(first: f64, second: f64, third: f64) -> AxisCoefficients {
    let first_forward = fround((second - first) * 3.0 * 0.25);
    let second_forward = fround(((third - first) * 3.0 - first_forward) * 0.25);
    let third_tangent = fround(((third - second) * 3.0 - second_forward) * 0.25);
    let second_tangent = fround(second_forward - third_tangent * 0.25);
    let first_tangent = fround(first_forward - second_tangent * 0.25);
    [
        [
            first,
            first_tangent,
            fround(3.0 * (second - first) - 2.0 * first_tangent - second_tangent),
            fround(2.0 * (first - second) + first_tangent + second_tangent),
        ],
        [
            second,
            second_tangent,
            fround(3.0 * (third - second) - 2.0 * second_tangent - third_tangent),
            fround(2.0 * (second - third) + second_tangent + third_tangent),
        ],
    ]
}

fn spline_point(axes: &[AxisCoefficients; 2], parameter: f64) -> Vec2 {
    let segment = usize::from(parameter >= 1.0);
    let local = if parameter >= 2.0 { 1.0 } else { fround(parameter - segment as f64) };
    let evaluate = |axis: &AxisCoefficients| {
        if parameter < 0.0 {
            return axis[0][0];
        }
        let [position, linear, quadratic, cubic] = axis[segment];
        fround(position + local * (linear + local * (quadratic + local * cubic)))
    };
    Vec2::new(evaluate(&axes[0]), evaluate(&axes[1]))
}

fn spline_normal(axes: &[AxisCoefficients; 2], parameter: f64) -> Vec2 {
    let current_parameter = parameter.max(NORMAL_DELTA);
    let previous_parameter = fround(current_parameter - NORMAL_DELTA);
    let delta =
        difference(spline_point(axes, current_parameter), spline_point(axes, previous_parameter));
    normalized(Vec2::new(delta.y, fround(-delta.x)))
}

fn parameter_samples(
    start: Vec2,
    control: Vec2,
    spacing: f64,
    maximum_pairs: usize,
) -> Result<Vec<f64>, String> {
    let delta = difference(control, start);
    let squared = fround(delta.x * delta.x + delta.y * delta.y);
    let half_squared = fround(squared * 0.5);
    let bits = 0x5f37_59df_u32.wrapping_sub((squared as f32).to_bits() >> 1);
    let estimate = f32::from_bits(bits) as f64;
    let inverse_distance = fround(estimate * (1.5 - estimate * half_squared * estimate));
    let distance = fround(1.0 / inverse_distance);
    let ratio = fround(distance / spacing);
    let step = fround(2.0 / ratio);
    let step = if step > 0.5 { 0.5 } else { step };
    if step <= 0.0 || !step.is_finite() {
        return Err("Invalid Flame first-leg sample step".into());
    }
    let mut parameters = Vec::new();
    let mut parameter = 0.0;
    // Native comparison uses the un-stored (2-step).
    while parameter < 2.0 - step {
        parameters.push(parameter);
        if parameters.len() >= maximum_pairs {
            return Err("Flame ribbon exceeds its pair budget".into());
        }
        parameter = fround(parameter + step);
    }
    parameters.push(2.0);
    Ok(parameters)
}

fn require_budget(budget: usize) -> Result<(), String> {
    if !(1..=32_768).contains(&budget) {
        return Err("Flame geometry budget must be an integer in [1,32768]".into());
    }
    Ok(())
}

fn mix32(mut state: i32) -> i32 {
    state ^= state.wrapping_shl(21);
    state ^= ((state as u32) >> 11) as i32;
    state ^= state.wrapping_shl(4);
    state.wrapping_mul(0x0a67_cfcf)
}

fn sine_degrees(degrees: f64) -> f64 {
    fround(sin(fround(degrees * NATIVE_PI / 180.0)))
}

fn direction_degrees(degrees: f64) -> Vec2 {
    let radians = fround(degrees * NATIVE_PI / 180.0);
    Vec2::new(fround(sin(radians)), fround(-crate::float::cos(radians)))
}

fn write_pair(
    ribbon: &mut NativeFlameLashRibbon,
    pair: usize,
    center: Vec2,
    normal: Vec2,
    half_width: f64,
    shift: Option<Vec2>,
) {
    let lateral = scaled(normal, half_width);
    let left = added(center, lateral);
    let right = difference(center, lateral);
    ribbon.vertices[pair * 2] = shift.map_or(left, |shift| added(left, shift));
    ribbon.vertices[pair * 2 + 1] = shift.map_or(right, |shift| added(right, shift));
}

fn write_style(ribbon: &mut NativeFlameLashRibbon, pair: usize, color: u32, bottom: bool) {
    let vertical = if bottom { 0.73828125 } else { 0.728515625 };
    ribbon.uvs[pair * 2] = Vec2::new(0.859375, vertical);
    ribbon.uvs[pair * 2 + 1] = Vec2::new(0.86865234375, vertical);
    ribbon.colors[pair * 2..pair * 2 + 2].fill(color);
}
