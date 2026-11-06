//! Weld world actors: the queue entry for every kind, plus the constructors and
//! per-tick steps of channels, Flame Lash fades, ground sparks, impacts, debris and
//! Meteor flashes.

use super::blizzard::{WeldBlizzardChainFrostState, WeldBlizzardGlowState};
use super::flame_state::{FlameChannelVisual, FlameFadeBirth, FlameFadeVisual, flame_fade_alpha};
use super::hail::{WeldHailChildActorState, step_weld_hail_child};
use super::meteor::{
    WeldMeteorActorState, WeldMeteorMarkerState, step_weld_meteor, step_weld_meteor_marker,
};
use super::model::{WeldActorBase, WeldActorBirth};
use super::persistent::{
    WeldEtherealBoulderState, WeldFrostFadeActorState, WeldPersistentActorState,
    WeldPersistentTraversal, step_weld_persistent_actor,
};
use super::steam::{WeldSteamActorState, step_weld_steam_actor};
use crate::float::{cos, fround, sin};
use crate::painter::{PainterRegistration, WorldManagerLane, WorldPainterOrder};
use crate::spells::earth_debris::{
    WeldBoulderDebrisParticleState, WeldMeteorDebrisSeed, create_weld_boulder_debris_particle,
    create_weld_ethereal_boulder_breakup_debris_program, step_weld_boulder_debris_particle,
};
use crate::spells::water::water_frost_jet_lifetime_ticks;
use crate::{Rng, Vec2};

/// The visible beam of a channel Weld: its endpoint, midpoint, target, power and any
/// Flame Lash visual.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct WeldChannelActorState {
    pub common: WeldActorBase,
    #[cfg_attr(feature = "persistence", serde(default = "enhanced_effects_default"))]
    pub enhanced_effects: bool,
    #[cfg_attr(feature = "persistence", serde(default, skip_serializing_if = "Option::is_none"))]
    pub flame_visual: Option<FlameChannelVisual>,
    pub endpoint: Option<Vec2>,
    pub midpoint: Option<Vec2>,
    pub target_id: Option<String>,
    pub underpowered: bool,
    pub variant: u32,
}

/// Inputs for creating a channel Weld's beam actor.
pub struct WeldChannelInput<'a> {
    pub birth: WeldActorBirth<'a>,
    pub endpoint: Option<Vec2>,
    pub midpoint: Option<Vec2>,
    pub target_id: Option<String>,
    pub underpowered: bool,
    pub enhanced_effects: bool,
    pub flame_visual: Option<FlameChannelVisual>,
}

#[must_use]
pub fn create_weld_channel_actor(input: WeldChannelInput<'_>) -> WeldChannelActorState {
    let flame_visual = if input.birth.build_id == 1003 {
        Some(input.flame_visual.unwrap_or_default())
    } else {
        None
    };
    WeldChannelActorState {
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

/// Where a Flame Lash fade appears: at the lash's source, at its endpoint, or on a chain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum WeldFlameLashFadeVariant {
    Chain,
    Endpoint,
    Source,
}

/// A fading flame sprite of a Flame Lash: position, rotation, scale, color and alpha.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct WeldFlameLashFadeState {
    pub common: WeldActorBase,
    pub alpha: f64,
    pub alpha_step: f64,
    pub base_scale: f64,
    pub color_green: f64,
    #[cfg_attr(feature = "persistence", serde(default))]
    pub flame_visual: FlameFadeVisual,
    pub position: Vec2,
    pub record: usize,
    pub rotation_degrees: f64,
    pub variant: WeldFlameLashFadeVariant,
    pub wrapper_scalar: f64,
}

/// Inputs for creating a Flame Lash fade.
pub struct WeldFlameLashFadeInput<'a> {
    pub birth: WeldActorBirth<'a>,
    pub alpha: f64,
    pub variant: WeldFlameLashFadeVariant,
    pub enhanced_effects: bool,
}

