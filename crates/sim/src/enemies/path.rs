//! Enemy steering, flanking, short cached routes and stall recovery.
//!
//! The navigation boundary answers authored world geometry; this module owns route timing and
//! waypoint selection.

use crate::float;
use crate::float::{atan2, fround, hypot};
use crate::{Rng, Vec2};

/// A movement cadence chosen by visibility: whether the actor moves this tick, how many ticks
/// one step covers, and whether it is both on screen and lit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EnemyMovementClock {
    pub due: bool,
    pub cadence: u64,
    pub full: bool,
}
#[must_use]
pub fn enemy_movement_clock(
    actor_id: usize,
    tick: u64,
    visible: bool,
    illuminated: bool,
    enhanced: bool,
) -> EnemyMovementClock {
    let cadence = if !visible {
        15
    } else if !illuminated {
        10
    } else if enhanced {
        2
    } else {
        5
    };
    EnemyMovementClock {
        due: actor_id as u64 % cadence == tick % cadence,
        cadence,
        full: visible && illuminated,
    }
}

/// An enemy's retained steering state: turn rate, wander heading, flank, cached two-point
/// route, speed and turn factors, stall counter and reorientation timer.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct EnemyPathState {
    pub base_turn_rate: f64,
    pub flank_angle_deg: f64,
    pub flank_radius: f64,
    pub flank_ticks_remaining: u64,
    pub reorientation_ticks_remaining: u64,
    pub route_previous_vector: Option<Vec2>,
    pub route_refresh_ticks_remaining: u64,
    pub route_ticks_remaining: u64,
    pub route_waypoint_index: usize,
    pub route_waypoints: Option<[Vec2; 2]>,
    pub speed_factor: f64,
    pub stalled_movement_ticks: u64,
    pub turn_factor: f64,
    pub wander_heading_deg: f64,
}

impl EnemyPathState {
    /// A new path state, drawn in RNG order: wander heading, turn rate, flank magnitude, flank
    /// sign and flank radius.
    #[must_use]
    pub fn new(rng: &mut Rng) -> Self {
        let wander_heading_deg = rng.draw_scaled_float(360.0, false);
        let turn = rng.draw_scaled_float(0.5_f32 as f64, false);
        let flank_magnitude = rng.draw_scaled_float(90.0, false);
        let flank_angle_deg = rng.draw_sign(flank_magnitude);
        let flank_radius = rng.draw_scaled_float(100.0, false);
        Self {
            base_turn_rate: fround((0.5_f32 as f64) + turn),
            flank_angle_deg,
            flank_radius,
            flank_ticks_remaining: 0,
            reorientation_ticks_remaining: 0,
            route_previous_vector: None,
            route_refresh_ticks_remaining: 0,
            route_ticks_remaining: 0,
            route_waypoint_index: 0,
            route_waypoints: None,
            speed_factor: 1.0,
            stalled_movement_ticks: 0,
            turn_factor: 1.0,
            wander_heading_deg,
        }
    }

    pub fn clear_route(&mut self) {
        self.clear_route_with_refresh(0);
    }

    fn clear_route_with_refresh(&mut self, refresh: u64) {
        self.route_previous_vector = None;
        self.route_refresh_ticks_remaining = refresh;
        self.route_ticks_remaining = 0;
        self.route_waypoint_index = 0;
        self.route_waypoints = None;
    }

    /// Picks a new wander heading, which always consumes a draw. Only an enabled flank also
    /// draws its duration and raises the speed and turn factors.
    pub fn select_flank(&mut self, rng: &mut Rng, flanking_enabled: bool) {
        self.wander_heading_deg = rng.draw_scaled_float(360.0, false);
        if flanking_enabled {
            self.flank_ticks_remaining = 200 + rng.draw_integer(201) as u64;
            self.speed_factor = 1.5;
            self.turn_factor = 1.4_f32 as f64;
        }
    }

    /// While a reorientation lasts (state 0D), face the target immediately instead of turning
    /// toward it. Without a target the reorientation ends.
    pub fn step_reorientation(
        &mut self,
        heading: f64,
        position: Vec2,
        target: Option<Vec2>,
    ) -> f64 {
        if self.reorientation_ticks_remaining == 0 {
            return heading;
        }
        let Some(target) = target else {
            self.reorientation_ticks_remaining = 0;
            return heading;
        };
        self.reorientation_ticks_remaining -= 1;
        heading_to(position, target, heading)
    }

