//! Primary spell actors from `core-kernels/primary-spells.ts` and
//! `primary-spell-fire-effects.ts`. Family modules own the rules; these rows retain
//! identity, painter ownership and the fields shared with rendering and contacts.

use super::earth_debris::NativeWeldBoulderDebrisParticleState;
use super::weld::{NativeFlameLashFactoryInput, NativeWeldProjectileState, NativeWeldWorldActor};
use crate::enemies::path::NativeEnemyPathState;
use crate::harden_effects::NativeHardenEffect;
use crate::painter::PainterRegistration;
use crate::profile::primary::{NativePrimarySkillProfile, NativeSpentEmber};
use crate::{NativeRng, Vec2};

pub use super::projectile_storage::{
    PrimaryProjectilePartsMut, PrimaryProjectileRead, PrimaryProjectileRef, PrimaryProjectiles,
    ProjectileMut, ProjectileRef,
};

pub use super::transient_storage::{PrimaryTransientMut, PrimaryTransientRef, PrimaryTransients};

pub type EarthBoulderOrientation = [f64; 9];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum PrimarySpellProjectilePhase {
    Flight,
    Held,
}

#[derive(Clone, Debug, PartialEq, bevy_ecs::component::Component)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct PrimarySpellProjectileBase {
    pub age_ticks: u64,
    pub charge: f64,
    pub damage: f64,
    pub direction: Vec2,
    pub flight_ticks: u64,
    pub id: u64,
    /// The light and single actor painter share one registration.
    pub registration: PainterRegistration,
    pub owner_id: String,
    pub phase: PrimarySpellProjectilePhase,
    pub position: Vec2,
    pub velocity: Vec2,
    pub world_key: String,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(
    feature = "persistence",
    derive(serde::Deserialize),
    serde(from = "PrimarySpellEarthProjectileFields")
)]
pub struct PrimarySpellEarthProjectile {
    pub common: PrimarySpellProjectileBase,
    pub program: EarthProjectileProgram,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EarthProjectileProgram {
    pub assembly_charge: f64,
    pub hit_target_ids: Vec<String>,
    pub maximum_charge: f64,
    pub orientation: EarthBoulderOrientation,
    pub remaining_damage: f64,
    pub shell_charge: f64,
    pub toughness: f64,
}

#[cfg(feature = "persistence")]
#[derive(serde::Deserialize)]
#[serde(rename = "PrimarySpellEarthProjectile")]
struct PrimarySpellEarthProjectileFields {
    common: PrimarySpellProjectileBase,
    assembly_charge: f64,
    hit_target_ids: Vec<String>,
    maximum_charge: f64,
    orientation: EarthBoulderOrientation,
    remaining_damage: f64,
    shell_charge: f64,
    toughness: f64,
}

#[cfg(feature = "persistence")]
impl From<PrimarySpellEarthProjectileFields> for PrimarySpellEarthProjectile {
    fn from(row: PrimarySpellEarthProjectileFields) -> Self {
        Self {
            common: row.common,
            program: EarthProjectileProgram {
                assembly_charge: row.assembly_charge,
                hit_target_ids: row.hit_target_ids,
                maximum_charge: row.maximum_charge,
                orientation: row.orientation,
                remaining_damage: row.remaining_damage,
                shell_charge: row.shell_charge,
                toughness: row.toughness,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(
    feature = "persistence",
    derive(serde::Deserialize),
    serde(from = "PrimarySpellEtherProjectileFields")
)]
pub struct PrimarySpellEtherProjectile {
    pub common: PrimarySpellProjectileBase,
    pub program: EtherProjectileProgram,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EtherProjectileProgram {
    pub damage_retention: f64,
    pub heading_degrees: f64,
    pub pierces_remaining: u32,
    pub reacquires_target: bool,
    pub speed: f64,
    pub target_id: Option<String>,
    pub turn_input: f64,
    pub turn_accumulator: f64,
    pub underpowered: bool,
    pub visual_scale: f64,
}

#[cfg(feature = "persistence")]
#[derive(serde::Deserialize)]
#[serde(rename = "PrimarySpellEtherProjectile")]
struct PrimarySpellEtherProjectileFields {
    common: PrimarySpellProjectileBase,
    damage_retention: f64,
    heading_degrees: f64,
    pierces_remaining: u32,
    reacquires_target: bool,
    speed: f64,
    target_id: Option<String>,
    turn_input: f64,
    turn_accumulator: f64,
    underpowered: bool,
    visual_scale: f64,
}

#[cfg(feature = "persistence")]
impl From<PrimarySpellEtherProjectileFields> for PrimarySpellEtherProjectile {
    fn from(row: PrimarySpellEtherProjectileFields) -> Self {
        Self {
            common: row.common,
            program: EtherProjectileProgram {
                damage_retention: row.damage_retention,
                heading_degrees: row.heading_degrees,
                pierces_remaining: row.pierces_remaining,
                reacquires_target: row.reacquires_target,
                speed: row.speed,
                target_id: row.target_id,
                turn_input: row.turn_input,
                turn_accumulator: row.turn_accumulator,
                underpowered: row.underpowered,
                visual_scale: row.visual_scale,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(
    feature = "persistence",
    derive(serde::Deserialize),
    serde(from = "PrimarySpellFireProjectileFields")
)]
pub struct PrimarySpellFireProjectile {
    pub common: PrimarySpellProjectileBase,
    pub program: FireProjectileProgram,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FireProjectileProgram {
    pub burn_damage: f64,
    pub ember_damage: f64,
    pub ember_fragments: u32,
    pub explode_damage: f64,
    pub explode_radius: f64,
    pub private_seed: i32,
    pub spent_ember: NativeSpentEmber,
    pub underpowered: bool,
}

#[cfg(feature = "persistence")]
#[derive(serde::Deserialize)]
#[serde(rename = "PrimarySpellFireProjectile")]
struct PrimarySpellFireProjectileFields {
    common: PrimarySpellProjectileBase,
    burn_damage: f64,
    ember_damage: f64,
    ember_fragments: u32,
    explode_damage: f64,
    explode_radius: f64,
    private_seed: i32,
    spent_ember: NativeSpentEmber,
    underpowered: bool,
}

#[cfg(feature = "persistence")]
impl From<PrimarySpellFireProjectileFields> for PrimarySpellFireProjectile {
    fn from(row: PrimarySpellFireProjectileFields) -> Self {
        Self {
            common: row.common,
            program: FireProjectileProgram {
                burn_damage: row.burn_damage,
                ember_damage: row.ember_damage,
                ember_fragments: row.ember_fragments,
                explode_damage: row.explode_damage,
                explode_radius: row.explode_radius,
                private_seed: row.private_seed,
                spent_ember: row.spent_ember,
                underpowered: row.underpowered,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum PrimarySpellProjectile {
    Weld(NativeWeldProjectileState),
    Earth(PrimarySpellEarthProjectile),
    Ether(PrimarySpellEtherProjectile),
    Fire(PrimarySpellFireProjectile),
}

impl PrimarySpellProjectile {
    #[must_use]
    pub fn common(&self) -> &PrimarySpellProjectileBase {
        match self {
            Self::Weld(spell) => &spell.common,
            Self::Earth(spell) => &spell.common,
            Self::Ether(spell) => &spell.common,
            Self::Fire(spell) => &spell.common,
        }
    }
    pub fn common_mut(&mut self) -> &mut PrimarySpellProjectileBase {
        match self {
            Self::Weld(spell) => &mut spell.common,
            Self::Earth(spell) => &mut spell.common,
            Self::Ether(spell) => &mut spell.common,
            Self::Fire(spell) => &mut spell.common,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct PrimarySpellTransientBase {
    pub age_ticks: u64,
    pub id: u64,
    pub light_registration: Option<PainterRegistration>,
    pub owner_id: String,
    pub painter_registrations: Vec<PainterRegistration>,
    pub world_key: String,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(bevy_ecs::component::Component)]
pub struct PrimarySpellAirTransient {
    pub common: PrimarySpellTransientBase,
    #[cfg_attr(feature = "persistence", serde(default))]
    pub chained: bool,
    #[cfg_attr(feature = "persistence", serde(default = "primary_effects_enhanced_default"))]
    pub enhanced_effects: bool,
    pub birth_tick: u64,
    pub direction: Vec2,
    pub endpoint: Vec2,
    pub hurricane_charge: f64,
    pub midpoint: Vec2,
    pub origin: Vec2,
    pub target_id: Option<String>,
    pub underpowered: bool,
    pub variant: u32,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(bevy_ecs::component::Component)]
pub struct PrimarySpellWaterTransient {
    pub common: PrimarySpellTransientBase,
    pub direction: Vec2,
    pub obstruction_distance: Option<f64>,
    pub obstruction_point: Option<Vec2>,
    pub origin: Vec2,
    pub speed: f64,
    pub underpowered: bool,
    pub variant: u32,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(bevy_ecs::component::Component)]
pub struct PrimarySpellEarthImpact {
    pub common: PrimarySpellTransientBase,
    pub birth_tick: u64,
    pub charge: f64,
    #[cfg_attr(feature = "persistence", serde(default = "default_earth_impact_enhanced_effects"))]
    pub enhanced_effects: bool,
    pub lifetime_ticks: u64,
    pub origin: Vec2,
}

#[cfg(feature = "persistence")]
const fn default_earth_impact_enhanced_effects() -> bool {
    true
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(bevy_ecs::component::Component)]
pub struct PrimarySpellEarthBoulderBit {
    pub common: PrimarySpellTransientBase,
    pub birth_tick: u64,
    pub debris: NativeWeldBoulderDebrisParticleState,
    pub origin: Vec2,
    pub position: Vec2,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(bevy_ecs::component::Component)]
pub struct PrimarySpellEarthCalledRock {
    pub common: PrimarySpellTransientBase,
    pub fall_velocity: f64,
    pub falling: bool,
    pub height: f64,
    pub lateral_magnitude: f64,
    pub parent_id: u64,
    pub position: Vec2,
    pub rotation: f64,
    pub rotation_step: f64,
    pub scale: f64,
    pub speed: f64,
    pub target_height: f64,
    pub variant: u32,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(bevy_ecs::component::Component)]
pub struct PrimarySpellFireParticle {
    pub common: PrimarySpellTransientBase,
    #[cfg_attr(feature = "persistence", serde(default = "primary_effects_enhanced_default"))]
    pub enhanced_effects: bool,
    pub direction: Vec2,
    pub origin: Vec2,
    pub variant: u32,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(bevy_ecs::component::Component)]
pub struct PrimarySpellEtherImpact {
    pub common: PrimarySpellTransientBase,
    pub birth_tick: u64,
    pub origin: Vec2,
    pub visual_scale: f64,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(bevy_ecs::component::Component)]
pub struct PrimarySpellEtherPierceStreak {
    pub common: PrimarySpellTransientBase,
    pub heading_degrees: f64,
    pub origin: Vec2,
    pub visual_scale: f64,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(bevy_ecs::component::Component)]
pub struct PrimarySpellEtherBlast {
    pub common: PrimarySpellTransientBase,
    pub birth_tick: u64,
    pub charges: u32,
    pub origin: Vec2,
    pub presentation_rng: NativeRng,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(bevy_ecs::component::Component)]
pub struct PrimarySpellFireImpact {
    pub common: PrimarySpellTransientBase,
    pub origin: Vec2,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(bevy_ecs::component::Component)]
pub struct PrimarySpellFireEmber {
    pub common: PrimarySpellTransientBase,
    pub burn_damage: f64,
    pub contact_cadence: u64,
    pub contact_due: bool,
    pub damage: f64,
    pub height: f64,
    pub horizontal_velocity: Vec2,
    pub life: f64,
    pub phase: f64,
    pub position: Vec2,
    pub spent_ember: NativeSpentEmber,
    pub vertical_velocity: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum FireExplosionPresentation {
    Fire,
    Steam,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(bevy_ecs::component::Component)]
pub struct PrimarySpellFireExplosion {
    pub common: PrimarySpellTransientBase,
    pub burn_damage: f64,
    pub damage: f64,
    pub footprint_dimension: f64,
    pub origin: Vec2,
    pub presentation: FireExplosionPresentation,
    pub sound_pitch: f64,
    pub visual_scale: f64,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(bevy_ecs::component::Component)]
pub struct PrimarySpellFireGoodImp {
    pub common: PrimarySpellTransientBase,
    pub body_rotation_degrees: f64,
    pub body_scale: f64,
    pub body_variant: u32,
    pub bounce_sound_index: u32,
    pub bounce_sound_pitch: f64,
    pub bounce_sound_sequence: u64,
    pub burn_damage: f64,
    pub collision_radius: f64,
    pub contact_age_ticks: Option<u64>,
    pub contact_origin: Option<Vec2>,
    pub contact_scale: f64,
    pub contact_sound_index: u32,
    pub contact_sound_pitch: f64,
    pub contact_sound_sequence: u64,
    pub damage: f64,
    pub effect_alpha: f64,
    pub effect_phase: f64,
    pub flight_speed: f64,
    pub heading_degrees: f64,
    pub light_glow: f64,
    pub next_target_refresh_tick: u64,
    pub path: NativeEnemyPathState,
    pub position: Vec2,
    pub remaining_ticks: u64,
    pub target_id: Option<String>,
    pub vertical_offset: f64,
    pub vertical_velocity: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum FirePatchType {
    Fire,
    GoodGuy,
    Moving,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(bevy_ecs::component::Component)]
pub struct PrimarySpellFirePatch {
    pub common: PrimarySpellTransientBase,
    pub atlas_phase: f64,
    pub atlas_phase_step: f64,
    pub burn_damage: f64,
    pub damage: f64,
    pub draw_alpha: f64,
    pub fade_alpha: f64,
    pub life: f64,
    pub native_type: FirePatchType,
    pub position: Vec2,
    pub horizontal_sign: f64,
    pub scale: f64,
    pub supplemental_contact: bool,
    pub velocity: Vec2,
    pub velocity_multiplier: Vec2,
}

/// Player-owned Hurricane is synchronized by the Air/Water host, not the generic effect clock.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(bevy_ecs::component::Component)]
pub struct PrimarySpellAirHurricane {
    pub common: PrimarySpellTransientBase,
    pub birth_tick: u64,
    pub charge: f64,
    pub contact_charge: f64,
    pub damage_maximum: f64,
    pub damage_minimum: f64,
    pub enhanced_effects: bool,
    pub lanes: [crate::hurricane::NativeHurricaneLane; 8],
    pub phase_degrees: f64,
    pub position: Vec2,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(bevy_ecs::component::Component)]
pub struct PrimarySpellWaterAura {
    pub common: PrimarySpellTransientBase,
    pub alpha_decay: f64,
    pub birth_tick: u64,
    pub duration_ticks: u64,
    pub initial_rotation_degrees: f64,
    pub origin: Vec2,
    pub rotation_step_degrees: f64,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(bevy_ecs::component::Component)]
pub struct PrimarySpellWaterHail {
    pub common: PrimarySpellTransientBase,
    pub birth_tick: u64,
    pub bounce_progress: f64,
    pub bounce_sound_index: Option<u32>,
    pub bounce_sound_pitch: Option<f64>,
    pub bounce_sound_sequence: u64,
    pub height: f64,
    pub horizontal_velocity: Vec2,
    pub life: f64,
    pub position: Vec2,
    pub rotation_degrees: f64,
    pub rotation_step_degrees: f64,
    pub saved_bounce_velocity: f64,
    pub scale: f64,
    pub vertical_velocity: f64,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum PrimarySpellTransient {
    AirHurricane(PrimarySpellAirHurricane),
    WaterAura(PrimarySpellWaterAura),
    WaterHail(PrimarySpellWaterHail),
    Harden(NativeHardenEffect),
    Staff(crate::spells::staff::NativePlayerStaffTransient),
    Weld(NativeWeldWorldActor),
    EarthBoulderBit(PrimarySpellEarthBoulderBit),
    Air(PrimarySpellAirTransient),
    Water(PrimarySpellWaterTransient),
    EarthImpact(PrimarySpellEarthImpact),
    EarthCalledRock(PrimarySpellEarthCalledRock),
    EtherImpact(PrimarySpellEtherImpact),
    EtherPierceStreak(PrimarySpellEtherPierceStreak),
    EtherBlast(PrimarySpellEtherBlast),
    FireParticle(PrimarySpellFireParticle),
    FireImpact(PrimarySpellFireImpact),
    FireEmber(PrimarySpellFireEmber),
    FireExplosion(PrimarySpellFireExplosion),
    FireGoodImp(PrimarySpellFireGoodImp),
    FirePatch(PrimarySpellFirePatch),
}

impl PrimarySpellTransient {
    #[must_use]
    pub fn common(&self) -> &PrimarySpellTransientBase {
        match self {
            Self::AirHurricane(spell) => &spell.common,
            Self::WaterAura(spell) => &spell.common,
            Self::WaterHail(spell) => &spell.common,
            Self::Harden(effect) => effect.common(),
            Self::Staff(spell) => spell.common(),
            Self::Weld(spell) => &spell.common().owner,
            Self::EarthBoulderBit(spell) => &spell.common,
            Self::Air(spell) => &spell.common,
            Self::Water(spell) => &spell.common,
            Self::EarthImpact(spell) => &spell.common,
            Self::EarthCalledRock(spell) => &spell.common,
            Self::EtherImpact(spell) => &spell.common,
            Self::EtherPierceStreak(spell) => &spell.common,
            Self::EtherBlast(spell) => &spell.common,
            Self::FireParticle(spell) => &spell.common,
            Self::FireImpact(spell) => &spell.common,
            Self::FireEmber(spell) => &spell.common,
            Self::FireExplosion(spell) => &spell.common,
            Self::FireGoodImp(spell) => &spell.common,
            Self::FirePatch(spell) => &spell.common,
        }
    }
    pub fn common_mut(&mut self) -> &mut PrimarySpellTransientBase {
        match self {
            Self::AirHurricane(spell) => &mut spell.common,
            Self::WaterAura(spell) => &mut spell.common,
            Self::WaterHail(spell) => &mut spell.common,
            Self::Harden(effect) => effect.common_mut(),
            Self::Staff(spell) => spell.common_mut(),
            Self::Weld(spell) => &mut spell.common_mut().owner,
            Self::EarthBoulderBit(spell) => &mut spell.common,
            Self::Air(spell) => &mut spell.common,
            Self::Water(spell) => &mut spell.common,
            Self::EarthImpact(spell) => &mut spell.common,
            Self::EarthCalledRock(spell) => &mut spell.common,
            Self::EtherImpact(spell) => &mut spell.common,
            Self::EtherPierceStreak(spell) => &mut spell.common,
            Self::EtherBlast(spell) => &mut spell.common,
            Self::FireParticle(spell) => &mut spell.common,
            Self::FireImpact(spell) => &mut spell.common,
            Self::FireEmber(spell) => &mut spell.common,
            Self::FireExplosion(spell) => &mut spell.common,
            Self::FireGoodImp(spell) => &mut spell.common,
            Self::FirePatch(spell) => &mut spell.common,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrimarySpellChannelKind {
    Air,
    Water,
    Weld,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PrimarySpellFlameLashPresentation {
    /// None is an older non-retained emission; Some(None) is a weak native birth.
    pub camera_displacement: Option<Option<Vec2>>,
    pub midpoint: Option<Vec2>,
    pub target_id: Option<String>,
    pub native_geometry: Option<NativeFlameLashFactoryInput>,
    pub preclip_endpoint: Option<Vec2>,
    pub creator_registration: Option<PainterRegistration>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PrimarySpellChannelEmission {
    pub flame_lash_presentation: Option<PrimarySpellFlameLashPresentation>,
    pub damage: f64,
    pub direction: Vec2,
    pub endpoint: Option<Vec2>,
    pub id: u64,
    pub kind: PrimarySpellChannelKind,
    pub mana_cost: f64,
    pub origin: Vec2,
    pub owner_id: String,
    pub primary_skill: NativePrimarySkillProfile,
    pub query_origin: Vec2,
    pub terrain_contact: bool,
    pub underpowered: bool,
    pub world_key: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FireRadialContact {
    pub amount: f64,
    pub burn_damage: f64,
    pub owner_id: String,
    pub position: Vec2,
    pub radius: f64,
    pub spell_id: u64,
    pub world_key: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FireTargetContact {
    pub amount: f64,
    pub owner_id: String,
    pub spell_id: u64,
    pub target_id: String,
    pub world_key: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum FireActorContact {
    Ember(FireRadialContact),
    GoodImp(FireTargetContact),
    Patch(FireRadialContact),
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct SpellState {
    pub next_id: u64,
    pub projectiles: PrimaryProjectiles,
    pub transients: PrimaryTransients,
}

impl Default for SpellState {
    fn default() -> Self {
        Self {
            next_id: 1,
            projectiles: PrimaryProjectiles::default(),
            transients: PrimaryTransients::default(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PrimarySpellTickResult {
    pub frost_missile_world_contacts: Vec<NativeWeldProjectileState>,
    pub channel_emissions: Vec<PrimarySpellChannelEmission>,
    pub fire_actor_contacts: Vec<FireActorContact>,
    pub mana_spent: f64,
    pub mana_underflow: bool,
}

#[cfg(feature = "persistence")]
fn primary_effects_enhanced_default() -> bool {
    true
}
