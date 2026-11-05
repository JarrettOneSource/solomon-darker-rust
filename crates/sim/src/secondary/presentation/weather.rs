//! Source storm composites, rain gradients and deterministic Freeze snow.
//! Freeze children follow `renderer/native-freeze-wave-presentation.ts`.

use super::draws::*;
use super::types::*;
use crate::Vec2;
use crate::float;
use crate::float::fround;
use crate::secondary::NativeSecondaryActorState;

#[must_use]
pub fn storm_cloud_draws(
    actor: &NativeSecondaryActorState,
    frame: f64,
) -> Vec<NativeSecondarySpriteDraw> {
    storm_cloud_draws_at_age(actor, actor.age_ticks as f64, frame, actor.enhanced)
}

pub(super) fn storm_cloud_draws_at_age(
    actor: &NativeSecondaryActorState,
    age_ticks: f64,
    frame: f64,
    enhanced_effects: bool,
) -> Vec<NativeSecondarySpriteDraw> {
    if actor.variant != 1 || !enhanced_effects || actor.presentation_rng.is_none() {
        return Vec::new();
    }
    let mut rng = actor.presentation_rng.clone().unwrap();
    let visual_phase = rng.draw_scaled_float(1.0, true);
    let mut angles = [0.0; 15];
    let mut speeds = [0.0; 15];
    for index in 0..15 {
        angles[index] = rng.draw_scaled_float(360.0, false);
        let speed = rng.draw_scaled_float(2.0, false);
        speeds[index] = fround((1.0 - index as f64 / 15.0 * 0.95) * (2.0 + speed) * 4.0);
    }
    let middle = degrees_to_radians(frame * 0.5);
    let spline = ThreePointSpline::new(
        Vec2::ZERO,
        Vec2::new(float::cos(middle) * 30.0, -float::sin(middle) * 30.0),
        Vec2::new(0.0, -175.0),
    );
    let mut draws = Vec::new();
    let mut scale = 0.2;
    for index in 0..15 {
        let time = index as f64 * 0.2;
        let angle = fround(angles[index] + speeds[index] * age_ticks);
        for (pass, offset, rotation) in
            [("a", spline.sample(time), angle), ("b", spline.sample(time + 0.1), angle * 1.35)]
        {
            let mut draw = NativeSecondarySpriteDraw::new(actor, NativeSecondaryAtlas::BadGuys, 84);
            draw.offset = offset;
            draw.role = format!("storm-cloud-arc-{index}-{pass}");
            draw.rotation_radians = degrees_to_radians(rotation);
            draw.scale_x = scale;
            draw.scale_y = scale * 0.8;
            draw.tint = 0xccffff;
            draws.push(draw);
        }
        scale = scale * 1.1 + 0.1;
    }
    let mut draw = NativeSecondarySpriteDraw::new(actor, NativeSecondaryAtlas::BadGuys, 78);
    draw.alpha = actor.alpha * 0.5;
    draw.offset = Vec2::new(0.0, actor.scale * -50.0);
    draw.role = "storm-cloud-core".into();
    draw.rotation_radians = degrees_to_radians(age_ticks * 0.5 * visual_phase * 15.0);
    draw.scale_x = actor.scale * 3.75;
    draw.scale_y = draw.scale_x;
    draws.push(draw);
    draws
}

#[must_use]
pub fn storm_auxiliary_draws(actor: &NativeSecondaryActorState) -> Vec<NativeSecondarySpriteDraw> {
    storm_auxiliary_draws_at_age(actor, actor.age_ticks as f64)
}

pub(super) fn storm_auxiliary_draws_at_age(
    actor: &NativeSecondaryActorState,
    age_ticks: f64,
) -> Vec<NativeSecondarySpriteDraw> {
    let phase = storm_visual_phase(actor);
    let mut draws = Vec::new();
    if actor.variant == 1 {
        let mut draw = NativeSecondarySpriteDraw::new(actor, NativeSecondaryAtlas::BadGuys, 78);
        draw.alpha = actor.alpha * 0.5;
        draw.blend = NativeSecondaryBlend::Add;
        draw.offset = Vec2::new(0.0, -175.0 - actor.scale * 50.0);
        draw.role = "storm-weather-moving-composite".into();
        draw.rotation_radians = degrees_to_radians(age_ticks / 48.0 * phase);
        draw.scale_x = actor.scale * 3.75;
        draw.scale_y = draw.scale_x;
        draw.tint = 0xccffff;
        draws.push(draw);
    }
    if actor.frame > 0.0 {
        let mut draw = NativeSecondarySpriteDraw::new(actor, NativeSecondaryAtlas::BadGuys, 78);
        draw.alpha = actor.alpha * 0.75;
        draw.color_mode = Some(NativeSecondaryColorMode::AlphaMask);
        draw.offset = Vec2::new(0.0, -175.0);
        draw.role = "storm-weather-strike-flash".into();
        draw.rotation_radians = degrees_to_radians(age_ticks * 0.0625 * phase);
        draw.scale_x = actor.scale * 4.0;
        draw.scale_y = actor.scale * 0.8 * 4.0;
        draw.tint = WHITE;
        draws.push(draw);
    }
    draws
}

