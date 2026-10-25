//! Mesh construction from `core-server/boneyard-enemy-navigation.ts::buildNativeNavMesh`.
//!
//! Constraint, point and triangle order are observable through endpoint attachment and A*.
//! The source uses its own incremental triangulator; an alternate Delaunay library would
//! not preserve its tie-breaking or directed adjacency order.

use crate::float;
use std::collections::{BTreeMap, BTreeSet};

use crate::Vec2;
use crate::boneyard::BoneyardBounds;
use crate::boneyard::collision::{BoneyardCollisionWorld, can_place_boneyard_body};
use crate::float::hypot;

use super::{
    NavigationMesh, NavigationTriangle, TRIANGLE_EPSILON, cross, float_point, path_is_clear,
};

// The source's authored NavMesh sampling and collision-membership constants.
const COLLISION_EXCLUSION_MASK: u32 = 0x80;
const CIRCLE_SEGMENTS: usize = 8;
const LATTICE_STEP: f64 = 500.0;
const BOUNDARY_MARGIN: f64 = 200.0;
const LOOKUP_CELL: f64 = 200.0;

#[derive(Clone, Copy)]
struct Constraint {
    start: Vec2,
    end: Vec2,
}

/// Build the same ordered graph for an accepted scene or first-query gate collision.
#[must_use]
pub fn build_native_nav_mesh(
    bounds: BoneyardBounds,
    world: &BoneyardCollisionWorld,
    clearance: f64,
) -> NavigationMesh {
    assert!(clearance.is_finite() && clearance > 0.0, "navigation clearance must be positive");
    let navigation_world = BoneyardCollisionWorld::from_primitives(
        world
            .circles()
            .iter()
            .filter(|shape| shape.native_line_mask & COLLISION_EXCLUSION_MASK == 0)
            .cloned()
            .collect(),
        world
            .polygons()
            .iter()
            .filter(|shape| shape.native_line_mask & COLLISION_EXCLUSION_MASK == 0)
            .cloned()
            .collect(),
        world
            .segments()
            .filter(|shape| shape.native_line_mask & COLLISION_EXCLUSION_MASK == 0)
            .cloned()
            .collect(),
    );
    let constraints = navigation_constraints(&navigation_world, clearance);
    let mut candidates = Vec::with_capacity(constraints.len() * 2);
    for constraint in &constraints {
        candidates.push(constraint.start);
        candidates.push(constraint.end);
    }
    append_constraint_intersections(&mut candidates, &constraints);
    append_native_lattice(&mut candidates, bounds, clearance);
    let margin = BOUNDARY_MARGIN + clearance;
    let candidate_bounds = BoneyardBounds::new(
        bounds.x - margin,
        bounds.y - margin,
        bounds.width + margin * 2.0,
        bounds.height + margin * 2.0,
    );
    let points: Vec<_> = unique_navigation_points(&candidates)
        .into_iter()
        .filter(|point| {
            can_place_boneyard_body(*point, candidate_bounds, &navigation_world, clearance)
        })
        .collect();
    let triangles = delaunay_triangles(&points);
    let retained: Vec<_> = triangles
        .into_iter()
        .filter(|vertices| {
            can_place_boneyard_body(
                triangle_center(*vertices, &points),
                bounds,
                &navigation_world,
                clearance,
            )
        })
        .collect();
    connect_triangles(points, retained, bounds, &navigation_world, clearance)
}

