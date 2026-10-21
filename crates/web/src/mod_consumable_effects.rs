//! `renderer/mod-consumable-effect-view.ts`, using the shared stock flash atlas.
//! Retained view order is birth order, even when accepted effect rows are reordered.

use std::collections::BTreeSet;

use solomon_sim::Vec2;
use solomon_sim::modding::consumable_presentation::{
    MOD_CONSUMABLE_TEXTURE_SIZE, ModConsumableEffectPlan, mod_consumable_effect_id,
    mod_consumable_effect_plan, mod_consumable_ring_rgba,
};
use solomon_sim::modding::inventory::GameSimulationModEffect;
use solomon_sim::presentation::multiply_native_tints;
use solomon_sim::spells::presentation::{NativeSpellSprite, NativeSpellSpritePlan, SpellBlend};

use crate::courtyard::Camera;
use crate::render::{BlendMode, Quad, Renderer, Texture};
use crate::spells::SpellView;

struct ModConsumableEffectView {
    id: String,
    position: Vec2,
    plan: ModConsumableEffectPlan,
    world_tint: u32,
}

pub struct ModConsumableEffectsView {
    ring: Texture,
    views: Vec<ModConsumableEffectView>,
}

impl ModConsumableEffectsView {
    pub fn new(renderer: &Renderer) -> Result<Self, String> {
        let ring = renderer.upload_rgba(
            MOD_CONSUMABLE_TEXTURE_SIZE,
            MOD_CONSUMABLE_TEXTURE_SIZE,
            &mod_consumable_ring_rgba(),
        )?;
        Ok(Self { ring, views: Vec::new() })
    }

    pub fn reset(&mut self) {
        self.views.clear();
    }

    pub fn update(
        &mut self,
        effects: &[GameSimulationModEffect],
        tick: f64,
        player_position: &dyn Fn(&str) -> Option<Vec2>,
        tint_at: &dyn Fn(Vec2) -> u32,
    ) {
        let mut live = BTreeSet::new();
        for effect in effects {
            let Some(position) = player_position(&effect.player_id) else { continue };
            let id = mod_consumable_effect_id(effect);
            live.insert(id.clone());
            let plan = mod_consumable_effect_plan(effect, tick);
            let world_tint = tint_at(position);
            if let Some(view) = self.views.iter_mut().find(|view| view.id == id) {
                view.position = position;
                view.plan = plan;
                view.world_tint = world_tint;
            } else {
                self.views.push(ModConsumableEffectView { id, position, plan, world_tint });
            }
        }
        self.views.retain(|view| live.contains(&view.id));
    }

    /// BoneyardDynamicScene places every carrier at foreground depth + 0.75.
    pub fn draw(&self, renderer: &mut Renderer, camera: &Camera, spells: &SpellView) {
        for view in &self.views {
            let plan = &view.plan;
            let tint = multiply_native_tints(plan.tint, view.world_tint);
            let diameter = plan.radius * 2.0;
            renderer.draw(
                &self.ring,
                Quad {
                    source: [
                        0.0,
                        0.0,
                        MOD_CONSUMABLE_TEXTURE_SIZE as f32,
                        MOD_CONSUMABLE_TEXTURE_SIZE as f32,
                    ],
                    destination: camera.project(
                        view.position.x - plan.radius,
                        view.position.y - plan.radius,
                        diameter,
                        diameter,
                    ),
                    tint: [
                        ((tint >> 16) & 255) as f32 / 255.0,
                        ((tint >> 8) & 255) as f32 / 255.0,
                        (tint & 255) as f32 / 255.0,
                        plan.alpha as f32,
                    ],
                    blend: BlendMode::Normal,
                },
            );
            if !plan.flash_visible {
                continue;
            }
            let mut flashes = NativeSpellSpritePlan {
                position: view.position,
                world_y: view.position.y,
                sort_bias: 0.0,
                region_light_point: None,
                draws: Vec::with_capacity(4),
            };
            for index in 0..4 {
                let mut draw = NativeSpellSprite::new(110, plan.flash_scales[index]);
                draw.alpha = plan.flash_alphas[index];
                draw.blend = SpellBlend::Add;
                draw.rotation = plan.flash_rotations[index];
                draw.tint = tint;
                flashes.draws.push(draw);
            }
            spells.draw_sprite_plan(renderer, camera, &flashes);
        }
    }
}
