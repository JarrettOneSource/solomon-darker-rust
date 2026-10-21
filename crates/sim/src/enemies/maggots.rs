//! Coffin-owned Maggot admission, movement, bite and terminal effects from `maggots.ts`.
//! Emerging rows are real bodies; the separate admission pass decides combat ownership.
use super::EnemyToken;
use super::config::EnemyFamilyConfig;
use super::effects::{
    BouncerOptions, DeathEffectOverrides, DeathEffectOwner, SimpleDeathEffectSpec, radial_vector,
};
use super::model::*;
use super::movement::{MOVEMENT_CADENCE_TICKS, positive_modulo};
use super::path::*;
use super::targeting::{nearest_eligible_target, target_by_id, target_eligible};
use super::world::{EnemyMovementPurpose, EnemyMovementRequest, EnemyStepContext, EnemyWorld};
use crate::Vec2;
use crate::float::{fround, hypot};
use crate::heading::heading_degrees_from_vector;
use crate::painter::WorldPainterOrder;
use std::collections::BTreeMap;

pub(super) fn native_maggot_death_offsets(world: &mut EnemyWorld) -> Vec<Vec2> {
    let count = world.draw_integer(3);
    let mut offsets = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let radius = world.draw_unit() * 30.0;
        offsets.push(radial_vector(world.draw_unit() * 360.0, radius));
    }
    offsets
}

#[derive(Clone, Copy, Default)]
struct AdmissionCount {
    active: usize,
    inactive: usize,
}

impl EnemyWorld {
    pub fn step_maggots(
        &mut self,
        context: &EnemyStepContext<'_>,
        elapsed_ticks: u64,
        order: &mut WorldPainterOrder,
    ) {
        let mut counts: BTreeMap<usize, AdmissionCount> = BTreeMap::new();
        for maggot in &self.maggots {
            if maggot.life_state != EnemyLifeState::Alive
                || maggot.movement_phase != MaggotMovementPhase::Crawl
            {
                continue;
            }
            let count = counts.entry(maggot.owner_coffin_actor_id).or_default();
            if maggot.combat_active {
                count.active += 1;
            } else {
                count.inactive += 1;
            }
        }
        let mut owners = BTreeMap::new();
        for actor in &self.actors {
            if actor.life_state != EnemyLifeState::Alive
                || actor.config.enemy_token != EnemyToken::Coffin
            {
                continue;
            }
            let EnemyFamilyConfig::Coffin { maximum_maggots, .. } = actor.config.family else {
                panic!("Coffin config required");
            };
            owners.entry(actor.id).or_insert(maximum_maggots);
        }
        let stored = std::mem::take(&mut self.maggots);
        for mut source in stored {
            if source.hurricane_contact_cooldown > 0.0 {
                source.hurricane_contact_cooldown =
                    (source.hurricane_contact_cooldown - elapsed_ticks as f64 * 10.0).max(0.0);
            }
            source.hit_feedback.step(context.tick, elapsed_ticks);
            if !owners.contains_key(&source.owner_coffin_actor_id) {
                retire_maggot(self, &source, context.tick);
                continue;
            }
            if source.life_state == EnemyLifeState::Dying {
                if source.ether_drain_captured {
                    retire_maggot(self, &source, context.tick);
                    continue;
                }
                let death_tick =
                    context.tick.saturating_sub(source.death_started_tick.unwrap_or(context.tick));
                if source.last_attack_tick.is_some() && death_tick < 6 {
                    source.death_tick = death_tick;
                    // Native mark-only bite disposal leaves the grid at t+3.
                    source.native_tree_query_member &= death_tick < 3;
                    self.maggots.push(source);
                    continue;
                }
                self.emit_event(EnemyEvent::new(
                    EnemyEventKind::EnemyDeath,
                    source.id,
                    context.tick,
                ));
                emit_maggot_death_sounds(self, &source, context.tick);
                spawn_maggot_death_effects(
                    self,
                    &source,
                    context.tick,
                    context.enhanced_effects,
                    order,
                );
                retire_maggot(self, &source, context.tick);
                continue;
            }
            let effect = context.ability_effect(source.id);
            if effect.disrupted_ticks > 0 || effect.time_scale == 0.0 {
                self.maggots.push(source);
                continue;
            }
            if source.movement_phase == MaggotMovementPhase::Emerging {
                step_emerging_maggot(&mut source, context);
                if source.movement_phase == MaggotMovementPhase::Emerging {
                    self.maggots.push(source);
                    continue;
                }
                if !admit_maggot(self, &mut source, context, &mut counts, &owners) {
                    continue;
                }
            }
            if !source.combat_active {
                source.target_player_id = None;
                self.maggots.push(source);
                continue;
            }
            let current = target_by_id(source.target_player_id.as_deref(), context.players);
            if !current.is_some_and(|current| {
                target_eligible(current) && context.tick < source.next_target_refresh_tick
            }) {
                let selected = nearest_eligible_target(source.position, context.players)
                    .map(|target| target.id.clone());
                if selected != source.target_player_id {
                    source.path.clear_route();
                }
                source.target_player_id = selected;
                source.next_target_refresh_tick =
                    context.tick + native_enemy_target_refresh_ticks(1);
            }
            let target = target_by_id(source.target_player_id.as_deref(), context.players);
            if source.path.reorientation_ticks_remaining > 0 {
                source.heading_deg = source.path.step_reorientation(
                    source.heading_deg,
                    source.position,
                    target.map(|target| target.position),
                );
                self.maggots.push(source);
                continue;
            }
            let distance = target.map_or(f64::INFINITY, |target| {
                hypot(target.position.x - source.position.x, target.position.y - source.position.y)
            });
            let fleeing = effect.flee_ticks > 0;
            if let Some(target) = target
                && !fleeing
                && distance <= 18.0_f64.max(source.collision_radius + target.collision_radius)
            {
                if context.tick >= source.next_attack_tick {
                    let mut event =
                        EnemyEvent::new(EnemyEventKind::AttackMarker, source.id, context.tick);
                    event.target_player_id = Some(Some(target.id.clone()));
                    let event_id = self.emit_event(event);
                    let mut damage = EnemyPlayerDamage::new(source.id, event_id, target.id.clone());
                    damage.physical_damage = source.damage * effect.weaken_factor;
                    damage.poison_damage = source.poison_damage;
                    damage.poison_duration = source.poison_duration;
                    self.player_damage.push(damage);
                    source.death_epoch = Some(self.next_death_epoch);
                    self.next_death_epoch += 1;
                    if let Some(count) = counts.get_mut(&source.owner_coffin_actor_id) {
                        count.active = count.active.saturating_sub(1);
                    }
                    source.death_started_tick = Some(context.tick);
                    source.death_tick = 0;
                    source.heading_deg = heading_degrees_from_vector(
                        target.position.x - source.position.x,
                        target.position.y - source.position.y,
                    );
                    source.last_attack_tick = Some(context.tick);
                    source.life_state = EnemyLifeState::Dying;
                    source.terminal_emitted = false;
                }
                self.maggots.push(source);
                continue;
            }
            if context.tick < source.next_movement_tick {
                self.maggots.push(source);
                continue;
            }
            move_maggot(self, &mut source, context, fleeing, effect.time_scale);
            self.maggots.push(source);
        }
    }
}

