//! The authoritative shared tick from `core-server/game-simulation.ts`.
//! Global actor epochs surround participant phases in source entity order.

use super::*;
use crate::Vec2;
use crate::audio::AudioEvent;
use crate::boneyard::navigation::NavigationSet;
use crate::boneyard::world::BoneyardWorldTickResult;
use crate::enemies::EnemySpawnIntent;
use crate::enemies::model::EnemyDamageRequest;
use crate::game_run::GameRunPhase;
use crate::secondary::NativeSecondaryTickResult;
use crate::spells::air_water::finalize_air_water_player_visual_actors;
use crate::spells::contacts::BoneyardSpellCombatResult;
use crate::spells::model::PrimarySpellTickResult;
use std::collections::{BTreeMap, BTreeSet};

/// Immutable navigation is identity-checked on loading. `None` readiness is the
/// source's unspecified set: every participant's College renderer is ready.
#[derive(Default)]
pub struct GameSessionTickContext<'a> {
    pub navigation: Option<&'a NavigationSet>,
    pub college_intro_ready_player_ids: Option<&'a BTreeSet<String>>,
    pub enemy_spawn_intents: &'a [EnemySpawnIntent],
}

pub struct GameSessionTickResult {
    pub advanced: bool,
    pub tick: u64,
    pub world: Option<BoneyardWorldTickResult>,
    pub primary: Option<PrimarySpellTickResult>,
    pub secondary: Option<NativeSecondaryTickResult>,
    pub primary_contacts: Option<BoneyardSpellCombatResult>,
    pub audio_events: Vec<AudioEvent>,
}
impl GameSessionTickResult {
    pub(super) fn frozen(tick: u64) -> Self {
        Self {
            advanced: false,
            tick,
            world: None,
            primary: None,
            secondary: None,
            primary_contacts: None,
            audio_events: Vec::new(),
        }
    }
}

impl GameSession {
    pub fn step(
        &mut self,
        inputs: &BTreeMap<String, GameSessionInput>,
        context: GameSessionTickContext<'_>,
    ) -> GameSessionTickResult {
        self.step_with_extensions(inputs, context, None)
            .expect("stock Session epochs have finite native mutations")
    }

