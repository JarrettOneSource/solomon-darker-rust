//! Arena's style-zero surface grid and bridge subtraction from current
//! `core-kernels/native-terrain-surface.ts`. This fixed-quarter QuickSpline is
//! distinct from the natural cubic spline used by Silk and College students.

use crate::float::{atan2, cos, fround as f, sin};
use crate::{NativeRng, Vec2};

use super::{BoneyardBounds, BoneyardRoad, BoneyardTerrain};

pub const NATIVE_GROUND_PI: f64 = 3.141592502593994;
pub type NativeSurfaceQuad = [Vec2; 4];

#[derive(Clone, Debug, Default, PartialEq)]
pub struct NativeTerrainSurface {
    pub cursors: Vec<f64>,
    pub quads: Vec<NativeSurfaceQuad>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeBridgeSurface {
    pub quad: NativeSurfaceQuad,
    pub bounds: BoneyardBounds,
}

struct Axis {
    points: Vec<f64>,
    coefficients: Vec<[f64; 3]>,
}

impl Axis {
    fn new(points: Vec<f64>) -> Self {
        let count = points.len();
        if count < 2 {
            return Self { points, coefficients: Vec::new() };
        }
        if count == 2 {
            let coefficients = vec![[f(points[1] - points[0]), 0.0, 0.0]];
            return Self { points, coefficients };
        }
        let mut forward = vec![0.0; count];
        let mut tangent = vec![0.0; count];
        forward[0] = f((points[1] - points[0]) * 3.0 * 0.25);
        for index in 1..count - 1 {
            forward[index] =
                f(((points[index + 1] - points[index - 1]) * 3.0 - forward[index - 1]) * 0.25);
        }
        forward[count - 1] =
            f(((points[count - 1] - points[count - 2]) * 3.0 - forward[count - 2]) * 0.25);
        tangent[count - 1] = forward[count - 1];
        for index in (0..count - 1).rev() {
            tangent[index] = f(forward[index] - tangent[index + 1] * 0.25);
        }
        let mut coefficients = Vec::with_capacity(count - 1);
        for index in 0..count - 1 {
            coefficients.push([
                tangent[index],
                f((points[index + 1] - points[index]) * 3.0
                    - tangent[index] * 2.0
                    - tangent[index + 1]),
                f((points[index] - points[index + 1]) * 2.0 + tangent[index] + tangent[index + 1]),
            ]);
        }
        Self { points, coefficients }
    }

    fn evaluate(&self, cursor: f64, derivative: bool) -> f64 {
        if self.coefficients.is_empty() {
            return 0.0;
        }
        let segment = cursor.trunc().clamp(0.0, (self.coefficients.len() - 1) as f64) as usize;
        let t = f(cursor.clamp(0.0, self.coefficients.len() as f64) - segment as f64);
        let [b, c, d] = self.coefficients[segment];
        if derivative {
            f(t * t * d * 3.0 + t * c * 2.0 + b)
        } else {
            f(self.points[segment] + t * (b + t * (c + t * d)))
        }
    }
}

struct Spline {
    extent: f64,
    x: Axis,
    y: Axis,
}

impl Spline {
    fn evaluate(&self, cursor: f64, derivative: bool) -> Vec2 {
        Vec2::new(self.x.evaluate(cursor, derivative), self.y.evaluate(cursor, derivative))
    }

