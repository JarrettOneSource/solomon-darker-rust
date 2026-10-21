//! Coffin timed phases and pre-admission Maggot births from `enemies/coffin.ts`.
use super::config::EnemyFamilyConfig;
use super::effects::radial_vector;
use super::maggots::native_maggot_death_offsets;
use super::model::*;
use super::movement::positive_modulo;
use super::path::{NativeEnemyPathState, native_enemy_target_refresh_ticks};
use super::world::{EnemyStepContext, EnemyWorld};
use crate::Vec2;
use crate::float;
use crate::float::fround;
use crate::painter::{WorldManagerLane, WorldPainterOrder};

pub fn step_coffin(
    world: &mut EnemyWorld,
    actor: &mut EnemyActor,
    context: &EnemyStepContext<'_>,
    order: &mut WorldPainterOrder,
) {
    let EnemyBrain::Coffin(mut brain) = actor.brain.clone() else {
        panic!("Coffin brain required");
    };
    actor.position = brain.anchor_position;
    if brain.phase == CoffinPhase::Open {
        let speed = fround(actor.config.base_speed * actor.staff_movement_factor);
        if speed <= 0.0 {
            return;
        }
        let ratio = fround(brain.maggot_charge / speed);
        let count = if ratio < 1.0 { 3 } else { usize::from(world.draw_unit() * ratio < 1.0) };
        spawn_coffin_maggots(world, actor, context, count, order);
        if count > 0 {
            emit_coffin_release(world, actor.id, context.tick, count);
        }
        brain.maggot_charge = fround(brain.maggot_charge + (0.025_f32 as f64)).min(10.0);
        actor.brain = EnemyBrain::Coffin(brain);
        return;
    }
    brain.phase_ticks_remaining = brain.phase_ticks_remaining.saturating_sub(1);
    if brain.phase_ticks_remaining > 0 {
        brain.phase_tick += 1;
        actor.brain = EnemyBrain::Coffin(brain);
        return;
    }
    match brain.phase {
        CoffinPhase::Hidden => {
            super::coffin_emergence::spawn_coffin_emergence(
                world,
                actor,
                context.tick,
                context.enhanced_effects,
                order,
            );
            brain.phase = CoffinPhase::Rising;
            brain.phase_tick = 0;
            brain.phase_ticks_remaining = 11;
        }
        CoffinPhase::Rising => {
            brain.phase = CoffinPhase::Holding;
            brain.phase_tick = 0;
            brain.phase_ticks_remaining = 150 + u64::from(world.draw_integer(150));
        }
        CoffinPhase::Holding => {
            brain.phase = CoffinPhase::Opening;
            brain.phase_tick = 0;
            brain.phase_ticks_remaining = 46;
        }
        CoffinPhase::Opening => {
            let count = spawn_coffin_maggots(world, actor, context, 3, order);
            emit_coffin_release(world, actor.id, context.tick, count);
            brain.maggot_charge = 0.0;
            brain.phase = CoffinPhase::Open;
            brain.phase_tick = 46;
            brain.phase_ticks_remaining = 0;
        }
        CoffinPhase::Open | CoffinPhase::Death => {}
    }
    actor.brain = EnemyBrain::Coffin(brain);
}

fn emit_coffin_release(world: &mut EnemyWorld, actor_id: usize, tick: u64, count: usize) {
    let mut event = EnemyEvent::new(EnemyEventKind::CoffinMaggotRelease, actor_id, tick);
    event.count = Some(count);
    world.emit_event(event);
}