fn navigation_constraints(world: &BoneyardCollisionWorld, clearance: f64) -> Vec<Constraint> {
    let mut constraints = Vec::new();
    for polygon in world.polygons() {
        for (index, start) in polygon.points.iter().enumerate() {
            append_capsule_constraints(
                &mut constraints,
                *start,
                polygon.points[(index + 1) % polygon.points.len()],
                clearance,
            );
        }
    }
    for segment in world.segments() {
        append_capsule_constraints(
            &mut constraints,
            segment.start,
            segment.end,
            clearance + segment.radius,
        );
    }
    for circle in world.circles() {
        let radius = circle.radius + clearance;
        let mut ring = Vec::with_capacity(CIRCLE_SEGMENTS);
        for index in 0..CIRCLE_SEGMENTS {
            let radians = index as f64 * std::f64::consts::PI * 2.0 / CIRCLE_SEGMENTS as f64;
            ring.push(float_point(Vec2::new(
                circle.center.x + float::cos(radians) * radius,
                circle.center.y + float::sin(radians) * radius,
            )));
        }
        append_polygon_constraints(&mut constraints, &ring);
    }
    constraints
}

fn append_capsule_constraints(
    constraints: &mut Vec<Constraint>,
    start: Vec2,
    end: Vec2,
    clearance: f64,
) {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let length = hypot(dx, dy);
    if length == 0.0 {
        return;
    }
    let along_x = dx / length * clearance;
    let along_y = dy / length * clearance;
    let across_x = dy / length * clearance;
    let across_y = -dx / length * clearance;
    let points = [
        Vec2::new(start.x - along_x, start.y - along_y),
        Vec2::new(start.x + across_x, start.y + across_y),
        Vec2::new(end.x + across_x, end.y + across_y),
        Vec2::new(end.x + along_x, end.y + along_y),
        Vec2::new(end.x - across_x, end.y - across_y),
        Vec2::new(start.x - across_x, start.y - across_y),
    ]
    .map(float_point);
    append_polygon_constraints(constraints, &points);
}

fn append_polygon_constraints(constraints: &mut Vec<Constraint>, points: &[Vec2]) {
    for (index, start) in points.iter().enumerate() {
        constraints.push(Constraint { start: *start, end: points[(index + 1) % points.len()] });
    }
}

fn append_constraint_intersections(candidates: &mut Vec<Vec2>, constraints: &[Constraint]) {
    for (index, left) in constraints.iter().enumerate() {
        for right in &constraints[index + 1..] {
            if let Some(point) = segment_intersection(*left, *right) {
                candidates.push(point);
            }
        }
    }
}

fn segment_intersection(left: Constraint, right: Constraint) -> Option<Vec2> {
    let left_x = left.end.x - left.start.x;
    let left_y = left.end.y - left.start.y;
    let right_x = right.end.x - right.start.x;
    let right_y = right.end.y - right.start.y;
    let denominator = cross(left_x, left_y, right_x, right_y);
    if denominator.abs() <= TRIANGLE_EPSILON {
        return None;
    }
    let offset_x = right.start.x - left.start.x;
    let offset_y = right.start.y - left.start.y;
    let left_progress = cross(offset_x, offset_y, right_x, right_y) / denominator;
    let right_progress = cross(offset_x, offset_y, left_x, left_y) / denominator;
    if left_progress <= TRIANGLE_EPSILON
        || left_progress >= 1.0 - TRIANGLE_EPSILON
        || right_progress <= TRIANGLE_EPSILON
        || right_progress >= 1.0 - TRIANGLE_EPSILON
    {
        return None;
    }
    Some(float_point(Vec2::new(
        left.start.x + left_x * left_progress,
        left.start.y + left_y * left_progress,
    )))
}

