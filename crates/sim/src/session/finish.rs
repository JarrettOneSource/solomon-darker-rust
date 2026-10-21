//! Late player/resource/death phases and the frozen Game Over branch of
//! `core-server/game-simulation.ts`.

use super::*;
use crate::Vec2;
use crate::boneyard::waves::director::WavePhase;
use crate::combat::PlayerLifeState;
use crate::enemies::model::{EnemyEvent, EnemyEventKind};
use crate::game_run::GameRunPhase;
use crate::painter::WorldManagerLane;
use crate::secondary::{NativePlayerFlashResponseInput, NativeSecondaryScreenFlashState};
use crate::spells::boneyard::boneyard_native_secondary_targets;
use std::collections::BTreeSet;

impl GameSession {
    pub(super) fn step_player_resource_clocks(
        &mut self,
        acting_staff: &BTreeSet<String>,
        tick: u64,
        extensions: Option<&dyn GameSimulationExtensions>,
    ) -> Result<Vec<(String, super::player_combat::PlayerCombatResult)>, String> {
        let mut results = Vec::with_capacity(self.participants.len());
        for participant in ordered_participants_mut(&mut self.participants, &self.participant_order)
        {
            let ceiling = self
                .secondary
                .players
                .get(&participant.id)
                .map(|player| player.mana_ceiling(participant.player.combat.maximum_mana));
            let result = super::player_combat::step_player_combat_with_extensions(
                super::player_combat::PlayerCombatContext {
                    player_id: &participant.id,
                    player: &mut participant.player,
                    wizard: &participant.wizard,
                    primary_cast: &participant.wizard.primary_cast,
                    acting_staff: acting_staff.contains(&participant.id),
                    blindness_ticks_remaining: participant.lighting.blindness_ticks_remaining,
                    tick,
                    mana_ceiling: ceiling,
                    rng: &mut self.secondary.rng,
                },
                extensions,
            )?;
            results.push((participant.id.clone(), result));
        }
        Ok(results)
    }

    pub(super) fn apply_staff_outputs(&mut self, staff: &super::staff::StaffTickResult, tick: u64) {
        let bodies = super::secondary::boneyard_player_bodies(
            &self.participants,
            &self.participant_order,
            &self.run,
        );
        let world = self.boneyard.as_mut().expect("Staff contacts belong to the Boneyard");
        world.apply_secondary_enemy_knockbacks(&staff.displacements, &bodies);
        for (actor, heading) in &staff.heading_perturbations {
            world.enemies.apply_staff_heading_perturbation(*actor, *heading);
        }
        let world_key = format!("boneyard:{}", world.loaded.run_id);
        for (target, duration) in &staff.dazzle_requests {
            crate::secondary::apply_native_secondary_dazzle(
                &mut self.secondary.target_effects,
                &world_key,
                *target,
                *duration,
            );
        }
        for (owner, position) in &staff.pike_break_feedback {
            self.secondary.emit_player_screen_flash(
                owner,
                &world_key,
                *position,
                NativeSecondaryScreenFlashState {
                    alpha: 1.0,
                    blue: 1.0,
                    decay_per_tick: 0.1_f32 as f64,
                    green: 1.0,
                    point_attenuated: false,
                    red: 1.0,
                },
                tick,
            );
        }
    }

