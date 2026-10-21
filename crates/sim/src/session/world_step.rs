//! Input admission and the one shared movement/world epoch of the public Game tick.

use super::*;
use crate::Vec2;
use crate::belt::NativeBeltEntry;
use crate::boneyard::navigation::NavigationSet;
use crate::boneyard::tutorial::{NativeTutorialSurfaceAction, native_tutorial_hud_access};
use crate::boneyard::world::{
    BoneyardPlayerCombatStatus, BoneyardTutorialPlayer, BoneyardWorldParticipant,
    BoneyardWorldTickContext, BoneyardWorldTickResult,
};
use crate::boneyard::world_light::BoneyardLightEnvironment;
use crate::economy::{EquipmentSlot, HubEconomyState, HubInventoryItem, project_inventory_items};
use crate::game_run::GameRunPhase;
use crate::hub::interaction::HubNpcMarkerEpoch;
use crate::loot::{NativeLootAttractionModifiers, native_loot_modifiers};
use crate::painter::WorldManagerLane;
use crate::profile::native_skill_category;
use crate::spells::model::PrimarySpellTransient;
use crate::spells::staff::NativePlayerStaffTransient;
use std::collections::{BTreeMap, BTreeSet};

/// Hosts project pointers/controllers into world-space input exactly once.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct GameSessionInput {
    pub aim: Option<Vec2>,
    pub movement: Vec2,
    pub primary: bool,
    pub quickbar: Option<usize>,
    pub viewport: Vec2,
}
impl Default for GameSessionInput {
    fn default() -> Self {
        Self {
            aim: None,
            movement: Vec2::ZERO,
            primary: false,
            quickbar: None,
            viewport: Vec2::new(1600.0, 900.0),
        }
    }
}

impl GameSession {
    pub fn apply_tutorial_action(&mut self, action: NativeTutorialSurfaceAction) -> bool {
        let Some(tutorial) = self.boneyard.as_mut().and_then(|world| world.tutorial.as_mut())
        else {
            return false;
        };
        tutorial.apply_surface_action(action);
        true
    }

    pub(super) fn admit_inputs(
        &self,
        inputs: &BTreeMap<String, GameSessionInput>,
    ) -> BTreeMap<String, GameSessionInput> {
        let mut admitted = BTreeMap::new();
        for id in &self.participant_order {
            let participant = &self.participants[id];
            let mut input = inputs.get(id).copied().unwrap_or_default();
            if participant.player.progression.pending_offer.is_some()
                || !participant.player.combat.can_accept_input()
                || !matches!(self.run.phase, GameRunPhase::Hub | GameRunPhase::Active)
            {
                input =
                    GameSessionInput { viewport: input.viewport, ..GameSessionInput::default() };
            }
            if self.boneyard.is_none() {
                input = self.seal_combat_input(id, input);
            }
            if let Some(tutorial) = self.boneyard.as_ref().and_then(|world| world.tutorial.as_ref())
            {
                input.primary &= tutorial.stage.id() >= 2;
                if !native_tutorial_hud_access(tutorial).quickbar {
                    input.quickbar = None;
                }
            }
            let acting_staff = self.spells.transients.iter().any(|effect| match effect {
                PrimarySpellTransient::Staff(NativePlayerStaffTransient::Action(actor)) => {
                    actor.common().owner_id == *id
                }
                _ => false,
            });
            if acting_staff {
                input.primary = false;
            }
            admitted.insert(id.clone(), input);
        }
        admitted
    }

    pub(super) fn seal_combat_input(
        &self,
        id: &str,
        mut input: GameSessionInput,
    ) -> GameSessionInput {
        let player = &self.participants[id].player;
        let category =
            input.quickbar.and_then(|slot| player.belt.entries.get(slot)).and_then(|entry| {
                match entry {
                    Some(NativeBeltEntry::Skill { skill_id }) => native_skill_category(*skill_id),
                    _ => None,
                }
            });
        input.aim = None;
        input.primary = false;
        if !matches!(category, Some(1 | 3)) {
            input.quickbar = None;
        }
        input
    }

