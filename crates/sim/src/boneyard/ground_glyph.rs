//! The player's +28 ground glyph, drawn a few units behind the wizard, opposite its facing.
//! It disappears late in the death animation or once the corpse is consumed.

use super::terrain_surface::GROUND_PI;
use crate::Vec2;
use crate::combat::PlayerLifeState;
use crate::float::{cos, fround as f, sin};

/// Where and how to draw a player's ground glyph: position, scale and alpha.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GroundGlyphPlan {
    pub position: Vec2,
    pub scale_x: f64,
    pub scale_y: f64,
    pub alpha: f64,
}

/// The player values a ground glyph depends on: position, facing, scale and death progress.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GroundPlayer {
    pub position: Vec2,
    pub heading_degrees: f64,
    pub scale: f64,
    pub life_state: PlayerLifeState,
    pub death_tick: u64,
    pub corpse_consumed: bool,
}

#[must_use]
pub fn player_ground_glyph(player: GroundPlayer, special_surface: bool) -> Option<GroundGlyphPlan> {
    let terminal =
        matches!(player.life_state, PlayerLifeState::Dying | PlayerLifeState::Spectating);
    if player.corpse_consumed || (terminal && player.death_tick > 150) {
        return None;
    }
    let radians = f(f(player.heading_degrees) * GROUND_PI / 180.0);
    let direction = Vec2::new(f(sin(radians)), -f(cos(radians)));
    let (distance, multiplier, alpha) =
        if special_surface { (2.0, 1.2000000476837158, 0.5) } else { (5.0, 1.25, 1.0) };
    let scale = f(player.scale * multiplier);
    Some(GroundGlyphPlan {
        position: Vec2::new(
            f(player.position.x - f(direction.x * distance)),
            f(player.position.y - f(direction.y * distance)),
        ),
        scale_x: scale,
        scale_y: scale,
        alpha,
    })
}