fn append_native_lattice(candidates: &mut Vec<Vec2>, bounds: BoneyardBounds, clearance: f64) {
    let horizontal = axis_samples(bounds.x, bounds.x + bounds.width);
    let vertical = axis_samples(bounds.y, bounds.y + bounds.height);
    for &x in &horizontal {
        for &y in &vertical {
            candidates.push(Vec2::new(x, y));
        }
    }
    let outer_left = bounds.x - BOUNDARY_MARGIN;
    let outer_right = bounds.x + bounds.width + BOUNDARY_MARGIN;
    let outer_top = bounds.y - BOUNDARY_MARGIN;
    let outer_bottom = bounds.y + bounds.height + BOUNDARY_MARGIN;
    for &x in &horizontal {
        candidates.push(Vec2::new(x, outer_top));
        candidates.push(Vec2::new(x, outer_bottom));
    }
    for &y in &vertical {
        candidates.push(Vec2::new(outer_left, y));
        candidates.push(Vec2::new(outer_right, y));
    }
    candidates.extend([
        Vec2::new(outer_left, outer_top),
        Vec2::new(outer_right, outer_top),
        Vec2::new(outer_right, outer_bottom),
        Vec2::new(outer_left, outer_bottom),
    ]);
    let left = bounds.x + clearance;
    let right = bounds.x + bounds.width - clearance;
    let top = bounds.y + clearance;
    let bottom = bounds.y + bounds.height - clearance;
    if left > right || top > bottom {
        return;
    }
    for x in axis_samples(left, right) {
        candidates.push(Vec2::new(x, top));
        candidates.push(Vec2::new(x, bottom));
    }
    for y in axis_samples(top, bottom) {
        candidates.push(Vec2::new(left, y));
        candidates.push(Vec2::new(right, y));
    }
}

fn axis_samples(start: f64, end: f64) -> Vec<f64> {
    let mut samples = vec![crate::float::fround(start)];
    let mut value = start + LATTICE_STEP;
    while value < end {
        samples.push(crate::float::fround(value));
        value += LATTICE_STEP;
    }
    if end != start {
        samples.push(crate::float::fround(end));
    }
    samples
}

fn unique_navigation_points(candidates: &[Vec2]) -> Vec<Vec2> {
    let mut retained = Vec::new();
    let mut keys = BTreeSet::new();
    for candidate in candidates {
        if !candidate.x.is_finite() || !candidate.y.is_finite() {
            continue;
        }
        let point = float_point(*candidate);
        let key = (rounded_key(point.x * 1000.0), rounded_key(point.y * 1000.0));
        if keys.insert(key) {
            retained.push(point);
        }
    }
    retained
}

fn rounded_key(value: f64) -> u64 {
    // Math.round ties toward positive infinity. Comparing the fractional part avoids
    // a premature value+0.5 rounding; JS string keys also identify -0 with +0.
    let lower = value.floor();
    let rounded = if value - lower < 0.5 { lower } else { lower + 1.0 };
    if rounded == 0.0 { 0 } else { rounded.to_bits() }
}

fn delaunay_triangles(source: &[Vec2]) -> Vec<[usize; 3]> {
    if source.len() < 3 {
        return Vec::new();
    }
    let mut minimum = Vec2::new(f64::INFINITY, f64::INFINITY);
    let mut maximum = Vec2::new(f64::NEG_INFINITY, f64::NEG_INFINITY);
    for point in source {
        minimum.x = minimum.x.min(point.x);
        minimum.y = minimum.y.min(point.y);
        maximum.x = maximum.x.max(point.x);
        maximum.y = maximum.y.max(point.y);
    }
    let span = (maximum.x - minimum.x).max(maximum.y - minimum.y).max(1.0);
    let center_x = (minimum.x + maximum.x) * 0.5;
    let center_y = (minimum.y + maximum.y) * 0.5;
    let first_super_vertex = source.len();
    let mut points = source.to_vec();
    points.extend([
        Vec2::new(center_x - span * 32.0, center_y - span),
        Vec2::new(center_x, center_y + span * 32.0),
        Vec2::new(center_x + span * 32.0, center_y - span),
    ]);
    let mut triangles = vec![oriented_triangle(
        first_super_vertex,
        first_super_vertex + 1,
        first_super_vertex + 2,
        &points,
    )];
    for point_index in 0..source.len() {
        let mut bad = vec![false; triangles.len()];
        let mut boundary = Vec::<([usize; 2], usize)>::new();
        let mut boundary_indices: BTreeMap<(usize, usize), usize> = BTreeMap::new();
        for (triangle_index, triangle) in triangles.iter().enumerate() {
            if !circumcircle_contains(*triangle, &points, points[point_index]) {
                continue;
            }
            bad[triangle_index] = true;
            for edge in triangle_edges(*triangle) {
                let key = edge_key(edge);
                if let Some(&index) = boundary_indices.get(&key) {
                    boundary[index].1 += 1;
                } else {
                    boundary_indices.insert(key, boundary.len());
                    boundary.push((edge, 1));
                }
            }
        }
        let mut index = 0;
        triangles.retain(|_| {
            let retained = !bad[index];
            index += 1;
            retained
        });
        // The JS Map iterates first-seen edges, not sorted edge keys.
        for (edge, count) in boundary {
            if count == 1 {
                let triangle = oriented_triangle(edge[0], edge[1], point_index, &points);
                if triangle_area(triangle, &points) > TRIANGLE_EPSILON {
                    triangles.push(triangle);
                }
            }
        }
    }
    triangles.retain(|vertices| vertices.iter().all(|vertex| *vertex < first_super_vertex));
    triangles
}

