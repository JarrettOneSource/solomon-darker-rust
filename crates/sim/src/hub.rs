//! Hub courtyard kernels: segment collision, the camera, and the wizard's walk.
//!
//! The courtyard is the first room of the web port. Its walls and props are line segments
//! (`segments.rs`), and the stock controller resolves a moving circle against them with a
//! two-pass bisection sweep. The wizard's walk is the Website's units-per-second form of the
//! retail-verified player kernel: the same 100 Hz tick, cap, retention, and `fround` points,
//! with velocity held in units per second and scaled by the tick length when it moves.
//!
//! Reference: `Website/frontend/src/game/core-kernels/hub-collision.ts`, `hub-math.ts`, and
//! `player-character.ts`.

pub mod ambient;
pub mod college_intro;
pub mod depth;
pub mod interaction;
pub mod memorial;
pub mod npc;
pub mod painter;
pub mod regions;
pub mod rooms;
mod segments;
pub mod teacher;
pub mod world;

pub use segments::HUB_COURTYARD_SEGMENTS;

use crate::collision::Vec2;
use crate::float::{fround, hypot};
use crate::heading::{
    advance_gait_degrees, advance_walk_cycle, heading_degrees_from_vector, heading_index,
};
use crate::spells::casting::PlayerPrimaryCastState;
use regions::{
    HUB_COLLEGE_INTRO_FADE_RATE, HUB_OUTGOING_FADE_RATE, HubRegion, HubTransition,
    HubTransitionPhase, begin_hub_transition, hub_incoming_fade_rate, hub_incoming_placement,
    hub_portal_touched_by_player, plan_hub_scripted_movement,
};

/// World extent of the courtyard room, in world units.
pub const HUB_WORLD_SIZE: Vec2 = Vec2::new(2000.0, 1024.0);
/// Zoom applied to the logical viewport while in the Hub.
pub const HUB_CAMERA_SCALE: f64 = 1.2;
/// Logical viewport the stock client letterboxes into the window.
pub const HUB_LOGICAL_VIEW: Vec2 = Vec2::new(1600.0, 900.0);
/// Where a wizard appears when entering the courtyard.
pub const HUB_SPAWN: Vec2 = Vec2::new(950.64, 164.04);
/// Collision radius of the wizard against courtyard segments.
pub const PLAYER_CHARACTER_RADIUS: f64 = 25.0;
/// Initial state of the courtyard's collision fallback sampler.
pub const NATIVE_HUB_COLLISION_RNG_SEED: u32 = 0x51a7_c011;

const NATIVE_SWEEP_FRACTION: f64 = 0.5;
const NATIVE_SWEEP_ITERATIONS: usize = 8;
const NATIVE_SWEEP_STOP_DISTANCE_SQUARED: f64 = 0.01;
const NATIVE_SURFACE_CLEARANCE: f64 = 0.1;

const INPUT_ACCELERATION: f64 = 10.0;
const MOVEMENT_LANE_CAP: f64 = 118.75;
const MOVEMENT_TICK_SECONDS: f64 = 0.01;
const MOVEMENT_RETENTION: f64 = 0.9;
const MOVEMENT_THRESHOLD_SQUARED: f64 = 0.01_f32 as f64;

/// A courtyard wall or prop edge.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HubSegment {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
}

impl HubSegment {
    #[must_use]
    pub const fn new(x1: f64, y1: f64, x2: f64, y2: f64) -> Self {
        Self { x1, y1, x2, y2 }
    }
}

/// Result of moving a circle against segments: where it ended and the sampler state after it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HubCollisionMove {
    pub position: Vec2,
    pub rng_state: u32,
}

#[must_use]
pub fn nearest_point_on_segment(point: Vec2, segment: &HubSegment) -> Vec2 {
    let segment_x = segment.x2 - segment.x1;
    let segment_y = segment.y2 - segment.y1;
    let length_squared = segment_x * segment_x + segment_y * segment_y;
    if length_squared == 0.0 {
        return Vec2::new(segment.x1, segment.y1);
    }
    let projection =
        ((point.x - segment.x1) * segment_x + (point.y - segment.y1) * segment_y) / length_squared;
    let clamped = projection.clamp(0.0, 1.0);
    Vec2::new(segment.x1 + segment_x * clamped, segment.y1 + segment_y * clamped)
}

