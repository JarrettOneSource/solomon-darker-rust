//! Native primary queries and Ether tracking from `core-kernels/primary-spell-targeting.ts`.
//!
//! Actor cell bindings and registration order are part of contact selection. In particular,
//! Fire and Ether query their post-move cell, rather than sweeping against every enemy.

use crate::Vec2;
use crate::boneyard::BoneyardBounds;
use crate::float;
use crate::float::{fround, hypot};
use crate::heading::heading_degrees_from_vector;
use crate::line_obstruction::line_bounds_exit_obstruction;

pub const AIR_PRIMARY_TARGET_Y_OFFSET: f64 = -20.0;
pub const NATIVE_PRIMARY_CAST_TERRAIN_EXCLUSION_MASK: u32 = 0x380;
pub const NATIVE_PRIMARY_FLIGHT_TERRAIN_EXCLUSION_MASK: u32 = 0x700;
pub const NATIVE_PRIMARY_HOSTILE_FLAG: u32 = 0x2;
pub const ETHER_PRIMARY_INITIAL_TURN: f64 = 0.01_f32 as f64;

/// `nativePrimaryViewRayEndpoint`: range ends at the current camera view, with
/// float32 stores before and after the shared bounds query.
#[must_use]
pub fn native_primary_view_ray_endpoint(
    start: Vec2,
    direction: Vec2,
    view: BoneyardBounds,
) -> Vec2 {
    let far = Vec2::new(
        fround(start.x + fround(direction.x * 100_000.0)),
        fround(start.y + fround(direction.y * 100_000.0)),
    );
    let endpoint = line_bounds_exit_obstruction(start, far, view).map_or(far, |hit| hit.point);
    Vec2::new(fround(endpoint.x), fround(endpoint.y))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrimarySpellTargetKind {
    Enemy,
    Gravestone,
    Projectile,
    Scenery,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrimarySpellQueryLane {
    Grid,
    Transient,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PrimarySpellTarget {
    pub active: bool,
    pub actor_flags: u32,
    pub attachment: Vec2,
    pub body_radius: f64,
    pub cell_binding_order: usize,
    pub heading_degrees: Option<f64>,
    /// A direct native position store can leave the Puppet bound to its old cell.
    pub grid_position: Option<Vec2>,
    pub id: String,
    pub kind: PrimarySpellTargetKind,
    pub native_priority: i32,
    pub pending_remove: bool,
    pub position: Vec2,
    pub query_lane: PrimarySpellQueryLane,
    pub registration_order: usize,
}

#[derive(Clone, Copy)]
pub struct PrimaryPointContactQuery<'a> {
    pub actor_mask: u32,
    pub position: Vec2,
    pub query_radius: f64,
    pub targets: &'a [PrimarySpellTarget],
}

pub struct FireballPointContactQuery<'a> {
    pub contact: PrimaryPointContactQuery<'a>,
    pub direction: Vec2,
    pub hostile_corridor_length: f64,
}

pub struct PrimaryConeQuery<'a> {
    pub actor_mask: u32,
    pub aim_direction: Vec2,
    pub half_angle_degrees: f64,
    pub has_line_of_sight: &'a dyn Fn(&PrimarySpellTarget) -> bool,
    pub origin: Vec2,
    pub reach: f64,
    pub targets: &'a [PrimarySpellTarget],
}

pub struct AirPrimaryTargetQuery<'a> {
    pub aim_direction: Vec2,
    pub has_line_of_sight: &'a dyn Fn(&PrimarySpellTarget) -> bool,
    pub maximum_range: f64,
    pub origin: Vec2,
    pub previous_target_id: Option<&'a str>,
    pub targets: &'a [PrimarySpellTarget],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AirPrimaryBoltGeometry {
    pub endpoint: Vec2,
    pub midpoint: Vec2,
    pub source: Vec2,
}

