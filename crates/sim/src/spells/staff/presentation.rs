//! Exact stock draw values from `renderer/player-staff-vfx-presentation.ts`.
//! Pike debris replays a private presentation snapshot; gameplay RNG is untouched.

use super::{NativePlayerStaffPikeBreakVfx, NativePlayerStaffVfx};
use crate::float;
use crate::float::fround;
use crate::secondary::presentation::NativeSecondaryBlend;
use crate::{NativeRng, Vec2};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerStaffVfxRenderPlan {
    pub alpha: f64,
    pub blend: NativeSecondaryBlend,
    pub entry: usize,
    pub position: Vec2,
    pub rotation_radians: f64,
    pub scale: f64,
    /// Smoke inherits world tint; the other effects retain their element tint.
    pub tint: Option<u32>,
}

#[must_use]
pub fn native_player_staff_vfx_render_plan(
    state: &NativePlayerStaffVfx,
) -> PlayerStaffVfxRenderPlan {
    let (alpha, entry, position, rotation, scale, tint) = match state {
        NativePlayerStaffVfx::Smoke(state) => {
            (state.alpha, state.entry, state.position, state.rotation_degrees, state.scale, None)
        }
        NativePlayerStaffVfx::MoveFade(state) => (
            state.alpha,
            state.entry,
            state.position,
            state.rotation_degrees,
            state.scale,
            Some(state.tint),
        ),
        NativePlayerStaffVfx::PerspectiveFade(state) => (
            state.alpha,
            state.entry,
            state.position,
            state.rotation_degrees,
            state.scale,
            Some(state.tint),
        ),
    };
    PlayerStaffVfxRenderPlan {
        alpha,
        blend: NativeSecondaryBlend::Add,
        entry,
        position,
        rotation_radians: rotation * std::f64::consts::PI / 180.0,
        scale,
        tint,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerStaffPikeBreakDraw {
    pub alpha: f64,
    pub blend: NativeSecondaryBlend,
    pub entry: usize,
    pub offset: Vec2,
    pub role: String,
    pub rotation_radians: f64,
    pub scale_x: f64,
    pub scale_y: f64,
    pub tint: Option<u32>,
}

#[derive(Clone, Copy)]
struct PikeBreakDebris {
    alpha: f64,
    bounce_progress: f64,
    bounce_velocity: f64,
    height: f64,
    position: Vec2,
    rotation_degrees: f64,
    rotation_step_degrees: f64,
    velocity: Vec2,
    vertical_velocity: f64,
}

#[must_use]
pub fn native_player_staff_pike_break_draws(
    state: &NativePlayerStaffPikeBreakVfx,
) -> Vec<PlayerStaffPikeBreakDraw> {
    let mut draws = Vec::new();
    let mut fade_alpha = fround(1.0);
    for _ in 0..state.common.age_ticks {
        fade_alpha = fround(fade_alpha - fround(0.025));
    }
    if fade_alpha > 0.0 {
        let radians = state.heading_degrees * std::f64::consts::PI / 180.0;
        draws.push(PlayerStaffPikeBreakDraw {
            alpha: fade_alpha.min(1.0),
            blend: NativeSecondaryBlend::Add,
            entry: 15,
            offset: Vec2::new(
                fround(float::sin(radians) * 75.0),
                fround(-float::cos(radians) * 75.0),
            ),
            role: "pike-break-flash".into(),
            rotation_radians: 0.0,
            scale_x: 3.0,
            scale_y: 3.0,
            tint: None,
        });
    }
    let mut rng = state.presentation_rng.clone();
    let mut debris = create_debris(&mut rng);
    step_debris(&mut debris, &mut rng, state.common.age_ticks + 1);
    for (index, particle) in debris.into_iter().enumerate() {
        if particle.alpha <= 0.0 {
            continue;
        }
        if state.enhanced_shadow {
            draws.push(PlayerStaffPikeBreakDraw {
                alpha: particle.alpha.min(1.0),
                blend: NativeSecondaryBlend::Normal,
                entry: 55,
                offset: Vec2::new(particle.position.x, particle.position.y + 2.0),
                role: format!("pike-break-shadow-{index}"),
                rotation_radians: particle.rotation_degrees * std::f64::consts::PI / 180.0,
                scale_x: 1.0,
                scale_y: 0.75,
                tint: Some(0),
            });
        }
        draws.push(PlayerStaffPikeBreakDraw {
            alpha: particle.alpha.min(1.0),
            blend: NativeSecondaryBlend::Normal,
            entry: 55,
            offset: Vec2::new(particle.position.x, particle.position.y + particle.height),
            role: format!("pike-break-debris-{index}"),
            rotation_radians: particle.rotation_degrees * std::f64::consts::PI / 180.0,
            scale_x: 1.0,
            scale_y: 1.0,
            tint: None,
        });
    }
    draws
}

fn create_debris(rng: &mut NativeRng) -> Vec<PikeBreakDebris> {
    let mut angle = rng.draw_scaled_float(360.0, false);
    let mut debris = Vec::with_capacity(7);
    for _ in 0..7 {
        let vertical = rng.draw_scaled_float(3.0, false);
        let height = rng.draw_scaled_float(20.0, false);
        let rotation = rng.draw_scaled_float(360.0, false);
        let rotation_step = rng.draw_scaled_float(10.0, false);
        let radial = rng.draw_scaled_float(10.0, false);
        let next_angle = rng.draw_scaled_float(10.0, true);
        let radians = angle * std::f64::consts::PI / 180.0;
        let velocity = Vec2::new(fround(float::sin(radians) * 1.5), fround(-float::cos(radians)));
        let distance = fround(15.0 + radial);
        debris.push(PikeBreakDebris {
            alpha: fround(1.5),
            bounce_progress: 0.0,
            bounce_velocity: fround(-(2.0 + vertical)),
            height: fround(-height),
            position: Vec2::new(
                fround(velocity.x * fround(distance + 2.0)),
                fround(velocity.y * distance),
            ),
            rotation_degrees: rotation,
            rotation_step_degrees: fround(1.0 + rotation_step),
            velocity,
            vertical_velocity: fround(-(2.0 + vertical)),
        });
        angle = fround(angle + 55.0 + next_angle);
    }
    debris
}

fn step_debris(debris: &mut [PikeBreakDebris], rng: &mut NativeRng, updates: u64) {
    for _ in 0..updates {
        for particle in debris.iter_mut() {
            if particle.height != 0.0 {
                particle.position.x = fround(particle.position.x + particle.velocity.x);
                particle.position.y = fround(particle.position.y + particle.velocity.y);
                particle.height = fround(particle.height + 2.0 * particle.vertical_velocity);
                particle.vertical_velocity =
                    fround(particle.vertical_velocity + 2.0 * particle.bounce_progress * 0.4);
                particle.bounce_progress = fround(particle.bounce_progress + 0.02).min(1.0);
                if particle.height > 0.0 {
                    let rotation = rng.draw_scaled_float(10.0, false);
                    if rng.draw_integer(3) == 1 {
                        // Source sound selection shares this private debris stream.
                        rng.draw_scaled_float(0.2, false);
                        rng.draw_integer(4);
                    }
                    let damping = rng.draw_integer(2);
                    particle.rotation_step_degrees = fround(1.0 + rotation);
                    particle.bounce_velocity = fround(particle.bounce_velocity * 0.65);
                    particle.vertical_velocity = particle.bounce_velocity;
                    if damping == 1 {
                        particle.velocity.x = fround(particle.velocity.x * 0.65);
                        particle.velocity.y = fround(particle.velocity.y * 0.65);
                    }
                    if particle.vertical_velocity > -0.75 {
                        particle.bounce_progress = 0.0;
                        particle.bounce_velocity = 0.0;
                        particle.height = 0.0;
                        particle.vertical_velocity = 0.0;
                        particle.velocity = Vec2::ZERO;
                        particle.rotation_step_degrees = 0.0;
                    } else {
                        particle.height = particle.vertical_velocity;
                    }
                }
            }
            particle.rotation_degrees =
                fround(particle.rotation_degrees + particle.rotation_step_degrees);
            particle.alpha = fround(particle.alpha - 0.015);
        }
    }
}
