//! Bomb fire handoff, poison bubbles and retained knockback from `projectile-hazards.ts`.
use super::config::EnemyFamilyConfig;
use super::effects::radial_vector;
use super::model::*;
use super::projectile_effects::*;
use super::projectiles::*;
use super::targeting::{target_by_id, target_eligible};
use super::world::{EnemyStepContext, EnemyWorld};
use crate::Vec2;
use crate::float::fround;
use crate::painter::{WorldManagerLane, WorldPainterOrder};
use crate::spells::fire::{
    CreateNativeFirePatchOptions, create_native_fire_patch, step_native_fire_patch,
};
use crate::spells::model::FirePatchType;

pub fn spawn_demon_fire_handoff(
    world: &mut EnemyWorld,
    projectile: &EnemyProjectileState,
    tick: u64,
    position: Vec2,
    order: &mut WorldPainterOrder,
) {
    for index in 0..2 {
        let phase = world.steering_rng.draw_scaled_float(32.0, false);
        let sign = if world.steering_rng.draw_integer(2) == 0 { -1.0 } else { 1.0 };
        let offset = if index == 0 {
            0.0
        } else {
            let magnitude = fround(10.0 + world.steering_rng.draw_scaled_float(10.0, false));
            world.steering_rng.draw_sign(magnitude)
        };
        let scale = fround(
            if index == 0 { 1.0 } else { fround(0.6) }
                + world.steering_rng.draw_scaled_float(0.2, false),
        );
        let position = Vec2::new(
            fround(position.x + offset),
            fround(position.y + if index == 0 { -10.0 } else { 5.0 }),
        );
        let mut state = create_projectile_effect(
            world,
            ProjectileEffectOwner::from(projectile),
            tick,
            position,
            ProjectileEffectSpec {
                entry: 46,
                lifetime_ticks: 501,
                registration_lane: WorldManagerLane::Actor,
            },
            ProjectileEffectOverrides {
                atlas: Some(EnemyEffectAtlas::DeadHawg),
                blend_mode: Some(EnemyEffectBlend::Add),
                scale: Some(scale),
                ..ProjectileEffectOverrides::default()
            },
            order,
        );
        let fire = create_enemy_fire_patch(&state, projectile.damage, 5.0, phase, sign);
        state.light_registration = Some(state.painter_registration);
        world.projectile_effects.push(EnemyProjectileEffect {
            state,
            kind: EnemyProjectileEffectKind::DemonFire { fire: Box::new(fire) },
        });
    }
}

fn create_enemy_fire_patch(
    state: &EnemyProjectileEffectState,
    damage: f64,
    life: f64,
    phase: f64,
    sign: f64,
) -> crate::spells::model::PrimarySpellFirePatch {
    create_native_fire_patch(
        CreateNativeFirePatchOptions {
            atlas_phase_step: None,
            burn_damage: 0.0,
            damage,
            draw_alpha: None,
            fade_alpha: None,
            id: state.id as u64,
            life: Some(life),
            native_type: FirePatchType::Fire,
            owner_id: format!("enemy:{}", state.owner_actor_id),
            painter_registration: Some(state.painter_registration),
            position: state.position,
            scale: Some(state.scale),
            supplemental_contact: false,
            velocity: None,
            velocity_multiplier: None,
            world_key: "enemy-store".to_owned(),
        },
        phase,
        sign,
    )
}

