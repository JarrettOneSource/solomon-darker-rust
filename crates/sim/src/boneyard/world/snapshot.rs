//! Borrowed network projection and in-place application of completed world ticks.

use super::super::waves::director::WaveSnapshot;
use super::*;

/// This serializes the same shape as BoneyardCheckpoint without cloning enemies,
/// loot, event histories or mutable wave state for each network frame.
#[cfg_attr(feature = "persistence", derive(serde::Serialize))]
pub struct BoneyardSnapshot<'a> {
    pub choice_id: &'a str,
    pub choice_source: &'a BoneyardSource,
    pub mod_id: &'a Option<String>,
    pub mod_name: &'a Option<String>,
    pub run_id: &'a str,
    pub seed: &'a str,
    pub source_sha256: &'a str,
    pub geometry_sha256: &'a str,
    pub ticks: &'a u64,
    pub gate_leaves: &'a Vec<GateLeaf>,
    pub arena_transition: &'a Option<ArenaTransition>,
    pub encounter: &'a Option<SolomonEncounter>,
    pub waves: Option<WaveSnapshot<'a>>,
    pub tutorial: &'a Option<NativeTutorialState>,
    pub scenery_retained_bounds: &'a Option<BoneyardBounds>,
    #[cfg_attr(feature = "persistence", serde(skip_serializing_if = "Option::is_none"))]
    pub scenery_target_retention: &'a Option<Arc<targets::BoneyardSceneryTargetRetention>>,
    pub enemies: &'a EnemyWorld,
    pub death_weapons: &'a Vec<crate::death_animations::NativeDeathWeaponActor>,
    pub next_death_weapon_id: &'a u64,
    pub loot: &'a BoneyardLootStore,
    pub enemy_world_feedback: &'a NativeEnemyWorldFeedback,
    pub enemy_events: &'a Vec<EnemyEvent>,
    pub loot_events: &'a Vec<BoneyardLootEvent>,
    pub player_ouch_deadline_tick: &'a u64,
    pub lantern_position: &'a Option<Vec2>,
    pub lantern_light_registration: &'a Option<PainterRegistration>,
    pub solomon_painter_registration: &'a Option<PainterRegistration>,
}

impl BoneyardWorld {
    #[must_use]
    pub fn snapshot(&self) -> BoneyardSnapshot<'_> {
        self.enemies.assert_painter_registrations_committed();
        BoneyardSnapshot {
            choice_id: &self.loaded.choice_id,
            choice_source: &self.loaded.choice_source,
            mod_id: &self.loaded.mod_id,
            mod_name: &self.loaded.mod_name,
            run_id: &self.loaded.run_id,
            seed: &self.loaded.seed,
            source_sha256: &self.loaded.source_sha256,
            geometry_sha256: &self.loaded.geometry_sha256,
            ticks: &self.ticks,
            gate_leaves: &self.gate_leaves,
            arena_transition: &self.arena_transition,
            encounter: &self.encounter,
            waves: self.waves.as_ref().map(WaveDirector::snapshot),
            tutorial: &self.tutorial,
            scenery_retained_bounds: &self.scenery_retained_bounds,
            scenery_target_retention: &self.scenery_target_retention,
            enemies: &self.enemies,
            death_weapons: &self.death_weapons,
            next_death_weapon_id: &self.next_death_weapon_id,
            loot: &self.loot,
            enemy_world_feedback: &self.enemy_world_feedback,
            enemy_events: &self.enemy_events,
            loot_events: &self.loot_events,
            player_ouch_deadline_tick: &self.player_ouch_deadline_tick,
            lantern_position: &self.lantern_position,
            lantern_light_registration: &self.lantern_light_registration,
            solomon_painter_registration: &self.solomon_painter_registration,
        }
    }

    pub(crate) fn validate_checkpoint(
        &self,
        checkpoint: &BoneyardCheckpoint,
    ) -> Result<(), String> {
        let loaded = &self.loaded;
        if loaded.choice_id != checkpoint.choice_id
            || loaded.choice_source != checkpoint.choice_source
            || loaded.mod_id != checkpoint.mod_id
            || loaded.mod_name != checkpoint.mod_name
            || loaded.run_id != checkpoint.run_id
            || loaded.seed != checkpoint.seed
            || loaded.source_sha256 != checkpoint.source_sha256
            || loaded.geometry_sha256 != checkpoint.geometry_sha256
        {
            return Err("Boneyard checkpoint identity does not match the loaded scene".into());
        }
        match (&self.waves, &checkpoint.waves) {
            (Some(waves), Some(checkpoint)) => waves.validate_checkpoint(checkpoint)?,
            (_, None) => {}
            (None, Some(checkpoint)) => {
                WaveDirector::new(&loaded.seed, Some(&loaded.source_sha256))
                    .validate_checkpoint(checkpoint)?;
            }
        }
        if checkpoint.tutorial.is_some() != (loaded.choice_id == "stock-tutorial") {
            return Err("Boneyard checkpoint Tutorial ownership differs from its scene".into());
        }
        if !checkpoint.enemies.painter_registrations_committed() {
            return Err("Boneyard checkpoint contains uncommitted painter registrations".into());
        }
        Ok(())
    }

    pub fn apply_checkpoint(&mut self, checkpoint: BoneyardCheckpoint) -> Result<(), String> {
        self.validate_checkpoint(&checkpoint)?;
        self.ticks = checkpoint.ticks;
        self.gate_leaves = checkpoint.gate_leaves;
        self.arena_transition = checkpoint.arena_transition;
        self.encounter = checkpoint.encounter;
        match (&mut self.waves, checkpoint.waves) {
            (Some(waves), Some(checkpoint)) => waves.apply_checkpoint(checkpoint)?,
            (slot, Some(checkpoint)) => *slot = Some(WaveDirector::restore(checkpoint)),
            (slot, None) => *slot = None,
        }
        self.tutorial = checkpoint.tutorial;
        self.scenery_retained_bounds = checkpoint.scenery_retained_bounds;
        self.scenery_target_retention = checkpoint.scenery_target_retention;
        self.enemies = checkpoint.enemies;
        self.death_weapons = checkpoint.death_weapons;
        self.next_death_weapon_id = checkpoint.next_death_weapon_id;
        self.loot = checkpoint.loot;
        self.enemy_world_feedback = checkpoint.enemy_world_feedback;
        self.enemy_events = checkpoint.enemy_events;
        self.loot_events = checkpoint.loot_events;
        self.player_ouch_deadline_tick = checkpoint.player_ouch_deadline_tick;
        self.lantern_position = checkpoint.lantern_position;
        self.lantern_light_registration = checkpoint.lantern_light_registration;
        self.solomon_painter_registration = checkpoint.solomon_painter_registration;
        self.dynamic_collision.set_gate_leaves(&self.gate_leaves);
        Ok(())
    }
}
