//! Native belt pull-off threshold and smoke/move-fade members from current
//! Website `skill-book-model.ts`. Random values are cosmetic caller inputs.

use crate::Vec2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeBeltPullOffStep {
    Degrees90,
    Degrees120,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeBeltPullOffBurstMember {
    pub brightness: f64,
    pub duration_ms: f64,
    pub end: Vec2,
    pub record: usize,
    pub rotation_degrees: f64,
    pub scale: Vec2,
    pub start: Vec2,
}

#[must_use]
pub fn native_belt_pull_off_started(origin: Vec2, current: Vec2) -> bool {
    let dx = current.x - origin.x;
    let dy = current.y - origin.y;
    dx * dx + dy * dy > 50.0 * 50.0
}

/// All 24 smoke puffs and the source's three/four move-fade spokes. The view
/// chooses the 90/120-degree branch separately, exactly as the React component.
#[must_use]
pub fn native_belt_pull_off_burst_members(
    step: NativeBeltPullOffStep,
    random: &mut impl FnMut() -> f64,
) -> Vec<NativeBeltPullOffBurstMember> {
    let mut members = Vec::new();
    for lane in (0..360).step_by(30) {
        members.push(smoke_puff(f64::from(lane), false, random));
        members.push(smoke_puff(f64::from(lane), true, random));
    }
    let step_degrees = match step {
        NativeBeltPullOffStep::Degrees90 => 90,
        NativeBeltPullOffStep::Degrees120 => 120,
    };
    for lane in (0..360).step_by(step_degrees) {
        let extent = step_degrees as f64 / 3.0;
        let angle = f64::from(lane) + (random_unit(random) * 2.0 - 1.0) * extent;
        let direction = screen_direction(angle);
        let radius = 20.0 + random_unit(random) * 2.0;
        let fade_per_tick = 0.05 + random_unit(random) * 0.05;
        let ticks = (1.0 / fade_per_tick).ceil();
        let velocity_x = direction.x * 3.25 + random_unit(random) * 0.5;
        let velocity_y = direction.y * 3.25 + random_unit(random) * 0.5;
        let travel = geometric_travel(0.94, ticks);
        members.push(NativeBeltPullOffBurstMember {
            brightness: 1.0,
            duration_ms: ticks * 10.0,
            end: Vec2::new(
                direction.x * radius + velocity_x * travel,
                direction.y * radius + velocity_y * travel,
            ),
            record: 69,
            rotation_degrees: 0.0,
            scale: Vec2::new(1.0 - random_unit(random) * 0.1, 1.0 - random_unit(random) * 0.1),
            start: Vec2::new(direction.x * radius, direction.y * radius),
        });
    }
    members
}

fn smoke_puff(
    lane: f64,
    inner: bool,
    random: &mut impl FnMut() -> f64,
) -> NativeBeltPullOffBurstMember {
    let direction = screen_direction(lane + random_unit(random) * 10.0);
    let radius = (if inner { 8.0 } else { 15.0 }) - random_unit(random) * 2.0;
    let speed = (2.5 + random_unit(random) * 0.1) * if inner { 0.5 } else { 1.0 };
    let fade_per_tick = (0.05 + random_unit(random) * 0.05) * if inner { 2.0 } else { 1.0 };
    let ticks = (1.0 / fade_per_tick).ceil();
    let travel = geometric_travel(0.910000026, ticks);
    let scale = 1.25 - random_unit(random) * 0.4;
    let rotation_per_tick = (2.0 + random_unit(random) * 2.0) / 3.0;
    NativeBeltPullOffBurstMember {
        brightness: if inner { 0.75 } else { 0.5 },
        duration_ms: ticks * 10.0,
        end: Vec2::new(
            direction.x * (radius + speed * travel),
            direction.y * (radius + speed * travel),
        ),
        record: 65,
        rotation_degrees: rotation_per_tick * ticks * 180.0 / std::f64::consts::PI,
        scale: Vec2::new(scale, scale),
        start: Vec2::new(direction.x * radius, direction.y * radius),
    }
}

fn screen_direction(angle_degrees: f64) -> Vec2 {
    let radians = angle_degrees * std::f64::consts::PI / 180.0;
    Vec2::new(radians.cos(), -radians.sin())
}

fn geometric_travel(decay: f64, ticks: f64) -> f64 {
    (1.0 - decay.powf(ticks)) / (1.0 - decay)
}

fn random_unit(random: &mut impl FnMut() -> f64) -> f64 {
    random().clamp(0.0, 1.0)
}
