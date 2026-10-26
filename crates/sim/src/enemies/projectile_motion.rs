//! Enemy projectile fixed ticks and contact ordering from `enemies/projectiles.ts`.
use super::model::*;
use super::projectile_effects::{spawn_firebolt_trail, spawn_projectile_impact_effects};
use super::projectile_hazards::{
    spawn_demon_explosion, spawn_demon_fire_handoff, spawn_poison_pool_bubble, step_demon_fires,
    step_projectile_knockbacks,
};
use super::projectiles::*;
use super::ranged::native_enemy_projectile_velocity;
use super::targeting::{target_by_id, target_eligible};
use super::world::{
    EnemyProjectileWorldQuery, EnemyStepContext, EnemyTargetCellBinding, EnemyWorld,
};
use crate::Vec2;
use crate::float::{fround, hypot};
use crate::heading::heading_degrees_from_vector;
use crate::painter::WorldPainterOrder;
use crate::spells::targeting::{native_heading_turn_direction, native_primary_cell_coordinate};

impl EnemyWorld {
    pub fn bind_enemy_targets(&mut self, players: &[EnemyTarget]) {
        for player in players {
            self.bind_native_query_target(&player.id, player.position);
        }
    }

    pub fn bind_native_query_target(&mut self, id: &str, position: Vec2) {
        let cell_x = native_primary_cell_coordinate(position.x);
        let cell_y = native_primary_cell_coordinate(position.y);
        if self
            .target_cell_bindings
            .get(id)
            .is_some_and(|binding| binding.cell_x == cell_x && binding.cell_y == cell_y)
        {
            return;
        }
        self.target_cell_bindings.insert(
            id.to_owned(),
            EnemyTargetCellBinding { cell_x, cell_y, order: self.next_native_cell_binding_order },
        );
        self.next_native_cell_binding_order += 1;
    }

    pub fn step_projectiles(
        &mut self,
        context: &EnemyStepContext<'_>,
        order: &mut WorldPainterOrder,
    ) {
        let mut first_tick = context.tick;
        for projectile in self.projectiles.iter() {
            first_tick = first_tick.min(projectile.state().last_step_tick + 1);
        }
        for effect in &self.projectile_effects {
            if matches!(effect.kind, EnemyProjectileEffectKind::DemonFire { .. }) {
                first_tick = first_tick.min(effect.state.last_step_tick + 1);
            }
        }
        for knockback in &self.projectile_knockbacks {
            first_tick = first_tick.min(knockback.last_step_tick + 1);
        }
        for tick in first_tick..=context.tick {
            step_projectile_knockbacks(self, context, tick);
            step_demon_fires(self, context, tick);
            let mut projectiles = std::mem::take(&mut self.projectiles);
            projectiles.retain_mut(|projectile| {
                projectile.state.last_step_tick >= tick
                    || step_projectile(self, projectile, context, tick, order)
            });
            self.projectiles = projectiles;
        }
        self.step_projectile_effects(context.tick);
    }
}

fn step_projectile(
    world: &mut EnemyWorld,
    projectile: &mut EnemyProjectileMut<'_>,
    context: &EnemyStepContext<'_>,
    tick: u64,
    order: &mut WorldPainterOrder,
) -> bool {
    let state = &mut *projectile.state;
    match projectile.motion {
        EnemyProjectileMotion::Arrow { velocity } => {
            step_arrow(world, state, velocity, context, tick, order)
        }
        EnemyProjectileMotion::Firebolt => step_firebolt(world, state, context, tick, order),
        EnemyProjectileMotion::GuidedMissile => {
            step_guided_missile(world, state, context, tick, order)
        }
        EnemyProjectileMotion::DemonBomb => step_demon_bomb(world, state, context, tick, order),
        EnemyProjectileMotion::PoisonPool => step_poison_pool(world, state, context, tick, order),
    }
}

fn move_projectile(state: &mut EnemyProjectileState, tick: u64) {
    let velocity = native_enemy_projectile_velocity(state.heading_deg, state.speed);
    state.age_ticks += 1;
    state.last_step_tick = tick;
    state.position =
        Vec2::new(fround(state.position.x + velocity.x), fround(state.position.y + velocity.y));
}

