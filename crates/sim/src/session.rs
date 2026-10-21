//! One authoritative game world, composed from the same owned domains as the Website
//! `core-server/game-simulation.ts`. Browser resources and immutable geometry stay outside.

mod college;
mod commands;
mod detached;
mod dye;
mod ether_drain;
mod extensions;
mod finish;
mod lifecycle;
mod membership;
mod participant;
mod primary;
mod progression;
mod puppet;
mod rescue;
mod rewards;
mod run;
mod secondary;
mod secondary_world;
mod snapshot;
mod staff;
mod tick;
mod world_step;
pub use detached::DetachedSessionProjection;
pub use membership::{DetachedSessionParticipant, ParticipantProfile, SessionPartition};
pub use participant::SessionParticipant;
pub use snapshot::GameSnapshot;
pub use tick::{GameSessionTickContext, GameSessionTickResult};
pub use world_step::GameSessionInput;

pub mod harden;
pub mod player_combat;
pub mod player_contacts;
pub mod player_feedback;

use crate::boneyard::world::BoneyardCheckpoint;
use crate::boneyard::{BoneyardWorld, LoadedBoneyard};
use crate::float::fround;
use crate::game_run::GameRunLifecycle;
use crate::hub::ambient::HubAmbientState;
use crate::hub::npc::{HubCommonTraderClock, HubNpcState};
use crate::modding::extensions::GameSimulationExtensions;
use crate::modding::inventory::GameSimulationModEffect;
use crate::painter::{PainterRegistration, WorldPainterOrder};
use crate::secondary::NativeSecondarySimulationState;
use crate::spells::model::SpellState;
use crate::{HubWizard, NativeRng};

/// Distinct source RNG owners. Combat must never consume the gameplay/loot stream.
#[derive(Clone, Copy, Debug)]
pub struct GameSessionOptions {
    pub gameplay_rng_seed: i64,
    pub combat_rng_seed: i64,
    pub enhanced_effects: bool,
    pub hub_trader_animation_seed: u32,
}

impl Default for GameSessionOptions {
    fn default() -> Self {
        Self {
            gameplay_rng_seed: 0,
            combat_rng_seed: 0,
            enhanced_effects: true,
            hub_trader_animation_seed: crate::hub::npc::DEFAULT_HUB_TRADER_ANIMATION_SEED,
        }
    }
}

/// `player-lighting.ts`: this state belongs to simulation, including overlay decay.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct PlayerLightingState {
    // Crow blindness is an fround duration and can retain a fractional final tick.
    pub blindness_ticks_remaining: f64,
    pub death_weapon_painter_registration: Option<PainterRegistration>,
    pub light_registration: PainterRegistration,
    pub overlay_effect_phase: f64,
}

impl PlayerLightingState {
    #[must_use]
    pub const fn new(light_registration: PainterRegistration) -> Self {
        Self {
            blindness_ticks_remaining: 0.0,
            death_weapon_painter_registration: None,
            light_registration,
            overlay_effect_phase: 0.0,
        }
    }

    pub fn step_overlay(&mut self) {
        self.overlay_effect_phase = fround(self.overlay_effect_phase * 0.8999999761581421);
        self.blindness_ticks_remaining = (self.blindness_ticks_remaining - 1.0).max(0.0);
    }
}

/// A pending skill choice pauses the entire authoritative clock, not just its player.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct PlayerLevelUpBarrier {
    pub barrier_id: u64,
    pub milestone_experience: f64,
    pub milestone_level: u32,
    pub participant_ids: Vec<String>,
    pub pending_player_ids: Vec<String>,
    pub run_id: Option<String>,
    pub source_player_id: String,
}

