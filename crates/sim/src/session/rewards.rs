//! Terminal XP and ground pickups, applied before the staff/secondary/primary epochs.

use super::*;
use crate::boneyard::loot::{BoneyardLootEvent, BoneyardLootEventType, BoneyardLootPickup};
use crate::enemies::model::EnemyReward;
use crate::loot::{NativeBonusKind, NativeLootDropKind, NativeOrbKind};
use crate::profile::{NATIVE_SKILL_CATALOG, boneyard_enemy_experience_award};

/// Births and pickup labels applied later in the same source epoch.
pub(super) struct WorldRewardEffects {
    pub unstepped_secondary_actor_ids: Vec<u64>,
    pub loot_text_overrides: Vec<(usize, String)>,
}

impl GameSession {
    pub(super) fn apply_world_rewards(
        &mut self,
        rewards: &[EnemyReward],
        pickups: &[BoneyardLootPickup],
        tick: u64,
        extensions: Option<&dyn GameSimulationExtensions>,
    ) -> Result<WorldRewardEffects, String> {
        let mut unstepped = Vec::new();
        for reward in rewards {
            let Some(id) = reward.player_id.as_deref() else {
                continue;
            };
            let cohort = self.level_up_participant_ids();
            if !cohort.iter().any(|participant| participant == id) {
                continue;
            }
            let player = &self.participants[id].player;
            let amount = boneyard_enemy_experience_award(
                reward.experience,
                cohort.len() as u32,
                player.progression.level,
                1.0,
                player.derived_stats().experience_bonus,
            );
            unstepped
                .extend(self.grant_experience_at_with_extensions(id, amount, tick, extensions));
        }
        let mut bonus_choices = Vec::new();
        let mut text_overrides = Vec::new();
        for pickup in pickups {
            let Some(participant) = self.participants.get_mut(&pickup.player_id) else {
                continue;
            };
            let player = &mut participant.player;
            match pickup.kind {
                NativeLootDropKind::Gold => player.economy.credit_loot_gold(pickup.amount),
                NativeLootDropKind::Sack => {
                    if let Some(item) = &pickup.item {
                        player.economy.insert_loot_item(item.clone());
                    }
                }
                NativeLootDropKind::Orb => match pickup.orb_kind {
                    Some(NativeOrbKind::Health) => {
                        player.combat.restore_health(crate::float::fround(pickup.orb_value * 25.0))
                    }
                    Some(NativeOrbKind::Mana) => {
                        let requested = crate::float::fround(pickup.orb_value * 40.0);
                        super::extensions::apply_filtered_mana_delta(
                            player,
                            &pickup.player_id,
                            requested,
                            crate::modding::extensions::GameSimulationManaSource::StockOrb,
                            tick,
                            extensions,
                            None,
                        )?;
                    }
                    None => {}
                },
                NativeLootDropKind::Bonus => match pickup.bonus_kind {
                    Some(NativeBonusKind::SkillChoice) => {
                        player.grant_bonus_skill_choice(&mut self.gameplay_rng);
                        bonus_choices.push(pickup.player_id.clone());
                        self.assign_automatic_skill_choices_with_extensions(
                            std::slice::from_ref(&pickup.player_id),
                            extensions,
                        );
                    }
                    Some(NativeBonusKind::SkillRank) => {
                        let world =
                            self.boneyard.as_mut().expect("ground bonus belongs to a Boneyard");
                        if let Some(id) = player.increase_random_skill(&mut world.loot.shared_rng) {
                            text_overrides.push((
                                pickup.actor_id,
                                format!("{} +1", NATIVE_SKILL_CATALOG[id].name),
                            ));
                        }
                    }
                    Some(NativeBonusKind::DamageX4) => player.progression.apply_damage_x4_bonus(),
                    None => {}
                },
            }
        }
        if let Some(id) = bonus_choices.first()
            && self.level_up_barrier.is_none()
        {
            let cohort = self.level_up_participant_ids();
            self.refresh_level_up_barrier(id, &cohort);
        }
        Ok(WorldRewardEffects {
            unstepped_secondary_actor_ids: unstepped,
            loot_text_overrides: text_overrides,
        })
    }

    pub(super) fn retain_world_events(
        &mut self,
        enemy_events: &[crate::enemies::model::EnemyEvent],
        mut loot_events: Vec<BoneyardLootEvent>,
        text_overrides: &[(usize, String)],
        tick: u64,
    ) {
        let Some(world) = &mut self.boneyard else {
            return;
        };
        retain_boneyard_enemy_events(world, enemy_events, tick);
        for event in &mut loot_events {
            if event.event_type == BoneyardLootEventType::LootPickup
                && let Some((_, text)) = text_overrides.iter().find(|(id, _)| *id == event.actor_id)
            {
                event.text = Some(text.clone());
            }
        }
        world.loot_events.extend(loot_events);
        world.loot_events.retain(|event| event.tick >= tick.saturating_sub(100));
        if world.loot_events.len() > 512 {
            world.loot_events.drain(..world.loot_events.len() - 512);
        }
    }
}

/// Source retained semantic lane, also used by direct authored damage.
pub(super) fn retain_boneyard_enemy_events(
    world: &mut crate::boneyard::BoneyardWorld,
    enemy_events: &[crate::enemies::model::EnemyEvent],
    tick: u64,
) {
    world.enemy_events.extend_from_slice(enemy_events);
    world.enemy_events.retain(|event| event.tick >= tick.saturating_sub(100));
    world.enemy_events.sort_by_key(|event| event.event_id);
    assert!(
        !world.enemy_events.windows(2).any(|events| events[0].event_id == events[1].event_id),
        "duplicate Boneyard enemy event ID"
    );
    if world.enemy_events.len() > 512 {
        world.enemy_events.drain(..world.enemy_events.len() - 512);
    }
}