    pub(super) fn apply_poison_responses(
        &mut self,
        id: &str,
        responses: &[super::player_combat::PlayerPoisonResponse],
        tick: u64,
    ) -> Vec<EnemyEvent> {
        let mut events = Vec::new();
        let mut flashes = self.screen_flashes.writer(tick);
        for response in responses {
            match response {
                super::player_combat::PlayerPoisonResponse::Deflect { pitch } => {
                    if let Some(world) = &mut self.boneyard {
                        let mut event = EnemyEvent::new(EnemyEventKind::PlayerDeflected, 0, tick);
                        event.event_id = world.enemies.next_event_id;
                        event.target_player_id = Some(Some(id.into()));
                        event.deflect_pitch = Some(*pitch);
                        world.enemies.next_event_id += 1;
                        events.push(event);
                    }
                }
                super::player_combat::PlayerPoisonResponse::Flash { response } => {
                    let position = self.participants[id].wizard.position;
                    let world_key = self.world_key(id).unwrap();
                    let targets: Vec<_> = self.boneyard.as_ref().map_or_else(Vec::new, |world| {
                        boneyard_native_secondary_targets(
                            &world.enemies,
                            position,
                            crate::harmful::NATIVE_FLASH_RESPONSE_RADIUS,
                        )
                        .into_iter()
                        .map(|target| target.id)
                        .collect()
                    });
                    let first_event = self.secondary.next_event_id;
                    self.secondary.materialize_player_flash_response(
                        NativePlayerFlashResponseInput {
                            owner_id: id,
                            position,
                            response,
                            target_ids: &targets,
                            tick,
                            world_key: &world_key,
                        },
                    );
                    self.secondary.write_screen_flashes_since(first_event, &mut flashes);
                }
            }
        }
        self.screen_flashes = flashes.finish();
        events
    }

    pub(super) fn synchronize_death_weapons(&mut self, tick: u64) {
        for participant in ordered_participants_mut(&mut self.participants, &self.participant_order)
        {
            if participant.player.combat.life_state == PlayerLifeState::Dying
                && !participant.player.combat.corpse_consumed
                && participant.lighting.death_weapon_painter_registration.is_none()
            {
                let registration = self.painter_order.register(WorldManagerLane::Actor);
                participant.lighting.death_weapon_painter_registration = Some(registration);
                if let Some(world) = &mut self.boneyard {
                    let appearance =
                        crate::appearance::equipment::player_death_equipment_appearance(
                            participant.player.selection.element,
                            &participant.player.economy.equipment,
                        );
                    let weapon = crate::death_animations::create_native_death_weapon_actor(
                        crate::death_animations::NativeDeathWeaponBirth {
                            death_epoch: participant.player.combat.death_epoch,
                            heading_index: participant.wizard.heading_index,
                            id: world.next_death_weapon_id,
                            owner_id: &participant.id,
                            painter_registration: registration,
                            position: participant.wizard.position,
                            tick,
                            weapon: appearance.weapon,
                        },
                        &mut self.secondary.rng,
                    );
                    world.death_weapons.push(weapon);
                    world.next_death_weapon_id += 1;
                }
            } else if participant.player.combat.life_state == PlayerLifeState::Alive {
                participant.lighting.death_weapon_painter_registration = None;
            }
        }
    }

    pub(super) fn step_death_weapons(&mut self, tick: u64) {
        if let Some(world) = &mut self.boneyard {
            crate::death_animations::step_native_death_weapon_actors(
                &mut world.death_weapons,
                tick,
                &mut self.secondary.rng,
            );
        }
    }

    pub(super) fn finish_player_deaths(
        &mut self,
        results: &[(String, super::player_combat::PlayerCombatResult)],
    ) {
        for (id, combat) in results {
            let participant = self.participants.get_mut(id).unwrap();
            if combat.emitted_death_burst {
                if let Some(world) = &mut self.boneyard {
                    world.enemies.remove_cocoon_owner(id);
                }
                self.secondary.remove_owner(id);
                participant.player.set_mindstar(false);
            }
            if combat.completed_death_presentation {
                participant.player.combat.set_spectating();
            }
        }
    }