#[derive(Clone, Debug)]
pub struct GameSession {
    /// Painting permission belongs to the open live menu, never a durable save.
    pub inventory_dye_sessions:
        std::collections::BTreeMap<String, crate::economy::NativeInventoryDyeSession>,
    pub screen_flashes: crate::screen_flash::NativeScreenFlashState,
    pub enhanced_effects: bool,
    pub participants: std::collections::BTreeMap<String, SessionParticipant>,
    /// Source entity/Record order. Sorted barrier cohorts deliberately use their own order.
    pub participant_order: Vec<String>,
    pub next_entity_id: u64,
    pub npcs: HubNpcState,
    pub ambient: HubAmbientState,
    pub dowser_clock: HubCommonTraderClock,
    pub hub_trader_animation_seed: u32,
    pub hub_memorial: crate::hub::memorial::HubMemorialState,
    pub collision_rng: u32,
    pub gameplay_rng: NativeRng,
    pub combat_rng: NativeRng,
    pub secondary: NativeSecondarySimulationState,
    pub spells: SpellState,
    pub hall_of_fame_clock_started_at_tick: u64,
    pub painter_order: WorldPainterOrder,
    pub run: GameRunLifecycle,
    pub level_up_barrier: Option<PlayerLevelUpBarrier>,
    pub next_level_up_barrier_id: u64,
    pub mod_effects: Vec<GameSimulationModEffect>,
    pub next_mod_consumable_use_id: u64,
    pub ticks: u64,
    pub boneyard: Option<BoneyardWorld>,
}

/// Completed mutable authority. Network snapshots borrow this same field shape;
/// durable saves own it. Immutable scenery/navigation is identity-checked on restore.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct GameCheckpoint {
    // Network views admit the live menu state; file checkpoints deliberately omit it.
    #[cfg_attr(feature = "persistence", serde(default, skip_serializing))]
    pub inventory_dye_sessions:
        std::collections::BTreeMap<String, crate::economy::NativeInventoryDyeSession>,
    pub screen_flashes: crate::screen_flash::NativeScreenFlashState,
    pub enhanced_effects: bool,
    pub participants: std::collections::BTreeMap<String, SessionParticipant>,
    pub participant_order: Vec<String>,
    // Zero identifies earlier standalone documents which did not retain entity IDs.
    #[cfg_attr(feature = "persistence", serde(default))]
    pub next_entity_id: u64,
    pub npcs: HubNpcState,
    pub ambient: HubAmbientState,
    pub dowser_clock: HubCommonTraderClock,
    pub hub_trader_animation_seed: u32,
    // Keep the fixed portrait table out of enclosing wasm save visitor frames, like
    // the existing large player/world checkpoint values. The wire remains an object.
    #[cfg_attr(feature = "persistence", serde(default = "boxed_hub_memorial"))]
    pub hub_memorial: Box<crate::hub::memorial::HubMemorialState>,
    pub collision_rng: u32,
    pub gameplay_rng: NativeRng,
    pub combat_rng: NativeRng,
    pub secondary: NativeSecondarySimulationState,
    pub spells: SpellState,
    pub hall_of_fame_clock_started_at_tick: u64,
    pub painter_order: WorldPainterOrder,
    pub run: GameRunLifecycle,
    pub level_up_barrier: Option<PlayerLevelUpBarrier>,
    pub next_level_up_barrier_id: u64,
    #[cfg_attr(feature = "persistence", serde(default))]
    pub mod_effects: Vec<GameSimulationModEffect>,
    #[cfg_attr(feature = "persistence", serde(default = "first_mod_consumable_use_id"))]
    pub next_mod_consumable_use_id: u64,
    pub ticks: u64,
    pub boneyard: Option<Box<BoneyardCheckpoint>>,
}

impl GameCheckpoint {
    /// Check shared identities and phase ownership before a file or network boundary
    /// replaces live authority. Immutable Boneyard validation still belongs to restore.
    pub fn validate(&self) -> Result<(), String> {
        validate_checkpoint(self)
    }
}

impl GameSession {
    /// Source `game-host.ts` changes this shared setting without advancing the world.
    /// Party admission chooses its owner; actor constructors sample it at their source phase.
    pub fn set_enhanced_effects(&mut self, enabled: bool) {
        self.enhanced_effects = enabled;
    }

    #[must_use]
    pub fn participant(&self, id: &str) -> Option<&SessionParticipant> {
        self.participants.get(id)
    }

    #[must_use]
    pub fn wizard(&self, id: &str) -> Option<&HubWizard> {
        self.participant(id).map(|participant| &participant.wizard)
    }

    #[must_use]
    pub fn world_key(&self, id: &str) -> Option<String> {
        let participant = self.participant(id)?;
        Some(self.boneyard.as_ref().map_or_else(
            || format!("hub:{}", participant.wizard.region.id()),
            |world| format!("boneyard:{}", world.loaded.run_id),
        ))
    }