#[must_use]
pub fn circle_overlaps_segment(position: Vec2, radius: f64, segment: &HubSegment) -> bool {
    let nearest = nearest_point_on_segment(position, segment);
    let dx = nearest.x - position.x;
    let dy = nearest.y - position.y;
    dx * dx + dy * dy < radius * radius
}

/// Index of the first segment, in list order, that the circle overlaps.
fn first_overlapping_segment(
    position: Vec2,
    radius: f64,
    segments: &[HubSegment],
    excluded: Option<usize>,
) -> Option<usize> {
    segments
        .iter()
        .enumerate()
        .find(|(index, segment)| {
            Some(*index) != excluded && circle_overlaps_segment(position, radius, segment)
        })
        .map(|(index, _)| index)
}

fn separate_from_segment(position: Vec2, radius: f64, segment: &HubSegment) -> Vec2 {
    let nearest = nearest_point_on_segment(position, segment);
    let dx = position.x - nearest.x;
    let dy = position.y - nearest.y;
    let distance_squared = dx * dx + dy * dy;
    if distance_squared >= radius * radius || distance_squared == 0.0 {
        return position;
    }
    let distance = distance_squared.sqrt();
    let separation = radius + NATIVE_SURFACE_CLEARANCE;
    Vec2::new(nearest.x + dx / distance * separation, nearest.y + dy / distance * separation)
}

/// The stock call site uses the game's shared additive RNG; the web port keeps this lane local
/// and deterministic (xorshift32) while preserving the recovered `[0, 1]` contract.
fn native_fallback_sample(state: u32) -> (u32, f64) {
    let mut value = state;
    value ^= value << 13;
    value ^= value >> 17;
    value ^= value << 5;
    let next = if value == 0 { 0x6d2b_79f5 } else { value };
    (next, f64::from(value) / f64::from(u32::MAX))
}

struct SweepResult {
    hit: Option<usize>,
    position: Vec2,
}

fn sweep_toward_destination(
    origin: Vec2,
    delta: Vec2,
    radius: f64,
    segments: &[HubSegment],
    slide_surface: Option<usize>,
) -> SweepResult {
    let mut current = origin;
    let mut target = Vec2::new(origin.x + delta.x, origin.y + delta.y);
    let mut accepted: Option<Vec2> = None;
    let mut hit: Option<usize> = None;

    for _ in 0..NATIVE_SWEEP_ITERATIONS {
        let remaining = Vec2::new(target.x - current.x, target.y - current.y);
        if remaining.x * remaining.x + remaining.y * remaining.y
            < NATIVE_SWEEP_STOP_DISTANCE_SQUARED
        {
            break;
        }
        let midpoint = Vec2::new(
            current.x + remaining.x * NATIVE_SWEEP_FRACTION,
            current.y + remaining.y * NATIVE_SWEEP_FRACTION,
        );
        let candidate = match slide_surface {
            Some(surface) => separate_from_segment(midpoint, radius, &segments[surface]),
            None => midpoint,
        };
        if let Some(collision) =
            first_overlapping_segment(candidate, radius, segments, slide_surface)
        {
            target = midpoint;
            hit = Some(collision);
            continue;
        }
        accepted = Some(candidate);
        current = midpoint;
    }

    SweepResult {
        hit,
        position: match (slide_surface, accepted) {
            (Some(_), Some(accepted)) => accepted,
            _ => origin,
        },
    }
}