#[must_use]
pub fn storm_weather_composite(
    actor: &NativeSecondaryActorState,
) -> Option<NativeStormWeatherComposite> {
    storm_weather_composite_at_age(actor, actor.age_ticks as f64)
}

pub(super) fn storm_weather_composite_at_age(
    actor: &NativeSecondaryActorState,
    age: f64,
) -> Option<NativeStormWeatherComposite> {
    if actor.variant == 1 {
        return None;
    }
    let phase = storm_visual_phase(actor);
    let alpha_wave = 0.5 + float::sin(degrees_to_radians(age * 0.5)) * 0.5;
    let scale_wave = 0.5 + float::sin(degrees_to_radians(age / 6.0)) * 0.25;
    let mut inner = NativeSecondarySpriteDraw::new(actor, NativeSecondaryAtlas::BadGuys, 78);
    inner.alpha = actor.alpha * 2.0;
    inner.role = "storm-weather-static-inner".into();
    inner.rotation_radians = degrees_to_radians(age * 0.03125 * phase);
    inner.scale_x = actor.scale;
    inner.scale_y = actor.scale * 0.8;
    let mut middle = NativeSecondarySpriteDraw::new(actor, NativeSecondaryAtlas::BadGuys, 78);
    middle.alpha = actor.alpha * 0.75;
    middle.offset = Vec2::new(0.0, actor.scale * -10.0);
    middle.role = "storm-weather-static-middle".into();
    middle.rotation_radians = degrees_to_radians(age / 48.0 * phase);
    middle.scale_x = actor.scale * 0.75;
    middle.scale_y = actor.scale * 0.8 * 0.75;
    middle.tint = 0xccffff;
    let mut outer = NativeSecondarySpriteDraw::new(actor, NativeSecondaryAtlas::BadGuys, 78);
    outer.alpha = actor.alpha * alpha_wave * 0.75;
    outer.offset = Vec2::new(0.0, actor.scale * -6.0);
    outer.role = "storm-weather-static-outer".into();
    outer.rotation_radians = degrees_to_radians(age * 0.125 * phase);
    outer.scale_x = actor.scale * 0.5;
    outer.scale_y = actor.scale * 0.8 * scale_wave;
    outer.tint = 0xccffff;
    Some(NativeStormWeatherComposite {
        draws: vec![inner, middle, outer],
        offset: Vec2::new(0.0, -175.0),
        scale: 5.0,
    })
}
fn storm_visual_phase(actor: &NativeSecondaryActorState) -> f64 {
    let mut rng = actor
        .presentation_rng
        .clone()
        .expect("Storm presentation retains its pre-consumption RNG state");
    let phase = rng.draw_scaled_float(1.0, true);
    if actor.variant == 1 { phase * 15.0 } else { phase }
}

#[must_use]
pub fn freeze_wave_visual_draws(actor: &NativeSecondaryActorState) -> NativeFreezeWaveVisualDraws {
    freeze_wave_visual_draws_at_age(actor, actor.age_ticks)
}

