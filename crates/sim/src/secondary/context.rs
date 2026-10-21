//! The source secondary tick boundary: immutable player snapshots and actual world queries.
//! The world resolves geometry; the secondary kernel owns actor order, effects and RNG.

use std::collections::BTreeMap;

use crate::belt::PlayerBeltComponent;
use crate::float::fround;
use crate::profile::offensive::{NativeOffensiveSpellFactors, resolve_native_skill_damage_value};
use crate::profile::{NativeSkillBookState, PlayerSkillDerivedStats, PlayerStatBookComponent};
use crate::spells::casting::NativePlayerWeaponKind;
use crate::{HubWizard, NativeRng, Vec2};

use super::{
    NativeSecondaryDamageContact, NativeSecondaryKnockbackContact, NativeSecondaryTarget,
    NativeSecondaryTargetHeadingChange,
};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct NativeSecondaryInput {
    pub aim: Option<Vec2>,
    pub primary: bool,
    pub quickbar: Option<usize>,
}

pub struct NativeSecondaryPlayerSnapshot<'a> {
    pub alive: bool,
    pub weapon_kind: Option<NativePlayerWeaponKind>,
    pub belt: &'a PlayerBeltComponent,
    pub character: &'a HubWizard,
    pub current_mana: f64,
    pub maximum_mana: f64,
    pub eligible: bool,
    pub input: NativeSecondaryInput,
    pub skill_book: &'a NativeSkillBookState,
    pub world_key: &'a str,
}

pub struct NativeSecondaryPlayerAuthority<'a> {
    pub alive: bool,
    pub weapon_kind: Option<NativePlayerWeaponKind>,
    pub belt: &'a PlayerBeltComponent,
    pub character: &'a HubWizard,
    pub cold_slow_factor: f64,
    pub current_mana: f64,
    pub eligible: bool,
    pub enhanced_effects: bool,
    pub explosive_shield_damage: f64,
    pub explosive_shield_raw_mana_cost: f64,
    pub fire_burn_damage: f64,
    pub freeze_duration_multiplier: f64,
    pub focus_instant_recharge_chance_percent: f64,
    pub golem_iron: bool,
    pub golem_raw_mana_cost: f64,
    pub golem_reflect_factor: f64,
    pub input: NativeSecondaryInput,
    pub maximum_mana: f64,
    pub magic_storm_duration_bonus_ticks: u64,
    pub magic_storm_frequency_factor: f64,
    pub magic_storm_raw_mana_cost: f64,
    pub maximum_golem: bool,
    pub maximum_leviathan: bool,
    pub maximum_magic_storm: bool,
    pub maximum_ring_of_fire: bool,
    pub maximum_ring_of_ice: bool,
    pub mana_recovery_per_tick: f64,
    pub offensive_factors: NativeOffensiveSpellFactors,
    pub secondary_recharge_factor: f64,
    pub skill_book: &'a NativeSkillBookState,
    pub world_key: &'a str,
}

impl<'a> NativeSecondaryPlayerAuthority<'a> {
    #[must_use]
    pub fn from_snapshot(
        snapshot: NativeSecondaryPlayerSnapshot<'a>,
        derived: &PlayerSkillDerivedStats,
        offensive_factors: NativeOffensiveSpellFactors,
    ) -> Self {
        Self::from_snapshot_with_stat_book(
            snapshot,
            &PlayerStatBookComponent::default(),
            derived,
            offensive_factors,
        )
    }

