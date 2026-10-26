//! Shared XP, cohort barriers and attributed score/Mindblast transactions.

use super::*;
use crate::enemies::model::{EnemyDamageRequest, EnemyDamageResult};
use crate::game_run::GameRunPhase;
use crate::painter::WorldManagerLane;
use crate::profile::{PlayerSkillChoice, SharedPlayerLevelMilestone};
use crate::score::NativeHallOfFamePlayerScoreState;
use crate::secondary::NativePlayerMindblastInput;
use crate::spells::boneyard::boneyard_native_secondary_targets;

impl GameSession {
    pub fn grant_experience(&mut self, id: &str, amount: f64) -> Vec<u64> {
        self.grant_experience_at(id, amount, self.ticks)
    }

    pub fn grant_experience_with_extensions(
        &mut self,
        id: &str,
        amount: f64,
        extensions: Option<&dyn GameSimulationExtensions>,
    ) -> Vec<u64> {
        self.grant_experience_at_with_extensions(id, amount, self.ticks, extensions)
    }

    /// Only the credited player gains sub-threshold XP. A crossed level synchronizes
    /// that milestone into every eligible participant's independently generated offers.
    pub(super) fn grant_experience_at(&mut self, id: &str, amount: f64, tick: u64) -> Vec<u64> {
        self.grant_experience_at_with_extensions(id, amount, tick, None)
    }

    pub(super) fn grant_experience_at_with_extensions(
        &mut self,
        id: &str,
        amount: f64,
        tick: u64,
        extensions: Option<&dyn GameSimulationExtensions>,
    ) -> Vec<u64> {
        let cohort = self.level_up_barrier.as_ref().map_or_else(
            || self.level_up_participant_ids(),
            |barrier| barrier.participant_ids.clone(),
        );
        assert!(
            cohort.iter().any(|participant| participant == id),
            "shared XP source belongs to its cohort"
        );
        let source = self.participants.get_mut(id).expect("XP source is an existing participant");
        let previous_level = source.player.progression.level;
        source.player.grant_experience(amount, &mut self.gameplay_rng);
        let level = source.player.progression.level;
        if level == previous_level {
            return Vec::new();
        }
        let milestone = SharedPlayerLevelMilestone {
            crossed_levels: (previous_level + 1..=level).collect(),
            experience: source.player.progression.experience,
            level,
        };
        for participant_id in &cohort {
            if participant_id == id {
                continue;
            }
            self.participants
                .get_mut(participant_id)
                .unwrap()
                .player
                .synchronize_level_milestone(&milestone, &mut self.gameplay_rng)
                .expect("source-generated shared level milestone is valid");
        }
        self.assign_automatic_skill_choices_with_extensions(&cohort, extensions);
        self.refresh_level_up_barrier(id, &cohort);
        self.trigger_level_up_mindblast(id, tick)
    }

    pub fn choose_skill(&mut self, id: &str, choice: PlayerSkillChoice) -> bool {
        let Some(mut participant) = self.participants.get_mut(id) else {
            return false;
        };
        let previous = participant.selected_primary_id();
        if !participant.player.choose_skill(choice, &mut self.gameplay_rng) {
            return false;
        }
        participant.synchronize_primary_selection(previous);
        self.update_pending_barrier();
        true
    }

    pub fn replace_skill_with_mod(&mut self, id: &str, offer_sequence: u64) -> bool {
        let Some(participant) = self.participants.get_mut(id) else { return false };
        if !participant.player.replace_skill_with_mod(offer_sequence, &mut self.gameplay_rng) {
            return false;
        }
        self.update_pending_barrier();
        true
    }

    pub fn reroll_skill_offer(&mut self, id: &str, sequence: u64) -> bool {
        if self
            .level_up_barrier
            .as_ref()
            .is_none_or(|barrier| !barrier.pending_player_ids.iter().any(|pending| pending == id))
        {
            return false;
        }
        self.participants.get_mut(id).is_some_and(|participant| {
            participant.player.reroll_skill_offer(sequence, &mut self.gameplay_rng)
        })
    }

    pub fn defer_skill_choice(&mut self, id: &str, sequence: u64) -> bool {
        if self
            .level_up_barrier
            .as_ref()
            .is_none_or(|barrier| !barrier.pending_player_ids.iter().any(|pending| pending == id))
        {
            return false;
        }
        let accepted = self.participants.get_mut(id).is_some_and(|participant| {
            participant.player.defer_skill_choice(sequence, &mut self.gameplay_rng)
        });
        if accepted {
            self.update_pending_barrier();
        }
        accepted
    }

    pub(super) fn level_up_participant_ids(&self) -> Vec<String> {
        if self.run.phase == GameRunPhase::Active {
            self.run
                .eligible_player_ids
                .iter()
                .filter(|id| self.participants.contains_key(id))
                .cloned()
                .collect()
        } else {
            self.participants.keys().cloned().collect()
        }
    }

    pub(super) fn assign_automatic_skill_choices(&mut self, cohort: &[String]) {
        self.assign_automatic_skill_choices_with_extensions(cohort, None);
    }