fn oriented_triangle(first: usize, second: usize, third: usize, points: &[Vec2]) -> [usize; 3] {
    let a = points[first];
    let b = points[second];
    let c = points[third];
    if cross(b.x - a.x, b.y - a.y, c.x - a.x, c.y - a.y) >= 0.0 {
        [first, second, third]
    } else {
        [first, third, second]
    }
}

fn circumcircle_contains(vertices: [usize; 3], points: &[Vec2], point: Vec2) -> bool {
    let a = points[vertices[0]];
    let b = points[vertices[1]];
    let c = points[vertices[2]];
    let ax = a.x - point.x;
    let ay = a.y - point.y;
    let bx = b.x - point.x;
    let by = b.y - point.y;
    let cx = c.x - point.x;
    let cy = c.y - point.y;
    let determinant = (ax * ax + ay * ay) * cross(bx, by, cx, cy)
        - (bx * bx + by * by) * cross(ax, ay, cx, cy)
        + (cx * cx + cy * cy) * cross(ax, ay, bx, by);
    determinant > TRIANGLE_EPSILON
}

fn triangle_area(vertices: [usize; 3], points: &[Vec2]) -> f64 {
    let a = points[vertices[0]];
    let b = points[vertices[1]];
    let c = points[vertices[2]];
    cross(b.x - a.x, b.y - a.y, c.x - a.x, c.y - a.y).abs() * 0.5
}

fn triangle_edges(vertices: [usize; 3]) -> [[usize; 2]; 3] {
    [[vertices[0], vertices[1]], [vertices[1], vertices[2]], [vertices[2], vertices[0]]]
}

fn edge_key(edge: [usize; 2]) -> (usize, usize) {
    if edge[0] < edge[1] { (edge[0], edge[1]) } else { (edge[1], edge[0]) }
}

fn triangle_center(vertices: [usize; 3], points: &[Vec2]) -> Vec2 {
    let a = points[vertices[0]];
    let b = points[vertices[1]];
    let c = points[vertices[2]];
    float_point(Vec2::new((a.x + b.x + c.x) / 3.0, (a.y + b.y + c.y) / 3.0))
}