    /// Advances stall recovery by one movement epoch, whose stall counter increments once even
    /// when the epoch spans two simulation ticks. Returns whether a reorientation started.
    pub fn step_recovery(&mut self, rng: &mut Rng, request: PathRecoveryRequest) -> bool {
        assert!(
            request.status_factor.is_finite() && request.status_factor > 0.0,
            "enemy path recovery status factor must be positive and finite"
        );
        let stalled = request.requested_distance > 0.0
            && request.traveled_distance <= request.requested_distance * 0.25;
        let mut triggered_reorientation = false;
        if stalled {
            self.stalled_movement_ticks = (self.stalled_movement_ticks + 1).min(101);
            if self.stalled_movement_ticks == 25 {
                if rng.draw_integer(15) == 3 {
                    let duration = 50 + rng.draw_integer(50);
                    self.reorientation_ticks_remaining =
                        (f64::from(duration) / request.status_factor).round_ties_even() as u64;
                    triggered_reorientation = true;
                } else {
                    self.select_flank(rng, request.flanking_enabled);
                    self.stalled_movement_ticks = 0;
                }
            }
        } else if self.stalled_movement_ticks > 0 {
            self.stalled_movement_ticks -= 1;
        }
        if request.tick.is_multiple_of(20) && rng.draw_integer(150) == 23 {
            self.select_flank(rng, request.flanking_enabled);
        }
        if self.flank_ticks_remaining == 0 {
            self.speed_factor = (self.speed_factor * 0.995).max(1.0);
            self.turn_factor = (self.turn_factor * 0.995).max(1.0);
        }
        triggered_reorientation
    }
}

/// How steering moves along the line to the target: away from it, neither, or toward it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadialDirection {
    Retreat,
    Hold,
    Approach,
}

impl RadialDirection {
    const fn scalar(self) -> f64 {
        match self {
            Self::Retreat => -1.0,
            Self::Hold => 0.0,
            Self::Approach => 1.0,
        }
    }
}

/// Which way steering circles the target, if at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TangentDirection {
    Counterclockwise,
    Hold,
    Clockwise,
}

impl TangentDirection {
    const fn scalar(self) -> f64 {
        match self {
            Self::Counterclockwise => -1.0,
            Self::Hold => 0.0,
            Self::Clockwise => 1.0,
        }
    }
}

/// The inputs to one steering step: the actor's heading and position, cadence and speed, radial
/// and tangent directions, status factor, and the target or goal to steer by.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EnemySteeringRequest {
    pub actor_heading_deg: f64,
    pub actor_position: Vec2,
    pub cadence_ticks: u64,
    pub full_movement: bool,
    pub goal_position: Option<Vec2>,
    pub movement_per_tick: f64,
    pub radial_direction: RadialDirection,
    pub status_factor: f64,
    pub tangent_direction: TangentDirection,
    pub target_heading_deg: f64,
    pub target_position: Option<Vec2>,
}

/// The result of one steering step: the requested displacement, new heading and path state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EnemySteering {
    pub delta: Vec2,
    pub heading_deg: f64,
    pub state: EnemyPathState,
}

/// One movement epoch's progress for stall recovery: the requested and travelled distances,
/// status factor, tick and whether the enemy may flank.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PathRecoveryRequest {
    pub flanking_enabled: bool,
    pub requested_distance: f64,
    pub status_factor: f64,
    pub tick: u64,
    pub traveled_distance: f64,
}

/// The world supplies navigation facts. Route expiry and two-waypoint caching stay in this
/// module, so every world caller uses the same route lifecycle.
pub trait EnemyNavigation {
    fn direct_path_clear(&self, start: Vec2, end: Vec2, clearance: f64, body_radius: f64) -> bool;
    fn find_route(
        &self,
        start: Vec2,
        end: Vec2,
        clearance: f64,
        body_radius: f64,
    ) -> Option<Vec<Vec2>>;
}

/// The inputs to route resolution: the actor's position and body radius, the raw steering goal,
/// the target, navigation clearance, cadence and the route refresh delay.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EnemyPathGoalRequest {
    pub actor_position: Vec2,
    pub body_radius: f64,
    pub cadence_ticks: u64,
    pub navigation_clearance: f64,
    pub raw_goal: Vec2,
    pub target_position: Option<Vec2>,
    pub target_refresh_ticks: u64,
}