pub fn create_weld_flame_lash_fade(
    input: WeldFlameLashFadeInput<'_>,
    rng: &mut Rng,
    painters: Option<&mut WorldPainterOrder>,
) -> WeldFlameLashFadeState {
    let birth = input.birth;
    let source = input.variant == WeldFlameLashFadeVariant::Source;
    // The first rotation is overwritten, but still consumes its native word.
    rng.draw_scaled_float(360.0, false);
    let rotation = rng.draw_scaled_float(360.0, false);
    let color_green = if input.variant == WeldFlameLashFadeVariant::Endpoint {
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
        // Callers without a painter order use the actor's ID as its registration ordinal.
        PainterRegistration {
            manager_lane: WorldManagerLane::Transient,
            registration_ordinal: birth.id as usize,
        }
    };
    let mut common = birth.actor();
    common.owner.painter_registrations.push(registration);
    common.owner.light_registration = if source { None } else { Some(registration) };
    WeldFlameLashFadeState {
        common,
        alpha: flame_fade_alpha(birth_alpha, alpha_step, 0),
        alpha_step,
        base_scale: fround((if source { 0.75 } else { 0.5 }) + scale),
        color_green,
        flame_visual: FlameFadeVisual::Birth(FlameFadeBirth { birth_alpha }),
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

/// One fading spark left by Crawling Shock's ground spark: alpha, position, record,
/// rotation and scale.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct WeldGroundSparkFadeSeed {
    pub alpha: f64,
    pub alpha_step: f64,
    pub position: Vec2,
    pub record: usize,
    pub rotation_degrees: f64,
    pub scale: f64,
}

pub fn create_weld_ground_spark_fade_program(
    projectile: &super::WeldProjectileState,
    rng: &mut Rng,
) -> Vec<WeldGroundSparkFadeSeed> {
    if projectile.build_id != 1009 {
        return Vec::new();
    }
    let age = projectile.ground_spark_age_ticks.expect("GroundSpark owns native age");
    let phase = sin(age as f64 * 12.0 * std::f64::consts::PI / 180.0).abs();
    let rotation = rng.draw_scaled_float(360.0, false);
    let scale = rng.draw_scaled_float(0.1_f32 as f64, true);
    let weak_factor = if projectile.underpowered { 0.5 } else { 1.0 };
    let mut fades = vec![WeldGroundSparkFadeSeed {
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
        fades.push(WeldGroundSparkFadeSeed {
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

/// A Crawling Shock spark fade in the world.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct WeldGroundSparkFadeActorState {
    pub common: WeldActorBase,
    pub seed: WeldGroundSparkFadeSeed,
}

#[must_use]
pub fn create_weld_ground_spark_fade_actor(
    birth: WeldActorBirth<'_>,
    seed: WeldGroundSparkFadeSeed,
) -> WeldGroundSparkFadeActorState {
    WeldGroundSparkFadeActorState {
        common: WeldActorBirth { origin: seed.position, ..birth }.actor(),
        seed,
    }
}

/// The impact a Weld projectile leaves where it hits: its fade, sound and, for a
/// boulder, the terminal charge.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct WeldImpactActorState {
    pub common: WeldActorBase,
    pub alpha: f64,
    pub boulder_terminal_charge: Option<f64>,
    pub impact_sound_pitch: Option<f64>,
    pub impact_sound_variant: Option<u32>,
    pub position: Vec2,
    pub presentation_rotation_degrees: Option<f64>,
    pub presentation_scale: f64,
}

pub fn create_primary_spell_weld_impact(
    birth: WeldActorBirth<'_>,
    rng: &mut Rng,
) -> WeldImpactActorState {
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
    WeldImpactActorState {
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

/// A Weld boulder debris bit in the world.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct WeldBoulderDebrisActorState {
    pub common: WeldActorBase,
    pub debris: WeldBoulderDebrisParticleState,
    pub position: Vec2,
}

#[must_use]
pub fn create_weld_boulder_debris_actor(
    birth: WeldActorBirth<'_>,
    debris: WeldMeteorDebrisSeed,
) -> WeldBoulderDebrisActorState {
    WeldBoulderDebrisActorState {
        common: birth.actor(),
        debris: create_weld_boulder_debris_particle(debris, true),
        position: birth.origin,
    }
}

/// A rock fade that a held Hailstones cluster sheds as it grows, visible for 400 ticks.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct WeldHailRockFadeActorState {
    pub common: WeldActorBase,
    pub position: Vec2,
    pub rotation_degrees: f64,
}

#[must_use]
pub fn create_weld_hail_rock_fade_actor(
    birth: WeldActorBirth<'_>,
    position: Vec2,
    rotation_degrees: f64,
) -> WeldHailRockFadeActorState {
    WeldHailRockFadeActorState { common: birth.actor(), position, rotation_degrees }
}

/// The fading flash of a Meteor impact.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct WeldMeteorFlashActorState {
    pub common: WeldActorBase,
    pub alpha: f64,
    pub alpha_step: f64,
    pub position: Vec2,
    pub record: usize,
    pub scale: f64,
}

#[must_use]
pub fn create_weld_meteor_flash(
    actor: &WeldMeteorActorState,
    id: u64,
    tick: u64,
) -> WeldMeteorFlashActorState {
    let birth = WeldActorBirth {
        build_id: 1007,
        direction: actor.common.direction,
        id,
        origin: actor.position,
        owner_id: &actor.common.owner.owner_id,
        tick,
        vector: &actor.common.vector,
        world_key: &actor.common.owner.world_key,
    };
    WeldMeteorFlashActorState {
        common: birth.actor(),
        alpha: 2.0,
        alpha_step: 0.1_f32 as f64,
        position: actor.position,
        record: 15,
        scale: 6.0,
    }
}

/// A Weld world actor of any kind, kept in the shared transient queue.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum WeldWorldActor {
    BlizzardChainFrost(WeldBlizzardChainFrostState),
    BlizzardGlow(WeldBlizzardGlowState),
    BoulderDebris(WeldBoulderDebrisActorState),
    Channel(WeldChannelActorState),
    FlameLashFade(WeldFlameLashFadeState),
    FrostFade(WeldFrostFadeActorState),
    GroundSparkFade(WeldGroundSparkFadeActorState),
    HailChild(WeldHailChildActorState),
    HailRockFade(WeldHailRockFadeActorState),
    Impact(WeldImpactActorState),
    Meteor(WeldMeteorActorState),
    MeteorFlash(WeldMeteorFlashActorState),
    MeteorMarker(WeldMeteorMarkerState),
    Persistent(WeldPersistentActorState),
    Steam(WeldSteamActorState),
}

impl WeldWorldActor {
    #[must_use]
    pub fn common(&self) -> &WeldActorBase {
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

    pub fn common_mut(&mut self) -> &mut WeldActorBase {
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

/// One Weld actor tick: whether it lives, the debris it threw, and an Ethereal Boulder
/// that reached terrain.
#[derive(Clone, Debug, PartialEq)]
pub struct WeldWorldActorStep {
    pub alive: bool,
    pub debris: Vec<WeldMeteorDebrisSeed>,
    pub terrain_contact: Option<WeldEtherealBoulderState>,
}

pub fn step_weld_world_actor(
    actor: &mut WeldWorldActor,
    rng: &mut Rng,
    can_advance: &WeldPersistentTraversal<'_>,
) -> WeldWorldActorStep {
    let mut debris = Vec::new();
    let mut terrain_contact = None;
    let alive = match actor {
        WeldWorldActor::BlizzardChainFrost(actor) => {
            let lifetime = water_frost_jet_lifetime_ticks(actor.common.owner.id);
            step_visible_age(&mut actor.common, lifetime)
        }
        WeldWorldActor::BlizzardGlow(_) => false,
        WeldWorldActor::FlameLashFade(actor) => {
            let alive = step_alpha(&mut actor.common, &mut actor.alpha, actor.alpha_step);
            if alive {
                actor.rotation_degrees = fround(actor.rotation_degrees + 1.0);
            }
            alive
        }
        WeldWorldActor::HailChild(actor) => step_weld_hail_child(actor, rng),
        WeldWorldActor::BoulderDebris(actor) => {
            let global_tick = actor.common.birth_tick + actor.common.owner.age_ticks + 1;
            if let Some(body) = step_weld_boulder_debris_particle(actor.debris, global_tick, rng) {
                actor.common.owner.age_ticks += 1;
                actor.debris = body;
                true
            } else {
                false
            }
        }
        WeldWorldActor::HailRockFade(actor) => step_visible_age(&mut actor.common, 400),
        WeldWorldActor::FrostFade(actor) => {
            let lifetime = if actor.common.build_id == 1004 { 3 } else { 20 };
            step_visible_age(&mut actor.common, lifetime)
        }
        WeldWorldActor::GroundSparkFade(actor) => {
            step_alpha(&mut actor.common, &mut actor.seed.alpha, actor.seed.alpha_step)
        }
        WeldWorldActor::MeteorMarker(actor) => step_weld_meteor_marker(actor),
        WeldWorldActor::MeteorFlash(actor) => {
            step_alpha(&mut actor.common, &mut actor.alpha, actor.alpha_step)
        }
        WeldWorldActor::Channel(actor) => step_visible_age(&mut actor.common, 2),
        WeldWorldActor::Steam(actor) => step_weld_steam_actor(actor),
        WeldWorldActor::Impact(actor) => {
            actor.alpha =
                if actor.alpha > 0.0 { fround(actor.alpha - 0.1_f32 as f64) } else { 0.0 };
            actor.alpha >= 0.0 && step_visible_age(&mut actor.common, 20)
        }
        WeldWorldActor::Meteor(actor) => {
            let result = step_weld_meteor(actor, rng);
            debris = result.1;
            result.0
        }
        WeldWorldActor::Persistent(actor) => {
            let result = step_weld_persistent_actor(actor, can_advance);
            terrain_contact = result.1;
            result.0
        }
    };
    WeldWorldActorStep { alive, debris, terrain_contact }
}

pub fn create_primary_spell_weld_boulder_terminal(
    first_id: u64,
    actor: &WeldEtherealBoulderState,
    tick: u64,
    rng: &mut Rng,
    painters: &mut WorldPainterOrder,
) -> Vec<WeldWorldActor> {
    let birth = WeldActorBirth {
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
    let mut actors = vec![WeldWorldActor::Impact(WeldImpactActorState {
        common,
        alpha: 2.0,
        boulder_terminal_charge: Some(actor.scale),
        impact_sound_pitch: None,
        impact_sound_variant: None,
        position: actor.common.origin,
        presentation_rotation_degrees: None,
        presentation_scale: 2.0,
    })];
    let debris = create_weld_ethereal_boulder_breakup_debris_program(actor.scale, rng);
    for body in debris {
        let birth = WeldActorBirth { id: first_id + actors.len() as u64, ..birth };
        actors.push(WeldWorldActor::BoulderDebris(create_weld_boulder_debris_actor(birth, body)));
    }
    actors
}

fn step_visible_age(common: &mut WeldActorBase, lifetime: u64) -> bool {
    if common.owner.age_ticks + 1 >= lifetime {
        return false;
    }
    common.owner.age_ticks += 1;
    true
}

fn step_alpha(common: &mut WeldActorBase, alpha: &mut f64, alpha_step: f64) -> bool {
    *alpha = fround(*alpha - alpha_step);
    if *alpha <= 0.0 {
        return false;
    }
    common.owner.age_ticks += 1;
    true
}