fn admit_maggot(
    world: &mut EnemyWorld,
    source: &mut EnemyMaggotActor,
    context: &EnemyStepContext<'_>,
    counts: &mut BTreeMap<usize, AdmissionCount>,
    owners: &BTreeMap<usize, usize>,
) -> bool {
    let Some(maximum) = owners.get(&source.owner_coffin_actor_id) else {
        retire_maggot(world, source, context.tick);
        return false;
    };
    let count = counts.entry(source.owner_coffin_actor_id).or_default();
    count.inactive += 1;
    if count.active < *maximum && world.draw_integer(5) == 3 {
        count.inactive -= 1;
        count.active += 1;
        source.combat_active = true;
        source.next_attack_tick = context.tick + 10;
        source.target_player_id = nearest_eligible_target(source.position, context.players)
            .map(|target| target.id.clone());
        return true;
    }
    if count.inactive > 30 {
        count.inactive -= 1;
        retire_maggot(world, source, context.tick);
        return false;
    }
    source.combat_active = false;
    source.target_player_id = None;
    true
}

fn move_maggot(
    world: &mut EnemyWorld,
    source: &mut EnemyMaggotActor,
    context: &EnemyStepContext<'_>,
    fleeing: bool,
    speed_scale: f64,
) {
    let target = target_by_id(source.target_player_id.as_deref(), context.players);
    let target_heading = target.map_or(0.0, |target| {
        if hypot(target.velocity_per_tick.x, target.velocity_per_tick.y) == 0.0 {
            target.heading_deg.unwrap_or(0.0)
        } else {
            heading_degrees_from_vector(target.velocity_per_tick.x, target.velocity_per_tick.y)
        }
    });
    let mut request = EnemySteeringRequest {
        actor_heading_deg: source.heading_deg,
        actor_position: source.position,
        cadence_ticks: MOVEMENT_CADENCE_TICKS,
        full_movement: true,
        goal_position: None,
        movement_per_tick: 0.5 / MOVEMENT_CADENCE_TICKS as f64
            * speed_scale
            * source.staff_movement_factor
            * source.path.speed_factor,
        radial_direction: if fleeing {
            RadialDirection::Retreat
        } else {
            RadialDirection::Approach
        },
        status_factor: speed_scale,
        tangent_direction: TangentDirection::Hold,
        target_heading_deg: target_heading,
        target_position: target.map(|target| target.position),
    };
    let mut path = source.path;
    let mut goal = native_enemy_steering_goal(&path, &request);
    if !fleeing {
        let routed = resolve_native_enemy_path_goal(
            path,
            EnemyPathGoalRequest {
                actor_position: source.position,
                body_radius: source.collision_radius,
                cadence_ticks: MOVEMENT_CADENCE_TICKS,
                navigation_clearance: 25.0,
                raw_goal: goal,
                target_position: target.map(|target| target.position),
                target_refresh_ticks: native_enemy_target_refresh_ticks(1),
            },
            context.geometry,
        );
        path = routed.state;
        goal = routed.goal;
        if routed.turn_around {
            request.actor_heading_deg = positive_modulo(request.actor_heading_deg + 180.0, 360.0);
        }
    }
    request.goal_position = Some(goal);
    let steering = build_native_enemy_steering(path, request);
    let delta = steering.delta;
    let position = context.geometry.resolve_movement(EnemyMovementRequest {
        actor_id: source.id,
        delta,
        position: source.position,
        purpose: EnemyMovementPurpose::Movement,
        radius: source.collision_radius,
        requested_position: Vec2::new(source.position.x + delta.x, source.position.y + delta.y),
    });
    assert!(
        position.x.is_finite() && position.y.is_finite(),
        "resolved Maggot position must be finite"
    );
    let traveled = hypot(position.x - source.position.x, position.y - source.position.y);
    let mut path = steering.state;
    path.step_recovery(
        &mut world.steering_rng,
        PathRecoveryRequest {
            flanking_enabled: true,
            requested_distance: hypot(delta.x, delta.y),
            status_factor: source.staff_movement_factor * speed_scale,
            tick: context.tick,
            traveled_distance: traveled,
        },
    );
    if traveled != 0.0 {
        source.gait_pose = positive_modulo(source.gait_pose + traveled / 2.0, 2.0);
        source.last_movement_tick = Some(context.tick);
    }
    source.heading_deg = steering.heading_deg;
    source.next_movement_tick = context.tick + MOVEMENT_CADENCE_TICKS;
    source.path = path;
    source.position = position;
    // Native free movement rebinds even when the resolved displacement is zero.
    source.native_tree_query_member = true;
}

