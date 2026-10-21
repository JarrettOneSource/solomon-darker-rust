//! The stateful v7 observation transaction from Source `ml-bot-policy/observer.ts`.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use crate::hub::PLAYER_CHARACTER_RADIUS;
use crate::session::{GameSession, GameSessionInput};

use super::OBSERVATION_WIDTH;
use super::enemies::{
    MlBotPolicyEnemyMemory, MlBotPolicyEnemyObservationOptions, MlBotPolicyEnemyRow,
    observe_ml_bot_policy_enemies,
};
use super::enemy_extensions::observe_ml_bot_policy_enemy_extensions;
use super::geometry::{MlBotPolicyGeometryObservation, observe_ml_bot_policy_geometry};
use super::hazards::observe_ml_bot_policy_hazards;
use super::inventory::{MlBotPolicyInventoryObservation, observe_ml_bot_policy_inventory};
use super::minions::{MlBotPolicyMinionObservation, observe_ml_bot_policy_minions};
use super::own_effects::observe_ml_bot_policy_own_effects;
use super::player_state::{MlBotPolicyPlayerObservation, observe_ml_bot_policy_player_state};
use super::skills::observe_ml_bot_policy_skill_loadout;
use super::spec::{BLOCKS, OBSERVATION_NAMES};
use super::world_state::{
    MlBotPolicyCommittedAction, MlBotPolicyHostContext, MlBotPolicyWorldMemory,
    observe_ml_bot_policy_world_state,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MlBotPolicyControllerKind {
    Bot,
    Human,
}

pub struct MlBotPolicyObservationContext<'a> {
    pub active_inputs: &'a BTreeMap<String, GameSessionInput>,
    pub controllers: &'a BTreeMap<String, MlBotPolicyControllerKind>,
    pub has_consumable: Option<&'a dyn Fn(&str) -> bool>,
    /// The host reuses its existing pinned en-US SourceCollator. Keeping that
    /// borrowed operation here avoids a sim-to-platform dependency or byte-sort
    /// substitution when arbitrary retained player IDs tie at the same distance.
    pub compare_player_ids: &'a dyn Fn(&str, &str) -> Ordering,
}

#[derive(Clone, Debug)]
pub struct MlBotPolicyObservedBlock {
    pub key: char,
    pub values: Vec<f32>,
}

#[derive(Clone, Debug)]
pub struct MlBotPolicyFrame {
    pub blocks: Vec<MlBotPolicyObservedBlock>,
    pub enemy_rows: Vec<MlBotPolicyEnemyRow>,
    pub geometry: MlBotPolicyGeometryObservation,
    pub inventory: MlBotPolicyInventoryObservation,
    pub loadout: Vec<f32>,
    pub minions: MlBotPolicyMinionObservation,
    pub player: MlBotPolicyPlayerObservation,
    pub target_id: Option<usize>,
    pub values: Vec<f32>,
}

#[derive(Clone, Debug)]
pub struct MlBotPolicyObserver {
    enemy_memory: MlBotPolicyEnemyMemory,
    player_id: String,
    previous_action: Option<MlBotPolicyCommittedAction>,
    world_memory: MlBotPolicyWorldMemory,
}

impl MlBotPolicyObserver {
    pub fn new(player_id: String) -> Result<Self, String> {
        if player_id.is_empty() {
            return Err("ML bot policy player id must not be empty".into());
        }
        Ok(Self {
            enemy_memory: MlBotPolicyEnemyMemory::default(),
            player_id,
            previous_action: None,
            world_memory: MlBotPolicyWorldMemory::default(),
        })
    }

