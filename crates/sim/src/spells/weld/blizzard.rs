//! Blizzard source/contact glows, native contact polygon and textured beam quads
//! from `native-weld-blizzard.ts`, plus the chain constructor in the Weld runtime.

use super::meteor::draw_unit_vector;
use super::model::{NativeWeldActorBase, NativeWeldActorBirth};
use super::persistent::NativeWeldFrostFadeActorState;
use super::projectiles::{native_ground_spark_private_word, native_weld_private_stage};
use crate::float;
use crate::float::{atan2, fround, hypot};
use crate::painter::{WorldManagerLane, WorldPainterOrder};
use crate::{NativeRng, Vec2};

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NativeWeldBlizzardGlowState {
    pub common: NativeWeldActorBase,
    pub rotation_degrees: f64,
    pub scale: f64,
    pub variant: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeWeldBlizzardQuad {
    pub record: usize,
    pub vertices: [f64; 8],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeWeldBlizzardBeamPlan {
    pub jitter: Vec2,
    pub quads: [NativeWeldBlizzardQuad; 2],
    pub tint: u32,
    pub width: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeWeldBlizzardContactPolygon {
    pub beam_width: f64,
    pub half_width: f64,
    pub points: [Vec2; 4],
}

pub fn create_native_weld_blizzard_source_glows(
    birth: NativeWeldActorBirth<'_>,
    rng: &mut NativeRng,
) -> [NativeWeldBlizzardGlowState; 2] {
    std::array::from_fn(|index| {
        let scale = rng.draw_scaled_float(0.5, false);
        let rotation = rng.draw_scaled_float(360.0, false);
        NativeWeldBlizzardGlowState {
            common: NativeWeldActorBirth { id: birth.id + index as u64, ..birth }.actor(),
            rotation_degrees: rotation,
            scale: fround(scale + 1.0),
            variant: 24,
        }
    })
}

pub fn create_native_weld_blizzard_contact_glow(
    birth: NativeWeldActorBirth<'_>,
    rng: &mut NativeRng,
    painters: &mut WorldPainterOrder,
) -> NativeWeldBlizzardGlowState {
    let scale = rng.draw_scaled_float(0.5, false);
    let rotation = rng.draw_scaled_float(360.0, false);
    let position = Vec2::new(fround(birth.origin.x), fround(birth.origin.y - 20.0));
    let mut common = NativeWeldActorBirth { origin: position, ..birth }.actor();
    common.owner.painter_registrations = painters.register_many(WorldManagerLane::Actor, 1);
    NativeWeldBlizzardGlowState {
        common,
        rotation_degrees: rotation,
        scale: fround(scale + 1.0),
        variant: 3,
    }
}

#[must_use]
pub fn native_weld_blizzard_contact_polygon(
    endpoint: Vec2,
    source: Vec2,
    underpowered: bool,
    widen: f64,
) -> NativeWeldBlizzardContactPolygon {
    let dx = fround(endpoint.x - source.x);
    let dy = fround(endpoint.y - source.y);
    let length = hypot(dx, dy);
    let direction = if length == 0.0 {
        Vec2::new(0.0, -1.0)
    } else {
        Vec2::new(fround(dx / length), fround(dy / length))
    };
    let beam_width = blizzard_width(widen, underpowered);
    let half_width = fround(fround(beam_width * 25.0).max(20.0));
    let perpendicular =
        Vec2::new(fround(direction.y * half_width), fround(-direction.x * half_width));
    let start = Vec2::new(fround(source.x), fround(source.y + 15.0));
    let end = Vec2::new(
        fround(endpoint.x + fround(direction.x * 50.0)),
        fround(endpoint.y + fround(direction.y * 50.0) + 15.0),
    );
    let point = |center: Vec2, sign: f64| {
        Vec2::new(
            fround(center.x + fround(perpendicular.x * sign)),
            fround(center.y + fround(perpendicular.y * sign)),
        )
    };
    NativeWeldBlizzardContactPolygon {
        beam_width,
        half_width,
        points: [point(start, 1.0), point(end, 1.0), point(end, -1.0), point(start, -1.0)],
    }
}

#[must_use]
pub fn native_weld_blizzard_beam_plan(
    birth_tick: u64,
    endpoint: Vec2,
    source: Vec2,
    underpowered: bool,
    widen: f64,
) -> NativeWeldBlizzardBeamPlan {
    let dx = fround(endpoint.x - source.x);
    let dy = fround(endpoint.y - source.y);
    let length = hypot(dx, dy);
    let divisor = if length == 0.0 { 1.0 } else { length };
    let direction = Vec2::new(fround(dx / divisor), fround(dy / divisor));
    let width = blizzard_width(widen, underpowered);
    let longitudinal =
        Vec2::new(fround(direction.x * width * 30.0), fround(direction.y * width * 30.0));
    let magnitude = fround(width * (25.0 * 0.908955));
    let perpendicular =
        Vec2::new(fround(direction.y * magnitude), fround(-direction.x * magnitude));
    let jitter = blizzard_jitter((birth_tick as u32).wrapping_mul(3).wrapping_neg());
    let point = |x: f64, y: f64| [fround(x + jitter.x), fround(y + jitter.y)];
    let source_minus =
        Vec2::new(fround(source.x - longitudinal.x), fround(source.y - longitudinal.y));
    let source_plus =
        Vec2::new(fround(source.x + longitudinal.x), fround(source.y + longitudinal.y));
    let vertices = |points: [[f64; 2]; 4]| {
        [
            points[0][0],
            points[0][1],
            points[1][0],
            points[1][1],
            points[2][0],
            points[2][1],
            points[3][0],
            points[3][1],
        ]
    };
    let cap = NativeWeldBlizzardQuad {
        record: 43,
        vertices: vertices([
            point(source_minus.x + perpendicular.x, source_minus.y + perpendicular.y),
            point(source_minus.x - perpendicular.x, source_minus.y - perpendicular.y),
            point(source_plus.x + perpendicular.x, source_plus.y + perpendicular.y),
            point(source_plus.x - perpendicular.x, source_plus.y - perpendicular.y),
        ]),
    };
    let strip = NativeWeldBlizzardQuad {
        record: 44,
        vertices: vertices([
            point(source_plus.x + perpendicular.x, source_plus.y + perpendicular.y),
            point(source_plus.x - perpendicular.x, source_plus.y - perpendicular.y),
            point(endpoint.x + perpendicular.x, endpoint.y + perpendicular.y),
            point(endpoint.x - perpendicular.x, endpoint.y - perpendicular.y),
        ]),
    };
    NativeWeldBlizzardBeamPlan {
        jitter,
        quads: [cap, strip],
        // The source packRgb truncates .5435550212860107*255 to red byte138.
        tint: if underpowered { 0x80_bfff } else { 0x8a_ffff },
        width,
    }
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NativeWeldBlizzardChainFrostState {
    pub common: NativeWeldActorBase,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeWeldBlizzardChainEffects {
    pub fade: NativeWeldFrostFadeActorState,
    pub chain_frost: Option<NativeWeldBlizzardChainFrostState>,
    pub next_id: u64,
}

pub fn create_native_weld_blizzard_chain_effects(
    birth: NativeWeldActorBirth<'_>,
    cast_direction: Vec2,
    rng: &mut NativeRng,
    painters: &mut WorldPainterOrder,
) -> NativeWeldBlizzardChainEffects {
    let fade_radius = rng.draw_scaled_float(10.0, false);
    let fade_direction = draw_unit_vector(rng);
    let fade_scale = rng.draw_scaled_float(0.5, false);
    rng.draw_scaled_float(0.75, false);
    let fade_position = Vec2::new(
        fround(birth.origin.x + fround(fade_direction.x * fade_radius)),
        fround(birth.origin.y - 15.0 + fround(fade_direction.y * fade_radius)),
    );
    let mut common = NativeWeldActorBirth { origin: fade_position, ..birth }.actor();
    common.owner.painter_registrations = painters.register_many(WorldManagerLane::Actor, 1);
    let fade = NativeWeldFrostFadeActorState { common, scale: fround(fade_scale + 1.0) };
    let mut next_id = birth.id + 1;
    let chaining = rng.draw_integer(2);
    let mut chain_frost = None;
    if chaining == 1 {
        for _ in 0..4 {
            rng.draw_word();
        }
        let radius = rng.draw_scaled_float(10.0, false);
        let heading_jitter = rng.draw_scaled_float(2.0, true);
        let cast_heading = atan2(cast_direction.x, -cast_direction.y);
        let jittered = cast_heading + heading_jitter * std::f64::consts::PI / 180.0;
        let chain_heading = atan2(birth.direction.x, -birth.direction.y)
            + float::sin(birth.tick as f64 * 65.0 * std::f64::consts::PI / 180.0)
                * 20.0
                * std::f64::consts::PI
                / 180.0;
        let position = Vec2::new(
            fround(birth.origin.x + float::sin(jittered) * radius),
            fround(birth.origin.y - 15.0 - float::cos(jittered) * radius),
        );
        let direction = Vec2::new(
            fround(float::sin(chain_heading) * 0.5),
            fround(-float::cos(chain_heading) * 0.5),
        );
        let mut common =
            NativeWeldActorBirth { direction, id: next_id, origin: position, ..birth }.actor();
        common.owner.painter_registrations = painters.register_many(WorldManagerLane::Actor, 1);
        chain_frost = Some(NativeWeldBlizzardChainFrostState { common });
        next_id += 1;
    }
    NativeWeldBlizzardChainEffects { fade, chain_frost, next_id }
}

fn blizzard_width(widen: f64, underpowered: bool) -> f64 {
    let width = if widen == 0.0 { 0.75 } else { fround(widen * 3.0 + 1.0) };
    if underpowered { fround(width * 0.5) } else { width }
}

fn blizzard_jitter(seed: u32) -> Vec2 {
    let first_stage = native_weld_private_stage(seed);
    let mixed = native_ground_spark_private_word((first_stage as i32).unsigned_abs());
    let unit = f64::from(mixed % 100_000) / 100_000.0;
    let radius = unit * 5.0;
    let radians = unit * std::f64::consts::PI * 2.0;
    Vec2::new(fround(float::sin(radians) * radius), fround(-float::cos(radians) * radius))
}
