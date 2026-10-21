//! Swinging fence gates, ported from `core-kernels/boneyard-gate.ts`.

use crate::Vec2;
use crate::float::{atan2, hypot};
use crate::rng::{next_xorshift_random, seed_from_text};

use super::BoneyardFence;
use super::collision::closest_point_on_segment;

const HINGE_TRIM: f64 = 13.5;
const CENTER_GAP: f64 = 1.0;
pub const INITIAL_SWAY: f64 = 20.0;
const CONTACT_SPEED: f64 = 2.0;
const CONTACT_DAMPING: f64 = 0.96;
const BOUNDARY_DAMPING: f64 = 0.98;
const IDLE_DAMPING: f64 = 0.999;
const BOUNDARY_BOUNCE: f64 = -0.5;
const MAX_TRAVEL_DEGREES: f64 = 60.0;
const STOP_SPEED_SQUARED: f64 = 0.001;

/// One of a gate's two independently swinging leaves.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct GateLeaf {
    pub damping: f64,
    pub fence_eid: String,
    pub hinge: Vec2,
    pub id: String,
    pub length: f64,
    pub rest_angle_degrees: f64,
    pub side: u8,
    pub tip: Vec2,
    pub velocity: Vec2,
}

impl GateLeaf {
    /// A requested player movement touches the leaf before the leaf advances this tick.
    pub fn apply_contact(&mut self, direction: Vec2) {
        let length = hypot(direction.x, direction.y);
        if length <= 0.000001 {
            return;
        }
        self.damping = CONTACT_DAMPING;
        self.velocity =
            Vec2::new(direction.x / length * CONTACT_SPEED, direction.y / length * CONTACT_SPEED);
    }

    /// `stepBoneyardGateLeaf`: retain hinge distance, bounce outside travel, damp velocity.
    pub fn step(&mut self) {
        let speed_squared = self.velocity.x * self.velocity.x + self.velocity.y * self.velocity.y;
        if speed_squared <= STOP_SPEED_SQUARED {
            self.damping = IDLE_DAMPING;
            self.velocity = Vec2::ZERO;
            return;
        }
        let candidate = point_at_length(
            self.hinge,
            Vec2::new(self.tip.x + self.velocity.x, self.tip.y + self.velocity.y),
            self.length,
        );
        let angle_delta =
            ((angle_degrees(self.hinge, candidate) - self.rest_angle_degrees + 540.0) % 360.0)
                - 180.0;
        if angle_delta.abs() > MAX_TRAVEL_DEGREES {
            self.damping = BOUNDARY_DAMPING;
            self.velocity.x *= BOUNDARY_BOUNCE;
            self.velocity.y *= BOUNDARY_BOUNCE;
        } else {
            self.tip = candidate;
        }
        self.velocity.x *= self.damping;
        self.velocity.y *= self.damping;
    }

    #[must_use]
    pub fn touches(&self, center: Vec2, radius: f64) -> bool {
        let nearest = closest_point_on_segment(center, self.hinge, self.tip);
        let dx = center.x - nearest.x;
        let dy = center.y - nearest.y;
        dx * dx + dy * dy < radius * radius
    }
}

/// The seed owns the same private FNV/xorshift stream as the Website's gate constructor.
#[must_use]
pub fn create_gate_leaves(fences: &[BoneyardFence], seed: &str) -> Vec<GateLeaf> {
    let mut random_state = seed_from_text(seed);
    let mut leaves = Vec::new();
    for fence in fences {
        if fence.code() != 2 {
            continue;
        }
        let roots = native_closed_gate_roots(&fence.points);
        for (side, (hinge, rest_tip)) in roots.into_iter().enumerate() {
            let magnitude = next_xorshift_random(&mut random_state);
            let sign = if next_xorshift_random(&mut random_state) < 0.5 { -1.0 } else { 1.0 };
            let y_displacement = magnitude * INITIAL_SWAY * sign;
            let length = hypot(rest_tip.x - hinge.x, rest_tip.y - hinge.y);
            leaves.push(GateLeaf {
                damping: IDLE_DAMPING,
                fence_eid: fence.eid.clone(),
                hinge,
                id: format!("{}:gate:{side}", fence.eid),
                length,
                rest_angle_degrees: angle_degrees(hinge, rest_tip),
                side: side as u8,
                tip: point_at_length(
                    hinge,
                    Vec2::new(rest_tip.x, rest_tip.y + y_displacement),
                    length,
                ),
                velocity: Vec2::ZERO,
            });
        }
    }
    leaves
}

/// The same closed roots admit both live leaves and static painter registrations.
#[must_use]
pub fn native_closed_gate_roots(points: &[Vec2]) -> Vec<(Vec2, Vec2)> {
    let [start, end, ..] = points else { return Vec::new() };
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let length = hypot(dx, dy);
    if length <= (HINGE_TRIM + CENTER_GAP) * 2.0 {
        return Vec::new();
    }
    let ux = dx / length;
    let uy = dy / length;
    let midpoint = Vec2::new((start.x + end.x) / 2.0, (start.y + end.y) / 2.0);
    vec![
        (
            Vec2::new(end.x - ux * HINGE_TRIM, end.y - uy * HINGE_TRIM),
            Vec2::new(midpoint.x + ux * CENTER_GAP, midpoint.y + uy * CENTER_GAP),
        ),
        (
            Vec2::new(start.x + ux * HINGE_TRIM, start.y + uy * HINGE_TRIM),
            Vec2::new(midpoint.x - ux * CENTER_GAP, midpoint.y - uy * CENTER_GAP),
        ),
    ]
}

fn point_at_length(origin: Vec2, target: Vec2, length: f64) -> Vec2 {
    let dx = target.x - origin.x;
    let dy = target.y - origin.y;
    let source_length = hypot(dx, dy);
    if source_length <= 0.000001 {
        return Vec2::new(origin.x + length, origin.y);
    }
    Vec2::new(origin.x + dx / source_length * length, origin.y + dy / source_length * length)
}

fn angle_degrees(origin: Vec2, target: Vec2) -> f64 {
    atan2(target.y - origin.y, target.x - origin.x) * 180.0 / std::f64::consts::PI
}
