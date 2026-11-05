//! Archer volleys, elemental Mage births and lightning pulses from `projectile-emission.ts`.
use super::config::{ArrowType, EnemyFamilyConfig, MageElement};
use super::effects::{DeathEffectOwner, radial_vector};
use super::mage_attachments::MAGE_LIGHTNING_ATTACHMENTS;
use super::model::*;
use super::movement::staff_attack_speed;
use super::projectiles::*;
use super::ranged::{
    ArcherVolleyRequest, build_native_archer_volley, native_enemy_projectile_velocity,
};
use super::targeting::{target_by_id, target_eligible};
use super::world::{EnemyStepContext, EnemyWorld};
use crate::Vec2;
use crate::float;
use crate::float::fround;
use crate::painter::{WorldManagerLane, WorldPainterOrder};

#[derive(Clone, Copy, Debug, Default)]
pub struct SpawnEnemyProjectileOptions {
    pub cold_slow_ticks: Option<u64>,
    pub heading_deg: Option<f64>,
    pub lifetime_ticks: Option<u64>,
    pub minimum_speed: Option<f64>,
    pub payload: Option<EnemyProjectilePayload>,
    pub poison_damage: Option<f64>,
    pub poison_duration: Option<f64>,
    pub position: Option<Vec2>,
    pub secondary_damage: Option<f64>,
    pub speed: Option<f64>,
    pub visual_phase_deg: Option<f64>,
}

