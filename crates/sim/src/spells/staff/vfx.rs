//! Ordered staff proc sounds, smoke, disabled-hit particles and source lifetimes.

use super::*;
use crate::float;
use crate::float::fround;
use crate::profile::runtime::PlayerStaffAttackOutcome;

pub struct NativeStaffContactPresentationInput<'a> {
    pub first_id: u64,
    pub action: &'a NativePlayerStaffAction,
    pub target_ids: &'a [String],
    pub target_mean: Vec2,
    pub element_tint: u32,
    pub impact_sound_pitches: &'a [f64],
    pub pike_break_sound_indexes: &'a [usize],
}

#[must_use]
pub fn create_native_staff_contact_presentation(
    input: NativeStaffContactPresentationInput<'_>,
    rng: &mut NativeRng,
) -> NativeStaffContactPresentation {
    let action = input.action;
    let mut next_id = input.first_id;
    let mut proc_sound = None;
    let mut proc_sound_pitches = Vec::new();
    let mut vfx = Vec::new();
    if !input.target_ids.is_empty() {
        match action.base().outcome {
            PlayerStaffAttackOutcome::Knockback | PlayerStaffAttackOutcome::CriticalHit => {
                let pitch = rng.draw_scaled_float(0.1, true);
                proc_sound =
                    Some(if action.base().outcome == PlayerStaffAttackOutcome::Knockback {
                        NativeStaffProcSound::Knockback
                    } else {
                        NativeStaffProcSound::CriticalHit
                    });
                proc_sound_pitches.push(fround(1.0 + pitch));
                vfx.push(NativePlayerStaffVfx::Smoke(create_smoke(next_id, action, rng)));
                next_id += 1;
                if action.base().outcome == PlayerStaffAttackOutcome::CriticalHit {
                    vfx.push(NativePlayerStaffVfx::MoveFade(create_critical_fade(
                        next_id,
                        action,
                        input.element_tint,
                    )));
                    next_id += 1;
                }
            }
            PlayerStaffAttackOutcome::DisablingHit => {
                proc_sound = Some(NativeStaffProcSound::DisableEnemy);
                proc_sound_pitches.push(1.0);
                let particles = create_disable_particles(
                    next_id,
                    action,
                    input.target_mean,
                    input.element_tint,
                    rng,
                );
                next_id += particles.len() as u64;
                for particle in particles {
                    vfx.push(NativePlayerStaffVfx::MoveFade(particle));
                }
            }
            PlayerStaffAttackOutcome::Whirl => {
                proc_sound = Some(NativeStaffProcSound::SpinAttack);
                proc_sound_pitches = vec![1.0, fround(0.9), fround(1.1)];
                let rotation = rng.draw_scaled_float(360.0, false);
                vfx.push(NativePlayerStaffVfx::PerspectiveFade(NativePlayerStaffPerspectiveVfx {
                    common: new_common(next_id, action),
                    alpha: fround(1.25),
                    alpha_loss: fround(0.1),
                    entry: 88,
                    position: action.base().origin,
                    rotation_degrees: rotation,
                    scale: 3.0,
                    tint: input.element_tint,
                }));
                next_id += 1;
            }
            PlayerStaffAttackOutcome::Normal => {}
        }
    }
    NativeStaffContactPresentation {
        event: NativePlayerStaffContactEvent {
            common: new_common(next_id, action),
            impact_sound_pitches: input.impact_sound_pitches.to_vec(),
            origin: action.base().origin,
            outcome: action.base().outcome,
            proc_sound,
            proc_sound_pitches,
            pike_break_sound_indexes: input.pike_break_sound_indexes.to_vec(),
            swoosh_pitch: action.base().swoosh_pitch,
            target_ids: input.target_ids.to_vec(),
        },
        next_id: next_id + 1,
        vfx,
    }
}

#[must_use]
pub fn create_native_staff_pike_break_vfx(
    id: u64,
    action: &NativePlayerStaffAction,
    target: &NativeStaffTarget,
    presentation_rng: NativeRng,
    heading_degrees: f64,
    enhanced_shadow: bool,
) -> NativePlayerStaffPikeBreakVfx {
    NativePlayerStaffPikeBreakVfx {
        common: new_common(id, action),
        enhanced_shadow,
        heading_degrees: normalized_degrees(heading_degrees),
        position: target.position,
        presentation_rng,
        target_id: target.id.clone(),
    }
}

#[must_use]
pub fn step_native_staff_pike_break_vfx(
    source: &NativePlayerStaffPikeBreakVfx,
) -> Option<NativePlayerStaffPikeBreakVfx> {
    if source.common.age_ticks + 1 >= NATIVE_STAFF_PIKE_BREAK_LIFETIME_TICKS {
        return None;
    }
    let mut actor = source.clone();
    actor.common.age_ticks += 1;
    Some(actor)
}