#[must_use]
pub fn air_primary_bolt_geometry(
    source: Vec2,
    aim_direction: Vec2,
    endpoint: Vec2,
) -> AirPrimaryBoltGeometry {
    let aim = normalized(aim_direction);
    let half_distance = hypot(endpoint.x - source.x, endpoint.y - source.y) * 0.5;
    AirPrimaryBoltGeometry {
        endpoint,
        midpoint: Vec2::new(source.x + aim.x * half_distance, source.y + aim.y * half_distance),
        source,
    }
}

#[must_use]
pub fn native_primary_target_eligible(target: &PrimarySpellTarget, actor_mask: u32) -> bool {
    target.active && !target.pending_remove && target.actor_flags & actor_mask != 0
}

#[must_use]
pub fn native_primary_cell_coordinate(position: f64) -> i64 {
    fround(position / 100.0).trunc() as i64
}

#[must_use]
pub fn first_native_primary_point_contact(
    query: PrimaryPointContactQuery<'_>,
) -> Option<&PrimarySpellTarget> {
    let cell_x = native_primary_cell_coordinate(query.position.x);
    let cell_y = native_primary_cell_coordinate(query.position.y);
    let mut selected: Option<&PrimarySpellTarget> = None;
    for target in query.targets {
        if selected.is_some_and(|previous| target.cell_binding_order >= previous.cell_binding_order)
        {
            continue;
        }
        if !native_primary_target_eligible(target, query.actor_mask) {
            continue;
        }
        if native_primary_cell_coordinate(target.position.x) != cell_x
            || native_primary_cell_coordinate(target.position.y) != cell_y
        {
            continue;
        }
        let radius = query.query_radius + target.body_radius;
        if squared_distance(target.position, query.position) < radius * radius {
            selected = Some(target);
        }
    }
    selected
}

#[must_use]
pub fn first_native_fireball_point_contact(
    query: FireballPointContactQuery<'_>,
) -> Option<&PrimarySpellTarget> {
    let contact = first_native_primary_point_contact(query.contact)?;
    if contact.actor_flags & NATIVE_PRIMARY_HOSTILE_FLAG != 0 {
        return Some(contact);
    }
    if squared_distance(contact.position, query.contact.position) < 2.0 {
        return Some(contact);
    }
    assert!(
        query.hostile_corridor_length.is_finite() && query.hostile_corridor_length > 0.0,
        "Fireball hostile corridor length must be positive and finite"
    );
    let direction = normalized(query.direction);
    let side = Vec2::new(fround(direction.y * 20.0), fround(-direction.x * 20.0));
    let position = query.contact.position;
    let far = Vec2::new(
        fround(position.x + direction.x * query.hostile_corridor_length),
        fround(position.y + direction.y * query.hostile_corridor_length),
    );
    let polygon = [
        Vec2::new(fround(position.x + side.x), fround(position.y + side.y)),
        Vec2::new(fround(position.x - side.x), fround(position.y - side.y)),
        Vec2::new(fround(far.x - side.x), fround(far.y - side.y)),
        Vec2::new(fround(far.x + side.x), fround(far.y + side.y)),
    ];
    let hostile_ahead = query.contact.targets.iter().any(|target| {
        native_primary_target_eligible(target, NATIVE_PRIMARY_HOSTILE_FLAG)
            && native_point_in_polygon(target.position, &polygon)
    });
    if hostile_ahead { None } else { Some(contact) }
}