fn step_emerging_maggot(source: &mut EnemyMaggotActor, context: &EnemyStepContext<'_>) {
    let requested = context.tick.saturating_sub(source.spawn_tick + source.emergence_tick);
    if requested == 0 {
        return;
    }
    let mut elapsed = 0;
    let mut settled = false;
    let mut delta = Vec2::ZERO;
    while elapsed < requested {
        delta.x += source.launch_velocity.x;
        delta.y += source.launch_velocity.y;
        source.vertical_offset = fround(source.vertical_offset + source.vertical_velocity);
        source.vertical_velocity = fround(source.vertical_velocity + 0.075);
        source.emergence_phase = fround(source.emergence_phase + 0.25);
        if source.emergence_phase >= 5.0 {
            source.emergence_phase = fround(source.emergence_phase - 5.0);
        }
        elapsed += 1;
        if source.vertical_offset <= 0.0 {
            continue;
        }
        source.vertical_velocity = source.landing_bounce_velocity;
        source.launch_velocity = Vec2::new(
            fround(source.launch_velocity.x * 0.5),
            fround(source.launch_velocity.y * 0.5),
        );
        source.landing_bounce_velocity = fround(source.landing_bounce_velocity * 0.5);
        if source.landing_bounce_velocity > -0.25 {
            settled = true;
            source.vertical_offset = 0.0;
            break;
        }
    }
    let position = context.geometry.resolve_movement(EnemyMovementRequest {
        actor_id: source.id,
        delta,
        position: source.position,
        purpose: EnemyMovementPurpose::Movement,
        radius: source.collision_radius,
        requested_position: Vec2::new(source.position.x + delta.x, source.position.y + delta.y),
    });
    assert!(
        position.x.is_finite() && position.y.is_finite(),
        "resolved emerging Maggot position must be finite"
    );
    if hypot(position.x - source.position.x, position.y - source.position.y) != 0.0 {
        source.last_movement_tick = Some(context.tick);
    }
    source.emergence_tick += elapsed;
    source.movement_phase =
        if settled { MaggotMovementPhase::Crawl } else { MaggotMovementPhase::Emerging };
    source.next_movement_tick = context.tick + if settled { MOVEMENT_CADENCE_TICKS } else { 1 };
    source.position = position;
}

