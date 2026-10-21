//! Circle-versus-axis-aligned-rectangle movement collision.
//!
//! Native `0x00522B20` (move) and `0x00522A30` (alternate response) semantics: a move is
//! accepted when the tentative circle overlaps nothing; otherwise the origin is legalized and
//! the requested delta is applied again from there, preserving the tangent along walls.

use crate::constants::COLLISION_RESPONSE;
use crate::float::{fround, hypot};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct Vec2 {
    pub x: f64,
    pub y: f64,
}

impl Vec2 {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    #[must_use]
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CollisionRectangle {
    pub id: String,
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

impl CollisionRectangle {
    #[must_use]
    pub fn new(id: impl Into<String>, min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> Self {
        Self { id: id.into(), min_x, min_y, max_x, max_y }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CollisionRectangleError {
    AmbiguousId(String),
    InvalidBounds(String),
}

impl std::fmt::Display for CollisionRectangleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AmbiguousId(id) => {
                write!(f, "collision rectangle lookup is ambiguous for id {id}")
            }
            Self::InvalidBounds(id) => {
                write!(f, "collision rectangle {id} has invalid ordered bounds")
            }
        }
    }
}

impl std::error::Error for CollisionRectangleError {}

pub fn validate_collision_rectangles(
    rectangles: &[CollisionRectangle],
) -> Result<(), CollisionRectangleError> {
    let mut ids = std::collections::HashSet::new();
    for rectangle in rectangles {
        if !ids.insert(rectangle.id.as_str()) {
            return Err(CollisionRectangleError::AmbiguousId(rectangle.id.clone()));
        }
        let finite = [rectangle.min_x, rectangle.min_y, rectangle.max_x, rectangle.max_y]
            .iter()
            .all(|value| value.is_finite());
        if !finite || rectangle.min_x >= rectangle.max_x || rectangle.min_y >= rectangle.max_y {
            return Err(CollisionRectangleError::InvalidBounds(rectangle.id.clone()));
        }
    }
    Ok(())
}

/// What a blocked move does after the primary (accept) test fails.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecondaryResponse {
    /// Legalize the tentative position in place.
    Primary,
    /// Stay at the (narrowed) origin.
    Stop,
    /// Legalize the origin, then apply the delta from there (native `0x00522A30`).
    Alternate,
}

fn clamp(value: f64, minimum: f64, maximum: f64) -> f64 {
    minimum.max(maximum.min(value))
}

fn closest_point(position: Vec2, rectangle: &CollisionRectangle) -> Vec2 {
    Vec2::new(
        clamp(position.x, rectangle.min_x, rectangle.max_x),
        clamp(position.y, rectangle.min_y, rectangle.max_y),
    )
}

fn overlaps(position: Vec2, radius: f64, rectangle: &CollisionRectangle) -> bool {
    let closest = closest_point(position, rectangle);
    let dx = position.x - closest.x;
    let dy = position.y - closest.y;
    dx * dx + dy * dy < radius * radius
}

/// True when the circle overlaps no rectangle.
#[must_use]
pub fn circle_placement_is_clear(
    rectangles: &[CollisionRectangle],
    position: Vec2,
    radius: f64,
) -> bool {
    assert!(
        radius.is_finite() && radius > 0.0,
        "circle placement requires a positive finite actor radius"
    );
    rectangles.iter().all(|rectangle| !overlaps(position, radius, rectangle))
}

fn resolve_inside_rectangle(position: Vec2, radius: f64, rectangle: &CollisionRectangle) -> Vec2 {
    let choices = [
        (position.x - rectangle.min_x, Vec2::new(rectangle.min_x - radius, position.y)),
        (rectangle.max_x - position.x, Vec2::new(rectangle.max_x + radius, position.y)),
        (position.y - rectangle.min_y, Vec2::new(position.x, rectangle.min_y - radius)),
        (rectangle.max_y - position.y, Vec2::new(position.x, rectangle.max_y + radius)),
    ];
    let mut selected = choices[0];
    for choice in &choices[1..] {
        if choice.0 < selected.0 {
            selected = *choice;
        }
    }
    Vec2::new(fround(selected.1.x), fround(selected.1.y))
}

fn resolve_one(position: Vec2, radius: f64, rectangle: &CollisionRectangle) -> Vec2 {
    let closest = closest_point(position, rectangle);
    let dx = position.x - closest.x;
    let dy = position.y - closest.y;
    let distance_squared = dx * dx + dy * dy;
    if distance_squared >= radius * radius {
        return position;
    }
    if distance_squared == 0.0 {
        return resolve_inside_rectangle(position, radius, rectangle);
    }
    let distance = distance_squared.sqrt();
    Vec2::new(
        fround(closest.x + dx / distance * radius),
        fround(closest.y + dy / distance * radius),
    )
}

/// Pushes the circle out of every rectangle, iterating until stable or the native limit.
#[must_use]
pub fn resolve_circle_placement(
    rectangles: &[CollisionRectangle],
    position: Vec2,
    radius: f64,
) -> Vec2 {
    let mut resolved = Vec2::new(fround(position.x), fround(position.y));
    for _ in 0..COLLISION_RESPONSE.iteration_limit {
        let mut changed = false;
        for rectangle in rectangles {
            let next = resolve_one(resolved, radius, rectangle);
            if next.x != resolved.x || next.y != resolved.y {
                changed = true;
                resolved = next;
            }
        }
        if !changed {
            break;
        }
    }
    resolved
}

/// Moves a circle by `delta`, applying `secondary` when the tentative placement is blocked.
#[must_use]
pub fn move_circle(
    rectangles: &[CollisionRectangle],
    position: Vec2,
    radius: f64,
    delta: Vec2,
    secondary: SecondaryResponse,
) -> Vec2 {
    let tentative = Vec2::new(fround(position.x + delta.x), fround(position.y + delta.y));
    if circle_placement_is_clear(rectangles, tentative, radius) {
        return tentative;
    }
    match secondary {
        SecondaryResponse::Primary => resolve_circle_placement(rectangles, tentative, radius),
        SecondaryResponse::Stop => Vec2::new(fround(position.x), fround(position.y)),
        SecondaryResponse::Alternate => {
            let legal_origin = resolve_circle_placement(rectangles, position, radius);
            Vec2::new(fround(legal_origin.x + delta.x), fround(legal_origin.y + delta.y))
        }
    }
}

/// Enemy moves are split into sub-steps no longer than `radius - 1` so fast enemies cannot
/// tunnel through thin walls.
#[must_use]
pub fn move_circle_with_enemy_substeps(
    rectangles: &[CollisionRectangle],
    position: Vec2,
    radius: f64,
    delta: Vec2,
) -> Vec2 {
    let length = hypot(delta.x, delta.y);
    let maximum_step = radius - COLLISION_RESPONSE.enemy_substep_radius_margin;
    let step_count =
        if maximum_step > 0.0 { ((length / maximum_step).ceil() as usize).max(1) } else { 1 };
    let step = Vec2::new(delta.x / step_count as f64, delta.y / step_count as f64);
    let mut result = position;
    for _ in 0..step_count {
        result = move_circle(rectangles, result, radius, step, SecondaryResponse::Alternate);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn north_wall() -> Vec<CollisionRectangle> {
        vec![CollisionRectangle::new("north", -10_000.0, -10_000.0, 10_000.0, -15.0)]
    }

    #[test]
    fn accepted_moves_are_narrowed_to_float32() {
        let moved = move_circle(
            &[],
            Vec2::new(0.1, 0.2),
            25.0,
            Vec2::new(0.3, 0.3),
            SecondaryResponse::Stop,
        );
        assert_eq!(moved, Vec2::new(fround(0.1 + 0.3), fround(0.2 + 0.3)));
    }

    #[test]
    fn alternate_response_legalizes_the_origin_and_keeps_the_tangent_along_a_wall() {
        let walls = north_wall();
        // Embedded five units into the wall face (y = 10 is the first clear row for radius 25).
        let moved = move_circle(
            &walls,
            Vec2::new(100.0, 5.0),
            25.0,
            Vec2::new(1.0, -1.0),
            SecondaryResponse::Alternate,
        );
        assert_eq!(moved, Vec2::new(101.0, 9.0), "delta is reapplied from the legalized origin");
        // A clear origin whose tentative placement is blocked moves the same way.
        let moved = move_circle(
            &walls,
            Vec2::new(100.0, 10.5),
            25.0,
            Vec2::new(1.0, -1.0),
            SecondaryResponse::Alternate,
        );
        assert_eq!(moved, Vec2::new(101.0, 9.5));
        let stopped = move_circle(
            &walls,
            Vec2::new(100.0, 10.5),
            25.0,
            Vec2::new(1.0, -1.0),
            SecondaryResponse::Stop,
        );
        assert_eq!(stopped, Vec2::new(100.0, 10.5));
    }

    #[test]
    fn placement_resolution_pushes_an_embedded_circle_out_of_the_nearest_face() {
        let walls = north_wall();
        let resolved = resolve_circle_placement(&walls, Vec2::new(0.0, -20.0), 25.0);
        assert_eq!(resolved, Vec2::new(0.0, 10.0));
    }

    #[test]
    fn invalid_rectangles_are_rejected() {
        let error =
            validate_collision_rectangles(&[CollisionRectangle::new("bad", 1.0, 0.0, 0.0, 1.0)]);
        assert_eq!(error, Err(CollisionRectangleError::InvalidBounds("bad".into())));
        let error = validate_collision_rectangles(&[
            CollisionRectangle::new("dup", 0.0, 0.0, 1.0, 1.0),
            CollisionRectangle::new("dup", 2.0, 2.0, 3.0, 3.0),
        ]);
        assert_eq!(error, Err(CollisionRectangleError::AmbiguousId("dup".into())));
    }
}