    pub(super) fn assign_automatic_skill_choices_with_extensions(
        &mut self,
        cohort: &[String],
        extensions: Option<&dyn GameSimulationExtensions>,
    ) {
        let mut ids = cohort.to_vec();
        ids.sort();
        ids.dedup();
        for id in ids {
            let Some(participant) = self.participants.get_mut(&id) else {
                continue;
            };
            let boast = &participant.player.economy.npc.boast;
            let rules = boast
                .selected
                .as_ref()
                .and_then(|selection| super::extensions::boast_rules(selection, extensions));
            if !boast.uses_random_skill_choices_with_rules(rules) {
                continue;
            }
            if let Some(offer) = &participant.player.progression.pending_offer
                && offer.automatic_choice_index.is_none()
            {
                let index = self.gameplay_rng.draw_integer(offer.options.len() as i32) as usize;
                assert!(participant.player.progression.set_automatic_choice(index));
            }
        }
    }

    pub(super) fn pending_offer_ids(&self, cohort: &[String]) -> Vec<String> {
        cohort
            .iter()
            .filter(|id| {
                self.participants.get(id).is_some_and(|participant| {
                    participant.player.progression.pending_offer.is_some()
                        || !participant.player.progression.pending_levels.is_empty()
                })
            })
            .cloned()
            .collect()
    }

    pub(super) fn refresh_level_up_barrier(&mut self, id: &str, cohort: &[String]) {
        let pending = self.pending_offer_ids(cohort);
        assert!(!pending.is_empty(), "shared milestone creates at least one pending choice");
        let progression =
            &self.participants.get(id).expect("participant exists").player.progression;
        if let Some(barrier) = &mut self.level_up_barrier {
            barrier.milestone_experience = progression.experience;
            barrier.milestone_level = progression.level;
            barrier.pending_player_ids = pending;
            barrier.source_player_id = id.into();
        } else {
            self.level_up_barrier = Some(PlayerLevelUpBarrier {
                barrier_id: self.next_level_up_barrier_id,
                milestone_experience: progression.experience,
                milestone_level: progression.level,
                participant_ids: cohort.to_vec(),
                pending_player_ids: pending,
                run_id: if self.run.phase == GameRunPhase::Active {
                    self.run.run_id.clone()
                } else {
                    None
                },
                source_player_id: id.into(),
            });
            self.next_level_up_barrier_id += 1;
        }
    }

    pub(super) fn update_pending_barrier(&mut self) {
        let Some(barrier) = &self.level_up_barrier else {
            return;
        };
        let pending = self.pending_offer_ids(&barrier.participant_ids);
        if pending.is_empty() {
            self.level_up_barrier = None;
        } else {
            self.level_up_barrier.as_mut().unwrap().pending_player_ids = pending;
        }
    }

    fn trigger_level_up_mindblast(&mut self, id: &str, tick: u64) -> Vec<u64> {
        let participant = &self.participants.get(id).expect("participant exists");
        if participant.player.skill_runtime.equipment_modifiers.feature_bits & 0x0400 == 0 {
            return Vec::new();
        }
        let first = self.secondary.next_actor_id;
        let position = participant.wizard.position;
        let world_key = self.world_key(id).unwrap();
        let light = self.painter_order.register(WorldManagerLane::Actor);
        let triggered = self.secondary.trigger_player_mindblast(
            NativePlayerMindblastInput {
                direct_damage: None,
                element: participant.player.selection.element,
                level: participant.player.progression.level,
                light_registration: light,
                owner_id: id,
                position,
                presentation_scale: None,
                world_key: &world_key,
            },
            Some(&mut self.painter_order),
        );
        if triggered.direct_damage > 0.0
            && let Some(world) = &self.boneyard
        {
            let targets = boneyard_native_secondary_targets(
                &world.enemies,
                position,
                triggered.direct_radius,
            );
            for target in targets {
                let x = target.position.x - position.x;
                let y = target.position.y - position.y;
                if x * x + y * y
                    >= triggered.direct_radius * triggered.direct_radius
                        + target.radius * target.radius
                {
                    continue;
                }
                self.damage_enemy(EnemyDamageRequest {
                    enhanced_effects: Some(self.enhanced_effects),
                    actor_id: target.id,
                    amount: triggered.direct_damage,
                    ether_drain_capture: false,
                    has_magic_damage: true,
                    hit_strength: 1.0,
                    magic: false,
                    source_player_id: Some(id.into()),
                    suppress_hit_reaction: false,
                    suppress_hurt_sound: false,
                    tick,
                });
            }
        }
        (first..self.secondary.next_actor_id).collect()
    }

    /// The lethal score observer runs immediately. Terminal XP and drops stay in
    /// the later enemy death epoch, attributed by the request's actual owner ID.
    pub(super) fn damage_enemy(&mut self, request: EnemyDamageRequest) -> EnemyDamageResult {
        let world = self.boneyard.as_mut().expect("enemy damage has an active Boneyard");
        let config = world
            .enemies
            .actors
            .iter()
            .find(|actor| actor.id == request.actor_id)
            .map(|actor| actor.config.clone());
        let credited = request.source_player_id.clone();
        let result = world.enemies.damage(request, Some(&mut self.painter_order));
        if result.killed
            && let Some(config) = config
            && let Some(id) = credited
            && let Some(participant) = self.participants.get_mut(&id)
        {
            let player = NativeHallOfFamePlayerScoreState {
                current_health: participant.player.combat.current_health,
                maximum_health: participant.player.combat.maximum_health,
                level: participant.player.progression.level,
                score_health_multiplier_enabled: participant.player.combat.life_state
                    == crate::combat::PlayerLifeState::Alive,
            };
            participant.score.record_enemy_kill(
                &config,
                player,
                world.enemy_world_feedback.accumulator,
                &mut self.gameplay_rng,
            );
        }
        result
    }
}
