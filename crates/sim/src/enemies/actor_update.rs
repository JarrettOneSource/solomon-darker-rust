//! Shared hit, head-facing and light writers from `enemies/actor-update.ts`.
use super::EnemyToken;
use super::config::{ArrowType, EnemyFamilyConfig};
use super::mage::apply_mage_provider_gate_after_action;
use super::model::*;
use super::skeleton::step_skeleton_recoil;
use super::world::{EnemyStepContext, EnemyWorld};
use crate::painter::WorldPainterOrder;

/// Runs the source actor epoch. Status scalars are a current-tick config view;
/// the world restores the authored config after its post-action effect producers.
pub fn step_living_actor(
    world: &mut EnemyWorld,
    actor: &mut EnemyActor,
    context: &mut EnemyStepContext<'_>,
    order: &mut WorldPainterOrder,
) {
    if matches!(actor.brain, EnemyBrain::Cocoon(_)) {
        super::spider::step_cocoon_actor(world, actor, context);
        return;
    }
    let authored_attack_speed = actor.config.attack_speed;
    let effect = context.ability_effect(actor.id);
    if effect.time_scale != 1.0 || effect.weaken_factor != 1.0 {
        actor.config.attack_speed *= effect.time_scale;
        actor.config.base_speed *= effect.time_scale;
        actor.config.extra_damage *= effect.weaken_factor;
        actor.config.primary_damage =
            actor.config.primary_damage.map(|damage| damage * effect.weaken_factor);
        actor.config.secondary_damage *= effect.weaken_factor;
        actor.config.tertiary_damage *= effect.weaken_factor;
    }
    if !matches!(actor.brain, EnemyBrain::Portal(_)) {
        super::targeting::refresh_target(actor, context);
    }
    // Coffin::Tick restores its native root before time holds and disruption gates.
    if let EnemyBrain::Coffin(brain) = &actor.brain {
        actor.position = brain.anchor_position;
    }
    if matches!(&actor.brain, EnemyBrain::Skeleton(brain) if brain.pike.is_some()) {
        roll_skeleton_family_head_facing(world, actor);
        super::skeleton::step_latched_skeleton(world, actor, context);
        if effect.time_scale != 0.0 {
            step_skeleton_recoil(actor);
        }
        step_enemy_lighting(actor);
        return;
    }
    if let EnemyBrain::Mage(brain) = &mut actor.brain {
        brain.disabled_primary_ticks = brain.disabled_primary_ticks.saturating_sub(1);
    }
    super::demon::snap_demon_root_to_extremities(actor);
    // These families own their status handling inside their native controller.
    match actor.brain {
        EnemyBrain::Portal(_) => {
            if effect.time_scale != 0.0 {
                super::portal::step_portal(world, actor, context);
            } else if let EnemyBrain::Portal(brain) = &actor.brain
                && let Some(anchor) = brain.anchor_position
            {
                actor.position = anchor;
            }
        }
        EnemyBrain::Heartmonger(_) => {
            super::heartmonger::step_heartmonger(world, actor, context, order)
        }
        EnemyBrain::DemonSkull(_) => super::demon_skull::step_demon_skull(
            world,
            actor,
            context,
            authored_attack_speed,
            order,
        ),
        EnemyBrain::Faculty(_) => super::faculty::step_faculty(world, actor, context, order),
        // Its clock and contact surround the common time/disruption movement gates.
        EnemyBrain::Wraith(_) => super::wraith::step_wraith(world, actor, context, order),
        _ => {
            step_ordinary_living_actor(world, actor, context, order);
            return;
        }
    }
    step_enemy_lighting(actor);
}

