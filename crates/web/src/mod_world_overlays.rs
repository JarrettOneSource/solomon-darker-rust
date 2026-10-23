//! GPU authored enemies, spell effects, powerups and shop NPCs from `mod-ui`.
//! Accepted projections own actor state. Birth clocks retain the source's keyed CSS animation lifetimes.

mod scene;
pub use scene::{ModSceneOverlay, ModSceneOverlayFrame, ModScenePartyMember, ModViewAction};

use crate::gpu_ui::UiFrame;
use crate::mod_presentation_assets::ModPresentationAssets;
use egui::{Color32, FontFamily, FontId, Id, Order, Pos2, Rect, Stroke, pos2, vec2};
use solomon_sim::Vec2;
use solomon_sim::modding::LuaObject;
use solomon_sim::modding::assets::GameModAsset;
use solomon_sim::modding::presentation::*;
use std::collections::{BTreeMap, BTreeSet};
use web_sys::Document;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModWorldProjection {
    pub location: ModWorldLocation,
    pub screen_origin: Vec2,
    pub local_player_position: Vec2,
    pub zoom: f64,
}
impl ModWorldProjection {
    #[must_use]
    pub fn screen_position(self, world: Vec2) -> Vec2 {
        Vec2::new(
            self.screen_origin.x + (world.x - self.local_player_position.x) * self.zoom,
            self.screen_origin.y + (world.y - self.local_player_position.y) * self.zoom,
        )
    }
}
pub struct ModWorldOverlayFrame<'a> {
    pub runtime: Option<&'a LuaObject>,
    pub assets: &'a [GameModAsset],
    pub projection: Option<ModWorldProjection>,
}
#[derive(Clone)]
struct Sprite {
    path: String,
    source: ModImageFrame,
    image_size: Vec2,
    center: Pos2,
    size: egui::Vec2,
}
struct Enemy {
    id: i64,
    sprite: Sprite,
    name: String,
    health: f32,
    dying: bool,
    glow: Color32,
    blur: f32,
}
struct Effect {
    id: i64,
    sprite: Sprite,
    kind: ModSpellEffectKind,
    radius: f32,
    target: Pos2,
}
struct Powerup {
    id: i64,
    sprite: Sprite,
}
struct Pickup {
    id: i64,
    tick: i64,
    center: Pos2,
}
struct Shop {
    path: Option<String>,
    name: String,
    center: Pos2,
}
pub struct ModWorldOverlays {
    document: Document,
    enemies: Vec<Enemy>,
    effects: Vec<Effect>,
    powerups: Vec<Powerup>,
    pickups: Vec<Pickup>,
    shops: Vec<Shop>,
    births: BTreeMap<(u8, i64, i64), f64>,
    pub scene: ModSceneOverlay,
}
impl ModWorldOverlays {
    pub fn new(document: &Document) -> Result<Self, String> {
        Ok(Self {
            document: document.clone(),
            enemies: Vec::new(),
            effects: Vec::new(),
            powerups: Vec::new(),
            pickups: Vec::new(),
            shops: Vec::new(),
            births: BTreeMap::new(),
            scene: ModSceneOverlay::new(document)?,
        })
    }
    pub fn reset(&mut self) {
        self.clear_world();
        self.scene.reset();
    }
    fn clear_world(&mut self) {
        self.enemies.clear();
        self.effects.clear();
        self.powerups.clear();
        self.pickups.clear();
        self.shops.clear();
        self.births.clear();
    }
    pub fn refresh(
        &mut self,
        frame: &ModWorldOverlayFrame<'_>,
        images: &ModPresentationAssets,
    ) -> Result<(), String> {
        let Some(projection) = frame.projection else {
            self.clear_world();
            return Ok(());
        };
        let boneyard = projection.location == ModWorldLocation::Boneyard;
        self.enemies.clear();
        if boneyard {
            for model in project_mod_enemies(frame.runtime, frame.assets) {
                let scale = (128.0 / model.frame.width.max(model.frame.height)).min(1.0)
                    * model.scale
                    * projection.zoom;
                self.enemies.push(Enemy {
                    id: model.id,
                    sprite: sprite(
                        images,
                        model.image,
                        model.frame,
                        model.image_size,
                        scale,
                        projection.screen_position(model.position),
                    )?,
                    name: model.name.to_owned(),
                    health: (model.current_health / model.maximum_health) as f32,
                    dying: model.life_state == ModEnemyLifeState::Dying,
                    glow: packed_color(model.light_color),
                    blur: (model.light_radius / 10.0).max(4.0) as f32,
                });
            }
        }
        // CSS stacks enemies by rounded screen y; insertion order resolves ties.
        self.enemies.sort_by(|left, right| {
            (left.sprite.center.y + 0.5).floor().total_cmp(&(right.sprite.center.y + 0.5).floor())
        });
        self.effects.clear();
        for model in project_mod_spell_effects(frame.runtime, frame.assets) {
            let scale =
                (96.0 / model.frame.width.max(model.frame.height)).min(1.0) * projection.zoom;
            self.effects.push(Effect {
                id: model.id,
                sprite: sprite(
                    images,
                    model.image,
                    model.frame,
                    model.image_size,
                    scale,
                    projection.screen_position(model.position),
                )?,
                kind: model.kind,
                radius: (model.radius * projection.zoom) as f32,
                target: point(projection.screen_position(model.target)),
            });
        }
        self.powerups.clear();
        self.pickups.clear();
        if boneyard {
            for model in project_mod_powerups(frame.runtime, frame.assets) {
                let scale =
                    (72.0 / model.frame.width.max(model.frame.height)).min(1.0) * projection.zoom;
                self.powerups.push(Powerup {
                    id: model.id,
                    sprite: sprite(
                        images,
                        model.image,
                        model.frame,
                        model.image_size,
                        scale,
                        projection.screen_position(model.position),
                    )?,
                });
            }
            for event in project_mod_powerup_events(frame.runtime, frame.assets) {
                self.pickups.push(Pickup {
                    id: event.id,
                    tick: event.tick,
                    center: point(projection.screen_position(event.position)),
                });
            }
        }
        self.shops.clear();
        for model in project_mod_shop_npcs(frame.runtime, frame.assets, projection.location) {
            self.shops.push(Shop {
                path: model
                    .image
                    .map(|image| {
                        images.image(&image.mod_id, &image.path).map(|image| image.url().to_owned())
                    })
                    .transpose()?,
                name: model.name.to_owned(),
                center: point(projection.screen_position(model.position)),
            });
        }
        let live: BTreeSet<_> = self
            .effects
            .iter()
            .filter(|effect| effect.kind == ModSpellEffectKind::Area)
            .map(|effect| (0, effect.id, 0))
            .chain(self.powerups.iter().map(|powerup| (1, powerup.id, 0)))
            .chain(self.pickups.iter().map(|event| (2, event.id, event.tick)))
            .collect();
        self.births.retain(|key, _| live.contains(key));
        Ok(())
    }
    pub fn paint(&mut self, frame: &mut UiFrame<'_>) -> Result<(), String> {
        let ctx = frame.ctx.clone();
        let now = ctx.input(|input| input.time);
        let reduced = self
            .document
            .default_view()
            .ok_or("mod overlay window")?
            .match_media("(prefers-reduced-motion: reduce)")
            .map_err(|_| "mod motion preference")?
            .is_some_and(|query| query.matches());
        let painter = ctx
            .layer_painter(egui::LayerId::new(Order::Background, Id::unique("mod/world")))
            .with_clip_rect(ctx.content_rect());
        for powerup in &self.powerups {
            let born = *self.births.entry((1, powerup.id, 0)).or_insert(now);
            let phase = alternate(now - born, 0.8);
            let mut sprite = powerup.sprite.clone();
            if !reduced {
                sprite.center.y -= 7.0 * phase;
            }
            paint_sprite(
                frame,
                &painter,
                &sprite,
                0.5,
                1.0,
                Some((Color32::from_rgba_unmultiplied(215, 186, 255, 217), 9.0)),
            );
            frame.record_rect(
                &format!("mod/powerup:{}", powerup.id),
                "Powerup",
                "img",
                Rect::from_center_size(sprite.center, sprite.size),
                true,
            );
        }
        for event in &self.pickups {
            let born = *self.births.entry((2, event.id, event.tick)).or_insert(now);
            let progress = ((now - born) / 0.5).clamp(0.0, 1.0) as f32;
            let eased = bezier(progress, 0.0, 0.0, 0.58, 1.0);
            painter.circle_stroke(
                event.center,
                12.0 * (1.0 + 2.0 * eased),
                Stroke::new(
                    3.0 * (1.0 + 2.0 * eased),
                    Color32::from_rgba_unmultiplied(227, 201, 255, ((1.0 - eased) * 255.0) as u8),
                ),
            );
        }
        for (index, shop) in self.shops.iter().enumerate() {
            let image = Rect::from_min_size(shop.center - vec2(27.0, 68.0), vec2(54.0, 54.0));
            if let Some(path) = &shop.path {
                frame.image(&painter, path, image, None, Color32::WHITE);
            } else {
                painter.circle_filled(image.center(), 27.0, Color32::from_rgb(37, 28, 48));
                for i in (0..12).rev() {
                    let ratio = i as f32 / 12.0;
                    painter.circle_filled(
                        image.center(),
                        27.0 * ratio,
                        Color32::from_rgb(
                            (133.0 - 96.0 * ratio) as u8,
                            (104.0 - 76.0 * ratio) as u8,
                            (161.0 - 113.0 * ratio) as u8,
                        ),
                    );
                }
                painter.circle_stroke(
                    image.center(),
                    27.0,
                    Stroke::new(2.0, Color32::from_rgb(213, 192, 142)),
                );
            }
            painter.text(
                shop.center - vec2(0.0, 7.0),
                egui::Align2::CENTER_CENTER,
                &shop.name,
                FontId::new(11.0, FontFamily::Name("Alegreya".into())),
                Color32::from_rgb(240, 228, 196),
            );
            frame.record_rect(&format!("mod/shop:{index}"), &shop.name, "img", image, true);
        }
        for enemy in &self.enemies {
            let alpha = if enemy.dying { 0.75 } else { 1.0 };
            paint_sprite(
                frame,
                &painter,
                &enemy.sprite,
                0.72,
                alpha,
                Some((enemy.glow, enemy.blur)),
            );
            let top = enemy.sprite.center.y - enemy.sprite.size.y * 0.72;
            painter.text(
                pos2(enemy.sprite.center.x, top - 6.0),
                egui::Align2::CENTER_BOTTOM,
                &enemy.name,
                FontId::new(10.0, FontFamily::Name("Alegreya Medium".into())),
                Color32::from_rgb(234, 223, 196).gamma_multiply(alpha),
            );
            let rail =
                Rect::from_min_size(pos2(enemy.sprite.center.x - 22.0, top + 9.0), vec2(44.0, 4.0));
            painter.rect_filled(rail, 0.0, Color32::from_rgb(36, 22, 26).gamma_multiply(alpha));
            painter.rect_filled(
                Rect::from_min_size(rail.min, vec2(rail.width() * enemy.health, rail.height())),
                0.0,
                Color32::from_rgb(165, 67, 82).gamma_multiply(alpha),
            );
            painter.rect_stroke(
                rail,
                0.0,
                Stroke::new(1.0, Color32::from_black_alpha((204.0 * alpha) as u8)),
                egui::StrokeKind::Inside,
            );
            frame.record_rect(
                &format!("mod/enemy:{}", enemy.id),
                &enemy.name,
                "img",
                Rect::from_min_size(
                    enemy.sprite.center
                        - vec2(enemy.sprite.size.x / 2.0, enemy.sprite.size.y * 0.72),
                    enemy.sprite.size,
                ),
                true,
            );
        }
        for effect in &self.effects {
            let center = effect.sprite.center;
            match effect.kind {
                ModSpellEffectKind::Area => {
                    let born = *self.births.entry((0, effect.id, 0)).or_insert(now);
                    let phase = if reduced { 1.0 } else { alternate(now - born, 0.55) };
                    let radius = effect.radius * if reduced { 1.0 } else { 0.96 + 0.08 * phase };
                    let alpha = if reduced { 1.0 } else { 0.55 + 0.45 * phase };
                    for i in (0..12).rev() {
                        let ratio = i as f32 / 12.0;
                        painter.circle_filled(
                            center,
                            radius * ratio,
                            Color32::from_rgba_unmultiplied(
                                133,
                                78,
                                219,
                                ((1.0 - ratio) * 56.0 * alpha) as u8,
                            ),
                        );
                    }
                    painter.circle_stroke(
                        center,
                        radius,
                        Stroke::new(
                            2.0,
                            Color32::from_rgba_unmultiplied(176, 132, 255, (199.0 * alpha) as u8),
                        ),
                    );
                }
                ModSpellEffectKind::Channel => {
                    painter.line_segment(
                        [center, effect.target],
                        Stroke::new(
                            (effect.radius * 2.0).max(4.0),
                            Color32::from_rgba_unmultiplied(178, 144, 241, 150),
                        ),
                    );
                }
                ModSpellEffectKind::Projectile => {}
            }
            paint_sprite(
                frame,
                &painter,
                &effect.sprite,
                0.5,
                1.0,
                Some((Color32::from_rgba_unmultiplied(183, 135, 255, 204), 7.0)),
            );
            frame.record_rect(
                &format!("mod/effect:{}", effect.id),
                effect.kind.as_str(),
                "img",
                Rect::from_center_size(center, effect.sprite.size),
                true,
            );
        }
        self.scene.paint(frame)
    }
}
fn sprite(
    images: &ModPresentationAssets,
    image: &GameModAsset,
    source: ModImageFrame,
    image_size: Vec2,
    scale: f64,
    position: Vec2,
) -> Result<Sprite, String> {
    Ok(Sprite {
        path: images.image(&image.mod_id, &image.path)?.url().to_owned(),
        source,
        image_size,
        center: point(position),
        size: vec2((source.width * scale) as f32, (source.height * scale) as f32),
    })
}
fn paint_sprite(
    frame: &UiFrame<'_>,
    painter: &egui::Painter,
    sprite: &Sprite,
    anchor_y: f32,
    alpha: f32,
    glow: Option<(Color32, f32)>,
) {
    let source = sprite.source;
    let uv = Rect::from_min_max(
        pos2((source.x / sprite.image_size.x) as f32, (source.y / sprite.image_size.y) as f32),
        pos2(
            ((source.x + source.width) / sprite.image_size.x) as f32,
            ((source.y + source.height) / sprite.image_size.y) as f32,
        ),
    );
    let target = Rect::from_min_size(
        sprite.center - vec2(sprite.size.x / 2.0, sprite.size.y * anchor_y),
        sprite.size,
    );
    if let Some((color, blur)) = glow {
        // A bounded separable Gaussian samples the same sprite alpha on the GPU.
        for y in -2_i32..=2 {
            for x in -2_i32..=2 {
                let weight = (-0.5 * ((x * x + y * y) as f32)).exp() / 6.1689;
                let tint = color.gamma_multiply(weight * alpha);
                let rect = target.translate(vec2(x as f32, y as f32) * blur / 2.0);
                frame.image(painter, &sprite.path, rect, Some(uv), tint);
            }
        }
    }
    frame.image(
        painter,
        &sprite.path,
        target,
        Some(uv),
        Color32::from_white_alpha((alpha * 255.0) as u8),
    );
}
fn point(point: Vec2) -> Pos2 {
    pos2(point.x as f32, point.y as f32)
}
fn packed_color(color: i64) -> Color32 {
    if (0..=0xffffff).contains(&color) {
        Color32::from_rgb((color >> 16) as u8, (color >> 8) as u8, color as u8)
    } else {
        Color32::TRANSPARENT
    }
}
fn alternate(seconds: f64, duration: f64) -> f32 {
    let phase = (seconds / duration).rem_euclid(2.0) as f32;
    bezier(if phase > 1.0 { 2.0 - phase } else { phase }, 0.42, 0.0, 0.58, 1.0)
}
fn bezier(t: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    let curve = |v: f32, p1: f32, p2: f32| {
        3.0 * (1.0 - v) * (1.0 - v) * v * p1 + 3.0 * (1.0 - v) * v * v * p2 + v * v * v
    };
    let mut low = 0.0;
    let mut high = 1.0;
    for _ in 0..16 {
        let middle = (low + high) * 0.5;
        if curve(middle, x1, x2) < t { low = middle } else { high = middle }
    }
    curve((low + high) * 0.5, y1, y2)
}