/// Replays the stock controller's collision-triggered two-pass sweep for one small step.
fn sweep_step(
    origin: Vec2,
    delta: Vec2,
    radius: f64,
    rng_state: u32,
    segments: &[HubSegment],
) -> HubCollisionMove {
    let destination = Vec2::new(origin.x + delta.x, origin.y + delta.y);
    if first_overlapping_segment(destination, radius, segments, None).is_none() {
        return HubCollisionMove { position: destination, rng_state };
    }

    let first_pass = sweep_toward_destination(origin, delta, radius, segments, None);
    match first_pass.hit {
        None => {
            let (state, value) = native_fallback_sample(rng_state);
            HubCollisionMove {
                position: Vec2::new(
                    origin.x + delta.x * NATIVE_SWEEP_FRACTION * value,
                    origin.y + delta.y * NATIVE_SWEEP_FRACTION * value,
                ),
                rng_state: state,
            }
        }
        Some(hit) => HubCollisionMove {
            position: sweep_toward_destination(origin, delta, radius, segments, Some(hit)).position,
            rng_state,
        },
    }
}

/// Moves a circle by `delta` against `segments`, splitting the move into sub-steps no longer
/// than the radius so the sweep's small-step invariant holds for any caller.
#[must_use]
pub fn move_with_segments(
    position: Vec2,
    delta: Vec2,
    radius: f64,
    rng_state: u32,
    segments: &[HubSegment],
) -> HubCollisionMove {
    if delta.x == 0.0 && delta.y == 0.0 {
        return HubCollisionMove { position, rng_state };
    }
    let distance = hypot(delta.x, delta.y);
    let step_count = (distance / radius).ceil().max(1.0) as usize;
    let step = Vec2::new(delta.x / step_count as f64, delta.y / step_count as f64);
    let mut current = position;
    let mut state = rng_state;
    for _ in 0..step_count {
        let moved = sweep_step(current, step, radius, state, segments);
        current = moved.position;
        state = moved.rng_state;
    }
    HubCollisionMove { position: current, rng_state: state }
}

/// Moves a circle against the courtyard's own segments.
#[must_use]
pub fn move_with_hub_collision(
    position: Vec2,
    delta: Vec2,
    radius: f64,
    rng_state: u32,
) -> HubCollisionMove {
    move_with_segments(position, delta, radius, rng_state, &HUB_COURTYARD_SEGMENTS)
}

#[must_use]
pub fn is_traversable_against_segments(point: Vec2, radius: f64, segments: &[HubSegment]) -> bool {
    first_overlapping_segment(point, radius, segments, None).is_none()
}

#[must_use]
pub fn is_hub_traversable(point: Vec2, radius: f64) -> bool {
    is_traversable_against_segments(point, radius, &HUB_COURTYARD_SEGMENTS)
}

fn camera_axis_origin(position: f64, view_size: f64, world_size: f64) -> f64 {
    if world_size <= view_size {
        return (world_size - view_size) / 2.0;
    }
    let half_view = view_size / 2.0;
    position.clamp(half_view, world_size - half_view) - half_view
}

/// World-space origin of the courtyard camera that follows `position`.
///
/// The logical viewport is divided by the Hub zoom and clamped inside the room on each axis.
#[must_use]
pub fn hub_camera_origin(position: Vec2) -> Vec2 {
    hub_region_camera_origin(HubRegion::Courtyard, position)
}

/// `hubRegionCameraOrigin`: centres rooms narrower than the logical view, and clamps
/// larger rooms around the wizard.
#[must_use]
pub fn hub_region_camera_origin(region: HubRegion, position: Vec2) -> Vec2 {
    hub_region_camera_origin_for_view(region, position, HUB_LOGICAL_VIEW, HUB_CAMERA_SCALE)
}

/// The same regional clamp with the caller's FOV-adjusted projection.
#[must_use]
pub fn hub_region_camera_origin_for_view(
    region: HubRegion,
    position: Vec2,
    view: Vec2,
    scale: f64,
) -> Vec2 {
    let size = region.size();
    Vec2::new(
        camera_axis_origin(position.x, view.x / scale, size.x),
        camera_axis_origin(position.y, view.y / scale, size.y),
    )
}