    pub fn step_with_extensions(
        &mut self,
        inputs: &BTreeMap<String, GameSessionInput>,
        context: GameSessionTickContext<'_>,
        extensions: Option<&dyn GameSimulationExtensions>,
    ) -> Result<GameSessionTickResult, String> {
        self.close_dye_sessions_for_inputs(inputs);
        // Source expiry happens before terminal and level-up early returns, against
        // the retained tick rather than the tick about to advance.
        self.mod_effects.retain(|effect| effect.expires_tick > self.ticks);
        if self.run.phase == GameRunPhase::GameOver {
            let result = self.step_game_over(extensions)?;
            self.close_inactive_dye_sessions();
            return Ok(result);
        }
        if self.level_up_barrier.is_some() {
            self.close_inactive_dye_sessions();
            return Ok(GameSessionTickResult::frozen(self.ticks));
        }
        let tick = self.ticks + 1;
        let before_runtime: BTreeMap<_, _> = self
            .participants
            .iter()
            .map(|(id, participant)| (id.clone(), participant.player.skill_runtime.clone()))
            .collect();
        let previous_walk_cycles: BTreeMap<_, _> = self
            .participants
            .iter()
            .map(|(id, participant)| (id.clone(), participant.wizard.walk_cycle_primary))
            .collect();
        let previous_owners: Vec<_> =
            self.secondary.actors.iter().map(|actor| (actor.id, actor.owner_id.clone())).collect();
        let previous_wave = self
            .boneyard
            .as_ref()
            .and_then(|world| world.waves.as_ref())
            .map(|waves| waves.state.phase);
        let previous_enemy_event = self
            .boneyard
            .as_ref()
            .and_then(|world| world.enemy_events.last())
            .map_or(0, |event| event.event_id);
        let inputs = self.admit_inputs(inputs);
        for participant in ordered_participants_mut(&mut self.participants, &self.participant_order)
        {
            participant.world_aim = inputs[&participant.id].aim;
        }
        let (world_result, mut unstepped_secondary) = self.step_world(
            &inputs,
            tick,
            context.navigation,
            context.enemy_spawn_intents,
            context.college_intro_ready_player_ids,
            extensions,
        );
        for participant in self.participants.values_mut() {
            participant.lighting.step_overlay();
        }
        self.step_rescue_protection();
        if let Some(result) = &world_result
            && let Some(tutorial) = self.boneyard.as_mut().unwrap().tutorial.as_mut()
            && let Some(id) = self.participant_order.first()
        {
            tutorial.acknowledge_movement_instruction(
                result.movement.get(id).is_some_and(|movement| movement.movement_active),
                inputs[id].movement != Vec2::ZERO,
            );
        }
        let mut combat_inputs = inputs.clone();
        if self.boneyard.as_ref().is_some_and(|world| {
            world.encounter.as_ref().is_some_and(|encounter| !encounter.combat_enabled())
        }) {
            for (id, input) in &mut combat_inputs {
                *input = self.seal_combat_input(id, *input);
            }
        }
        let contacts = world_result
            .as_ref()
            .map(|result| {
                let mut players: Vec<_> =
                    ordered_participants_mut(&mut self.participants, &self.participant_order)
                        .into_iter()
                        .map(|participant| super::player_contacts::PlayerContactPlayer {
                            player_id: &participant.id,
                            player: &mut participant.player,
                            wizard: &mut participant.wizard,
                            lighting: &mut participant.lighting,
                        })
                        .collect();
                super::player_contacts::apply_player_contacts_with_extensions(
                    super::player_contacts::PlayerContactContext {
                        enhanced_effects: self.enhanced_effects,
                        players: &mut players,
                        world: self.boneyard.as_mut().unwrap(),
                        secondary: &mut self.secondary,
                        tick,
                        painter_order: &mut self.painter_order,
                        screen_flashes: &mut self.screen_flashes,
                    },
                    &result.player_damage,
                    extensions,
                )
            })
            .transpose()?;
        let mut text_overrides = Vec::new();
        if let Some(result) = &world_result {
            let rewards =
                self.apply_world_rewards(&result.rewards, &result.loot.pickups, tick, extensions)?;
            unstepped_secondary.extend(rewards.unstepped_secondary_actor_ids);
            text_overrides = rewards.loot_text_overrides;
        }
        let mut acting_staff = BTreeSet::new();
        let mut staff_camera_displacements = Vec::new();
        if let Some(result) = &world_result {
            let staff = self.step_staff(&combat_inputs, &result.movement, tick);
            acting_staff = staff.acting_player_ids.clone();
            staff_camera_displacements = staff.camera_displacements.clone();
            self.apply_staff_outputs(&staff, tick);
            for id in &acting_staff {
                combat_inputs.get_mut(id).unwrap().primary = false;
            }
        }
        for id in self.participant_order.clone() {
            let primary_unset = self.hub_primary_unset(&id);
            self.dispatch_belt_selection(
                &id,
                combat_inputs[&id].quickbar,
                primary_unset,
                extensions,
            );
        }
        self.step_death_weapons(tick);
        let secondary =
            self.step_secondary(&combat_inputs, tick, &unstepped_secondary, extensions)?;
        let secondary_camera_displacements =
            std::mem::take(&mut self.secondary.camera_displacements);
        staff_camera_displacements.extend(secondary_camera_displacements);
        for proposal in staff_camera_displacements {
            self.secondary.record_camera_displacement(
                &proposal.world_key,
                proposal.displacement,
                proposal.tick,
            );
        }
        for (id, input) in &mut combat_inputs {
            if secondary.primary_override_player_ids.contains(id)
                || self.secondary.players.get(id).is_some_and(|player| {
                    player.cast_action.is_some() || player.cast_spin_ticks_remaining > 0
                })
            {
                input.primary = false;
            }
        }
        let cast =
            self.step_primary_cast(&combat_inputs, tick, context.navigation, extensions)?.tick;
        if let Some(world) = &mut self.boneyard {
            world.enemies.commit_deferred_projectile_painters(&mut self.painter_order);
        }
        self.synchronize_air_water_visuals(&cast, tick);
        if let Some(contacts) = &contacts {
            for reflection in &contacts.reflected_enemy_damage {
                self.damage_enemy(EnemyDamageRequest {
                    enhanced_effects: Some(self.enhanced_effects),
                    actor_id: reflection.actor_id,
                    amount: reflection.amount,
                    ether_drain_capture: false,
                    has_magic_damage: false,
                    hit_strength: 1.0,
                    magic: false,
                    source_player_id: Some(reflection.player_id.clone()),
                    suppress_hit_reaction: false,
                    suppress_hurt_sound: false,
                    tick,
                });
            }
        }
        let secondary_deaths = self.apply_secondary_contacts(&secondary, tick);
        self.apply_secondary_knockbacks(&secondary, &previous_owners);
        let primary_contacts = if self.boneyard.is_some() {
            Some(self.resolve_primary_contacts(&combat_inputs, tick, &cast, secondary_deaths))
        } else {
            None
        };
        if self.boneyard.is_none() {
            self.spells
                .finalize_hub_flame_lash_visuals(
                    &cast.channel_emissions,
                    tick,
                    &mut self.combat_rng,
                    self.enhanced_effects,
                    &mut self.painter_order,
                )
                .expect("valid native Flame factory birth");
        }
        finalize_air_water_player_visual_actors(
            &mut self.spells,
            &cast.channel_emissions,
            tick,
            &mut self.combat_rng,
            &mut self.painter_order,
        );
        for participant in ordered_participants_mut(&mut self.participants, &self.participant_order)
        {
            if tick.is_multiple_of(25)
                && previous_walk_cycles[&participant.id] != participant.wizard.walk_cycle_primary
            {
                participant.wizard.footstep_tick = tick;
            }
            if secondary.staff_cast_pulse_player_ids.contains(&participant.id) {
                participant.wizard.primary_cast.weapon_pulse = 0.45_f32 as f64;
            }
        }
        let combat = self.step_player_resource_clocks(&acting_staff, tick, extensions)?;
        let resource_events = self.materialize_resource_rescues(&combat, tick);
        self.consume_ether_drain_player_corpses(tick);
        let mut chips =
            contacts.as_ref().map_or(&[][..], |contacts| contacts.harden_chips.as_slice());
        for participant in ordered_participants_mut(&mut self.participants, &self.participant_order)
        {
            let world_key = self.boneyard.as_ref().map_or_else(
                || format!("hub:{}", participant.wizard.region.id()),
                |world| format!("boneyard:{}", world.loaded.run_id),
            );
            super::harden::synchronize_player_harden_effects(
                super::harden::PlayerHardenContext {
                    enhanced_effects: self.enhanced_effects,
                    player_id: &participant.id,
                    before: before_runtime.get(&participant.id),
                    after: &participant.player.skill_runtime,
                    position: participant.wizard.position,
                    world_key: &world_key,
                    spells: &mut self.spells,
                    secondary: &mut self.secondary,
                    tick,
                    rng: &mut self.combat_rng,
                    painter_order: &mut self.painter_order,
                },
                chips,
            );
            // Contact chips are a single global stream, ahead of participant transitions.
            chips = &[];
        }
        let mut contact_events = contacts
            .as_ref()
            .map_or_else(Vec::new, |contacts| contacts.player_damage_sound_events.clone());
        contact_events.extend(resource_events);
        for (id, result) in &combat {
            contact_events.extend(self.apply_poison_responses(id, &result.poison_responses, tick));
        }
        self.synchronize_death_weapons(tick);
        if self.boneyard.is_some() {
            for (id, result) in &combat {
                if result.last_word_burst {
                    self.trigger_last_word(id, tick, true);
                }
            }
            for (id, result) in &combat {
                if result.last_word_archive {
                    self.archive_last_word_ground_loot(id);
                }
            }
        }
        self.finish_player_deaths(&combat);
        if let Some(contacts) = &contacts {
            for damage in &contacts.applied_player_damage {
                if let Some(participant) = self.participants.get_mut(&damage.player_id) {
                    participant.player.combat.cold_slow(damage.cold_slow_ticks);
                    participant.player.combat.dazzle(damage.dazzle_ticks);
                    participant.player.combat.poison(damage.poison_damage, damage.poison_duration);
                }
            }
        }
        self.complete_wave_boundary(previous_wave, extensions);
        let mut emitted =
            self.boneyard.as_ref().map_or_else(Vec::new, |world| world.enemies.events.clone());
        if let Some(contacts) = &contacts {
            for event in &mut emitted {
                if let Some(pitch) = contacts.deflect_pitches_by_event_id.get(&event.event_id) {
                    event.deflect_pitch = Some(*pitch);
                }
            }
        }
        emitted.extend(contact_events);
        let loot_events =
            world_result.as_ref().map_or_else(Vec::new, |result| result.loot.events.clone());
        self.retain_world_events(&emitted, loot_events, &text_overrides, tick);
        self.finish_world_dialogue(previous_enemy_event, tick);
        self.finalize_puppet_queries();
        let audio_events = self.finish_run_clock();
        self.ticks = tick;
        self.close_inactive_dye_sessions();
        Ok(GameSessionTickResult {
            advanced: true,
            tick,
            world: world_result,
            primary: Some(cast),
            secondary: Some(secondary),
            primary_contacts,
            audio_events,
        })
    }
}
