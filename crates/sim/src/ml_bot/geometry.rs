//! Obstacle features and local placement probes from Source `ml-bot-policy/geometry.ts`.

use crate::Vec2;
use crate::boneyard::BoneyardBounds;
use crate::boneyard::collision::{
    BoneyardCollisionWorld, can_place_boneyard_body, closest_point_on_segment, point_in_polygon,
};
use crate::float::hypot;
use crate::hub::PLAYER_CHARACTER_RADIUS;

use super::numbers::directions;
use super::spec::scales;

#[derive(Clone, Debug)]
pub struct MlBotPolicyGeometryObservation {
    pub obstacles: Vec<f32>,
    pub patch_and_rays: Vec<f32>,
}

/// The caller supplies the current gate-overlay collision and actual scenery target IDs.
/// Entrance routing separately borrows static collision, as the Source controller does.
pub fn observe_ml_bot_policy_geometry(
    bounds: BoneyardBounds,
    collision: &BoneyardCollisionWorld,
    destructible_ids: &[String],
    position: Vec2,
) -> MlBotPolicyGeometryObservation {
    let mut patch_and_rays = Vec::with_capacity(56);
    for direction in directions() {
        let mut clearance = scales::RAY_RANGE;
        let mut distance = scales::RAY_STEP;
        while distance <= scales::RAY_RANGE {
            let sample =
                Vec2::new(position.x + direction.x * distance, position.y + direction.y * distance);
            if !can_place_boneyard_body(sample, bounds, collision, PLAYER_CHARACTER_RADIUS) {
                clearance = distance;
                break;
            }
            distance += scales::RAY_STEP;
        }
        patch_and_rays.push((clearance / scales::RAY_RANGE) as f32);
    }
    for row in -3..=3 {
        for column in -3..=3 {
            if row == 0 && column == 0 {
                continue;
            }
            let sample = Vec2::new(
                position.x + f64::from(column) * scales::PATCH_SPACING,
                position.y + f64::from(row) * scales::PATCH_SPACING,
            );
            patch_and_rays.push(f32::from(can_place_boneyard_body(
                sample,
                bounds,
                collision,
                PLAYER_CHARACTER_RADIUS,
            )));
        }
    }

    let mut features = Vec::new();
    for circle in collision.circles() {
        let normal = normalized(position.x - circle.center.x, position.y - circle.center.y);
        let closest = Vec2::new(
            circle.center.x + normal.x * circle.radius,
            circle.center.y + normal.y * circle.radius,
        );
        let center_distance = hypot(position.x - circle.center.x, position.y - circle.center.y);
        features.push(PrimitiveFeature {
            clearance: (center_distance - circle.radius - PLAYER_CHARACTER_RADIUS).max(0.0),
            extent: Vec2::ZERO,
            index: features.len(),
            destructible: destructible(&circle.source_id, destructible_ids),
            kind: PrimitiveKind::Circle,
            nearest: Vec2::new(closest.x - position.x, closest.y - position.y),
            normal,
            radius: circle.radius,
        });
    }
    for segment in collision.segments() {
        let closest = closest_point_on_segment(position, segment.start, segment.end);
        let delta = Vec2::new(closest.x - position.x, closest.y - position.y);
        features.push(PrimitiveFeature {
            clearance: (hypot(delta.x, delta.y) - segment.radius - PLAYER_CHARACTER_RADIUS)
                .max(0.0),
            extent: Vec2::new(
                (segment.end.x - segment.start.x).abs() / 2.0,
                (segment.end.y - segment.start.y).abs() / 2.0,
            ),
            index: features.len(),
            destructible: destructible(&segment.source_id, destructible_ids),
            kind: PrimitiveKind::Segment,
            nearest: delta,
            normal: normalized(position.x - closest.x, position.y - closest.y),
            radius: segment.radius,
        });
    }
    for polygon in collision.polygons() {
        let mut closest = position;
        let mut closest_distance = f64::INFINITY;
        let mut minimum = Vec2::new(f64::INFINITY, f64::INFINITY);
        let mut maximum = Vec2::new(f64::NEG_INFINITY, f64::NEG_INFINITY);
        for (index, start) in polygon.points.iter().enumerate() {
            let end = polygon.points[(index + 1) % polygon.points.len()];
            let candidate = closest_point_on_segment(position, *start, end);
            let dx = candidate.x - position.x;
            let dy = candidate.y - position.y;
            let distance = dx * dx + dy * dy;
            if distance < closest_distance {
                closest = candidate;
                closest_distance = distance;
            }
            minimum.x = minimum.x.min(start.x);
            minimum.y = minimum.y.min(start.y);
            maximum.x = maximum.x.max(start.x);
            maximum.y = maximum.y.max(start.y);
        }
        let delta = Vec2::new(closest.x - position.x, closest.y - position.y);
        let inside = !polygon.points.is_empty() && point_in_polygon(position, &polygon.points);
        features.push(PrimitiveFeature {
            clearance: if inside {
                0.0
            } else {
                (hypot(delta.x, delta.y) - PLAYER_CHARACTER_RADIUS).max(0.0)
            },
            extent: if polygon.points.is_empty() {
                Vec2::ZERO
            } else {
                Vec2::new((maximum.x - minimum.x) / 2.0, (maximum.y - minimum.y) / 2.0)
            },
            index: features.len(),
            destructible: destructible(&polygon.source_id, destructible_ids),
            kind: PrimitiveKind::Polygon,
            nearest: delta,
            normal: normalized(position.x - closest.x, position.y - closest.y),
            radius: 0.0,
        });
    }
    features.sort_by(|left, right| {
        left.clearance.total_cmp(&right.clearance).then(left.index.cmp(&right.index))
    });
    let mut obstacles = vec![0.0; 8 * 13];
    for (slot, feature) in features.iter().take(8).enumerate() {
        let start = slot * 13;
        let values = [
            1.0,
            (feature.nearest.x / scales::RANGE).clamp(-1.0, 1.0),
            (feature.nearest.y / scales::RANGE).clamp(-1.0, 1.0),
            (feature.clearance / scales::RANGE).clamp(0.0, 1.0),
            feature.normal.x.clamp(-1.0, 1.0),
            feature.normal.y.clamp(-1.0, 1.0),
            (feature.radius / scales::RADIUS).clamp(0.0, 1.0),
            (feature.extent.x / scales::RANGE).clamp(0.0, 1.0),
            (feature.extent.y / scales::RANGE).clamp(0.0, 1.0),
            f64::from(feature.kind == PrimitiveKind::Circle),
            f64::from(feature.kind == PrimitiveKind::Segment),
            f64::from(feature.kind == PrimitiveKind::Polygon),
            f64::from(feature.destructible),
        ];
        for (offset, value) in values.into_iter().enumerate() {
            obstacles[start + offset] = value as f32;
        }
    }
    MlBotPolicyGeometryObservation { obstacles, patch_and_rays }
}

#[derive(PartialEq)]
enum PrimitiveKind {
    Circle,
    Segment,
    Polygon,
}

struct PrimitiveFeature {
    clearance: f64,
    extent: Vec2,
    index: usize,
    destructible: bool,
    kind: PrimitiveKind,
    nearest: Vec2,
    normal: Vec2,
    radius: f64,
}

fn destructible(source_id: &Option<String>, ids: &[String]) -> bool {
    source_id.as_ref().is_some_and(|id| ids.contains(id))
}

fn normalized(x: f64, y: f64) -> Vec2 {
    let magnitude = hypot(x, y);
    if magnitude > 1e-9 { Vec2::new(x / magnitude, y / magnitude) } else { Vec2::new(1.0, 0.0) }
}
