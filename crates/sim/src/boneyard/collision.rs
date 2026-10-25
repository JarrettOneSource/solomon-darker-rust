//! Authored Boneyard collision and swept circle movement.
//!
//! Reference: `Website/frontend/src/game/core-server/boneyard-collision.ts`. This is its
//! ordered all-pairs narrow phase: polygons, fence segments, then circles. The Website's
//! broadphase is an optimization of that same solver, so it is not needed for correctness.

use std::collections::HashSet;
use std::sync::Arc;

use crate::Vec2;
use crate::float::hypot;
use crate::line_obstruction::{
    line_bounds_exit_obstruction, line_capsule_obstruction, line_circle_obstruction,
    line_segment_obstruction, nearer_line_obstruction,
};

use super::gate::GateLeaf;
use super::{BoneyardBounds, BoneyardObject, BoneyardScene};

const fn rectangle(left: f64, top: f64, right: f64, bottom: f64) -> [Vec2; 4] {
    [Vec2::new(left, bottom), Vec2::new(left, top), Vec2::new(right, top), Vec2::new(right, bottom)]
}

// Exact native actor footprints retained by boneyard-collision.ts. Object scale and rotation
// dress the art; the collision owner intentionally translates these authored points only.
const MONUMENT_POLYGONS: [&[Vec2]; 21] = [
    &rectangle(-51.0, -27.0, 50.0, 22.0),
    &rectangle(-51.0, -27.0, 50.0, 22.0),
    &rectangle(-29.0, -27.0, 25.0, 19.0),
    &rectangle(-29.0, -27.0, 25.0, 19.0),
    &rectangle(-32.0, -14.0, 30.0, 35.0),
    &rectangle(-32.0, -14.0, 30.0, 35.0),
    &rectangle(-21.0, -17.0, 20.0, 19.0),
    &rectangle(-48.0, -23.0, 49.0, 21.0),
    &rectangle(-48.0, -23.0, 49.0, 21.0),
    &rectangle(-23.0, -20.0, 22.0, 18.0),
    &rectangle(-33.5, -11.5, 34.5, 22.5),
    &rectangle(-68.5, -22.5, 71.5, 33.5),
    &rectangle(-68.5, -22.5, 71.5, 33.5),
    &rectangle(-23.0, -15.0, 24.0, 19.0),
    &rectangle(-23.0, -15.0, 24.0, 19.0),
    &rectangle(-26.0, -18.0, 28.0, 17.0),
    &rectangle(-26.0, -18.0, 28.0, 17.0),
    &rectangle(-25.0, -16.0, 28.0, 27.0),
    &rectangle(-11.0, -10.0, 11.0, 10.0),
    &[Vec2::new(-3.5, 8.5), Vec2::new(-11.5, -5.5), Vec2::new(5.5, -14.5), Vec2::new(14.5, 1.5)],
    &[Vec2::new(-2.5, 14.5), Vec2::new(-14.5, 1.5), Vec2::new(-1.5, -10.5), Vec2::new(12.5, 3.5)],
];

