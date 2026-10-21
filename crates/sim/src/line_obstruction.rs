//! Segment, circle, capsule and bounds queries from `core-kernels/line-obstruction.ts`.

use crate::Vec2;
use crate::boneyard::BoneyardBounds;
use crate::float::{fround, hypot};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineObstruction {
    pub point: Vec2,
    pub t: f64,
}

#[must_use]
pub fn line_bounds_exit_obstruction(
    start: Vec2,
    end: Vec2,
    bounds: BoneyardBounds,
) -> Option<LineObstruction> {
    let delta = subtract(end, start);
    let mut t = f64::INFINITY;
    if delta.x > 0.0 {
        t = t.min((bounds.x + bounds.width - start.x) / delta.x);
    } else if delta.x < 0.0 {
        t = t.min((bounds.x - start.x) / delta.x);
    }
    if delta.y > 0.0 {
        t = t.min((bounds.y + bounds.height - start.y) / delta.y);
    } else if delta.y < 0.0 {
        t = t.min((bounds.y - start.y) / delta.y);
    }
    if !(0.0..=1.0).contains(&t) {
        return None;
    }
    Some(LineObstruction { point: mix(start, end, t), t })
}

#[must_use]
pub fn nearer_line_obstruction(
    current: Option<LineObstruction>,
    candidate: Option<LineObstruction>,
) -> Option<LineObstruction> {
    let Some(candidate) = candidate else { return current };
    if !(0.0..=1.0).contains(&candidate.t) {
        return current;
    }
    if current.is_none_or(|current| candidate.t < current.t) {
        return Some(candidate);
    }
    current
}

#[must_use]
pub fn line_segment_obstruction(
    start: Vec2,
    end: Vec2,
    obstacle_start: Vec2,
    obstacle_end: Vec2,
) -> Option<LineObstruction> {
    let ray = subtract(end, start);
    let obstacle = subtract(obstacle_end, obstacle_start);
    let denominator = cross(ray, obstacle);
    if denominator.abs() < f64::EPSILON {
        return None;
    }
    let offset = subtract(obstacle_start, start);
    let t = cross(offset, obstacle) / denominator;
    let u = cross(offset, ray) / denominator;
    if !(0.0..=1.0).contains(&t) || !(0.0..=1.0).contains(&u) {
        return None;
    }
    Some(LineObstruction { point: mix(start, end, t), t })
}

#[must_use]
pub fn line_circle_obstruction(
    start: Vec2,
    end: Vec2,
    center: Vec2,
    radius: f64,
) -> Option<LineObstruction> {
    let ray = subtract(end, start);
    let relative = subtract(start, center);
    let a = dot(ray, ray);
    if a == 0.0 {
        return None;
    }
    let c = dot(relative, relative) - radius * radius;
    if c <= 0.0 {
        return Some(LineObstruction { point: start, t: 0.0 });
    }
    let b = 2.0 * dot(relative, ray);
    let discriminant = b * b - 4.0 * a * c;
    if discriminant < 0.0 {
        return None;
    }
    let root = discriminant.sqrt();
    let near = (-b - root) / (2.0 * a);
    let far = (-b + root) / (2.0 * a);
    let t = if (0.0..=1.0).contains(&near) {
        near
    } else if (0.0..=1.0).contains(&far) {
        far
    } else {
        return None;
    };
    Some(LineObstruction { point: mix(start, end, t), t })
}

#[must_use]
pub fn line_capsule_obstruction(
    start: Vec2,
    end: Vec2,
    capsule_start: Vec2,
    capsule_end: Vec2,
    radius: f64,
) -> Option<LineObstruction> {
    if radius <= 0.0 {
        return line_segment_obstruction(start, end, capsule_start, capsule_end);
    }
    let mut nearest = nearer_line_obstruction(
        line_circle_obstruction(start, end, capsule_start, radius),
        line_circle_obstruction(start, end, capsule_end, radius),
    );
    let axis = subtract(capsule_end, capsule_start);
    let length = hypot(axis.x, axis.y);
    if length == 0.0 {
        return nearest;
    }
    let normal = Vec2::new(-axis.y / length, axis.x / length);
    for sign in [-1.0, 1.0] {
        let offset = Vec2::new(normal.x * radius * sign, normal.y * radius * sign);
        nearest = nearer_line_obstruction(
            nearest,
            line_segment_obstruction(
                start,
                end,
                Vec2::new(capsule_start.x + offset.x, capsule_start.y + offset.y),
                Vec2::new(capsule_end.x + offset.x, capsule_end.y + offset.y),
            ),
        );
    }
    nearest
}

/// Native beam views may begin outside the viewport, so both ends are clipped.
#[must_use]
pub fn clip_line_to_bounds(start: Vec2, end: Vec2, bounds: BoneyardBounds) -> Option<(Vec2, Vec2)> {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let mut enter: f64 = 0.0;
    let mut exit: f64 = 1.0;
    for (direction, distance) in [
        (-dx, start.x - bounds.x),
        (dx, bounds.x + bounds.width - start.x),
        (-dy, start.y - bounds.y),
        (dy, bounds.y + bounds.height - start.y),
    ] {
        if direction == 0.0 {
            if distance < 0.0 {
                return None;
            }
            continue;
        }
        let crossing = distance / direction;
        if direction < 0.0 {
            enter = enter.max(crossing);
        } else {
            exit = exit.min(crossing);
        }
        if enter > exit {
            return None;
        }
    }
    let point = |t| Vec2::new(fround(start.x + dx * t), fround(start.y + dy * t));
    Some((point(enter), point(exit)))
}

fn subtract(first: Vec2, second: Vec2) -> Vec2 {
    Vec2::new(first.x - second.x, first.y - second.y)
}

fn dot(first: Vec2, second: Vec2) -> f64 {
    first.x * second.x + first.y * second.y
}

fn cross(first: Vec2, second: Vec2) -> f64 {
    first.x * second.y - first.y * second.x
}

fn mix(start: Vec2, end: Vec2, t: f64) -> Vec2 {
    Vec2::new(start.x + (end.x - start.x) * t, start.y + (end.y - start.y) * t)
}
