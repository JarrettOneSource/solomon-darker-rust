//! Weld world-effect constructors and their ordered native epochs from
//! `native-weld-primary-runtime.ts`, `native-weld-flame-lash.ts`,
//! `native-weld-ground-spark.ts` and `primary-spells.ts`.

use super::blizzard::{NativeWeldBlizzardChainFrostState, NativeWeldBlizzardGlowState};
use super::flame_state::{
    NativeFlameChannelVisual, NativeFlameFadeBirth, NativeFlameFadeVisual, native_flame_fade_alpha,
};
use super::hail::{NativeWeldHailChildActorState, step_native_weld_hail_child};
use super::meteor::{
    NativeWeldMeteorActorState, NativeWeldMeteorMarkerState, step_native_weld_meteor,
    step_native_weld_meteor_marker,
};
use super::model::{NativeWeldActorBase, NativeWeldActorBirth};
use super::persistent::{
    NativeWeldEtherealBoulderState, NativeWeldFrostFadeActorState, NativeWeldPersistentActorState,
    NativeWeldPersistentTraversal, step_native_weld_persistent_actor,
};
use super::projectiles::NativeWeldProjectileState;
use super::steam::{NativeWeldSteamActorState, step_native_weld_steam_actor};
use crate::float::{cos, fround, sin};
use crate::painter::{PainterRegistration, WorldManagerLane, WorldPainterOrder};
use crate::spells::earth_debris::{
    NativeWeldBoulderDebrisParticleState, NativeWeldMeteorDebrisSeed,
    create_native_weld_boulder_debris_particle,
    create_native_weld_ethereal_boulder_breakup_debris_program,
    step_native_weld_boulder_debris_particle,
};
use crate::spells::water::water_frost_jet_lifetime_ticks;
use crate::{NativeRng, Vec2};

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NativeWeldChannelActorState {
    pub common: NativeWeldActorBase,
    #[cfg_attr(feature = "persistence", serde(default = "enhanced_effects_default"))]
    pub enhanced_effects: bool,
    #[cfg_attr(feature = "persistence", serde(default, skip_serializing_if = "Option::is_none"))]
    pub flame_visual: Option<NativeFlameChannelVisual>,
    pub endpoint: Option<Vec2>,
    pub midpoint: Option<Vec2>,
    pub target_id: Option<String>,
    pub underpowered: bool,
    pub variant: u32,
}

pub struct NativeWeldChannelInput<'a> {
    pub birth: NativeWeldActorBirth<'a>,
    pub endpoint: Option<Vec2>,
    pub midpoint: Option<Vec2>,
    pub target_id: Option<String>,
    pub underpowered: bool,
    pub enhanced_effects: bool,
    pub flame_visual: Option<NativeFlameChannelVisual>,
}

#[must_use]
pub fn create_native_weld_channel_actor(
    input: NativeWeldChannelInput<'_>,
) -> NativeWeldChannelActorState {
    let flame_visual = if input.birth.build_id == 1003 {
        Some(input.flame_visual.unwrap_or_default())
    } else {
        None
    };
    NativeWeldChannelActorState {
        common: input.birth.actor(),
        enhanced_effects: input.enhanced_effects,
        flame_visual,
        endpoint: input.endpoint,
        midpoint: input.midpoint,
        target_id: input.target_id,
        underpowered: input.underpowered,
        variant: (input.birth.id % 4) as u32,
    }
}