/// One tick of planned wizard motion, before collision resolves the position.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HubWizardPlan {
    /// Scripted movement owns facing even while the wizard retains a cast action (`face`).
    pub face: bool,
    /// Requested displacement this tick; zero when the request is below the movement threshold.
    pub delta: Vec2,
    pub movement_active: bool,
    /// Velocity in units per second after input and the lane cap.
    pub requested_velocity: Vec2,
    /// Velocity carried into the next tick.
    pub retained_velocity: Vec2,
}

/// The Website's units-per-second player kernel (`planPlayerCharacterTick`).
///
/// `movement` is the raw input vector (any length; it is normalized) and `movement_scale`
/// scales both acceleration and the lane cap.
#[must_use]
pub fn plan_hub_wizard_tick(velocity: Vec2, movement: Vec2, movement_scale: f64) -> HubWizardPlan {
    let input_length = hypot(movement.x, movement.y);
    let direction = if input_length > 0.0 {
        Vec2::new(movement.x / input_length, movement.y / input_length)
    } else {
        Vec2::ZERO
    };
    let accumulated = Vec2::new(
        fround(velocity.x + direction.x * INPUT_ACCELERATION * movement_scale),
        fround(velocity.y + direction.y * INPUT_ACCELERATION * movement_scale),
    );
    let accumulated_length = hypot(accumulated.x, accumulated.y);
    let movement_lane_cap = MOVEMENT_LANE_CAP * movement_scale;
    let cap_scale = if accumulated_length > movement_lane_cap {
        movement_lane_cap / accumulated_length
    } else {
        1.0
    };
    let requested_velocity =
        Vec2::new(fround(accumulated.x * cap_scale), fround(accumulated.y * cap_scale));
    let requested_delta = Vec2::new(
        fround(requested_velocity.x * MOVEMENT_TICK_SECONDS),
        fround(requested_velocity.y * MOVEMENT_TICK_SECONDS),
    );
    let movement_active = requested_delta.x * requested_delta.x
        + requested_delta.y * requested_delta.y
        > MOVEMENT_THRESHOLD_SQUARED;
    HubWizardPlan {
        face: false,
        delta: if movement_active { requested_delta } else { Vec2::ZERO },
        movement_active,
        requested_velocity,
        retained_velocity: Vec2::new(
            fround(requested_velocity.x * MOVEMENT_RETENTION),
            fround(requested_velocity.y * MOVEMENT_RETENTION),
        ),
    }
}

/// The local Hub wizard, its current room and any active doorway walk.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct HubWizard {
    pub region: HubRegion,
    pub transition: Option<HubTransition>,
    pub position: Vec2,
    /// Retained velocity in units per second.
    pub velocity: Vec2,
    /// Native +0x6C angle; the sprite bank is derived after its float32 store.
    pub heading_degrees: f64,
    /// One of the 24 facing directions (0 = north, clockwise).
    pub heading_index: usize,
    pub primary_cast: PlayerPrimaryCastState,
    /// Last accepted native footstep epoch; the session compares final walk phases.
    pub footstep_tick: u64,
    /// Drives the head bob and the fixed robe sway.
    pub gait_degrees: f64,
    /// Walk animation phase; the robe pose is its integer part (0..5).
    pub walk_cycle_primary: f64,
}

impl HubWizard {
    /// A wizard standing at the courtyard spawn, facing south.
    #[must_use]
    pub fn spawn() -> Self {
        Self::at(HUB_SPAWN)
    }

    #[must_use]
    pub fn at(position: Vec2) -> Self {
        Self {
            region: HubRegion::Courtyard,
            transition: None,
            position,
            velocity: Vec2::ZERO,
            heading_degrees: 180.0,
            heading_index: heading_index(180.0),
            primary_cast: PlayerPrimaryCastState::default(),
            footstep_tick: 0,
            gait_degrees: 0.0,
            walk_cycle_primary: 0.0,
        }
    }