    pub(super) fn complete_wave_boundary(
        &mut self,
        previous: Option<WavePhase>,
        extensions: Option<&dyn GameSimulationExtensions>,
    ) {
        let Some(world) = &mut self.boneyard else {
            return;
        };
        let Some(waves) = &world.waves else {
            return;
        };
        if previous != Some(WavePhase::WaveThreshold)
            || waves.state.phase != WavePhase::WaveLullDelay
            || waves.state.wave_ordinal == 0
        {
            return;
        }
        for id in &self.run.eligible_player_ids {
            let Some(participant) = self.participants.get_mut(id) else {
                continue;
            };
            if participant.player.combat.life_state == PlayerLifeState::Alive {
                let previous = participant.player.economy.npc.boast.clone();
                let wave = waves.state.wave_ordinal.min(u64::from(u32::MAX)) as u32;
                let rules = previous
                    .selected
                    .as_ref()
                    .and_then(|selection| super::extensions::boast_rules(selection, extensions));
                participant.player.economy.npc.boast.complete_wave_with_rules(wave, rules);
                if previous != participant.player.economy.npc.boast {
                    participant.player.economy.revision += 1;
                    participant.player.refresh_economy();
                }
            }
        }
        for id in &self.run.eligible_player_ids {
            let Some(participant) = self.participants.get_mut(id) else {
                continue;
            };
            if participant.player.combat.current_health <= 0.0 {
                participant.player.combat.respawn();
                participant.wizard.position = world.loaded.scene.spawn.position;
                participant.wizard.velocity = Vec2::ZERO;
                participant.wizard.primary_cast.reset();
            }
        }
    }

    pub(super) fn finish_world_dialogue(&mut self, previous_enemy_event_id: usize, tick: u64) {
        let Some(world) = &mut self.boneyard else { return };
        if world
            .enemy_events
            .iter()
            .any(|event| event.kind == EnemyEventKind::CocoonReleased && event.tick == tick)
        {
            world.enemy_world_feedback.magnitude = 0.2_f32 as f64;
        }
        if world.enemy_events.iter().any(|event| {
            event.kind == EnemyEventKind::EnemyDialogueStop
                && event.event_id > previous_enemy_event_id
        }) {
            if let Some(encounter) = &mut world.encounter {
                encounter.voice_ticks_remaining = 0;
                encounter.queued_get_him_boys = false;
            }
            if let Some(tutorial) = &mut world.tutorial {
                tutorial.narration.current = None;
                tutorial.narration.pending.clear();
                tutorial.narration.ticks_remaining = 0;
            }
        }
    }

    pub(super) fn step_game_over(
        &mut self,
        extensions: Option<&dyn GameSimulationExtensions>,
    ) -> Result<GameSessionTickResult, String> {
        let world = self.boneyard.as_ref().expect("Game Over has the terminal Boneyard");
        let previous_events = world.enemies.events.len();
        for participant in self.participants.values_mut() {
            participant.lighting.step_overlay();
        }
        self.step_rescue_protection();
        // Game Over freezes the world and supplies the preceding tick to resources.
        let combat = self.step_player_resource_clocks(&BTreeSet::new(), self.ticks, extensions)?;
        self.secondary.step_mindblast_presentation(self.ticks + 1);
        for (id, result) in &combat {
            if result.emitted_death_burst {
                self.participants.get_mut(id).unwrap().player.set_mindstar(false);
                self.secondary.remove_owner(id);
            }
        }
        let tick = self.ticks + 1;
        let resource_events = self.materialize_resource_rescues(&combat, tick);
        self.consume_ether_drain_player_corpses(tick);
        if !resource_events.is_empty() {
            let world = self.boneyard.as_mut().unwrap();
            super::rewards::retain_boneyard_enemy_events(world, &resource_events, tick);
        }
        let audio_events = self.finish_run_clock();
        // These independent animations keep their shared Bouncer clock while
        // GameOver freezes enemy/world authority, even after the owner leaves.
        self.step_death_weapons(tick);
        for (id, result) in &combat {
            if result.last_word_burst {
                self.trigger_last_word(id, tick, false);
            }
        }
        for (id, result) in &combat {
            if result.last_word_archive {
                self.archive_last_word_ground_loot(id);
            }
        }
        if self.boneyard.as_ref().unwrap().enemies.events.len() > previous_events {
            let emitted =
                self.boneyard.as_ref().unwrap().enemies.events[previous_events..].to_vec();
            self.retain_world_events(&emitted, Vec::new(), &[], tick);
        }
        self.archive_run_portraits_if_due(tick, extensions);
        self.ticks = tick;
        if self.run.phase == GameRunPhase::Loadout
            && let Some(id) = self.enter_post_run_loadout()
        {
            self.arm_college_intro(&id);
        }
        Ok(GameSessionTickResult {
            advanced: true,
            audio_events,
            ..GameSessionTickResult::frozen(tick)
        })
    }
}
