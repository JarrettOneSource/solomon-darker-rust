//! GPU authored enemies, spell effects, powerups and shop NPCs from `mod-ui`.
//! Accepted projections own actor state. Birth clocks retain the source's keyed CSS animation lifetimes.

mod scene;
pub use scene::{ModSceneOverlay, ModSceneOverlayFrame, ModScenePartyMember, ModViewAction};

use crate::gpu_ui::{GroupEffect, ImageShadow, UiFrame};
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
                10.5 * (1.0 + 2.0 * eased),
                Stroke::new(
                    3.0 * (1.0 + 2.0 * eased),
                    Color32::from_rgba_unmultiplied(227, 201, 255, ((1.0 - eased) * 255.0) as u8),
                ),
            );
        }
        for (index, shop) in self.shops.iter().enumerate() {
            let name = crate::gpu_ui::font_run(
                frame.ctx,
                FontId::new(11.0, FontFamily::Name("Alegreya Semibold".into())),
                Color32::from_rgb(240, 228, 196),
                &shop.name,
                None,
            )?;
            let image = Rect::from_min_size(
                shop.center - vec2(27.0, 54.0 + name.size().y),
                vec2(54.0, 54.0),
            );
            if let Some(path) = &shop.path {
                if let Some(size) = frame.image_size(path) {
                    let fit = size * (54.0 / size.x).min(54.0 / size.y);
                    let target = Rect::from_center_size(image.center(), fit);
                    let polygon = crate::gpu_ui::clip_polygon_ellipse(
                        vec![
                            target.left_top(),
                            target.right_top(),
                            target.right_bottom(),
                            target.left_bottom(),
                        ],
                        image,
                    );
                    for index in 1..polygon.len().saturating_sub(1) {
                        let positions = [polygon[0], polygon[index], polygon[index + 1]];
                        let uv = positions.map(|p| ((p - target.min) / target.size()).to_pos2());
                        frame.image_triangle(
                            &painter,
                            path,
                            positions,
                            uv,
                            Color32::WHITE,
                            solomon_graphics::BlendMode::Normal,
                        );
                    }
                }
            } else {
                let mut gradient = egui::Mesh::default();
                gradient.colored_vertex(image.center(), Color32::from_rgb(133, 104, 161));
                let t = 27.0 / (25.0 * std::f32::consts::SQRT_2);
                let edge = Color32::from_rgb(
                    (133.0 - 96.0 * t).round() as u8,
                    (104.0 - 76.0 * t).round() as u8,
                    (161.0 - 113.0 * t).round() as u8,
                );
                for index in 0..=64 {
                    let angle = std::f32::consts::TAU * index as f32 / 64.0;
                    gradient.colored_vertex(
                        image.center() + vec2(angle.cos(), angle.sin()) * 27.0,
                        edge,
                    );
                    if index > 0 {
                        gradient.add_triangle(0, index, index + 1);
                    }
                }
                painter.add(egui::Shape::mesh(gradient));
                painter.circle_stroke(
                    image.center(),
                    26.0,
                    Stroke::new(2.0, Color32::from_rgb(213, 192, 142)),
                );
            }
            frame.paint_group(
                &painter,
                GroupEffect {
                    shadows: vec![ImageShadow {
                        offset: vec2(0.0, 1.0),
                        blur: 1.5,
                        color: Color32::BLACK,
                    }],
                    ..Default::default()
                },
                |frame, painter| {
                    name.paint(
                        frame,
                        painter,
                        shop.center - vec2(name.size().x / 2.0, name.size().y),
                        1.0,
                    )
                },
            )?;
            frame.record_rect(&format!("mod/shop:{index}"), &shop.name, "img", image, true);
        }
        for enemy in &self.enemies {
            frame.paint_group(
                &painter,
                GroupEffect {
                    opacity: if enemy.dying { 0.75 } else { 1.0 },
                    shadows: vec![ImageShadow {
                        offset: egui::Vec2::ZERO,
                        blur: enemy.blur,
                        color: enemy.glow,
                    }],
                },
                |frame, painter| {
                    paint_sprite(frame, painter, &enemy.sprite, 0.72, 1.0, None);
                    let top = enemy.sprite.center.y - enemy.sprite.size.y * 0.72;
                    let name = crate::gpu_ui::font_run_with_options(
                        frame.ctx,
                        FontId::new(10.0, FontFamily::Name("Alegreya Semibold".into())),
                        Color32::from_rgb(234, 223, 196),
                        &enemy.name,
                        crate::gpu_ui::FontRunOptions {
                            line_height: Some(12.0),
                            ..Default::default()
                        },
                    )?;
                    let name_origin = egui::Align2::CENTER_BOTTOM
                        .anchor_size(pos2(enemy.sprite.center.x, top), name.size())
                        .min;
                    frame.paint_group(
                        painter,
                        GroupEffect {
                            shadows: vec![
                                ImageShadow {
                                    offset: vec2(0.0, 1.0),
                                    blur: 1.0,
                                    color: Color32::BLACK,
                                },
                                ImageShadow {
                                    offset: egui::Vec2::ZERO,
                                    blur: 1.5,
                                    color: Color32::BLACK,
                                },
                            ],
                            ..Default::default()
                        },
                        |frame, painter| name.paint(frame, painter, name_origin, 1.0),
                    )?;
                    let rail = Rect::from_min_size(
                        pos2(enemy.sprite.center.x - 22.0, top + 9.0),
                        vec2(44.0, 4.0),
                    );
                    painter.rect_filled(rail, 0.0, Color32::from_rgb(36, 22, 26));
                    let content = rail.shrink(1.0);
                    painter.rect_filled(
                        Rect::from_min_size(
                            content.min,
                            vec2(content.width() * enemy.health, content.height()),
                        ),
                        0.0,
                        Color32::from_rgb(165, 67, 82),
                    );
                    painter.rect_stroke(
                        rail,
                        0.0,
                        Stroke::new(1.0, Color32::from_black_alpha(204)),
                        egui::StrokeKind::Inside,
                    );
                    Ok(())
                },
            )?;
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
                    let pulse_scale = if reduced { 1.0 } else { 0.96 + 0.08 * phase };
                    let radius = effect.radius * pulse_scale;
                    let alpha = if reduced { 1.0 } else { 0.55 + 0.45 * phase };
                    let border_width = 2.0 * pulse_scale;
                    frame.paint_group(
                        &painter,
                        GroupEffect { opacity: alpha, ..Default::default() },
                        |frame, painter| {
                            frame.paint_outer_shadow(
                                painter,
                                ImageShadow {
                                    offset: egui::Vec2::ZERO,
                                    blur: 9.0 * pulse_scale,
                                    color: Color32::from_rgba_unmultiplied(151, 96, 238, 166),
                                },
                                |caster| {
                                    caster.circle_filled(center, radius, Color32::WHITE);
                                    Ok(())
                                },
                            )?;
                            // CSS's circle gradient ends at the padding box's farthest corner.
                            let fade_radius =
                                (radius - border_width).max(0.0) * std::f32::consts::SQRT_2 * 0.68;
                            let mut gradient = egui::Mesh::default();
                            gradient.colored_vertex(
                                center,
                                Color32::from_rgba_unmultiplied(133, 78, 219, 56),
                            );
                            for index in 0..=64 {
                                let angle = std::f32::consts::TAU * index as f32 / 64.0;
                                gradient.colored_vertex(
                                    center + vec2(angle.cos(), angle.sin()) * fade_radius,
                                    Color32::TRANSPARENT,
                                );
                                if index > 0 {
                                    gradient.add_triangle(0, index, index + 1);
                                }
                            }
                            painter.add(egui::Shape::mesh(gradient));
                            painter.circle_stroke(
                                center,
                                (radius - border_width / 2.0).max(0.0),
                                Stroke::new(
                                    border_width,
                                    Color32::from_rgba_unmultiplied(176, 132, 255, 199),
                                ),
                            );
                            Ok(())
                        },
                    )?;
                }
                ModSpellEffectKind::Channel => {
                    paint_channel(
                        frame,
                        &painter,
                        center,
                        effect.target,
                        (effect.radius * 2.0).max(4.0),
                    )?;
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
fn paint_channel(
    frame: &UiFrame<'_>,
    painter: &egui::Painter,
    start: Pos2,
    end: Pos2,
    height: f32,
) -> Result<(), String> {
    let delta = end - start;
    let length = delta.length();
    if length == 0.0 {
        return Ok(());
    }
    let direction = delta / length;
    let point =
        |local: egui::Vec2| start + direction * local.x + vec2(-direction.y, direction.x) * local.y;
    let radius = (length / 2.0).min(height / 2.0);
    let mut outline = Vec::with_capacity(68);
    for (corner, angle) in [
        (vec2(radius, -height / 2.0 + radius), std::f32::consts::PI),
        (vec2(length - radius, -height / 2.0 + radius), std::f32::consts::PI * 1.5),
        (vec2(length - radius, height / 2.0 - radius), 0.0),
        (vec2(radius, height / 2.0 - radius), std::f32::consts::FRAC_PI_2),
    ] {
        for step in 0..=16 {
            let angle = angle + std::f32::consts::FRAC_PI_2 * step as f32 / 16.0;
            let p = point(corner + vec2(angle.cos(), angle.sin()) * radius);
            if outline.last().is_none_or(|previous: &Pos2| *previous != p) {
                outline.push(p);
            }
        }
    }
    if outline.first().zip(outline.last()).is_some_and(|(first, last)| first == last) {
        outline.pop();
    }
    if outline.len() < 3 {
        return Ok(());
    }
    frame.paint_outer_shadow(
        painter,
        ImageShadow {
            offset: egui::Vec2::ZERO,
            blur: 6.0,
            color: Color32::from_rgba_unmultiplied(166, 112, 255, 191),
        },
        |caster| {
            caster.add(egui::Shape::convex_polygon(outline.clone(), Color32::WHITE, Stroke::NONE));
            Ok(())
        },
    )?;
    let first = Color32::from_rgba_unmultiplied(224, 211, 255, 204).to_array();
    let last = Color32::from_rgba_unmultiplied(132, 77, 228, 97).to_array();
    let color = |p: Pos2| {
        let t = ((p - start).dot(direction) / length).clamp(0.0, 1.0);
        let channels = std::array::from_fn::<_, 4, _>(|i| {
            (first[i] as f32 + (last[i] as f32 - first[i] as f32) * t).round() as u8
        });
        Color32::from_rgba_premultiplied(channels[0], channels[1], channels[2], channels[3])
    };
    let mut mesh = egui::Mesh::default();
    let center = point(vec2(length / 2.0, 0.0));
    mesh.colored_vertex(center, color(center));
    for (index, p) in outline.iter().enumerate() {
        let previous = outline[(index + outline.len() - 1) % outline.len()];
        let next = outline[(index + 1) % outline.len()];
        let normal = |edge: egui::Vec2| {
            let edge = edge.normalized();
            vec2(edge.y, -edge.x)
        };
        let average = (normal(*p - previous) + normal(next - *p)) / 2.0;
        let feather = average / average.length_sq() * 0.5;
        let inner = *p - feather;
        mesh.colored_vertex(inner, color(inner));
        mesh.colored_vertex(*p + feather, Color32::TRANSPARENT);
    }
    for index in 0..outline.len() {
        let current = 1 + 2 * index as u32;
        let next = 1 + 2 * ((index + 1) % outline.len()) as u32;
        mesh.add_triangle(0, current, next);
        mesh.add_triangle(current, current + 1, next);
        mesh.add_triangle(current + 1, next + 1, next);
    }
    painter.add(egui::Shape::mesh(mesh));
    Ok(())
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
        frame.atlas_image_shadowed(
            painter,
            &sprite.path,
            Rect::from_min_max(
                pos2(source.x as f32, source.y as f32),
                pos2((source.x + source.width) as f32, (source.y + source.height) as f32),
            ),
            target,
            crate::gpu_ui::ImageShadow { offset: egui::Vec2::ZERO, blur, color },
            Color32::from_white_alpha((alpha * 255.0) as u8),
        );
        return;
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