fn spawn_coffin_maggots(
    world: &mut EnemyWorld,
    actor: &EnemyActor,
    context: &EnemyStepContext<'_>,
    count: usize,
    order: &mut WorldPainterOrder,
) -> usize {
    let EnemyBrain::Coffin(brain) = &actor.brain else {
        return 0;
    };
    let EnemyFamilyConfig::Coffin { maggot_health, maggot_damage, maggot_poison_damage, .. } =
        actor.config.family
    else {
        return 0;
    };
    for _ in 0..count {
        let trajectory = if world.draw_integer(2) == 0 {
            MaggotLaunchTrajectory::Edge
        } else {
            MaggotLaunchTrajectory::Lid
        };
        let (start, end, heading_min) = match trajectory {
            MaggotLaunchTrajectory::Edge => (Vec2::new(5.5, 8.5), Vec2::new(15.5, -29.5), 140.0),
            MaggotLaunchTrajectory::Lid => (Vec2::new(-9.5, -4.5), Vec2::new(5.5, -41.5), 270.0),
        };
        let start = transform_launch_point(start, brain);
        let end = transform_launch_point(end, brain);
        let interpolation = world.draw_unit();
        let x = start.x + (end.x - start.x) * interpolation;
        let y = start.y + (end.y - start.y) * interpolation;
        let source_heading = heading_min + world.draw_unit() * 60.0;
        let heading = positive_modulo(
            if brain.launch_scale < 0 { 360.0 - source_heading } else { source_heading },
            360.0,
        );
        let scale = 1.0 + world.draw_unit() * 0.25;
        let velocity = radial_vector(heading, 1.0);
        let phase = world.draw_unit() * 5.0;
        let bounce = fround(-world.draw_unit() * 0.5);
        let offset = fround(y - start.y - world.draw_unit() * 8.0);
        let position =
            Vec2::new(actor.position.x + x, actor.position.y + start.y - world.draw_unit() * 8.0);
        let hurricane = world.draw_integer(100);
        let path = NativeEnemyPathState::new(&mut world.steering_rng);
        let death_offsets = native_maggot_death_offsets(world);
        let registration = order.register(WorldManagerLane::Actor);
        let id = world.next_actor_id;
        world.maggots.push(EnemyMaggotActor {
            ether_drain_captured: false,
            hit_feedback: PuppetHit { strength: 1.0, tick: context.tick, timer: 0.0 },
            blizzard_push_accumulator: 0.0,
            blizzard_push_last_tick: None,
            combat_active: false,
            native_tree_query_member: false,
            collision_radius: 8.0,
            current_health: maggot_health,
            death_offsets,
            damage: maggot_damage,
            death_epoch: None,
            death_started_tick: None,
            death_tick: 0,
            gait_pose: 0.0,
            heading_deg: heading,
            hurricane_contact_cooldown: f64::from(hurricane),
            id,
            emergence_tick: 0,
            emergence_phase: phase,
            launch_trajectory: trajectory,
            launch_velocity: velocity,
            landing_bounce_velocity: bounce,
            last_attack_tick: None,
            last_damaged_by_player_id: None,
            last_damage_tick: None,
            last_movement_tick: None,
            life_state: EnemyLifeState::Alive,
            registration_ordinal: registration.registration_ordinal,
            maximum_health: maggot_health,
            next_attack_tick: context.tick + 10,
            next_movement_tick: context.tick + 1,
            next_target_refresh_tick: context.tick + native_enemy_target_refresh_ticks(1),
            native_cell_binding_order: world.next_native_cell_binding_order,
            native_registration_order: world.next_native_registration_order,
            owner_coffin_actor_id: actor.id,
            path,
            poison_damage: maggot_poison_damage,
            poison_duration: if maggot_poison_damage > 0.0 { 10.0 } else { 0.0 },
            position,
            movement_phase: MaggotMovementPhase::Emerging,
            spawn_tick: context.tick,
            staff_action_factor: 1.0,
            staff_movement_factor: 1.0,
            target_player_id: None,
            terminal_emitted: false,
            vertical_offset: offset,
            vertical_velocity: 0.0,
            visual_scale: scale,
        });
        world.next_native_cell_binding_order += 1;
        world.next_native_registration_order += 1;
        world.spawned_actor_ids.push(id);
        let mut event = EnemyEvent::new(EnemyEventKind::EnemySpawned, id, context.tick);
        event.target_player_id = Some(None);
        world.emit_event(event);
        world.next_actor_id += 1;
    }
    count
}

fn transform_launch_point(point: Vec2, brain: &CoffinBrain) -> Vec2 {
    let radians = brain.launch_rotation_deg * std::f64::consts::PI / 180.0;
    let x = point.x * f64::from(brain.launch_scale);
    Vec2::new(
        x * float::cos(radians) - point.y * float::sin(radians),
        x * float::sin(radians) + point.y * float::cos(radians),
    )
}
