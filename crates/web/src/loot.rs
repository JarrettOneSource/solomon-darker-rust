//! Ground loot and Goodie GPU binding for `renderer/native-loot-view.ts`.
//! The store and Puppet feedback are read-only. Only the source view's gold
//! scatter seed persists here, and it resets when the host installs a new run.

mod messages;
pub use messages::WorldNotifications;

use std::collections::{BTreeMap, BTreeSet};

use solomon_sim::Vec2;
use solomon_sim::boneyard::loot::presentation::*;
use solomon_sim::boneyard::loot::{BoneyardLootStore, LootActorRead};
use solomon_sim::enemies::puppet_hits::{EnemyPuppetHit, EnemyPuppetHitKind};
use solomon_sim::hub::painter::PainterLayer;
use solomon_sim::modding::inventory::ModConsumableCatalogEntry;
use solomon_sim::secondary::presentation::{
    NativeSecondaryAtlas, NativeSecondaryBlend, NativeSecondarySpriteDraw,
};
use solomon_sim::spells::presentation::{SpellAtlas, SpellBlend};

use crate::courtyard::Camera;
use crate::mod_presentation_assets::{ModPresentationAssets, ModSpriteDraw, ModSpriteTexture};
use crate::secondary::SecondaryView;
use solomon_graphics::{BlendMode, Renderer, TextureColorMode};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LootPainterId {
    Loot(usize),
    Goodie(usize),
}

pub struct LootPainter {
    pub id: LootPainterId,
    pub layer: PainterLayer,
    pub position: Vec2,
}

struct PreparedLoot {
    painter: LootPainter,
    layers: LootLayers,
}

enum LootLayers {
    Native(Vec<NativeLootVisualLayer>),
    Mod { texture: ModSpriteTexture, bounce_height: f64 },
}

struct GoldScatter {
    initial_seed: u32,
    seed: u32,
}

#[derive(Default)]
pub struct LootView {
    scatter: BTreeMap<usize, GoldScatter>,
    prepared: Vec<PreparedLoot>,
}

impl LootView {
    /// Called at the accepted snapshot presentation cadence, once per update.
    /// Repeated draw calls do not advance the source's private scatter animation.
    #[allow(clippy::too_many_arguments)] // Actual loot, feedback and the session's trusted art catalog.
    pub fn prepare(
        &mut self,
        store: &BoneyardLootStore,
        tick: f64,
        hits: &solomon_sim::ComponentEntities<EnemyPuppetHit>,
        hit_alphas: Option<&BTreeMap<String, f64>>,
        complex_lighting: bool,
        mod_catalog: &[ModConsumableCatalogEntry],
        mut mod_assets: Option<&mut ModPresentationAssets>,
    ) -> Result<(), String> {
        self.prepared.clear();
        let mut live_ids = BTreeSet::new();
        for actor in store.actors.iter() {
            live_ids.insert(actor.id);
            let scatter = self.scatter.entry(actor.id).or_insert(GoldScatter {
                initial_seed: actor.scatter_seed,
                seed: actor.scatter_seed,
            });
            if scatter.initial_seed != actor.scatter_seed {
                scatter.initial_seed = actor.scatter_seed;
                scatter.seed = actor.scatter_seed;
            }
            let item_content_id = actor.reward().item.as_ref().and_then(|item| {
                item.mod_content.as_ref().map(|content| content.content_id.as_str()).or_else(|| {
                    item.mod_item_content.as_ref().map(|content| content.content_id.as_str())
                })
            });
            let layers = if let Some(content_id) = item_content_id {
                let catalog = mod_catalog
                    .iter()
                    .find(|row| row.content.content_id == content_id)
                    .ok_or_else(|| format!("mod loot catalog is missing {content_id}"))?;
                let assets =
                    mod_assets.as_deref_mut().ok_or("mod presentation textures are unavailable")?;
                LootLayers::Mod {
                    texture: assets.consumable_icon(&catalog.content)?,
                    bounce_height: actor.bounce_height,
                }
            } else {
                let plan = native_loot_presentation_plan(&actor, scatter.seed);
                scatter.seed = plan.next_scatter_seed;
                LootLayers::Native(plan.layers)
            };
            self.prepared.push(PreparedLoot {
                painter: LootPainter {
                    id: LootPainterId::Loot(actor.id),
                    layer: native_loot_painter_layer(&actor),
                    position: actor.position,
                },
                layers,
            });
        }
        self.scatter.retain(|id, _| live_ids.contains(id));
        for goodie in &store.goodies {
            let id = format!("goodie:{}", goodie.id);
            let hit = hits
                .iter()
                .find(|hit| hit.kind == EnemyPuppetHitKind::Goodie && hit.target_id == id);
            // The display timeline has already sampled feedback at its fractional tick.
            let hit_alpha = hit_alphas
                .and_then(|alphas| alphas.get(&id))
                .copied()
                .unwrap_or_else(|| hit.map_or(0.0, |hit| hit.feedback.alpha(tick)));
            self.prepared.push(PreparedLoot {
                painter: LootPainter {
                    id: LootPainterId::Goodie(goodie.id),
                    layer: native_goodie_painter_layer(goodie),
                    position: goodie.position,
                },
                layers: LootLayers::Native(native_goodie_presentation_plan(
                    goodie,
                    tick,
                    hit_alpha,
                    complex_lighting,
                )),
            });
        }
        Ok(())
    }