    pub(super) fn step_world(
        &mut self,
        inputs: &BTreeMap<String, GameSessionInput>,
        tick: u64,
        navigation: Option<&NavigationSet>,
        external: &[crate::enemies::EnemySpawnIntent],
        college_ready: Option<&BTreeSet<String>>,
        extensions: Option<&dyn GameSimulationExtensions>,
    ) -> (Option<BoneyardWorldTickResult>, Vec<u64>) {
        if self.boneyard.is_none() {
            self.step_hub_world(inputs, college_ready);
            return (None, Vec::new());
        }
        let tutorial = if self.boneyard.as_ref().unwrap().tutorial.is_some() {
            let id = &self.participant_order[0];
            let participant = self.participants.get_mut(id).unwrap();
            self.boneyard.as_mut().unwrap().step_tutorial(
                &mut participant.wizard,
                BoneyardTutorialPlayer {
                    combat: &participant.player.combat,
                    economy: &participant.player.economy,
                    level: participant.player.progression.level,
                    level_up_pending: self.level_up_barrier.is_some(),
                    secondary: self.secondary.players.get(id),
                },
                tick,
            )
        } else {
            None
        };
        let mut spawns = Vec::new();
        let mut unstepped = Vec::new();
        if let Some(tutorial) = tutorial {
            let id = self.participant_order[0].clone();
            if let Some(ids) = tutorial.force_offer_skill_ids {
                self.participants.get_mut(&id).unwrap().player.progression.forced_offer_skill_ids =
                    ids.to_vec();
            }
            if tutorial.grant_experience > 0.0 {
                unstepped = self.grant_experience_at_with_extensions(
                    &id,
                    tutorial.grant_experience,
                    tick,
                    extensions,
                );
            }
            spawns = tutorial.spawn_intents;
        }
        spawns.extend_from_slice(external);
        let recipes: BTreeMap<_, _> = self
            .participants
            .iter()
            .map(|(id, participant)| {
                (id.clone(), economy_owned_recipes(&participant.player.economy))
            })
            .collect();
        let environment = BoneyardLightEnvironment {
            primary_spells: Some(&self.spells),
            secondary_abilities: Some(&self.secondary),
        };
        let mut rows = Vec::with_capacity(self.participants.len());
        for participant in ordered_participants_mut(&mut self.participants, &self.participant_order)
        {
            let id = participant.id.as_str();
            let player = &participant.player;
            let derived = player.derived_stats();
            let has_health_potion =
                project_inventory_items(&player.economy.backpack).iter().any(|entry| {
                    entry.item.native_type_id == 7001 && entry.item.native_subtype == Some(0)
                });
            let modifiers = native_loot_modifiers(
                &player.economy.owned_perk_selectors,
                NativeLootAttractionModifiers {
                    gold_amount: derived.gold_amount_multiplier,
                    orb_pull: derived.orb_pull_multiplier,
                    pickup_factor: derived.pickup_range_scalar,
                },
            );
            let status = crate::boneyard::world::BoneyardPlayerStatus {
                id,
                alive: player.combat.life_state == crate::combat::PlayerLifeState::Alive,
                eligible: self.run.eligible_player_ids.contains(&participant.id),
                collision_enabled: player.combat.collision_enabled(),
                movement_scale: player.combat.movement_scale() * derived.movement_factor,
            };
            let input = inputs[id];
            rows.push(BoneyardWorldParticipant {
                wizard: &mut participant.wizard,
                combat: BoneyardPlayerCombatStatus {
                    status,
                    advanced_unlocks: &player.book.advanced_unlocks,
                    inventory_has_health_potion: has_health_potion,
                    level: player.progression.level,
                    loot_modifiers: modifiers,
                    owned_recipe_indexes: &recipes[id],
                },
                movement: input.movement,
                force_movement_facing: player.combat.dazzle_ticks_remaining > 0,
                viewport: input.viewport,
                light_registration: Some(participant.lighting.light_registration),
                overlay_effect_phase: participant.lighting.overlay_effect_phase,
            });
        }
        let custom_loot = extensions.map(|extensions| {
            move |actor_seed: u32, enemy_token| {
                extensions.create_loot_items(crate::modding::extensions::GameSimulationLootInput {
                    actor_seed: u64::from(actor_seed),
                    enemy_token,
                })
            }
        });
        let result = self.boneyard.as_mut().unwrap().step(
            BoneyardWorldTickContext {
                enhanced_effects: self.enhanced_effects,
                tick,
                participants: &mut rows,
                light_environment: environment,
                external_spawn_intents: spawns,
                navigation: navigation.expect("active Boneyard has identity-checked navigation"),
                custom_loot: custom_loot.as_ref().map(|factory| {
                    factory
                        as &dyn Fn(
                            u32,
                            crate::enemies::EnemyToken,
                        ) -> Vec<crate::economy::HubInventoryItem>
                }),
            },
            &mut self.painter_order,
        );
        (Some(result), unstepped)
    }