#[must_use]
pub fn step_native_staff_contact_event(
    source: &NativePlayerStaffContactEvent,
) -> Option<NativePlayerStaffContactEvent> {
    if source.common.age_ticks + 1 >= NATIVE_STAFF_CONTACT_EVENT_TICKS {
        return None;
    }
    let mut event = source.clone();
    event.common.age_ticks += 1;
    Some(event)
}

#[must_use]
pub fn step_native_player_staff_vfx(source: StaffVfxRef<'_>) -> Option<NativePlayerStaffVfx> {
    let mut actor = source.snapshot();
    match &mut actor {
        NativePlayerStaffVfx::Smoke(vfx) => {
            vfx.alpha = fround(vfx.alpha - vfx.alpha_loss);
            if vfx.alpha <= 0.0 {
                return None;
            }
            vfx.rotation_degrees =
                normalized_degrees(vfx.rotation_degrees + vfx.angular_velocity_degrees);
        }
        NativePlayerStaffVfx::MoveFade(vfx) => {
            vfx.alpha = fround(vfx.alpha - vfx.alpha_loss);
            if vfx.alpha <= 0.0 {
                return None;
            }
            vfx.position = Vec2::new(
                fround(vfx.position.x + vfx.velocity.x),
                fround(vfx.position.y + vfx.velocity.y),
            );
            vfx.velocity = Vec2::new(
                fround(vfx.velocity.x * vfx.velocity_factor),
                fround(vfx.velocity.y * vfx.velocity_factor),
            );
        }
        NativePlayerStaffVfx::PerspectiveFade(vfx) => {
            vfx.alpha = fround(vfx.alpha - vfx.alpha_loss);
            if vfx.alpha <= 0.0 {
                return None;
            }
        }
    }
    actor.common_mut().age_ticks += 1;
    Some(actor)
}

fn create_smoke(
    id: u64,
    action: &NativePlayerStaffAction,
    rng: &mut NativeRng,
) -> NativePlayerStaffSmokeVfx {
    // The factory draws a loss value, then overwrites it with its authored .05.
    // Retaining this discarded word keeps all later combat/proc draws aligned.
    rng.draw_scaled_float(0.05, false);
    let angular = rng.draw_scaled_float(2.0, false);
    let radians = action.base().heading_degrees * std::f64::consts::PI / 180.0;
    NativePlayerStaffSmokeVfx {
        common: new_common(id, action),
        alpha: 1.0,
        alpha_loss: fround(0.05),
        angular_velocity_degrees: fround((angular + 2.0) / 3.0),
        entry: 15,
        position: Vec2::new(
            fround(action.base().origin.x + float::sin(radians) * 25.0),
            fround(action.base().origin.y - float::cos(radians) * 25.0),
        ),
        rotation_degrees: 0.0,
        scale: 8.0,
    }
}

fn create_disable_particles(
    first_id: u64,
    action: &NativePlayerStaffAction,
    target_mean: Vec2,
    tint: u32,
    rng: &mut NativeRng,
) -> Vec<NativePlayerStaffMoveFadeVfx> {
    let mut angle = rng.draw_scaled_float(360.0, false);
    let mut particles = Vec::with_capacity(50);
    for index in 0..50 {
        let angle_step = rng.draw_integer(5);
        let speed = rng.draw_scaled_float(3.0, false);
        let scale = rng.draw_scaled_float(0.75, false);
        angle = fround(angle + f64::from(angle_step) + 20.0);
        let radians = angle * std::f64::consts::PI / 180.0;
        let velocity = Vec2::new(
            fround(float::sin(radians) * (speed + 3.0)),
            fround(-float::cos(radians) * (speed + 3.0)),
        );
        particles.push(NativePlayerStaffMoveFadeVfx {
            common: new_common(first_id + index, action),
            alpha: fround(1.5),
            alpha_loss: fround(0.05),
            entry: 45,
            position: Vec2::new(
                fround(target_mean.x + velocity.x * 3.0),
                fround(target_mean.y + velocity.y * 3.0),
            ),
            rotation_degrees: angle,
            scale: fround(scale + 0.25),
            tint,
            velocity,
            velocity_factor: fround(0.92),
        });
    }
    particles
}

fn create_critical_fade(
    id: u64,
    action: &NativePlayerStaffAction,
    tint: u32,
) -> NativePlayerStaffMoveFadeVfx {
    let base = action.base();
    let radians = base.heading_degrees * std::f64::consts::PI / 180.0;
    NativePlayerStaffMoveFadeVfx {
        common: new_common(id, action),
        alpha: 2.0,
        alpha_loss: fround(0.25),
        entry: 40,
        position: Vec2::new(base.origin.x, base.origin.y - 15.0),
        rotation_degrees: base.heading_degrees,
        scale: 4.0,
        tint,
        velocity: Vec2::new(fround(float::sin(radians) * 5.0), fround(-float::cos(radians) * 5.0)),
        velocity_factor: 1.0,
    }
}