fn step_ordinary_living_actor(
    world: &mut EnemyWorld,
    actor: &mut EnemyActor,
    context: &mut EnemyStepContext<'_>,
    order: &mut WorldPainterOrder,
) {
    let effect = context.ability_effect(actor.id);
    if effect.disrupted_ticks > 0 {
        interrupt_native_secondary_action(actor);
        actor.head_facing_offset = 0;
        if effect.time_scale != 0.0 {
            step_skeleton_recoil(actor);
        }
        step_enemy_lighting(actor);
        if matches!(actor.brain, EnemyBrain::Mage(_)) {
            apply_mage_provider_gate_after_action(actor);
        }
        return;
    }
    if effect.time_scale == 0.0 {
        step_enemy_lighting(actor);
        if matches!(actor.brain, EnemyBrain::Mage(_)) {
            apply_mage_provider_gate_after_action(actor);
        }
        return;
    }
    if actor.path.reorientation_ticks_remaining > 0 && !matches!(actor.brain, EnemyBrain::Coffin(_))
    {
        let target =
            super::targeting::target_by_id(actor.target_player_id.as_deref(), context.players)
                .map(|target| target.position);
        actor.heading_deg =
            actor.body.path.step_reorientation(actor.body.heading_deg, actor.body.position, target);
        step_skeleton_recoil(actor);
        step_enemy_lighting(actor);
        if matches!(actor.brain, EnemyBrain::Mage(_)) {
            apply_mage_provider_gate_after_action(actor);
        }
        return;
    }
    if effect.flee_ticks > 0 && !matches!(actor.brain, EnemyBrain::Coffin(_)) {
        interrupt_native_secondary_action(actor);
        actor.head_facing_offset = 0;
        super::movement::move_toward_target(
            world,
            actor,
            context,
            super::path::RadialDirection::Retreat,
            super::path::TangentDirection::Hold,
        );
        step_skeleton_recoil(actor);
        step_enemy_lighting(actor);
        return;
    }
    roll_skeleton_family_head_facing(world, actor);
    let source_brain = actor.brain.clone();
    if matches!(actor.brain, EnemyBrain::Mage(_)) {
        step_skeleton_recoil(actor);
        step_enemy_lighting(actor);
        super::mage::step_mage(world, actor, context, order);
        finalize_skeleton_family_head_facing(&source_brain, actor);
        apply_mage_provider_gate_after_action(actor);
        return;
    }
    match actor.brain {
        EnemyBrain::Skeleton(_) => super::skeleton::step_skeleton(world, actor, context, order),
        EnemyBrain::Archer(_) => super::archer::step_archer(world, actor, context, order),
        EnemyBrain::Imp(_) => super::imp::step_imp(world, actor, context, order),
        EnemyBrain::Zombie(_) => super::zombie::step_zombie(world, actor, context, order),
        EnemyBrain::Wraith(_) => super::wraith::step_wraith(world, actor, context, order),
        EnemyBrain::Demon(_) => super::demon::step_demon(world, actor, context, order),
        EnemyBrain::Coffin(_) => super::coffin::step_coffin(world, actor, context, order),
        EnemyBrain::Spider(_) => super::spider::step_spider(world, actor, context, order),
        _ => {
            unreachable!("families with private controllers return before ordinary action dispatch")
        }
    }
    step_skeleton_recoil(actor);
    finalize_skeleton_family_head_facing(&source_brain, actor);
    step_enemy_lighting(actor);
}

pub fn interrupt_native_secondary_action(actor: &mut EnemyActor) {
    match &mut actor.brain {
        EnemyBrain::Skeleton(brain) => super::skeleton::reset_skeleton(brain),
        EnemyBrain::Archer(brain) => super::archer::reset_archer(brain),
        EnemyBrain::Mage(brain) => super::mage::reset_mage(brain),
        EnemyBrain::Zombie(brain) => super::zombie::reset_zombie(brain),
        EnemyBrain::Demon(brain) => super::demon::reset_demon(brain),
        EnemyBrain::Faculty(_) => super::faculty::reset_faculty(actor),
        EnemyBrain::DemonSkull(brain) => {
            brain.actions.clear();
            brain.phase = BossPhase::RangeControl;
        }
        _ => {}
    }
}

pub fn step_damage_presentation_timers(actor: &mut EnemyActor, elapsed_ticks: u64, tick: u64) {
    if elapsed_ticks == 0 {
        return;
    }
    actor.hit_feedback.step(tick, elapsed_ticks);
    actor.hit_reaction_timer =
        crate::combat::step_native_puppet_hit_timer(actor.hit_reaction_timer, elapsed_ticks);
    if let EnemyBrain::Spider(brain) = &mut actor.brain {
        brain.spit_ticks_remaining = brain.spit_ticks_remaining.saturating_sub(elapsed_ticks);
    }
    actor.hurricane_contact_cooldown =
        (actor.hurricane_contact_cooldown - elapsed_ticks as f64 * 10.0).max(0.0);
    actor.shield_pulse = (actor.shield_pulse - elapsed_ticks as f64 * 0.05).max(0.0);
    actor.shield_sound_cooldown_ticks =
        actor.shield_sound_cooldown_ticks.saturating_sub(elapsed_ticks);
}

