//! Enemy-store ownership and world callbacks from Website `core-server/boneyard-enemy-store.ts`
//! and `enemies/model.ts`. `construction.rs` owns materializing authored births.

use super::config::EnemyArenaScalars;
use super::model::{
    EnemyActor, EnemyDeathEffect, EnemyEvent, EnemyMaggotActor, EnemyPlayerDamage,
    EnemyPlayerKnockback, EnemyTarget, WebbedState,
};
use super::path::EnemyNavigation;
use super::projectiles::{
    EnemyProjectile, EnemyProjectileEffect, EnemyProjectileKnockback, MageLightningPulse,
};
pub use super::step::EnemyLiveCounts;
use super::voices::BossNarration;
pub use super::voices::FacultyVoiceController;
use super::{EnemySpawnIntent, SpawnPositionPolicy};
use crate::painter::{WorldManagerLane, WorldPainterOrder};
use crate::rng::{next_xorshift_random, seed_from_text};
use crate::{NativeRng, Vec2};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnemyMovementPurpose {
    Movement,
    SpawnPlacement,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EnemyMovementRequest {
    pub actor_id: usize,
    pub delta: Vec2,
    pub position: Vec2,
    pub purpose: EnemyMovementPurpose,
    pub radius: f64,
    pub requested_position: Vec2,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EnemySpawnPlacementRequest {
    pub actor_id: usize,
    pub navigation_clearance: f64,
    pub position: Vec2,
    pub position_policy: SpawnPositionPolicy,
    pub radius: f64,
    pub reachability_radius: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EnemyProjectileWorldQuery {
    Line { projectile_id: usize, start: Vec2, end: Vec2, native_exclusion_mask: u32 },
    Point { projectile_id: usize, position: Vec2, radius: f64 },
    Bounds { projectile_id: usize, position: Vec2, margin: f64 },
    View { projectile_id: usize, position: Vec2, margin: f64 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct EnemyTargetCellBinding {
    pub cell_x: i64,
    pub cell_y: i64,
    pub order: usize,
}

/// The source world callbacks, kept outside the deterministic actor state. The default
/// spawn placement is the source's documented zero-delta movement path when no dedicated
/// spawn-placement callback exists.
pub trait EnemyGeometry: EnemyNavigation {
    fn resolve_movement(&self, request: EnemyMovementRequest) -> Vec2;
    fn line_obstruction(&self, start: Vec2, end: Vec2, native_exclusion_mask: u32) -> Option<Vec2>;
    fn projectile_world_blocked(&self, query: EnemyProjectileWorldQuery) -> bool;
    fn on_projectile_explosion(&self, _position: Vec2) {}
    fn on_collision_recipient_cleared(&self, _actor_id: usize) {}
    fn on_terminal_output(
        &self,
        _output: super::model::EnemyTerminalOutput,
        _output_count: Option<usize>,
    ) {
    }
    /// Native Mage lightning clips the target line against active world geometry.
    fn clip_spell_segment(&self, start: Vec2, end: Vec2) -> Vec2 {
        self.line_obstruction(start, end, 0).unwrap_or(end)
    }
    fn resolve_spawn_placement(
        &self,
        request: EnemySpawnPlacementRequest,
        _rng: &mut NativeRng,
    ) -> Vec2 {
        self.resolve_movement(EnemyMovementRequest {
            actor_id: request.actor_id,
            delta: Vec2::ZERO,
            position: request.position,
            purpose: EnemyMovementPurpose::SpawnPlacement,
            radius: request.radius,
            requested_position: request.position,
        })
    }
}

pub struct EnemySpawnContext<'a> {
    pub players: &'a [EnemyTarget],
    pub tick: u64,
    pub arena_scalars: EnemyArenaScalars,
    pub roll_loot_seed: Option<&'a mut dyn FnMut(u32) -> u32>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EnemyAbilityEffect {
    pub time_scale: f64,
    pub weaken_factor: f64,
    pub disrupted_ticks: u64,
    pub dazzle_ticks: u64,
    pub flee_ticks: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeEnemyVisibility {
    pub admitted: bool,
    pub intensity: f64,
}

impl Default for NativeEnemyVisibility {
    fn default() -> Self {
        Self { admitted: true, intensity: 1.0 }
    }
}
impl Default for EnemyAbilityEffect {
    fn default() -> Self {
        Self {
            time_scale: 1.0,
            weaken_factor: 1.0,
            disrupted_ticks: 0,
            dazzle_ticks: 0,
            flee_ticks: 0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct NativeEnemyMovementView<'a> {
    pub arena_bounds: crate::boneyard::BoneyardBounds,
    pub cameras: &'a [crate::boneyard::BoneyardBounds],
    pub enhanced_effects: bool,
}

pub struct EnemyStepContext<'a> {
    pub enhanced_effects: bool,
    pub tick: u64,
    pub paused: bool,
    pub players: &'a [EnemyTarget],
    pub geometry: &'a dyn EnemyGeometry,
    pub ability_effects: &'a [(usize, EnemyAbilityEffect)],
    pub arena_scalars: EnemyArenaScalars,
    pub dialogue_busy: bool,
    pub roll_loot_seed: Option<&'a mut dyn FnMut(u32) -> u32>,
    pub native_visibility: Option<&'a dyn Fn(Vec2) -> NativeEnemyVisibility>,
    pub projected_point_visible: Option<&'a dyn Fn(Vec2) -> bool>,
    pub native_view_bounds: Option<crate::boneyard::BoneyardBounds>,
    pub puppet_targets: &'a [super::puppet_hits::EnemyPuppetTarget],
    pub light_at: Option<&'a dyn Fn(Vec2) -> f64>,
    pub native_movement_view: Option<NativeEnemyMovementView<'a>>,
}
impl EnemyStepContext<'_> {
    pub(super) fn with_players<'a>(
        &'a mut self,
        players: &'a [EnemyTarget],
    ) -> EnemyStepContext<'a> {
        let roll_loot_seed =
            self.roll_loot_seed.as_mut().map(|draw| &mut **draw as &mut dyn FnMut(u32) -> u32);
        EnemyStepContext {
            enhanced_effects: self.enhanced_effects,
            tick: self.tick,
            paused: self.paused,
            players,
            geometry: self.geometry,
            ability_effects: self.ability_effects,
            arena_scalars: self.arena_scalars,
            dialogue_busy: self.dialogue_busy,
            roll_loot_seed,
            native_visibility: self.native_visibility,
            projected_point_visible: self.projected_point_visible,
            native_view_bounds: self.native_view_bounds,
            puppet_targets: self.puppet_targets,
            light_at: self.light_at,
            native_movement_view: self.native_movement_view,
        }
    }
    #[must_use]
    pub fn ability_effect(&self, id: usize) -> EnemyAbilityEffect {
        self.ability_effects
            .iter()
            .find(|(actor_id, _)| *actor_id == id)
            .map_or_else(EnemyAbilityEffect::default, |(_, effect)| *effect)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct DemonSkullEncounter {
    pub death_stream_ticks_remaining: u64,
    pub scream_stream_ticks_remaining: u64,
    pub health_triggers_enabled: bool,
    pub health_triggers_fired_mask: u8,
    pub pending_capabilities: u8,
}

/// The actor store owns each random stream at the same seam as the source. Constructor
/// randomness, path decisions, locomotion phases and head facing never share a cursor.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct EnemyWorld {
    #[cfg_attr(feature = "persistence", serde(skip))]
    pub(super) deferred_projectile_painters:
        Option<super::deferred_painters::DeferredEnemyPainterRegistrations>,
    pub actors: Vec<EnemyActor>,
    pub maggots: Vec<EnemyMaggotActor>,
    pub webbed_players: BTreeMap<String, WebbedState>,
    pub rng_state: u32,
    pub steering_rng: NativeRng,
    pub locomotion_rng: NativeRng,
    pub head_facing_rng: NativeRng,
    pub last_step_tick: Option<u64>,
    pub next_actor_id: usize,
    pub next_death_epoch: u64,
    pub next_death_effect_id: usize,
    pub next_event_id: usize,
    pub next_synthetic_spawn_intent_id: usize,
    pub next_projectile_id: usize,
    pub next_mage_lightning_pulse_id: usize,
    pub next_projectile_effect_id: usize,
    pub next_native_cell_binding_order: usize,
    pub next_native_registration_order: usize,
    pub featured_boss_id: Option<usize>,
    pub faculty_voice_controller: Option<FacultyVoiceController>,
    pub boss_narration: BossNarration,
    pub demon_skull_encounter: DemonSkullEncounter,
    pub spawned_actor_ids: Vec<usize>,
    pub events: Vec<EnemyEvent>,
    pub death_effects: Vec<EnemyDeathEffect>,
    pub player_damage: Vec<EnemyPlayerDamage>,
    pub player_knockbacks: Vec<EnemyPlayerKnockback>,
    pub player_positions: BTreeMap<String, Vec2>,
    pub pending_spawn_intents: Vec<EnemySpawnIntent>,
    pub projectiles: Vec<EnemyProjectile>,
    pub mage_lightning_pulses: Vec<MageLightningPulse>,
    pub projectile_effects: Vec<EnemyProjectileEffect>,
    pub projectile_knockbacks: Vec<EnemyProjectileKnockback>,
    pub target_cell_bindings: BTreeMap<String, EnemyTargetCellBinding>,
    pub retired: Vec<super::model::EnemyRetirement>,
    pub rewards: Vec<super::model::EnemyReward>,
    pub silks: Vec<super::silk::EnemySilkActor>,
    pub silk_fragments: Vec<super::silk_force::NativeFadeLineActor>,
    pub spider_spit_ticks_remaining: u64,
    pub spider_remains: Vec<super::spider_remains::SpiderRemains>,
    pub detached_crows: Vec<super::heartmonger::DetachedCrow>,
    pub boss_spells: Vec<super::boss_spell::NativeBossSpell>,
    pub puppet_hits: Vec<super::puppet_hits::EnemyPuppetHit>,
}

impl EnemyWorld {
    #[must_use]
    pub fn new(seed: &str, native_registration_base: usize) -> Self {
        Self {
            deferred_projectile_painters: None,
            actors: Vec::new(),
            maggots: Vec::new(),
            webbed_players: BTreeMap::new(),
            rng_state: seed_from_text(&format!("{seed}:enemy-actors")),
            steering_rng: NativeRng::new(i64::from(seed_from_text(&format!(
                "{seed}:enemy-steering"
            )))),
            locomotion_rng: NativeRng::new(i64::from(seed_from_text(&format!(
                "{seed}:enemy-locomotion"
            )))),
            head_facing_rng: NativeRng::new(i64::from(seed_from_text(&format!(
                "{seed}:skeleton-head-facing"
            )))),
            last_step_tick: None,
            next_actor_id: 1,
            next_death_epoch: 1,
            next_death_effect_id: 1,
            next_event_id: 1,
            next_synthetic_spawn_intent_id: 1,
            next_projectile_id: 1,
            next_mage_lightning_pulse_id: 1,
            next_projectile_effect_id: 1,
            next_native_cell_binding_order: native_registration_base,
            next_native_registration_order: native_registration_base,
            featured_boss_id: None,
            faculty_voice_controller: None,
            boss_narration: BossNarration::default(),
            demon_skull_encounter: DemonSkullEncounter::default(),
            spawned_actor_ids: Vec::new(),
            events: Vec::new(),
            death_effects: Vec::new(),
            player_damage: Vec::new(),
            player_knockbacks: Vec::new(),
            player_positions: BTreeMap::new(),
            pending_spawn_intents: Vec::new(),
            projectiles: Vec::new(),
            mage_lightning_pulses: Vec::new(),
            projectile_effects: Vec::new(),
            projectile_knockbacks: Vec::new(),
            target_cell_bindings: BTreeMap::new(),
            retired: Vec::new(),
            rewards: Vec::new(),
            silks: Vec::new(),
            silk_fragments: Vec::new(),
            spider_spit_ticks_remaining: 0,
            spider_remains: Vec::new(),
            detached_crows: Vec::new(),
            boss_spells: Vec::new(),
            puppet_hits: Vec::new(),
        }
    }

    pub(super) fn draw_unit(&mut self) -> f64 {
        next_xorshift_random(&mut self.rng_state)
    }
    pub(super) fn draw_integer(&mut self, count: u32) -> u32 {
        (self.draw_unit() * f64::from(count)).floor().min(f64::from(count - 1)) as u32
    }
    pub(super) fn draw_loot_seed(
        &mut self,
        bound: u32,
        shared: &mut Option<&mut dyn FnMut(u32) -> u32>,
    ) -> u32 {
        if let Some(shared) = shared {
            let seed = shared(bound);
            assert!(seed < bound, "native loot seed writer returned an invalid seed");
            seed
        } else {
            self.draw_integer(bound)
        }
    }
    pub(crate) fn emit_event(&mut self, mut event: EnemyEvent) -> usize {
        let id = self.next_event_id;
        self.next_event_id += 1;
        event.event_id = id;
        self.events.push(event);
        id
    }

    pub(super) fn standalone_painter_order(&self) -> WorldPainterOrder {
        self.assert_painter_registrations_committed();
        let mut order = WorldPainterOrder::default();
        for actor in &self.actors {
            order.next_actor_ordinal = order.next_actor_ordinal.max(actor.registration_ordinal + 1);
        }
        for maggot in &self.maggots {
            order.next_actor_ordinal =
                order.next_actor_ordinal.max(maggot.registration_ordinal + 1);
        }
        for projectile in &self.projectiles {
            for registration in [
                Some(projectile.state().painter_registration),
                projectile.state().light_registration,
            ]
            .into_iter()
            .flatten()
            {
                let next = match registration.manager_lane {
                    WorldManagerLane::Actor => &mut order.next_actor_ordinal,
                    WorldManagerLane::Transient => &mut order.next_transient_ordinal,
                };
                *next = (*next).max(registration.registration_ordinal + 1);
            }
        }
        for pulse in &self.mage_lightning_pulses {
            for registration in
                std::iter::once(&pulse.light_registration).chain(&pulse.painter_registrations)
            {
                let next = match registration.manager_lane {
                    WorldManagerLane::Actor => &mut order.next_actor_ordinal,
                    WorldManagerLane::Transient => &mut order.next_transient_ordinal,
                };
                *next = (*next).max(registration.registration_ordinal + 1);
            }
        }
        for registration in
            self.death_effects.iter().filter_map(|effect| effect.painter_registration)
        {
            let next = match registration.manager_lane {
                WorldManagerLane::Actor => &mut order.next_actor_ordinal,
                WorldManagerLane::Transient => &mut order.next_transient_ordinal,
            };
            *next = (*next).max(registration.registration_ordinal + 1);
        }
        for effect in &self.projectile_effects {
            for registration in
                [Some(effect.state.painter_registration), effect.state.light_registration]
                    .into_iter()
                    .flatten()
            {
                let next = match registration.manager_lane {
                    WorldManagerLane::Actor => &mut order.next_actor_ordinal,
                    WorldManagerLane::Transient => &mut order.next_transient_ordinal,
                };
                *next = (*next).max(registration.registration_ordinal + 1);
            }
        }
        for silk in &self.silks {
            order.next_actor_ordinal =
                order.next_actor_ordinal.max(silk.painter_registration.registration_ordinal + 1);
        }
        for spell in &self.boss_spells {
            let registration = spell.owner.painter_registration;
            let next = match registration.manager_lane {
                WorldManagerLane::Actor => &mut order.next_actor_ordinal,
                WorldManagerLane::Transient => &mut order.next_transient_ordinal,
            };
            *next = (*next).max(registration.registration_ordinal + 1);
        }
        order
    }
}
