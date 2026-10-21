//! Collision-safe births from `core-server/boneyard-collision.ts`.
//!
//! Native policy retries own the steering stream. An accepted original position consumes no
//! draw, each searched ring consumes one angle, and light placement deliberately falls back
//! to the direct policy after its source radius threshold.

use crate::float;
use std::fmt;

use crate::enemies::SpawnPositionPolicy;
use crate::float::{fround, hypot};
use crate::random_domain::native_random_float_from_word;
use crate::{NativeRng, Vec2};

use super::BoneyardBounds;
use super::collision::{
    BoneyardCollisionWorld, can_place_boneyard_body, resolve_boneyard_movement,
};

const MOVEMENT_PROBE: f64 = 0.5;
const LIGHT_FALLBACK_RADIUS: f64 = 350.0;
const VERTICAL_SCALE: f64 = 0.8;

/// World-owned policy inputs. In live play `light_at` samples the actual light field and
/// `accepts_domain` checks navigation reachability; neither is inferred from placement.
pub trait SpawnPolicyContext {
    fn light_at(&self, position: Vec2) -> f64;

    fn accepts_domain(&self, _position: Vec2, _radius: f64) -> bool {
        true
    }

    fn is_offscreen(&self, _position: Vec2) -> bool {
        false
    }

    fn is_outside_policy_bounds(&self, _position: Vec2) -> bool {
        false
    }

    fn retry_bounds(&self) -> Option<BoneyardBounds> {
        None
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SpawnPlacement<'a> {
    pub position: Vec2,
    pub bounds: BoneyardBounds,
    pub collision: &'a BoneyardCollisionWorld,
    pub radius: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NoSpawnPlacement {
    pub position: Vec2,
    pub radius: f64,
    pub policy: Option<SpawnPositionPolicy>,
}

impl fmt::Display for NoSpawnPlacement {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "Boneyard has no collision-safe {:?} spawn for radius {} from ({}, {})",
            self.policy, self.radius, self.position.x, self.position.y
        )
    }
}

impl std::error::Error for NoSpawnPlacement {}

impl SpawnPlacement<'_> {
    /// The older source placement path uses a semantic angle and first clamps its search
    /// origin. Native enemy births use `resolve_native` below instead.
    pub fn resolve(self, starting_angle_degrees: Option<f64>) -> Result<Vec2, NoSpawnPlacement> {
        self.validate_radius();
        if self.is_mobile(self.position) {
            return Ok(self.position);
        }
        let origin = Vec2::new(
            self.position
                .x
                .max(self.bounds.x + self.radius)
                .min(self.bounds.x + self.bounds.width - self.radius),
            self.position
                .y
                .max(self.bounds.y + self.radius)
                .min(self.bounds.y + self.bounds.height - self.radius),
        );
        if self.is_mobile(origin) {
            return Ok(origin);
        }
        let angle = starting_angle_degrees
            .unwrap_or_else(|| semantic_spawn_angle(self.position, self.radius));
        let maximum_radius = hypot(self.bounds.width, self.bounds.height) + self.radius * 2.0;
        let mut ring_radius = self.radius;
        while ring_radius <= maximum_radius {
            let sample_count = native_spawn_ring_sample_count(ring_radius, self.radius);
            for index in 0..sample_count {
                let candidate = ring_candidate(origin, angle, index, sample_count, ring_radius);
                if self.is_mobile(candidate) {
                    return Ok(candidate);
                }
            }
            ring_radius += self.radius;
        }
        Err(self.failure(None))
    }

    pub fn resolve_native(
        self,
        policy: SpawnPositionPolicy,
        rng: &mut NativeRng,
        context: &impl SpawnPolicyContext,
    ) -> Result<Vec2, NoSpawnPlacement> {
        self.validate_radius();
        if self.policy_accepts(self.position, policy, context, false) {
            return Ok(self.position);
        }
        // Source draws are immutable until a placement is returned. Failed searches must
        // leave the caller's generator unchanged, just as a thrown source call does.
        let mut search_rng = rng.clone();
        let mut active_policy = policy;
        let mut ring_radius = self.radius;
        let maximum_radius = hypot(self.bounds.width, self.bounds.height) + self.radius * 2.0;
        while ring_radius <= maximum_radius {
            let angle = search_rng.draw_scaled_float(360.0, false);
            let sample_count = native_spawn_ring_sample_count(ring_radius, self.radius);
            for index in 0..sample_count {
                let candidate =
                    ring_candidate(self.position, angle, index, sample_count, ring_radius);
                if self.policy_accepts(candidate, active_policy, context, true) {
                    *rng = search_rng;
                    return Ok(candidate);
                }
            }
            ring_radius += self.radius;
            if active_policy == SpawnPositionPolicy::Light && ring_radius > LIGHT_FALLBACK_RADIUS {
                active_policy = SpawnPositionPolicy::Direct;
                ring_radius = self.radius * 2.0;
            }
        }
        Err(self.failure(Some(policy)))
    }

    fn is_mobile(self, position: Vec2) -> bool {
        if !can_place_boneyard_body(position, self.bounds, self.collision, self.radius) {
            return false;
        }
        for delta in [
            Vec2::new(MOVEMENT_PROBE, 0.0),
            Vec2::new(0.0, MOVEMENT_PROBE),
            Vec2::new(-MOVEMENT_PROBE, 0.0),
            Vec2::new(0.0, -MOVEMENT_PROBE),
        ] {
            let requested = Vec2::new(position.x + delta.x, position.y + delta.y);
            let resolved = resolve_boneyard_movement(
                position,
                requested,
                self.bounds,
                self.collision,
                self.radius,
            );
            if hypot(resolved.x - position.x, resolved.y - position.y) >= MOVEMENT_PROBE - 0.01 {
                return true;
            }
        }
        false
    }

    fn policy_accepts(
        self,
        position: Vec2,
        policy: SpawnPositionPolicy,
        context: &impl SpawnPolicyContext,
        retry: bool,
    ) -> bool {
        if !self.is_mobile(position) || !context.accepts_domain(position, self.radius) {
            return false;
        }
        if retry
            && policy != SpawnPositionPolicy::Dark
            && context
                .retry_bounds()
                .is_some_and(|bounds| !circle_inside_bounds(position, bounds, self.radius))
        {
            return false;
        }
        match policy {
            SpawnPositionPolicy::Dark => context.light_at(position) <= 0.0,
            SpawnPositionPolicy::Light => context.light_at(position) > 0.0,
            SpawnPositionPolicy::Offscreen => context.is_offscreen(position),
            SpawnPositionPolicy::Edge => context.is_outside_policy_bounds(position),
            SpawnPositionPolicy::Direct => true,
        }
    }

    fn validate_radius(self) {
        assert!(
            self.radius.is_finite() && self.radius > 0.0,
            "Boneyard spawn radius must be positive and finite"
        );
    }

    fn failure(self, policy: Option<SpawnPositionPolicy>) -> NoSpawnPlacement {
        NoSpawnPlacement { position: self.position, radius: self.radius, policy }
    }
}