    #[must_use]
    pub fn checkpoint(&self) -> GameCheckpoint {
        GameCheckpoint {
            inventory_dye_sessions: std::collections::BTreeMap::new(),
            screen_flashes: self.screen_flashes.clone(),
            enhanced_effects: self.enhanced_effects,
            participants: self.participants.clone(),
            participant_order: self.participant_order.clone(),
            next_entity_id: self.next_entity_id,
            npcs: self.npcs.clone(),
            ambient: self.ambient.clone(),
            dowser_clock: self.dowser_clock.clone(),
            hub_trader_animation_seed: self.hub_trader_animation_seed,
            hub_memorial: Box::new(self.hub_memorial.clone()),
            collision_rng: self.collision_rng,
            gameplay_rng: self.gameplay_rng.clone(),
            combat_rng: self.combat_rng.clone(),
            secondary: self.secondary.clone(),
            spells: self.spells.clone(),
            hall_of_fame_clock_started_at_tick: self.hall_of_fame_clock_started_at_tick,
            painter_order: self.painter_order,
            run: self.run.clone(),
            level_up_barrier: self.level_up_barrier.clone(),
            next_level_up_barrier_id: self.next_level_up_barrier_id,
            mod_effects: self.mod_effects.clone(),
            next_mod_consumable_use_id: self.next_mod_consumable_use_id,
            ticks: self.ticks,
            boneyard: self.boneyard.as_ref().map(|world| Box::new(world.checkpoint())),
        }
    }

    pub fn restore(
        mut checkpoint: GameCheckpoint,
        loaded: Option<LoadedBoneyard>,
    ) -> Result<Self, String> {
        validate_checkpoint(&checkpoint)?;
        let boneyard = match (checkpoint.boneyard.take(), loaded) {
            (None, None) => None,
            (Some(world), Some(loaded)) => Some(BoneyardWorld::restore(loaded, *world)?),
            _ => return Err("checkpoint Boneyard and immutable scene must both be present".into()),
        };
        Ok(session_from_checkpoint(checkpoint, boneyard))
    }
}

fn validate_participants(
    order: &[String],
    players: &std::collections::BTreeMap<String, SessionParticipant>,
) -> Result<(), String> {
    let mut seen = std::collections::HashSet::new();
    if order.len() != players.len() {
        return Err("participant order and records differ".into());
    }
    for id in order {
        if !seen.insert(id) || players.get(id).is_none_or(|player| player.id != *id) {
            return Err("participant identities must be unique and match their records".into());
        }
    }
    Ok(())
}

/// Borrow map values in source entity order without cloning a participant.
fn ordered_participants_mut<'a>(
    participants: &'a mut std::collections::BTreeMap<String, SessionParticipant>,
    order: &[String],
) -> Vec<&'a mut SessionParticipant> {
    let mut rows: Vec<_> = participants.values_mut().collect();
    rows.sort_by_key(|participant| {
        order
            .iter()
            .position(|id| *id == participant.id)
            .expect("participant order covers every record")
    });
    rows
}

fn session_from_checkpoint(
    mut checkpoint: GameCheckpoint,
    boneyard: Option<BoneyardWorld>,
) -> GameSession {
    if checkpoint.next_entity_id == 0 {
        // A pre-hosted standalone save has no recoverable removed-ID history. Assign
        // its retained actors in source enrollment order once, at the restore edge.
        checkpoint.next_entity_id = 1;
        for id in &checkpoint.participant_order {
            checkpoint.participants.get_mut(id).unwrap().entity_id = checkpoint.next_entity_id;
            checkpoint.next_entity_id += 1;
        }
    }
    GameSession {
        inventory_dye_sessions: checkpoint.inventory_dye_sessions,
        screen_flashes: checkpoint.screen_flashes,
        enhanced_effects: checkpoint.enhanced_effects,
        participants: checkpoint.participants,
        participant_order: checkpoint.participant_order,
        next_entity_id: checkpoint.next_entity_id,
        npcs: checkpoint.npcs,
        ambient: checkpoint.ambient,
        dowser_clock: checkpoint.dowser_clock,
        hub_trader_animation_seed: checkpoint.hub_trader_animation_seed,
        hub_memorial: *checkpoint.hub_memorial,
        collision_rng: checkpoint.collision_rng,
        gameplay_rng: checkpoint.gameplay_rng,
        combat_rng: checkpoint.combat_rng,
        secondary: checkpoint.secondary,
        spells: checkpoint.spells,
        hall_of_fame_clock_started_at_tick: checkpoint.hall_of_fame_clock_started_at_tick,
        painter_order: checkpoint.painter_order,
        run: checkpoint.run,
        level_up_barrier: checkpoint.level_up_barrier,
        next_level_up_barrier_id: checkpoint.next_level_up_barrier_id,
        mod_effects: checkpoint.mod_effects,
        next_mod_consumable_use_id: checkpoint.next_mod_consumable_use_id,
        ticks: checkpoint.ticks,
        boneyard,
    }
}

