//! Faculty action and orbit clocks from `native-faculty.ts` and
//! `native-faculty-actions.ts`. The actor and spell producer own their world effects.

use super::model::{BossPhase, FacultyAction, FacultyActionKind, FacultyBrain};
use super::path::NativeEnemyPathState;
use crate::NativeRng;
use crate::float::fround;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeFacultyContext {
    pub advance_action: bool,
    pub actor_time_scale: f64,
    pub line_of_sight: bool,
    pub target_distance_squared: Option<f64>,
    pub tick: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeFacultyActionStep {
    pub action: FacultyAction,
    pub body_pose: f64,
    pub complete: bool,
    pub dispatch: bool,
    pub locked_facing: bool,
    pub marker: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeFacultyStep {
    pub marker: bool,
    pub dispatch: Option<FacultyActionKind>,
}

#[must_use]
pub fn create_native_faculty(rng: &mut NativeRng) -> FacultyBrain {
    let orbit_sign = rng.draw_sign(1.0);
    let light_phase = rng.draw_scaled_float(360.0, false);
    let gait_phase = rng.draw_scaled_float(4.0, false);
    let secondary_cooldown = rng.draw_integer(300) as i64 + 500;
    FacultyBrain {
        body_heading_deg: 0.0,
        action: None,
        attack_range: 250.0,
        body_pose: 0.0,
        completed_dispatches: 0,
        disabled_primary_ticks: 0,
        gait_phase,
        hand_mask: 0,
        heading_locked: false,
        lightning_active: false,
        light_intensity: 0.0,
        light_phase,
        orbit_sign,
        primary_poll: 0,
        secondary_cooldown,
        phase: BossPhase::RangeControl,
    }
}

pub fn register_native_faculty(brain: &mut FacultyBrain, rng: &mut NativeRng, member_count: usize) {
    brain.primary_poll = if member_count > 2 {
        rng.draw_integer(300) as u64
    } else {
        (member_count as u64 + 1) * 100
    };
    let (count, delay) = match member_count {
        1 => (301, 100),
        2 => (401, 400),
        _ => (201, 800),
    };
    brain.secondary_cooldown += delay + rng.draw_integer(count) as i64;
}

pub fn advance_native_faculty_orbit(
    path: &mut NativeEnemyPathState,
    orbit_sign: f64,
    rng: &mut NativeRng,
) {
    let angle = rng.draw_float_range(1.0, 2.0);
    let radius = rng.draw_scaled_float(2.0, true);
    path.flank_ticks_remaining = 100;
    path.flank_angle_deg = fround(path.flank_angle_deg + angle * orbit_sign);
    path.flank_radius = fround(path.flank_radius + radius).clamp(200.0, 300.0);
}

#[must_use]
pub fn create_native_faculty_action(kind: FacultyActionKind, rng: &mut NativeRng) -> FacultyAction {
    let hand = if rng.draw_integer(2) == 1 { 1 } else { 2 };
    let end =
        if kind == FacultyActionKind::Lightning { 19 + rng.draw_integer(10) as usize } else { 20 };
    let speed = rng.draw_scaled_float(0.050_000_000_745_058_06, false);
    let fast = rng.draw_integer(8);
    let mut rate = fround(0.100_000_001_490_116_12 + speed);
    if fast == 2 {
        rate = fround(rate * 1.350_000_023_841_858);
    }
    let mut frames = vec![f64::from(hand); 14];
    frames.push(0.0);
    frames.extend(vec![if hand == 1 { 2.0 } else { 1.0 }; end - 15]);
    frames.push(0.0);
    let two_hand = kind == FacultyActionKind::TwoHand;
    if two_hand {
        frames = vec![3.0; 20];
        frames.extend(vec![4.0; 8]);
        frames.push(0.0);
    }
    FacultyAction {
        end: if two_hand { 28.0 } else { end as f64 },
        frames,
        hand_mask: if two_hand { 3 } else { hand },
        kind,
        marker: if two_hand { 20.0 } else { 15.0 },
        progress: 0.0,
        rate,
    }
}

#[must_use]
pub fn step_native_faculty_action(
    source: &FacultyAction,
    actor_time_scale: f64,
) -> NativeFacultyActionStep {
    let progress = fround(source.progress + fround(source.rate * actor_time_scale));
    // Retail admits both marker endpoints, including a repeated exact-boundary hit.
    let marker =
        progress > source.progress && source.progress <= source.marker && progress >= source.marker;
    let mut action = source.clone();
    action.progress = progress;
    NativeFacultyActionStep {
        body_pose: source.frames.get(progress.trunc() as usize).copied().unwrap_or(0.0),
        complete: progress > source.end,
        dispatch: if source.kind == FacultyActionKind::Lightning {
            progress >= source.marker && progress < source.end
        } else {
            marker
        },
        locked_facing: source.kind == FacultyActionKind::Lightning && progress >= 9.0,
        action,
        marker,
    }
}

#[must_use]
pub fn step_native_faculty(
    brain: &mut FacultyBrain,
    rng: &mut NativeRng,
    primary: u8,
    context: NativeFacultyContext,
) -> NativeFacultyStep {
    brain.gait_phase = fround(brain.gait_phase + 0.25);
    if brain.gait_phase >= 5.0 {
        brain.gait_phase = fround(brain.gait_phase - 5.0);
    }
    brain.lightning_active = false;
    brain.disabled_primary_ticks = brain.disabled_primary_ticks.saturating_sub(1);
    brain.light_phase = fround(brain.light_phase + 1.0);
    brain.light_intensity = fround(brain.light_intensity + 0.001_000_000_047_497_451_3).min(1.0);
    let mut marker = false;
    let mut dispatch = None;
    if let Some(source_action) = &brain.action
        && context.advance_action
    {
        let step = step_native_faculty_action(source_action, context.actor_time_scale);
        marker = step.marker;
        dispatch = step.dispatch.then_some(source_action.kind);
        brain.lightning_active =
            source_action.kind == FacultyActionKind::Lightning && step.dispatch;
        brain.body_pose = step.body_pose;
        brain.completed_dispatches += usize::from(step.dispatch);
        if step.complete || (step.dispatch && source_action.kind != FacultyActionKind::Lightning) {
            brain.hand_mask = 0;
        }
        brain.heading_locked = !step.complete && step.locked_facing;
        brain.action = if step.complete { None } else { Some(step.action) };
    }
    poll_primary(brain, rng, primary, context);
    if brain.completed_dispatches > 0 {
        brain.secondary_cooldown -= 1;
    }
    NativeFacultyStep { marker, dispatch }
}

fn poll_primary(
    brain: &mut FacultyBrain,
    rng: &mut NativeRng,
    primary: u8,
    context: NativeFacultyContext,
) {
    let Some(distance_squared) = context.target_distance_squared else { return };
    if !context.tick.is_multiple_of(2) {
        return;
    }
    let ready = brain.primary_poll == 0;
    brain.primary_poll = if ready { 150 } else { brain.primary_poll - 1 };
    if !ready
        || brain.disabled_primary_ticks > 0
        || !context.line_of_sight
        || distance_squared <= 150.0 * 150.0
        || distance_squared >= brain.attack_range * brain.attack_range
    {
        return;
    }
    let mut kind =
        if primary == 1 { FacultyActionKind::Lightning } else { FacultyActionKind::Throw };
    if brain.secondary_cooldown < 0 && rng.draw_integer(2) == 1 {
        kind = FacultyActionKind::TwoHand;
        brain.secondary_cooldown = 1500 + rng.draw_integer(1001) as i64;
    }
    let action = create_native_faculty_action(kind, rng);
    let range = rng.draw_scaled_float(250.0, false);
    brain.attack_range = fround(350.0 + range);
    brain.primary_poll = 300;
    brain.hand_mask = action.hand_mask;
    brain.action = Some(action);
}