    pub(super) fn dispatch_belt_selection(
        &mut self,
        id: &str,
        slot: Option<usize>,
        primary_unset: bool,
        extensions: Option<&dyn GameSimulationExtensions>,
    ) {
        let Some(slot) = slot else {
            return;
        };
        let participant = self.participants.get_mut(id).unwrap();
        let Some(Some(NativeBeltEntry::Skill { skill_id })) =
            participant.player.belt.entries.get(slot)
        else {
            return;
        };
        let skill_id = *skill_id;
        match native_skill_category(skill_id) {
            Some(1) if !primary_unset => {
                let previous = participant.player.book.primary_skill_id;
                if participant.player.select_primary(skill_id)
                    && previous != participant.player.book.primary_skill_id
                {
                    let selected = if skill_id == 52 {
                        participant.player.book.weld_build_id.expect("selected Weld has a build")
                    } else {
                        skill_id as u32
                    };
                    participant.wizard.primary_cast.reset_for_selection(selected);
                }
            }
            Some(2) => self.fail_boast_with_extensions(
                id,
                crate::hub::interaction::BoastFailureProducer::SecondaryCast,
                extensions,
            ),
            Some(3) => {
                let held = self.secondary.players.get(id).and_then(|row| row.held_slot);
                if held != Some(slot)
                    && participant.player.progression.mind_chug_ticks_remaining == 0
                    && participant.player.book.permanent_ranks[skill_id] > 0
                    && participant.player.book.effective_ranks[skill_id] > 0
                {
                    participant.player.select_concentration(skill_id);
                }
            }
            _ => {}
        }
    }

    fn step_hub_world(
        &mut self,
        inputs: &BTreeMap<String, GameSessionInput>,
        ready: Option<&BTreeSet<String>>,
    ) {
        let results = {
            let mut rows = Vec::with_capacity(self.participants.len());
            for participant in
                ordered_participants_mut(&mut self.participants, &self.participant_order)
            {
                rows.push(crate::hub::world::HubParticipantMotion {
                    id: &participant.id,
                    movement: inputs[&participant.id].movement,
                    movement_scale: participant.player.combat.movement_scale()
                        * participant.player.derived_stats().movement_factor,
                    force_movement_facing: participant.player.combat.dazzle_ticks_remaining > 0,
                    college_intro_pending: participant.player.economy.college_intro_pending,
                    renderer_ready: ready.is_none_or(|ids| ids.contains(&participant.id)),
                    wizard: &mut participant.wizard,
                    college_intro: &mut participant.college_intro,
                });
            }
            crate::hub::world::step_hub_participants(
                &mut rows,
                &mut self.npcs,
                &mut self.collision_rng,
                &mut self.painter_order,
            )
        };
        self.ambient.step(Some(&mut self.painter_order));
        self.dowser_clock.step();
        for result in results {
            let participant = self.participants.get_mut(&result.id).unwrap();
            if result.region_changed {
                participant.lighting.light_registration =
                    self.painter_order.register(WorldManagerLane::Actor);
                participant.marker_epoch = HubNpcMarkerEpoch::new(
                    self.hub_trader_animation_seed,
                    self.ticks + 1,
                    &participant.player.economy.npc.help_flags,
                );
            }
            if result.clear_college_intro_pending {
                participant.player.economy.college_intro_pending = false;
                participant.player.economy.revision += 1;
                participant.player.refresh_economy();
            }
        }
    }
}

fn economy_owned_recipes(economy: &HubEconomyState) -> Vec<usize> {
    fn visit(item: &HubInventoryItem, output: &mut Vec<usize>) {
        if let Some(index) = item.recipe_index
            && !output.contains(&index)
        {
            output.push(index);
        }
        if let Some(children) = &item.contents {
            for child in children {
                visit(child, output);
            }
        }
    }
    let mut indexes = Vec::new();
    for item in economy.backpack.iter().chain(&economy.storage) {
        visit(item, &mut indexes);
    }
    for slot in EquipmentSlot::ALL {
        if let Some(item) = economy.equipment.at(slot) {
            visit(item, &mut indexes);
        }
    }
    indexes
}
