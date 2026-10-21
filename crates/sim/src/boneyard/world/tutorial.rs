//! Actual Tutorial player/world projection from `game-simulation.ts:1930–2082`.

use crate::HubWizard;
use crate::combat::PlayerCombatState;
use crate::economy::{HubEconomyState, HubInventoryItem};
use crate::enemies::targeting::boneyard_enemy_collision_radius;
use crate::loot::{NATIVE_LOOT_CARRIER_PLACEMENT_RADIUS, NativeLootDropKind};
use crate::secondary::{NativeSecondaryAbilityId, NativeSecondaryPlayerState};

use super::super::tutorial::{
    NativeTutorialTickInput, NativeTutorialTickResult, native_tutorial_camera_lock_safety_clear,
    native_tutorial_forced_velocity,
};
use super::BoneyardWorld;

pub struct BoneyardTutorialPlayer<'a> {
    pub combat: &'a PlayerCombatState,
    pub economy: &'a HubEconomyState,
    pub level: u32,
    pub level_up_pending: bool,
    pub secondary: Option<&'a NativeSecondaryPlayerState>,
}

impl BoneyardWorld {
    /// Session admits inputs from the incoming Tutorial HUD access before this phase,
    /// then applies returned offers/XP before `step`. All counts here come from retained
    /// entities; health potions include addressed children inside inventory Sacks.
    pub fn step_tutorial(
        &mut self,
        wizard: &mut HubWizard,
        player: BoneyardTutorialPlayer<'_>,
        tick: u64,
    ) -> Option<NativeTutorialTickResult> {
        let tutorial = self.tutorial.as_mut()?;
        if let Some(velocity) = native_tutorial_forced_velocity(tutorial) {
            wizard.velocity = velocity;
        }
        let mut safety_bodies = Vec::new();
        for actor in &self.enemies.actors {
            safety_bodies.push((actor.position, boneyard_enemy_collision_radius(actor)));
        }
        for maggot in &self.enemies.maggots {
            safety_bodies.push((maggot.position, maggot.collision_radius));
        }
        let mut ground_sack_count = 0;
        for actor in &self.loot.actors {
            if actor.kind == NativeLootDropKind::Sack {
                ground_sack_count += 1;
                safety_bodies.push((actor.position, NATIVE_LOOT_CARRIER_PLACEMENT_RADIUS));
            }
        }
        Some(
            tutorial.step(NativeTutorialTickInput {
                acid_rain_cast_sequence: player
                    .secondary
                    .map_or(0, |secondary| secondary.cast_sequence),
                acid_rain_last_skill_id: player
                    .secondary
                    .and_then(|secondary| secondary.last_skill_id)
                    .map(NativeSecondaryAbilityId::skill_id),
                camera_lock_safety_clear: native_tutorial_camera_lock_safety_clear(&safety_bodies),
                current_health: player.combat.current_health,
                enemy_count: self.enemies.actors.len(),
                ground_sack_count,
                has_top_level_non_potion_item: player
                    .economy
                    .backpack
                    .iter()
                    .any(|item| item.native_type_id != 7001),
                health_potion_count: count_health_potions(&player.economy.backpack),
                level: player.level,
                level_up_pending: player.level_up_pending,
                maximum_health: player.combat.maximum_health,
                player_action_idle: !wizard.primary_cast.held
                    && player.secondary.is_none_or(|secondary| secondary.cast_action.is_none()),
                player_position: wizard.position,
                primary_cast_sequence: wizard.primary_cast.cast_sequence,
                solomon_phase: self.encounter.as_ref().map(|encounter| encounter.phase),
                solomon_run_event_id: self
                    .encounter
                    .as_ref()
                    .map_or(0, |encounter| encounter.run_event_id),
                tick,
            }),
        )
    }
}

fn count_health_potions(items: &[HubInventoryItem]) -> usize {
    let mut count = 0;
    for item in items {
        if item.native_type_id == 7001 && item.native_subtype == Some(0) {
            count += item.quantity as usize;
        }
        if let Some(contents) = &item.contents {
            count += count_health_potions(contents);
        }
    }
    count
}