    /// Actual supplemental authority reads `playerEntities.statBooks[index]` in
    /// `game-simulation.ts`. The ability's own base rank table remains shared.
    #[must_use]
    pub fn from_snapshot_with_stat_book(
        snapshot: NativeSecondaryPlayerSnapshot<'a>,
        stat_book: &PlayerStatBookComponent,
        derived: &PlayerSkillDerivedStats,
        offensive_factors: NativeOffensiveSpellFactors,
    ) -> Self {
        let book = snapshot.skill_book;
        let ranked =
            |id, property| stat_book.stat(id, property, book.effective_ranks[id]).unwrap_or(0.0);
        let effective =
            |id, property| if book.effective_ranks[id] > 0 { ranked(id, property) } else { 0.0 };
        let feature = |bit| {
            offensive_factors
                .equipment
                .as_ref()
                .is_some_and(|equipment| equipment.feature_bits & bit != 0)
        };
        let fire_burn_damage =
            resolve_native_skill_damage_value(22, effective(22, "mDamage"), &offensive_factors);
        // Raw mana additions intentionally retain the authored rank-zero row. The
        // unlearned Iron Golem upgrade still contributes Raise Golem's base addition.
        Self {
            alive: snapshot.alive,
            weapon_kind: snapshot.weapon_kind,
            belt: snapshot.belt,
            character: snapshot.character,
            cold_slow_factor: fround((0.5 / (1.0 + effective(39, "mSlowdown") / 100.0)).max(0.0)),
            current_mana: snapshot.current_mana,
            eligible: snapshot.eligible,
            enhanced_effects: true,
            explosive_shield_damage: effective(54, "mAbsorb") * effective(55, "mDamage") / 100.0,
            explosive_shield_raw_mana_cost: ranked(55, "mManaCost"),
            fire_burn_damage,
            freeze_duration_multiplier: 1.0 + effective(39, "mSlowdown") / 100.0,
            focus_instant_recharge_chance_percent: derived.focus_instant_recharge_chance_percent,
            golem_iron: book.effective_ranks[75] > 0,
            golem_raw_mana_cost: ranked(75, "mManaCost"),
            golem_reflect_factor: effective(75, "mReflect") / 100.0,
            input: snapshot.input,
            maximum_mana: snapshot.maximum_mana,
            magic_storm_duration_bonus_ticks: (effective(28, "mDuration") * 100.0).trunc() as u64,
            magic_storm_frequency_factor: 1.0 + effective(28, "mSpeed") / 100.0,
            magic_storm_raw_mana_cost: ranked(28, "mManaCost"),
            maximum_golem: feature(0x0008),
            maximum_leviathan: feature(0x0001),
            maximum_magic_storm: feature(0x0002),
            maximum_ring_of_fire: feature(0x0004),
            maximum_ring_of_ice: feature(0x0010),
            mana_recovery_per_tick: derived.mana_recovery_per_tick,
            secondary_recharge_factor: derived.secondary_recharge_factor,
            offensive_factors,
            skill_book: book,
            world_key: snapshot.world_key,
        }
    }
}

pub struct NativeSecondaryTickContext<'a> {
    pub enhanced_effects: Option<bool>,
    pub players: &'a BTreeMap<String, NativeSecondaryPlayerAuthority<'a>>,
    pub tick: u64,
}

