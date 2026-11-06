//! Primary spell projectiles and effects, and the per-tick results of casting them.
//! Family modules own the rules; these rows retain identity, painter ownership and the
//! fields shared with rendering and contacts.

use super::earth_debris::WeldBoulderDebrisParticleState;
use super::weld::{FlameLashFactoryInput, WeldProjectileState, WeldWorldActor};
use crate::enemies::path::EnemyPathState;
use crate::harden_effects::HardenEffect;
use crate::painter::PainterRegistration;
use crate::profile::primary::{PrimarySkillProfile, SpentEmber};
use crate::{Rng, Vec2};

pub use super::transient_storage::PrimaryTransients;

pub type EarthBoulderOrientation = [f64; 9];

/// Whether a projectile is still held by its caster or in flight.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum PrimarySpellProjectilePhase {
    Flight,
    Held,
}

/// Flight state every primary projectile family shares.
#[derive(Clone, Debug, PartialEq)]
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

/// An Earth boulder, held while it charges and then thrown. Fields follow the
/// save record's order.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct PrimarySpellEarthProjectile {
    pub common: PrimarySpellProjectileBase,
    pub assembly_charge: f64,
    pub hit_target_ids: Vec<String>,
    pub maximum_charge: f64,
    pub orientation: EarthBoulderOrientation,
    pub remaining_damage: f64,
    pub shell_charge: f64,
    pub toughness: f64,
}

/// An Ether projectile that homes on targets and can pierce them. Fields follow the
/// save record's order.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct PrimarySpellEtherProjectile {
    pub common: PrimarySpellProjectileBase,
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

/// A fireball. It explodes on impact and scatters embers. Fields follow the
/// save record's order.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct PrimarySpellFireProjectile {
    pub common: PrimarySpellProjectileBase,
    pub burn_damage: f64,
    pub ember_damage: f64,
    pub ember_fragments: u32,
    pub explode_damage: f64,
    pub explode_radius: f64,
    pub private_seed: i32,
    pub spent_ember: SpentEmber,
    pub underpowered: bool,
}

/// A primary spell projectile of any family.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum PrimarySpellProjectile {
    Weld(WeldProjectileState),
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

/// Identity, age, owner and painter registrations shared by every primary spell effect.
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

/// One Lightning strike from the Air primary: its path from origin through midpoint to
/// endpoint, its target and whether it chained.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
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

/// One Frost jet particle from the Water primary: origin, direction, speed and any
/// terrain obstruction.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
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

/// The rock burst where a boulder lands; its fragments replay from its charge and birth
/// tick.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
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

/// One debris bit thrown off by a boulder contact.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct PrimarySpellEarthBoulderBit {
    pub common: PrimarySpellTransientBase,
    pub birth_tick: u64,
    pub debris: WeldBoulderDebrisParticleState,
    pub origin: Vec2,
    pub position: Vec2,
}

/// A rock that rises around a held boulder and spirals into it, then falls once the
/// boulder is released.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
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

/// A particle shed each tick by a fireball in flight.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct PrimarySpellFireParticle {
    pub common: PrimarySpellTransientBase,
    #[cfg_attr(feature = "persistence", serde(default = "primary_effects_enhanced_default"))]
    pub enhanced_effects: bool,
    pub direction: Vec2,
    pub origin: Vec2,
    pub variant: u32,
}

/// The burst drawn where an Ether missile hits.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct PrimarySpellEtherImpact {
    pub common: PrimarySpellTransientBase,
    pub birth_tick: u64,
    pub origin: Vec2,
    pub visual_scale: f64,
}

/// The streak drawn where an Ether missile pierces a target.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct PrimarySpellEtherPierceStreak {
    pub common: PrimarySpellTransientBase,
    pub heading_degrees: f64,
    pub origin: Vec2,
    pub visual_scale: f64,
}

/// An Ether Blast pulse: its charges, origin and the saved RNG its particles replay from.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct PrimarySpellEtherBlast {
    pub common: PrimarySpellTransientBase,
    pub birth_tick: u64,
    pub charges: u32,
    pub origin: Vec2,
    pub presentation_rng: Rng,
}

/// The flash drawn where a fireball hits.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct PrimarySpellFireImpact {
    pub common: PrimarySpellTransientBase,
    pub origin: Vec2,
}

/// A bouncing ember scattered by a fireball's detonation: its damage, motion, life and
/// what it becomes when spent.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
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
    pub spent_ember: SpentEmber,
    pub vertical_velocity: f64,
}

/// Whether an explosion is drawn as Fire or as Steam.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum FireExplosionPresentation {
    Fire,
    Steam,
}

/// A Fire or Steam explosion: its damage, burn, footprint, scale and sound pitch.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
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

