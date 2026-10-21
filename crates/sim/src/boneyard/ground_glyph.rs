//! Player +28 ground glyph from `core-kernels/native-ground-auxiliary.ts`.

use super::terrain_surface::NATIVE_GROUND_PI;
use crate::Vec2;
use crate::combat::PlayerLifeState;
use crate::float::{cos, fround as f, sin};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeGroundGlyphPlan {
    pub position: Vec2,
    pub scale_x: f64,
    pub scale_y: f64,
    pub alpha: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeGroundPlayer {
    pub position: Vec2,
    pub heading_degrees: f64,
    pub scale: f64,
    pub life_state: PlayerLifeState,
    pub death_tick: u64,
    pub corpse_consumed: bool,
}

#[must_use]
pub fn native_player_ground_glyph(
    player: NativeGroundPlayer,
    special_surface: bool,
) -> Option<NativeGroundGlyphPlan> {
    let terminal =
        matches!(player.life_state, PlayerLifeState::Dying | PlayerLifeState::Spectating);
    if player.corpse_consumed || (terminal && player.death_tick > 150) {
        return None;
    }
    let radians = f(f(player.heading_degrees) * NATIVE_GROUND_PI / 180.0);
    let direction = Vec2::new(f(sin(radians)), -f(cos(radians)));
    let (distance, multiplier, alpha) =
        if special_surface { (2.0, 1.2000000476837158, 0.5) } else { (5.0, 1.25, 1.0) };
    let scale = f(player.scale * multiplier);
    Some(NativeGroundGlyphPlan {
        position: Vec2::new(
            f(player.position.x - f(direction.x * distance)),
            f(player.position.y - f(direction.y * distance)),
        ),
        scale_x: scale,
        scale_y: scale,
        alpha,
    })
}