const BUILDING_POLYGONS: [&[Vec2]; 4] = [
    &[
        Vec2::new(92.5, 140.5),
        Vec2::new(56.5, 140.5),
        Vec2::new(54.5, 161.5),
        Vec2::new(31.5, 161.5),
        Vec2::new(31.5, 155.5),
        Vec2::new(-31.5, 155.5),
        Vec2::new(-32.5, 161.5),
        Vec2::new(-57.5, 161.5),
        Vec2::new(-56.5, 139.5),
        Vec2::new(-93.5, 139.5),
        Vec2::new(-93.5, -19.5),
        Vec2::new(92.5, -19.5),
    ],
    &[
        Vec2::new(-60.0, 103.0),
        Vec2::new(-60.0, 116.0),
        Vec2::new(-82.0, 132.0),
        Vec2::new(-103.0, 117.0),
        Vec2::new(-103.0, 77.0),
        Vec2::new(-91.0, 77.0),
        Vec2::new(-91.0, -23.0),
        Vec2::new(-101.0, -23.0),
        Vec2::new(-102.0, -49.0),
        Vec2::new(101.0, -49.0),
        Vec2::new(101.0, -23.0),
        Vec2::new(90.0, -23.0),
        Vec2::new(90.0, 77.0),
        Vec2::new(103.0, 77.0),
        Vec2::new(103.0, 108.0),
        Vec2::new(82.0, 132.0),
        Vec2::new(59.0, 108.0),
        Vec2::new(59.0, 103.0),
    ],
    &[
        Vec2::new(74.0, 141.0),
        Vec2::new(-75.0, 141.0),
        Vec2::new(-75.0, 85.0),
        Vec2::new(-132.0, 85.0),
        Vec2::new(-132.0, -61.0),
        Vec2::new(131.0, -61.0),
        Vec2::new(131.0, 85.0),
        Vec2::new(74.0, 85.0),
    ],
    &rectangle(-64.5, 6.0, 65.5, 132.0),
];

const GRAVE_POLYGON: [Vec2; 4] = [
    Vec2::new(-38.0, 104.0),
    Vec2::new(-35.0, 36.0),
    Vec2::new(27.0, 35.0),
    Vec2::new(31.0, 105.0),
];
const GOODIE_POLYGON: [Vec2; 4] = rectangle(-25.125, -8.625, 25.875, 16.875);

