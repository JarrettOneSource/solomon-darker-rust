//! Runtime NavMesh queries from `core-server/boneyard-enemy-navigation.ts`.
//!
//! A geometry-owned cache retains source eager preparations and first-query runtime meshes.
//! Endpoint attachment, A*, inset portal crossings and three-point route reduction consume
//! that same ordered graph. Geometry checks keep their ordered all-pairs oracle.

mod build;

pub use build::build_native_nav_mesh;

use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::sync::{Arc, Mutex};

use crate::Vec2;
use crate::float::{fround, hypot};

use super::arena::ArenaTransition;
use super::collision::{BoneyardCollisionWorld, closest_point_on_segment, point_in_polygon};
use super::{BoneyardBounds, BoneyardSource, LoadedBoneyard};

const TRIANGLE_EPSILON: f64 = 1e-7;
const PORTAL_INSET: f64 = 1.0;
const HEURISTIC_CELL: f64 = 200.0;
const SIMPLIFICATION_CLEARANCE_FACTOR: f64 = 1.200_000_047_683_715_8;

#[derive(Clone, Debug, PartialEq)]
pub struct NavigationTriangle {
    pub center: Vec2,
    pub id: usize,
    pub neighbors: Vec<usize>,
    pub vertices: [usize; 3],
}

#[derive(Clone, Debug, PartialEq)]
pub struct NavigationMesh {
    pub points: Vec<Vec2>,
    pub triangles: Vec<NavigationTriangle>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PreparedNavigationMesh {
    pub bounds: BoneyardBounds,
    pub clearance: f64,
    pub mesh: NavigationMesh,
}

/// One source collision-geometry identity and its first-build mesh cache.
/// Clones share that geometry service; a newly decoded scene owns a fresh cache.
#[derive(Clone, Debug)]
pub struct NavigationSet {
    pub source_sha256: String,
    pub geometry_sha256: String,
    meshes: Arc<Mutex<Vec<Arc<PreparedNavigationMesh>>>>,
}

impl PartialEq for NavigationSet {
    fn eq(&self, other: &Self) -> bool {
        if self.source_sha256 != other.source_sha256
            || self.geometry_sha256 != other.geometry_sha256
        {
            return false;
        }
        let first = self.meshes.lock().expect("navigation mesh cache is not poisoned").clone();
        let second = other.meshes.lock().expect("navigation mesh cache is not poisoned").clone();
        first == second
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RouteRequest<'a> {
    pub body_radius: f64,
    pub bounds: BoneyardBounds,
    pub clearance: f64,
    pub end: Vec2,
    pub end_body_radius: Option<f64>,
    pub ignored_source_ids: &'a [&'a str],
    pub start: Vec2,
}

impl NavigationSet {
    #[must_use]
    pub fn new(
        source_sha256: String,
        geometry_sha256: String,
        prepared: Vec<PreparedNavigationMesh>,
    ) -> Self {
        Self {
            source_sha256,
            geometry_sha256,
            meshes: Arc::new(Mutex::new(prepared.into_iter().map(Arc::new).collect())),
        }
    }

    /// `prepareBoneyardWorldNavigation`: build the actual accepted scene's eager domains.
    #[must_use]
    pub fn for_scene(loaded: &LoadedBoneyard) -> Self {
        let collision = BoneyardCollisionWorld::from_scene(&loaded.scene);
        let mut prepared = Vec::new();
        for (bounds, clearance) in boneyard_world_navigation_preparations(loaded) {
            prepared.push(PreparedNavigationMesh {
                bounds,
                clearance,
                mesh: build_native_nav_mesh(bounds, &collision, clearance),
            });
        }
        Self::new(loaded.source_sha256.clone(), loaded.geometry_sha256.clone(), prepared)
    }

    #[must_use]
    pub fn prepared_for(
        &self,
        bounds: BoneyardBounds,
        clearance: f64,
    ) -> Option<Arc<PreparedNavigationMesh>> {
        self.meshes
            .lock()
            .expect("navigation mesh cache is not poisoned")
            .iter()
            .find(|mesh| mesh.bounds == bounds && mesh.clearance == clearance)
            .cloned()
    }

