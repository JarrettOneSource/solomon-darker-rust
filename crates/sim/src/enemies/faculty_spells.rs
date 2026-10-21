//! Faculty projectile, rain and contact kernels from `native-faculty-spells.ts`, plus
//! the Dire Fire state from `native-boss-spell.ts`.

use crate::float;
use crate::float::fround;
use crate::spells::targeting::native_point_in_polygon;
use crate::{NativeRng, Vec2};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeDarkFireballState {
    pub age_ticks: u64,
    pub arc_phase: f64,
    pub arc_phase_step: f64,
    pub heading_deg: f64,
    pub position: Vec2,
    pub remaining_ticks: f64,
    pub velocity: Vec2,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeDarkFireballStep {
    pub impact: bool,
    pub spell: NativeDarkFireballState,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeRainOfBonesState {
    pub alpha: f64,
    pub phase: f64,
    pub position: Vec2,
    pub remaining_ticks: i64,
    pub rotation_sign: f64,
    pub scale: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeRainBone {
    pub entry: usize,
    pub position: Vec2,
    pub rotation_deg: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeRainOfBonesStep {
    pub bone: Option<NativeRainBone>,
    pub state: NativeRainOfBonesState,
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NativeDireFireState {
    pub alpha: f64,
    pub glow: bool,
    pub phase: f64,
    pub ramp: f64,
    pub rotation_phases: [f64; 2],
    pub rotation_rates: [f64; 2],
    pub rotation_signs: [f64; 2],
    pub scale: f64,
    pub scale_sign: f64,
}

#[must_use]
pub fn native_blightning_contains(origin: Vec2, heading: f64, point: Vec2) -> bool {
    let direction = heading_vector(heading);
    let corner = |forward, across| {
        Vec2::new(
            fround(origin.x + direction.x * forward + direction.y * across),
            fround(origin.y + direction.y * forward - direction.x * across),
        )
    };
    native_point_in_polygon(
        point,
        &[corner(0.0, 20.0), corner(0.0, -20.0), corner(1000.0, -30.0), corner(1000.0, 30.0)],
    )
}

#[must_use]
pub fn native_tragic_circle_contains(origin: Vec2, point: Vec2) -> bool {
    let x = (origin.x - point.x) / 212.0;
    let y = (origin.y - point.y) / (fround(210.0 * 0.800_000_011_920_929) + 2.0);
    x * x + y * y < 1.0
}

#[must_use]
pub fn create_native_dark_fireballs(
    origin: Vec2,
    heading: f64,
    ring: bool,
    target_distance: Option<f64>,
    rng: &mut NativeRng,
) -> Vec<NativeDarkFireballState> {
    if !ring {
        let direction = heading_vector(heading);
        return vec![NativeDarkFireballState {
            age_ticks: 0,
            arc_phase: 0.0,
            arc_phase_step: 0.0,
            heading_deg: heading,
            position: Vec2::new(
                fround(origin.x + direction.x * 55.0),
                fround(origin.y + direction.y * 55.0 - 30.0),
            ),
            remaining_ticks: 400.0,
            velocity: Vec2::new(fround(direction.x * 5.0), fround(direction.y * 5.0)),
        }];
    }
    let phase = rng.draw_scaled_float(360.0, false);
    let mut spells = Vec::new();
    for offset in (0..360).step_by(27) {
        let perturbation = rng.draw_scaled_float(10.0, true);
        let heading_deg = fround(f64::from(offset) + phase + perturbation);
        let direction = heading_vector(heading_deg);
        let mut arc_phase_step = 0.0;
        if let Some(target_distance) = target_distance {
            let trial = rng.draw_scaled_float(100.0, true);
            let mut distance = 200.0;
            if target_distance + trial >= 200.0 {
                // The repeated retail expression draws a second perturbation here.
                distance = target_distance + rng.draw_scaled_float(100.0, true);
            }
            arc_phase_step = fround(180.0 / (distance / 3.0));
        }
        spells.push(NativeDarkFireballState {
            age_ticks: 0,
            arc_phase: 0.0,
            arc_phase_step,
            heading_deg,
            position: Vec2::new(
                fround(origin.x + direction.x * 50.0),
                fround(origin.y + direction.y * 50.0 - 30.0),
            ),
            remaining_ticks: 400.0,
            velocity: Vec2::new(fround(direction.x * 3.0), fround(direction.y * 3.0)),
        });
    }
    spells
}

#[must_use]
pub fn step_native_dark_fireball(
    source: NativeDarkFireballState,
    actor_time_scale: f64,
) -> NativeDarkFireballStep {
    let arc_phase = fround(source.arc_phase + source.arc_phase_step);
    NativeDarkFireballStep {
        impact: source.arc_phase_step > 0.0 && arc_phase >= 180.0,
        spell: NativeDarkFireballState {
            age_ticks: source.age_ticks + 1,
            arc_phase,
            position: Vec2::new(
                fround(source.position.x + fround(source.velocity.x * actor_time_scale)),
                fround(source.position.y + fround(source.velocity.y * actor_time_scale)),
            ),
            remaining_ticks: fround(source.remaining_ticks - actor_time_scale),
            ..source
        },
    }
}

#[must_use]
pub fn create_native_rain_of_bones(position: Vec2, rng: &mut NativeRng) -> NativeRainOfBonesState {
    let rotation_sign = rng.draw_sign(1.0);
    NativeRainOfBonesState {
        alpha: 0.0,
        phase: 0.0,
        position,
        remaining_ticks: 1200,
        rotation_sign,
        scale: fround(0.01),
    }
}

#[must_use]
pub fn step_native_rain_of_bones(
    source: NativeRainOfBonesState,
    rng: &mut NativeRng,
) -> NativeRainOfBonesStep {
    let remaining_ticks = source.remaining_ticks - 1;
    let alpha = if remaining_ticks > 0 {
        fround(source.alpha + 0.050_000_000_745_058_06).min(1.0)
    } else {
        fround(source.alpha - 0.009_999_999_776_482_582)
    };
    let state = NativeRainOfBonesState {
        alpha,
        remaining_ticks,
        phase: fround(source.phase + 1.0),
        scale: if remaining_ticks > 0 {
            fround(source.scale * 1.200_000_047_683_715_8).min(1.0)
        } else {
            source.scale
        },
        ..source
    };
    let mut bone = None;
    if remaining_ticks > 0 && roll_rain_drop(alpha, rng) {
        let skull = rng.draw_integer(30);
        let entry = rng.draw_integer(if skull == 3 { 4 } else { 9 });
        let radius = rng.draw_scaled_float(200.0, false);
        let angle = rng.draw_scaled_float(360.0, false);
        let rotation_deg = rng.draw_scaled_float(360.0, false);
        let direction = heading_vector(angle);
        bone = Some(NativeRainBone {
            entry: (if skull == 3 { 1819 } else { 113 }) + entry as usize,
            position: Vec2::new(
                fround(source.position.x + fround(radius * direction.x)),
                fround(
                    source.position.y
                        + fround(fround(radius * direction.y) * 0.800_000_011_920_929),
                ),
            ),
            rotation_deg,
        });
    }
    if alpha >= 1.0 {
        // Two private render-manager billows consume eight words each at full alpha.
        for _ in 0..16 {
            rng.draw_word();
        }
    }
    NativeRainOfBonesStep { bone, state }
}

fn roll_rain_drop(alpha: f64, rng: &mut NativeRng) -> bool {
    let mut admitted = false;
    if alpha > 0.75 {
        admitted = rng.draw_integer(2) == 1;
    }
    if alpha >= 1.0 {
        admitted = true;
    }
    if alpha > 0.5 {
        admitted = rng.draw_integer(4) == 3;
    }
    admitted
}

#[must_use]
pub fn create_native_dire_fire(rng: &mut NativeRng) -> NativeDireFireState {
    let phase = rng.draw_scaled_float(32.0, false);
    let scale_sign = rng.draw_sign(1.0);
    let first_phase = rng.draw_scaled_float(360.0, false);
    let second_phase = rng.draw_scaled_float(360.0, false);
    let first_rate = rng.draw_scaled_float(0.5, false);
    let second_rate = rng.draw_scaled_float(0.5, false);
    let first_sign = rng.draw_sign(1.0);
    let second_sign = rng.draw_sign(1.0);
    NativeDireFireState {
        alpha: 2.0,
        glow: true,
        phase,
        ramp: 0.0,
        rotation_phases: [first_phase, second_phase],
        rotation_rates: [fround(0.5 + first_rate), fround(0.5 + second_rate)],
        rotation_signs: [first_sign, second_sign],
        scale: 1.0,
        scale_sign,
    }
}

#[must_use]
pub fn step_native_dire_fire(
    source: NativeDireFireState,
    rng: &mut NativeRng,
) -> NativeDireFireState {
    let first = rng.draw_scaled_float(0.200_000_002_980_232_24, false);
    let second = rng.draw_scaled_float(0.200_000_002_980_232_24, false);
    let rate =
        |current, draw| fround(current + fround(draw - 0.100_000_001_490_116_12)).clamp(-1.0, 1.0);
    let mut phase = fround(source.phase + 0.25);
    if phase >= 32.0 {
        phase = fround(phase - 32.0);
    }
    NativeDireFireState {
        alpha: fround(source.alpha - 0.009_999_999_776_482_582),
        phase,
        ramp: fround(source.ramp + 0.050_000_000_745_058_06).min(1.0),
        rotation_phases: [
            fround(source.rotation_phases[0] + source.rotation_rates[0]),
            fround(source.rotation_phases[1] + source.rotation_rates[1]),
        ],
        rotation_rates: [
            rate(source.rotation_rates[0], first),
            rate(source.rotation_rates[1], second),
        ],
        ..source
    }
}

#[must_use]
pub fn heading_vector(heading: f64) -> Vec2 {
    let radians = heading * std::f64::consts::PI / 180.0;
    Vec2::new(fround(float::sin(radians)), fround(-float::cos(radians)))
}
