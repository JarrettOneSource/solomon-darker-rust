//! Source `mod-ui/Mod{Enemies,SpellEffects,Powerups,ShopNpcs}.tsx` DOM consumers.
//! Refresh on accepted snapshot/runtime notifications and resize, using the last rendered
//! scene projection. Keyed elements survive refreshes so CSS animation clocks do too.

mod scene;
pub use scene::{ModSceneOverlay, ModSceneOverlayFrame, ModScenePartyMember, ModViewAction};

use std::collections::{BTreeMap, BTreeSet};

use solomon_sim::Vec2;
use solomon_sim::modding::LuaObject;
use solomon_sim::modding::assets::GameModAsset;
use solomon_sim::modding::presentation::*;
use web_sys::{Document, HtmlElement};

use crate::mod_presentation_assets::ModPresentationAssets;
use crate::ui::{append, element, style};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModWorldProjection {
    pub location: ModWorldLocation,
    /// Scene client rect origin plus its last rendered local-player screen point.
    pub screen_origin: Vec2,
    /// Source layouts subtract the accepted local position, not a predicted display copy.
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

pub struct ModWorldOverlays {
    document: Document,
    root: HtmlElement,
    enemy_root: HtmlElement,
    effect_root: HtmlElement,
    powerup_root: HtmlElement,
    shop_root: HtmlElement,
    enemy_nodes: BTreeMap<i64, EnemyNodes>,
    effect_nodes: BTreeMap<i64, EffectNodes>,
    powerup_nodes: BTreeMap<i64, HtmlElement>,
    pickup_nodes: BTreeMap<(i64, i64), HtmlElement>,
    shop_nodes: BTreeMap<usize, ShopNodes>,
    pub scene: ModSceneOverlay,
}

struct EnemyNodes {
    root: HtmlElement,
    name: HtmlElement,
    health: HtmlElement,
}

struct EffectNodes {
    root: HtmlElement,
    sprite: HtmlElement,
    decoration: Option<(ModSpellEffectKind, HtmlElement)>,
}

struct ShopNodes {
    root: HtmlElement,
    icon: HtmlElement,
    name: HtmlElement,
    image: bool,
}

impl ModWorldOverlays {
    pub fn new(document: &Document) -> Result<Self, String> {
        let root = element(document, "div", "mod-world-overlays")?;
        let css = element(document, "style", "mod-world-styles")?;
        css.set_text_content(Some(include_str!("../../../web/mod-world.css")));
        append(&root, &css)?;
        let powerup_root = element(document, "div", "mod-powerups")?;
        let enemy_root = element(document, "div", "mod-enemies")?;
        let shop_root = element(document, "div", "mod-shop-npcs")?;
        let effect_root = element(document, "div", "mod-spell-effects")?;
        for group in [&enemy_root, &powerup_root, &shop_root, &effect_root] {
            attribute(group, "aria-hidden", "true")?;
            append(&root, group)?;
        }
        document
            .body()
            .ok_or("Mod overlays need the game document")?
            .append_child(&root)
            .map_err(|_| "Could not attach mod world overlays")?;
        let scene = ModSceneOverlay::new(document, &root)?;
        Ok(Self {
            document: document.clone(),
            root,
            enemy_root,
            effect_root,
            powerup_root,
            shop_root,
            enemy_nodes: BTreeMap::new(),
            effect_nodes: BTreeMap::new(),
            powerup_nodes: BTreeMap::new(),
            pickup_nodes: BTreeMap::new(),
            shop_nodes: BTreeMap::new(),
            scene,
        })
    }

    pub fn reset(&mut self) {
        self.clear_world();
        self.scene.reset();
    }

    fn clear_world(&mut self) {
        for group in [&self.enemy_root, &self.effect_root, &self.powerup_root, &self.shop_root] {
            group.set_text_content(None);
        }
        self.enemy_nodes.clear();
        self.effect_nodes.clear();
        self.powerup_nodes.clear();
        self.pickup_nodes.clear();
        self.shop_nodes.clear();
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
        let enemies =
            if boneyard { project_mod_enemies(frame.runtime, frame.assets) } else { Vec::new() };
        let powerups =
            if boneyard { project_mod_powerups(frame.runtime, frame.assets) } else { Vec::new() };
        let pickups = if boneyard {
            project_mod_powerup_events(frame.runtime, frame.assets)
        } else {
            Vec::new()
        };
        self.refresh_enemies(&enemies, projection, images)?;
        self.refresh_effects(
            &project_mod_spell_effects(frame.runtime, frame.assets),
            projection,
            images,
        )?;
        self.refresh_powerups(&powerups, &pickups, projection, images)?;
        self.refresh_shops(
            &project_mod_shop_npcs(frame.runtime, frame.assets, projection.location),
            projection,
            images,
        )
    }

    fn refresh_enemies(
        &mut self,
        models: &[ModEnemyModel<'_>],
        projection: ModWorldProjection,
        images: &ModPresentationAssets,
    ) -> Result<(), String> {
        let live: BTreeSet<_> = models.iter().map(|enemy| enemy.id).collect();
        self.enemy_nodes.retain(|id, nodes| {
            if live.contains(id) {
                true
            } else {
                nodes.root.remove();
                false
            }
        });
        for (index, enemy) in models.iter().enumerate() {
            if !self.enemy_nodes.contains_key(&enemy.id) {
                let root = element(&self.document, "div", "mod-enemy")?;
                let name = element(&self.document, "span", "mod-enemy__name")?;
                let rail = element(&self.document, "span", "mod-enemy__health")?;
                let health = element(&self.document, "i", "")?;
                append(&rail, &health)?;
                append(&root, &name)?;
                append(&root, &rail)?;
                self.enemy_nodes.insert(enemy.id, EnemyNodes { root, name, health });
            }
            let nodes = &self.enemy_nodes[&enemy.id];
            let point = projection.screen_position(enemy.position);
            let scale = (128.0 / enemy.frame.width.max(enemy.frame.height)).min(1.0)
                * enemy.scale
                * projection.zoom;
            sprite_style(
                &nodes.root,
                enemy.image,
                enemy.frame,
                enemy.image_size,
                scale,
                point,
                images,
            )?;
            property(
                &nodes.root,
                "filter",
                &format!(
                    "drop-shadow(0 0 {}px #{})",
                    (enemy.light_radius / 10.0).max(4.0),
                    packed_color(enemy.light_color)
                ),
            )?;
            property(&nodes.root, "z-index", &(point.y + 0.5).floor().to_string())?;
            attribute(&nodes.root, "data-enemy-id", &enemy.id.to_string())?;
            attribute(&nodes.root, "data-life-state", enemy.life_state.as_str())?;
            nodes.name.set_text_content(Some(enemy.name));
            property(
                &nodes.health,
                "width",
                &format!("{}%", enemy.current_health / enemy.maximum_health * 100.0),
            )?;
            place_in_order(&self.enemy_root, &nodes.root, index)?;
        }
        Ok(())
    }

    fn refresh_effects(
        &mut self,
        models: &[ModSpellEffectModel<'_>],
        projection: ModWorldProjection,
        images: &ModPresentationAssets,
    ) -> Result<(), String> {
        let live: BTreeSet<_> = models.iter().map(|effect| effect.id).collect();
        self.effect_nodes.retain(|id, nodes| {
            if live.contains(id) {
                true
            } else {
                nodes.root.remove();
                false
            }
        });
        for (index, effect) in models.iter().enumerate() {
            if !self.effect_nodes.contains_key(&effect.id) {
                let root = element(&self.document, "div", "mod-spell-effect")?;
                let sprite = element(&self.document, "span", "mod-spell-effect__sprite")?;
                append(&root, &sprite)?;
                self.effect_nodes.insert(effect.id, EffectNodes { root, sprite, decoration: None });
            }
            let nodes = self.effect_nodes.get_mut(&effect.id).expect("admitted mod effect");
            attribute(&nodes.root, "data-effect-id", &effect.id.to_string())?;
            attribute(&nodes.root, "data-kind", effect.kind.as_str())?;
            if nodes.decoration.as_ref().is_some_and(|(kind, _)| *kind != effect.kind) {
                nodes.decoration.take().expect("existing effect decoration").1.remove();
            }
            let point = projection.screen_position(effect.position);
            if effect.kind != ModSpellEffectKind::Projectile {
                if nodes.decoration.is_none() {
                    let class = if effect.kind == ModSpellEffectKind::Area {
                        "mod-spell-effect__area"
                    } else {
                        "mod-spell-effect__channel"
                    };
                    let decoration = element(&self.document, "span", class)?;
                    nodes
                        .root
                        .insert_before(&decoration, Some(&nodes.sprite))
                        .map_err(|_| "Could not attach mod field")?;
                    nodes.decoration = Some((effect.kind, decoration));
                }
                let decoration = &nodes.decoration.as_ref().expect("admitted effect decoration").1;
                let (width, height) = if effect.kind == ModSpellEffectKind::Area {
                    (effect.radius * 2.0 * projection.zoom, effect.radius * 2.0 * projection.zoom)
                } else {
                    let dx = (effect.target.x - effect.position.x) * projection.zoom;
                    let dy = (effect.target.y - effect.position.y) * projection.zoom;
                    property(
                        decoration,
                        "transform",
                        &format!("translateY(-50%) rotate({}rad)", dy.atan2(dx)),
                    )?;
                    (
                        solomon_sim::float::hypot(dx, dy),
                        (effect.radius * 2.0 * projection.zoom).max(4.0),
                    )
                };
                property(decoration, "left", &format!("{}px", point.x))?;
                property(decoration, "top", &format!("{}px", point.y))?;
                property(decoration, "width", &format!("{width}px"))?;
                property(decoration, "height", &format!("{height}px"))?;
            }
            let scale =
                (96.0 / effect.frame.width.max(effect.frame.height)).min(1.0) * projection.zoom;
            sprite_style(
                &nodes.sprite,
                effect.image,
                effect.frame,
                effect.image_size,
                scale,
                point,
                images,
            )?;
            place_in_order(&self.effect_root, &nodes.root, index)?;
        }
        Ok(())
    }

    fn refresh_powerups(
        &mut self,
        models: &[ModPowerupModel<'_>],
        events: &[ModPowerupEventModel<'_>],
        projection: ModWorldProjection,
        images: &ModPresentationAssets,
    ) -> Result<(), String> {
        let live: BTreeSet<_> = models.iter().map(|actor| actor.id).collect();
        self.powerup_nodes.retain(|id, node| {
            if live.contains(id) {
                true
            } else {
                node.remove();
                false
            }
        });
        let live_pickups: BTreeSet<_> = events.iter().map(|event| (event.id, event.tick)).collect();
        self.pickup_nodes.retain(|key, node| {
            if live_pickups.contains(key) {
                true
            } else {
                node.remove();
                false
            }
        });
        for (index, actor) in models.iter().enumerate() {
            if !self.powerup_nodes.contains_key(&actor.id) {
                self.powerup_nodes
                    .insert(actor.id, element(&self.document, "span", "mod-powerup")?);
            }
            let node = &self.powerup_nodes[&actor.id];
            let scale =
                (72.0 / actor.frame.width.max(actor.frame.height)).min(1.0) * projection.zoom;
            sprite_style(
                node,
                actor.image,
                actor.frame,
                actor.image_size,
                scale,
                projection.screen_position(actor.position),
                images,
            )?;
            attribute(node, "data-powerup-id", &actor.id.to_string())?;
            place_in_order(&self.powerup_root, node, index)?;
        }
        for (index, event) in events.iter().enumerate() {
            let key = (event.id, event.tick);
            if !self.pickup_nodes.contains_key(&key) {
                self.pickup_nodes.insert(key, element(&self.document, "i", "mod-powerup-pickup")?);
            }
            let node = &self.pickup_nodes[&key];
            let point = projection.screen_position(event.position);
            style(node, &format!("left:{}px;top:{}px", point.x, point.y))?;
            place_in_order(&self.powerup_root, node, models.len() + index)?;
        }
        Ok(())
    }

    fn refresh_shops(
        &mut self,
        models: &[ModShopNpcModel<'_>],
        projection: ModWorldProjection,
        images: &ModPresentationAssets,
    ) -> Result<(), String> {
        for (index, shop) in models.iter().enumerate() {
            let image = shop.image.is_some();
            if self.shop_nodes.get(&index).is_some_and(|nodes| nodes.image != image) {
                self.shop_nodes.remove(&index).expect("existing shop icon").root.remove();
            }
            if !self.shop_nodes.contains_key(&index) {
                let root = element(&self.document, "div", "")?;
                let icon = element(&self.document, if image { "img" } else { "i" }, "")?;
                if image {
                    attribute(&icon, "alt", "")?;
                }
                let name = element(&self.document, "span", "")?;
                append(&root, &icon)?;
                append(&root, &name)?;
                self.shop_nodes.insert(index, ShopNodes { root, icon, name, image });
            }
            let nodes = &self.shop_nodes[&index];
            if let Some(image) = shop.image {
                attribute(&nodes.icon, "src", images.image(&image.mod_id, &image.path)?.url())?;
            }
            let point = projection.screen_position(shop.position);
            style(&nodes.root, &format!("left:{}px;top:{}px", point.x, point.y))?;
            nodes.name.set_text_content(Some(shop.name));
            place_in_order(&self.shop_root, &nodes.root, index)?;
        }
        self.shop_nodes.retain(|&index, nodes| {
            if index < models.len() {
                true
            } else {
                nodes.root.remove();
                false
            }
        });
        Ok(())
    }
}

impl Drop for ModWorldOverlays {
    fn drop(&mut self) {
        self.root.remove();
    }
}

#[allow(clippy::too_many_arguments)] // The Source CSS crop uses these sprite properties independently.
fn sprite_style(
    node: &HtmlElement,
    image: &GameModAsset,
    frame: ModImageFrame,
    image_size: Vec2,
    scale: f64,
    point: Vec2,
    images: &ModPresentationAssets,
) -> Result<(), String> {
    let source = serde_json::to_string(images.image(&image.mod_id, &image.path)?.url())
        .map_err(|error| error.to_string())?;
    style(
        node,
        &format!(
            "background-image:url({source});background-position:{}px {}px;background-size:{}px {}px;width:{}px;height:{}px;left:{}px;top:{}px",
            -frame.x * scale,
            -frame.y * scale,
            image_size.x * scale,
            image_size.y * scale,
            frame.width * scale,
            frame.height * scale,
            point.x,
            point.y,
        ),
    )
}

fn packed_color(color: i64) -> String {
    let hex = if color < 0 { format!("-{:x}", color.unsigned_abs()) } else { format!("{color:x}") };
    format!("{hex:0>6}")
}

fn attribute(node: &HtmlElement, name: &str, value: &str) -> Result<(), String> {
    node.set_attribute(name, value).map_err(|_| format!("Could not set mod {name}"))
}

fn property(node: &HtmlElement, name: &str, value: &str) -> Result<(), String> {
    node.style().set_property(name, value).map_err(|_| format!("Could not set mod {name}"))
}

fn place_in_order(parent: &HtmlElement, node: &HtmlElement, index: usize) -> Result<(), String> {
    let current = parent.children().item(index as u32);
    if current.as_ref().is_some_and(|current| current.is_same_node(Some(node))) {
        return Ok(());
    }
    // A no-op DOM append can restart CSS animations. Source React retains an unchanged
    // keyed child, so only insert/move when its actual order changed.
    parent
        .insert_before(node, current.as_ref().map(|current| current.as_ref()))
        .map(|_| ())
        .map_err(|_| "Could not order mod world overlays".into())
}
