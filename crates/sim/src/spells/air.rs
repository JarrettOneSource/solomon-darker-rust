//! Lightning ribbon geometry and corona factories from `core-kernels/native-air-presentation.ts`.
//! Cosmetic samples own a semantic stream; they never advance the world's native RNG.

use super::presentation::{NativeSpellSprite, SpellBlend};
use crate::Vec2;
use crate::float;
use crate::float::{atan2, fround, hypot};
use crate::random_domain::{native_random_float_from_word, native_random_int_from_word};

#[derive(Clone, Debug, PartialEq)]
pub struct NativeAirMesh {
    pub indices: Vec<u32>,
    pub uvs: Vec<Vec2>,
    pub vertices: Vec<Vec2>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeAirBranchPlan {
    pub geometry_record: usize,
    pub mirror_x: bool,
    pub scale: f64,
    pub texture_record: usize,
    pub mesh: NativeAirMesh,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeAirRibbonLayer {
    pub alpha: f64,
    pub branch: Option<NativeAirBranchPlan>,
    pub mesh: NativeAirMesh,
    pub parameter_samples: Vec<f64>,
    pub phase_degrees: f64,
    pub phase_offset: f64,
    pub texture_record: usize,
    pub tint: u32,
    pub width: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeAirCoronaPlan {
    pub alpha: f64,
    pub center: Vec2,
    pub draws: Vec<NativeSpellSprite>,
}

/// Source-shaped corona input shared by Lightning and the Weld compositors.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeAirCoronaInput {
    pub alpha: f64,
    pub angle: f64,
    pub center: Vec2,
    pub random_salt: u32,
    pub scale: f64,
    pub seed: u64,
}

#[must_use]
pub fn build_native_air_corona_plan(input: NativeAirCoronaInput) -> NativeAirCoronaPlan {
    build_corona(
        input.center,
        input.alpha,
        input.scale,
        input.angle,
        NativeAirPresentationRandom::new(input.seed, input.random_salt),
    )
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeAirLightPlan {
    pub casts_directional_shadow: bool,
    pub intensity: f64,
    pub position: Vec2,
    pub radius: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeAirLightningPlan {
    pub body: Option<Vec<NativeAirRibbonLayer>>,
    pub contact_corona: NativeAirCoronaPlan,
    pub contact_light: Option<NativeAirLightPlan>,
    pub endpoint: Vec2,
    pub midpoint: Vec2,
    pub source: Vec2,
    pub source_corona: Option<NativeAirCoronaPlan>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeAirLightningInput {
    pub chained: bool,
    pub enhanced_effects: bool,
    pub age_ticks: u64,
    pub birth_tick: u64,
    pub endpoint: Vec2,
    pub id: u64,
    pub midpoint: Vec2,
    pub underpowered: bool,
}

/// Common native Air body factory. ElectricBurn owns its contact flare separately.
#[must_use]
pub fn build_native_air_lightning_body(
    input: NativeAirLightningInput,
) -> Option<Vec<NativeAirRibbonLayer>> {
    let points = [Vec2::ZERO, input.midpoint, input.endpoint];
    if input.age_ticks < 2 {
        let (width, tint, alpha) =
            if input.underpowered { (0.75, 0x80ffff, 0.5) } else { (1.0, 0xffffff, 1.0) };
        let ribbon = NativeAirRibbonInput {
            points,
            id: input.id,
            birth_tick: input.birth_tick,
            width,
            base_phase_degrees: -3.0 * input.birth_tick as f64,
            phase_offset: 0.0,
            tint,
            alpha,
            dark: false,
            taper_scale: 1.0,
            enhanced_effects: input.enhanced_effects,
            ribbon_family: NativeAirRibbonFamily::Air,
        };
        Some(vec![
            build_native_air_ribbon_with_style(ribbon),
            build_native_air_ribbon_with_style(NativeAirRibbonInput {
                width: width * 0.75,
                phase_offset: 15.0,
                tint: 0x00ffff,
                alpha: alpha * 0.5,
                ..ribbon
            }),
        ])
    } else {
        None
    }
}

#[must_use]
pub fn build_native_air_lightning_plan(input: NativeAirLightningInput) -> NativeAirLightningPlan {
    let body = build_native_air_lightning_body(input);
    let source_corona = if input.age_ticks == 0 {
        let mut random = NativeAirPresentationRandom::new(input.id, 0x534f5552);
        let scale = fround(1.0 + random.float(0.5));
        let angle = random.float(std::f64::consts::PI * 2.0);
        Some(build_corona(
            Vec2::ZERO,
            1.0,
            scale,
            angle,
            NativeAirPresentationRandom::new(input.id, 0x474c4f57),
        ))
    } else {
        None
    };
    let contact = contact_samples(input.id);
    let center = Vec2::new(
        input.endpoint.x + float::cos(contact.offset_angle) * contact.offset_radius,
        input.endpoint.y + float::sin(contact.offset_angle) * contact.offset_radius,
    );
    let angle = contact.angle + input.age_ticks as f64 * (std::f64::consts::PI / 180.0);
    let alpha = contact_alpha_levels(input.underpowered, input.chained, input.enhanced_effects)
        .get(input.age_ticks as usize)
        .copied()
        .unwrap_or(0.0);
    let contact_corona = build_corona(
        center,
        alpha,
        contact.scale,
        angle,
        NativeAirPresentationRandom::new(input.id, 0x46414445 ^ input.age_ticks as u32),
    );
    NativeAirLightningPlan {
        body,
        contact_corona,
        contact_light: contact_light(
            input.id,
            input.age_ticks,
            center,
            input.underpowered,
            input.chained,
            input.enhanced_effects,
        ),
        endpoint: input.endpoint,
        midpoint: input.midpoint,
        source: Vec2::ZERO,
        source_corona,
    }
}

/// World-space ZAnimLit source; the collector owns its registration and translation.
#[must_use]
pub fn build_native_air_contact_light_source(
    id: u64,
    age_ticks: u64,
    endpoint: Vec2,
    origin: Vec2,
    underpowered: bool,
    chained: bool,
    enhanced_effects: bool,
) -> Option<NativeAirLightPlan> {
    let samples = contact_samples(id);
    let position = Vec2::new(
        endpoint.x + float::cos(samples.offset_angle) * samples.offset_radius,
        endpoint.y + float::sin(samples.offset_angle) * samples.offset_radius,
    );
    contact_light(id, age_ticks, position, underpowered, chained, enhanced_effects).map(
        |mut light| {
            light.position.x += origin.x;
            light.position.y += origin.y;
            light
        },
    )
}

#[must_use]
pub fn build_native_air_path_light_sources(
    id: u64,
    birth_tick: u64,
    origin: Vec2,
    midpoint: Vec2,
    endpoint: Vec2,
    weak_cast: bool,
    enhanced_effects: bool,
) -> Vec<NativeAirLightPlan> {
    let mut random =
        NativeAirPresentationRandom::new(id, semantic_seed(id, birth_tick, 0x4d495343));
    let jitter = fround(random.float(1.0) * 0.75);
    let intensity = fround(fround(0.25 + jitter) * if weak_cast { 0.25 } else { 1.0 });
    let mut result = Vec::new();
    for (start, end) in [(origin, midpoint), (midpoint, endpoint)] {
        let mut current = Vec2::new(fround(start.x), fround(start.y));
        let delta = Vec2::new(fround(end.x - start.x), fround(end.y - start.y));
        let mut remaining = fround(fround(delta.x * delta.x + delta.y * delta.y).sqrt());
        let inverse = if remaining > 0.0 { fround(1.0 / remaining) } else { 0.0 };
        let step = Vec2::new(
            fround(fround(inverse * delta.x) * 100.0),
            fround(fround(inverse * delta.y) * 100.0),
        );
        while remaining > 50.0 {
            append_path_light(
                &mut result,
                &mut random,
                current,
                origin,
                intensity,
                enhanced_effects,
            );
            current = Vec2::new(fround(current.x + step.x), fround(current.y + step.y));
            remaining = fround(remaining - 100.0);
        }
        append_path_light(
            &mut result,
            &mut random,
            Vec2::new(fround(end.x), fround(end.y)),
            origin,
            intensity,
            enhanced_effects,
        );
    }
    result
}

fn append_path_light(
    result: &mut Vec<NativeAirLightPlan>,
    random: &mut NativeAirPresentationRandom,
    point: Vec2,
    origin: Vec2,
    intensity: f64,
    enhanced_effects: bool,
) {
    let dx = fround(point.x - origin.x);
    let dy = fround(point.y - origin.y);
    if fround(dx * dx + dy * dy) < 220.0 * 220.0 {
        return;
    }
    result.push(NativeAirLightPlan {
        casts_directional_shadow: enhanced_effects,
        intensity,
        position: Vec2::new(fround(point.x), fround(point.y + 35.0)),
        radius: fround(0.75 + fround(random.float(1.0) * 0.25)),
    });
}

fn contact_light(
    id: u64,
    age: u64,
    position: Vec2,
    underpowered: bool,
    chained: bool,
    enhanced_effects: bool,
) -> Option<NativeAirLightPlan> {
    if age >= native_air_contact_lifetime_ticks(underpowered, chained, enhanced_effects) {
        return None;
    }
    let maximum: f64 = if underpowered { 0.5 } else { 1.0 };
    let mut intensity = maximum;
    for _ in 0..age {
        intensity = fround(intensity - 0.05_f32 as f64);
    }
    Some(NativeAirLightPlan {
        casts_directional_shadow: false,
        intensity: intensity.min(maximum),
        position,
        radius: contact_samples(id).light_radius * if underpowered { 0.5 } else { 1.0 },
    })
}

/// First contacts always use .2 decay; only coarse chained contacts use .4.
#[must_use]
pub fn native_air_contact_lifetime_ticks(
    underpowered: bool,
    chained: bool,
    enhanced_effects: bool,
) -> u64 {
    contact_alpha_levels(underpowered, chained, enhanced_effects).len() as u64
}

fn contact_alpha_levels(
    underpowered: bool,
    chained: bool,
    enhanced_effects: bool,
) -> &'static [f64] {
    match (chained && !enhanced_effects, underpowered) {
        (true, true) => &[0.5, 0.1],
        (true, false) => &[1.0, 0.6, 0.2],
        (false, true) => &[0.5, 0.3, 0.1],
        (false, false) => &[1.0, 0.8, 0.6, 0.4, 0.2],
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeAirRibbonFamily {
    Air,
    FlameLash,
}

/// Source ribbon inputs shared by Lightning, Weld and Faculty Dark Lightning.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeAirRibbonInput {
    pub points: [Vec2; 3],
    pub id: u64,
    pub birth_tick: u64,
    pub width: f64,
    pub base_phase_degrees: f64,
    pub phase_offset: f64,
    pub tint: u32,
    pub alpha: f64,
    pub dark: bool,
    pub taper_scale: f64,
    pub enhanced_effects: bool,
    pub ribbon_family: NativeAirRibbonFamily,
}

#[must_use]
pub fn build_native_air_ribbon_with_style(input: NativeAirRibbonInput) -> NativeAirRibbonLayer {
    let NativeAirRibbonInput {
        points,
        id,
        birth_tick,
        width,
        base_phase_degrees,
        phase_offset,
        tint,
        alpha,
        dark,
        taper_scale,
        enhanced_effects,
        ribbon_family,
    } = input;
    let spacing = if !enhanced_effects {
        30.0
    } else {
        match ribbon_family {
            NativeAirRibbonFamily::Air => 15.0,
            NativeAirRibbonFamily::FlameLash => 7.0,
        }
    };
    let samples = parameter_samples(points[0], points[1], spacing);
    let pair_count = samples.len();
    let phase = base_phase_degrees + phase_offset;
    let mut random_state = semantic_seed(id, birth_tick, 0x52494242 ^ phase_offset as u32);
    let mut vertices = Vec::with_capacity(pair_count * 2);
    let mut uvs = Vec::with_capacity(pair_count * 2);
    let mut indices = Vec::with_capacity((pair_count - 1) * 6);
    for (pair, parameter) in samples.iter().copied().enumerate() {
        let point = spline_point(points, parameter);
        let normal = spline_normal(points, parameter);
        let mut center = point;
        let mut half_width = width * 25.0 * 0.5;
        if pair != pair_count - 1 {
            let envelope = float::sin(parameter / 2.0 * std::f64::consts::PI);
            let primary = envelope * float::sin(radians(parameter * 360.0 + phase)) * 25.0;
            let secondary = envelope * float::sin(radians(phase * 2.5 - parameter * 90.0)) * 12.0;
            center.x += normal.x * (primary + secondary);
            center.y += normal.y * (primary + secondary);
            let (angle, radius, next_state) = ribbon_random_sample(random_state);
            random_state = next_state;
            center.x += float::sin(radians(angle)) * (radius * envelope);
            center.y -= float::cos(radians(angle)) * (radius * envelope);
            half_width *= ((1.0 - envelope) * 0.75 + 0.5) * taper_scale;
        }
        vertices.push(Vec2::new(
            fround(center.x + normal.x * half_width),
            fround(center.y + normal.y * half_width),
        ));
        vertices.push(Vec2::new(
            fround(center.x - normal.x * half_width),
            fround(center.y - normal.y * half_width),
        ));
        let v = if pair == 0 || pair == pair_count - 1 {
            0.0
        } else if pair % 2 == 1 {
            1.0
        } else {
            0.5
        };
        uvs.push(Vec2::new(0.0, v));
        uvs.push(Vec2::new(1.0, v));
    }
    for segment in 0..pair_count - 1 {
        let left = (segment * 2) as u32;
        indices.extend_from_slice(&[left, left + 1, left + 2, left + 1, left + 3, left + 2]);
    }
    NativeAirRibbonLayer {
        alpha,
        branch: build_branch(
            points,
            NativeAirPresentationRandom::new(
                id,
                semantic_seed(id, birth_tick, 0x4252414e ^ phase_offset as u32),
            ),
            dark,
        ),
        mesh: NativeAirMesh { indices, uvs, vertices },
        parameter_samples: samples,
        phase_degrees: phase,
        phase_offset,
        texture_record: if dark { 64 } else { 44 },
        tint,
        width,
    }
}

fn ribbon_random_sample(state: u32) -> (f64, f64, u32) {
    let magnitude = f64::from(state % 360_000) / 360_000.0 * 65.0;
    let signed_state = (air_mix_raw(state) as i32).unsigned_abs();
    let radius_state = (air_mix_raw(signed_state) as i32).unsigned_abs();
    let angle = if signed_state % 2 == 1 { -magnitude } else { magnitude };
    (
        angle,
        f64::from(radius_state % 360_000) / 360_000.0 * 30.0,
        (air_mix_raw(radius_state) as i32).unsigned_abs(),
    )
}

fn build_branch(
    points: [Vec2; 3],
    mut random: NativeAirPresentationRandom,
    dark: bool,
) -> Option<NativeAirBranchPlan> {
    if random.integer(2) != 1 {
        return None;
    }
    let attachment = spline_point(points, random.float(2.0));
    let mut scale = fround(0.25 + random.float(0.5));
    if random.integer(30) == 1 {
        scale = 1.0;
    }
    let mirror_x = random.integer(2) == 1;
    let geometry_index = random.integer(2) as usize;
    let texture_index = random.integer(2) as usize;
    let geometry = if dark {
        [
            [
                Vec2::new(-49.0, -75.0),
                Vec2::new(13.0, -75.0),
                Vec2::new(-49.0, 20.0),
                Vec2::new(13.0, 20.0),
            ],
            [
                Vec2::new(-49.0, -183.0),
                Vec2::new(14.0, -183.0),
                Vec2::new(-49.0, 23.0),
                Vec2::new(14.0, 23.0),
            ],
        ][geometry_index]
    } else {
        [
            [
                Vec2::new(-38.0, -64.0),
                Vec2::new(1.0, -64.0),
                Vec2::new(-38.0, 9.0),
                Vec2::new(1.0, 9.0),
            ],
            [
                Vec2::new(-40.0, -170.0),
                Vec2::new(0.0, -170.0),
                Vec2::new(-40.0, 15.0),
                Vec2::new(0.0, 15.0),
            ],
        ][geometry_index]
    };
    let first = geometry[0];
    let base = (atan2(-first.y, first.x) * 180.0 / std::f64::consts::PI).rem_euclid(360.0);
    let angle = random.float(45.0);
    let signed_angle = if random.integer(2) == 1 { -angle } else { angle };
    let angle = radians(base + signed_angle);
    let x_scale = if mirror_x { -scale } else { scale };
    let mut vertices = Vec::with_capacity(4);
    for point in geometry {
        let x = point.x * x_scale;
        let y = point.y * scale;
        vertices.push(Vec2::new(
            fround(attachment.x + x * float::cos(angle) - y * float::sin(angle)),
            fround(attachment.y + x * float::sin(angle) + y * float::cos(angle)),
        ));
    }
    Some(NativeAirBranchPlan {
        geometry_record: if dark { 373 } else { 375 } + geometry_index,
        mirror_x,
        scale,
        texture_record: if dark { 373 } else { 375 } + texture_index,
        mesh: NativeAirMesh {
            indices: vec![0, 1, 2, 1, 3, 2],
            uvs: vec![
                Vec2::new(0.0, 0.0),
                Vec2::new(1.0, 0.0),
                Vec2::new(0.0, 1.0),
                Vec2::new(1.0, 1.0),
            ],
            vertices,
        },
    })
}

fn parameter_samples(source: Vec2, midpoint: Vec2, spacing: f64) -> Vec<f64> {
    let dx = fround(fround(midpoint.x) - fround(source.x));
    let dy = fround(fround(midpoint.y) - fround(source.y));
    let squared = fround(dx * dx + dy * dy);
    let half = fround(squared * 0.5);
    let bits = 0x5f3759df_u32.wrapping_sub((squared as f32).to_bits() >> 1);
    let estimate = f64::from(f32::from_bits(bits));
    let inverse = fround(estimate * (1.5 - half * estimate * estimate));
    let distance = fround(1.0 / inverse);
    let ratio = fround(distance / spacing);
    let step = fround(2.0 / ratio).min(0.5);
    let mut samples = Vec::new();
    let mut parameter = 0.0;
    while parameter < 2.0 - step {
        samples.push(parameter);
        parameter = fround(parameter + step);
    }
    samples.push(2.0);
    samples
}

fn spline_point(points: [Vec2; 3], parameter: f64) -> Vec2 {
    Vec2::new(
        spline_axis([points[0].x, points[1].x, points[2].x], parameter),
        spline_axis([points[0].y, points[1].y, points[2].y], parameter),
    )
}
fn spline_axis(points: [f64; 3], parameter: f64) -> f64 {
    let first = (points[1] - points[0]) * 3.0 * 0.25;
    let second = ((points[2] - points[0]) * 3.0 - first) * 0.25;
    let third = ((points[2] - points[1]) * 3.0 - second) * 0.25;
    let tangents = [first - (second - third * 0.25) * 0.25, second - third * 0.25, third];
    let segment = (parameter.floor() as usize).min(1);
    let local = parameter - segment as f64;
    let a = tangents[segment];
    let b = 3.0 * (points[segment + 1] - points[segment])
        - 2.0 * tangents[segment]
        - tangents[segment + 1];
    let c =
        2.0 * (points[segment] - points[segment + 1]) + tangents[segment] + tangents[segment + 1];
    points[segment] + local * (a + local * (b + local * c))
}
fn spline_normal(points: [Vec2; 3], parameter: f64) -> Vec2 {
    let current_parameter = parameter.max(0.001);
    let previous = spline_point(points, current_parameter - 0.001);
    let current = spline_point(points, current_parameter);
    let direction = Vec2::new(current.y - previous.y, previous.x - current.x);
    let length = hypot(direction.x, direction.y);
    if length > 0.0 {
        Vec2::new(direction.x / length, direction.y / length)
    } else {
        Vec2::new(1.0, 0.0)
    }
}

fn build_corona(
    center: Vec2,
    alpha: f64,
    scale: f64,
    angle: f64,
    mut random: NativeAirPresentationRandom,
) -> NativeAirCoronaPlan {
    let pulse_scale = (float::sin(angle * 15.0).abs() * 0.15 + 3.5) * scale;
    let first_fork = random.integer(4) as usize;
    let second_fork = 3 - first_fork;
    let mut draws = Vec::with_capacity(6);
    for (opacity, circle_scale) in [
        (fround(0.2 + random.float(0.25)), pulse_scale),
        (0.5, pulse_scale * 0.75),
        (0.5, pulse_scale * 0.5),
        (0.25, pulse_scale * fround(0.2 + random.float(0.2))),
    ] {
        draws.push(NativeSpellSprite {
            alpha: opacity,
            blend: SpellBlend::Add,
            tint: 0x80bfbf,
            ..NativeSpellSprite::new(110, circle_scale)
        });
    }
    draws.push(NativeSpellSprite {
        blend: SpellBlend::Add,
        rotation: angle,
        ..NativeSpellSprite::new(1836 + first_fork, scale * fround(0.75 + random.float(0.25)))
    });
    draws.push(NativeSpellSprite {
        blend: SpellBlend::Add,
        rotation: angle + std::f64::consts::PI / 2.0,
        ..NativeSpellSprite::new(1836 + second_fork, scale)
    });
    NativeAirCoronaPlan { alpha, center, draws }
}

struct ContactSamples {
    offset_radius: f64,
    offset_angle: f64,
    scale: f64,
    angle: f64,
    light_radius: f64,
}
fn contact_samples(id: u64) -> ContactSamples {
    let mut random = NativeAirPresentationRandom::new(id, 0x434f4e54);
    ContactSamples {
        offset_radius: random.float(10.0),
        offset_angle: random.float(std::f64::consts::PI * 2.0),
        scale: fround(1.0 + random.float(0.5)),
        angle: random.float(std::f64::consts::PI * 2.0),
        light_radius: fround(1.0 + random.float(0.75)),
    }
}

pub struct NativeAirPresentationRandom {
    value: u32,
}
impl NativeAirPresentationRandom {
    #[must_use]
    pub fn new(id: u64, salt: u32) -> Self {
        Self { value: mix32(id as u32 ^ salt) }
    }
    fn word(&mut self) -> u32 {
        self.value ^= self.value << 13;
        self.value ^= self.value >> 17;
        self.value ^= self.value << 5;
        self.value
    }
    pub fn float(&mut self, maximum: f64) -> f64 {
        native_random_float_from_word(self.word(), maximum)
    }
    fn integer(&mut self, bound: u32) -> u32 {
        native_random_int_from_word(self.word(), bound)
    }
}
fn radians(degrees: f64) -> f64 {
    degrees * std::f64::consts::PI / 180.0
}
fn mix32(mut value: u32) -> u32 {
    value = (value ^ (value >> 16)).wrapping_mul(0x7feb352d);
    value = (value ^ (value >> 15)).wrapping_mul(0x846ca68b);
    value ^ (value >> 16)
}
fn semantic_seed(id: u64, birth_tick: u64, salt: u32) -> u32 {
    (air_mix_raw(id as u32 ^ (birth_tick as u32).wrapping_add(1).wrapping_mul(0x9e3779b1) ^ salt)
        as i32)
        .unsigned_abs()
}
fn air_mix_raw(mut value: u32) -> u32 {
    value ^= value << 21;
    value ^= value >> 11;
    value ^= value << 4;
    value.wrapping_mul(0x0a67cfcf)
}