impl NativeSecondaryTickContext<'_> {
    #[must_use]
    pub fn enhanced_effects_for(&self, owner_id: &str) -> bool {
        self.enhanced_effects
            .or_else(|| self.players.get(owner_id).map(|owner| owner.enhanced_effects))
            .unwrap_or(true)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeSecondarySceneryTarget {
    pub id: usize,
    pub position: Vec2,
    pub type_id: u32,
    pub secondary_variant: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeSecondaryDampenProjectileKind {
    Firebolt,
    GuidedMissile,
    SkullMissile,
    DarkFireball,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeSecondaryDampenPayload {
    Cold,
    Fire,
    Poison,
    Dark,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeSecondaryDampenProjectileCandidate {
    pub id: usize,
    pub kind: NativeSecondaryDampenProjectileKind,
    pub payload: NativeSecondaryDampenPayload,
    pub position: Vec2,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NativeSecondaryDampenCandidates {
    pub caster_target_ids: Vec<usize>,
    pub projectiles: Vec<NativeSecondaryDampenProjectileCandidate>,
    pub shield_target_ids: Vec<usize>,
}

pub trait NativeSecondaryWorld {
    /// Source requires this callback only when an actual Golem area moves a target.
    fn knockback_area_movement(
        &mut self,
        _world_key: &str,
        _target_id: usize,
        _delta: Vec2,
        _source_actor_id: u64,
    ) -> Vec<usize> {
        panic!("Golem Knockback requires the native world movement owner")
    }
    fn dampen_candidates(&self, world_key: &str, origin: Vec2) -> NativeSecondaryDampenCandidates;
    fn phasing_destination(&self, player_id: &str, origin: Vec2, direction: Vec2) -> Option<Vec2>;
    fn golem_movement(
        &self,
        player_id: &str,
        world_key: &str,
        origin: Vec2,
        requested: Vec2,
        radius: f64,
    ) -> Vec2;
    fn golem_placement(
        &self,
        player_id: &str,
        world_key: &str,
        requested: Vec2,
        rng: &mut NativeRng,
    ) -> Vec2;
    fn teleport_destination(&self, player_id: &str, rng: &mut NativeRng) -> Vec2;
    /// Actor destruction ends an owned modifier. Retained dying or inactive rows
    /// still exist even when `target` excludes them from combat queries.
    fn target_exists(&self, world_key: &str, target_id: usize) -> bool;
    fn target(&self, world_key: &str, target_id: usize) -> Option<NativeSecondaryTarget>;
    fn targets(&self, world_key: &str, center: Vec2, radius: f64) -> Vec<NativeSecondaryTarget>;

    // These callbacks are optional in the source contract. Their defaults preserve its
    // absent-callback behavior; the live world supplies its actual bounds and visibility.
    fn effect_visible(&self, _world_key: &str, _position: Vec2, _margin: f64) -> bool {
        true
    }
    fn golem_foot_placement(
        &self,
        _player_id: &str,
        _world_key: &str,
        _current: Vec2,
        requested: Vec2,
    ) -> Vec2 {
        requested
    }
    fn effect_position_blocked(&self, _world_key: &str, _position: Vec2) -> bool {
        false
    }
    fn line_obstruction(&self, _world_key: &str, _start: Vec2, _end: Vec2) -> bool {
        false
    }
    fn ether_drain_targets(
        &self,
        _world_key: &str,
    ) -> Option<Vec<super::ether_drain::NativeEtherDrainTarget>> {
        None
    }
    fn ether_drain_world_animations(
        &self,
        _world_key: &str,
    ) -> Vec<super::ether_drain::NativeEtherDrainWorldAnimationTarget> {
        Vec::new()
    }
    fn scenery_targets(
        &self,
        _world_key: &str,
        _center: Vec2,
        _radius: f64,
    ) -> Vec<NativeSecondarySceneryTarget> {
        Vec::new()
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct NativeSecondaryTickResult {
    pub dampened_caster_target_ids: Vec<usize>,
    pub damage: Vec<NativeSecondaryDamageContact>,
    pub dispelled_shield_target_ids: Vec<usize>,
    pub ether_drain_contacts: Vec<super::ether_drain::NativeEtherDrainContact>,
    pub ether_drain_world_animation_contacts:
        Vec<super::ether_drain::NativeEtherDrainWorldAnimationContact>,
    pub disrupted_target_ids: Vec<usize>,
    pub mana_recovered: BTreeMap<String, f64>,
    pub mana_underflow_player_ids: Vec<String>,
    pub mana_spent: BTreeMap<String, f64>,
    pub health_recovered: BTreeMap<String, f64>,
    pub target_heading_changes: Vec<NativeSecondaryTargetHeadingChange>,
    pub facing_heading_degrees_by_player: BTreeMap<String, f64>,
    pub knockbacks: Vec<NativeSecondaryKnockbackContact>,
    pub relocated_players: BTreeMap<String, Vec2>,
    pub removed_projectile_ids: Vec<usize>,
    pub overloaded_player_ids: Vec<String>,
    pub primary_override_player_ids: Vec<String>,
    pub staff_cast_pulse_player_ids: Vec<String>,
}
