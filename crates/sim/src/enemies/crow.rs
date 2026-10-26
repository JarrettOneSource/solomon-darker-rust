//! Flock flight from Website `core-kernels/native-crow.ts`.
//!
//! Parent-owned birds update in flock order. A bird may enter and finish a dive in the
//! same update; the two phase branches deliberately remain independent.

use super::model::{CrowPhase, CrowState};
use crate::float;
use crate::float::fround;
use crate::heading::heading_degrees_from_vector;
use crate::spells::targeting::native_heading_turn_direction;
use crate::{NativeRng, Vec2};

#[derive(Clone, Copy, Debug)]
pub struct NativeCrowTarget<'a> {
    pub id: &'a str,
    pub position: Vec2,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeCrowStep {
    pub retained: bool,
    pub parent_cooldown_ticks: Option<u64>,
    pub return_pitch: Option<f64>,
    pub struck_target_id: Option<String>,
}

#[must_use]
pub fn create_native_crow(id: usize, rng: &mut NativeRng) -> CrowState {
    let phase = rng.draw_scaled_float(360.0, false);
    let radius = rng.draw_scaled_float(40.0, false);
    let speed = rng.draw_scaled_float(0.150_000_035_762_786_87, false);
    let glide = rng.draw_integer(201) as u64;
    let height = rng.draw_scaled_float(50.0, false);
    let age = rng.draw_integer(10_000) as u64;
    let maximum_speed = fround(speed + 0.949_999_988_079_071);
    CrowState {
        age,
        dive_speed: 0.0,
        flap_cycles: 0.0,
        flap_phase: 5.0,
        glide_ticks: glide + 100,
        heading_deg: fround(phase + 90.0),
        height: fround(height + 100.0),
        height_per_distance: 0.0,
        id,
        light_intensity: 1.0,
        maximum_speed,
        orbit_phase_deg: phase,
        orbit_radius: fround(radius + 40.0),
        phase: CrowPhase::Orbit,
        position: None,
        speed: maximum_speed,
        target_id: None,
    }
}