    /// `playerCharacterFacing`: continuous angle and body bank change together.
    pub fn set_facing(&mut self, heading: f64) {
        self.heading_degrees = fround(heading);
        self.heading_index = heading_index(self.heading_degrees);
    }

    /// Applies a plan whose displacement collision resolved to `resolved_position`
    /// (`commitPlayerCharacterTick`). Gait and walk cycle advance by the requested distance.
    pub fn commit(&mut self, plan: &HubWizardPlan, resolved_position: Vec2) {
        let requested_speed = hypot(plan.requested_velocity.x, plan.requested_velocity.y);
        let requested_distance = hypot(plan.delta.x, plan.delta.y);
        self.gait_degrees = advance_gait_degrees(self.gait_degrees, requested_distance);
        if plan.movement_active
            && requested_speed > 0.01
            && (plan.face || !self.primary_cast.owns_facing())
        {
            self.set_facing(heading_degrees_from_vector(
                plan.requested_velocity.x,
                plan.requested_velocity.y,
            ));
        }
        self.position = resolved_position;
        self.velocity = plan.retained_velocity;
        self.walk_cycle_primary = advance_walk_cycle(self.walk_cycle_primary, requested_distance);
    }

    /// Advances a participant tick: forced or input movement, regional collision, commit,
    /// then portal contact/fades (`hub-participant-movement.ts`). Doorway walks bypass
    /// segments; fixed room actors still separate the wizard.
    pub fn tick(&mut self, movement: Vec2, collision_rng: &mut u32) {
        let plan = self.movement_plan(movement);
        let bypass = self.transition.is_some();
        let position = if bypass {
            Vec2::new(self.position.x + plan.delta.x, self.position.y + plan.delta.y)
        } else {
            let moved = move_with_segments(
                self.position,
                plan.delta,
                PLAYER_CHARACTER_RADIUS,
                *collision_rng,
                self.region.segments(),
            );
            *collision_rng = moved.rng_state;
            moved.position
        };
        let position = rooms::separate_from_room_actors(self.region, position, bypass);
        self.commit(&plan, position);
        self.step_transition();
    }

    /// The input plan or the active transition's forced walk. External actor solvers may
    /// resolve and commit this plan, then call `step_transition` once afterwards.
    #[must_use]
    pub fn movement_plan(&self, movement: Vec2) -> HubWizardPlan {
        match self.transition {
            Some(transition) => {
                let target = if transition.phase == HubTransitionPhase::CollegeLoadout {
                    self.position
                } else {
                    transition.scripted_target
                };
                plan_hub_scripted_movement(self.position, target, transition.scripted_speed)
            }
            None => plan_hub_wizard_tick(self.velocity, movement, 1.0),
        }
    }

    /// `stepHubParticipantTransition`, after this tick's movement committed. Outgoing
    /// placement happens one tick after alpha reaches one; incoming completion happens
    /// one tick after both the fade and the walk have finished.
    pub fn step_transition(&mut self) {
        self.step_transition_for_college(false, false);
    }