fn terrain_blocked(
    state: &EnemyProjectileState,
    velocity: Vec2,
    context: &EnemyStepContext<'_>,
    ticks_ahead: f64,
    mask: u32,
) -> bool {
    context.geometry.projectile_world_blocked(EnemyProjectileWorldQuery::Line {
        projectile_id: state.id,
        start: state.position,
        end: Vec2::new(
            fround(state.position.x + velocity.x * ticks_ahead),
            fround(state.position.y + velocity.y * ticks_ahead),
        ),
        native_exclusion_mask: mask,
    })
}

fn step_arrow(
    world: &mut EnemyWorld,
    state: &mut EnemyProjectileState,
    velocity: &mut Vec2,
    context: &EnemyStepContext<'_>,
    tick: u64,
    order: &mut WorldPainterOrder,
) -> bool {
    state.age_ticks += 1;
    state.last_step_tick = tick;
    state.position =
        Vec2::new(fround(state.position.x + velocity.x), fround(state.position.y + velocity.y));
    if context.geometry.projectile_world_blocked(EnemyProjectileWorldQuery::Bounds {
        projectile_id: state.id,
        position: state.position,
        margin: 0.0,
    }) {
        retire_projectile(
            world,
            &EnemyProjectileRef {
                state,
                motion: EnemyProjectileMotion::Arrow { velocity: *velocity },
            },
            tick,
            state.target_player_id.as_deref(),
        );
        return false;
    }
    let player = if state.vertical_offset == 0.0 {
        None
    } else {
        first_player_center_contact(state.position, context.players, 20.0)
    };
    if let Some(player) = player {
        emit_projectile_impact(
            world,
            &EnemyProjectileRef {
                state,
                motion: EnemyProjectileMotion::Arrow { velocity: *velocity },
            },
            tick,
            Some(&player.id),
            order,
        );
    }
    let blocked =
        state.age_ticks.is_multiple_of(5) && terrain_blocked(state, *velocity, context, 5.0, 0x380);
    if blocked {
        emit_projectile_impact(
            world,
            &EnemyProjectileRef {
                state,
                motion: EnemyProjectileMotion::Arrow { velocity: *velocity },
            },
            tick,
            None,
            order,
        );
    }
    if player.is_some() || blocked {
        retire_projectile(
            world,
            &EnemyProjectileRef {
                state,
                motion: EnemyProjectileMotion::Arrow { velocity: *velocity },
            },
            tick,
            player.map(|player| player.id.as_str()),
        );
        return false;
    }
    state.settled_ticks_remaining = (state.settled_ticks_remaining - 1).max(0);
    if state.settled_ticks_remaining > 0 {
        state.visual_phase_deg = state.heading_deg;
        return true;
    }
    if state.vertical_offset >= -3.0 {
        state.visual_scale = fround(state.visual_scale - 0.05000000074505806);
        if state.visual_scale <= 0.0 {
            retire_projectile(
                world,
                &EnemyProjectileRef {
                    state,
                    motion: EnemyProjectileMotion::Arrow { velocity: *velocity },
                },
                tick,
                state.target_player_id.as_deref(),
            );
            return false;
        }
        state.speed = 0.0;
        *velocity = Vec2::ZERO;
        state.vertical_offset = 0.0;
        return true;
    }
    state.vertical_offset = fround(state.vertical_offset + 0.75);
    let launch = native_enemy_projectile_velocity(state.heading_deg, state.minimum_speed);
    let launch_speed = fround(fround(launch.x * launch.x + launch.y * launch.y).sqrt());
    let vertical_term = fround((-25.0 / state.vertical_offset) * launch_speed * 0.25);
    *velocity =
        Vec2::new(fround(velocity.x * 0.9900000095367432), fround(velocity.y * 0.9900000095367432));
    state.speed = fround(hypot(velocity.x, velocity.y));
    state.visual_phase_deg =
        fround(heading_degrees_from_vector(launch.x, fround(launch.y + vertical_term)));
    true
}