    fn cross_section(&self, cursor: f64, left: f64, right: f64) -> [Vec2; 2] {
        let center = self.evaluate(cursor, false);
        let normal_cursor = f(0.001).max(cursor);
        let delta = difference(
            self.evaluate(normal_cursor, false),
            self.evaluate(f(normal_cursor - f(0.001)), false),
        );
        let normal = unit(Vec2::new(delta.y, -delta.x));
        [
            Vec2::new(f(center.x + f(left * normal.x)), f(center.y + f(left * normal.y))),
            Vec2::new(f(center.x - f(right * normal.x)), f(center.y - f(right * normal.y))),
        ]
    }
}

struct DistanceLookup {
    distances: Vec<f64>,
    cursors: Vec<f64>,
    total: f64,
}

impl DistanceLookup {
    fn new(spline: &Spline) -> Self {
        let mut result = Self { distances: vec![0.0], cursors: vec![0.0], total: 0.0 };
        let step = f(0.025);
        let mut previous = Vec2::new(
            spline.x.points.first().copied().unwrap_or(0.0),
            spline.y.points.first().copied().unwrap_or(0.0),
        );
        let mut cursor = step;
        while cursor < spline.extent {
            let mut point = spline.evaluate(cursor, false);
            let mut squared = length_squared(difference(point, previous));
            while squared < 6.25 && cursor < spline.extent {
                cursor = f(cursor + step);
                point = spline.evaluate(cursor, false);
                squared = length_squared(difference(point, previous));
            }
            cursor = cursor.min(spline.extent);
            result.total = f(result.total + f(squared.sqrt()));
            result.distances.push(result.total);
            result.cursors.push(cursor);
            cursor = f(cursor + step);
            previous = point;
        }
        result
    }