    /// College admission shares the ordinary doorway epoch. Its Office walk may finish
    /// the incoming fade before reaching the doorway target, and leaving the Office holds
    /// behind the full cover until the player confirms a loadout.
    pub(crate) fn step_transition_for_college(
        &mut self,
        college_intro_pending: bool,
        office_walk: bool,
    ) {
        let Some(mut transition) = self.transition else {
            if let Some(portal) = hub_portal_touched_by_player(self.region, self.position) {
                self.transition = Some(begin_hub_transition(portal, self.position));
            }
            return;
        };
        match transition.phase {
            HubTransitionPhase::CollegeIntro => {
                transition.alpha = if transition.alpha <= HUB_COLLEGE_INTRO_FADE_RATE {
                    0.0
                } else {
                    transition.alpha - HUB_COLLEGE_INTRO_FADE_RATE
                };
                if transition.alpha == 0.0 {
                    self.transition = None;
                    return;
                }
            }
            HubTransitionPhase::CollegeLoadout => return,
            HubTransitionPhase::Outgoing => {
                if transition.alpha < 1.0 {
                    transition.alpha = (transition.alpha + HUB_OUTGOING_FADE_RATE).min(1.0);
                } else {
                    let incoming =
                        hub_incoming_placement(transition.source_region, transition.destination);
                    self.region = transition.destination;
                    self.position = incoming.position;
                    self.velocity = Vec2::ZERO;
                    transition.alpha = 1.0;
                    transition.phase = if college_intro_pending
                        && transition.source_region == HubRegion::Office
                        && transition.destination == HubRegion::Courtyard
                    {
                        HubTransitionPhase::CollegeLoadout
                    } else {
                        HubTransitionPhase::Incoming
                    };
                    transition.scripted_speed = incoming.scripted_speed;
                    transition.scripted_target = incoming.scripted_target;
                }
            }
            HubTransitionPhase::Incoming => {
                let dx = self.position.x - transition.scripted_target.x;
                let dy = self.position.y - transition.scripted_target.y;
                if transition.alpha == 0.0 && (dx * dx + dy * dy < 0.01 || office_walk) {
                    self.transition = None;
                    return;
                }
                transition.alpha =
                    (transition.alpha - hub_incoming_fade_rate(self.region)).max(0.0);
            }
        }
        self.transition = Some(transition);
    }

    /// Advances one tick against an explicit segment set (another room, or a test fixture).
    ///
    /// `collision_rng` is the room's shared fallback sampler state; it only advances when the
    /// sweep cannot find the blocking segment.
    pub fn tick_against(
        &mut self,
        movement: Vec2,
        collision_rng: &mut u32,
        segments: &[HubSegment],
    ) {
        let plan = plan_hub_wizard_tick(self.velocity, movement, 1.0);
        let moved = move_with_segments(
            self.position,
            plan.delta,
            PLAYER_CHARACTER_RADIUS,
            *collision_rng,
            segments,
        );
        *collision_rng = moved.rng_state;
        self.commit(&plan, moved.position);
    }

