//! Frost jet emission, native f32 motion and draw plans from `primary-spell-water.ts`.
//!
//! Noise is derived from the particle id, never from gameplay RNG. The normal jet clips and
//! splays at terrain; its overlaid sibling follows the post-world painter lane.

use super::model::PrimarySpellWaterTransient;
use super::visual::{visual_hash, visual_unit_random};
use crate::Vec2;
use crate::float;
use crate::float::{atan2, fround};

const NATIVE_PI: f64 = std::f32::consts::PI as f64;
const LIFETIME_STEP: f64 = 0.04_f32 as f64;
const CORE_GROWTH: f64 = 0.01_f32 as f64;
const GLINT_SHRINK: f64 = 0.95_f32 as f64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaterFrostJetKind {
    Normal,
    Over,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaterFrostJetBlend {
    Normal,
    Add,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaterFrostJetSprite {
    Core,
    Glint,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaterFrostJetPass {
    AdditiveCore,
    Core,
    Glint,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaterFrostJetPainterLane {
    WorldSorted,
    PostWorldQueue,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaterFrostJetDraw {
    pub alpha: f64,
    pub blend: WaterFrostJetBlend,
    /// RGB, in the source color's channel order.
    pub color: [f64; 3],
    pub pass: WaterFrostJetPass,
    pub position: Vec2,
    /// Radians, as in the Website's Pixi sprite rotation.
    pub rotation: f64,
    pub scale: f64,
    pub sprite: WaterFrostJetSprite,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WaterFrostJetPlan {
    pub core_scale: f64,
    pub draws: Vec<WaterFrostJetDraw>,
    pub glint_scale: f64,
    pub heading: f64,
    pub kind: WaterFrostJetKind,
    pub lifetime: f64,
    pub position: Vec2,
    pub velocity: Vec2,
    pub world_y: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaterFrostJetEmission {
    pub direction: Vec2,
    pub jitter_radius: f64,
    pub origin: Vec2,
    pub speed: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaterFrostJetEmissionInput {
    pub emitter: Vec2,
    pub base_direction: Vec2,
    pub tick: u64,
    pub ordinal: u32,
    pub id: u64,
    pub particle_count: u32,
    pub speed: f64,
    pub widen_half_degrees: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaterFrostJetObstruction {
    pub distance: f64,
    pub point: Vec2,
}

struct WaterFrostJetFields {
    additive_core_alpha: f64,
    color_ramp: f64,
    core_scale: f64,
    glint_scale: f64,
    lifetime: f64,
    opacity_multiplier: f64,
    phase: f64,
}

#[must_use]
pub fn water_frost_jet_emission(input: WaterFrostJetEmissionInput) -> WaterFrostJetEmission {
    let WaterFrostJetEmissionInput {
        emitter,
        base_direction,
        tick,
        ordinal,
        id,
        particle_count,
        speed,
        widen_half_degrees,
    } = input;
    let base_heading =
        fround(atan2(base_direction.x, -base_direction.y) * 180.0 / std::f64::consts::PI);
    let phase_step = fround(65.0 / f64::from(particle_count));
    let mut phase = fround(tick as f64);
    for _ in 0..ordinal {
        phase = fround(phase + phase_step);
    }
    let phase_degrees = fround(phase * 65.0);
    let phase_radians = fround(phase_degrees * NATIVE_PI / 180.0);
    let wave = fround(float::sin(phase_radians));
    let amplitude = fround(widen_half_degrees + 15.0);
    let heading = fround(base_heading + wave * amplitude);
    let jitter_heading = fround(base_heading + signed_bounded_random(id, 2, 45.0));
    let jitter_radius = bounded_random(id, 3, 10.0);
    let jitter = unit_for_heading_degrees(jitter_heading, jitter_radius);
    WaterFrostJetEmission {
        direction: unit_for_heading_degrees(heading, 1.0),
        jitter_radius,
        origin: Vec2::new(
            fround(emitter.x + fround(jitter.x)),
            fround(emitter.y + fround(jitter.y)),
        ),
        speed,
    }
}

#[must_use]
pub fn water_frost_jet_particle_count(widen_half_degrees: f64, enhanced_effects: bool) -> u32 {
    let divisor = if enhanced_effects { -10.0 } else { -20.0 };
    (1.0 - ((widen_half_degrees + 15.0) / divisor).trunc()) as u32
}

#[must_use]
pub fn water_frost_jet_speed(widen_half_degrees: f64) -> f64 {
    let factor = fround(fround(widen_half_degrees / 2.5) * 0.05_f32 as f64 + 1.0);
    fround(4.0 * factor)
}

#[must_use]
pub fn water_frost_jet_obstruction(
    emission: WaterFrostJetEmission,
    caster_position: Vec2,
    id: u64,
    clip: &dyn Fn(Vec2, Vec2) -> Option<Vec2>,
    underpowered: bool,
) -> Option<WaterFrostJetObstruction> {
    if water_frost_jet_kind(id, underpowered) != WaterFrostJetKind::Normal {
        return None;
    }
    let velocity = frost_velocity(emission.direction, emission.speed);
    let prediction_steps = fround(initial_lifetime(id) / LIFETIME_STEP + emission.jitter_radius);
    let prediction_end = Vec2::new(
        fround(caster_position.x + fround(velocity.x * prediction_steps)),
        fround(caster_position.y + fround(velocity.y * prediction_steps)),
    );
    let hit = clip(caster_position, prediction_end)?;
    let point = float32_vector(hit);
    let distance = native_float32_distance(emission.origin, point);
    let beyond_hit = native_float32_distance_squared(caster_position, emission.origin)
        > native_float32_distance_squared(caster_position, point);
    Some(WaterFrostJetObstruction { distance: if beyond_hit { 0.0 } else { distance }, point })
}

#[must_use]
pub fn water_frost_jet_lifetime_ticks(id: u64) -> u64 {
    let mut lifetime = initial_lifetime(id);
    let mut updates = 0;
    loop {
        updates += 1;
        lifetime = fround(lifetime - LIFETIME_STEP);
        if lifetime <= 0.0 {
            return updates;
        }
    }
}

#[must_use]
pub fn water_frost_jet_kind(id: u64, underpowered: bool) -> WaterFrostJetKind {
    if !underpowered && visual_hash(id, 0) & 3 == 1 {
        WaterFrostJetKind::Over
    } else {
        WaterFrostJetKind::Normal
    }
}

#[must_use]
pub fn water_frost_jet_painter_lane(kind: WaterFrostJetKind) -> WaterFrostJetPainterLane {
    match kind {
        WaterFrostJetKind::Normal => WaterFrostJetPainterLane::WorldSorted,
        WaterFrostJetKind::Over => WaterFrostJetPainterLane::PostWorldQueue,
    }
}

#[must_use]
pub fn water_frost_jet_plan(state: &PrimarySpellWaterTransient) -> WaterFrostJetPlan {
    let kind = water_frost_jet_kind(state.common.id, state.underpowered);
    let fields = fields(state.common.id, state.common.age_ticks, kind, state.underpowered);
    let heading = atan2(state.direction.x, -state.direction.y);
    let (position, velocity) = motion(state);
    plan_from_fields(kind, fields, heading, position, velocity)
}

#[must_use]
pub fn water_frost_jet_chaining_plan(
    id: u64,
    age_ticks: u64,
    direction: Vec2,
    origin: Vec2,
) -> WaterFrostJetPlan {
    let mut fields = fields(id, age_ticks, WaterFrostJetKind::Normal, false);
    let mut lifetime = initial_lifetime(id);
    let mut core_scale = 0.5_f32 as f64;
    for _ in 0..age_ticks {
        lifetime = fround(lifetime - LIFETIME_STEP);
        if lifetime < 1.0 {
            core_scale = fround(core_scale + CORE_GROWTH);
        }
        core_scale = fround(core_scale - CORE_GROWTH);
    }
    fields.core_scale = core_scale;
    let velocity = frost_velocity(direction, 4.0);
    let mut position = float32_vector(origin);
    for _ in 0..age_ticks {
        position.x = fround(position.x + velocity.x);
        position.y = fround(position.y + velocity.y);
    }
    plan_from_fields(
        WaterFrostJetKind::Normal,
        fields,
        atan2(direction.x, -direction.y),
        position,
        velocity,
    )
}

#[must_use]
pub fn pack_water_frost_tint(color: [f64; 3]) -> u32 {
    let channel = |value: f64| (fround(value).clamp(0.0, 1.0) * 255.0).trunc() as u32;
    channel(color[0]) << 16 | channel(color[1]) << 8 | channel(color[2])
}

#[must_use]
pub fn quantize_water_frost_alpha(alpha: f64) -> f64 {
    (fround(alpha).clamp(0.0, 1.0) * 255.0).trunc() / 255.0
}

fn plan_from_fields(
    kind: WaterFrostJetKind,
    fields: WaterFrostJetFields,
    heading: f64,
    position: Vec2,
    velocity: Vec2,
) -> WaterFrostJetPlan {
    let color = [(1.0 - fields.color_ramp).clamp(0.0, 1.0), 1.0, 1.0];
    let core_alpha = match kind {
        WaterFrostJetKind::Normal => fround(
            fields.opacity_multiplier * fround(fields.lifetime * fields.lifetime).min(fields.phase),
        ),
        WaterFrostJetKind::Over => fround(0.5 * fields.lifetime.min(fields.phase)),
    };
    let mut draws = vec![WaterFrostJetDraw {
        alpha: core_alpha,
        blend: WaterFrostJetBlend::Normal,
        color,
        pass: WaterFrostJetPass::Core,
        position,
        rotation: heading,
        scale: fields.core_scale,
        sprite: WaterFrostJetSprite::Core,
    }];
    if kind == WaterFrostJetKind::Normal && fields.additive_core_alpha > 0.0 {
        draws.push(WaterFrostJetDraw {
            alpha: fields.additive_core_alpha,
            blend: WaterFrostJetBlend::Add,
            color: [1.0; 3],
            pass: WaterFrostJetPass::AdditiveCore,
            position,
            rotation: heading,
            scale: fround(fields.core_scale * 0.5),
            sprite: WaterFrostJetSprite::Core,
        });
    }
    if kind == WaterFrostJetKind::Over || fields.opacity_multiplier >= 0.9_f32 as f64 {
        let alpha = match kind {
            WaterFrostJetKind::Normal => {
                fround(fields.opacity_multiplier * (fields.lifetime * 10.0).min(1.0))
            }
            WaterFrostJetKind::Over => {
                fround((3.0 * (fields.phase * 0.5).min(fields.lifetime)).min(1.0))
            }
        };
        let scale = match kind {
            WaterFrostJetKind::Normal => fields.glint_scale.min(1.0),
            WaterFrostJetKind::Over => fround(fields.glint_scale * 0.25),
        };
        let glint_position = Vec2::new(
            fround(position.x + fround(velocity.x * 3.0)),
            fround(position.y + fround(velocity.y * 3.0)),
        );
        draws.push(WaterFrostJetDraw {
            alpha,
            blend: WaterFrostJetBlend::Add,
            color: [1.0; 3],
            pass: WaterFrostJetPass::Glint,
            position: glint_position,
            rotation: heading,
            scale,
            sprite: WaterFrostJetSprite::Glint,
        });
    }
    WaterFrostJetPlan {
        core_scale: fields.core_scale,
        draws,
        glint_scale: fields.glint_scale,
        heading,
        kind,
        lifetime: fields.lifetime,
        position,
        velocity,
        world_y: position.y,
    }
}

fn initial_lifetime(id: u64) -> f64 {
    fround(1.25 + bounded_random(id, 1, 0.05_f32 as f64))
}

fn fields(
    id: u64,
    age_ticks: u64,
    kind: WaterFrostJetKind,
    underpowered: bool,
) -> WaterFrostJetFields {
    let mut lifetime = initial_lifetime(id);
    let mut phase = 0.0;
    let mut additive_core_alpha = fround(0.75 * if underpowered { 0.25 } else { 1.0 });
    let mut core_scale = fround(0.5 + bounded_random(id, 4, 0.75_f32 as f64));
    let mut glint_scale = fround((2.0 + bounded_random(id, 5, 1.0)) * core_scale);
    let mut color_ramp = if kind == WaterFrostJetKind::Normal {
        fround(1.0 + bounded_random(id, 6, 0.1_f32 as f64))
    } else {
        0.0
    };
    let opacity_multiplier = fround(if underpowered { 0.25 } else { 1.0 });
    let phase_step = match kind {
        WaterFrostJetKind::Normal => 0.05_f32 as f64,
        WaterFrostJetKind::Over => 0.025_f32 as f64,
    };
    for _ in 0..age_ticks {
        lifetime = fround(lifetime - LIFETIME_STEP);
        phase = fround(phase + phase_step);
        additive_core_alpha = fround(additive_core_alpha - 0.05_f32 as f64);
        color_ramp = fround((color_ramp - 0.075_f32 as f64).max(0.0));
        if lifetime < 1.0 {
            glint_scale = fround(glint_scale * GLINT_SHRINK);
            core_scale = fround(core_scale + CORE_GROWTH);
        }
    }
    WaterFrostJetFields {
        additive_core_alpha,
        color_ramp,
        core_scale,
        glint_scale,
        lifetime,
        opacity_multiplier,
        phase,
    }
}

fn motion(state: &PrimarySpellWaterTransient) -> (Vec2, Vec2) {
    let mut velocity = frost_velocity(state.direction, state.speed);
    let mut pending_distance = state.obstruction_distance;
    let mut position = float32_vector(state.origin);
    for _ in 0..state.common.age_ticks {
        if let (Some(distance), Some(point)) = (pending_distance, state.obstruction_point) {
            let remaining = fround(distance - native_float32_distance(Vec2::ZERO, velocity));
            if remaining <= 0.0 {
                position = float32_vector(point);
                let sign = if visual_hash(state.common.id, 7) & 1 == 0 { -1.0 } else { 1.0 };
                velocity =
                    Vec2::new(fround(sign * velocity.y * 0.5), fround(sign * -velocity.x * 0.5));
                pending_distance = None;
            } else {
                pending_distance = Some(remaining);
            }
        }
        position.x = fround(position.x + velocity.x);
        position.y = fround(position.y + velocity.y);
    }
    (position, velocity)
}

fn frost_velocity(direction: Vec2, speed: f64) -> Vec2 {
    let factor = fround(speed / 4.0);
    Vec2::new(
        fround(fround(fround(direction.x) * 4.0) * factor),
        fround(fround(fround(direction.y) * 4.0) * factor),
    )
}

fn unit_for_heading_degrees(heading: f64, magnitude: f64) -> Vec2 {
    let radians = fround(NATIVE_PI * fround(heading) / 180.0);
    Vec2::new(
        fround(fround(float::sin(radians)) * magnitude),
        fround(fround(-float::cos(radians)) * magnitude),
    )
}

fn signed_bounded_random(id: u64, salt: u32, bound: f64) -> f64 {
    fround((visual_unit_random(id, salt) * 2.0 - 1.0) * fround(bound))
}
fn bounded_random(id: u64, salt: u32, bound: f64) -> f64 {
    fround(visual_unit_random(id, salt) * fround(bound))
}
fn float32_vector(value: Vec2) -> Vec2 {
    Vec2::new(fround(value.x), fround(value.y))
}
fn native_float32_distance(first: Vec2, second: Vec2) -> f64 {
    fround(native_float32_distance_squared(first, second).sqrt())
}
fn native_float32_distance_squared(first: Vec2, second: Vec2) -> f64 {
    let dx = fround(fround(first.x) - fround(second.x));
    let dy = fround(fround(first.y) - fround(second.y));
    fround(dx * dx + dy * dy)
}