fn retire_maggot(world: &mut EnemyWorld, maggot: &EnemyMaggotActor, tick: u64) {
    let event_id = world.emit_event(EnemyEvent::new(EnemyEventKind::EnemyRetired, maggot.id, tick));
    world.retired.push(EnemyRetirement { actor_id: maggot.id, event_id });
}

fn emit_maggot_death_sounds(world: &mut EnemyWorld, maggot: &EnemyMaggotActor, tick: u64) {
    let pitch = 1.0 + world.draw_unit() * 0.2;
    let sound = [EnemySound::MaggotSquish1, EnemySound::MaggotSquish2, EnemySound::MaggotSquish3]
        [world.draw_integer(3) as usize];
    emit_maggot_death_sound(world, maggot, tick, sound, pitch, 1.0);
    let pitch = 1.0 + world.draw_unit() * 0.2;
    let gain = 0.25 + world.draw_unit() * 0.25;
    let sound =
        [EnemySound::MaggotSqueak1, EnemySound::MaggotSqueak2][world.draw_integer(2) as usize];
    emit_maggot_death_sound(world, maggot, tick, sound, pitch, gain);
}
fn emit_maggot_death_sound(
    world: &mut EnemyWorld,
    maggot: &EnemyMaggotActor,
    tick: u64,
    sound: EnemySound,
    pitch: f64,
    gain: f64,
) {
    let mut event = EnemyEvent::new(EnemyEventKind::EnemyDeathSound, maggot.id, tick);
    event.sound = Some(sound);
    event.pitch = Some(pitch);
    event.gain_scale = Some(gain);
    event.source_position = Some(maggot.position);
    world.emit_event(event);
}

fn spawn_maggot_death_effects(
    world: &mut EnemyWorld,
    maggot: &EnemyMaggotActor,
    tick: u64,
    enhanced_effects: bool,
    order: &mut WorldPainterOrder,
) {
    let owner = DeathEffectOwner { id: maggot.id, position: maggot.position };
    spawn_fragment(world, owner, tick, "maggot-fragment:root", enhanced_effects, order);
    spawn_perspective_fade(world, owner, tick, "maggot-perspective-fade:root", order);
    for (index, offset) in maggot.death_offsets.iter().enumerate() {
        let owner = DeathEffectOwner {
            id: maggot.id,
            position: Vec2::new(maggot.position.x + offset.x, maggot.position.y + offset.y),
        };
        spawn_perspective_fade(
            world,
            owner,
            tick,
            &format!("maggot-perspective-fade:{}", index + 1),
            order,
        );
        spawn_fragment(
            world,
            owner,
            tick,
            &format!("maggot-fragment:{}", index + 1),
            enhanced_effects,
            order,
        );
    }
}
fn spawn_fragment(
    world: &mut EnemyWorld,
    owner: DeathEffectOwner,
    tick: u64,
    role: &str,
    enhanced_effects: bool,
    order: &mut WorldPainterOrder,
) {
    world.spawn_bouncer(
        owner,
        tick,
        |world| 2013 + world.draw_integer(50) as usize,
        role,
        |world| {
            let speed = 0.5 + world.draw_unit() * 0.5;
            let direction = radial_vector(world.draw_unit() * 360.0, speed);
            let velocity = Vec2::new(direction.x * 1.5, direction.y);
            let distance = 15.0 + world.draw_unit() * 10.0;
            BouncerOptions {
                position: Some(Vec2::new(
                    owner.position.x + velocity.x * (distance + 2.0),
                    owner.position.y + velocity.y * distance,
                )),
                velocity: Some(velocity),
                ..BouncerOptions::default()
            }
        },
        enhanced_effects,
        order,
    );
}
fn spawn_perspective_fade(
    world: &mut EnemyWorld,
    owner: DeathEffectOwner,
    tick: u64,
    role: &str,
    order: &mut WorldPainterOrder,
) {
    let rotation = world.draw_unit() * 360.0;
    let scale = 0.65 + world.draw_unit() * 0.35;
    let alpha = 0.25 + world.draw_unit() * 0.25;
    world.spawn_simple_death_effect(
        owner,
        tick,
        SimpleDeathEffectSpec {
            alpha,
            alpha_loss_per_tick: 0.01,
            atlas: EnemyEffectAtlas::DeadHawg,
            blend_mode: EnemyEffectBlend::Normal,
            entry: 28,
            kind: EnemyDeathEffectKind::FadePerspective,
            lifetime_ticks: 250,
            role,
            scale,
        },
        DeathEffectOverrides {
            alpha_multiplier: Some(alpha),
            opacity_timer: Some(2.5),
            rotation_deg: Some(rotation),
            tint: Some(0x828c6b),
            ..DeathEffectOverrides::default()
        },
        order,
    );
}
