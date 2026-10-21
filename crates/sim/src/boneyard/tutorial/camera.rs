//! Tutorial camera lock and entrance fence from `native-tutorial.ts`.

use crate::Vec2;
use crate::boneyard::BoneyardBounds;
use crate::float::fround;
use crate::hub::{HubSegment, nearest_point_on_segment};

use super::NativeTutorialState;

pub const NATIVE_TUTORIAL_CAMERA_TRIGGER: BoneyardBounds =
    BoneyardBounds::new(-77.77447509765625, -55.6324462890625, 2099.2255859375, 887.3675537109375);
pub const NATIVE_TUTORIAL_LEVEL_BOUNDS: BoneyardBounds =
    BoneyardBounds::new(0.0, 0.0, 2043.0, 2053.0);
pub const NATIVE_TUTORIAL_CAMERA_TARGET: BoneyardBounds =
    BoneyardBounds::new(0.0, 0.0, 2043.0, 849.91796875);
pub const NATIVE_TUTORIAL_CAMERA_CLEANUP_TICKS: u64 = 300;
pub const NATIVE_TUTORIAL_CAMERA_LOCK_INITIAL_BLEND: f64 = 0.01_f32 as f64;
pub const NATIVE_TUTORIAL_CAMERA_LOCK_BLEND_GROWTH: f64 = 1.01;
pub const NATIVE_TUTORIAL_CAMERA_LOCK_SETTLE_TICKS: usize = 464;

pub const NATIVE_TUTORIAL_ENTRANCE_FENCE_CHAIN: [Vec2; 17] = [
    Vec2::new(4.150634765625, 1627.25146484375),
    Vec2::new(149.150634765625, 1623.25146484375),
    Vec2::new(311.150634765625, 1620.25146484375),
    Vec2::new(451.150634765625, 1620.25146484375),
    Vec2::new(604.150634765625, 1618.25146484375),
    Vec2::new(746.150634765625, 1616.25146484375),
    Vec2::new(874.206787109375, 1614.6953125),
    Vec2::new(933.454345703125, 1616.0),
    Vec2::new(1118.454345703125, 1609.0),
    Vec2::new(1171.994140625, 1609.0),
    Vec2::new(1248.994140625, 1609.0),
    Vec2::new(1325.994140625, 1610.0),
    Vec2::new(1473.150634765625, 1611.25146484375),
    Vec2::new(1610.150634765625, 1609.25146484375),
    Vec2::new(1747.150634765625, 1608.25146484375),
    Vec2::new(1901.150634765625, 1606.25146484375),
    Vec2::new(2044.150634765625, 1605.25146484375),
];

const CAMERA_BOUNDS: [BoneyardBounds; NATIVE_TUTORIAL_CAMERA_LOCK_SETTLE_TICKS + 1] =
    build_camera_bounds();

/// The source's immutable camera table is compiled into the program, outside checkpoints.
#[must_use]
pub fn native_tutorial_camera_bounds(state: &NativeTutorialState) -> Option<BoneyardBounds> {
    if !state.camera_lock_triggered {
        return None;
    }
    let age =
        state.camera_lock_age_ticks.clamp(0.0, NATIVE_TUTORIAL_CAMERA_LOCK_SETTLE_TICKS as f64);
    let whole_age = age.floor() as usize;
    let current = CAMERA_BOUNDS[whole_age];
    let fraction = age - whole_age as f64;
    if fraction == 0.0 || whole_age == NATIVE_TUTORIAL_CAMERA_LOCK_SETTLE_TICKS {
        return Some(current);
    }
    let next = CAMERA_BOUNDS[whole_age + 1];
    Some(BoneyardBounds::new(
        current.x + (next.x - current.x) * fraction,
        current.y + (next.y - current.y) * fraction,
        current.width + (next.width - current.width) * fraction,
        current.height + (next.height - current.height) * fraction,
    ))
}

const fn build_camera_bounds() -> [BoneyardBounds; NATIVE_TUTORIAL_CAMERA_LOCK_SETTLE_TICKS + 1] {
    let mut bounds = [NATIVE_TUTORIAL_LEVEL_BOUNDS; NATIVE_TUTORIAL_CAMERA_LOCK_SETTLE_TICKS + 1];
    let mut current = NATIVE_TUTORIAL_LEVEL_BOUNDS;
    let target = NATIVE_TUTORIAL_CAMERA_TARGET;
    let mut blend = NATIVE_TUTORIAL_CAMERA_LOCK_INITIAL_BLEND;
    let mut age = 1;
    while age <= NATIVE_TUTORIAL_CAMERA_LOCK_SETTLE_TICKS {
        current = BoneyardBounds::new(
            fround(current.x + (target.x - current.x) * blend),
            fround(current.y + (target.y - current.y) * blend),
            fround(current.width + (target.width - current.width) * blend),
            fround(current.height + (target.height - current.height) * blend),
        );
        bounds[age] = current;
        let grown_blend = blend * NATIVE_TUTORIAL_CAMERA_LOCK_BLEND_GROWTH;
        blend = fround(if grown_blend < 1.0 { grown_blend } else { 1.0 });
        age += 1;
    }
    bounds
}

#[must_use]
pub fn native_tutorial_enemy_camera_position_is_allowed(position: Vec2, radius: f64) -> bool {
    if !radius.is_finite() || radius <= 0.0 {
        return false;
    }
    let bounds = NATIVE_TUTORIAL_CAMERA_TARGET;
    position.x - radius >= bounds.x
        && position.y - radius >= bounds.y
        && position.x + radius <= bounds.x + bounds.width
        && position.y + radius <= bounds.y + bounds.height
}

#[must_use]
pub fn native_tutorial_camera_lock_safety_clear(bodies: &[(Vec2, f64)]) -> bool {
    bodies.iter().all(|(position, radius)| {
        native_tutorial_enemy_camera_position_is_allowed(*position, *radius)
    })
}

#[must_use]
pub fn native_tutorial_enemy_spawn_position_is_allowed(position: Vec2, radius: f64) -> bool {
    if !radius.is_finite() || radius <= 0.0 {
        return false;
    }
    if position.y >= entrance_fence_y(position.x) {
        return false;
    }
    let mut distance_squared = f64::INFINITY;
    for points in NATIVE_TUTORIAL_ENTRANCE_FENCE_CHAIN.windows(2) {
        let segment = HubSegment::new(points[0].x, points[0].y, points[1].x, points[1].y);
        let nearest = nearest_point_on_segment(position, &segment);
        let dx = position.x - nearest.x;
        let dy = position.y - nearest.y;
        distance_squared = distance_squared.min(dx * dx + dy * dy);
    }
    distance_squared >= radius * radius
}

fn entrance_fence_y(x: f64) -> f64 {
    let chain = NATIVE_TUTORIAL_ENTRANCE_FENCE_CHAIN;
    if x <= chain[0].x {
        return chain[0].y;
    }
    for points in chain.windows(2) {
        let [start, end] = [points[0], points[1]];
        if x > end.x {
            continue;
        }
        let progress = (x - start.x) / (end.x - start.x);
        return start.y + (end.y - start.y) * progress;
    }
    chain[chain.len() - 1].y
}

pub(super) fn point_inside(point: Vec2, bounds: BoneyardBounds) -> bool {
    point.x >= bounds.x
        && point.x <= bounds.x + bounds.width
        && point.y >= bounds.y
        && point.y <= bounds.y + bounds.height
}