pub fn step_demon_fires(world: &mut EnemyWorld, context: &EnemyStepContext<'_>, tick: u64) {
    let source = std::mem::take(&mut world.projectile_effects);
    for mut effect in source {
        if effect.state.last_step_tick >= tick {
            world.projectile_effects.push(effect);
            continue;
        }
        let EnemyProjectileEffectKind::DemonFire { fire } = effect.kind else {
            world.projectile_effects.push(effect);
            continue;
        };
        let (patch, contact) = step_native_fire_patch(*fire, tick);
        if let Some(contact) = contact {
            for player in context.players {
                if !target_eligible(player) {
                    continue;
                }
                let x = fround(player.position.x - contact.position.x);
                let y = fround(player.position.y - contact.position.y);
                if fround(x * x + y * y) >= fround(contact.radius * contact.radius) {
                    continue;
                }
                world.steering_rng.draw_scaled_float(0.5, false);
                let mut damage = EnemyPlayerDamage::new(
                    effect.state.owner_actor_id,
                    world.next_event_id,
                    player.id.clone(),
                );
                world.next_event_id += 1;
                damage.physical_damage = fround(contact.amount * 0.5);
                damage.magic_damage = fround(contact.amount * 0.5);
                damage.suppress_hit_response = true;
                world.player_damage.push(damage);
            }
        }
        if let Some(fire) = patch {
            effect.state.age_ticks = fire.common.age_ticks;
            effect.state.alpha = fire.life.min(1.0);
            effect.state.last_step_tick = tick;
            effect.kind = EnemyProjectileEffectKind::DemonFire { fire: Box::new(fire) };
            world.projectile_effects.push(effect);
        }
    }
}

pub fn spawn_poison_pool_bubble(
    world: &mut EnemyWorld,
    pool: &EnemyProjectileState,
    tick: u64,
    order: &mut WorldPainterOrder,
) {
    if native_poison_pool_alpha(pool.age_ticks) <= 0.75 || world.steering_rng.draw_integer(20) != 3
    {
        return;
    }
    let growth = fround(fround(0.05) + world.steering_rng.draw_scaled_float(0.05, false));
    let maximum = fround(0.5 + world.steering_rng.draw_scaled_float(0.75, false));
    let hold = 25 + world.steering_rng.draw_integer(76) as u64;
    let radius = world.steering_rng.draw_scaled_float(50.0, false);
    let offset = radial_vector(world.steering_rng.draw_scaled_float(360.0, false), radius);
    let mut growth_ticks = 0;
    let mut scale = 0.0;
    while scale < maximum {
        scale = fround(scale + growth).min(maximum);
        growth_ticks += 1;
    }
    let position = Vec2::new(
        fround(pool.position.x + offset.x),
        fround(pool.position.y + fround(offset.y * fround(0.8))),
    );
    let state = create_projectile_effect(
        world,
        ProjectileEffectOwner::from(pool),
        tick,
        position,
        ProjectileEffectSpec {
            entry: 57,
            lifetime_ticks: growth_ticks + hold - 1,
            registration_lane: WorldManagerLane::Actor,
        },
        ProjectileEffectOverrides {
            alpha: Some(0.75),
            scale: Some(0.0),
            ..ProjectileEffectOverrides::default()
        },
        order,
    );
    world.projectile_effects.push(EnemyProjectileEffect {
        state,
        kind: EnemyProjectileEffectKind::PoisonBubble {
            growth_per_tick: growth,
            maximum_scale: maximum,
        },
    });
}

pub fn spawn_demon_explosion(
    world: &mut EnemyWorld,
    projectile: &EnemyProjectileState,
    tick: u64,
    order: &mut WorldPainterOrder,
) {
    let pitch = fround(1.0 + world.steering_rng.draw_scaled_float(0.1, false));
    for (sound, pitch) in [(EnemySound::FireballHit, pitch), (EnemySound::ThrowFire, fround(0.8))] {
        let mut event =
            EnemyEvent::new(EnemyEventKind::EnemyActionSound, projectile.owner_actor_id, tick);
        event.gain_scale = Some(2.0);
        event.pitch = Some(pitch);
        event.sound = Some(sound);
        event.source_position = Some(projectile.position);
        world.emit_event(event);
    }
    for (kind, lifetime, entry) in [
        (EnemyProjectileEffectKind::DemonExplosionCore, 10, 15),
        (EnemyProjectileEffectKind::DemonExplosionArray, 35, 401),
        (EnemyProjectileEffectKind::DemonExplosionLitArray, 37, 420),
    ] {
        let core = matches!(kind, EnemyProjectileEffectKind::DemonExplosionCore);
        let lit = matches!(kind, EnemyProjectileEffectKind::DemonExplosionLitArray);
        let registration = if lit {
            Some(world.register_projectile_painter(order, WorldManagerLane::Transient))
        } else {
            None
        };
        let state = create_projectile_effect(
            world,
            ProjectileEffectOwner::from(projectile),
            tick,
            projectile.position,
            ProjectileEffectSpec {
                entry,
                lifetime_ticks: lifetime,
                registration_lane: WorldManagerLane::Actor,
            },
            ProjectileEffectOverrides {
                light_registration: registration,
                blend_mode: Some(if core {
                    EnemyEffectBlend::Normal
                } else {
                    EnemyEffectBlend::Add
                }),
                scale: Some(1.5),
                ..ProjectileEffectOverrides::default()
            },
            order,
        );
        world.projectile_effects.push(EnemyProjectileEffect { state, kind });
    }
}