#[must_use]
pub fn native_spawn_ring_sample_count(ring_radius: f64, radius_step: f64) -> usize {
    (std::f64::consts::PI * (ring_radius + radius_step) / ring_radius).trunc().max(1.0) as usize
}

/// Every participant's stock view must exclude the point. Source policy cameras are fixed
/// at 1600×900 and 1.35 zoom, independent of a user's actual viewport and rendering scale.
#[must_use]
pub fn boneyard_spawn_position_is_offscreen(
    position: Vec2,
    bounds: BoneyardBounds,
    focuses: &[Vec2],
) -> bool {
    assert!(!focuses.is_empty(), "Boneyard offscreen placement requires a camera focus");
    let half_width = 1600.0 / 1.35 / 2.0;
    let half_height = 900.0 / 1.35 / 2.0;
    focuses.iter().all(|focus| {
        let camera_x = clamp_camera_axis(focus.x, bounds.x, bounds.width, half_width);
        let camera_y = clamp_camera_axis(focus.y, bounds.y, bounds.height, half_height);
        position.x < camera_x - half_width
            || position.x > camera_x + half_width
            || position.y < camera_y - half_height
            || position.y > camera_y + half_height
    })
}

fn ring_candidate(origin: Vec2, angle: f64, index: usize, count: usize, radius: f64) -> Vec2 {
    let radians = (angle + index as f64 * (360.0 / count as f64)) * std::f64::consts::PI / 180.0;
    Vec2::new(
        fround(origin.x + float::sin(radians) * radius),
        fround(origin.y - float::cos(radians) * radius * VERTICAL_SCALE),
    )
}

fn semantic_spawn_angle(position: Vec2, radius: f64) -> f64 {
    let mut hash = 0x811c_9dc5_u32;
    for value in [position.x, position.y, radius] {
        hash = (hash ^ (value as f32).to_bits()).wrapping_mul(0x0100_0193);
    }
    native_random_float_from_word(hash, 360.0)
}

fn circle_inside_bounds(position: Vec2, bounds: BoneyardBounds, radius: f64) -> bool {
    position.x >= bounds.x + radius
        && position.x <= bounds.x + bounds.width - radius
        && position.y >= bounds.y + radius
        && position.y <= bounds.y + bounds.height - radius
}

fn clamp_camera_axis(position: f64, start: f64, size: f64, half_view: f64) -> f64 {
    if size <= half_view * 2.0 {
        start + size / 2.0
    } else {
        position.max(start + half_view).min(start + size - half_view)
    }
}