#[must_use]
pub fn select_air_primary_target(query: AirPrimaryTargetQuery<'_>) -> Option<&PrimarySpellTarget> {
    let aim = normalized(query.aim_direction);
    let apex = directional_apex(query.origin, aim);
    let maximum_distance_squared = query.maximum_range * query.maximum_range;
    let cone_dot = float::cos(15.0 * std::f64::consts::PI / 180.0);
    let mut selected: Option<&PrimarySpellTarget> = None;
    let mut selected_distance_squared = f64::INFINITY;
    for target in native_broadphase_order(query.origin, query.maximum_range, query.targets) {
        if !native_primary_target_eligible(target, 0x6) {
            continue;
        }
        let distance_squared = squared_distance(target.position, query.origin);
        if distance_squared >= maximum_distance_squared || distance_squared == 0.0 {
            continue;
        }
        let angle_delta = difference(target.position, apex);
        let angle_distance_squared = squared_length(angle_delta);
        if angle_distance_squared == 0.0 {
            continue;
        }
        let inverse_angle_distance = 1.0 / angle_distance_squared.sqrt();
        if dot(angle_delta, aim) * inverse_angle_distance < cone_dot {
            continue;
        }
        if !(query.has_line_of_sight)(target) {
            continue;
        }
        if selected.is_none_or(|previous| {
            target.native_priority < previous.native_priority
                || (target.native_priority == previous.native_priority
                    && distance_squared < selected_distance_squared)
        }) {
            selected = Some(target);
            selected_distance_squared = distance_squared;
        }
    }
    if selected.is_some() {
        return selected;
    }
    let previous_id = query.previous_target_id?;
    let previous = query.targets.iter().find(|target| target.id == previous_id)?;
    if !native_primary_target_eligible(previous, 0x6) {
        return None;
    }
    let retained_direction = normalized(difference(previous.position, query.origin));
    if dot(aim, retained_direction) >= 0.71_f32 as f64 { Some(previous) } else { None }
}

#[must_use]
pub fn primary_spell_target_point(target: &PrimarySpellTarget) -> Vec2 {
    Vec2::new(
        target.position.x + target.attachment.x,
        target.position.y + target.attachment.y + AIR_PRIMARY_TARGET_Y_OFFSET,
    )
}

#[must_use]
pub fn select_ether_primary_target(
    origin: Vec2,
    aim_direction: Vec2,
    targets: &[PrimarySpellTarget],
) -> Option<&PrimarySpellTarget> {
    let aim = normalized(aim_direction);
    let probe = Vec2::new(origin.x + aim.x * 100.0, origin.y + aim.y * 100.0);
    select_ether_primary_target_at_point(probe, None, targets)
}

#[must_use]
pub fn select_ether_primary_target_at_point<'a>(
    origin: Vec2,
    excluded_target_id: Option<&str>,
    targets: &'a [PrimarySpellTarget],
) -> Option<&'a PrimarySpellTarget> {
    let mut ordered: Vec<_> = targets.iter().collect();
    ordered.sort_by_key(|target| target.registration_order);
    let mut selected = None;
    let mut selected_distance_squared = 999_999.0_f32 as f64;
    for target in ordered {
        // The native Ether constructor consults hostile flags and registration order here,
        // even for a target whose active/pending-remove fields changed since enrollment.
        if excluded_target_id == Some(target.id.as_str())
            || target.actor_flags & NATIVE_PRIMARY_HOSTILE_FLAG == 0
        {
            continue;
        }
        let distance_squared = squared_distance(target.position, origin);
        if distance_squared < selected_distance_squared {
            selected = Some(target);
            selected_distance_squared = distance_squared;
        }
    }
    selected
}

/// Native root rectangle: inclusive lower edges, exclusive upper edges.
#[must_use]
pub fn native_primary_rectangle_targets(
    origin: Vec2,
    width: f64,
    height: f64,
    actor_mask: u32,
    targets: &[PrimarySpellTarget],
) -> Vec<&PrimarySpellTarget> {
    let minimum = Vec2::new(fround(origin.x - width * 0.5), fround(origin.y - height * 0.5));
    let maximum = Vec2::new(fround(minimum.x + width), fround(minimum.y + height));
    native_cell_range_order(minimum, maximum, targets)
        .into_iter()
        .filter(|target| {
            native_primary_target_eligible(target, actor_mask)
                && target.position.x >= minimum.x
                && target.position.x < maximum.x
                && target.position.y >= minimum.y
                && target.position.y < maximum.y
        })
        .collect()
}

