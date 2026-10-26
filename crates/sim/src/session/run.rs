//! Terminal run profile/archive operations. The world clock is stepped by the session;
//! UI and storage only project these committed values.

use super::*;
use crate::audio::{AudioEvent, AudioOptions};
use crate::boneyard::loot::LootActorRead;
use crate::economy::CompletedRunEconomyArchive;
use crate::economy::HubEconomyState;
use crate::enemies::model::EnemyDamageRequest;
use crate::game_run::{GameRunPhase, presentation::game_over_audio_events};
use crate::loot::NativeLootDropKind;
use crate::painter::WorldManagerLane;
use crate::profile::native_hagatha_boss_damage_factor;
use crate::secondary::NativePlayerMindblastInput;
use crate::spells::boneyard::boneyard_native_secondary_targets;

impl GameSession {
    #[must_use]
    pub fn durable_profile_economy(&self, id: &str) -> Option<HubEconomyState> {
        let participant = self.participants.get(id)?;
        let completed = matches!(self.run.phase, GameRunPhase::GameOver | GameRunPhase::Loadout);
        if self.boneyard.as_ref().is_some_and(|world| world.tutorial.is_some()) {
            let mut economy = (**participant
                .tutorial_profile_economy
                .as_ref()
                .expect("Tutorial profile baseline is present"))
            .clone();
            if completed {
                economy.revision = economy.revision.max(participant.player.economy.revision) + 1;
                economy.tutorial_pending = false;
            }
            return Some(economy);
        }
        let mut economy = participant.player.economy.clone();
        if self.run.phase == GameRunPhase::Loadout && self.boneyard.is_none() {
            return Some(economy);
        }
        let completed = self.boneyard.is_some() && completed;
        let last_word = completed && economy.owned_perk_selectors.contains(&12);
        let mut ground_gold = 0;
        let mut ground_items = Vec::new();
        if last_word && let Some(world) = &self.boneyard {
            for actor in world.loot.actors.iter() {
                if actor.kind == NativeLootDropKind::Gold {
                    ground_gold += actor.reward().amount;
                }
                if actor.kind == NativeLootDropKind::Sack
                    && let Some(item) = &actor.reward().item
                {
                    ground_items.push(item.clone());
                }
            }
        }
        economy.archive_completed_run(CompletedRunEconomyArchive {
            display_name: &participant.player.selection.name,
            ground_gold,
            ground_items: &ground_items,
            starter_element: participant.player.selection.element,
            transfer_carried_items: completed,
        });
        Some(economy)
    }

    #[must_use]
    pub fn retired_wizard_economy(&self, id: &str) -> Option<HubEconomyState> {
        let participant = self.participants.get(id)?;
        if self.boneyard.as_ref().is_some_and(|world| world.tutorial.is_some()) {
            return Some(
                (**participant
                    .tutorial_profile_economy
                    .as_ref()
                    .expect("Tutorial profile baseline is present"))
                .clone(),
            );
        }
        let mut economy = participant.player.economy.clone();
        economy.archive_completed_run(CompletedRunEconomyArchive {
            display_name: &participant.player.selection.name,
            ground_gold: 0,
            ground_items: &[],
            starter_element: participant.player.selection.element,
            transfer_carried_items: true,
        });
        Some(economy)
    }

    pub fn continue_game_over(&mut self, run_id: &str, event_id: u64) -> bool {
        self.run.continue_game_over(run_id, event_id)
    }

    /// Complete the shared archive/Hub reset once. Tutorial has exactly one owner.
    pub(super) fn enter_post_run_loadout(&mut self) -> Option<String> {
        assert_eq!(self.run.phase, GameRunPhase::Loadout, "post-run loadout follows its fade");
        let tutorial = self.boneyard.as_ref().is_some_and(|world| world.tutorial.is_some());
        let economies: Vec<_> = self
            .participant_order
            .iter()
            .map(|id| (id.clone(), self.durable_profile_economy(id).unwrap()))
            .collect();
        let seed = self.gameplay_rng.draw_integer(0x4000_0000) as u32;
        self.rebuild_hub(seed);
        for (id, economy) in economies {
            let participant = self.participants.get_mut(&id).unwrap();
            participant.player.economy = economy;
            participant.player.refresh_economy();
        }
        self.hall_of_fame_clock_started_at_tick = self.ticks;
        if tutorial
            && let Some(id) = self.participant_order.first()
            && self
                .participants
                .get(id)
                .expect("participant exists")
                .player
                .economy
                .college_intro_pending
        {
            assert!(self.run.continue_post_run_to_college_intro());
            return Some(id.clone());
        }
        None
    }

