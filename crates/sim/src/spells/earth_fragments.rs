//! Registered lit-rock fragment programs from `core-kernels/primary-spell-earth.ts`.

use super::earth::earth_visual_random_int;
use super::visual::visual_unit_random;
use crate::Vec2;
use crate::float;
use crate::float::fround;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EarthImpactSeed {
    pub birth_tick: u64,
    pub charge: f64,
    pub enhanced_effects: bool,
    pub id: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EarthFragmentState {
    pub alpha: f64,
    pub height: f64,
    pub index: usize,
    pub position: Vec2,
    pub record: usize,
    pub rotation: f64,
    pub scale: f64,
    pub velocity: Vec2,
    pub vertical_velocity: f64,
}

#[derive(Clone, Copy)]
struct EarthFragment {
    frame: EarthFragmentState,
    alive: bool,
    bounce_count: u32,
    bounce_seed: f64,
    rotation_step: f64,
}

#[must_use]
pub fn earth_impact_fragment_count(charge: f64) -> usize {
    (30.0 * charge).max(8.0).floor() as usize
}

#[must_use]
pub fn earth_impact_lifetime_ticks(seed: EarthImpactSeed) -> u64 {
    let mut fragments = create_fragments(seed);
    let mut age = 0;
    while fragments.iter().any(|fragment| fragment.alive) {
        age += 1;
        for fragment in &mut fragments {
            advance_fragment(fragment, seed, seed.birth_tick + age);
        }
    }
    age
}

#[must_use]
pub fn earth_impact_fragments_at_age(
    seed: EarthImpactSeed,
    age_ticks: f64,
) -> Vec<EarthFragmentState> {
    let whole_ticks = age_ticks.floor().max(0.0) as u64;
    let blend = (age_ticks - whole_ticks as f64).clamp(0.0, 1.0);
    let mut fragments = create_fragments(seed);
    for tick in 1..=whole_ticks {
        for fragment in &mut fragments {
            advance_fragment(fragment, seed, seed.birth_tick + tick);
        }
    }
    let mut visible = Vec::new();
    for fragment in fragments {
        if !fragment.alive {
            continue;
        }
        let mut after = fragment;
        if blend > 0.0 {
            advance_fragment(&mut after, seed, seed.birth_tick + whole_ticks + 1);
        }
        let frame = if blend == 0.0 || !after.alive {
            fragment.frame
        } else {
            interpolate_fragment(fragment.frame, after.frame, blend)
        };
        visible.push(frame);
    }
    visible
}

fn create_fragments(seed: EarthImpactSeed) -> Vec<EarthFragment> {
    let count = earth_impact_fragment_count(seed.charge);
    let q = seed.charge.min(1.0);
    let radius = (30.0 * seed.charge).max(8.0);
    let angle_step = 360.0 / radius;
    let mut angle = visual_unit_random(seed.id, 0x8000) * 360.0;
    let mut fragments = Vec::with_capacity(count);
    for index in 0..count {
        let radians = angle * std::f64::consts::PI / 180.0;
        let direction = Vec2::new(float::cos(radians), float::sin(radians) * 0.8);
        let bounce_seed = fround(-(visual_unit_random(seed.id, lane(index, 0)) * 3.0 + 2.0));
        let vertical_scale = visual_unit_random(seed.id, lane(index, 4)) * 1.5 * q + 0.75;
        let vertical_velocity = fround(bounce_seed * vertical_scale);
        let spawn_distance = visual_unit_random(seed.id, lane(index, 6)) * 45.0 * seed.charge;
        let speed = visual_unit_random(seed.id, lane(index, 9)) * 1.5 * seed.charge + 1.5;
        let scale_probe = (visual_unit_random(seed.id, lane(index, 7)) * 0.75 + 0.5) * seed.charge;
        let scale_source = if scale_probe >= 0.45_f32 as f64 {
            (visual_unit_random(seed.id, lane(index, 8)) * 0.75 + 0.5) * seed.charge
        } else {
            0.45_f32 as f64
        };
        let frame = EarthFragmentState {
            alpha: if seed.enhanced_effects { 10.0 } else { 2.0 },
            height: fround(-visual_unit_random(seed.id, lane(index, 5)) * 50.0 * q),
            index,
            position: Vec2::new(
                fround(direction.x * spawn_distance),
                fround(direction.y * spawn_distance),
            ),
            record: [2008, 2009, 2010]
                [earth_visual_random_int(seed.id, lane(index, 3), 3) as usize],
            rotation: fround(visual_unit_random(seed.id, lane(index, 1)) * 360.0),
            scale: fround(scale_source * 0.65_f32 as f64).min(0.75),
            velocity: Vec2::new(fround(direction.x * speed), fround(direction.y * speed)),
            vertical_velocity,
        };
        fragments.push(EarthFragment {
            frame,
            alive: true,
            bounce_count: 0,
            bounce_seed: vertical_velocity,
            rotation_step: fround(visual_unit_random(seed.id, lane(index, 2)) * 10.0 + 1.0),
        });
        let jitter = (visual_unit_random(seed.id, lane(index, 10)) * 2.0 - 1.0) * angle_step / 3.0;
        angle = fround(angle + angle_step + jitter);
    }
    fragments
}

fn advance_fragment(fragment: &mut EarthFragment, seed: EarthImpactSeed, global_tick: u64) {
    if !fragment.alive {
        return;
    }
    let motion_active = fragment.frame.height != 0.0;
    if !(motion_active && global_tick.is_multiple_of(3)) {
        if motion_active {
            let frame = &mut fragment.frame;
            frame.position.x = fround(frame.position.x + frame.velocity.x);
            frame.position.y = fround(frame.position.y + frame.velocity.y);
            frame.height = fround(frame.height + frame.vertical_velocity);
            frame.vertical_velocity = fround(frame.vertical_velocity + 0.4_f32 as f64);
            if frame.height > 0.0 {
                fragment.rotation_step = fround(
                    visual_unit_random(seed.id, bounce_lane(frame.index, fragment.bounce_count, 0))
                        * 10.0
                        + 1.0,
                );
                fragment.bounce_seed = fround(fragment.bounce_seed * 0.3_f32 as f64);
                frame.vertical_velocity = fragment.bounce_seed;
                if earth_visual_random_int(
                    seed.id,
                    bounce_lane(frame.index, fragment.bounce_count, 1),
                    2,
                ) == 1
                {
                    frame.velocity.x = fround(frame.velocity.x * 0.65_f32 as f64);
                    frame.velocity.y = fround(frame.velocity.y * 0.65_f32 as f64);
                }
                fragment.bounce_count += 1;
                if frame.vertical_velocity > -0.75 {
                    fragment.bounce_seed = 0.0;
                    frame.vertical_velocity = 0.0;
                    frame.velocity = Vec2::ZERO;
                    fragment.rotation_step = 0.0;
                }
                frame.height = frame.vertical_velocity;
            }
            frame.rotation = fround(frame.rotation + fragment.rotation_step);
        }
        fragment.frame.alpha = fround(fragment.frame.alpha - 0.015_f32 as f64);
    }
    fragment.frame.alpha = fround(fragment.frame.alpha - 0.025_f32 as f64);
    fragment.alive = fragment.frame.alpha > 0.0;
}

fn interpolate_fragment(
    first: EarthFragmentState,
    second: EarthFragmentState,
    blend: f64,
) -> EarthFragmentState {
    let mix = |first: f64, second: f64| first + (second - first) * blend;
    EarthFragmentState {
        alpha: mix(first.alpha, second.alpha),
        height: mix(first.height, second.height),
        index: first.index,
        position: Vec2::new(
            mix(first.position.x, second.position.x),
            mix(first.position.y, second.position.y),
        ),
        record: first.record,
        rotation: mix(first.rotation, second.rotation),
        scale: first.scale,
        velocity: Vec2::new(
            mix(first.velocity.x, second.velocity.x),
            mix(first.velocity.y, second.velocity.y),
        ),
        vertical_velocity: mix(first.vertical_velocity, second.vertical_velocity),
    }
}

fn lane(index: usize, sample: u32) -> u32 {
    0x9000_u32.wrapping_add(index as u32 * 16).wrapping_add(sample)
}
fn bounce_lane(index: usize, bounce: u32, sample: u32) -> u32 {
    0xc000_u32.wrapping_add(index as u32 * 32).wrapping_add(bounce * 2).wrapping_add(sample)
}