#[must_use]
pub fn native_primary_root_targets(
    origin: Vec2,
    reach: f64,
    actor_mask: u32,
    targets: &[PrimarySpellTarget],
) -> Vec<&PrimarySpellTarget> {
    let reach_squared = reach * reach;
    native_broadphase_order(origin, reach, targets)
        .into_iter()
        .filter(|target| {
            native_primary_target_eligible(target, actor_mask)
                && squared_distance(target.position, origin) < reach_squared
        })
        .collect()
}

/// `nativePrimaryCircleTargets`: native cell order, then the exact squared-radius
/// contact test. Adding the radii first admits targets the Source excludes.
#[must_use]
pub fn native_primary_circle_targets(
    origin: Vec2,
    radius: f64,
    actor_mask: u32,
    targets: &[PrimarySpellTarget],
) -> Vec<&PrimarySpellTarget> {
    let radius_squared = radius * radius;
    let mut selected = Vec::new();
    for target in native_broadphase_order(origin, radius, targets) {
        if native_primary_target_eligible(target, actor_mask)
            && squared_distance(target.position, origin)
                < radius_squared + target.body_radius * target.body_radius
        {
            selected.push(target);
        }
    }
    selected
}

#[must_use]
pub fn native_primary_cone_targets(query: PrimaryConeQuery<'_>) -> Vec<&PrimarySpellTarget> {
    let aim = normalized(query.aim_direction);
    let apex = directional_apex(query.origin, aim);
    let reach_squared = query.reach * query.reach;
    let cone_dot = float::cos(query.half_angle_degrees * std::f64::consts::PI / 180.0);
    let mut selected = Vec::new();
    for target in native_broadphase_order(query.origin, query.reach, query.targets) {
        if !native_primary_target_eligible(target, query.actor_mask) {
            continue;
        }
        let distance_squared = squared_distance(target.position, query.origin);
        if distance_squared == 0.0 || distance_squared >= reach_squared {
            continue;
        }
        let angle_delta = difference(target.position, apex);
        let angle_distance_squared = squared_length(angle_delta);
        if angle_distance_squared == 0.0
            || dot(angle_delta, aim) / angle_distance_squared.sqrt() < cone_dot
        {
            continue;
        }
        if (query.has_line_of_sight)(target) {
            selected.push(target);
        }
    }
    selected
}

#[must_use]
pub fn native_primary_polygon_targets<'a>(
    actor_mask: u32,
    polygon: &[Vec2],
    targets: &'a [PrimarySpellTarget],
) -> Vec<&'a PrimarySpellTarget> {
    if polygon.len() < 3 {
        return Vec::new();
    }
    let mut minimum = Vec2::new(f64::INFINITY, f64::INFINITY);
    let mut maximum = Vec2::new(f64::NEG_INFINITY, f64::NEG_INFINITY);
    for point in polygon {
        minimum.x = minimum.x.min(point.x);
        minimum.y = minimum.y.min(point.y);
        maximum.x = maximum.x.max(point.x);
        maximum.y = maximum.y.max(point.y);
    }
    let mut selected = Vec::new();
    let minimum = Vec2::new(fround(minimum.x - 25.0), fround(minimum.y - 25.0));
    let maximum = Vec2::new(fround(maximum.x + 25.0), fround(maximum.y + 25.0));
    for target in native_cell_range_order(minimum, maximum, targets) {
        if native_primary_target_eligible(target, actor_mask)
            && native_point_in_polygon(target.position, polygon)
        {
            selected.push(target);
        }
    }
    selected
}