/// The goal to steer toward after route resolution, the updated path state, and whether a
/// failed route asks the actor to turn around.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EnemyPathGoal {
    pub goal: Vec2,
    pub state: EnemyPathState,
    pub turn_around: bool,
}

#[must_use]
pub fn enemy_target_refresh_ticks(mode: u8) -> u64 {
    match mode {
        0 => 1000,
        1 => 300,
        2 => 100,
        3 => 10,
        _ => panic!("enemy pathfinding mode must be within 0..3"),
    }
}

/// The point to steer toward: 10 units along the flee heading when fleeing, the supplied goal,
/// a long wander ray when no target exists, radial/tangential range control, target-relative
/// flanking, or a distance-scaled approach offset.
#[must_use]
pub fn enemy_steering_goal(state: &EnemyPathState, request: &EnemySteeringRequest) -> Vec2 {
    if request.radial_direction == RadialDirection::Retreat
        && request.tangent_direction == TangentDirection::Hold
    {
        let heading = if request.full_movement {
            request.target_position.map_or(request.actor_heading_deg, |target| {
                crate::heading::heading_degrees_from_vector(
                    request.actor_position.x - target.x,
                    request.actor_position.y - target.y,
                )
            })
        } else {
            request.actor_heading_deg
        };
        return offset_point(request.actor_position, heading, 10.0);
    }
    if let Some(goal) = request.goal_position {
        return goal;
    }
    let Some(target) = request.target_position else {
        return offset_point(request.actor_position, state.wander_heading_deg, 10_000.0);
    };
    if request.radial_direction != RadialDirection::Approach
        || request.tangent_direction != TangentDirection::Hold
    {
        let dx = target.x - request.actor_position.x;
        let dy = target.y - request.actor_position.y;
        let length = hypot(dx, dy);
        if length == 0.0 {
            return request.actor_position;
        }
        let unit_x = dx / length;
        let unit_y = dy / length;
        let radial = request.radial_direction.scalar();
        let tangent = request.tangent_direction.scalar();
        return Vec2::new(
            request.actor_position.x + (unit_x * radial - unit_y * tangent) * 10_000.0,
            request.actor_position.y + (unit_y * radial + unit_x * tangent) * 10_000.0,
        );
    }
    if state.flank_ticks_remaining > 0 {
        return offset_point(
            target,
            request.target_heading_deg + state.flank_angle_deg,
            state.flank_radius,
        );
    }
    let distance = hypot(target.x - request.actor_position.x, target.y - request.actor_position.y);
    offset_point(target, state.wander_heading_deg, (distance / 500.0).min(1.0) * 300.0)
}

/// Turns and moves one small step per cadence tick, rather than turning once and multiplying
/// the resulting displacement.
#[must_use]
pub fn build_enemy_steering(
    source: EnemyPathState,
    request: EnemySteeringRequest,
) -> EnemySteering {
    assert!(request.cadence_ticks > 0, "enemy steering cadence must be positive");
    let mut state = source;
    let mut heading_deg = request.actor_heading_deg;
    let mut delta = Vec2::ZERO;
    let fleeing = request.radial_direction == RadialDirection::Retreat
        && request.tangent_direction == TangentDirection::Hold;
    for _ in 0..request.cadence_ticks {
        let position =
            Vec2::new(request.actor_position.x + delta.x, request.actor_position.y + delta.y);
        let goal = enemy_steering_goal(
            &state,
            &EnemySteeringRequest {
                actor_heading_deg: heading_deg,
                actor_position: position,
                ..request
            },
        );
        let desired_heading = heading_to(position, goal, heading_deg);
        heading_deg = if fleeing {
            desired_heading
        } else {
            turn_toward_heading(
                heading_deg,
                desired_heading,
                source.base_turn_rate * source.turn_factor * request.status_factor,
            )
        };
        let radians = heading_deg * std::f64::consts::PI / 180.0;
        delta.x += float::sin(radians) * request.movement_per_tick;
        delta.y -= float::cos(radians) * request.movement_per_tick;
        if fleeing && request.full_movement && request.target_position.is_some() {
            state.wander_heading_deg = heading_deg;
        }
        if !fleeing {
            state.flank_ticks_remaining = state.flank_ticks_remaining.saturating_sub(1);
        }
    }
    EnemySteering { delta, heading_deg, state }
}