/// A friendly Imp born from a spent ember, which hunts enemies until its time runs out.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
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
    pub path: EnemyPathState,
    pub position: Vec2,
    pub remaining_ticks: u64,
    pub target_id: Option<String>,
    pub vertical_offset: f64,
    pub vertical_velocity: f64,
}

/// The native fire patch type. A Moving patch defaults to a higher draw alpha and a
/// slower animation step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum FirePatchType {
    Fire,
    GoodGuy,
    Moving,
}

/// A patch of fire on the ground that damages enemies within its radius until its life
/// runs out.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct PrimarySpellFirePatch {
    pub common: PrimarySpellTransientBase,
    pub atlas_phase: f64,
    pub atlas_phase_step: f64,
    pub burn_damage: f64,
    pub damage: f64,
    pub draw_alpha: f64,
    pub fade_alpha: f64,
    pub life: f64,
    #[cfg_attr(feature = "persistence", serde(rename = "native_type"))]
    pub patch_type: FirePatchType,
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
pub struct PrimarySpellAirHurricane {
    pub common: PrimarySpellTransientBase,
    pub birth_tick: u64,
    pub charge: f64,
    pub contact_charge: f64,
    pub damage_maximum: f64,
    pub damage_minimum: f64,
    pub enhanced_effects: bool,
    pub lanes: [crate::hurricane::HurricaneLane; 8],
    pub phase_degrees: f64,
    pub position: Vec2,
}

/// A Cold Aura around a Water caster: its rotation, fade and duration.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct PrimarySpellWaterAura {
    pub common: PrimarySpellTransientBase,
    pub alpha_decay: f64,
    pub birth_tick: u64,
    pub duration_ticks: u64,
    pub initial_rotation_degrees: f64,
    pub origin: Vec2,
    pub rotation_step_degrees: f64,
}

/// A Water Hail stone thrown from the Frost jet, bouncing until its life runs out.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
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

/// A primary spell effect of any kind, kept in the shared transient queue.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum PrimarySpellTransient {
    AirHurricane(PrimarySpellAirHurricane),
    WaterAura(PrimarySpellWaterAura),
    WaterHail(PrimarySpellWaterHail),
    Harden(HardenEffect),
    Staff(crate::spells::staff::PlayerStaffTransient),
    Weld(WeldWorldActor),
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

/// The primaries that channel: Air, Water and the channel Welds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrimarySpellChannelKind {
    Air,
    Water,
    Weld,
}

/// What drawing a Flame Lash main ribbon needs: geometry, midpoint, target, unclipped
/// endpoint, camera displacement and the creator's registration.
#[derive(Clone, Debug, PartialEq)]
pub struct PrimarySpellFlameLashPresentation {
    /// None is an older non-retained emission; Some(None) is a weak native birth.
    pub camera_displacement: Option<Option<Vec2>>,
    pub midpoint: Option<Vec2>,
    pub target_id: Option<String>,
    pub native_geometry: Option<FlameLashFactoryInput>,
    pub preclip_endpoint: Option<Vec2>,
    pub creator_registration: Option<PainterRegistration>,
}

/// One tick of a channeled primary: its origin, direction, endpoint, damage, mana cost
/// and profile, read by the contact phase and the effects that follow.
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
    pub primary_skill: PrimarySkillProfile,
    pub query_origin: Vec2,
    pub terrain_contact: bool,
    pub underpowered: bool,
    pub world_key: String,
}

/// A Fire contact that hits every enemy within a radius, as embers and patches do.
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

/// A Fire contact on one target, as a friendly Imp's attack.
#[derive(Clone, Debug, PartialEq)]
pub struct FireTargetContact {
    pub amount: f64,
    pub owner_id: String,
    pub spell_id: u64,
    pub target_id: String,
    pub world_key: String,
}

/// A contact made by an ember, a friendly Imp or a fire patch.
#[derive(Clone, Debug, PartialEq)]
pub enum FireActorContact {
    Ember(FireRadialContact),
    GoodImp(FireTargetContact),
    Patch(FireRadialContact),
}

/// Every live primary spell: projectiles, effects and the next spell ID.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct SpellState {
    pub next_id: u64,
    pub projectiles: Vec<PrimarySpellProjectile>,
    pub transients: PrimaryTransients,
}

impl Default for SpellState {
    fn default() -> Self {
        Self { next_id: 1, projectiles: Vec::new(), transients: PrimaryTransients::default() }
    }
}

/// What one primary spell tick hands to the contact phase: channel emissions, Fire actor
/// contacts and Frost Missile world contacts, plus the mana spent and any shortfall.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PrimarySpellTickResult {
    pub frost_missile_world_contacts: Vec<WeldProjectileState>,
    pub channel_emissions: Vec<PrimarySpellChannelEmission>,
    pub fire_actor_contacts: Vec<FireActorContact>,
    pub mana_spent: f64,
    pub mana_underflow: bool,
}

#[cfg(feature = "persistence")]
fn primary_effects_enhanced_default() -> bool {
    true
}