    pub fn painters(&self) -> impl Iterator<Item = (usize, &LootPainter)> {
        self.prepared.iter().enumerate().map(|(index, prepared)| (index, &prepared.painter))
    }

    /// The caller joins stock and custom rows with the region's one painter queue.
    pub fn draw(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        sprites: &SecondaryView,
        index: usize,
        region_tint: [f32; 3],
    ) {
        let prepared = &self.prepared[index];
        let layers = match &prepared.layers {
            LootLayers::Mod { texture, bounce_height } => {
                let position = prepared.painter.position;
                let center = camera.project(position.x, position.y + bounce_height, 0.0, 0.0);
                // Source updateModPotionLayer ignores the actor's fade, rotation and
                // stack amount: one ordinary icon follows the accepted bounce height.
                texture.draw(
                    renderer,
                    &ModSpriteDraw {
                        center: Vec2::new(f64::from(center[0]), f64::from(center[1])),
                        scale: Vec2::new(camera.scale, camera.scale),
                        anchor: Vec2::new(0.5, 0.5),
                        rotation_radians: 0.0,
                        tint: [region_tint[0], region_tint[1], region_tint[2], 1.0],
                        blend: BlendMode::Normal,
                    },
                );
                return;
            }
            LootLayers::Native(layers) => layers,
        };
        for layer in layers {
            let draw = NativeSecondarySpriteDraw {
                alpha: layer.alpha,
                atlas: match layer.atlas {
                    SpellAtlas::BadGuys => NativeSecondaryAtlas::BadGuys,
                    SpellAtlas::DeadHawg => NativeSecondaryAtlas::DeadHawg,
                },
                blend: match layer.blend {
                    SpellBlend::Normal => NativeSecondaryBlend::Normal,
                    SpellBlend::Add => NativeSecondaryBlend::Add,
                },
                color_mode: None,
                entry: layer.entry,
                offset: layer.offset,
                role: layer.role.clone(),
                rotation_radians: layer.rotation_radians,
                scale_x: layer.scale.x,
                scale_y: layer.scale.y,
                tint: layer.tint,
            };
            let mode = if layer.diffuse_color {
                TextureColorMode::AlphaMask
            } else {
                TextureColorMode::Texture
            };
            sprites.draw_sprite_with_color_mode(
                renderer,
                camera,
                prepared.painter.position,
                &draw,
                region_tint,
                mode,
            );
        }
    }

    pub fn reset(&mut self) {
        self.scatter.clear();
        self.prepared.clear();
    }
}