pub(super) fn freeze_wave_visual_draws_at_age(
    actor: &NativeSecondaryActorState,
    age: u64,
) -> NativeFreezeWaveVisualDraws {
    const PERSPECTIVE: f64 = 0.8_f32 as f64;
    const ANGULAR_DECAY: f64 = 0.975_f32 as f64;
    const HEIGHT_DECAY: f64 = 0.99_f32 as f64;
    const SNOW_LOSS: f64 = 0.02_f32 as f64;
    const PI: f64 = std::f32::consts::PI as f64;

    let mut rng = actor
        .presentation_rng
        .clone()
        .expect("FreezeWave presentation retains its pre-consumption RNG state");
    let mut visual = NativeFreezeWaveVisualDraws::default();
    for (index, (growth, loss)) in
        [(1.02, 0.05), (1.015, 0.05), (1.01, 0.065)].into_iter().enumerate()
    {
        let rotation = rng.draw_scaled_float(360.0, false);
        let life = repeated_float_decay(4.5, loss, age);
        if life <= 0.0 {
            continue;
        }
        let scale = repeated_float_multiply(1.0, growth, age);
        // These are the inline bundle records, separate from compact decorations.
        let mut draw = NativeSecondarySpriteDraw::new(actor, NativeSecondaryAtlas::DeadHawg, 16);
        draw.alpha = life.min(1.0);
        draw.blend = NativeSecondaryBlend::Add;
        draw.role = format!("freeze-wave-burst-{index}");
        draw.rotation_radians = degrees_to_radians(rotation);
        draw.scale_x = scale;
        draw.scale_y = fround(scale * PERSPECTIVE);
        visual.underlay_draws.push(draw);
    }
    let life = repeated_float_decay(1.75, 0.01, age);
    if life > 0.0 {
        let mut draw = NativeSecondarySpriteDraw::new(actor, NativeSecondaryAtlas::DeadHawg, 17);
        draw.alpha = life.min(1.0);
        draw.role = "freeze-wave-ring".into();
        draw.scale_x = 1.5;
        draw.scale_y = 1.5;
        visual.background_draws.push(draw);
    }
    for index in 0..if actor.enhanced { 200 } else { 100 } {
        let mut angle = rng.draw_scaled_float(360.0, false);
        let initial_velocity = rng.draw_scaled_float(10.0, false);
        let initial_radius = rng.draw_scaled_float(40.0, false);
        let radial_velocity = rng.draw_scaled_float(4.0, false);
        let initial_height = rng.draw_scaled_float(250.0, false);
        let scale = rng.draw_scaled_float(0.5, false);
        let mut rotation = rng.draw_scaled_float(360.0, false);
        let life = rng.draw_scaled_float(1.5, false);
        let mut velocity = fround(10.0 + initial_velocity);
        let mut radius = fround(20.0 + initial_radius);
        let radial_step = fround(1.0 + radial_velocity);
        let mut height = fround(50.0 + initial_height);
        let mut life = fround(2.0 + life);
        for _ in 0..age {
            if life <= 0.0 {
                break;
            }
            angle = fround(angle + velocity);
            velocity = fround(velocity * ANGULAR_DECAY * ANGULAR_DECAY);
            height = fround(height * HEIGHT_DECAY);
            radius = fround(radius + radial_step * velocity.min(1.0));
            rotation = fround(rotation + velocity);
            life = fround(life - SNOW_LOSS);
        }
        if life <= 0.0 {
            continue;
        }
        // Native UnitVector stores float32 radians and uses (sin, -cos).
        let radians = fround(angle * PI / 180.0);
        let x = fround(actor.position.x + radius * fround(float::sin(radians)));
        let y = fround(actor.position.y - radius * fround(float::cos(radians)) * PERSPECTIVE);
        let scale = fround(1.0 - scale);
        let mut draw = NativeSecondarySpriteDraw::new(actor, NativeSecondaryAtlas::BadGuys, 72);
        draw.alpha = life.min(1.0);
        draw.offset = Vec2::new(x - actor.position.x, fround(y - height) - actor.position.y);
        draw.role = format!("freeze-wave-snow-{index}");
        draw.rotation_radians = degrees_to_radians(rotation);
        draw.scale_x = scale;
        draw.scale_y = scale;
        visual.draws.push(draw);
    }
    visual
}

#[must_use]
pub fn raindrop_gradient(
    actor: &NativeSecondaryActorState,
    acid: bool,
) -> NativeSecondaryGradientDraw {
    NativeSecondaryGradientDraw {
        bottom_alpha: 0.5,
        bottom_color: if acid { 0xb3f2bf } else { 0xccf2ff },
        height: actor.quantity,
        role: if acid { "acid-raindrop-streak" } else { "storm-raindrop-streak" }.into(),
        top_alpha: 0.0,
        top_color: if acid { 0x66f280 } else { 0x66f2ff },
        top_left: Vec2::new(if acid { -1.0 } else { 0.0 }, actor.phase),
        width: if acid { 3.0 } else { 2.0 },
    }
}

// This renderer helper uses different endpoint equations than the gameplay
// natural spline. Preserve its source coefficients for the storm arc.
struct ThreePointSpline {
    points: [Vec2; 3],
    x: [f64; 3],
    y: [f64; 3],
}
impl ThreePointSpline {
    fn new(first: Vec2, second: Vec2, third: Vec2) -> Self {
        fn axis(a: f64, b: f64, c: f64) -> [f64; 3] {
            let p0 = (b - a) * 0.75;
            let p1 = ((c - a) * 3.0 - p0) * 0.25;
            let p2 = ((c - b) * 3.0 - p1) * 0.25;
            let t1 = p1 - p2 * 0.25;
            [p0 - t1 * 0.25, t1, p2]
        }
        Self {
            points: [first, second, third],
            x: axis(first.x, second.x, third.x),
            y: axis(first.y, second.y, third.y),
        }
    }
    fn sample(&self, time: f64) -> Vec2 {
        fn axis(points: [f64; 3], tangents: [f64; 3], time: f64) -> f64 {
            if time <= 0.0 {
                return points[0];
            }
            if time >= 2.0 {
                return points[2];
            }
            let segment = time.floor() as usize;
            let fraction = time - segment as f64;
            let start = points[segment];
            let end = points[segment + 1];
            let st = tangents[segment];
            let et = tangents[segment + 1];
            let quadratic = (end - start) * 3.0 - st * 2.0 - et;
            let cubic = (start - end) * 2.0 + st + et;
            start + fraction * (st + fraction * (quadratic + fraction * cubic))
        }
        Vec2::new(
            axis(self.points.map(|point| point.x), self.x, time),
            axis(self.points.map(|point| point.y), self.y, time),
        )
    }
}