fn step_firebolt(
    world: &mut EnemyWorld,
    state: &mut EnemyProjectileState,
    context: &EnemyStepContext<'_>,
    tick: u64,
    order: &mut WorldPainterOrder,
) -> bool {
    let velocity = native_enemy_projectile_velocity(state.heading_deg, state.speed);
    if (state.age_ticks + 1).is_multiple_of(10)
        && terrain_blocked(state, velocity, context, 10.0, 0x700)
    {
        let projectile = EnemyProjectileRef { state, motion: EnemyProjectileMotion::Firebolt };
        emit_projectile_impact(world, &projectile, tick, None, order);
        retire_projectile(world, &projectile, tick, None);
        return false;
    }
    let previous_tick = state.last_step_tick;
    move_projectile(state, tick);
    let outside = context.geometry.projectile_world_blocked(EnemyProjectileWorldQuery::View {
        projectile_id: state.id,
        position: state.position,
        margin: 50.0,
    });
    spawn_firebolt_trail(world, state, previous_tick, order);
    let player = first_player_center_contact(state.position, context.players, 30.0);
    let expired = state.age_ticks >= state.lifetime_ticks;
    let projectile = EnemyProjectileRef { state, motion: EnemyProjectileMotion::Firebolt };
    if let Some(player) = player {
        emit_projectile_impact(world, &projectile, tick, Some(&player.id), order);
    }
    if player.is_some() || outside || expired {
        retire_projectile(world, &projectile, tick, player.map(|player| player.id.as_str()));
        false
    } else {
        true
    }
}

fn step_guided_missile(
    world: &mut EnemyWorld,
    state: &mut EnemyProjectileState,
    context: &EnemyStepContext<'_>,
    tick: u64,
    order: &mut WorldPainterOrder,
) -> bool {
    let old_heading = state.heading_deg;
    let old_speed = state.speed;
    let old_phase = state.visual_phase_deg;
    move_projectile(state, tick);
    if context.geometry.projectile_world_blocked(EnemyProjectileWorldQuery::Bounds {
        projectile_id: state.id,
        position: state.position,
        margin: 500.0,
    }) {
        let projectile = EnemyProjectileRef { state, motion: EnemyProjectileMotion::GuidedMissile };
        retire_projectile(world, &projectile, tick, projectile.state().target_player_id.as_deref());
        return false;
    }
    let target = target_by_id(state.target_player_id.as_deref(), context.players)
        .filter(|target| target_eligible(target));
    let desired = target.map_or(old_heading, |target| {
        heading_degrees_from_vector(
            target.position.x - state.position.x,
            target.position.y - state.position.y,
        )
    });
    state.heading_deg = fround(
        (old_heading
            + state.turn_speed * f64::from(native_heading_turn_direction(old_heading, desired))
            + 360.0)
            % 360.0,
    );
    state.target_player_id = target.map(|target| target.id.clone());
    state.visual_phase_deg = fround(old_phase + old_speed * 6.0);
    let player = first_guided_missile_contact(
        world,
        state.position,
        state.target_player_id.as_deref(),
        context.players,
    );
    let projectile = EnemyProjectileRef { state, motion: EnemyProjectileMotion::GuidedMissile };
    if let Some(player) = player {
        emit_projectile_impact(world, &projectile, tick, Some(&player.id), order);
    }
    // The native lookahead keeps the vector calculated before this tick's turn.
    let blocked = projectile.state().age_ticks.is_multiple_of(5)
        && terrain_blocked(
            projectile.state(),
            native_enemy_projectile_velocity(old_heading, old_speed),
            context,
            5.0,
            0x700,
        );
    if blocked {
        emit_projectile_impact(world, &projectile, tick, None, order);
    }
    if player.is_some() || blocked {
        retire_projectile(world, &projectile, tick, player.map(|player| player.id.as_str()));
        return false;
    }
    if projectile.state().age_ticks >= projectile.state().lifetime_ticks {
        retire_projectile(world, &projectile, tick, projectile.state().target_player_id.as_deref());
        return false;
    }
    state.speed = fround((old_speed - 0.07500000298023224).max(state.minimum_speed));
    true
}

pub(super) fn first_guided_missile_contact<'a>(
    world: &EnemyWorld,
    position: Vec2,
    target_player_id: Option<&str>,
    players: &'a [EnemyTarget],
) -> Option<&'a EnemyTarget> {
    if let Some(target) =
        target_by_id(target_player_id, players).filter(|target| target_eligible(target))
        && squared_distance(position, target.position) < 100.0
    {
        return Some(target);
    }
    let cell_x = native_primary_cell_coordinate(position.x);
    let cell_y = native_primary_cell_coordinate(position.y);
    let mut selected = None;
    let mut first_order = usize::MAX;
    for player in players {
        if !target_eligible(player)
            || native_primary_cell_coordinate(player.position.x) != cell_x
            || native_primary_cell_coordinate(player.position.y) != cell_y
        {
            continue;
        }
        let radius = player.collision_radius + 2.0;
        let order = world
            .target_cell_bindings
            .get(&player.id)
            .expect("projectile targets must be bound before their update")
            .order;
        if order < first_order && squared_distance(position, player.position) < radius * radius {
            selected = Some(player);
            first_order = order;
        }
    }
    selected
}