#[cfg(feature = "persistence")]
fn enhanced_effects_default() -> bool {
    true
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum NativeWeldFlameLashFadeVariant {
    Chain,
    Endpoint,
    Source,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NativeWeldFlameLashFadeState {
    pub common: NativeWeldActorBase,
    pub alpha: f64,
    pub alpha_step: f64,
    pub base_scale: f64,
    pub color_green: f64,
    #[cfg_attr(feature = "persistence", serde(default))]
    pub flame_visual: NativeFlameFadeVisual,
    pub position: Vec2,
    pub record: usize,
    pub rotation_degrees: f64,
    pub variant: NativeWeldFlameLashFadeVariant,
    pub wrapper_scalar: f64,
}

pub struct NativeWeldFlameLashFadeInput<'a> {
    pub birth: NativeWeldActorBirth<'a>,
    pub alpha: f64,
    pub variant: NativeWeldFlameLashFadeVariant,
    pub enhanced_effects: bool,
}

pub fn create_native_weld_flame_lash_fade(
    input: NativeWeldFlameLashFadeInput<'_>,
    rng: &mut NativeRng,
    painters: Option<&mut WorldPainterOrder>,
) -> NativeWeldFlameLashFadeState {
    let birth = input.birth;
    let source = input.variant == NativeWeldFlameLashFadeVariant::Source;
    // The first rotation is overwritten, but still consumes its native word.
    rng.draw_scaled_float(360.0, false);
    let rotation = rng.draw_scaled_float(360.0, false);
    let color_green = if input.variant == NativeWeldFlameLashFadeVariant::Endpoint {
        fround(1.0 - rng.draw_scaled_float(0.5, false))
    } else if source {
        1.0
    } else {
        0.75
    };
    let radius = rng.draw_scaled_float(if source { 15.0 } else { 10.0 }, false);
    let heading = rng.draw_scaled_float(360.0, false);
    let scale = rng.draw_scaled_float(0.5, false);
    let wrapper_scalar =
        if source { 1.0 } else { fround(0.75 + rng.draw_scaled_float(0.75, false)) };
    let birth_alpha = if source { 0.5 } else { input.alpha };
    let alpha_step = fround(if !input.enhanced_effects {
        0.4
    } else if source {
        0.1
    } else {
        0.2
    });
    let radians = fround(heading * 3.141592502593994 / 180.0);
    let radial = Vec2::new(fround(sin(radians)), fround(-cos(radians)));
    let registration = if let Some(order) = painters {
        order.register(WorldManagerLane::Transient)
    } else {
        // Source standalone kernel callers seed both manager lanes from the ID.
        PainterRegistration {
            manager_lane: WorldManagerLane::Transient,
            registration_ordinal: birth.id as usize,
        }
    };
    let mut common = birth.actor();
    common.owner.painter_registrations.push(registration);
    common.owner.light_registration = if source { None } else { Some(registration) };
    NativeWeldFlameLashFadeState {
        common,
        alpha: native_flame_fade_alpha(birth_alpha, alpha_step, 0),
        alpha_step,
        base_scale: fround((if source { 0.75 } else { 0.5 }) + scale),
        color_green,
        flame_visual: NativeFlameFadeVisual::NativeBirth(NativeFlameFadeBirth { birth_alpha }),
        position: Vec2::new(
            fround(birth.origin.x + fround(radial.x * radius)),
            fround(birth.origin.y + fround(radial.y * radius)),
        ),
        record: 35,
        rotation_degrees: fround(rotation + 1.0),
        variant: input.variant,
        wrapper_scalar,
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NativeWeldGroundSparkFadeSeed {
    pub alpha: f64,
    pub alpha_step: f64,
    pub position: Vec2,
    pub record: usize,
    pub rotation_degrees: f64,
    pub scale: f64,
}

pub fn create_native_weld_ground_spark_fade_program(
    projectile: &NativeWeldProjectileState,
    rng: &mut NativeRng,
) -> Vec<NativeWeldGroundSparkFadeSeed> {
    if projectile.build_id != 1009 {
        return Vec::new();
    }
    let native_age = projectile.ground_spark_native_age_ticks.expect("GroundSpark owns native age");
    let phase = sin(native_age as f64 * 12.0 * std::f64::consts::PI / 180.0).abs();
    let rotation = rng.draw_scaled_float(360.0, false);
    let scale = rng.draw_scaled_float(0.1_f32 as f64, true);
    let weak_factor = if projectile.underpowered { 0.5 } else { 1.0 };
    let mut fades = vec![NativeWeldGroundSparkFadeSeed {
        alpha: fround(0.75 * weak_factor),
        alpha_step: fround(0.1_f32 as f64 * weak_factor),
        position: Vec2::new(
            projectile.common.position.x,
            fround(projectile.common.position.y - phase * 15.0),
        ),
        record: 71,
        rotation_degrees: rotation,
        scale: fround(scale + 0.35_f32 as f64),
    }];
    let low_phase = phase < 0.1_f32 as f64;
    let fork_due = low_phase || rng.draw_integer(6) == 1;
    if fork_due {
        let record = rng.draw_integer(4) as usize;
        let rotation = rng.draw_scaled_float(360.0, false);
        let alpha = rng.draw_scaled_float(0.25, false);
        let base_alpha = if low_phase {
            fround(fround(1.0 + alpha) * 0.5)
        } else {
            fround(fround(1.0 + alpha) + 0.5)
        };
        fades.push(NativeWeldGroundSparkFadeSeed {
            alpha: fround(base_alpha * weak_factor),
            alpha_step: 0.1_f32 as f64,
            position: Vec2::new(
                projectile.common.position.x,
                fround(projectile.common.position.y - phase * 20.0),
            ),
            record: 1836 + record,
            rotation_degrees: rotation,
            scale: if low_phase { 0.75 } else { 1.0 },
        });
    }
    fades
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NativeWeldGroundSparkFadeActorState {
    pub common: NativeWeldActorBase,
    pub seed: NativeWeldGroundSparkFadeSeed,
}

#[must_use]
pub fn create_native_weld_ground_spark_fade_actor(
    birth: NativeWeldActorBirth<'_>,
    seed: NativeWeldGroundSparkFadeSeed,
) -> NativeWeldGroundSparkFadeActorState {
    NativeWeldGroundSparkFadeActorState {
        common: NativeWeldActorBirth { origin: seed.position, ..birth }.actor(),
        seed,
    }
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NativeWeldImpactActorState {
    pub common: NativeWeldActorBase,
    pub alpha: f64,
    pub boulder_terminal_charge: Option<f64>,
    pub impact_sound_pitch: Option<f64>,
    pub impact_sound_variant: Option<u32>,
    pub position: Vec2,
    pub presentation_rotation_degrees: Option<f64>,
    pub presentation_scale: f64,
}

pub fn create_primary_spell_weld_impact(
    birth: NativeWeldActorBirth<'_>,
    rng: &mut NativeRng,
) -> NativeWeldImpactActorState {
    let mut pitch = None;
    let mut variant = None;
    let mut rotation = None;
    match birth.build_id {
        1001 => pitch = Some(1.5),
        1002 => {
            pitch = Some(1.5);
            variant = Some(0);
            rotation = Some(rng.draw_scaled_float(360.0, false));
        }
        1009 => {
            pitch = Some(fround(1.0 + rng.draw_scaled_float(0.1_f32 as f64, false)));
            variant = Some(rng.draw_integer(3) as u32);
            rotation = Some(rng.draw_scaled_float(360.0, false));
        }
        _ => {}
    }
    let owns_fade = matches!(birth.build_id, 1001 | 1002 | 1009);
    NativeWeldImpactActorState {
        common: birth.actor(),
        alpha: if owns_fade { 2.0 } else { 0.0 },
        boulder_terminal_charge: None,
        impact_sound_pitch: pitch,
        impact_sound_variant: variant,
        position: birth.origin,
        presentation_rotation_degrees: rotation,
        presentation_scale: if owns_fade { 1.5 } else { 0.0 },
    }
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NativeWeldBoulderDebrisActorState {
    pub common: NativeWeldActorBase,
    pub debris: NativeWeldBoulderDebrisParticleState,
    pub position: Vec2,
}

#[must_use]
pub fn create_native_weld_boulder_debris_actor(
    birth: NativeWeldActorBirth<'_>,
    debris: NativeWeldMeteorDebrisSeed,
) -> NativeWeldBoulderDebrisActorState {
    NativeWeldBoulderDebrisActorState {
        common: birth.actor(),
        debris: create_native_weld_boulder_debris_particle(debris, true),
        position: birth.origin,
    }
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NativeWeldHailRockFadeActorState {
    pub common: NativeWeldActorBase,
    pub position: Vec2,
    pub rotation_degrees: f64,
}

#[must_use]
pub fn create_native_weld_hail_rock_fade_actor(
    birth: NativeWeldActorBirth<'_>,
    position: Vec2,
    rotation_degrees: f64,
) -> NativeWeldHailRockFadeActorState {
    NativeWeldHailRockFadeActorState { common: birth.actor(), position, rotation_degrees }
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NativeWeldMeteorFlashActorState {
    pub common: NativeWeldActorBase,
    pub alpha: f64,
    pub alpha_step: f64,
    pub position: Vec2,
    pub record: usize,
    pub scale: f64,
}

#[must_use]
pub fn create_native_weld_meteor_flash(
    actor: &NativeWeldMeteorActorState,
    id: u64,
    tick: u64,
) -> NativeWeldMeteorFlashActorState {
    let birth = NativeWeldActorBirth {
        build_id: 1007,
        direction: actor.common.direction,
        id,
        origin: actor.position,
        owner_id: &actor.common.owner.owner_id,
        tick,
        vector: &actor.common.vector,
        world_key: &actor.common.owner.world_key,
    };
    NativeWeldMeteorFlashActorState {
        common: birth.actor(),
        alpha: 2.0,
        alpha_step: 0.1_f32 as f64,
        position: actor.position,
        record: 15,
        scale: 6.0,
    }
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum NativeWeldWorldActor {
    BlizzardChainFrost(NativeWeldBlizzardChainFrostState),
    BlizzardGlow(NativeWeldBlizzardGlowState),
    BoulderDebris(NativeWeldBoulderDebrisActorState),
    Channel(NativeWeldChannelActorState),
    FlameLashFade(NativeWeldFlameLashFadeState),
    FrostFade(NativeWeldFrostFadeActorState),
    GroundSparkFade(NativeWeldGroundSparkFadeActorState),
    HailChild(NativeWeldHailChildActorState),
    HailRockFade(NativeWeldHailRockFadeActorState),
    Impact(NativeWeldImpactActorState),
    Meteor(NativeWeldMeteorActorState),
    MeteorFlash(NativeWeldMeteorFlashActorState),
    MeteorMarker(NativeWeldMeteorMarkerState),
    Persistent(NativeWeldPersistentActorState),
    Steam(NativeWeldSteamActorState),
}

impl NativeWeldWorldActor {
    #[must_use]
    pub fn common(&self) -> &NativeWeldActorBase {
        match self {
            Self::BlizzardChainFrost(actor) => &actor.common,
            Self::BlizzardGlow(actor) => &actor.common,
            Self::BoulderDebris(actor) => &actor.common,
            Self::Channel(actor) => &actor.common,
            Self::FlameLashFade(actor) => &actor.common,
            Self::FrostFade(actor) => &actor.common,
            Self::GroundSparkFade(actor) => &actor.common,
            Self::HailChild(actor) => actor.common(),
            Self::HailRockFade(actor) => &actor.common,
            Self::Impact(actor) => &actor.common,
            Self::Meteor(actor) => &actor.common,
            Self::MeteorFlash(actor) => &actor.common,
            Self::MeteorMarker(actor) => &actor.common,
            Self::Persistent(actor) => actor.common(),
            Self::Steam(actor) => &actor.common,
        }
    }

    pub fn common_mut(&mut self) -> &mut NativeWeldActorBase {
        match self {
            Self::BlizzardChainFrost(actor) => &mut actor.common,
            Self::BlizzardGlow(actor) => &mut actor.common,
            Self::BoulderDebris(actor) => &mut actor.common,
            Self::Channel(actor) => &mut actor.common,
            Self::FlameLashFade(actor) => &mut actor.common,
            Self::FrostFade(actor) => &mut actor.common,
            Self::GroundSparkFade(actor) => &mut actor.common,
            Self::HailChild(actor) => actor.common_mut(),
            Self::HailRockFade(actor) => &mut actor.common,
            Self::Impact(actor) => &mut actor.common,
            Self::Meteor(actor) => &mut actor.common,
            Self::MeteorFlash(actor) => &mut actor.common,
            Self::MeteorMarker(actor) => &mut actor.common,
            Self::Persistent(actor) => actor.common_mut(),
            Self::Steam(actor) => &mut actor.common,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeWeldWorldActorStep {
    pub alive: bool,
    pub debris: Vec<NativeWeldMeteorDebrisSeed>,
    pub terrain_contact: Option<NativeWeldEtherealBoulderState>,
}

pub fn step_native_weld_world_actor(
    actor: &mut NativeWeldWorldActor,
    rng: &mut NativeRng,
    can_advance: &NativeWeldPersistentTraversal<'_>,
) -> NativeWeldWorldActorStep {
    let mut debris = Vec::new();
    let mut terrain_contact = None;
    let alive = match actor {
        NativeWeldWorldActor::BlizzardChainFrost(actor) => {
            let lifetime = water_frost_jet_lifetime_ticks(actor.common.owner.id);
            step_visible_age(&mut actor.common, lifetime)
        }
        NativeWeldWorldActor::BlizzardGlow(_) => false,
        NativeWeldWorldActor::FlameLashFade(actor) => {
            let alive = step_alpha(&mut actor.common, &mut actor.alpha, actor.alpha_step);
            if alive {
                actor.rotation_degrees = fround(actor.rotation_degrees + 1.0);
            }
            alive
        }
        NativeWeldWorldActor::HailChild(actor) => step_native_weld_hail_child(actor, rng),
        NativeWeldWorldActor::BoulderDebris(actor) => {
            let global_tick = actor.common.birth_tick + actor.common.owner.age_ticks + 1;
            if let Some(body) =
                step_native_weld_boulder_debris_particle(actor.debris, global_tick, rng)
            {
                actor.common.owner.age_ticks += 1;
                actor.debris = body;
                true
            } else {
                false
            }
        }
        NativeWeldWorldActor::HailRockFade(actor) => step_visible_age(&mut actor.common, 400),
        NativeWeldWorldActor::FrostFade(actor) => {
            let lifetime = if actor.common.build_id == 1004 { 3 } else { 20 };
            step_visible_age(&mut actor.common, lifetime)
        }
        NativeWeldWorldActor::GroundSparkFade(actor) => {
            step_alpha(&mut actor.common, &mut actor.seed.alpha, actor.seed.alpha_step)
        }
        NativeWeldWorldActor::MeteorMarker(actor) => step_native_weld_meteor_marker(actor),
        NativeWeldWorldActor::MeteorFlash(actor) => {
            step_alpha(&mut actor.common, &mut actor.alpha, actor.alpha_step)
        }
        NativeWeldWorldActor::Channel(actor) => step_visible_age(&mut actor.common, 2),
        NativeWeldWorldActor::Steam(actor) => step_native_weld_steam_actor(actor),
        NativeWeldWorldActor::Impact(actor) => {
            actor.alpha =
                if actor.alpha > 0.0 { fround(actor.alpha - 0.1_f32 as f64) } else { 0.0 };
            actor.alpha >= 0.0 && step_visible_age(&mut actor.common, 20)
        }
        NativeWeldWorldActor::Meteor(actor) => {
            let result = step_native_weld_meteor(actor, rng);
            debris = result.1;
            result.0
        }
        NativeWeldWorldActor::Persistent(actor) => {
            let result = step_native_weld_persistent_actor(actor, can_advance);
            terrain_contact = result.1;
            result.0
        }
    };
    NativeWeldWorldActorStep { alive, debris, terrain_contact }
}

pub fn create_primary_spell_weld_boulder_terminal(
    first_id: u64,
    actor: &NativeWeldEtherealBoulderState,
    tick: u64,
    rng: &mut NativeRng,
    painters: &mut WorldPainterOrder,
) -> Vec<NativeWeldWorldActor> {
    let birth = NativeWeldActorBirth {
        build_id: 1006,
        direction: actor.common.direction,
        id: first_id,
        origin: actor.common.origin,
        owner_id: &actor.common.owner.owner_id,
        tick,
        vector: &actor.common.vector,
        world_key: &actor.common.owner.world_key,
    };
    let mut common = birth.actor();
    common.owner.light_registration = Some(painters.register(WorldManagerLane::Transient));
    let mut actors = vec![NativeWeldWorldActor::Impact(NativeWeldImpactActorState {
        common,
        alpha: 2.0,
        boulder_terminal_charge: Some(actor.scale),
        impact_sound_pitch: None,
        impact_sound_variant: None,
        position: actor.common.origin,
        presentation_rotation_degrees: None,
        presentation_scale: 2.0,
    })];
    let debris = create_native_weld_ethereal_boulder_breakup_debris_program(actor.scale, rng);
    for body in debris {
        let birth = NativeWeldActorBirth { id: first_id + actors.len() as u64, ..birth };
        actors.push(NativeWeldWorldActor::BoulderDebris(create_native_weld_boulder_debris_actor(
            birth, body,
        )));
    }
    actors
}

fn step_visible_age(common: &mut NativeWeldActorBase, lifetime: u64) -> bool {
    if common.owner.age_ticks + 1 >= lifetime {
        return false;
    }
    common.owner.age_ticks += 1;
    true
}

fn step_alpha(common: &mut NativeWeldActorBase, alpha: &mut f64, alpha_step: f64) -> bool {
    *alpha = fround(*alpha - alpha_step);
    if *alpha <= 0.0 {
        return false;
    }
    common.owner.age_ticks += 1;
    true
}