#[must_use]
pub fn native_point_in_polygon(point: Vec2, polygon: &[Vec2]) -> bool {
    let Some(mut previous) = polygon.last() else {
        return false;
    };
    let mut inside = false;
    for current in polygon {
        if (point.y < current.y) != (point.y < previous.y)
            && point.x
                < (previous.x - current.x) * (point.y - current.y) / (previous.y - current.y)
                    + current.x
        {
            inside = !inside;
        }
        previous = current;
    }
    inside
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EtherPrimaryHomingInput {
    pub heading_degrees: f64,
    pub movement_scalar: f64,
    pub position: Vec2,
    pub speed: f64,
    pub target_position: Option<Vec2>,
    pub turn_accumulator: f64,
    pub turn_input: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EtherPrimaryHomingResult {
    pub direction: Vec2,
    pub heading_degrees: f64,
    pub position: Vec2,
    pub turn_accumulator: f64,
}

pub struct EtherPrimaryTrackingInput<'a> {
    pub heading_degrees: f64,
    pub movement_scalar: f64,
    pub position: Vec2,
    pub reacquires_target: bool,
    pub speed: f64,
    pub target_id: Option<&'a str>,
    pub targets: &'a [PrimarySpellTarget],
    pub turn_accumulator: f64,
    pub turn_input: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EtherPrimaryTrackingResult {
    pub movement: EtherPrimaryHomingResult,
    pub reacquires_target: bool,
    pub target_id: Option<String>,
}

#[must_use]
pub fn advance_ether_primary_homing(input: EtherPrimaryHomingInput) -> EtherPrimaryHomingResult {
    let heading = normalize_degrees(fround(input.heading_degrees));
    let old_direction = direction_from_heading(heading);
    let distance = fround(input.movement_scalar * input.speed);
    let position = Vec2::new(
        fround(input.position.x + old_direction.x * distance),
        fround(input.position.y + old_direction.y * distance),
    );
    let turn_accumulator = fround(input.turn_accumulator);
    let Some(target) = input.target_position else {
        return EtherPrimaryHomingResult {
            direction: old_direction,
            heading_degrees: heading,
            position,
            turn_accumulator,
        };
    };
    // Native movement uses the old heading; steering changes the following tick's direction.
    let desired = heading_degrees_from_vector(target.x - position.x, target.y - position.y);
    let turn_direction = native_heading_turn_direction(heading, desired);
    let turn = fround(
        input.turn_input * turn_accumulator * input.movement_scalar * f64::from(turn_direction),
    );
    let heading_degrees = normalize_degrees(fround(heading + turn));
    let step = if turn_accumulator > 1.0 { 0.002_f32 as f64 } else { 0.05_f32 as f64 };
    EtherPrimaryHomingResult {
        direction: direction_from_heading(heading_degrees),
        heading_degrees,
        position,
        turn_accumulator: fround(turn_accumulator + step).min(10.0),
    }
}

#[must_use]
pub fn advance_ether_primary_tracking(
    input: EtherPrimaryTrackingInput<'_>,
) -> EtherPrimaryTrackingResult {
    let target = input.target_id.and_then(|id| input.targets.iter().find(|target| target.id == id));
    let movement = advance_ether_primary_homing(EtherPrimaryHomingInput {
        heading_degrees: input.heading_degrees,
        movement_scalar: input.movement_scalar,
        position: input.position,
        speed: input.speed,
        target_position: target.map(|target| target.position),
        turn_accumulator: input.turn_accumulator,
        turn_input: input.turn_input,
    });
    if let Some(target) = target {
        return EtherPrimaryTrackingResult {
            movement,
            reacquires_target: input.reacquires_target,
            target_id: if target.active { Some(target.id.clone()) } else { None },
        };
    }
    if input.target_id.is_some() && input.reacquires_target {
        let replacement =
            select_ether_primary_target_at_point(movement.position, None, input.targets);
        return EtherPrimaryTrackingResult {
            movement,
            reacquires_target: replacement.is_some(),
            target_id: replacement.map(|target| target.id.clone()),
        };
    }
    EtherPrimaryTrackingResult {
        movement,
        reacquires_target: input.reacquires_target,
        target_id: None,
    }
}

#[must_use]
pub fn native_missile_fan_heading(aim_heading: f64, quantity: usize, index: usize) -> f64 {
    assert!(quantity > 0 && index < quantity, "missile index is outside its fan");
    let step = if quantity < 4 { 30.0 } else { 20.0 };
    let base = aim_heading + if quantity.is_multiple_of(2) { step / 2.0 } else { 0.0 };
    let tier = index.div_ceil(2) as f64;
    let sign = if index.is_multiple_of(2) { 1.0 } else { -1.0 };
    fround(normalize_degrees(base + sign * tier * step))
}

#[must_use]
pub fn native_missile_fan_turn_scale(index: usize) -> f64 {
    0.75_f64.powi(index.div_ceil(2) as i32)
}

#[must_use]
pub fn native_heading_turn_direction(current: f64, desired: f64) -> i32 {
    let current = normalize_degrees(current);
    let desired = normalize_degrees(desired);
    let gap = (current - desired).abs();
    if gap <= 1.0 || gap >= 359.0 {
        return 0;
    }
    if desired <= current {
        if current - desired <= 180.0 { -1 } else { 1 }
    } else if desired - current > 180.0 {
        -1
    } else {
        1
    }
}

#[must_use]
pub fn direction_from_heading(heading_degrees: f64) -> Vec2 {
    let radians = heading_degrees * std::f64::consts::PI / 180.0;
    Vec2::new(float::sin(radians), -float::cos(radians))
}

fn normalize_degrees(degrees: f64) -> f64 {
    ((degrees % 360.0) + 360.0) % 360.0
}

fn native_broadphase_order(
    origin: Vec2,
    reach: f64,
    targets: &[PrimarySpellTarget],
) -> Vec<&PrimarySpellTarget> {
    let minimum = Vec2::new(fround(origin.x - reach), fround(origin.y - reach));
    let diameter = fround(reach + reach);
    let maximum = Vec2::new(fround(minimum.x + diameter), fround(minimum.y + diameter));
    native_cell_range_order(minimum, maximum, targets)
}

fn native_cell_range_order(
    minimum: Vec2,
    maximum: Vec2,
    targets: &[PrimarySpellTarget],
) -> Vec<&PrimarySpellTarget> {
    let min_cell_x = native_primary_cell_coordinate(minimum.x);
    let min_cell_y = native_primary_cell_coordinate(minimum.y);
    let max_cell_x = native_primary_cell_coordinate(maximum.x);
    let max_cell_y = native_primary_cell_coordinate(maximum.y);
    let mut grid = Vec::new();
    let mut transient = Vec::new();
    for target in targets {
        if target.query_lane == PrimarySpellQueryLane::Transient {
            if target.actor_flags & 0x180 != 0 {
                transient.push(target);
            }
            continue;
        }
        let bound = target.grid_position.unwrap_or(target.position);
        let x = native_primary_cell_coordinate(bound.x);
        let y = native_primary_cell_coordinate(bound.y);
        if x >= min_cell_x && x <= max_cell_x && y >= min_cell_y && y <= max_cell_y {
            grid.push(target);
        }
    }
    grid.sort_by_key(|target| {
        let bound = target.grid_position.unwrap_or(target.position);
        (
            native_primary_cell_coordinate(bound.x),
            native_primary_cell_coordinate(bound.y),
            target.cell_binding_order,
        )
    });
    transient.sort_by_key(|target| target.registration_order);
    grid.extend(transient);
    grid
}

fn directional_apex(origin: Vec2, direction: Vec2) -> Vec2 {
    Vec2::new(fround(origin.x - direction.x * 30.0), fround(origin.y - direction.y * 30.0))
}

fn normalized(vector: Vec2) -> Vec2 {
    let length = hypot(vector.x, vector.y);
    if length > 0.0 {
        Vec2::new(vector.x / length, vector.y / length)
    } else {
        Vec2::new(0.0, -1.0)
    }
}

fn difference(left: Vec2, right: Vec2) -> Vec2 {
    Vec2::new(left.x - right.x, left.y - right.y)
}

fn squared_distance(left: Vec2, right: Vec2) -> f64 {
    squared_length(difference(left, right))
}

fn squared_length(vector: Vec2) -> f64 {
    vector.x * vector.x + vector.y * vector.y
}

fn dot(left: Vec2, right: Vec2) -> f64 {
    left.x * right.x + left.y * right.y
}