    /// Explicit source prewarming, using the current geometry rather than a substitute map.
    pub fn prepare(&self, bounds: BoneyardBounds, world: &BoneyardCollisionWorld, clearance: f64) {
        self.native_nav_mesh(bounds, world, clearance);
    }

    #[must_use]
    pub fn find_route(
        &self,
        request: RouteRequest<'_>,
        world: &BoneyardCollisionWorld,
    ) -> Option<Vec<Vec2>> {
        validate_route_request(request);
        if path_is_clear(
            request.start,
            request.end,
            request.bounds,
            world,
            request.clearance,
            request.ignored_source_ids,
        ) {
            // A clear source route does not initialize the lazy cache.
            return Some(vec![request.start, request.end]);
        }
        let prepared = self.native_nav_mesh(request.bounds, world, request.clearance);
        route_from_mesh(request, &prepared.mesh, world)
    }

    fn native_nav_mesh(
        &self,
        bounds: BoneyardBounds,
        world: &BoneyardCollisionWorld,
        clearance: f64,
    ) -> Arc<PreparedNavigationMesh> {
        let mut meshes = self.meshes.lock().expect("navigation mesh cache is not poisoned");
        if let Some(mesh) =
            meshes.iter().find(|mesh| mesh.bounds == bounds && mesh.clearance == clearance)
        {
            return Arc::clone(mesh);
        }
        let prepared = Arc::new(PreparedNavigationMesh {
            bounds,
            clearance,
            mesh: build_native_nav_mesh(bounds, world, clearance),
        });
        meshes.push(Arc::clone(&prepared));
        prepared
    }
}

/// The exact eager domain inventory from `boneyard-world-construction.ts`.
#[must_use]
pub fn boneyard_world_navigation_preparations(
    loaded: &LoadedBoneyard,
) -> Vec<(BoneyardBounds, f64)> {
    let bounds = loaded.scene.bounds;
    let owns_encounter =
        loaded.choice_source == BoneyardSource::Default && loaded.scene.solomon_dig.is_some();
    let ordinary = owns_encounter && loaded.choice_id != "stock-tutorial";
    let hostile_bounds = if ordinary {
        ArenaTransition::new(bounds, loaded.scene.spawn.position).combat_bounds
    } else {
        bounds
    };
    let mut preparations = vec![(hostile_bounds, 25.0), (hostile_bounds, 50.0)];
    if owns_encounter {
        preparations.push((super::escape::solomon_escape_traversal_bounds(bounds), 25.0));
    }
    preparations
}

#[must_use]
pub fn find_boneyard_enemy_route(
    request: RouteRequest<'_>,
    mesh: &NavigationMesh,
    world: &BoneyardCollisionWorld,
) -> Option<Vec<Vec2>> {
    validate_route_request(request);
    if path_is_clear(
        request.start,
        request.end,
        request.bounds,
        world,
        request.clearance,
        request.ignored_source_ids,
    ) {
        return Some(vec![request.start, request.end]);
    }
    route_from_mesh(request, mesh, world)
}

fn validate_route_request(request: RouteRequest<'_>) {
    assert!(
        request.clearance.is_finite() && request.clearance > 0.0,
        "enemy navigation clearance must be positive and finite"
    );
    assert!(
        request.body_radius.is_finite() && request.body_radius >= 0.0,
        "enemy navigation body radius must be nonnegative and finite"
    );
    assert!(
        request.end_body_radius.is_none_or(|radius| radius.is_finite() && radius >= 0.0),
        "enemy navigation destination radius must be nonnegative and finite"
    );
    assert!(
        [request.start.x, request.start.y, request.end.x, request.end.y]
            .iter()
            .all(|value| value.is_finite()),
        "enemy route endpoints must be finite"
    );
}

fn route_from_mesh(
    request: RouteRequest<'_>,
    mesh: &NavigationMesh,
    world: &BoneyardCollisionWorld,
) -> Option<Vec<Vec2>> {
    let RouteRequest {
        start,
        end,
        bounds,
        clearance,
        body_radius,
        end_body_radius,
        ignored_source_ids,
    } = request;
    let start_triangle =
        resolve_endpoint_triangle(mesh, start, bounds, world, body_radius, ignored_source_ids)?;
    let end_triangle = resolve_endpoint_triangle(
        mesh,
        end,
        bounds,
        world,
        end_body_radius.unwrap_or(body_radius),
        ignored_source_ids,
    )?;
    let triangles = find_triangle_path(mesh, start_triangle, end_triangle)?;
    let mut route = vec![start];
    append_distinct(&mut route, mesh.triangles[start_triangle].center);
    for pair in triangles.windows(2) {
        let previous = &mesh.triangles[pair[0]];
        let next = &mesh.triangles[pair[1]];
        if let Some(portal) = shared_portal(previous, next) {
            let crossing =
                project_to_inset_portal(end, mesh.points[portal[0]], mesh.points[portal[1]]);
            if path_is_clear(previous.center, crossing, bounds, world, clearance, &[])
                && path_is_clear(crossing, next.center, bounds, world, clearance, &[])
            {
                append_distinct(&mut route, crossing);
            }
        }
        append_distinct(&mut route, next.center);
    }
    route.push(end);
    Some(simplify_route_prefix(route, bounds, world, body_radius, ignored_source_ids))
}

/// Clearance against the unchanged polygon, circle and capsule primitives.
#[must_use]
pub fn path_is_clear(
    start: Vec2,
    end: Vec2,
    bounds: BoneyardBounds,
    world: &BoneyardCollisionWorld,
    radius: f64,
    ignored_source_ids: &[&str],
) -> bool {
    if !point_inside_bounds(start, bounds, radius) || !point_inside_bounds(end, bounds, radius) {
        return false;
    }
    for polygon in &world.polygons {
        if ignored(polygon.source_id.as_deref(), ignored_source_ids) {
            continue;
        }
        if point_in_polygon(start, &polygon.points) || point_in_polygon(end, &polygon.points) {
            return false;
        }
        for (index, point) in polygon.points.iter().enumerate() {
            if separation_blocks(
                segment_distance_squared(
                    start,
                    end,
                    *point,
                    polygon.points[(index + 1) % polygon.points.len()],
                ),
                radius,
            ) {
                return false;
            }
        }
    }
    for circle in &world.circles {
        if ignored(circle.source_id.as_deref(), ignored_source_ids) {
            continue;
        }
        if separation_blocks(
            point_segment_distance_squared(circle.center, start, end),
            radius + circle.radius,
        ) {
            return false;
        }
    }
    for segment in &world.segments {
        if ignored(segment.source_id.as_deref(), ignored_source_ids) {
            continue;
        }
        if separation_blocks(
            segment_distance_squared(start, end, segment.start, segment.end),
            radius + segment.radius,
        ) {
            return false;
        }
    }
    true
}

fn resolve_endpoint_triangle(
    mesh: &NavigationMesh,
    point: Vec2,
    bounds: BoneyardBounds,
    world: &BoneyardCollisionWorld,
    radius: f64,
    ignored_source_ids: &[&str],
) -> Option<usize> {
    if !point_inside_bounds(point, bounds, radius) {
        return None;
    }
    let mut containing = Vec::new();
    let mut remaining = Vec::new();
    for triangle in &mesh.triangles {
        let vertices = triangle.vertices.map(|vertex| mesh.points[vertex]);
        let candidate = (point_triangle_distance_squared(point, vertices), triangle.id);
        if point_in_triangle(point, vertices) {
            containing.push(candidate);
        } else {
            remaining.push(candidate);
        }
    }
    let order = |left: &(f64, usize), right: &(f64, usize)| {
        left.0.total_cmp(&right.0).then_with(|| left.1.cmp(&right.1))
    };
    containing.sort_by(order);
    remaining.sort_by(order);
    let mut triangle_ids = containing.into_iter().chain(remaining).map(|(_, id)| id);
    triangle_ids.find(|id| {
        path_is_clear(point, mesh.triangles[*id].center, bounds, world, radius, ignored_source_ids)
    })
}

// BinaryHeap is a max heap. Reversing the two source key fields reproduces TriangleMinHeap:
// the lowest f wins, with insertion order as the only tie break.
#[derive(Clone, Copy, Debug)]
struct OpenTriangle {
    f: f64,
    g: f64,
    id: usize,
    order: usize,
}

impl PartialEq for OpenTriangle {
    fn eq(&self, other: &Self) -> bool {
        self.f.to_bits() == other.f.to_bits() && self.order == other.order
    }
}
impl Eq for OpenTriangle {}
impl PartialOrd for OpenTriangle {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for OpenTriangle {
    fn cmp(&self, other: &Self) -> Ordering {
        other.f.total_cmp(&self.f).then_with(|| other.order.cmp(&self.order))
    }
}

fn find_triangle_path(mesh: &NavigationMesh, start: usize, end: usize) -> Option<Vec<usize>> {
    if start == end {
        return Some(vec![start]);
    }
    let mut scores = vec![f64::INFINITY; mesh.triangles.len()];
    let mut came_from = vec![None; mesh.triangles.len()];
    scores[start] = 0.0;
    let mut open = BinaryHeap::new();
    let mut order = 0;
    open.push(OpenTriangle {
        f: heuristic(&mesh.triangles[start], &mesh.triangles[end]),
        g: 0.0,
        id: start,
        order,
    });
    order += 1;
    while let Some(current) = open.pop() {
        if scores[current.id] != current.g {
            continue;
        }
        if current.id == end {
            let mut reversed = vec![end];
            let mut previous = came_from[end];
            while let Some(id) = previous {
                reversed.push(id);
                previous = came_from[id];
            }
            reversed.reverse();
            return Some(reversed);
        }
        for neighbor in &mesh.triangles[current.id].neighbors {
            let tentative = current.g + 1.0;
            if tentative >= scores[*neighbor] {
                continue;
            }
            scores[*neighbor] = tentative;
            came_from[*neighbor] = Some(current.id);
            open.push(OpenTriangle {
                f: tentative + heuristic(&mesh.triangles[*neighbor], &mesh.triangles[end]),
                g: tentative,
                id: *neighbor,
                order,
            });
            order += 1;
        }
    }
    None
}

fn heuristic(left: &NavigationTriangle, right: &NavigationTriangle) -> f64 {
    hypot(right.center.x - left.center.x, right.center.y - left.center.y) / HEURISTIC_CELL
}

fn shared_portal(left: &NavigationTriangle, right: &NavigationTriangle) -> Option<[usize; 2]> {
    let shared: Vec<_> =
        left.vertices.iter().copied().filter(|vertex| right.vertices.contains(vertex)).collect();
    if shared.len() != 2 {
        return None;
    }
    Some([shared[0], shared[1]])
}

fn project_to_inset_portal(destination: Vec2, start: Vec2, end: Vec2) -> Vec2 {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let length = hypot(dx, dy);
    if length == 0.0 {
        return float_point(start);
    }
    let inset = (PORTAL_INSET / length).min(0.5);
    let projected =
        ((destination.x - start.x) * dx + (destination.y - start.y) * dy) / (length * length);
    let progress = projected.max(inset).min(1.0 - inset);
    float_point(Vec2::new(start.x + dx * progress, start.y + dy * progress))
}

fn simplify_route_prefix(
    route: Vec<Vec2>,
    bounds: BoneyardBounds,
    world: &BoneyardCollisionWorld,
    body_radius: f64,
    ignored_source_ids: &[&str],
) -> Vec<Vec2> {
    if route.len() < 3 {
        return route;
    }
    let mut first = 1;
    let mut second = 2;
    let mut clearance = body_radius;
    for index in 2..route.len() - 1 {
        if !path_is_clear(route[0], route[index], bounds, world, clearance, ignored_source_ids) {
            break;
        }
        first = index;
        second = index + 1;
        clearance *= SIMPLIFICATION_CLEARANCE_FACTOR;
    }
    vec![route[0], route[first], route[second]]
}

fn append_distinct(route: &mut Vec<Vec2>, point: Vec2) {
    if route.last().is_some_and(|previous| *previous == point) {
        return;
    }
    route.push(point);
}

fn ignored(source_id: Option<&str>, ignored_source_ids: &[&str]) -> bool {
    source_id.is_some_and(|id| ignored_source_ids.contains(&id))
}

fn point_inside_bounds(point: Vec2, bounds: BoneyardBounds, radius: f64) -> bool {
    point.x >= bounds.x + radius
        && point.x <= bounds.x + bounds.width - radius
        && point.y >= bounds.y + radius
        && point.y <= bounds.y + bounds.height - radius
}

fn separation_blocks(distance_squared: f64, required: f64) -> bool {
    if required == 0.0 {
        return distance_squared <= TRIANGLE_EPSILON;
    }
    distance_squared < required * required
}

fn segment_distance_squared(
    left_start: Vec2,
    left_end: Vec2,
    right_start: Vec2,
    right_end: Vec2,
) -> f64 {
    if segments_intersect(left_start, left_end, right_start, right_end) {
        return 0.0;
    }
    point_segment_distance_squared(left_start, right_start, right_end)
        .min(point_segment_distance_squared(left_end, right_start, right_end))
        .min(point_segment_distance_squared(right_start, left_start, left_end))
        .min(point_segment_distance_squared(right_end, left_start, left_end))
}

fn segments_intersect(
    left_start: Vec2,
    left_end: Vec2,
    right_start: Vec2,
    right_end: Vec2,
) -> bool {
    let first = orientation(left_start, left_end, right_start);
    let second = orientation(left_start, left_end, right_end);
    let third = orientation(right_start, right_end, left_start);
    let fourth = orientation(right_start, right_end, left_end);
    if ((first > 0.0 && second < 0.0) || (first < 0.0 && second > 0.0))
        && ((third > 0.0 && fourth < 0.0) || (third < 0.0 && fourth > 0.0))
    {
        return true;
    }
    (first.abs() <= TRIANGLE_EPSILON && point_on_segment(right_start, left_start, left_end))
        || (second.abs() <= TRIANGLE_EPSILON && point_on_segment(right_end, left_start, left_end))
        || (third.abs() <= TRIANGLE_EPSILON && point_on_segment(left_start, right_start, right_end))
        || (fourth.abs() <= TRIANGLE_EPSILON && point_on_segment(left_end, right_start, right_end))
}

fn orientation(start: Vec2, end: Vec2, point: Vec2) -> f64 {
    cross(end.x - start.x, end.y - start.y, point.x - start.x, point.y - start.y)
}

fn point_on_segment(point: Vec2, start: Vec2, end: Vec2) -> bool {
    point.x >= start.x.min(end.x) - TRIANGLE_EPSILON
        && point.x <= start.x.max(end.x) + TRIANGLE_EPSILON
        && point.y >= start.y.min(end.y) - TRIANGLE_EPSILON
        && point.y <= start.y.max(end.y) + TRIANGLE_EPSILON
}

fn point_in_triangle(point: Vec2, vertices: [Vec2; 3]) -> bool {
    let first = triangle_side(point, vertices[0], vertices[1]);
    let second = triangle_side(point, vertices[1], vertices[2]);
    let third = triangle_side(point, vertices[2], vertices[0]);
    let negative =
        first < -TRIANGLE_EPSILON || second < -TRIANGLE_EPSILON || third < -TRIANGLE_EPSILON;
    let positive =
        first > TRIANGLE_EPSILON || second > TRIANGLE_EPSILON || third > TRIANGLE_EPSILON;
    !(negative && positive)
}

fn triangle_side(point: Vec2, start: Vec2, end: Vec2) -> f64 {
    cross(point.x - end.x, point.y - end.y, start.x - end.x, start.y - end.y)
}

fn point_triangle_distance_squared(point: Vec2, vertices: [Vec2; 3]) -> f64 {
    point_segment_distance_squared(point, vertices[0], vertices[1])
        .min(point_segment_distance_squared(point, vertices[1], vertices[2]))
        .min(point_segment_distance_squared(point, vertices[2], vertices[0]))
}

fn point_segment_distance_squared(point: Vec2, start: Vec2, end: Vec2) -> f64 {
    let nearest = closest_point_on_segment(point, start, end);
    let dx = point.x - nearest.x;
    let dy = point.y - nearest.y;
    dx * dx + dy * dy
}

fn float_point(point: Vec2) -> Vec2 {
    Vec2::new(fround(point.x), fround(point.y))
}
fn cross(ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    ax * by - ay * bx
}
