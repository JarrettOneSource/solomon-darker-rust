//! Discorporeal controller and private action construction from `native-demon-skull.ts`.

use super::model::{
    BossPhase, DemonSkullAction, DemonSkullActionProgram, DemonSkullBrain, DemonSkullRangedAttack,
};
use crate::float::fround;
use crate::{NativeRng, Vec2};

pub const NATIVE_DEMON_SKULL_SCREAM_TICKS: u64 = 275;
pub const NATIVE_DEMON_SKULL_DEATH_STREAM_TICKS: u64 = 600;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DemonSkullActionKind {
    Scream,
    Bite,
    Eyes,
    Mouth,
    Spit,
    Flair,
}

impl DemonSkullActionKind {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Scream => "scream",
            Self::Bite => "bite",
            Self::Eyes => "eyes",
            Self::Mouth => "mouth",
            Self::Spit => "spit",
            Self::Flair => "flair",
        }
    }
}
impl From<DemonSkullRangedAttack> for DemonSkullActionKind {
    fn from(attack: DemonSkullRangedAttack) -> Self {
        match attack {
            DemonSkullRangedAttack::Eyes => Self::Eyes,
            DemonSkullRangedAttack::Mouth => Self::Mouth,
            DemonSkullRangedAttack::Spit => Self::Spit,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeDemonSkullControllerContext {
    pub alternate_player: bool,
    pub has_action: bool,
    pub heading_deg: f64,
    pub health_ratio: f64,
    pub line_of_sight: bool,
    pub movement_scale: f64,
    pub scream_playing: bool,
    pub target_distance_squared: Option<f64>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct NativeDemonSkullControllerStep {
    pub begin_action: Option<DemonSkullActionKind>,
    pub increment_attack_seed: bool,
    pub landing: bool,
    pub scream_shake: bool,
}

#[must_use]
pub fn create_native_demon_skull(
    rng: &mut NativeRng,
    movement_scale: f64,
    capabilities: u8,
) -> DemonSkullBrain {
    let body_phase_deg = rng.draw_scaled_float(360.0, false);
    let flicker_phase_deg = rng.draw_scaled_float(360.0, false);
    DemonSkullBrain {
        attack_ticks_remaining: 500,
        body_heading_deg: -1.0,
        body_offset: Vec2::new(0.0, 25.0),
        body_phase_deg,
        body_pose: 0.0,
        capabilities,
        charge_glow: 0.0,
        eye_charge: 0.0,
        flair_glow: 0.0,
        flicker_phase_deg,
        heading_delay_ticks: 0,
        jitter: Vec2::ZERO,
        light_intensity: 0.0,
        pending_attack: None,
        recoil: Vec2::ZERO,
        scream_active: false,
        seen: false,
        speed: fround(movement_scale * 4.0),
        spin: 0.0,
        target_speed: fround(movement_scale * 4.0),
        death_countdown: 1000,
        actions: Vec::new(),
        phase: BossPhase::RangeControl,
    }
}

#[must_use]
pub fn create_native_demon_skull_action(
    kind: DemonSkullActionKind,
    heading: f64,
    target_heading: Option<f64>,
    attack_speed: f64,
    action_seed: u32,
) -> DemonSkullAction {
    let program = match kind {
        DemonSkullActionKind::Scream => DemonSkullActionProgram::Scream,
        DemonSkullActionKind::Bite => DemonSkullActionProgram::Bite {
            progress: 0.0,
            rate: fround(attack_speed * 0.125),
            marker_emitted: false,
        },
        DemonSkullActionKind::Eyes => DemonSkullActionProgram::Eyes {
            warmup_ticks: 50,
            shots_remaining: 3,
            recovery_ticks: 50,
            attack_speed,
        },
        DemonSkullActionKind::Mouth => DemonSkullActionProgram::Mouth {
            warmup_ticks: 300,
            warmup_jitter: 0.0,
            warmup_turn_speed: 1.0,
            beam_power: 10.0,
            tracking_delay_ticks: 150,
            tracking_ramp: 0.0,
            recovery_ticks: 80,
        },
        DemonSkullActionKind::Flair => DemonSkullActionProgram::Flair {
            remaining_ticks: 200,
            settling_ticks: 0,
            glow: 0.0,
            original_heading_deg: heading,
        },
        DemonSkullActionKind::Spit => {
            let mut rng = NativeRng::new(i64::from(action_seed));
            let base = rng.draw_integer(3);
            let short = rng.draw_integer(20);
            let extra = rng.draw_integer(30);
            let count = if short == 5 { 2 } else { base + 3 };
            let shots = count + if extra == 3 { rng.draw_integer(5) } else { 0 };
            DemonSkullActionProgram::Spit {
                shots_remaining: shots as usize,
                cooldown_ticks: 0,
                mouth_ticks: -1,
                target_heading_deg: target_heading.unwrap_or(heading),
            }
        }
    };
    DemonSkullAction { retired: kind == DemonSkullActionKind::Scream, program }
}

pub fn prepare_native_demon_skull_tick(brain: &mut DemonSkullBrain, admitted: bool) {
    brain.flair_glow = fround(brain.flair_glow - fround(0.05)).max(0.0);
    brain.seen |= admitted;
    brain.speed = if brain.speed > brain.target_speed {
        fround(brain.speed - fround(0.025)).max(brain.target_speed)
    } else {
        fround(brain.speed + fround(0.025)).min(brain.target_speed)
    };
}

#[must_use]
pub fn native_demon_skull_attack_choices(
    capabilities: u8,
    distance_squared: f64,
    line_of_sight: bool,
) -> Vec<DemonSkullRangedAttack> {
    let mut choices = Vec::new();
    if capabilities & 1 != 0 && line_of_sight {
        choices.push(DemonSkullRangedAttack::Eyes);
    }
    if capabilities & 2 != 0 && line_of_sight {
        choices.push(DemonSkullRangedAttack::Mouth);
        if distance_squared > 250.0 * 250.0 {
            choices.extend([DemonSkullRangedAttack::Mouth; 2]);
        }
    }
    if capabilities & 4 != 0 {
        choices.push(DemonSkullRangedAttack::Spit);
        if distance_squared > 250.0 * 250.0 {
            choices.push(DemonSkullRangedAttack::Spit);
        }
    }
    choices
}

#[must_use]
pub fn step_native_demon_skull_controller(
    brain: &mut DemonSkullBrain,
    rng: &mut NativeRng,
    context: NativeDemonSkullControllerContext,
) -> NativeDemonSkullControllerStep {
    let phase = rng.draw_float_range(3.0, 6.0);
    if brain.body_heading_deg == -1.0 {
        brain.body_heading_deg = context.heading_deg;
    }
    brain.heading_delay_ticks = if context.heading_deg == brain.body_heading_deg {
        0
    } else {
        brain.heading_delay_ticks + 1
    };
    if brain.heading_delay_ticks > 10 {
        brain.body_heading_deg = context.heading_deg;
        brain.heading_delay_ticks = 0;
    }
    brain.body_phase_deg = fround(brain.body_phase_deg + 3.0);
    brain.flicker_phase_deg = fround(brain.flicker_phase_deg + phase);
    brain.light_intensity = fround(brain.light_intensity + fround(0.005)).min(1.0);
    if brain.scream_active {
        if context.scream_playing {
            brain.body_pose = 1.0;
            brain.speed =
                fround(brain.speed + fround(0.04)).min(fround(context.movement_scale * 10.0));
            return NativeDemonSkullControllerStep { scream_shake: true, ..Default::default() };
        }
        let landing = brain.body_pose == 1.0;
        let speed = fround(brain.speed - fround(0.1));
        brain.body_pose = 0.0;
        brain.scream_active = speed >= context.movement_scale;
        brain.speed = speed.max(context.movement_scale);
        return NativeDemonSkullControllerStep { landing, ..Default::default() };
    }
    if let Some(attack) = brain.pending_attack {
        brain.speed = fround(brain.speed - fround(0.025)).max(0.0);
        if context.alternate_player || brain.speed >= fround(0.005) {
            return Default::default();
        }
        brain.pending_attack = None;
        return NativeDemonSkullControllerStep {
            begin_action: Some(attack.into()),
            ..Default::default()
        };
    }
    if !context.has_action && brain.capabilities & 32 != 0 {
        brain.capabilities &= !32;
        return NativeDemonSkullControllerStep {
            begin_action: Some(DemonSkullActionKind::Flair),
            ..Default::default()
        };
    }
    let Some(distance) = context.target_distance_squared else { return Default::default() };
    if context.has_action || context.alternate_player || !brain.seen {
        return Default::default();
    }
    if brain.speed >= brain.target_speed && distance > 400.0 * 400.0 && rng.draw_integer(100) == 50
    {
        brain.scream_active = true;
        brain.body_pose = 1.0;
        return NativeDemonSkullControllerStep {
            begin_action: Some(DemonSkullActionKind::Scream),
            ..Default::default()
        };
    }
    brain.attack_ticks_remaining -= 1;
    if brain.attack_ticks_remaining > 0 || brain.speed <= brain.target_speed * 0.75 {
        return Default::default();
    }
    let delay = rng.draw_float_range(1.5, 3.75);
    let divisor = if context.health_ratio < 0.125 {
        4.0
    } else if context.health_ratio < 0.25 {
        3.0
    } else if context.health_ratio < 0.5 {
        2.0
    } else {
        1.0
    };
    brain.attack_ticks_remaining =
        (fround(delay * 100.0).round_ties_even() / divisor).trunc() as i64;
    let choices =
        native_demon_skull_attack_choices(brain.capabilities, distance, context.line_of_sight);
    if choices.is_empty() {
        return Default::default();
    }
    brain.pending_attack = Some(choices[rng.draw_integer(choices.len() as i32) as usize]);
    brain.target_speed = fround(0.0001);
    NativeDemonSkullControllerStep { increment_attack_seed: true, ..Default::default() }
}
