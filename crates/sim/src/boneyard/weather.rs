//! Renderer-owned ambient Arena rain from `native-boneyard-weather.ts`.
//! Its tick-seeded stream is separate from gameplay RNG. Lighting is captured
//! when a drop first paints, rather than when an interpolated camera spawns it.

use crate::float::fround;
use crate::{NativeRng, Vec2};

use super::BoneyardBounds;

pub const NATIVE_BONEYARD_WEATHER_SPLASH_ENTRY: usize = 24;
pub const NATIVE_BONEYARD_WEATHER_COLLISION_RADIUS: f64 = 4.0;
pub const NATIVE_BONEYARD_WEATHER_STREAK_RAMP_HEIGHT: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeBoneyardWeatherDropPlan {
    pub end: Vec2,
    pub end_alpha: f64,
    pub end_color: u32,
    pub height: f64,
    pub id: u64,
    pub length: f64,
    pub position: Vec2,
    pub start: Vec2,
    pub start_alpha: f64,
    pub start_color: u32,
    pub width: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeBoneyardWeatherSplashPlan {
    pub age_ticks: u64,
    pub alpha: f64,
    pub id: u64,
    pub position: Vec2,
    pub scale: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeBoneyardWeatherPlan {
    pub drops: Vec<NativeBoneyardWeatherDropPlan>,
    pub mode: u32,
    pub spawn_count: usize,
    pub splashes: Vec<NativeBoneyardWeatherSplashPlan>,
    pub tick: u64,
}

struct WeatherDrop {
    height: f64,
    id: u64,
    length: f64,
    light_scalar: Option<f64>,
    position: Vec2,
}

struct WeatherSplash {
    age_ticks: u64,
    growth: f64,
    id: u64,
    life: f64,
    loss: f64,
    position: Vec2,
    scale: f64,
}

pub struct NativeBoneyardWeather {
    arena_age: u64,
    enhanced_effects: bool,
    drops: Vec<WeatherDrop>,
    mode: u32,
    splashes: Vec<WeatherSplash>,
    current_tick: u64,
    next_id: u64,
}

impl NativeBoneyardWeather {
    #[must_use]
    pub fn new(initial_tick: u64, mode: u32, enhanced_effects: bool) -> Self {
        Self {
            arena_age: 0,
            enhanced_effects,
            drops: Vec::new(),
            mode,
            splashes: Vec::new(),
            current_tick: initial_tick,
            next_id: 1,
        }
    }

    /// A frame may catch up several snapshot ticks using its actual visible
    /// bounds and gate collision. Older snapshots never rewind existing rain.
    pub fn advance_to(
        &mut self,
        tick: u64,
        bounds: BoneyardBounds,
        viewport_world_height: f64,
        collides: &dyn Fn(Vec2, f64) -> bool,
    ) {
        if tick <= self.current_tick {
            return;
        }
        assert!(viewport_world_height.is_finite() && viewport_world_height > 0.0);
        for next_tick in self.current_tick + 1..=tick {
            let mut rng = NativeRng::new(i64::from(native_boneyard_weather_tick_seed(next_tick)));
            self.step_existing_effects();
            self.arena_age += 1;
            if self.arena_age < 2 {
                continue;
            }
            for _ in 0..native_boneyard_weather_spawn_count(self.mode, self.enhanced_effects) {
                let position = sample_clear_point(bounds, collides, &mut rng);
                let length = fround(20.0 + rng.draw_scaled_float(10.0, false));
                self.drops.push(WeatherDrop {
                    height: fround(-viewport_world_height),
                    id: self.next_id,
                    length,
                    light_scalar: None,
                    position,
                });
                self.next_id += 1;
                self.spawn_splash(position, &mut rng);
            }
        }
        self.current_tick = tick;
    }

    pub fn update_lighting(&mut self, light_at: &dyn Fn(Vec2) -> f64) {
        for drop in &mut self.drops {
            if drop.light_scalar.is_none() {
                drop.light_scalar = Some(clamp_unit(light_at(drop.position)));
            }
        }
    }

    pub fn drops(&self) -> impl Iterator<Item = NativeBoneyardWeatherDropPlan> + '_ {
        self.drops.iter().map(|drop| {
            let channel = (drop.light_scalar.expect("rain lighting is sampled before drawing")
                * 255.0)
                .round() as u32;
            let color = channel << 16 | channel << 8 | channel;
            NativeBoneyardWeatherDropPlan {
                end: Vec2::new(drop.position.x, drop.position.y + drop.height),
                end_alpha: 0.5,
                end_color: color,
                height: drop.height,
                id: drop.id,
                length: drop.length,
                position: drop.position,
                start: Vec2::new(drop.position.x, drop.position.y + drop.height - drop.length),
                start_alpha: 0.0,
                start_color: color,
                width: 1.0,
            }
        })
    }

    pub fn splashes(&self) -> impl Iterator<Item = NativeBoneyardWeatherSplashPlan> + '_ {
        self.splashes.iter().map(|splash| NativeBoneyardWeatherSplashPlan {
            age_ticks: splash.age_ticks,
            alpha: splash.life.clamp(0.0, 1.0),
            id: splash.id,
            position: splash.position,
            scale: splash.scale,
        })
    }

    pub fn plan(&mut self, light_at: &dyn Fn(Vec2) -> f64) -> NativeBoneyardWeatherPlan {
        self.update_lighting(light_at);
        NativeBoneyardWeatherPlan {
            drops: self.drops().collect(),
            mode: self.mode,
            spawn_count: native_boneyard_weather_spawn_count(self.mode, self.enhanced_effects),
            splashes: self.splashes().collect(),
            tick: self.current_tick,
        }
    }

    #[must_use]
    pub fn counts(&self) -> [usize; 2] {
        [self.drops.len(), self.splashes.len()]
    }

    fn spawn_splash(&mut self, position: Vec2, rng: &mut NativeRng) {
        let initial_scale = rng.draw_scaled_float(0.25, false);
        let loss = rng.draw_scaled_float(0.05, false);
        let factor = rng.draw_scaled_float(0.25, false);
        let scale_factor = fround(0.75 + factor);
        self.splashes.push(WeatherSplash {
            age_ticks: 0,
            growth: fround(1.0 + scale_factor * 0.01),
            id: self.next_id,
            life: scale_factor,
            loss: fround((0.05 + loss) * scale_factor),
            position,
            scale: fround(0.5 + initial_scale),
        });
        self.next_id += 1;
    }

    fn step_existing_effects(&mut self) {
        self.drops.retain_mut(|drop| {
            drop.height = fround(drop.height + drop.length);
            drop.height <= 0.0
        });
        self.splashes.retain_mut(|splash| {
            splash.age_ticks += 1;
            splash.life = fround(splash.life - splash.loss);
            splash.scale = fround(splash.scale * splash.growth);
            splash.life > 0.0
        });
    }
}