fn connect_triangles(
    points: Vec<Vec2>,
    triangles: Vec<[usize; 3]>,
    bounds: BoneyardBounds,
    world: &BoneyardCollisionWorld,
    clearance: f64,
) -> NavigationMesh {
    let mut retained: Vec<_> = triangles
        .into_iter()
        .enumerate()
        .map(|(id, vertices)| NavigationTriangle {
            center: triangle_center(vertices, &points),
            id,
            neighbors: Vec::new(),
            vertices,
        })
        .collect();
    let mut owner_by_edge = BTreeMap::new();
    for index in 0..retained.len() {
        for edge in triangle_edges(retained[index].vertices) {
            let key = edge_key(edge);
            let Some(&owner) = owner_by_edge.get(&key) else {
                owner_by_edge.insert(key, index);
                continue;
            };
            // Source adjacency is deliberately directed: this edge is appended before
            // the clearance check; only its reverse edge depends on that check.
            retained[index].neighbors.push(owner);
            if path_is_clear(
                retained[index].center,
                retained[owner].center,
                bounds,
                world,
                clearance,
                &[],
            ) {
                retained[owner].neighbors.push(index);
            }
        }
    }
    connect_visible_triangle_centers(&mut retained, bounds, world, clearance);
    NavigationMesh { points, triangles: retained }
}

fn connect_visible_triangle_centers(
    triangles: &mut [NavigationTriangle],
    bounds: BoneyardBounds,
    world: &BoneyardCollisionWorld,
    clearance: f64,
) {
    let mut cells = BTreeMap::<(i64, i64), Vec<usize>>::new();
    for triangle in triangles.iter() {
        cells.entry(triangle_cell(triangle.center)).or_default().push(triangle.id);
    }
    let cell_radius = (LATTICE_STEP / LOOKUP_CELL).ceil() as i64;
    let maximum_distance_squared = LATTICE_STEP * LATTICE_STEP;
    let mut parent: Vec<_> = (0..triangles.len()).collect();
    let mut component_count = triangles.len();
    for triangle in triangles.iter() {
        for &neighbor in &triangle.neighbors {
            if union(&mut parent, triangle.id, neighbor) {
                component_count -= 1;
            }
        }
    }
    if component_count <= 1 {
        return;
    }
    let mut links = Vec::new();
    for triangle in triangles.iter() {
        let (cell_x, cell_y) = triangle_cell(triangle.center);
        for offset_y in -cell_radius..=cell_radius {
            for offset_x in -cell_radius..=cell_radius {
                let Some(candidates) = cells.get(&(cell_x + offset_x, cell_y + offset_y)) else {
                    continue;
                };
                for &candidate_id in candidates {
                    if candidate_id <= triangle.id || triangle.neighbors.contains(&candidate_id) {
                        continue;
                    }
                    let candidate = &triangles[candidate_id];
                    let dx = triangle.center.x - candidate.center.x;
                    let dy = triangle.center.y - candidate.center.y;
                    let distance_squared = dx * dx + dy * dy;
                    if distance_squared <= maximum_distance_squared {
                        links.push((distance_squared, triangle.id, candidate_id));
                    }
                }
            }
        }
    }
    // Stable order on equal distances is inherited from source candidate enumeration.
    links.sort_by(|left, right| left.0.total_cmp(&right.0));
    for (_, left, right) in links {
        if component_count <= 1 {
            return;
        }
        if find(&mut parent, left) == find(&mut parent, right) {
            continue;
        }
        if !path_is_clear(
            triangles[left].center,
            triangles[right].center,
            bounds,
            world,
            clearance,
            &[],
        ) {
            continue;
        }
        triangles[left].neighbors.push(right);
        triangles[right].neighbors.push(left);
        if union(&mut parent, left, right) {
            component_count -= 1;
        }
    }
}

fn triangle_cell(point: Vec2) -> (i64, i64) {
    ((point.x / LOOKUP_CELL).floor() as i64, (point.y / LOOKUP_CELL).floor() as i64)
}

fn find(parent: &mut [usize], source: usize) -> usize {
    let mut root = source;
    while parent[root] != root {
        root = parent[root];
    }
    let mut current = source;
    while parent[current] != current {
        let next = parent[current];
        parent[current] = root;
        current = next;
    }
    root
}

fn union(parent: &mut [usize], left: usize, right: usize) -> bool {
    let left_root = find(parent, left);
    let right_root = find(parent, right);
    if left_root == right_root {
        return false;
    }
    parent[right_root] = left_root;
    true
}