    fn at_distance(&self, requested: f64) -> f64 {
        if self.distances.len() < 2 {
            return 0.0;
        }
        let distance = f(requested).clamp(0.0, self.total);
        let low = self.distances.partition_point(|stored| *stored < distance);
        let index = low.saturating_sub(1);
        let span = f(self.distances[index + 1] - self.distances[index]);
        // Native's short-span branch returns the stored distance, not cursor.
        if span <= 0.25 {
            return self.distances[index + 1];
        }
        let t = f(1.0 - (self.distances[index + 1] - distance) / span).clamp(0.0, 1.0);
        f((self.cursors[index + 1] - self.cursors[index]) * t + self.cursors[index])
    }
}

fn sample_cursors(spline: &Spline) -> Vec<f64> {
    let lookup = DistanceLookup::new(spline);
    let mut cursors = vec![0.0];
    if lookup.at_distance(spline.extent) > 0.0 {
        let step = lookup.at_distance(20.0).min(1.0);
        let maximum_gap = lookup.at_distance(120.0);
        let mut last = 0.0;
        let mut previous = unit(Vec2::new(
            spline.x.coefficients.first().map_or(0.0, |row| row[0]),
            spline.y.coefficients.first().map_or(0.0, |row| row[0]),
        ));
        let mut cursor = step;
        while cursor < spline.extent {
            let direction = unit(spline.evaluate(f(0.0001).max(cursor), true));
            let dot = f(direction.x * previous.x + direction.y * previous.y);
            if maximum_gap < cursor - last || (dot < f(0.995) && -f(0.995) < dot) {
                cursors.push(cursor);
                previous = direction;
                last = cursor;
            }
            cursor = f(cursor + step);
        }
    }
    cursors.push(spline.extent);
    cursors
}

/// `buildNativeTerrainSurface`: style one profiles never enter the style-zero grid.
#[must_use]
pub fn build_native_terrain_surface(terrain: &BoneyardTerrain) -> NativeTerrainSurface {
    if terrain.style.unwrap_or(0) != 0 {
        return NativeTerrainSurface::default();
    }
    let points = terrain.points.as_deref().unwrap_or(&[]);
    let spline = Spline {
        extent: points.len().saturating_sub(1) as f64,
        x: Axis::new(points.iter().map(|point| f(point.x)).collect()),
        y: Axis::new(points.iter().map(|point| f(point.y)).collect()),
    };
    let cursors = sample_cursors(&spline);
    let seed = terrain.uid.unwrap_or(0.0);
    assert!(
        seed.is_finite() && seed.trunc() == seed && seed.abs() <= 9_007_199_254_740_991.0,
        "native Terrain RNG seed must be a safe integer"
    );
    let mut rng = NativeRng::new(seed as i64);
    let mut sections = Vec::with_capacity(cursors.len());
    for index in 1..cursors.len() {
        let mut widths = [0.0; 4];
        for width in &mut widths {
            *width = rng.draw_scaled_float(16.25, false);
        }
        let left = f(widths[1] * 0.5 + 32.5);
        let right = f(widths[2] * 0.5 + 32.5);
        if index == 1 {
            sections.push(spline.cross_section(cursors[0], left, right));
        }
        sections.push(spline.cross_section(cursors[index], left, right));
    }
    let quads =
        sections.windows(2).map(|pair| [pair[1][0], pair[1][1], pair[0][0], pair[0][1]]).collect();
    NativeTerrainSurface { cursors, quads }
}

fn difference(a: Vec2, b: Vec2) -> Vec2 {
    Vec2::new(f(a.x - b.x), f(a.y - b.y))
}
fn length_squared(point: Vec2) -> f64 {
    f(point.x * point.x + point.y * point.y)
}
fn unit(point: Vec2) -> Vec2 {
    let length = f(length_squared(point).sqrt());
    let scale = if length > 0.0 { f(1.0 / length) } else { 0.0 };
    Vec2::new(f(point.x * scale), f(point.y * scale))
}

fn triangle_contains(point: Vec2, a: Vec2, b: Vec2, c: Vec2) -> bool {
    let negative = f((point.x - b.x) * (a.y - b.y) - (point.y - b.y) * (a.x - b.x)) < 0.0;
    (f((b.y - c.y) * (point.x - c.x) - (b.x - c.x) * (point.y - c.y)) < 0.0) == negative
        && (f((c.y - a.y) * (point.x - a.x) - (c.x - a.x) * (point.y - a.y)) < 0.0) == negative
}

#[must_use]
pub fn native_surface_quad_contains(quad: &NativeSurfaceQuad, point: Vec2) -> bool {
    triangle_contains(point, quad[0], quad[1], quad[2])
        || triangle_contains(point, quad[1], quad[3], quad[2])
}

#[must_use]
pub fn native_surface_quad_bounds(quad: &NativeSurfaceQuad) -> BoneyardBounds {
    let x = quad.iter().map(|point| point.x).fold(f64::INFINITY, f64::min);
    let y = quad.iter().map(|point| point.y).fold(f64::INFINITY, f64::min);
    let right = quad.iter().map(|point| point.x).fold(f64::NEG_INFINITY, f64::max);
    let bottom = quad.iter().map(|point| point.y).fold(f64::NEG_INFINITY, f64::max);
    BoneyardBounds::new(x, y, f(right - x), f(bottom - y))
}

#[must_use]
pub fn native_bridge_surface_contains(bridge: &NativeBridgeSurface, point: Vec2) -> bool {
    let bounds = bridge.bounds;
    point.x >= bounds.x
        && point.y >= bounds.y
        && point.x < bounds.x + bounds.width
        && point.y < bounds.y + bounds.height
        && native_surface_quad_contains(&bridge.quad, point)
}

fn segments_cross(a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> bool {
    ((c.y - a.y) * (d.x - a.x) < (d.y - a.y) * (c.x - a.x))
        != ((c.y - b.y) * (d.x - b.x) < (d.y - b.y) * (c.x - b.x))
        && ((b.y - a.y) * (c.x - a.x) < (b.x - a.x) * (c.y - a.y))
            != ((b.y - a.y) * (d.x - a.x) < (b.x - a.x) * (d.y - a.y))
}

fn crossing(a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> Option<Vec2> {
    if !segments_cross(a, b, c, d) {
        return None;
    }
    let ay = f(b.y - a.y);
    let ax = f(a.x - b.x);
    let ac = f(ay * a.x + ax * a.y);
    let by = f(d.y - c.y);
    let bx = f(c.x - d.x);
    let bc = f(by * c.x + bx * c.y);
    let determinant = f(bx * ay - by * ax);
    if determinant == 0.0 {
        None
    } else {
        Some(Vec2::new(f((ac * bx - bc * ax) / determinant), f((bc * ay - by * ac) / determinant)))
    }
}

fn road_crosses_quad(quad: &NativeSurfaceQuad, a: Vec2, b: Vec2) -> bool {
    if native_surface_quad_contains(quad, a) || native_surface_quad_contains(quad, b) {
        return true;
    }
    // Source visits both triangles, including their shared diagonal twice.
    [(0, 1), (1, 2), (0, 2), (1, 3), (3, 2), (1, 2)]
        .into_iter()
        .any(|(i, j)| segments_cross(a, b, quad[i], quad[j]))
}

fn bridge_quad(center: Vec2, direction: Vec2) -> NativeSurfaceQuad {
    let radians = f(atan2(direction.x, -direction.y));
    let mut degrees = f(radians * 180.0 / NATIVE_GROUND_PI);
    if degrees < 0.0 {
        degrees = f(degrees + 360.0);
    }
    let angle = f(-degrees * NATIVE_GROUND_PI / 180.0);
    let cosine = f(cos(angle));
    let sine = -f(sin(angle));
    let yx = f(f(0.9) * sine);
    let yy = f(f(0.9) * cosine);
    [(-36.0, -67.5), (36.0, -67.5), (-36.0, 67.5), (36.0, 67.5)].map(|(x, y)| {
        Vec2::new(f(f(x * cosine - y * sine) + center.x), f(f(x * yx + y * yy) + center.y))
    })
}

/// RegionLayout creates one bridge per Road/Terrain, using the first intersecting
/// inner quad. Visual road width, style, links and explicit mesh never enter here.
#[must_use]
pub fn build_native_bridge_surfaces(
    surfaces: &[NativeTerrainSurface],
    roads: &[BoneyardRoad],
) -> Vec<NativeBridgeSurface> {
    let mut bridges = Vec::new();
    for surface in surfaces {
        for road in roads {
            if road.points.len() < 2 {
                continue;
            }
            let start = Vec2::new(f(road.points[0].x), f(road.points[0].y));
            let end = Vec2::new(f(road.points[1].x), f(road.points[1].y));
            let Some(quad) = surface.quads.iter().find(|quad| road_crosses_quad(quad, start, end))
            else {
                continue;
            };
            let mut nearest_start =
                Vec2::new(f((start.x + end.x) * 0.5), f((start.y + end.y) * 0.5));
            let mut nearest_end = nearest_start;
            let mut start_distance = 1e9;
            let mut end_distance = 1e9;
            for (i, j) in [(0, 1), (2, 3), (0, 2), (1, 3)] {
                let Some(hit) = crossing(start, end, quad[i], quad[j]) else {
                    continue;
                };
                let from_start = length_squared(difference(hit, start));
                let from_end = length_squared(difference(hit, end));
                if from_start < start_distance {
                    start_distance = from_start;
                    nearest_start = hit;
                }
                if from_end < end_distance {
                    end_distance = from_end;
                    nearest_end = hit;
                }
            }
            let direction = unit(difference(end, start));
            let mut center = Vec2::new(
                f((nearest_start.x + nearest_end.x) * 0.5),
                f((nearest_start.y + nearest_end.y) * 0.5),
            );
            if start_distance == end_distance {
                center = Vec2::new(
                    f(f(direction.x * 15.0) + nearest_start.x),
                    f(f(direction.y * 15.0) + nearest_start.y),
                );
            }
            center =
                Vec2::new(f(center.x + f(direction.x * 5.0)), f(center.y + f(direction.y * 5.0)));
            let quad = bridge_quad(center, difference(end, start));
            bridges.push(NativeBridgeSurface { quad, bounds: native_surface_quad_bounds(&quad) });
        }
    }
    bridges
}
