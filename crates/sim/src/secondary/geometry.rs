//! Native Teleport candidate search and collision legalization from
//! `core-server/native-secondary-world.ts`, plus the authored Phase probe sequence.

use crate::boneyard::BoneyardBounds;
use crate::boneyard::collision::{BoneyardCollisionWorld, can_place_boneyard_body};
use crate::float;
use crate::float::fround;
use crate::{NativeRng, Vec2};

pub const NATIVE_TELEPORT_COLLISION_RADIUS: f64 = 40.0;
pub const NATIVE_GOLEM_PLACEMENT_RADIUS: f64 = 25.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoneyardNativeTeleportBody {
    pub position: Vec2,
    pub radius: f64,
}

pub struct BoneyardNativeTeleportWorld<'a> {
    pub bodies: &'a [BoneyardNativeTeleportBody],
    pub bounds: BoneyardBounds,
    pub collision: &'a BoneyardCollisionWorld,
}

pub fn resolve_boneyard_native_teleport(
    world: BoneyardNativeTeleportWorld<'_>,
    rng: &mut NativeRng,
) -> Vec2 {
    let mut candidates = Vec::new();
    let maximum_x = fround(world.bounds.x + world.bounds.width - 100.0);
    let maximum_y = fround(world.bounds.y + world.bounds.height - 100.0);
    let mut y = fround(world.bounds.y + 100.0);
    while y < maximum_y {
        let mut x = fround(world.bounds.x + 100.0);
        while x < maximum_x {
            let mut score: f64 = 0.0;
            for body in world.bodies {
                let dx = x - body.position.x;
                let dy = y - body.position.y;
                // The stock score is the farthest body's squared distance. Using
                // the nearest body here would change the native candidate order.
                score = score.max((dx * dx + dy * dy).trunc());
            }
            candidates.push((Vec2::new(x, y), score.min(0x10_0000 as f64)));
            x = fround(x + 100.0);
        }
        y = fround(y + 100.0);
    }
    for index in 0..candidates.len() {
        let selected = rng.draw_integer(candidates.len() as i32) as usize;
        candidates.swap(index, selected);
    }
    let mut selected = None;
    let mut best_score = 0.0;
    for (position, score) in candidates {
        if score > best_score {
            best_score = score;
            selected = Some(position);
        }
    }
    let selected = selected.unwrap_or_else(|| {
        // Y precedes X in the source fallback, including an empty candidate grid.
        let y = rng.draw_scaled_float(world.bounds.height, false);
        let x = rng.draw_scaled_float(world.bounds.width, false);
        Vec2::new(fround(world.bounds.x + x), fround(world.bounds.y + y))
    });
    resolve_native_collision_adjusted_position(
        selected,
        NATIVE_TELEPORT_COLLISION_RADIUS,
        rng,
        |position| {
            if !can_place_boneyard_body(
                position,
                world.bounds,
                world.collision,
                NATIVE_TELEPORT_COLLISION_RADIUS,
            ) {
                return false;
            }
            world.bodies.iter().all(|body| {
                let minimum = NATIVE_TELEPORT_COLLISION_RADIUS + body.radius;
                let dx = position.x - body.position.x;
                let dy = position.y - body.position.y;
                dx * dx + dy * dy >= minimum * minimum
            })
        },
    )
}

/// Source legalization searches expanding, vertically compressed rings and returns
/// the first legal probe. The host supplies the real hub/Boneyard static predicate.
pub fn resolve_native_collision_adjusted_position(
    requested: Vec2,
    body_radius: f64,
    rng: &mut NativeRng,
    can_place: impl Fn(Vec2) -> bool,
) -> Vec2 {
    assert!(
        body_radius.is_finite() && body_radius > 0.0,
        "native collision-adjustment radius is finite and positive"
    );
    if can_place(requested) {
        return requested;
    }
    let mut search_radius = fround(body_radius);
    let mut expansion_multiplier = 1.0;
    loop {
        let samples = ((std::f32::consts::PI as f64) * (search_radius + body_radius)
            / search_radius)
            .round_ties_even();
        let heading_step = fround(360.0 / samples);
        let phase = rng.draw_scaled_float(360.0, false);
        let vertical_radius = fround(search_radius * (0.8_f32 as f64));
        let mut offset = 0.0;
        while offset < 360.0 {
            let heading = fround(phase + offset);
            let radians = heading * std::f64::consts::PI / 180.0;
            let candidate = Vec2::new(
                fround(requested.x + fround(fround(float::sin(radians)) * search_radius)),
                fround(requested.y + fround(-fround(float::cos(radians)) * vertical_radius)),
            );
            if can_place(candidate) {
                return candidate;
            }
            offset = fround(offset + heading_step);
        }
        search_radius = fround(search_radius + expansion_multiplier * body_radius);
        let expansion = rng.draw_scaled_float(1.0, false);
        expansion_multiplier = fround(expansion_multiplier * (1.0 + expansion));
    }
}

/// Phase uses unrounded positions and the first traversable 80..270 unit probe.
/// `can_place` includes the actual player's radius and current world geometry.
#[must_use]
pub fn resolve_native_phasing_destination(
    origin: Vec2,
    direction: Vec2,
    can_place: impl Fn(Vec2) -> bool,
) -> Option<Vec2> {
    for distance in (80..=270).step_by(10) {
        let distance = f64::from(distance);
        let candidate =
            Vec2::new(origin.x + direction.x * distance, origin.y + direction.y * distance);
        if can_place(candidate) {
            return Some(candidate);
        }
    }
    None
}