#[derive(Clone, Debug, PartialEq)]
pub struct CollisionPolygon {
    pub points: Vec<Vec2>,
    pub native_line_mask: u32,
    pub source_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CollisionCircle {
    pub center: Vec2,
    pub radius: f64,
    pub native_line_mask: u32,
    pub source_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CollisionSegment {
    pub start: Vec2,
    pub end: Vec2,
    pub radius: f64,
    pub native_line_mask: u32,
    pub source_id: Option<String>,
}

/// Static scenery primitives followed by the current dynamic gate leaves.
#[derive(Clone, Debug, PartialEq)]
pub struct BoneyardCollisionWorld {
    scenery: Arc<CollisionScenery>,
    gate_segments: Vec<CollisionSegment>,
}

#[derive(Debug, Default, PartialEq)]
struct CollisionScenery {
    circles: Vec<CollisionCircle>,
    polygons: Vec<CollisionPolygon>,
    segments: Vec<CollisionSegment>,
}

impl BoneyardCollisionWorld {
    #[must_use]
    pub fn from_scene(scene: &BoneyardScene) -> Self {
        let mut world = CollisionScenery::default();
        for object in &scene.objects {
            world.append_object(object);
        }
        let mut posts = HashSet::new();
        for fence in &scene.fences {
            let [start, end, ..] = fence.points.as_slice() else { continue };
            let code = fence.code();
            if code != 3 {
                for point in [start, end] {
                    if posts.insert((point.x.to_bits(), point.y.to_bits())) {
                        world.circles.push(CollisionCircle {
                            center: *point,
                            radius: 10.0,
                            native_line_mask: 0x700,
                            source_id: None,
                        });
                    }
                }
            }
            match code {
                2 => {}
                1 => {
                    let midpoint = mix(*start, *end, 0.5);
                    world.append_segment(*start, mix(*start, midpoint, 0.82), 0.0, 0x100);
                    world.append_segment(*end, mix(*end, midpoint, 0.82), 0.0, 0x100);
                }
                3 => world.append_segment(*start, *end, 10.0, 0),
                _ => world.append_segment(*start, *end, 0.0, 0x100),
            }
        }
        Self { scenery: Arc::new(world), gate_segments: Vec::new() }
    }

    /// Prepared navigation and imported collision fixtures retain primitive order.
    #[must_use]
    pub fn from_primitives(
        circles: Vec<CollisionCircle>,
        polygons: Vec<CollisionPolygon>,
        segments: Vec<CollisionSegment>,
    ) -> Self {
        Self {
            scenery: Arc::new(CollisionScenery { circles, polygons, segments }),
            gate_segments: Vec::new(),
        }
    }

    #[must_use]
    pub fn circles(&self) -> &[CollisionCircle] {
        &self.scenery.circles
    }

    #[must_use]
    pub fn polygons(&self) -> &[CollisionPolygon] {
        &self.scenery.polygons
    }

    pub fn segments(&self) -> impl DoubleEndedIterator<Item = &CollisionSegment> {
        self.scenery.segments.iter().chain(&self.gate_segments)
    }

    /// Gate collision is replaced in place so a 100 Hz tick never copies static scenery.
    pub fn set_gate_leaves(&mut self, leaves: &[GateLeaf]) {
        self.gate_segments.clear();
        self.gate_segments.extend(leaves.iter().map(|leaf| CollisionSegment {
            start: leaf.hinge,
            end: leaf.tip,
            radius: 0.0,
            native_line_mask: 0x100,
            source_id: None,
        }));
    }
}

impl CollisionScenery {
    fn append_segment(&mut self, start: Vec2, end: Vec2, radius: f64, mask: u32) {
        self.segments.push(CollisionSegment {
            start,
            end,
            radius,
            native_line_mask: mask,
            source_id: None,
        });
    }

    fn append_object(&mut self, object: &BoneyardObject) {
        let variant = object.variant.unwrap_or(0) as usize;
        match object.type_id {
            2001 => self.circles.push(CollisionCircle {
                center: object.position,
                radius: if variant == 1 { 12.0 } else { 8.0 },
                native_line_mask: 0x700,
                source_id: None,
            }),
            2009 => {
                if let Some(points) = MONUMENT_POLYGONS.get(variant) {
                    self.append_polygon(object.position, points, 0, None);
                }
            }
            2029 => {
                let source_id = format!("scenery:{}", object.eid);
                self.circles.push(CollisionCircle {
                    center: object.position,
                    radius: if variant == 1 { 0.0 } else { 1.0 },
                    native_line_mask: 0x600,
                    source_id: Some(source_id.clone()),
                });
                if object.overlay_variant.unwrap_or(0) >= 7 {
                    self.append_polygon(object.position, &GRAVE_POLYGON, 0x600, Some(source_id));
                }
            }
            2040 => {
                if let Some(points) = BUILDING_POLYGONS.get(variant) {
                    self.append_polygon(object.position, points, 0, None);
                }
            }
            2061 => {
                self.circles.push(CollisionCircle {
                    center: object.position,
                    radius: 8.0,
                    native_line_mask: 0x700,
                    source_id: None,
                });
                self.append_polygon(object.position, &GOODIE_POLYGON, 0x700, None);
            }
            _ => {}
        }
    }

    fn append_polygon(
        &mut self,
        origin: Vec2,
        local: &[Vec2],
        mask: u32,
        source_id: Option<String>,
    ) {
        let points =
            local.iter().map(|point| Vec2::new(origin.x + point.x, origin.y + point.y)).collect();
        self.polygons.push(CollisionPolygon { points, native_line_mask: mask, source_id });
    }
}

#[must_use]
pub fn can_place_boneyard_body(
    position: Vec2,
    bounds: BoneyardBounds,
    world: &BoneyardCollisionWorld,
    radius: f64,
) -> bool {
    can_place_boneyard_body_ignoring(position, bounds, world, radius, &[])
}

/// Native MovementCollision omits arena bounds; projectile point and bounds queries are
/// separate source operations, so an otherwise clear point outside the arena stays clear.
#[must_use]
pub fn boneyard_body_collides(position: Vec2, world: &BoneyardCollisionWorld, radius: f64) -> bool {
    first_contact(position, world, radius, position, &[]).is_some()
}

#[must_use]
pub fn can_place_boneyard_body_ignoring(
    position: Vec2,
    bounds: BoneyardBounds,
    world: &BoneyardCollisionWorld,
    radius: f64,
    ignored_source_ids: &[&str],
) -> bool {
    position.x >= bounds.x + radius
        && position.x <= bounds.x + bounds.width - radius
        && position.y >= bounds.y + radius
        && position.y <= bounds.y + bounds.height - radius
        && first_contact(position, world, radius, position, ignored_source_ids).is_none()
}

/// The source actor may be excluded so its collision does not hide its own spell attachment.
#[must_use]
pub fn first_boneyard_line_obstruction(
    start: Vec2,
    end: Vec2,
    bounds: BoneyardBounds,
    world: &BoneyardCollisionWorld,
    excluded_source_id: Option<&str>,
    native_exclusion_mask: u32,
    ignored_source_ids: &[&str],
) -> Option<Vec2> {
    let excluded = |source_id: &Option<String>, mask: u32| {
        source_id
            .as_deref()
            .is_some_and(|id| Some(id) == excluded_source_id || ignored_source_ids.contains(&id))
            || mask & native_exclusion_mask != 0
    };
    let mut nearest = line_bounds_exit_obstruction(start, end, bounds);
    for polygon in world.polygons() {
        if excluded(&polygon.source_id, polygon.native_line_mask) {
            continue;
        }
        for (index, point) in polygon.points.iter().enumerate() {
            nearest = nearer_line_obstruction(
                nearest,
                line_segment_obstruction(
                    start,
                    end,
                    *point,
                    polygon.points[(index + 1) % polygon.points.len()],
                ),
            );
        }
    }
    for circle in world.circles() {
        if !excluded(&circle.source_id, circle.native_line_mask) {
            nearest = nearer_line_obstruction(
                nearest,
                line_circle_obstruction(start, end, circle.center, circle.radius),
            );
        }
    }
    for segment in world.segments() {
        if !excluded(&segment.source_id, segment.native_line_mask) {
            nearest = nearer_line_obstruction(
                nearest,
                line_capsule_obstruction(start, end, segment.start, segment.end, segment.radius),
            );
        }
    }
    nearest.map(|hit| hit.point)
}

#[must_use]
pub fn first_boneyard_path_block_progress(
    start: Vec2,
    end: Vec2,
    bounds: BoneyardBounds,
    world: &BoneyardCollisionWorld,
    radius: f64,
    ignored_source_ids: &[&str],
) -> Option<f64> {
    if !can_place_boneyard_body_ignoring(start, bounds, world, radius, ignored_source_ids) {
        return Some(0.0);
    }
    let distance = hypot(end.x - start.x, end.y - start.y);
    if distance == 0.0 {
        return if can_place_boneyard_body_ignoring(end, bounds, world, radius, ignored_source_ids) {
            None
        } else {
            Some(0.0)
        };
    }
    let sample_count = distance.ceil().max(1.0) as u64;
    let mut last_clear_progress = 0.0;
    for index in 1..=sample_count {
        let progress = index as f64 / sample_count as f64;
        if can_place_boneyard_body_ignoring(
            mix(start, end, progress),
            bounds,
            world,
            radius,
            ignored_source_ids,
        ) {
            last_clear_progress = progress;
            continue;
        }
        let mut clear = last_clear_progress;
        let mut blocked = progress;
        for _ in 0..10 {
            let candidate = (clear + blocked) / 2.0;
            if can_place_boneyard_body_ignoring(
                mix(start, end, candidate),
                bounds,
                world,
                radius,
                ignored_source_ids,
            ) {
                clear = candidate;
            } else {
                blocked = candidate;
            }
        }
        return Some(blocked);
    }
    None
}

/// Collision owners enclosing a body, retained during Solomon's escape from his grave.
#[must_use]
pub fn boneyard_body_collision_source_ids(
    position: Vec2,
    world: &BoneyardCollisionWorld,
    radius: f64,
) -> Vec<String> {
    let mut ids = Vec::new();
    for polygon in world.polygons() {
        let Some(id) = &polygon.source_id else { continue };
        if polygon_contact(position, radius, &polygon.points, position).is_some() {
            ids.push(id.clone());
        }
    }
    for segment in world.segments() {
        let Some(id) = &segment.source_id else { continue };
        if segment_contact(position, radius + segment.radius, segment.start, segment.end, position)
            .is_some()
        {
            ids.push(id.clone());
        }
    }
    for circle in world.circles() {
        let Some(id) = &circle.source_id else { continue };
        let dx = position.x - circle.center.x;
        let dy = position.y - circle.center.y;
        let required = radius + circle.radius;
        if dx * dx + dy * dy < required * required {
            ids.push(id.clone());
        }
    }
    ids.sort();
    ids.dedup();
    ids
}

/// `resolveBoneyardMovement`: clamp the destination, sweep to contact, then retain its tangent.
#[must_use]
pub fn resolve_boneyard_movement(
    start: Vec2,
    requested: Vec2,
    bounds: BoneyardBounds,
    world: &BoneyardCollisionWorld,
    radius: f64,
) -> Vec2 {
    resolve_boneyard_movement_ignoring(start, requested, bounds, world, radius, &[])
}

#[must_use]
pub fn resolve_boneyard_movement_ignoring(
    start: Vec2,
    requested: Vec2,
    bounds: BoneyardBounds,
    world: &BoneyardCollisionWorld,
    radius: f64,
    ignored_source_ids: &[&str],
) -> Vec2 {
    let desired = clamp_to_bounds(requested, bounds, radius);
    if first_contact(desired, world, radius, start, ignored_source_ids).is_none() {
        return desired;
    }
    let swept = sweep_to_last_clear(start, desired, world, radius, ignored_source_ids);
    let contact =
        first_contact(mix(swept, desired, 1.0 / 64.0), world, radius, start, ignored_source_ids)
            .or_else(|| first_contact(desired, world, radius, start, ignored_source_ids));
    let Some(normal) = contact else { return swept };
    let remaining = Vec2::new(desired.x - swept.x, desired.y - swept.y);
    let toward_surface = remaining.x * normal.x + remaining.y * normal.y;
    let slide = if toward_surface < 0.0 {
        Vec2::new(remaining.x - normal.x * toward_surface, remaining.y - normal.y * toward_surface)
    } else {
        remaining
    };
    let slide_target =
        clamp_to_bounds(Vec2::new(swept.x + slide.x, swept.y + slide.y), bounds, radius);
    if first_contact(slide_target, world, radius, swept, ignored_source_ids).is_some() {
        return sweep_to_last_clear(swept, slide_target, world, radius, ignored_source_ids);
    }
    slide_target
}

fn first_contact(
    center: Vec2,
    world: &BoneyardCollisionWorld,
    radius: f64,
    previous: Vec2,
    ignored_source_ids: &[&str],
) -> Option<Vec2> {
    let ignored = |source_id: &Option<String>| {
        source_id.as_ref().is_some_and(|id| ignored_source_ids.contains(&id.as_str()))
    };
    for polygon in world.polygons() {
        if ignored(&polygon.source_id) {
            continue;
        }
        if let Some(normal) = polygon_contact(center, radius, &polygon.points, previous) {
            return Some(normal);
        }
    }
    for segment in world.segments() {
        if ignored(&segment.source_id) {
            continue;
        }
        if let Some(normal) =
            segment_contact(center, radius + segment.radius, segment.start, segment.end, previous)
        {
            return Some(normal);
        }
    }
    for circle in world.circles() {
        if ignored(&circle.source_id) {
            continue;
        }
        let dx = center.x - circle.center.x;
        let dy = center.y - circle.center.y;
        let required = radius + circle.radius;
        if dx * dx + dy * dy < required * required {
            return Some(normalized(dx, dy, previous.x - center.x, previous.y - center.y));
        }
    }
    None
}

fn polygon_contact(center: Vec2, radius: f64, points: &[Vec2], previous: Vec2) -> Option<Vec2> {
    // Source's empty loop retains infinite distance and returns no contact.
    let mut nearest = *points.first()?;
    let mut nearest_distance_squared = f64::INFINITY;
    for (index, start) in points.iter().enumerate() {
        let point = closest_point_on_segment(center, *start, points[(index + 1) % points.len()]);
        let dx = center.x - point.x;
        let dy = center.y - point.y;
        let distance_squared = dx * dx + dy * dy;
        if distance_squared < nearest_distance_squared {
            nearest_distance_squared = distance_squared;
            nearest = point;
        }
    }
    let inside = point_in_polygon(center, points);
    if !inside && nearest_distance_squared >= radius * radius {
        return None;
    }
    let (dx, dy) = if inside {
        (nearest.x - center.x, nearest.y - center.y)
    } else {
        (center.x - nearest.x, center.y - nearest.y)
    };
    Some(normalized(dx, dy, previous.x - center.x, previous.y - center.y))
}

fn segment_contact(
    center: Vec2,
    radius: f64,
    start: Vec2,
    end: Vec2,
    previous: Vec2,
) -> Option<Vec2> {
    let nearest = closest_point_on_segment(center, start, end);
    let dx = center.x - nearest.x;
    let dy = center.y - nearest.y;
    if dx * dx + dy * dy >= radius * radius {
        return None;
    }
    Some(normalized(dx, dy, previous.x - center.x, previous.y - center.y))
}

fn sweep_to_last_clear(
    start: Vec2,
    end: Vec2,
    world: &BoneyardCollisionWorld,
    radius: f64,
    ignored_source_ids: &[&str],
) -> Vec2 {
    let mut clear = start;
    let mut blocked = end;
    for _ in 0..8 {
        let candidate = mix(clear, blocked, 0.5);
        if first_contact(candidate, world, radius, clear, ignored_source_ids).is_some() {
            blocked = candidate;
        } else {
            clear = candidate;
        }
    }
    clear
}

pub(crate) fn point_in_polygon(point: Vec2, polygon: &[Vec2]) -> bool {
    let mut inside = false;
    let Some(mut previous) = polygon.last().copied() else {
        return false;
    };
    for current in polygon {
        if (current.y > point.y) != (previous.y > point.y)
            && point.x
                < (previous.x - current.x) * (point.y - current.y) / (previous.y - current.y)
                    + current.x
        {
            inside = !inside;
        }
        previous = *current;
    }
    inside
}

#[must_use]
pub fn closest_point_on_segment(point: Vec2, start: Vec2, end: Vec2) -> Vec2 {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let length_squared = dx * dx + dy * dy;
    if length_squared == 0.0 {
        return start;
    }
    let t =
        (((point.x - start.x) * dx + (point.y - start.y) * dy) / length_squared).clamp(0.0, 1.0);
    Vec2::new(start.x + dx * t, start.y + dy * t)
}

fn normalized(x: f64, y: f64, fallback_x: f64, fallback_y: f64) -> Vec2 {
    let length = hypot(x, y);
    if length > 0.000001 {
        return Vec2::new(x / length, y / length);
    }
    let fallback_length = hypot(fallback_x, fallback_y);
    if fallback_length > 0.000001 {
        return Vec2::new(fallback_x / fallback_length, fallback_y / fallback_length);
    }
    Vec2::new(1.0, 0.0)
}

fn mix(start: Vec2, end: Vec2, t: f64) -> Vec2 {
    Vec2::new(start.x + (end.x - start.x) * t, start.y + (end.y - start.y) * t)
}

fn clamp_to_bounds(point: Vec2, bounds: BoneyardBounds, radius: f64) -> Vec2 {
    Vec2::new(
        point.x.max(bounds.x + radius).min(bounds.x + bounds.width - radius),
        point.y.max(bounds.y + radius).min(bounds.y + bounds.height - radius),
    )
}
