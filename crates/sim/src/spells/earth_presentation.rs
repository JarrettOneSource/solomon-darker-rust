//! The native boulder's projected rock shell from `renderer/earth-boulder-presentation.ts`.
//! Ordinary Earth and welded Ethereal Boulder share this geometry before choosing
//! their own stock sprite layers.

use super::earth::{EarthBoulderVector3, earth_boulder_transform_point};
use super::model::{
    EarthBoulderOrientation, PrimarySpellEarthProjectile, PrimarySpellProjectilePhase,
};
use super::presentation::{NativeSpellSprite, NativeSpellSpritePlan, SpellBlend};
use super::visual::visual_unit_random;
use crate::Vec2;
use crate::float;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EarthBoulderPresentationState {
    pub age_ticks: f64,
    pub assembly_charge: f64,
    pub charge: f64,
    pub flight_ticks: u64,
    pub id: u64,
    pub orientation: EarthBoulderOrientation,
    pub phase: PrimarySpellProjectilePhase,
    pub shell_charge: f64,
}

impl From<&PrimarySpellEarthProjectile> for EarthBoulderPresentationState {
    fn from(state: &PrimarySpellEarthProjectile) -> Self {
        Self {
            age_ticks: state.common.age_ticks as f64,
            assembly_charge: state.assembly_charge,
            charge: state.common.charge,
            flight_ticks: state.common.flight_ticks,
            id: state.common.id,
            orientation: state.orientation,
            phase: state.common.phase,
            shell_charge: state.shell_charge,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EarthBoulderAuraPlan {
    pub alpha: f64,
    pub record: usize,
    pub scale: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EarthBoulderOpeningFlashPlan {
    pub alpha: f64,
    pub record: usize,
    pub rotation: f64,
    pub scale: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EarthBoulderRockPlan {
    pub local: EarthBoulderVector3,
    pub position: Vec2,
    pub record: usize,
    pub rotation: f64,
    pub scale: f64,
    pub shell_index: Option<usize>,
    pub stored_scale: f64,
    pub transformed: EarthBoulderVector3,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EarthBoulderPresentationPlan {
    pub aura: EarthBoulderAuraPlan,
    pub body_alpha: f64,
    pub jitter: Vec2,
    pub opening_flash: EarthBoulderOpeningFlashPlan,
    pub orientation: EarthBoulderOrientation,
    pub rocks: Vec<EarthBoulderRockPlan>,
    pub sort_bias: f64,
    pub visual_offset: Vec2,
}

#[must_use]
pub fn earth_boulder_presentation_plan(
    state: &EarthBoulderPresentationState,
    render_tick: f64,
) -> EarthBoulderPresentationPlan {
    let opening_mix = (1.0 - state.age_ticks * 0.03500000014901161).clamp(0.0, 1.0);
    let visual_tick = render_tick.max(0.0).floor() as u32;
    let radius =
        visual_unit_random(state.id, 0x3000_u32.wrapping_add(visual_tick.wrapping_mul(2))) * 3.0;
    let angle = visual_unit_random(state.id, 0x3001_u32.wrapping_add(visual_tick.wrapping_mul(2)))
        * std::f64::consts::PI
        * 2.0;
    let jitter = Vec2::new(float::cos(angle) * radius, float::sin(angle) * radius);
    let count = 30.0 * state.assembly_charge;
    let rebuild_bucket = count.floor() as u32;
    let shell_radius = 30.0 * state.shell_charge;
    let golden_angle = std::f64::consts::PI * (3.0 - 5.0_f64.sqrt());
    let mut rocks = Vec::with_capacity(count.ceil() as usize + 1);
    rocks.push(rock_plan(
        EarthBoulderVector3 { x: 0.0, y: 0.0, z: 0.0 },
        171,
        4.0 * state.assembly_charge,
        None,
        state.orientation,
    ));
    for index in 0..count.ceil() as u32 {
        let y = 2.0 * f64::from(index) / count - 1.0 + 1.0 / count;
        let radial = (1.0 - y * y).max(0.0).sqrt();
        let theta = f64::from(index) * golden_angle;
        let local = EarthBoulderVector3 {
            x: float::cos(theta) * radial * shell_radius,
            y: y * shell_radius,
            z: float::sin(theta) * radial * shell_radius,
        };
        let record = 168
            + (visual_unit_random(state.id, 0x1000 + rebuild_bucket * 64 + index) * 3.0).floor()
                as usize;
        let stored_scale =
            ((visual_unit_random(state.id, 0x2000 + rebuild_bucket * 64 + index) * 0.75 + 0.5)
                * state.assembly_charge.min(1.0))
            .min(1.0);
        rocks.push(rock_plan(local, record, stored_scale, Some(index as usize), state.orientation));
    }
    rocks.retain(|rock| rock.transformed.z > -40.0);
    rocks.sort_by(|left, right| {
        left.transformed
            .z
            .total_cmp(&right.transformed.z)
            .then(left.shell_index.cmp(&right.shell_index))
    });
    EarthBoulderPresentationPlan {
        aura: EarthBoulderAuraPlan {
            alpha: 0.35 + visual_unit_random(state.id, 0x4000_u32.wrapping_add(visual_tick)) * 0.25,
            record: 15,
            scale: 4.1_f32 as f64 * state.charge,
        },
        body_alpha: 1.0 - opening_mix,
        jitter,
        opening_flash: EarthBoulderOpeningFlashPlan {
            alpha: opening_mix,
            record: 86,
            rotation: render_tick * 6.0 * std::f64::consts::PI / 180.0,
            scale: 2.5 * opening_mix,
        },
        orientation: state.orientation,
        rocks,
        sort_bias: (20.0 + 10.0 * state.charge) * state.charge * 1.5,
        visual_offset: Vec2::new(jitter.x, -20.0 - 32.5 * state.charge + jitter.y),
    }
}

/// Ordinary Earth converts the shared shell to its original sprite layers.
/// Weld uses the raw stored scales and shell centers from the same geometry.
#[must_use]
pub fn earth_boulder_sprite_plan(
    state: &PrimarySpellEarthProjectile,
    render_tick: f64,
) -> NativeSpellSpritePlan {
    earth_boulder_sprite_plan_at_age(state, render_tick, state.common.age_ticks as f64)
}

/// The source opening flash reads fractional presentation age; cosmetic jitter reads render time.
#[must_use]
pub fn earth_boulder_sprite_plan_at_age(
    state: &PrimarySpellEarthProjectile,
    render_tick: f64,
    age_ticks: f64,
) -> NativeSpellSpritePlan {
    let mut input = EarthBoulderPresentationState::from(state);
    input.age_ticks = age_ticks;
    let geometry = earth_boulder_presentation_plan(&input, render_tick);
    let mut plan = NativeSpellSpritePlan::at(state.common.position);
    plan.region_light_point = Some(state.common.position);
    plan.sort_bias = geometry.sort_bias;
    plan.draws.push(NativeSpellSprite {
        alpha: geometry.aura.alpha,
        offset: geometry.visual_offset,
        tint: 0xe6ffe6,
        ..NativeSpellSprite::new(geometry.aura.record, geometry.aura.scale)
    });
    plan.draws.push(NativeSpellSprite {
        alpha: geometry.opening_flash.alpha,
        blend: SpellBlend::Add,
        offset: geometry.visual_offset,
        rotation: geometry.opening_flash.rotation,
        ..NativeSpellSprite::new(geometry.opening_flash.record, geometry.opening_flash.scale)
    });
    for rock in geometry.rocks {
        plan.draws.push(NativeSpellSprite {
            alpha: geometry.body_alpha,
            offset: Vec2::new(
                geometry.visual_offset.x + rock.position.x,
                geometry.visual_offset.y + rock.position.y,
            ),
            rotation: rock.rotation,
            ..NativeSpellSprite::new(rock.record, rock.scale)
        });
    }
    plan
}

fn rock_plan(
    local: EarthBoulderVector3,
    record: usize,
    stored_scale: f64,
    shell_index: Option<usize>,
    orientation: EarthBoulderOrientation,
) -> EarthBoulderRockPlan {
    let transformed = earth_boulder_transform_point(local, orientation);
    EarthBoulderRockPlan {
        local,
        position: Vec2::new(transformed.x, transformed.y),
        record,
        rotation: 0.0,
        scale: stored_scale.max(0.45_f32 as f64),
        shell_index,
        stored_scale,
        transformed,
    }
}
