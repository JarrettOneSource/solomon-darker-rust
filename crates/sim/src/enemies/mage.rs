//! Mage cast scheduling, elemental dispatch and ally shielding from `skeleton-family.ts`.
use super::EnemyToken;
use super::combat::attack_marker;
use super::config::{EnemyFamilyConfig, MageElement};
use super::model::*;
use super::movement::{move_toward_target, staff_attack_speed};
use super::path::{RadialDirection, TangentDirection};
use super::projectile_emission::{emit_mage_attack, step_mage_lightning_pulse};
use super::ranged::{RangedAttackRange, restore_native_range_easy_after_volley};
use super::targeting::{
    enemy_target_line_of_sight_is_clear, target_distance, track_enemy_action_heading,
};
use super::world::{EnemyStepContext, EnemyWorld};
use crate::float::{fround, hypot};
use crate::painter::WorldPainterOrder;

#[must_use]
pub fn mage_body_pose(program: MageCastProgram, progress: f64) -> f64 {
    assert!(
        progress.is_finite() && progress >= 0.0,
        "Mage action progress must be finite and nonnegative"
    );
    let index = progress.floor() as usize;
    let marker = if program == MageCastProgram::Short { 24 } else { 30 };
    if index < marker {
        2.0
    } else if index == marker || index == marker + 14 {
        3.0
    } else if index < marker + 14 {
        4.0
    } else {
        0.0
    }
}

pub fn step_mage(
    world: &mut EnemyWorld,
    actor: &mut EnemyActor,
    context: &mut EnemyStepContext<'_>,
    order: &mut WorldPainterOrder,
) {
    let EnemyBrain::Mage(mut brain) = actor.brain.clone() else {
        panic!("Mage brain required");
    };
    let mut lightning = None;
    if actor.target_player_id.is_none() {
        reset_mage(&mut brain);
        actor.brain = EnemyBrain::Mage(brain);
        move_toward_target(
            world,
            actor,
            context,
            RadialDirection::Approach,
            TangentDirection::Hold,
        );
    } else if brain.phase == MagePhase::Cast {
        track_enemy_action_heading(actor, context.players);
        let progress = brain.action_progress
            + 0.253125012 * (1.0 + brain.cast_roll) * staff_attack_speed(actor);
        let (marker, end) =
            if brain.cast_program == MageCastProgram::Short { (25.0, 41.0) } else { (31.0, 47.0) };
        let dispatched = !brain.marker_emitted && progress >= marker;
        if dispatched {
            attack_marker(world, actor, context.tick, actor.target_player_id.clone(), order);
            let mut range = RangedAttackRange {
                range: brain.attack_range,
                range_easy_pending: brain.range_easy_pending,
            };
            restore_native_range_easy_after_volley(&mut range);
            brain.attack_range = range.range;
            brain.range_easy_pending = range.range_easy_pending;
            lightning = emit_mage_attack(world, actor, context, order);
            brain.marker_emitted = true;
        }
        if progress > end {
            reset_mage(&mut brain);
        } else {
            actor.body_pose = mage_body_pose(brain.cast_program, progress);
            brain.action_progress = progress;
        }
        if dispatched {
            actor.lighting.charge = if matches!(
                actor.config.family,
                EnemyFamilyConfig::Mage { element: MageElement::Lightning, .. }
            ) {
                1.0
            } else {
                0.0
            };
        }
        actor.brain = EnemyBrain::Mage(brain);
    } else if brain.disabled_primary_ticks == 0
        && target_distance(actor, context.players) < brain.attack_range
        && enemy_target_line_of_sight_is_clear(actor, context)
    {
        let program = world.draw_unit();
        let roll = world.draw_unit();
        // The action scheduler and cast scheduler both overwrite the retained loot seed.
        world.draw_loot_seed(1_000_000, &mut context.roll_loot_seed);
        actor.loot_seed = world.draw_loot_seed(1_000_000, &mut context.roll_loot_seed);
        brain.action_progress = 0.0;
        brain.cast_program =
            if program < 0.5 { MageCastProgram::Short } else { MageCastProgram::Long };
        brain.cast_roll = roll;
        brain.vertical_velocity = fround(-(2.5 + world.steering_rng.draw_scaled_float(1.0, false)));
        brain.marker_emitted = false;
        brain.phase = MagePhase::Cast;
        actor.body_pose = 2.0;
        actor.brain = EnemyBrain::Mage(brain);
    } else {
        move_toward_target(
            world,
            actor,
            context,
            RadialDirection::Approach,
            TangentDirection::Hold,
        );
    }
    if let Some(dispatch) = lightning {
        let duration = (50.0 / staff_attack_speed(actor)).trunc();
        assert!(
            duration.is_finite() && duration >= 0.0,
            "Mage lightning duration must be nonnegative and finite"
        );
        let EnemyBrain::Mage(brain) = &mut actor.brain else { unreachable!() };
        brain.lightning_target_player_id = Some(dispatch.target_player_id);
        brain.lightning_target_position = Some(dispatch.target_position);
        brain.lightning_ticks_remaining = duration as u64;
    }
    step_mage_lightning_pulse(world, actor, context, order);
}