pub fn step_projectile_knockbacks(
    world: &mut EnemyWorld,
    context: &EnemyStepContext<'_>,
    tick: u64,
) {
    world.projectile_knockbacks.retain_mut(|source| {
        if source.last_step_tick >= tick {
            return true;
        }
        let Some(player) = target_by_id(Some(&source.player_id), context.players)
            .filter(|player| target_eligible(player))
        else {
            return false;
        };
        world.player_knockbacks.push(EnemyPlayerKnockback {
            actor_id: source.actor_id,
            delta: source.delta,
            event_id: source.event_id,
            player_id: player.id.clone(),
        });
        if source.remaining_ticks == 1 {
            return false;
        }
        source.last_step_tick = tick;
        source.remaining_ticks -= 1;
        true
    });
}

pub fn spawn_burning_skeleton_fire(
    world: &mut EnemyWorld,
    actor: &impl EnemyActorRead,
    tick: u64,
    order: &mut WorldPainterOrder,
) {
    if actor.life_state != EnemyLifeState::Alive
        || !actor.config.burning
        || actor.config.base_speed * actor.staff_movement_factor == 0.0
        || !matches!(
            actor.config.family,
            EnemyFamilyConfig::Skeleton { .. }
                | EnemyFamilyConfig::Archer { .. }
                | EnemyFamilyConfig::Mage { .. }
        )
    {
        return;
    }
    let copies = if matches!(actor.brain(), EnemyBrain::Mage(_)) { 2 } else { 1 };
    for _ in 0..copies {
        if world.steering_rng.draw_integer(5) != 3 {
            continue;
        }
        let phase = world.steering_rng.draw_scaled_float(32.0, false);
        let sign = world.steering_rng.draw_sign(1.0);
        let scale = fround(
            0.6000000238418579 + world.steering_rng.draw_scaled_float(0.3999999761581421, false),
        );
        let life = fround(0.35);
        let position = Vec2::new(fround(actor.position.x), fround(actor.position.y));
        let state = create_projectile_effect(
            world,
            ProjectileEffectOwner { id: actor.id, owner_actor_id: actor.id, age_ticks: 0 },
            tick,
            position,
            ProjectileEffectSpec {
                entry: 46,
                lifetime_ticks: 36,
                registration_lane: WorldManagerLane::Actor,
            },
            ProjectileEffectOverrides {
                alpha: Some(life),
                atlas: Some(EnemyEffectAtlas::DeadHawg),
                blend_mode: Some(EnemyEffectBlend::Add),
                scale: Some(scale),
                ..ProjectileEffectOverrides::default()
            },
            order,
        );
        let fire = create_enemy_fire_patch(
            &state,
            actor.config.primary_damage.unwrap_or(0.0),
            life,
            phase,
            sign,
        );
        world.projectile_effects.push(EnemyProjectileEffect {
            state,
            kind: EnemyProjectileEffectKind::DemonFire { fire: Box::new(fire) },
        });
    }
}
