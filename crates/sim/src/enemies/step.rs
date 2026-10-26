//! Ordered live enemy-store composition from `boneyard-enemy-store.ts`.

use super::actor_update::{step_damage_presentation_timers, step_living_actor};
use super::boss_spell::BossSpellKind;
use super::config::{EnemyClassification, EnemyFamilyConfig};
use super::dampen::spawn_dampened_mage_smoke;
use super::model::{EnemyActorRead, EnemyBrain, EnemyLifeState, EnemyTarget};
use super::targeting::primary_cell_changed;
use super::world::{EnemyStepContext, EnemyWorld};
use super::{EnemySpawnIntent, EnemyToken};
use crate::painter::WorldPainterOrder;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EnemyLiveCounts {
    pub enemies: usize,
    pub zombies: usize,
    pub bosses: usize,
}

impl EnemyWorld {
    /// Counts retained rows, including terminal presentations. Cocoons do not count
    /// toward the enemy limit; every non-normal classification counts as a boss.
    #[must_use]
    pub fn live_counts(&self) -> EnemyLiveCounts {
        let mut counts = EnemyLiveCounts::default();
        for actor in self.actors.iter() {
            counts.enemies += usize::from(actor.config.enemy_token != EnemyToken::Cocoon);
            counts.zombies += usize::from(actor.config.enemy_token == EnemyToken::Zombie);
            counts.bosses +=
                usize::from(actor.config.classification != EnemyClassification::Normal);
        }
        counts
    }

    /// Advances one source world epoch. The callback materializes wave births after
    /// projectile and boss-spell work, preserving the shared RNG and painter order.
    pub fn step(
        &mut self,
        mut context: EnemyStepContext<'_>,
        painter_order: Option<&mut WorldPainterOrder>,
        resolve_spawn_intents: Option<&mut dyn FnMut(EnemyLiveCounts) -> Vec<EnemySpawnIntent>>,
    ) {
        assert!(
            context.tick <= 9_007_199_254_740_991,
            "enemy store tick must be a non-negative safe integer"
        );
        assert!(
            self.last_step_tick.is_none_or(|prior| context.tick > prior),
            "enemy store ticks must advance monotonically"
        );
        let mut standalone_order;
        let order = if let Some(order) = painter_order {
            order
        } else {
            standalone_order = self.standalone_painter_order();
            &mut standalone_order
        };
        self.clear_step_outputs();
        self.bind_enemy_targets(context.players);
        self.bind_world_puppet_targets(context.puppet_targets);
        let skull_queries: Vec<_> = self
            .boss_spells
            .iter()
            .filter_map(|spell| {
                matches!(spell.kind, BossSpellKind::SkullMissile { .. })
                    .then_some((spell.owner.id, spell.owner.position))
            })
            .collect();
        for (id, position) in skull_queries {
            self.bind_native_query_target(&format!("boss-spell:{id}"), position);
        }
        if context.paused {
            self.step_paused_timestamps(context.tick);
        } else {
            self.step_active_epoch(&mut context, order);
        }
        // The working player roots survive through late constructors and PuppetHit
        // retention, without changing the caller-owned player records mid-step.
        let mut players = context.players.to_vec();
        self.apply_working_player_positions(&mut players);
        let mut context = context.with_players(&players);
        if let Some(resolve) = resolve_spawn_intents {
            let intents = resolve(self.live_counts());
            self.materialize_step_intents(&intents, &mut context, None, order);
        }
        if context.paused {
            // Paused wave constructors still allocate IDs and register births. The
            // source suppresses their event delivery only after those side effects.
            self.events.clear();
        } else {
            self.step_puppet_hits(&context);
        }
        self.step_pre_world_effect_births(context.tick);
        if self.featured_boss_id.is_some_and(|id| !self.actors.iter().any(|actor| actor.id == id)) {
            self.featured_boss_id = None;
        }
        self.last_step_tick = Some(context.tick);
    }

    fn clear_step_outputs(&mut self) {
        self.events.clear();
        self.player_damage.clear();
        self.player_knockbacks.clear();
        self.player_positions.clear();
        self.pending_spawn_intents.clear();
        self.retired.clear();
        self.rewards.clear();
        self.spawned_actor_ids.clear();
    }

