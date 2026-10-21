//! Player and enemy movement kernels at the native 100 Hz fixed tick.
//!
//! `PlayerActor::tick_movement` is the native `MoveStep`: input accumulates at one tenth of a
//! unit per tick into a float32 velocity, the velocity is capped, the circle moves with the
//! alternate wall response, and the velocity decays by the ordinary or controlled damping. The
//! damping multiplies the un-narrowed accumulator when no cap applied, which reproduces the
//! native x87 extended-precision register path and the retail velocity tails.

use crate::collision::{
    CollisionRectangle, SecondaryResponse, Vec2, circle_placement_is_clear, move_circle,
    move_circle_with_enemy_substeps, resolve_circle_placement,
};
use crate::constants::{COLLISION_RESPONSE, ENEMY_MOVEMENT, PLAYER_MOVEMENT};
use crate::float::{fround, hypot};
use crate::rng::NativeRng;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerMovementState {
    /// Requested direction; magnitudes above one are normalized.
    pub intent: Vec2,
    /// Float32 velocity accumulator (actor `+0x158`), world units per tick.
    pub velocity: Vec2,
    pub transient_multiplier: f64,
    pub move_speed_scale: f64,
    pub progression_multiplier: f64,
    /// Movement scalar (actor `+0x218`).
    pub move_step_scale: f64,
    pub controlled_damping: bool,
}