pub fn roll_skeleton_family_head_facing(world: &mut EnemyWorld, actor: &mut EnemyActor) {
    if !matches!(actor.brain, EnemyBrain::Skeleton(_) | EnemyBrain::Mage(_))
        || actor.target_player_id.is_none()
    {
        return;
    }
    if world.head_facing_rng.draw_integer(300) == 1 {
        actor.head_facing_offset = [-1, 0, 1][world.head_facing_rng.draw_integer(3) as usize];
    }
}

pub fn finalize_skeleton_family_head_facing(source: &EnemyBrain, actor: &mut EnemyActor) {
    if matches!(&actor.brain,EnemyBrain::Skeleton(brain) if brain.phase==SkeletonPhase::Attack) {
        return;
    }
    if matches!(source,EnemyBrain::Mage(brain) if brain.phase==MagePhase::Cast)
        && matches!(&actor.brain,EnemyBrain::Mage(brain) if brain.phase==MagePhase::Cast)
    {
        return;
    }
    actor.head_facing_offset = 0;
}

pub fn step_enemy_lighting(actor: &mut EnemyActor) {
    let prior = actor.lighting;
    let active = actor.config.scale != 0.0;
    let moving = actor.config.base_speed * actor.staff_movement_factor != 0.0;
    let burning = actor.config.burning;
    actor.lighting = match actor.config.enemy_token {
        EnemyToken::Spider | EnemyToken::Cocoon => prior,
        EnemyToken::DemonSkull => EnemyLighting {
            charge: 0.0,
            glow: match &actor.brain {
                EnemyBrain::DemonSkull(brain) => brain.light_intensity,
                _ => 0.0,
            },
            provider_copies: u8::from(active),
        },
        EnemyToken::DireFaculty => EnemyLighting {
            charge: 0.0,
            glow: match &actor.brain {
                EnemyBrain::Faculty(brain) => brain.light_intensity,
                _ => 0.0,
            },
            provider_copies: u8::from(active),
        },
        EnemyToken::Heartmonger => {
            EnemyLighting { charge: 0.0, glow: 0.5, provider_copies: u8::from(active) }
        }
        EnemyToken::Skeleton => EnemyLighting {
            glow: if moving && burning { (prior.glow + 0.05).min(1.0) } else { prior.glow },
            provider_copies: u8::from(moving && burning),
            ..prior
        },
        EnemyToken::SkeletonArcher => {
            if !moving {
                EnemyLighting { provider_copies: 0, ..prior }
            } else {
                let fire = matches!(
                    actor.config.family,
                    EnemyFamilyConfig::Archer { arrow_type: ArrowType::Fire, .. }
                );
                EnemyLighting {
                    charge: (prior.charge + 0.02).min(1.0),
                    glow: if burning { (prior.glow + 0.05).min(1.0) } else { prior.glow },
                    provider_copies: u8::from(fire) + u8::from(burning),
                }
            }
        }
        EnemyToken::SkeletonMage => {
            if !moving {
                EnemyLighting { provider_copies: 0, ..prior }
            } else {
                let once = if burning { (prior.glow + 0.05).min(1.0) } else { prior.glow };
                let glow = if burning { (once + 0.05).min(1.0) } else { once };
                let casting =
                    matches!(actor.brain, EnemyBrain::Mage(_)) && actor.body_pose.floor() == 4.0;
                let charge = if casting { prior.charge } else { (prior.charge + 0.02).min(1.0) };
                EnemyLighting {
                    charge,
                    glow,
                    provider_copies: if burning { 2 } else { u8::from(charge > 0.0) },
                }
            }
        }
        EnemyToken::Imp => EnemyLighting {
            glow: (prior.glow + 0.01).min(1.0),
            provider_copies: u8::from(moving),
            ..prior
        },
        EnemyToken::Portal => {
            let alpha = match &actor.brain {
                EnemyBrain::Portal(brain) => Some(brain.alpha),
                _ => None,
            };
            EnemyLighting {
                charge: alpha.unwrap_or(prior.charge),
                glow: alpha.unwrap_or(prior.glow),
                provider_copies: u8::from(active),
            }
        }
        EnemyToken::Wraith => EnemyLighting {
            glow: if burning { (prior.glow + 0.05).min(1.0) } else { prior.glow },
            provider_copies: u8::from(burning),
            ..prior
        },
        EnemyToken::Demon => EnemyLighting { provider_copies: 1, ..prior },
        EnemyToken::Coffin => EnemyLighting {
            provider_copies: u8::from(
                matches!(&actor.brain,EnemyBrain::Coffin(brain) if brain.phase!=CoffinPhase::Hidden),
            ),
            ..prior
        },
        EnemyToken::Zombie => EnemyLighting { provider_copies: 0, ..prior },
    };
}