    fn step_active_epoch(
        &mut self,
        context: &mut EnemyStepContext<'_>,
        order: &mut WorldPainterOrder,
    ) {
        let tick = context.tick;
        let elapsed = self.last_step_tick.map_or(tick + 1, |prior| tick - prior);
        let mut players = context.players.to_vec();
        self.step_spider_webs(context);
        self.step_silk_fragments();
        self.step_spider_remains();
        self.step_boss_voices(context);
        self.step_detached_crows(context);
        self.mage_lightning_pulses.retain(|pulse| tick - pulse.tick < 5);
        self.step_death_effects(tick, order);
        self.step_projectile_effects(tick);
        let capabilities = self.demon_skull_encounter.pending_capabilities;
        self.demon_skull_encounter.pending_capabilities = 0;
        self.demon_skull_encounter.death_stream_ticks_remaining =
            self.demon_skull_encounter.death_stream_ticks_remaining.saturating_sub(1);
        self.demon_skull_encounter.scream_stream_ticks_remaining =
            self.demon_skull_encounter.scream_stream_ticks_remaining.saturating_sub(1);

        let mut index = 0;
        while index < self.actors.len() {
            let mut actor = self.actors.get(index).expect("enemy index is live").snapshot();
            let before = actor.position;
            let authored_config = actor.config.clone();
            if let EnemyBrain::DemonSkull(brain) = &mut actor.brain {
                brain.capabilities |= capabilities;
            }
            step_damage_presentation_timers(&mut actor, elapsed, tick);
            let mut actor_context = context.with_players(&players);
            let retained = if actor.life_state == EnemyLifeState::Dying {
                self.step_dying_actor(&mut actor, &mut actor_context, order)
            } else {
                step_living_actor(self, &mut actor, &mut actor_context, order);
                true
            };
            if retained {
                spawn_dampened_mage_smoke(self, &actor, tick, order);
                super::zombie::spawn_rotten_zombie_particle(self, &actor, tick, order);
                super::projectile_hazards::spawn_burning_skeleton_fire(self, &actor, tick, order);
                super::wraith::spawn_wraith_wisp(self, &actor, &actor_context, order);
                if primary_cell_changed(before, actor.position) {
                    actor.native_cell_binding_order = self.next_native_cell_binding_order;
                    self.next_native_cell_binding_order += 1;
                }
                actor.config = authored_config;
                self.actors.replace(index, actor);
                index += 1;
            } else {
                self.actors.remove(index);
            }
            // Terminal children join the same admission order and receive their first
            // epoch in this pass. Do not replace this with an iterator over old rows.
            self.apply_working_player_positions(&mut players);
        }
        let mut context = context.with_players(&players);
        let intents = std::mem::take(&mut self.pending_spawn_intents);
        self.materialize_step_intents(&intents, &mut context, None, order);
        self.step_mage_shields(&context);
        let maggot_positions: Vec<_> =
            self.maggots.iter().map(|actor| (actor.id, actor.position)).collect();
        self.step_maggots(&context, elapsed, order);
        for actor in &mut self.maggots {
            if maggot_positions.iter().any(|(id, position)| {
                *id == actor.id && primary_cell_changed(*position, actor.position)
            }) {
                actor.native_cell_binding_order = self.next_native_cell_binding_order;
                self.next_native_cell_binding_order += 1;
            }
        }
        let projectile_positions: Vec<_> = self
            .projectiles
            .iter()
            .map(|projectile| (projectile.state().id, projectile.state().position))
            .collect();
        self.step_projectiles(&context, order);
        self.step_silks(&context, order);
        self.step_boss_spells(&mut context, order);
        for projectile in &mut self.projectiles {
            let state = projectile.state_mut();
            if projectile_positions.iter().any(|(id, position)| {
                *id == state.id && primary_cell_changed(*position, state.position)
            }) {
                state.native_cell_binding_order = self.next_native_cell_binding_order;
                self.next_native_cell_binding_order += 1;
            }
        }
    }

    fn apply_working_player_positions(&self, players: &mut [EnemyTarget]) {
        for player in players {
            if let Some(position) = self.player_positions.get(&player.id) {
                player.position = *position;
            }
        }
    }

    fn step_boss_voices(&mut self, context: &EnemyStepContext<'_>) {
        self.boss_narration.step(context.tick, context.dialogue_busy);
        let Some(voice) = self.faculty_voice_controller else { return };
        let mut members = Vec::new();
        for actor in self.actors.iter() {
            if actor.life_state != EnemyLifeState::Alive {
                continue;
            }
            if let EnemyFamilyConfig::Faculty(family) = &actor.config.family {
                members.push(family.female);
            }
        }
        let busy = context.dialogue_busy
            || self.boss_narration.current.is_some()
            || !self.boss_narration.pending.is_empty();
        let voices = voice.step(&mut self.steering_rng, &members, busy);
        self.faculty_voice_controller = voices.state;
        self.boss_narration.enqueue(&voices.cues, context.tick, context.dialogue_busy);
    }

    fn step_paused_timestamps(&mut self, tick: u64) {
        for mut actor in self.actors.iter_mut() {
            if actor.hit_feedback.timer != 0.0 {
                actor.hit_feedback.tick = tick;
            }
        }
        for actor in &mut self.maggots {
            if actor.hit_feedback.timer != 0.0 {
                actor.hit_feedback.tick = tick;
            }
        }
        for hit in self.puppet_hits.iter_mut() {
            hit.feedback.tick = tick;
        }
        for knockback in self.projectile_knockbacks.iter_mut() {
            knockback.last_step_tick = tick;
        }
        for projectile in &mut self.projectiles {
            projectile.state_mut().last_step_tick = tick;
        }
        for effect in self.projectile_effects.iter_mut() {
            effect.state.last_step_tick = tick;
        }
    }
}
