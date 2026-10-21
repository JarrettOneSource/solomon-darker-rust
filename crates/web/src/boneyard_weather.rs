//! Ambient rain owns only presentation state. Arena environment mode selects
//! the native population; snapshot ticks and actual gate collision advance it.

use solomon_sim::Vec2;
use solomon_sim::boneyard::BoneyardBounds;
use solomon_sim::boneyard::weather::{
    NATIVE_BONEYARD_WEATHER_SPLASH_ENTRY, NATIVE_BONEYARD_WEATHER_STREAK_RAMP_HEIGHT,
    NativeBoneyardWeather, native_boneyard_weather_streak_ramp,
};
use solomon_sim::spells::presentation::{
    NativeSpellSprite, NativeSpellSpritePlan, SpellAtlas, SpellBlend,
};

use crate::courtyard::Camera;
use crate::render::{BlendMode, Quad, Renderer, Texture};
use crate::spells::SpellView;

pub struct BoneyardWeatherView {
    weather: NativeBoneyardWeather,
    streak_ramp: Texture,
}

impl BoneyardWeatherView {
    pub fn new(
        renderer: &Renderer,
        initial_tick: u64,
        environment_mode: u32,
        enhanced_effects: bool,
    ) -> Result<Self, String> {
        Ok(Self {
            weather: NativeBoneyardWeather::new(initial_tick, environment_mode, enhanced_effects),
            streak_ramp: renderer.upload_rgba(
                1,
                NATIVE_BONEYARD_WEATHER_STREAK_RAMP_HEIGHT as u32,
                &native_boneyard_weather_streak_ramp(),
            )?,
        })
    }

    /// Use the visible world bounds without padding, viewport height / zoom,
    /// actual articulated gate collision and the current logical light sampler.
    pub fn prepare(
        &mut self,
        tick: u64,
        visible_bounds: BoneyardBounds,
        viewport_world_height: f64,
        collides: &dyn Fn(Vec2, f64) -> bool,
        light_at: &dyn Fn(Vec2) -> f64,
    ) {
        self.weather.advance_to(tick, visible_bounds, viewport_world_height, collides);
        self.weather.update_lighting(light_at);
    }

    /// Source depth 0.25: ground splashes precede the region-light composite.
    pub fn draw_splashes(&self, renderer: &mut Renderer, camera: &Camera, spells: &SpellView) {
        let mut draw = NativeSpellSprite::new(NATIVE_BONEYARD_WEATHER_SPLASH_ENTRY, 1.0);
        draw.atlas = SpellAtlas::DeadHawg;
        draw.blend = SpellBlend::Add;
        let mut plan = NativeSpellSpritePlan {
            position: Vec2::ZERO,
            world_y: 0.0,
            sort_bias: 0.0,
            region_light_point: None,
            draws: vec![draw],
        };
        for splash in self.weather.splashes() {
            plan.position = splash.position;
            plan.world_y = splash.position.y;
            plan.draws[0].alpha = splash.alpha;
            plan.draws[0].scale = Vec2::new(splash.scale, splash.scale);
            spells.draw_sprite_plan(renderer, camera, &plan);
        }
    }

    /// Source depth foreground +0.5: streaks paint above the world and retain
    /// their first-render logical light, independent of subsequent camera moves.
    pub fn draw_streaks(&self, renderer: &mut Renderer, camera: &Camera) {
        for drop in self.weather.drops() {
            let channel = (drop.start_color & 0xff) as f32 / 255.0;
            let center_y = drop.position.y + drop.height - drop.length / 2.0;
            renderer.draw(
                &self.streak_ramp,
                Quad {
                    source: [0.0, 0.0, 1.0, NATIVE_BONEYARD_WEATHER_STREAK_RAMP_HEIGHT as f32],
                    destination: camera.project(
                        drop.position.x - 0.5,
                        center_y - drop.length / 2.0,
                        1.0,
                        drop.length,
                    ),
                    tint: [channel, channel, channel, 1.0],
                    blend: BlendMode::Normal,
                },
            );
        }
    }

    pub fn counts(&self) -> [usize; 2] {
        self.weather.counts()
    }

    pub fn destroy(self, renderer: &Renderer) {
        renderer.destroy_texture(&self.streak_ramp);
    }
}