    pub(super) fn finish_run_clock(&mut self) -> Vec<AudioEvent> {
        let previous = self.run.clone();
        let alive: Vec<_> = self
            .participants
            .iter()
            .filter(|(_, participant)| {
                matches!(
                    participant.player.combat.life_state,
                    crate::combat::PlayerLifeState::Alive
                        | crate::combat::PlayerLifeState::LethalPending
                )
            })
            .map(|(id, _)| id.clone())
            .collect();
        self.run.step(&alive);
        game_over_audio_events(&previous, &self.run)
            .into_iter()
            .map(|cue| AudioEvent::Play(cue, AudioOptions::default()))
            .collect()
    }

    pub(super) fn archive_run_portraits_if_due(
        &mut self,
        tick: u64,
        extensions: Option<&dyn GameSimulationExtensions>,
    ) {
        if self.run.game_over_ticks != 300 {
            return;
        }
        // The source sorts Hall rows, independently of entity/registration order.
        for participant in self.participants.values_mut() {
            let heading = self.gameplay_rng.draw_scaled_float(65.0, true);
            let scale = self.gameplay_rng.draw_scaled_float(0.15_f32 as f64, false);
            let boast = &participant.player.economy.npc.boast;
            let rules = boast
                .selected
                .as_ref()
                .and_then(|selection| super::extensions::boast_rules(selection, extensions));
            participant.score.awesomeness =
                boast.score_with_rules(participant.score.awesomeness, rules);
            participant.score.archive(
                tick,
                crate::float::fround(180.0 + heading),
                crate::float::fround(0.85_f32 as f64 + scale),
            );
        }
    }

    pub(super) fn trigger_last_word(&mut self, id: &str, tick: u64, record_score: bool) {
        let world = self.boneyard.as_ref().expect("Last Word belongs to the current Boneyard");
        let participant = &self.participants.get(id).expect("participant exists");
        let position = participant.wizard.position;
        let key = format!("boneyard:{}", world.loaded.run_id);
        let registration = self.painter_order.register(WorldManagerLane::Actor);
        let burst = self.secondary.trigger_player_mindblast(
            NativePlayerMindblastInput {
                direct_damage: Some(5000.0),
                element: participant.player.selection.element,
                level: 10_000,
                light_registration: registration,
                owner_id: id,
                position,
                presentation_scale: Some(15.0),
                world_key: &key,
            },
            Some(&mut self.painter_order),
        );
        let targets =
            boneyard_native_secondary_targets(&world.enemies, position, burst.direct_radius);
        for target in targets {
            let x = target.position.x - position.x;
            let y = target.position.y - position.y;
            if x * x + y * y
                >= burst.direct_radius * burst.direct_radius + target.radius * target.radius
            {
                continue;
            }
            let world = self.boneyard.as_ref().unwrap();
            let multiplier =
                world.enemies.actors.iter().find(|actor| actor.id == target.id).map_or(
                    1.0,
                    |actor| {
                        native_hagatha_boss_damage_factor(
                            &self
                                .participants
                                .get(id)
                                .expect("participant exists")
                                .player
                                .economy
                                .owned_perk_selectors,
                            u32::from(actor.config.native_type_id),
                        )
                    },
                );
            let request = EnemyDamageRequest {
                enhanced_effects: Some(self.enhanced_effects),
                actor_id: target.id,
                amount: burst.direct_damage * multiplier,
                ether_drain_capture: false,
                has_magic_damage: true,
                hit_strength: 1.0,
                magic: false,
                source_player_id: Some(id.into()),
                suppress_hit_reaction: false,
                suppress_hurt_sound: false,
                tick,
            };
            if record_score {
                self.damage_enemy(request);
            } else {
                // The frozen Game Over branch deliberately omits the lethal observer.
                self.boneyard
                    .as_mut()
                    .unwrap()
                    .enemies
                    .damage(request, Some(&mut self.painter_order));
            }
        }
    }

    pub(super) fn archive_last_word_ground_loot(&mut self, id: &str) {
        let participant = self.participants.get_mut(id).expect("Last Word owner exists");
        let world = self.boneyard.as_ref().expect("Last Word has a ground loot owner");
        let retained = world.loot.native_hagatha_last_word_loot();
        if retained.actor_ids.is_empty() {
            return;
        }
        let mut remove = Vec::new();
        for actor in world.loot.actors.iter() {
            if actor.reward().native_type_id == 2012 {
                remove.push(actor.id);
            }
        }
        if retained.gold > 0 {
            participant.player.economy.credit_loot_gold(retained.gold);
        }
        if !retained.items.is_empty() {
            let suffix = self.gameplay_rng.draw_integer(5) as usize;
            let name = format!(
                "{}'s {}",
                participant.player.selection.name,
                crate::economy::NATIVE_RETAINED_SACK_SUFFIXES[suffix]
            );
            if participant.player.economy.archive_hagatha_last_word_items(&retained.items, &name) {
                for actor in world.loot.actors.iter() {
                    if actor.reward().native_type_id == 2013 {
                        remove.push(actor.id);
                    }
                }
                participant.player.refresh_economy();
            }
        }
        self.boneyard.as_mut().unwrap().loot.remove_actors(&remove);
    }
}
