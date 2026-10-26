//! Full Golem assembly, targeting, walking, articulation, attacks and damage from Website
//! `core-kernels/native-secondary-golem.ts`. World placement stays with the world owner.

use crate::float;
use crate::float::{atan2, fround, hypot};
use crate::spells::targeting::native_heading_turn_direction;
use crate::{NativeRng, Vec2};

pub const NATIVE_GOLEM_RADIUS: f64 = 30.0;
pub const NATIVE_GOLEM_PLACEMENT_RADIUS: f64 = 25.0;
pub const NATIVE_GOLEM_REFLECT_DISTANCE_SQUARED: f64 = 6400.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum NativeGolemPhase {
    Active,
    Assembly,
    Attack,
    Provoke,
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(bevy_ecs::component::Component)]
pub struct NativeSecondaryGolemState {
    pub action_heading_offset_degrees: f64,
    pub action_duration_ticks: u64,
    pub action_tick: u64,
    pub circle_slow_ticks: u64,
    pub current_health: f64,
    pub damage_maximum: f64,
    pub iron: bool,
    pub gait_tick: u64,
    pub left_connector_offset: Vec2,
    pub left_foot: Vec2,
    pub left_foot_bob: Vec2,
    pub left_foot_next: Vec2,
    pub left_foot_previous: Vec2,
    pub left_foot_progress: f64,
    pub left_foot_rotation_degrees: f64,
    pub left_limb_mode: u32,
    pub maximum_health: f64,
    pub orbit_direction: f64,
    pub orbit_heading_radians: Option<f64>,
    pub phase: NativeGolemPhase,
    pub pose_variant: u32,
    pub provoke_roll_bound: i32,
    pub reflect_factor: f64,
    pub right_connector_offset: Vec2,
    pub right_foot: Vec2,
    pub right_foot_bob: Vec2,
    pub right_foot_next: Vec2,
    pub right_foot_previous: Vec2,
    pub right_foot_progress: f64,
    pub right_foot_rotation_degrees: f64,
    pub right_limb_mode: u32,
    pub target_poll_ticks_remaining: i32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeGolemBirthStats {
    pub health: f64,
    pub damage_maximum: f64,
    pub iron: bool,
    pub reflect_factor: f64,
    pub pose_variant: u32,
}

/// The source allows foot targets to use an optional world resolver. Its absent resolver
/// returns the requested position, so unobstructed worlds need only supply root movement.
pub trait NativeGolemWorld {
    fn resolve_movement(&mut self, requested_position: Vec2) -> Vec2;
    fn resolve_foot_target(&mut self, _current_position: Vec2, requested_position: Vec2) -> Vec2 {
        requested_position
    }
}

impl NativeSecondaryGolemState {
    #[must_use]
    pub fn new(
        position: Vec2,
        rotation_radians: f64,
        stats: NativeGolemBirthStats,
        world: &mut impl NativeGolemWorld,
    ) -> Self {
        let left =
            world.resolve_foot_target(position, foot_target(position, rotation_radians, 0, -19.0));
        let right =
            world.resolve_foot_target(position, foot_target(position, rotation_radians, 1, -19.0));
        Self {
            action_heading_offset_degrees: 0.0,
            action_duration_ticks: 0,
            action_tick: 0,
            circle_slow_ticks: 0,
            current_health: stats.health,
            damage_maximum: stats.damage_maximum,
            iron: stats.iron,
            gait_tick: 0,
            left_connector_offset: Vec2::ZERO,
            left_foot: left,
            left_foot_bob: Vec2::ZERO,
            left_foot_next: left,
            left_foot_previous: left,
            left_foot_progress: 1.0,
            left_foot_rotation_degrees: 0.0,
            left_limb_mode: 0,
            maximum_health: stats.health,
            orbit_direction: 0.0,
            orbit_heading_radians: None,
            phase: NativeGolemPhase::Assembly,
            pose_variant: stats.pose_variant,
            provoke_roll_bound: 0,
            reflect_factor: stats.reflect_factor,
            right_connector_offset: Vec2::ZERO,
            right_foot: right,
            right_foot_bob: Vec2::ZERO,
            right_foot_next: right,
            right_foot_previous: right,
            right_foot_progress: 1.0,
            right_foot_rotation_degrees: 0.0,
            right_limb_mode: 0,
            target_poll_ticks_remaining: 50,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeGolemKernelActor {
    pub age_ticks: u64,
    pub damage_minimum: f64,
    pub golem: NativeSecondaryGolemState,
    pub id: u64,
    pub owner_id: String,
    pub position: Vec2,
    pub rotation_radians: f64,
    pub target_id: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeGolemKernelTarget {
    pub id: usize,
    pub position: Vec2,
    pub radius: f64,
    pub registration_order: Option<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeGolemContact {
    pub damage: f64,
    pub heading_degrees: f64,
    pub movement_budget: f64,
    pub origin: Vec2,
    pub target_ids: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeGolemKernelStepResult {
    pub assembly_milestone: Option<u64>,
    pub contact: Option<NativeGolemContact>,
    pub footstep: bool,
    pub provoke_started: bool,
}

pub fn step_native_secondary_golem(
    actor: &mut NativeGolemKernelActor,
    owner_position: Option<Vec2>,
    targets: &[NativeGolemKernelTarget],
    world: &mut impl NativeGolemWorld,
    rng: &mut NativeRng,
) -> NativeGolemKernelStepResult {
    let assembling = actor.age_ticks < 201;
    let previous_age = actor.age_ticks;
    actor.age_ticks += if assembling { 2 } else { 1 };
    if assembling {
        actor.golem.phase = NativeGolemPhase::Assembly;
        actor.golem.left_foot = world.resolve_foot_target(
            actor.golem.left_foot,
            foot_target(actor.position, actor.rotation_radians, 0, 0.0),
        );
        actor.golem.right_foot = world.resolve_foot_target(
            actor.golem.right_foot,
            foot_target(actor.position, actor.rotation_radians, 1, 0.0),
        );
        actor.golem.circle_slow_ticks = actor.golem.circle_slow_ticks.saturating_sub(1);
        actor.golem.left_foot_bob = Vec2::ZERO;
        actor.golem.right_foot_bob = Vec2::ZERO;
        return NativeGolemKernelStepResult {
            assembly_milestone: if matches!(previous_age, 0 | 50 | 100 | 200) {
                Some(previous_age)
            } else {
                None
            },
            contact: None,
            footstep: false,
            provoke_started: false,
        };
    }
    if actor.golem.phase == NativeGolemPhase::Assembly {
        actor.golem.phase = NativeGolemPhase::Active;
    }
    if actor.golem.phase == NativeGolemPhase::Provoke {
        actor.golem.action_tick += 1;
        if actor.golem.action_tick < actor.golem.action_duration_ticks {
            return active_result(actor, None, false, world, rng);
        }
        actor.golem.action_duration_ticks = 0;
        actor.golem.action_tick = 0;
        actor.golem.phase = NativeGolemPhase::Active;
    }
    if actor.golem.phase == NativeGolemPhase::Attack {
        if let Some(target) = targets.iter().find(|target| Some(target.id) == actor.target_id) {
            actor.rotation_radians = turn_golem(actor, target.position, true);
        }
        let action_tick = actor.golem.action_tick + 1;
        let mut contact = None;
        if action_tick == 37 {
            let damage = rng.draw_float_range(actor.damage_minimum, actor.golem.damage_maximum);
            let heading_degrees = fround(actor.rotation_radians * 180.0 / std::f64::consts::PI);
            let backward = fround(
                (std::f64::consts::PI as f32 as f64) * fround(heading_degrees + 180.0) / 180.0,
            );
            let origin = Vec2::new(
                fround(
                    fround((actor.golem.left_foot.x + actor.golem.right_foot.x) * 0.5)
                        + fround(fround(float::sin(backward)) * 20.0),
                ),
                fround(
                    fround((actor.golem.left_foot.y + actor.golem.right_foot.y) * 0.5)
                        + fround(fround(-float::cos(backward)) * 20.0),
                ),
            );
            contact = Some(NativeGolemContact {
                damage,
                heading_degrees,
                movement_budget: 50.0,
                origin,
                target_ids: contact_targets(origin, actor.rotation_radians, targets),
            });
        }
        if action_tick < actor.golem.action_duration_ticks {
            actor.golem.action_tick = action_tick;
            return active_result(actor, contact, false, world, rng);
        }
        actor.golem.action_duration_ticks = 0;
        actor.golem.action_tick = 0;
        actor.golem.phase = NativeGolemPhase::Active;
        actor.golem.provoke_roll_bound = 75;
        let provoked = maybe_start_provoke(actor, rng);
        return active_result(actor, contact, provoked, world, rng);
    }

    let eligible: Vec<_> = targets
        .iter()
        .filter(|target| squared_distance(target.position, actor.position) < 999999.0)
        .copied()
        .collect();
    let mut target = eligible.iter().find(|target| Some(target.id) == actor.target_id).copied();
    if target.is_none() {
        actor.golem.target_poll_ticks_remaining -= 1;
        if actor.golem.target_poll_ticks_remaining < 1 {
            target = nearest_target(actor.position, &eligible);
            actor.golem.target_poll_ticks_remaining = 50;
        }
    }
    actor.target_id = target.map(|target| target.id);
    if let Some(target) = target {
        let distance =
            hypot(target.position.x - actor.position.x, target.position.y - actor.position.y);
        if distance < target.radius + NATIVE_GOLEM_RADIUS + 20.0 {
            let duration = rng.draw_integer(20);
            actor.golem.action_duration_ticks = (90 - duration) as u64;
            actor.golem.action_tick = 0;
            actor.golem.phase = NativeGolemPhase::Attack;
            actor.golem.pose_variant = 1 - actor.golem.pose_variant;
            actor.rotation_radians = turn_golem(actor, target.position, true);
            return active_result(actor, None, false, world, rng);
        }
    }
    let mut desired = target.map(|target| target.position);
    if desired.is_none()
        && let Some(owner) = owner_position
    {
        let mut orbit_heading = actor.golem.orbit_heading_radians;
        if squared_distance(actor.position, owner) > 90.0 * 90.0 {
            orbit_heading = None;
        }
        if orbit_heading.is_none() {
            orbit_heading = Some(native_heading(owner, actor.position));
            actor.golem.orbit_direction = rng.draw_scaled_float(1.0, true);
        }
        let heading = orbit_heading.expect("Golem has an owner orbit heading");
        let destination =
            Vec2::new(owner.x + float::sin(heading) * 80.0, owner.y - float::cos(heading) * 80.0);
        if squared_distance(actor.position, destination) < 20.0 * 20.0 {
            orbit_heading = Some(normalize_radians(
                heading + actor.golem.orbit_direction * 20.0 * std::f64::consts::PI / 180.0,
            ));
        }
        desired = Some(destination);
        actor.golem.orbit_heading_radians = orbit_heading;
    }
    if let Some(desired) = desired
        && hypot(desired.x - actor.position.x, desired.y - actor.position.y) > 0.0
    {
        let rotation = turn_golem(actor, desired, false);
        let speed = 0.5 * if actor.golem.circle_slow_ticks > 0 { 0.5 } else { 1.0 };
        let requested = Vec2::new(
            fround(actor.position.x + float::sin(rotation) * speed),
            fround(actor.position.y - float::cos(rotation) * speed),
        );
        actor.position = world.resolve_movement(requested);
        actor.rotation_radians = rotation;
    }
    let provoked = maybe_start_provoke(actor, rng);
    active_result(actor, None, provoked, world, rng)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeGolemIncomingDamage {
    pub primary_damage: f64,
    pub reflectable_physical_source_in_range: bool,
    pub secondary_damage: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeGolemDamageResult {
    pub actor: Option<NativeGolemKernelActor>,
    pub ignored: bool,
    pub killed: bool,
    pub reflected_damage: f64,
}

#[must_use]
pub fn damage_native_secondary_golem(
    mut actor: NativeGolemKernelActor,
    damage: NativeGolemIncomingDamage,
) -> NativeGolemDamageResult {
    assert!(
        damage.primary_damage.is_finite() && damage.primary_damage >= 0.0,
        "Golem incoming primary damage is finite and non-negative"
    );
    assert!(
        damage.secondary_damage.is_finite() && damage.secondary_damage >= 0.0,
        "Golem incoming secondary damage is finite and non-negative"
    );
    if actor.age_ticks < 400 {
        return NativeGolemDamageResult {
            actor: Some(actor),
            ignored: true,
            killed: false,
            reflected_damage: 0.0,
        };
    }
    actor.golem.current_health =
        actor.golem.current_health - damage.primary_damage - damage.secondary_damage;
    let killed = actor.golem.current_health <= 0.0;
    let reflected_damage = if damage.reflectable_physical_source_in_range {
        damage.primary_damage * actor.golem.reflect_factor
    } else {
        0.0
    };
    NativeGolemDamageResult {
        actor: if killed { None } else { Some(actor) },
        ignored: false,
        killed,
        reflected_damage,
    }
}

fn maybe_start_provoke(actor: &mut NativeGolemKernelActor, rng: &mut NativeRng) -> bool {
    let provoke = rng.draw_integer(actor.golem.provoke_roll_bound);
    if provoke != 70 && actor.golem.provoke_roll_bound != 0 {
        return false;
    }
    actor.golem.action_duration_ticks = 151;
    actor.golem.action_tick = 0;
    actor.golem.phase = NativeGolemPhase::Provoke;
    actor.golem.provoke_roll_bound = 1200;
    true
}

fn contact_targets(
    origin: Vec2,
    rotation_radians: f64,
    targets: &[NativeGolemKernelTarget],
) -> Vec<usize> {
    use crate::spells::targeting::{
        PrimaryConeQuery, PrimarySpellQueryLane, PrimarySpellTarget, PrimarySpellTargetKind,
        native_primary_cone_targets,
    };
    let mut rows = Vec::with_capacity(targets.len());
    for (index, target) in targets.iter().enumerate() {
        let order = target.registration_order.unwrap_or(index);
        rows.push(PrimarySpellTarget {
            active: true,
            actor_flags: 2,
            attachment: target.position,
            body_radius: target.radius,
            cell_binding_order: order,
            heading_degrees: None,
            grid_position: None,
            id: target.id.to_string(),
            kind: PrimarySpellTargetKind::Enemy,
            native_priority: 0,
            pending_remove: false,
            position: target.position,
            query_lane: PrimarySpellQueryLane::Grid,
            registration_order: order,
        });
    }
    native_primary_cone_targets(PrimaryConeQuery {
        actor_mask: 2,
        aim_direction: Vec2::new(float::sin(rotation_radians), -float::cos(rotation_radians)),
        half_angle_degrees: 90.0,
        has_line_of_sight: &|_| true,
        origin,
        reach: 120.0,
        targets: &rows,
    })
    .into_iter()
    .map(|target| target.id.parse().expect("Golem targets retain numeric IDs"))
    .collect()
}

#[derive(Clone, Copy)]
struct FootMotion {
    current: Vec2,
    previous: Vec2,
    next: Vec2,
    progress: f64,
}

fn advance_foot(
    actor: &mut NativeGolemKernelActor,
    side: u64,
    mut foot: FootMotion,
    world: &mut impl NativeGolemWorld,
    rng: &mut NativeRng,
    footstep: &mut bool,
) -> FootMotion {
    if actor.golem.gait_tick % 100 == side * 50 {
        foot.previous = foot.current;
        foot.next = world.resolve_foot_target(
            foot.current,
            foot_target(actor.position, actor.rotation_radians, side, -19.0),
        );
        foot.progress = 0.0;
    }
    if foot.progress < 1.0 {
        let speed = if actor.golem.circle_slow_ticks > 0 { 0.5 } else { 1.0 };
        let advanced = fround(fround(foot.progress + fround(0.015 * speed)) * (1.06_f32 as f64));
        if advanced > 1.0 {
            *footstep = true;
            rng.draw_scaled_float(0.1, false);
            actor.golem.left_foot_rotation_degrees = rng.draw_scaled_float(8.0, true);
            actor.golem.right_foot_rotation_degrees = rng.draw_scaled_float(8.0, true);
        }
        foot.progress = advanced.min(1.0);
    }
    foot.current = Vec2::new(
        fround(foot.previous.x + foot.progress * (foot.next.x - foot.previous.x)),
        fround(foot.previous.y + foot.progress * (foot.next.y - foot.previous.y)),
    );
    foot
}

fn advance_articulation(
    actor: &mut NativeGolemKernelActor,
    world: &mut impl NativeGolemWorld,
    rng: &mut NativeRng,
) -> bool {
    actor.golem.gait_tick += 1;
    let mut footstep = false;
    let left = FootMotion {
        current: actor.golem.left_foot,
        previous: actor.golem.left_foot_previous,
        next: actor.golem.left_foot_next,
        progress: actor.golem.left_foot_progress,
    };
    let left = advance_foot(actor, 0, left, world, rng, &mut footstep);
    let right = FootMotion {
        current: actor.golem.right_foot,
        previous: actor.golem.right_foot_previous,
        next: actor.golem.right_foot_next,
        progress: actor.golem.right_foot_progress,
    };
    let right = advance_foot(actor, 1, right, world, rng, &mut footstep);
    let bob = fround(
        -float::sin(actor.golem.gait_tick as f64 * 3.6 * std::f64::consts::PI / 180.0) * 3.0,
    );
    actor.golem.left_foot = left.current;
    actor.golem.left_foot_previous = left.previous;
    actor.golem.left_foot_next = left.next;
    actor.golem.left_foot_progress = left.progress;
    actor.golem.left_foot_bob = Vec2::new(0.0, bob);
    actor.golem.right_foot = right.current;
    actor.golem.right_foot_previous = right.previous;
    actor.golem.right_foot_next = right.next;
    actor.golem.right_foot_progress = right.progress;
    actor.golem.right_foot_bob = Vec2::new(0.0, bob);
    update_pose(&mut actor.golem);
    footstep
}

fn update_pose(golem: &mut NativeSecondaryGolemState) {
    golem.left_connector_offset = Vec2::ZERO;
    golem.right_connector_offset = Vec2::ZERO;
    if golem.phase == NativeGolemPhase::Attack {
        let left = golem.pose_variant == 0;
        if golem.action_tick <= 37 {
            golem.action_heading_offset_degrees =
                if golem.action_tick < 25 { if left { -38.0 } else { 38.0 } } else { 0.0 };
            golem.left_limb_mode = if left { 1 } else { 0 };
            golem.right_limb_mode = if left { 0 } else { 1 };
        } else {
            golem.action_heading_offset_degrees = if left { 47.0 } else { -47.0 };
            golem.left_limb_mode = if left { 2 } else { 1 };
            golem.right_limb_mode = if left { 1 } else { 2 };
        }
    } else if golem.phase == NativeGolemPhase::Provoke && golem.action_tick > 100 {
        golem.action_heading_offset_degrees = 0.0;
        golem.left_connector_offset = Vec2::new(0.0, -12.0);
        golem.right_connector_offset = Vec2::new(0.0, -12.0);
        golem.left_limb_mode = 3;
        golem.right_limb_mode = 3;
    } else {
        golem.action_heading_offset_degrees = 0.0;
        golem.left_limb_mode = 0;
        golem.right_limb_mode = 0;
    }
}

fn foot_target(position: Vec2, rotation: f64, side: u64, forward: f64) -> Vec2 {
    let full = std::f64::consts::TAU;
    let normalized = (rotation % full + full) % full;
    let facing = ((normalized / full * 16.0 + 0.5).floor() as u32) % 16;
    let radians = f64::from(facing) * full / 16.0;
    let forward_x = float::sin(radians);
    let forward_y = -float::cos(radians);
    let lateral = if side == 0 { -10.0 } else { 10.0 };
    Vec2::new(
        fround(position.x + forward_x * forward + forward_y * lateral),
        fround(position.y + forward_y * forward - forward_x * lateral),
    )
}

fn nearest_target(
    position: Vec2,
    targets: &[NativeGolemKernelTarget],
) -> Option<NativeGolemKernelTarget> {
    let mut nearest = None;
    let mut nearest_distance = f64::INFINITY;
    for target in targets {
        let distance = squared_distance(position, target.position);
        if distance >= nearest_distance {
            continue;
        }
        nearest = Some(*target);
        nearest_distance = distance;
    }
    nearest
}

fn native_heading(origin: Vec2, target: Vec2) -> f64 {
    normalize_radians(atan2(target.x - origin.x, -(target.y - origin.y)))
}
fn normalize_radians(radians: f64) -> f64 {
    (radians % std::f64::consts::TAU + std::f64::consts::TAU) % std::f64::consts::TAU
}
fn squared_distance(left: Vec2, right: Vec2) -> f64 {
    let dx = left.x - right.x;
    let dy = left.y - right.y;
    dx * dx + dy * dy
}

fn active_result(
    actor: &mut NativeGolemKernelActor,
    contact: Option<NativeGolemContact>,
    provoke_started: bool,
    world: &mut impl NativeGolemWorld,
    rng: &mut NativeRng,
) -> NativeGolemKernelStepResult {
    let footstep = advance_articulation(actor, world, rng);
    actor.golem.circle_slow_ticks = actor.golem.circle_slow_ticks.saturating_sub(1);
    NativeGolemKernelStepResult { assembly_milestone: None, contact, footstep, provoke_started }
}

fn turn_golem(actor: &NativeGolemKernelActor, target: Vec2, attacking: bool) -> f64 {
    let target_degrees = native_heading(actor.position, target) * 180.0 / std::f64::consts::PI;
    let mut degrees = actor.rotation_radians * 180.0 / std::f64::consts::PI;
    let step = if actor.golem.circle_slow_ticks > 0 { 0.5 } else { 1.0 };
    for index in 0..if attacking { 5 } else { 1 } {
        let gap = (((target_degrees - degrees + 540.0) % 360.0) - 180.0).abs();
        if index > 0 && gap < 3.0 {
            break;
        }
        let direction = native_heading_turn_direction(degrees, target_degrees);
        if direction == 0 {
            break;
        }
        degrees = fround(degrees + f64::from(direction) * step);
    }
    normalize_radians(degrees * std::f64::consts::PI / 180.0)
}