fn step_demon_bomb(
    world: &mut EnemyWorld,
    state: &mut EnemyProjectileState,
    context: &EnemyStepContext<'_>,
    tick: u64,
    order: &mut WorldPainterOrder,
) -> bool {
    let blocked = context.geometry.projectile_world_blocked(EnemyProjectileWorldQuery::Point {
        projectile_id: state.id,
        position: state.position,
        radius: 1.0,
    });
    let source_damage = state.damage;
    if blocked {
        state.speed = 0.0;
    }
    move_projectile(state, tick);
    let player = first_player_center_contact(state.position, context.players, 35.0);
    state.speed = fround(state.speed * 0.9950000047683716);
    state.vertical_offset = fround(state.vertical_offset + state.vertical_velocity);
    state.vertical_velocity = fround(state.vertical_velocity + 0.10000000149011612);
    if state.vertical_offset > 0.0 {
        state.vertical_offset = 0.0;
        state.bounce_velocity = fround(state.bounce_velocity * 0.8500000238418579);
        state.vertical_velocity =
            if state.bounce_velocity > -0.10000000149011612 { 0.0 } else { state.bounce_velocity };
    }
    if blocked || player.is_some() {
        state.speed = 0.0;
        state.settled_ticks_remaining = 0;
    }
    if state.bounce_velocity > -1.0 {
        state.speed = fround(state.speed * 0.9800000190734863);
    }
    if state.speed < 1.0 {
        state.settled_ticks_remaining -= 1;
        state.bounce_velocity = 0.0;
    }
    if state.settled_ticks_remaining > 0 {
        if state.speed >= 1.0 && context.enhanced_effects {
            super::projectile_effects::spawn_demon_bomb_particle(world, state, tick, order);
        }
        return true;
    }
    context.geometry.on_projectile_explosion(state.position);
    spawn_demon_explosion(world, state, tick, order);
    spawn_demon_fire_handoff(world, state, tick, state.position, order);
    let mut event = EnemyEvent::new(EnemyEventKind::ProjectileImpact, state.owner_actor_id, tick);
    event.projectile_id = Some(state.id);
    event.target_player_id = Some(None);
    let event_id = world.emit_event(event);
    for player in context.players {
        if !target_eligible(player)
            || squared_distance(state.position, player.position) >= 82.5 * 82.5
        {
            continue;
        }
        let x = fround(player.position.x - state.position.x);
        let y = fround(player.position.y - state.position.y);
        let distance = fround(hypot(x, y));
        if !player.summoned && distance > 0.0 {
            world.projectile_knockbacks.push(EnemyProjectileKnockback {
                actor_id: state.owner_actor_id,
                event_id,
                player_id: player.id.clone(),
                last_step_tick: tick,
                remaining_ticks: 10,
                delta: Vec2::new(fround(x / distance * 4.5), fround(y / distance * 4.5)),
            });
        }
        let mut damage = EnemyPlayerDamage::new(state.owner_actor_id, event_id, player.id.clone());
        damage.physical_damage = fround(source_damage * 0.5);
        damage.magic_damage = fround(source_damage * 0.5);
        world.player_damage.push(damage);
    }
    retire_projectile(
        world,
        &EnemyProjectileRef { state, motion: EnemyProjectileMotion::DemonBomb },
        tick,
        None,
    );
    false
}