    /// Walk pose 0..5 selecting the dynamic robe frame.
    #[must_use]
    pub fn robe_pose(&self) -> usize {
        (self.walk_cycle_primary.trunc() as usize) % 5
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEED: u32 = NATIVE_HUB_COLLISION_RNG_SEED;
    const OPEN_FIELD: [HubSegment; 0] = [];
    const CORNER: [HubSegment; 2] =
        [HubSegment::new(0.0, 0.0, 2000.0, 0.0), HubSegment::new(0.0, 0.0, 0.0, 1100.0)];
    const START: Vec2 = Vec2::new(500.0, 300.0);

    fn walk(
        wizard: &mut HubWizard,
        rng: &mut u32,
        segments: &[HubSegment],
        movement: Vec2,
        ticks: usize,
    ) {
        for _ in 0..ticks {
            wizard.tick_against(movement, rng, segments);
            assert!(
                is_traversable_against_segments(wizard.position, PLAYER_CHARACTER_RADIUS, segments),
                "wizard entered a wall at {:?}",
                wizard.position
            );
        }
    }

    #[test]
    fn the_spawn_is_clear_and_faces_south() {
        let wizard = HubWizard::spawn();
        assert!(is_hub_traversable(wizard.position, PLAYER_CHARACTER_RADIUS));
        assert_eq!(wizard.heading_index, 12);
        assert_eq!(wizard.robe_pose(), 0);
    }

    #[test]
    fn a_held_direction_walks_at_the_retail_speed_and_animates() {
        let mut wizard = HubWizard::at(START);
        let mut rng = SEED;
        walk(&mut wizard, &mut rng, &OPEN_FIELD, Vec2::new(0.0, 1.0), 200);
        let speed = hypot(wizard.velocity.x, wizard.velocity.y);
        assert!((speed - 90.0).abs() < 1e-3, "steady speed {speed} units/s");
        assert!(wizard.position.y > START.y + 150.0, "moved to {:?}", wizard.position);
        assert_eq!(wizard.heading_index, 12);
        assert!(wizard.gait_degrees > 0.0);
        assert_eq!(rng, SEED, "open-field walking never samples the fallback");
    }

    #[test]
    fn turning_updates_the_heading_from_the_requested_velocity() {
        let mut wizard = HubWizard::at(START);
        let mut rng = SEED;
        walk(&mut wizard, &mut rng, &OPEN_FIELD, Vec2::new(1.0, 0.0), 30);
        assert_eq!(wizard.heading_index, 6, "east");
        walk(&mut wizard, &mut rng, &OPEN_FIELD, Vec2::new(-1.0, 0.0), 30);
        assert_eq!(wizard.heading_index, 18, "west");
        walk(&mut wizard, &mut rng, &OPEN_FIELD, Vec2::ZERO, 200);
        assert_eq!(wizard.heading_index, 18, "idle keeps the last facing");
        assert!(hypot(wizard.velocity.x, wizard.velocity.y) < 0.01, "velocity decays");
    }

    #[test]
    fn walls_stop_the_wizard_without_tunnelling() {
        let mut wizard = HubWizard::at(START);
        let mut rng = SEED;
        walk(&mut wizard, &mut rng, &CORNER, Vec2::new(0.0, -1.0), 600);
        assert!(wizard.position.y >= PLAYER_CHARACTER_RADIUS, "{:?}", wizard.position);
        assert!(wizard.position.y < PLAYER_CHARACTER_RADIUS + 2.0, "{:?}", wizard.position);
        assert_eq!(wizard.position.x, START.x, "a flat wall adds no lateral drift");
    }

    #[test]
    fn sliding_follows_a_diagonal_push_along_a_wall() {
        let mut wizard = HubWizard::at(START);
        let mut rng = SEED;
        walk(&mut wizard, &mut rng, &CORNER, Vec2::new(0.0, -1.0), 400);
        let pinned_x = wizard.position.x;
        walk(&mut wizard, &mut rng, &CORNER, Vec2::new(1.0, -1.0), 200);
        assert!(
            wizard.position.x > pinned_x + 50.0,
            "slid east along the wall: {:?}",
            wizard.position
        );
        assert!(wizard.position.y < PLAYER_CHARACTER_RADIUS + 2.0);
    }

    #[test]
    fn the_arch_blocks_a_straight_walk_south_from_the_spawn() {
        // The spawn sits inside the north arch; its south edge runs (914,248)-(1014,246).
        let mut wizard = HubWizard::spawn();
        let mut rng = SEED;
        walk(&mut wizard, &mut rng, &HUB_COURTYARD_SEGMENTS, Vec2::new(0.0, 1.0), 200);
        assert!(wizard.position.y > 215.0 && wizard.position.y < 223.5, "{:?}", wizard.position);
    }

    #[test]
    fn the_fallback_sampler_is_deterministic_and_never_returns_zero() {
        let (state, value) = native_fallback_sample(SEED);
        assert_eq!((state, value), native_fallback_sample(SEED));
        assert!((0.0..=1.0).contains(&value));
        assert_eq!(native_fallback_sample(0).0, 0x6d2b_79f5);
    }

    #[test]
    fn the_camera_clamps_inside_the_room() {
        let view =
            Vec2::new(HUB_LOGICAL_VIEW.x / HUB_CAMERA_SCALE, HUB_LOGICAL_VIEW.y / HUB_CAMERA_SCALE);
        let origin = hub_camera_origin(HUB_SPAWN);
        assert!((origin.x - (HUB_SPAWN.x - view.x / 2.0)).abs() < 1e-9);
        assert_eq!(origin.y, 0.0, "spawn is near the top edge");
        let corner = hub_camera_origin(Vec2::new(5000.0, 5000.0));
        assert!((corner.x - (HUB_WORLD_SIZE.x - view.x)).abs() < 1e-9);
        assert!((corner.y - (HUB_WORLD_SIZE.y - view.y)).abs() < 1e-9);
    }
}