#[must_use]
pub fn step_native_crow(
    crow: &mut CrowState,
    rng: &mut NativeRng,
    parent_position: Option<Vec2>,
    may_attack: bool,
    targets: &[NativeCrowTarget<'_>],
    is_visible: &dyn Fn(Vec2) -> bool,
) -> NativeCrowStep {
    crow.age += 1;
    let mut parent_cooldown_ticks = None;
    let mut return_pitch = None;
    let mut struck_target_id = None;
    if let Some(parent) = parent_position {
        if crow.phase == CrowPhase::Orbit {
            step_orbit(crow, parent);
            if may_attack && crow.speed >= crow.maximum_speed && crow.age.is_multiple_of(5) {
                parent_cooldown_ticks = acquire_dive_target(crow, rng, targets);
            }
            crow.speed = crow.maximum_speed.min(fround(crow.speed + 0.025_000_000_372_529_03));
            step_orbit_wings(crow, rng);
        }
        if crow.phase == CrowPhase::Dive {
            step_dive(crow, rng, &mut return_pitch, &mut struck_target_id);
        }
    } else {
        if crow.position.is_none() {
            return NativeCrowStep {
                retained: false,
                parent_cooldown_ticks,
                return_pitch,
                struck_target_id,
            };
        }
        step_detached_crow(crow);
        let position = crow.position.expect("detached crow has a root");
        if !is_visible(Vec2::new(position.x, position.y - crow.height)) {
            return NativeCrowStep {
                retained: false,
                parent_cooldown_ticks,
                return_pitch,
                struck_target_id,
            };
        }
    }
    NativeCrowStep { retained: true, parent_cooldown_ticks, return_pitch, struck_target_id }
}

fn step_orbit(crow: &mut CrowState, parent: Vec2) {
    if crow.position.is_none() {
        let direction = crow_direction(crow.orbit_phase_deg);
        crow.position = Some(Vec2::new(
            fround(parent.x + direction.x * crow.orbit_radius),
            fround(parent.y + direction.y * crow.orbit_radius),
        ));
        crow.orbit_phase_deg = fround(crow.orbit_phase_deg + 5.0);
    }
    let radial = crow_direction(crow.orbit_phase_deg);
    let goal = Vec2::new(
        fround(parent.x + radial.x * crow.orbit_radius),
        fround(parent.y + radial.y * crow.orbit_radius),
    );
    let position = crow.position.expect("orbit initialized the root");
    let desired = heading_degrees_from_vector(goal.x - position.x, goal.y - position.y);
    let turn = f64::from(native_heading_turn_direction(crow.heading_deg, desired));
    crow.heading_deg = normalize(fround(crow.heading_deg + turn * 1.25));
    move_crow(crow, fround(crow.speed * 2.5));
    let position = crow.position.expect("orbit retains its root");
    let dx = goal.x - position.x;
    let dy = goal.y - position.y;
    if dx * dx + dy * dy < 100.0 {
        crow.orbit_phase_deg = fround(crow.orbit_phase_deg + 2.0);
    }
}

fn acquire_dive_target(
    crow: &mut CrowState,
    rng: &mut NativeRng,
    targets: &[NativeCrowTarget<'_>],
) -> Option<u64> {
    let position = crow.position.expect("orbit initialized the root");
    for target in targets {
        let dx = target.position.x - position.x;
        let dy = target.position.y - position.y;
        let distance_squared = dx * dx + dy * dy;
        if distance_squared <= 10_000.0 || distance_squared >= 22_500.0 {
            continue;
        }
        let heading = heading_degrees_from_vector(dx, dy);
        let gap = (normalize(crow.heading_deg) - normalize(heading)).abs();
        if gap.min(360.0 - gap) >= 20.0 {
            continue;
        }
        crow.target_id = Some(target.id.to_owned());
        crow.height_per_distance = fround((crow.height - 15.0) / distance_squared.sqrt());
        crow.dive_speed = fround(crow.maximum_speed * 2.5);
        crow.heading_deg = fround(heading);
        crow.phase = CrowPhase::Dive;
        crow.glide_ticks = 2;
        return Some(rng.draw_integer(76) as u64 + 50);
    }
    None
}

fn step_orbit_wings(crow: &mut CrowState, rng: &mut NativeRng) {
    if crow.glide_ticks > 0 && crow.height > 100.0 {
        crow.glide_ticks -= 1;
        crow.height = 100.0_f64.max(fround(crow.height - 0.25));
        crow.flap_phase = 5.0;
        if crow.glide_ticks == 0 {
            crow.flap_phase = fround(5.9);
            crow.flap_cycles = rng.draw_integer(4) as f64 + 2.0;
        }
        return;
    }
    if crow.height >= 100.0
        && crow.flap_phase <= 5.0
        && fround(crow.flap_phase + 0.150_000_005_960_464_48) >= 5.0
    {
        crow.flap_cycles -= 1.0;
        if crow.flap_cycles < 1.0 {
            crow.glide_ticks = rng.draw_integer(201) as u64 + 100;
        }
    }
    crow.height = 150.0_f64.min(fround(crow.height + 0.349_999_994_039_535_5));
    if crow.speed < 0.0 {
        crow.height = 175.0_f64.min(fround(crow.height + 0.5));
    }
    if crow.flap_cycles > 0.0 {
        advance_wings(crow);
    }
}

fn step_dive(
    crow: &mut CrowState,
    rng: &mut NativeRng,
    return_pitch: &mut Option<f64>,
    struck_target_id: &mut Option<String>,
) {
    crow.flap_phase = 3.0;
    move_crow(crow, crow.dive_speed);
    crow.height = fround(crow.height - crow.height_per_distance * crow.dive_speed);
    crow.dive_speed =
        fround(fround(crow.dive_speed + 0.100_000_001_490_116_12) * 1.009_999_990_463_256_8);
    crow.speed = -1.0;
    if crow.height > 15.0 {
        return;
    }
    *struck_target_id = crow.target_id.clone();
    crow.glide_ticks = 0;
    crow.flap_cycles = 5.0;
    let perturb = rng.draw_float_range(20.0, 30.0);
    let signed = rng.draw_sign(perturb);
    crow.heading_deg = fround(crow.heading_deg + signed);
    *return_pitch = Some(rng.draw_float_range(0.949_999_988_079_071, 1.049_999_952_316_284_2));
    crow.orbit_radius = rng.draw_float_range(40.0, 120.0);
    crow.orbit_phase_deg = rng.draw_scaled_float(360.0, false);
    crow.phase = CrowPhase::Orbit;
}

fn step_detached_crow(crow: &mut CrowState) {
    move_crow(crow, fround(crow.speed * 2.5));
    let turn = if crow.id.is_multiple_of(2) { -1.0 } else { 1.0 };
    crow.heading_deg = normalize(fround(crow.heading_deg + turn));
    crow.speed = fround(crow.speed + 0.019_999_999_552_965_164);
    crow.height = 150.0_f64.min(fround(crow.height + 0.5));
    if crow.speed < 0.0 {
        crow.height = 175.0_f64.min(fround(crow.height + 0.5));
    }
    advance_wings(crow);
}

fn advance_wings(crow: &mut CrowState) {
    crow.flap_phase = fround(crow.flap_phase + 0.150_000_005_960_464_48);
    if crow.height < 100.0 {
        crow.flap_phase = fround(crow.flap_phase + 0.150_000_005_960_464_48);
    }
    if crow.flap_phase >= 6.0 {
        crow.flap_phase = fround(crow.flap_phase - 6.0);
    }
}

fn move_crow(crow: &mut CrowState, distance: f64) {
    let direction = crow_direction(crow.heading_deg);
    let position = crow.position.expect("moving crow has a root");
    crow.position = Some(Vec2::new(
        fround(position.x + direction.x * distance),
        fround(position.y + direction.y * distance),
    ));
}

fn normalize(heading: f64) -> f64 {
    if heading < 0.0 {
        heading + 360.0
    } else if heading >= 360.0 {
        heading - 360.0
    } else {
        heading
    }
}

fn crow_direction(heading: f64) -> Vec2 {
    // The native crow rounds pi and the angle before calling sin/cos. Other actor
    // heading helpers intentionally keep their own, different rounding boundary.
    let radians = fround(fround(std::f64::consts::PI) * heading / 180.0);
    Vec2::new(fround(float::sin(radians)), fround(-float::cos(radians)))
}