fn step_poison_pool(
    world: &mut EnemyWorld,
    state: &mut EnemyProjectileState,
    context: &EnemyStepContext<'_>,
    tick: u64,
    order: &mut WorldPainterOrder,
) -> bool {
    state.age_ticks += 1;
    state.last_step_tick = tick;
    state.visual_scale = fround(state.visual_scale + fround(0.025)).min(fround(1.6));
    spawn_poison_pool_bubble(world, state, tick, order);
    let projectile = EnemyProjectileRef { state, motion: EnemyProjectileMotion::PoisonPool };
    for player in context.players {
        if !player.summoned
            && target_eligible(player)
            && native_poison_pool_contact(projectile.state().position, player.position)
        {
            let event = world.next_event_id;
            world.next_event_id += 1;
            apply_projectile_contact(world, &projectile, &player.id, event);
        }
    }
    if native_poison_pool_alpha(projectile.state().age_ticks) == 0.0 {
        retire_projectile(world, &projectile, tick, projectile.state().target_player_id.as_deref());
        false
    } else {
        true
    }
}

fn first_player_center_contact(
    position: Vec2,
    players: &[EnemyTarget],
    radius: f64,
) -> Option<&EnemyTarget> {
    players.iter().find(|player| {
        !player.summoned
            && target_eligible(player)
            && squared_distance(position, player.position) < radius * radius
    })
}
fn squared_distance(left: Vec2, right: Vec2) -> f64 {
    let x = fround(left.x - right.x);
    let y = fround(left.y - right.y);
    fround(x * x + y * y)
}

fn emit_projectile_impact(
    world: &mut EnemyWorld,
    projectile: &impl EnemyProjectileRead,
    tick: u64,
    player: Option<&str>,
    order: &mut WorldPainterOrder,
) {
    spawn_projectile_impact_effects(world, projectile, tick, order);
    let state = projectile.state();
    let mut event = EnemyEvent::new(EnemyEventKind::ProjectileImpact, state.owner_actor_id, tick);
    event.projectile_id = Some(state.id);
    event.target_player_id = Some(player.map(str::to_owned));
    let event_id = world.emit_event(event);
    let Some(player) = player else {
        return;
    };
    if projectile.kind() == EnemyProjectileKind::GuidedMissile {
        let mut event =
            EnemyEvent::new(EnemyEventKind::PlayerStatusSound, state.owner_actor_id, tick);
        event.gain_scale = Some(1.0);
        event.pitch = Some(fround(0.85));
        event.sound = Some(EnemySound::MagicMissileHit);
        event.source_position = Some(state.position);
        event.target_player_id = Some(Some(player.to_owned()));
        world.emit_event(event);
    }
    apply_projectile_contact(world, projectile, player, event_id);
}
fn apply_projectile_contact(
    world: &mut EnemyWorld,
    projectile: &impl EnemyProjectileRead,
    player: &str,
    event_id: usize,
) {
    let state = projectile.state();
    let fire = fround(if projectile.kind() == EnemyProjectileKind::Firebolt {
        state.damage * 0.5
    } else {
        state.secondary_damage * 0.5
    });
    let mut damage = EnemyPlayerDamage::new(state.owner_actor_id, event_id, player.to_owned());
    damage.physical_damage = if projectile.kind() == EnemyProjectileKind::Arrow {
        fround(state.damage + fire)
    } else {
        fire
    };
    damage.magic_damage = match projectile.kind() {
        EnemyProjectileKind::Arrow | EnemyProjectileKind::Firebolt => fire,
        _ => state.damage,
    };
    damage.cold_slow_ticks = state.cold_slow_ticks;
    damage.poison_damage = state.poison_damage;
    damage.poison_duration = state.poison_duration;
    if projectile.kind() == EnemyProjectileKind::GuidedMissile {
        damage.poison_contact_damage =
            Some(if state.payload == EnemyProjectilePayload::Poison { 1.0 } else { 0.0 });
        damage.suppress_hit_response = true;
    }
    if projectile.kind() == EnemyProjectileKind::PoisonPool {
        damage.suppress_flash = true;
    }
    world.player_damage.push(damage);
}
fn retire_projectile(
    world: &mut EnemyWorld,
    projectile: &impl EnemyProjectileRead,
    tick: u64,
    target: Option<&str>,
) {
    let state = projectile.state();
    if projectile.kind() == EnemyProjectileKind::Firebolt {
        world.projectile_effects.retain(|effect| {
            !matches!(effect.kind, EnemyProjectileEffectKind::FireboltTrail)
                || effect.state.owner_projectile_id != state.id
        });
    }
    let mut event = EnemyEvent::new(EnemyEventKind::ProjectileRetired, state.owner_actor_id, tick);
    event.projectile_id = Some(state.id);
    event.target_player_id = Some(target.map(str::to_owned));
    world.emit_event(event);
}