    pub fn observe(
        &mut self,
        session: &GameSession,
        context: &MlBotPolicyObservationContext<'_>,
    ) -> Result<MlBotPolicyFrame, String> {
        let world = session.boneyard.as_ref().ok_or("ML bot policy requires a Boneyard world")?;
        let participant = session
            .participant(&self.player_id)
            .ok_or_else(|| format!("ML bot policy has no player {}", self.player_id))?;
        let position = participant.wizard.position;
        let quickbar = participant.player.belt.skill_projection();
        // The pinned observer passes raw runId, although live spell ownership uses
        // boneyard:<runId>. Preserve that deployed policy admission exactly.
        let world_key = &world.loaded.run_id;
        let minions = observe_ml_bot_policy_minions(
            &session.secondary,
            &self.player_id,
            position,
            &quickbar,
            world_key,
        );
        let own_effects = observe_ml_bot_policy_own_effects(
            &session.spells,
            &session.secondary,
            &self.player_id,
            position,
            &quickbar,
            world_key,
        );
        let activity = std::array::from_fn(|slot| {
            own_effects.secondary_effect_active[slot] || minions.secondary_minion_active[slot]
        });
        let player = observe_ml_bot_policy_player_state(
            session,
            &self.player_id,
            own_effects.primary_effect_active,
            &activity,
        )?;
        let enemies = observe_ml_bot_policy_enemies(
            &world.enemies,
            MlBotPolicyEnemyObservationOptions {
                memory: &self.enemy_memory,
                own_minion_target_ids: &minions.own_target_ids,
                primary_range: player.primary_range,
                self_position: position,
                tick: session.ticks,
            },
        );
        let extensions = observe_ml_bot_policy_enemy_extensions(
            &world.enemies,
            &enemies.rows,
            &session.secondary,
            &self.player_id,
            session.ticks,
            world_key,
        );
        let geometry = observe_ml_bot_policy_geometry(
            world.active_bounds(),
            &world.dynamic_collision,
            &world.scenery_gravestone_target_ids(),
            position,
        );
        let hazards = observe_ml_bot_policy_hazards(
            &world.enemies,
            &self.player_id,
            position,
            PLAYER_CHARACTER_RADIUS,
        );
        let inventory =
            observe_ml_bot_policy_inventory(session, &self.player_id, context.has_consumable)?;
        let loadout = observe_ml_bot_policy_skill_loadout(session, &self.player_id)?;
        let target_id = enemies.next_memory.target_id;
        let world_observation = observe_ml_bot_policy_world_state(
            session,
            &self.player_id,
            &enemies.rows,
            MlBotPolicyHostContext {
                observation: context,
                memory: &self.world_memory,
                previous_action: self.previous_action,
                target_id,
            },
        )?;
        let block_values = [
            player.block_a.clone(),
            player.block_b.clone(),
            player.block_c.clone(),
            enemies.block_d,
            enemies.block_e,
            geometry.patch_and_rays.clone(),
            world_observation.block_g,
            world_observation.block_i,
            world_observation.block_h,
            world_observation.block_j,
            extensions,
            enemies.block_l,
            geometry.obstacles.clone(),
            hazards,
            inventory.block_o.clone(),
            inventory.block_p.clone(),
            inventory.block_q.clone(),
            own_effects.block_r,
            minions.block_s.clone(),
            loadout.clone(),
        ];
        let mut values = vec![0.0; OBSERVATION_WIDTH];
        let mut blocks = Vec::with_capacity(BLOCKS.len());
        for (spec, block_values) in BLOCKS.iter().zip(block_values) {
            if block_values.len() != spec.length {
                return Err(format!("ML bot policy Block {} does not match schema v7", spec.key));
            }
            values[spec.start..spec.start + spec.length].copy_from_slice(&block_values);
            blocks.push(MlBotPolicyObservedBlock { key: spec.key, values: block_values });
        }
        for (index, value) in values.iter().enumerate() {
            if !value.is_finite() {
                return Err(format!(
                    "ML bot policy observation {} is not finite",
                    OBSERVATION_NAMES[index]
                ));
            }
        }
        // Source commits both histories only after the complete row passes its
        // finite/schema checks; rejected observations leave future history intact.
        self.enemy_memory = enemies.next_memory;
        self.world_memory = world_observation.next_memory;
        Ok(MlBotPolicyFrame {
            blocks,
            enemy_rows: enemies.rows,
            geometry,
            inventory,
            loadout,
            minions,
            player,
            target_id,
            values,
        })
    }

    pub fn commit(&mut self, action: MlBotPolicyCommittedAction, target_id: Option<usize>) {
        self.previous_action = Some(action);
        self.enemy_memory.target_id = target_id;
    }

    pub fn reset(&mut self) {
        self.enemy_memory = MlBotPolicyEnemyMemory::default();
        self.previous_action = None;
        self.world_memory = MlBotPolicyWorldMemory::default();
    }
}