#[allow(clippy::too_many_arguments)] // Source birth receives the current work policy.
pub fn spawn_projectile(
    world: &mut EnemyWorld,
    actor: &EnemyActor,
    tick: u64,
    kind: EnemyProjectileKind,
    damage: f64,
    options: SpawnEnemyProjectileOptions,
    enhanced_effects: bool,
    order: &mut WorldPainterOrder,
) -> EnemyProjectile {
    let payload = options.payload.unwrap_or(if kind == EnemyProjectileKind::PoisonPool {
        EnemyProjectilePayload::Poison
    } else {
        EnemyProjectilePayload::None
    });
    let lane = if matches!(kind, EnemyProjectileKind::Arrow | EnemyProjectileKind::Firebolt) {
        WorldManagerLane::Transient
    } else {
        WorldManagerLane::Actor
    };
    let registration = world.register_projectile_painter(order, lane);
    let light = match kind {
        EnemyProjectileKind::Arrow => payload == EnemyProjectilePayload::Fire,
        EnemyProjectileKind::PoisonPool => false,
        _ => true,
    };
    let settled = if kind == EnemyProjectileKind::DemonBomb {
        100 + world.steering_rng.draw_integer(101) as u64
    } else {
        0
    };
    let heading = options.heading_deg.unwrap_or(actor.heading_deg);
    let visual_phase = if kind == EnemyProjectileKind::GuidedMissile {
        world.steering_rng.draw_scaled_float(360.0, false)
    } else {
        options.visual_phase_deg.unwrap_or(if kind == EnemyProjectileKind::Arrow {
            heading
        } else {
            0.0
        })
    };
    let turn = if kind == EnemyProjectileKind::GuidedMissile {
        fround(0.5 + world.steering_rng.draw_scaled_float(0.75, false))
    } else {
        0.0
    };
    let minimum = options.minimum_speed.unwrap_or_else(|| {
        if kind == EnemyProjectileKind::GuidedMissile {
            fround(0.75 + world.steering_rng.draw_scaled_float(0.44999998807907104, false))
        } else {
            0.0
        }
    });
    let scale = match kind {
        EnemyProjectileKind::Arrow => {
            if enhanced_effects {
                15.0
            } else {
                5.0
            }
        }
        EnemyProjectileKind::GuidedMissile => fround(
            0.8999999761581421 + world.steering_rng.draw_scaled_float(0.20000004768371582, false),
        ),
        _ => 1.0,
    };
    let speed = options.speed.unwrap_or_else(|| match kind {
        EnemyProjectileKind::Arrow => panic!("Arrow birth requires native speed"),
        EnemyProjectileKind::DemonBomb => {
            fround(2.0 + world.steering_rng.draw_scaled_float(1.0, false))
        }
        EnemyProjectileKind::Firebolt => 4.0,
        EnemyProjectileKind::GuidedMissile => 3.0,
        EnemyProjectileKind::PoisonPool => 0.0,
    });
    let lifetime = options.lifetime_ticks.unwrap_or_else(|| match kind {
        EnemyProjectileKind::Arrow => panic!("Arrow birth requires native lifetime"),
        EnemyProjectileKind::DemonBomb => settled,
        EnemyProjectileKind::Firebolt => 400,
        EnemyProjectileKind::GuidedMissile => 1300,
        EnemyProjectileKind::PoisonPool => 3200,
    });
    let origin = options.position.unwrap_or(actor.position);
    let forward = match kind {
        EnemyProjectileKind::Firebolt => 20.0,
        EnemyProjectileKind::GuidedMissile => 5.0,
        _ => 0.0,
    };
    let radians = heading * std::f64::consts::PI / 180.0;
    let zombie_duration = match actor.config.family {
        EnemyFamilyConfig::Zombie { poison_duration, .. } => poison_duration,
        _ => 0.0,
    };
    let state = EnemyProjectileState {
        age_ticks: 0,
        bounce_velocity: if kind == EnemyProjectileKind::DemonBomb { -3.0 } else { 0.0 },
        chill_tumble_accumulator: 0.0,
        cold_slow_ticks: options.cold_slow_ticks.unwrap_or(0),
        contact_radius: match kind {
            EnemyProjectileKind::Arrow => 20.0,
            EnemyProjectileKind::DemonBomb => 35.0,
            EnemyProjectileKind::Firebolt => 30.0,
            EnemyProjectileKind::GuidedMissile => 2.0,
            EnemyProjectileKind::PoisonPool => 70.0,
        },
        damage: if kind == EnemyProjectileKind::PoisonPool { 0.0 } else { damage },
        secondary_damage: options.secondary_damage.unwrap_or(0.0),
        heading_deg: heading,
        hit_player_ids: Vec::new(),
        homing: kind == EnemyProjectileKind::GuidedMissile,
        id: world.next_projectile_id,
        last_step_tick: tick,
        light_registration: light.then_some(registration),
        lifetime_ticks: lifetime,
        minimum_speed: minimum,
        native_type_id: kind.native_type_id(),
        native_cell_binding_order: world.next_native_cell_binding_order,
        native_registration_order: world.next_native_registration_order,
        owner_actor_id: actor.id,
        painter_registration: registration,
        payload,
        poison_damage: if kind == EnemyProjectileKind::PoisonPool {
            damage
        } else {
            options.poison_damage.unwrap_or(0.0)
        },
        poison_duration: if kind == EnemyProjectileKind::PoisonPool {
            zombie_duration
        } else {
            options.poison_duration.unwrap_or(0.0)
        },
        position: Vec2::new(
            fround(origin.x + float::sin(radians) * forward),
            fround(origin.y - float::cos(radians) * forward),
        ),
        speed,
        settled_ticks_remaining: if kind == EnemyProjectileKind::Arrow {
            lifetime as i64
        } else {
            settled as i64
        },
        spawn_tick: tick,
        target_player_id: actor.target_player_id.clone(),
        turn_speed: turn,
        vertical_offset: match kind {
            EnemyProjectileKind::DemonBomb => -35.0,
            EnemyProjectileKind::Arrow => -25.0,
            _ => 0.0,
        },
        vertical_velocity: 0.0,
        visual_phase_deg: visual_phase,
        visual_scale: scale,
    };
    let projectile = match kind {
        EnemyProjectileKind::Arrow => EnemyProjectile::Arrow {
            state,
            velocity: native_enemy_projectile_velocity(heading, speed),
        },
        EnemyProjectileKind::DemonBomb => EnemyProjectile::DemonBomb(state),
        EnemyProjectileKind::Firebolt => EnemyProjectile::Firebolt(state),
        EnemyProjectileKind::GuidedMissile => EnemyProjectile::GuidedMissile(state),
        EnemyProjectileKind::PoisonPool => EnemyProjectile::PoisonPool(state),
    };
    world.next_native_cell_binding_order += 1;
    world.next_native_registration_order += 1;
    world.next_projectile_id += 1;
    world.projectiles.push(projectile.clone());
    let mut event = EnemyEvent::new(EnemyEventKind::ProjectileSpawned, actor.id, tick);
    event.projectile_id = Some(projectile.state().id);
    event.target_player_id = Some(actor.target_player_id.clone());
    world.emit_event(event);
    projectile
}