pub fn reset_mage(brain: &mut MageBrain) {
    if brain.phase == MagePhase::RangeControl {
        return;
    }
    brain.action_progress = 0.0;
    brain.marker_emitted = false;
    brain.phase = MagePhase::RangeControl;
}
pub fn apply_mage_provider_gate_after_action(actor: &mut EnemyActor) {
    if !actor.config.burning && actor.lighting.charge <= 0.0 {
        actor.lighting.provider_copies = 0;
    }
}

impl EnemyWorld {
    pub fn step_mage_shields(&mut self, context: &EnemyStepContext<'_>) {
        for index in 0..self.actors.len() {
            let source = self.actors[index].clone();
            let EnemyBrain::Mage(brain) = source.brain else {
                continue;
            };
            let EnemyFamilyConfig::Mage {
                shield_interval,
                self_shield,
                self_shield_health,
                other_shield,
                other_shield_health,
                ..
            } = source.config.family
            else {
                continue;
            };
            if source.life_state != EnemyLifeState::Alive
                || context.ability_effect(source.id).disrupted_ticks > 0
                || shield_interval <= 0.0
            {
                continue;
            }
            let remaining = brain.shield_ticks_remaining.saturating_sub(1);
            let mage = &mut self.actors[index];
            let EnemyBrain::Mage(mage_brain) = &mut mage.brain else { unreachable!() };
            if remaining > 0 || brain.disabled_primary_ticks > 0 {
                mage_brain.shield_ticks_remaining = remaining;
                continue;
            }
            mage_brain.shield_ticks_remaining = (shield_interval * 100.0).round().max(1.0) as u64;
            if self_shield && self_shield_health > 0.0 {
                refresh_shield(mage, self_shield_health);
            }
            if !other_shield || other_shield_health <= 0.0 {
                continue;
            }
            let mut selected = None;
            let mut selected_distance = f64::INFINITY;
            for (ally_index, ally) in self.actors.iter().enumerate() {
                if ally_index == index
                    || ally.life_state != EnemyLifeState::Alive
                    || !matches!(
                        ally.config.enemy_token,
                        EnemyToken::Skeleton | EnemyToken::SkeletonArcher | EnemyToken::Zombie
                    )
                {
                    continue;
                }
                let distance =
                    hypot(ally.position.x - source.position.x, ally.position.y - source.position.y);
                if distance > 240.0 {
                    continue;
                }
                let lower_id =
                    selected.is_none_or(|selected: usize| ally.id < self.actors[selected].id);
                if distance < selected_distance || (distance == selected_distance && lower_id) {
                    selected = Some(ally_index);
                    selected_distance = distance;
                }
            }
            if let Some(index) = selected {
                refresh_shield(&mut self.actors[index], other_shield_health);
            }
        }
    }
}
fn refresh_shield(actor: &mut EnemyActor, strength: f64) {
    actor.shield_health = actor.shield_health.max(strength);
    actor.shield_maximum_health = actor.shield_maximum_health.max(actor.shield_health);
    actor.shield_pulse = 3.0;
}