impl PlayerMovementState {
    /// The retail defaults for a fresh wizard.
    #[must_use]
    pub const fn stock() -> Self {
        Self {
            intent: Vec2::ZERO,
            velocity: Vec2::ZERO,
            transient_multiplier: PLAYER_MOVEMENT.transient_multiplier,
            move_speed_scale: PLAYER_MOVEMENT.move_speed_scale,
            progression_multiplier: PLAYER_MOVEMENT.progression_multiplier,
            move_step_scale: PLAYER_MOVEMENT.move_step_scale,
            controlled_damping: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KnockbackState {
    pub origin: Vec2,
    pub remaining_distance: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerActor {
    pub position: Vec2,
    pub radius: f64,
    pub movement: PlayerMovementState,
    pub knockback: Option<KnockbackState>,
}

/// Normalizes directions longer than one unit and leaves shorter ones untouched.
#[must_use]
pub fn normalize_at_most_one(direction: Vec2) -> Vec2 {
    let magnitude = hypot(direction.x, direction.y);
    if magnitude <= 1.0 {
        direction
    } else {
        Vec2::new(direction.x / magnitude, direction.y / magnitude)
    }
}

/// The capped velocity, or `None` when the velocity was already within the cap.
fn cap_velocity(velocity: Vec2, cap: f64) -> Option<Vec2> {
    let magnitude = hypot(velocity.x, velocity.y);
    if magnitude <= cap {
        None
    } else {
        Some(Vec2::new(fround(velocity.x / magnitude * cap), fround(velocity.y / magnitude * cap)))
    }
}

impl PlayerActor {
    /// A wizard at `position` with the retail movement defaults and collision radius.
    #[must_use]
    pub const fn stock(position: Vec2) -> Self {
        Self {
            position,
            radius: COLLISION_RESPONSE.movement_fixture_player_radius,
            movement: PlayerMovementState::stock(),
            knockback: None,
        }
    }

    /// Native `MoveStep` for one fixed tick.
    pub fn tick_movement(&mut self, rectangles: &[CollisionRectangle]) {
        let movement = self.movement;
        let input = normalize_at_most_one(movement.intent);
        let accumulated_extended = Vec2::new(
            movement.velocity.x + input.x / PLAYER_MOVEMENT.input_divisor,
            movement.velocity.y + input.y / PLAYER_MOVEMENT.input_divisor,
        );
        let accumulated = Vec2::new(fround(accumulated_extended.x), fround(accumulated_extended.y));
        let cap = movement.transient_multiplier
            * movement.move_speed_scale
            * movement.progression_multiplier
            * PLAYER_MOVEMENT.global_cap_scale;
        let (velocity_before_damping, damping_source) = match cap_velocity(accumulated, cap) {
            Some(capped) => (capped, capped),
            None => (accumulated, accumulated_extended),
        };
        let length_squared = velocity_before_damping.x * velocity_before_damping.x
            + velocity_before_damping.y * velocity_before_damping.y;

        if length_squared > PLAYER_MOVEMENT.move_threshold_squared {
            self.position = move_circle(
                rectangles,
                self.position,
                self.radius,
                Vec2::new(
                    velocity_before_damping.x * movement.move_step_scale,
                    velocity_before_damping.y * movement.move_step_scale,
                ),
                SecondaryResponse::Alternate,
            );
        } else if !circle_placement_is_clear(rectangles, self.position, self.radius) {
            self.position = resolve_circle_placement(rectangles, self.position, self.radius);
        }

        let damping = if movement.controlled_damping {
            PLAYER_MOVEMENT.controlled_damping
        } else {
            PLAYER_MOVEMENT.ordinary_damping
        };
        self.movement.velocity =
            Vec2::new(fround(damping_source.x * damping), fround(damping_source.y * damping));
    }

    /// The retail Knockback actor's tick (`0x00600220`): up to ten units per tick radially
    /// away from the origin, followed by a separation pass at 60% of the radius.
    pub fn tick_knockback(&mut self, rectangles: &[CollisionRectangle]) {
        let Some(knockback) = self.knockback else {
            return;
        };
        if knockback.remaining_distance <= 0.0 {
            return;
        }
        let dx = self.position.x - knockback.origin.x;
        let dy = self.position.y - knockback.origin.y;
        let distance = hypot(dx, dy);
        assert!(distance != 0.0, "knockback has no radial direction from its origin");
        let step = knockback.remaining_distance.min(10.0);
        let moved = move_circle(
            rectangles,
            self.position,
            self.radius,
            Vec2::new(dx / distance * step, dy / distance * step),
            SecondaryResponse::Alternate,
        );
        let separated = move_circle(
            rectangles,
            moved,
            self.radius * COLLISION_RESPONSE.knockback_radius_scale,
            Vec2::ZERO,
            SecondaryResponse::Alternate,
        );
        let remaining_distance = fround(knockback.remaining_distance - step);
        self.position = separated;
        self.knockback = (remaining_distance > 0.0)
            .then_some(KnockbackState { origin: knockback.origin, remaining_distance });
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EnemyFamily {
    Imp,
    GreenImp,
    Zombie,
    Wraith,
    DemonSkull,
    DireFaculty,
    Spider,
    Skeleton,
    Demon,
    Coffin,
    Maggot,
    SkeletonArcher,
    SkeletonMage,
    Heartmonger,
}

/// The per-family base speed rolled when an enemy spawns. Skeleton-derived families share one
/// roll; Spider and Maggot roll their own.
pub fn create_enemy_base_speed(family: EnemyFamily, rng: &mut NativeRng) -> f64 {
    match family {
        EnemyFamily::Imp | EnemyFamily::GreenImp => 4.5,
        EnemyFamily::Zombie => fround(1.0 * 0.85),
        EnemyFamily::Wraith => 1.0,
        EnemyFamily::DemonSkull => 4.0,
        EnemyFamily::DireFaculty => 2.75,
        EnemyFamily::Spider => {
            let random = rng.draw_scaled_float(2.0, false);
            fround(3.0 + random)
        }
        EnemyFamily::Demon | EnemyFamily::Coffin => fround(1.0 * 0.75),
        EnemyFamily::Maggot => {
            let random = rng.draw_scaled_float(1.0, false);
            fround(1.0 + random)
        }
        EnemyFamily::Skeleton
        | EnemyFamily::SkeletonArcher
        | EnemyFamily::SkeletonMage
        | EnemyFamily::Heartmonger => {
            let random = rng.draw_scaled_float(1.0, false);
            let skeleton = fround(fround(1.25 + random) * 1.25 * 1.25);
            match family {
                EnemyFamily::Skeleton => skeleton,
                EnemyFamily::SkeletonArcher => fround(skeleton * 0.75),
                EnemyFamily::SkeletonMage => fround(fround(skeleton * 0.75) * 0.65),
                _ => fround(fround(skeleton * 0.65) * 0.75),
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct EnemyActor {
    pub position: Vec2,
    pub radius: f64,
    pub insertion_order: u64,
    pub movement_direction: Vec2,
    /// 2, 5, 10, or 15 ticks between moves.
    pub movement_cadence_ticks: u64,
    pub base_speed: f64,
    pub local_speed_factor: f64,
    pub shared_status_multiplier: f64,
}

impl EnemyActor {
    /// Moves on the ticks that match this enemy's insertion slot within its cadence.
    pub fn tick_movement(&mut self, global_tick: u64, rectangles: &[CollisionRectangle]) {
        let cadence = self.movement_cadence_ticks;
        if self.insertion_order % cadence != global_tick % cadence {
            return;
        }
        let direction_length = hypot(self.movement_direction.x, self.movement_direction.y);
        if direction_length == 0.0 {
            return;
        }
        let scale = ENEMY_MOVEMENT.direct_step_scale
            * self.base_speed
            * self.local_speed_factor
            * self.shared_status_multiplier
            * cadence as f64;
        self.position = move_circle_with_enemy_substeps(
            rectangles,
            self.position,
            self.radius,
            Vec2::new(
                self.movement_direction.x / direction_length * scale,
                self.movement_direction.y / direction_length * scale,
            ),
        );
        if self.local_speed_factor > ENEMY_MOVEMENT.local_factor_floor {
            self.local_speed_factor = ENEMY_MOVEMENT
                .local_factor_floor
                .max(fround(self.local_speed_factor * ENEMY_MOVEMENT.local_factor_decay));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_held_direction_settles_at_the_retail_walking_speed() {
        // v' = (v + 0.1) * 0.9 has the fixed point 0.9 units per tick, below the 1.1875 cap, so
        // ordinary walking never reaches the cap; only external impulses do.
        let mut actor = PlayerActor::stock(Vec2::new(1050.0, 1050.0));
        actor.movement.intent = Vec2::new(1.0, 0.0);
        for _ in 0..200 {
            actor.tick_movement(&[]);
        }
        let speed = hypot(actor.movement.velocity.x, actor.movement.velocity.y);
        assert!((speed - 0.9).abs() < 1e-5, "speed {speed} did not settle at 0.9");
        assert_eq!(actor.movement.velocity.y, 0.0);
        assert!(
            actor.position.x > 1050.0 + 150.0,
            "the wizard travelled {}",
            actor.position.x - 1050.0
        );
    }

    #[test]
    fn an_impulse_above_the_cap_is_clamped_before_it_moves_the_actor() {
        let mut actor = PlayerActor::stock(Vec2::new(0.0, 0.0));
        actor.movement.velocity = Vec2::new(3.0, 4.0);
        actor.tick_movement(&[]);
        let moved = hypot(actor.position.x, actor.position.y);
        assert!((moved - PLAYER_MOVEMENT.resulting_cap).abs() < 1e-6, "moved {moved}");
        let speed = hypot(actor.movement.velocity.x, actor.movement.velocity.y);
        assert!(
            (speed - PLAYER_MOVEMENT.resulting_cap * PLAYER_MOVEMENT.ordinary_damping).abs() < 1e-6,
            "speed {speed}"
        );
    }

    #[test]
    fn idle_velocity_decays_but_the_position_holds_under_the_threshold() {
        let mut actor = PlayerActor::stock(Vec2::new(100.0, 100.0));
        actor.movement.velocity = Vec2::new(0.05, 0.0);
        actor.tick_movement(&[]);
        assert_eq!(actor.position, Vec2::new(100.0, 100.0));
        assert_eq!(actor.movement.velocity.x, fround(0.05 * PLAYER_MOVEMENT.ordinary_damping));
    }

    #[test]
    fn knockback_travels_ten_units_per_tick_and_clears_itself() {
        let mut actor = PlayerActor::stock(Vec2::new(1050.0, 1050.0));
        actor.knockback =
            Some(KnockbackState { origin: Vec2::new(950.0, 1050.0), remaining_distance: 20.0 });
        actor.tick_knockback(&[]);
        assert_eq!(actor.position, Vec2::new(1060.0, 1050.0));
        assert!(actor.knockback.is_some());
        actor.tick_knockback(&[]);
        assert_eq!(actor.position, Vec2::new(1070.0, 1050.0));
        assert!(actor.knockback.is_none());
    }

    #[test]
    fn fixed_family_speeds_do_not_touch_the_rng() {
        let mut rng = NativeRng::new(3);
        let before = rng.clone();
        assert_eq!(create_enemy_base_speed(EnemyFamily::Imp, &mut rng), 4.5);
        assert_eq!(create_enemy_base_speed(EnemyFamily::Zombie, &mut rng), fround(0.85));
        assert_eq!(rng, before);
        let _ = create_enemy_base_speed(EnemyFamily::Spider, &mut rng);
        assert_ne!(rng, before);
    }
}