#[must_use]
pub const fn native_boneyard_weather_spawn_count(mode: u32, enhanced_effects: bool) -> usize {
    match mode {
        1 => 3,
        2 if enhanced_effects => 20,
        2 => 10,
        _ => 0,
    }
}

#[must_use]
pub const fn native_boneyard_weather_tick_seed(tick: u64) -> u32 {
    (tick as u32).wrapping_mul(0x0ef3) & 0x3fff_ffff
}

/// Exact source one-pixel white ramp; alpha is rounded to bytes before filtering.
#[must_use]
pub fn native_boneyard_weather_streak_ramp() -> [u8; 4 * NATIVE_BONEYARD_WEATHER_STREAK_RAMP_HEIGHT]
{
    let mut pixels = [255; 4 * NATIVE_BONEYARD_WEATHER_STREAK_RAMP_HEIGHT];
    for row in 0..NATIVE_BONEYARD_WEATHER_STREAK_RAMP_HEIGHT {
        pixels[row * 4 + 3] = (row as f64 / 255.0 * 0.5 * 255.0).round() as u8;
    }
    pixels
}

fn sample_clear_point(
    bounds: BoneyardBounds,
    collides: &dyn Fn(Vec2, f64) -> bool,
    rng: &mut NativeRng,
) -> Vec2 {
    loop {
        let x = rng.draw_scaled_float(bounds.width, false);
        let y = rng.draw_scaled_float(bounds.height, false);
        let point = Vec2::new(fround(bounds.x + x), fround(bounds.y + y));
        if !collides(point, NATIVE_BONEYARD_WEATHER_COLLISION_RADIUS) {
            return point;
        }
    }
}

fn clamp_unit(value: f64) -> f64 {
    if value.is_finite() { value.clamp(0.0, 1.0) } else { 0.0 }
}