pub fn emit_archer_volley(
    world: &mut EnemyWorld,
    actor: &EnemyActor,
    private_seed: u32,
    context: &EnemyStepContext<'_>,
    order: &mut WorldPainterOrder,
) {
    let EnemyFamilyConfig::Archer {
        accuracy_mode, arrow_type, extra_arrows, multi_arrow_mode, ..
    } = actor.config.family
    else {
        return;
    };
    let Some(target) = target_by_id(actor.target_player_id.as_deref(), context.players)
        .filter(|target| target_eligible(target))
    else {
        return;
    };
    let volley = build_native_archer_volley(
        ArcherVolleyRequest {
            accuracy_mode,
            arrow_type,
            extra_arrows,
            multi_arrow_mode,
            origin: actor.position,
            private_seed,
            target_position: target.position,
            target_velocity_per_tick: target.velocity_per_tick,
        },
        &mut world.steering_rng,
    );
    world.emit_action_sound(
        DeathEffectOwner::from(actor),
        context.tick,
        EnemySound::ShootArrow,
        volley.shot_pitch,
    );
    for arrow in volley.arrows {
        let poison = arrow.arrow_type == ArrowType::Poison;
        let payload = match arrow.arrow_type {
            ArrowType::Normal => EnemyProjectilePayload::Normal,
            ArrowType::Fire => EnemyProjectilePayload::Fire,
            ArrowType::Poison => EnemyProjectilePayload::Poison,
        };
        spawn_projectile(
            world,
            actor,
            context.tick,
            EnemyProjectileKind::Arrow,
            actor.config.primary_damage.unwrap_or(0.0),
            SpawnEnemyProjectileOptions {
                heading_deg: Some(arrow.heading_deg),
                lifetime_ticks: Some(arrow.lifetime_ticks),
                minimum_speed: Some(arrow.speed),
                payload: Some(payload),
                poison_damage: Some(if poison { actor.config.secondary_damage / 3.0 } else { 0.0 }),
                poison_duration: Some(if poison { 3.0 } else { 0.0 }),
                position: Some(arrow.position),
                secondary_damage: Some(if arrow.arrow_type == ArrowType::Fire {
                    actor.config.secondary_damage
                } else {
                    0.0
                }),
                speed: Some(arrow.speed),
                visual_phase_deg: Some(arrow.visual_heading_deg),
                ..SpawnEnemyProjectileOptions::default()
            },
            context.enhanced_effects,
            order,
        );
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MageLightningDispatch {
    pub target_player_id: String,
    pub target_position: Vec2,
}

pub fn emit_mage_attack(
    world: &mut EnemyWorld,
    actor: &EnemyActor,
    context: &EnemyStepContext<'_>,
    order: &mut WorldPainterOrder,
) -> Option<MageLightningDispatch> {
    let EnemyFamilyConfig::Mage { element, .. } = actor.config.family else {
        return None;
    };
    if element != MageElement::Lightning {
        let fire = element == MageElement::Fire;
        let pitch = if fire {
            1.25
        } else {
            fround(1.0 + world.steering_rng.draw_scaled_float(0.25, true))
        };
        world.emit_action_sound(
            DeathEffectOwner::from(actor),
            context.tick,
            if fire { EnemySound::ThrowFire } else { EnemySound::ThrowSpell },
            pitch,
        );
    }
    let (kind, damage, options) = match element {
        MageElement::Fire => (
            EnemyProjectileKind::Firebolt,
            actor.config.primary_damage.unwrap_or(0.0),
            SpawnEnemyProjectileOptions {
                payload: Some(EnemyProjectilePayload::Fire),
                ..SpawnEnemyProjectileOptions::default()
            },
        ),
        MageElement::Frost => (
            EnemyProjectileKind::GuidedMissile,
            actor.config.primary_damage.unwrap_or(0.0),
            SpawnEnemyProjectileOptions {
                cold_slow_ticks: Some(250),
                payload: Some(EnemyProjectilePayload::Cold),
                ..SpawnEnemyProjectileOptions::default()
            },
        ),
        MageElement::Poison => (
            EnemyProjectileKind::GuidedMissile,
            1.0,
            SpawnEnemyProjectileOptions {
                payload: Some(EnemyProjectilePayload::Poison),
                poison_damage: Some(actor.config.primary_damage.unwrap_or(0.0) / 10.0),
                poison_duration: Some(10.0),
                ..SpawnEnemyProjectileOptions::default()
            },
        ),
        MageElement::Lightning => {
            let target = target_by_id(actor.target_player_id.as_deref(), context.players)
                .filter(|target| target_eligible(target))?;
            return Some(MageLightningDispatch {
                target_player_id: target.id.clone(),
                target_position: target.position,
            });
        }
    };
    spawn_projectile(
        world,
        actor,
        context.tick,
        kind,
        damage,
        options,
        context.enhanced_effects,
        order,
    );
    None
}

pub fn step_mage_lightning_pulse(
    world: &mut EnemyWorld,
    actor: &mut EnemyActor,
    context: &EnemyStepContext<'_>,
    order: &mut WorldPainterOrder,
) {
    let EnemyBrain::Mage(brain) = &actor.brain else {
        return;
    };
    if brain.lightning_ticks_remaining == 0 {
        return;
    }
    let target_id = brain.lightning_target_player_id.as_deref();
    let target = target_by_id(target_id, context.players).filter(|target| target_eligible(target));
    let target_position = target
        .map(|target| target.position)
        .or(brain.lightning_target_position)
        .expect("active Mage lightning preserves its target position");
    let pose = if brain.phase == MagePhase::Cast {
        super::mage::mage_body_pose(brain.cast_program, brain.action_progress)
    } else {
        actor.body_pose.floor().clamp(0.0, 4.0)
    };
    let facing = ((actor.heading_deg + 10.0) / 20.0).trunc() as i32;
    let attachment = MAGE_LIGHTNING_ATTACHMENTS[pose as usize][facing.rem_euclid(18) as usize];
    let source = Vec2::new(actor.position.x + attachment.x, actor.position.y + attachment.y - 5.0);
    let clipped = context.geometry.clip_spell_segment(source, target_position);
    assert!(
        clipped.x.is_finite() && clipped.y.is_finite(),
        "clipped Mage lightning endpoint must be finite"
    );
    let clear = target.is_some() && clipped == target_position;
    if clear {
        let mut event =
            EnemyEvent::new(EnemyEventKind::MageLightningContact, actor.id, context.tick);
        event.target_player_id = Some(brain.lightning_target_player_id.clone());
        let event_id = world.emit_event(event);
        world.player_damage.push(EnemyPlayerDamage {
            hit_strength: None,
            webbed_strength: None,
            actor_id: actor.id,
            physical_damage: 0.0,
            magic_damage: fround(
                actor.config.primary_damage.unwrap_or(0.0) / 50.0 * staff_attack_speed(actor),
            ),
            cold_slow_ticks: 0,
            dazzle_ticks: 0,
            event_id,
            poison_damage: 0.0,
            poison_duration: 0.0,
            poison_contact_damage: None,
            suppress_hit_response: false,
            suppress_flash: true,
            player_id: target_id.unwrap().to_owned(),
            source: None,
            mana_damage_maximum_fraction: None,
            tragic_circle: false,
            crow_blind_chance_percent: None,
        });
    }
    let endpoint_offset = random_radial_displacement(world, 10.0);
    let contact_offset = random_radial_displacement(world, 15.0);
    let mut registrations =
        vec![order.register(WorldManagerLane::Actor), order.register(WorldManagerLane::Actor)];
    if !clear {
        registrations.push(order.register(WorldManagerLane::Actor));
    }
    world.mage_lightning_pulses.push(MageLightningPulse {
        enhanced_effects: context.enhanced_effects,
        contact: if clear {
            MageLightningContact::TargetAttached {
                local_offset: contact_offset,
                target_player_id: target_id.unwrap().to_owned(),
            }
        } else {
            MageLightningContact::World {
                position: Vec2::new(clipped.x + contact_offset.x, clipped.y + contact_offset.y),
            }
        },
        endpoint: Vec2::new(clipped.x + endpoint_offset.x, clipped.y + endpoint_offset.y),
        id: world.next_mage_lightning_pulse_id,
        light_registration: crate::painter::PainterRegistration {
            manager_lane: WorldManagerLane::Actor,
            registration_ordinal: actor.registration_ordinal,
        },
        midpoint: Vec2::new(
            (actor.position.x + target_position.x) * 0.5,
            (actor.position.y + target_position.y) * 0.5,
        ),
        owner_actor_id: actor.id,
        painter_registrations: registrations,
        seed: world.rng_state,
        source,
        tick: context.tick,
    });
    world.next_mage_lightning_pulse_id += 1;
    let EnemyBrain::Mage(brain) = &mut actor.brain else { unreachable!() };
    brain.lightning_ticks_remaining -= 1;
    if brain.lightning_ticks_remaining == 0 {
        brain.lightning_target_player_id = None;
        brain.lightning_target_position = None;
    } else {
        brain.lightning_target_position = Some(target_position);
    }
}

fn random_radial_displacement(world: &mut EnemyWorld, maximum: f64) -> Vec2 {
    let radius = world.draw_unit() * maximum;
    radial_vector(world.draw_unit() * 360.0, radius)
}