#[cfg(feature = "persistence")]
fn boxed_hub_memorial() -> Box<crate::hub::memorial::HubMemorialState> {
    Box::new(crate::hub::memorial::create_hub_memorial_state())
}

#[cfg(feature = "persistence")]
const fn first_mod_consumable_use_id() -> u64 {
    1
}

fn validate_checkpoint(checkpoint: &GameCheckpoint) -> Result<(), String> {
    for (id, session) in &checkpoint.inventory_dye_sessions {
        if !checkpoint.participants.contains_key(id)
            || session.id.is_empty()
            || session.id.encode_utf16().count() > 64
        {
            return Err("inventory painting session has an invalid owner or identity".into());
        }
    }
    if checkpoint.next_mod_consumable_use_id == 0
        || checkpoint.next_mod_consumable_use_id > 9_007_199_254_740_991
    {
        return Err("authored consumable sequence must be a positive safe integer".into());
    }
    let mut use_ids = std::collections::HashSet::new();
    for effect in &checkpoint.mod_effects {
        if effect.use_id == 0
            || effect.use_id >= checkpoint.next_mod_consumable_use_id
            || !use_ids.insert(effect.use_id)
            || effect.expires_tick <= effect.started_tick
            || effect.started_tick > checkpoint.ticks
            || effect.color.iter().any(|channel| !channel.is_finite())
        {
            return Err("authored effect ownership or lifetime is invalid".into());
        }
    }
    validate_participants(&checkpoint.participant_order, &checkpoint.participants)?;
    let legacy_entity_ids = checkpoint.next_entity_id == 0
        && checkpoint.participants.values().all(|participant| participant.entity_id == 0);
    if !legacy_entity_ids {
        let mut entity_ids = std::collections::HashSet::new();
        for participant in checkpoint.participants.values() {
            if participant.entity_id == 0
                || participant.entity_id >= checkpoint.next_entity_id
                || !entity_ids.insert(participant.entity_id)
            {
                return Err(
                    "player entity IDs must be unique positive values below next_entity_id".into(),
                );
            }
        }
        if checkpoint.next_entity_id == 0 {
            return Err("next_entity_id must be positive".into());
        }
    }
    let mut eligible = std::collections::HashSet::new();
    for id in &checkpoint.run.eligible_player_ids {
        if !eligible.insert(id) || !checkpoint.participants.contains_key(id) {
            return Err("run participants must be unique existing players".into());
        }
    }
    if let Some(world) = &checkpoint.boneyard {
        if checkpoint.run.run_id.as_deref() != Some(world.run_id.as_str()) {
            return Err("run and Boneyard checkpoint identities differ".into());
        }
        if world.choice_id == "stock-tutorial" && checkpoint.participants.len() != 1 {
            return Err("Tutorial checkpoint requires exactly one participant".into());
        }
    }
    if let Some(barrier) = &checkpoint.level_up_barrier {
        let mut seen = std::collections::HashSet::new();
        for id in &barrier.participant_ids {
            if !seen.insert(id) || !checkpoint.participants.contains_key(id) {
                return Err("level-up cohort must contain unique existing players".into());
            }
        }
        let mut pending = std::collections::HashSet::new();
        for id in &barrier.pending_player_ids {
            if !seen.contains(id) || !pending.insert(id) {
                return Err("level-up pending players must belong to the cohort".into());
            }
        }
        if !seen.contains(&barrier.source_player_id)
            || pending.is_empty()
            || !barrier.milestone_experience.is_finite()
            || barrier.milestone_experience < 0.0
            || !(1..=crate::profile::MAX_PLAYER_LEVEL).contains(&barrier.milestone_level)
        {
            return Err("level-up barrier is inconsistent".into());
        }
    }
    Ok(())
}
