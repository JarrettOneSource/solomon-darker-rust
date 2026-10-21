//! Gameplay viewport from `renderer/game-viewport.ts`. Small windows scale the
//! native minimum down; larger windows reveal more world instead of enlarging UI.

use crate::Vec2;

pub const GAME_VIEWPORT_MIN_SIZE: Vec2 = Vec2::new(1600.0, 900.0);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GameViewport {
    pub display_scale: f64,
    pub logical_size: Vec2,
    pub world_zoom: f64,
}

#[must_use]
pub fn game_viewport(size: Vec2) -> GameViewport {
    if !valid_size(size) {
        return GameViewport {
            display_scale: 1.0,
            logical_size: GAME_VIEWPORT_MIN_SIZE,
            world_zoom: 1.0,
        };
    }
    let display_scale =
        (size.x / GAME_VIEWPORT_MIN_SIZE.x).min(size.y / GAME_VIEWPORT_MIN_SIZE.y).min(1.0);
    GameViewport {
        display_scale,
        logical_size: Vec2::new(size.x / display_scale, size.y / display_scale),
        world_zoom: 1.0,
    }
}

#[must_use]
pub fn bounded_game_viewport(size: Vec2, world_size: Vec2, camera_zoom: f64) -> GameViewport {
    let mut layout = game_viewport(size);
    if valid_size(size) {
        let maximum_width = GAME_VIEWPORT_MIN_SIZE.x.max(world_size.x * camera_zoom);
        let maximum_height = GAME_VIEWPORT_MIN_SIZE.y.max(world_size.y * camera_zoom);
        let bounded_scale =
            layout.display_scale.max(size.x / maximum_width).max(size.y / maximum_height);
        layout.world_zoom = bounded_scale / layout.display_scale;
    }
    layout
}

fn valid_size(size: Vec2) -> bool {
    size.x.is_finite() && size.y.is_finite() && size.x > 0.0 && size.y > 0.0
}