/// Reuses two cached route points until they are passed or expire, and re-paths only after
/// the configured refresh delay. A failed route asks the actor to turn.
#[must_use]
pub fn resolve_enemy_path_goal(
    mut state: EnemyPathState,
    request: EnemyPathGoalRequest,
    navigation: &(impl EnemyNavigation + ?Sized),
) -> EnemyPathGoal {
    if let Some(waypoints) = state.route_waypoints
        && state.route_ticks_remaining > 0
    {
        let waypoint = waypoints[state.route_waypoint_index];
        let current = vector_from_waypoint(request.actor_position, waypoint);
        let previous = state.route_previous_vector.unwrap_or(current);
        let dot = current.x * previous.x + current.y * previous.y;
        if dot > 100.0 {
            state.route_previous_vector = Some(current);
            state.route_refresh_ticks_remaining = 0;
            state.route_ticks_remaining =
                state.route_ticks_remaining.saturating_sub(request.cadence_ticks);
            return EnemyPathGoal { goal: waypoint, state, turn_around: false };
        }
        if state.route_waypoint_index == 0 {
            state.route_previous_vector =
                Some(vector_from_waypoint(request.actor_position, waypoints[1]));
            state.route_refresh_ticks_remaining = 0;
            state.route_ticks_remaining = request.target_refresh_ticks;
            state.route_waypoint_index = 1;
            return resolve_enemy_path_goal(state, request, navigation);
        }
    }
    state.route_refresh_ticks_remaining =
        state.route_refresh_ticks_remaining.saturating_sub(request.cadence_ticks);
    if state.route_refresh_ticks_remaining > 0 {
        return EnemyPathGoal { goal: request.raw_goal, state, turn_around: false };
    }
    let route_goal = request.target_position.unwrap_or(request.raw_goal);
    if navigation.direct_path_clear(
        request.actor_position,
        route_goal,
        request.navigation_clearance,
        request.body_radius,
    ) {
        state.clear_route_with_refresh(request.target_refresh_ticks);
        return EnemyPathGoal { goal: route_goal, state, turn_around: false };
    }
    let route = navigation.find_route(
        request.actor_position,
        route_goal,
        request.navigation_clearance,
        request.body_radius,
    );
    let Some(route) = route.filter(|route| route.len() >= 3) else {
        state.clear_route();
        return EnemyPathGoal { goal: route_goal, state, turn_around: true };
    };
    let first = route[1];
    state.flank_ticks_remaining = 0;
    state.route_previous_vector = Some(vector_from_waypoint(request.actor_position, first));
    state.route_refresh_ticks_remaining = 0;
    state.route_ticks_remaining = request.target_refresh_ticks;
    state.route_waypoint_index = 0;
    state.route_waypoints = Some([first, route[2]]);
    EnemyPathGoal { goal: first, state, turn_around: false }
}

fn vector_from_waypoint(position: Vec2, waypoint: Vec2) -> Vec2 {
    Vec2::new(position.x - waypoint.x, position.y - waypoint.y)
}

fn offset_point(point: Vec2, heading: f64, distance: f64) -> Vec2 {
    let radians = heading * std::f64::consts::PI / 180.0;
    Vec2::new(point.x + float::sin(radians) * distance, point.y - float::cos(radians) * distance)
}

fn heading_to(source: Vec2, target: Vec2, fallback: f64) -> f64 {
    let dx = target.x - source.x;
    let dy = target.y - source.y;
    if dx == 0.0 && dy == 0.0 {
        return fallback;
    }
    positive_degrees(atan2(dx, -dy) * 180.0 / std::f64::consts::PI)
}

fn turn_toward_heading(current: f64, target: f64, maximum_step: f64) -> f64 {
    let current = positive_degrees(current);
    let target = positive_degrees(target);
    let separation = (current - target).abs();
    if !(1.0..359.0).contains(&separation) {
        return current;
    }
    let direction = if target <= current {
        if current - target <= 180.0 { -1.0 } else { 1.0 }
    } else if target - current > 180.0 {
        -1.0
    } else {
        1.0
    };
    positive_degrees(current + direction * maximum_step)
}

fn positive_degrees(value: f64) -> f64 {
    ((value % 360.0) + 360.0) % 360.0
}
