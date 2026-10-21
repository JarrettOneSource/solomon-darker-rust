//! Ordered terminal loot composition from `boneyard-world.ts:539–621`.

use crate::Vec2;
use crate::boneyard::waves::programs::BossKind;
use crate::economy::HubInventoryItem;
use crate::enemies::EnemyToken;
use crate::enemies::model::EnemyReward;
use crate::enemies::world::EnemyWorld;
use crate::loot::{
    NativeLootArenaInput, NativeLootDropKind, NativeLootKeyInput, NativeLootModifiers,
    NativeLootParticipantInput, NativeLootPolicies, NativeLootPolicy, NativeLootSelectionInput,
    dire_boss_item_recipe_pool,
};
use crate::painter::WorldPainterOrder;

use super::super::collision::can_place_boneyard_body;
use super::super::tutorial::{native_tutorial_amulet_item, native_tutorial_health_potion_item};
use super::{BoneyardPlayerCombatStatus, BoneyardWorld};

/// The source uses retained pre-death rows, including dying enemies and Maggots, for these
/// two loot queries. Capture only the fields they read before the live store advances.
pub(super) struct LootEnemySnapshot {
    badguy_count: usize,
    positions: Vec<(usize, Vec2)>,
}

impl LootEnemySnapshot {
    pub(super) fn new(enemies: &EnemyWorld) -> Self {
        let mut positions = Vec::with_capacity(enemies.actors.len() + enemies.maggots.len());
        for actor in &enemies.actors {
            positions.push((actor.id, actor.position));
        }
        for actor in &enemies.maggots {
            positions.push((actor.id, actor.position));
        }
        Self { badguy_count: enemies.actors.len(), positions }
    }

    fn nearby_count(&self, excluded_actor_id: usize, position: Vec2) -> usize {
        let mut count = 0;
        for (id, candidate) in &self.positions {
            let dx = candidate.x - position.x;
            let dy = candidate.y - position.y;
            if *id != excluded_actor_id && dx * dx + dy * dy < 250.0 * 250.0 {
                count += 1;
            }
        }
        count
    }
}

impl BoneyardWorld {
    pub(super) fn materialize_enemy_rewards(
        &mut self,
        rewards: &[EnemyReward],
        snapshot: &LootEnemySnapshot,
        players: &[BoneyardPlayerCombatStatus<'_>],
        tick: u64,
        painter_order: &mut WorldPainterOrder,
        custom_loot: Option<&dyn Fn(u32, EnemyToken) -> Vec<HubInventoryItem>>,
    ) {
        let bounds = self.active_bounds();
        let collision = &self.dynamic_collision;
        let placement =
            |position, radius| can_place_boneyard_body(position, bounds, collision, radius);
        let arena_level = self
            .waves
            .as_ref()
            .map(|waves| waves.state.wave_ordinal)
            .or_else(|| self.tutorial.as_ref().map(|tutorial| tutorial.wave_ordinal))
            .unwrap_or(0);
        for (index, reward) in rewards.iter().enumerate() {
            let player =
                players.iter().find(|player| reward.player_id.as_deref() == Some(player.status.id));
            let policies = reward.loot_source.policies.unwrap_or_default();
            let policy = |id| {
                NativeLootPolicy::from_id(u32::from(id))
                    .expect("authored enemy loot policy is in0..6")
            };
            let world_has_health_potion_sack = self.loot.actors.iter().any(|actor| {
                actor.kind == NativeLootDropKind::Sack
                    && actor.item.as_ref().is_some_and(|item| {
                        item.native_type_id == 7001 && item.native_subtype == Some(0)
                    })
            });
            let remaining_goodies = self
                .loot
                .goodies
                .iter()
                .filter(|goodie| !goodie.active && !goodie.exhausted)
                .count();
            // Source obtains this identity from the retained Faculty encounter.
            // Its authored programs share that exact source SHA with the director.
            let faculty_source = self.waves.as_ref().and_then(|waves| {
                if !waves.state.boss_encounters.iter().any(|boss| boss.kind == BossKind::Faculty) {
                    return None;
                }
                waves.programs.as_ref().map(|programs| programs.source_sha256.as_str())
            });
            let item_recipe_pool = dire_boss_item_recipe_pool(
                faculty_source,
                reward.loot_source.enemy_token,
                reward.loot_source.recipe_uid,
            );
            self.loot.materialize_enemy_loot(
                NativeLootSelectionInput {
                    actor_seed: reward.loot_source.actor_seed,
                    ground_recipe_indexes: &[],
                    item_recipe_pool: item_recipe_pool.as_deref(),
                    arena: NativeLootArenaInput {
                        disable_mask: 0,
                        item_level_maximum: 100,
                        item_level_minimum: 0,
                        last_successful_item_level: self.loot.last_successful_item_level,
                        level: arena_level as i32,
                        mode: 0,
                        special_suppression: false,
                    },
                    explicit_gold_amount: None,
                    drop_delay_context: 0,
                    inventory_has_health_potion: player
                        .is_some_and(|player| player.inventory_has_health_potion),
                    key: NativeLootKeyInput {
                        current: self.loot.next_key_drop_level,
                        level: arena_level as u32,
                        remaining: remaining_goodies,
                    },
                    nearby_mask_two_count: snapshot
                        .nearby_count(reward.actor_id, reward.loot_source.position),
                    participant: NativeLootParticipantInput {
                        advanced_unlocks: player
                            .map_or(&[false; 8], |player| player.advanced_unlocks),
                        level: player.map_or(1, |player| player.level),
                        modifiers: player.map_or_else(NativeLootModifiers::default, |player| {
                            player.loot_modifiers
                        }),
                        owned_recipe_indexes: player
                            .map_or(&[][..], |player| player.owned_recipe_indexes),
                        slot: usize::from(reward.loot_source.participant_slot),
                    },
                    placement: &placement,
                    policies: NativeLootPolicies {
                        gold: policy(policies.gold),
                        item: policy(policies.item),
                        orb: policy(policies.orb),
                        potion: policy(policies.potion),
                        powerup: policy(policies.powerup),
                        specific_item: policies.specific_item,
                    },
                    scene_forces_health_potion: false,
                    source_position: reward.loot_source.position,
                    world_badguy_count: snapshot
                        .badguy_count
                        .checked_sub(index + 1)
                        .expect("terminal rewards belong to retained pre-step actors"),
                    world_has_health_potion_sack,
                },
                reward.loot_source.on_death_program,
                tick,
                painter_order,
            );
            if reward.loot_source.recipe_uid == Some(10051)
                && self.tutorial.as_ref().is_some_and(|tutorial| tutorial.item_drop_armed)
            {
                self.loot.spawn_custom_items(
                    &[native_tutorial_amulet_item()],
                    reward.loot_source.position,
                    tick,
                    painter_order,
                );
                self.tutorial.as_mut().expect("Tutorial item drop is armed").item_drop_armed =
                    false;
            } else if reward.loot_source.recipe_uid == Some(10065) && self.tutorial.is_some() {
                self.loot.spawn_custom_items(
                    &[native_tutorial_health_potion_item()],
                    reward.loot_source.position,
                    tick,
                    painter_order,
                );
            }
            if let Some(custom_loot) = custom_loot {
                let items =
                    custom_loot(reward.loot_source.actor_seed, reward.loot_source.enemy_token);
                if !items.is_empty() {
                    self.loot.spawn_custom_items(
                        &items,
                        reward.loot_source.position,
                        tick,
                        painter_order,
                    );
                }
            }
        }
    }
}
