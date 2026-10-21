//! Borrowed world models from Website `mod-ui/mod-{enemies,spell-effects,scene-model}.ts`
//! and the Powerup/Shop component projections. These interpret the accepted Lua snapshot;
//! they do not spawn, step, repair or retain a second set of mod actors.

mod scene;
pub use scene::{
    ModRoomModel, ModSceneModel, mod_scene_color, mod_scene_number, project_mod_scene,
};

use super::assets::GameModAsset;
use super::{LuaObject, LuaValue};
use crate::Vec2;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModImageFrame {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModEnemyLifeState {
    Alive,
    Dying,
}

impl ModEnemyLifeState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Alive => "alive",
            Self::Dying => "dying",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ModEnemyModel<'a> {
    pub current_health: f64,
    pub frame: ModImageFrame,
    pub id: i64,
    pub image: &'a GameModAsset,
    pub image_size: Vec2,
    pub life_state: ModEnemyLifeState,
    pub light_color: i64,
    pub light_radius: f64,
    pub maximum_health: f64,
    pub name: &'a str,
    pub scale: f64,
    pub sound_event_tick: Option<i64>,
    pub sound: Option<&'a GameModAsset>,
    pub sound_volume: f64,
    pub position: Vec2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModSpellEffectKind {
    Area,
    Channel,
    Projectile,
}

impl ModSpellEffectKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Area => "area",
            Self::Channel => "channel",
            Self::Projectile => "projectile",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ModSpellEffectModel<'a> {
    pub frame: ModImageFrame,
    pub id: i64,
    pub image: &'a GameModAsset,
    pub image_size: Vec2,
    pub kind: ModSpellEffectKind,
    pub radius: f64,
    pub sound: Option<&'a GameModAsset>,
    pub sound_volume: f64,
    pub started_tick: i64,
    pub target: Vec2,
    pub position: Vec2,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ModPowerupModel<'a> {
    pub frame: ModImageFrame,
    pub id: i64,
    pub image: &'a GameModAsset,
    pub image_size: Vec2,
    pub position: Vec2,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ModPowerupEventModel<'a> {
    pub id: i64,
    pub tick: i64,
    pub sound: Option<&'a GameModAsset>,
    pub sound_volume: f64,
    pub position: Vec2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModWorldLocation {
    HubCourtyard,
    HubPrivate,
    Boneyard,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ModShopNpcModel<'a> {
    pub image: Option<&'a GameModAsset>,
    pub name: &'a str,
    pub position: Vec2,
}

#[must_use]
pub fn project_mod_enemies<'a>(
    runtime: Option<&'a LuaObject>,
    assets: &'a [GameModAsset],
) -> Vec<ModEnemyModel<'a>> {
    let mut models = Vec::new();
    for row in rows(runtime, "enemies") {
        let mod_id = text(row.get("mod_id"));
        let image_path = text(row.get("image_path"));
        let life_state = match text(row.get("life_state")) {
            "alive" => ModEnemyLifeState::Alive,
            "dying" => ModEnemyLifeState::Dying,
            _ => continue,
        };
        if mod_id.is_empty() || image_path.is_empty() {
            continue;
        }
        let Some(image) = asset(assets, mod_id, image_path, "image") else { continue };
        let sound = asset(assets, mod_id, text(row.get("sound_path")), "audio");
        let model = ModEnemyModel {
            current_health: finite(row.get("current_health")),
            frame: frame(row),
            id: integer(row.get("id")),
            image,
            image_size: image_size(row),
            life_state,
            light_color: integer(row.get("light_color")),
            light_radius: finite(row.get("light_radius")),
            maximum_health: finite(row.get("maximum_health")),
            name: text(row.get("name")),
            scale: finite(row.get("scale")),
            sound_event_tick: if row.get("sound_event_tick") == Some(&LuaValue::Null) {
                None
            } else {
                Some(integer(row.get("sound_event_tick")))
            },
            sound,
            sound_volume: finite(row.get("sound_volume")),
            position: position(row),
        };
        if model.id > 0
            && model.frame.width > 0.0
            && model.frame.height > 0.0
            && model.maximum_health > 0.0
            && model.scale > 0.0
        {
            models.push(model);
        }
    }
    models
}

#[must_use]
pub fn project_mod_spell_effects<'a>(
    runtime: Option<&'a LuaObject>,
    assets: &'a [GameModAsset],
) -> Vec<ModSpellEffectModel<'a>> {
    let mut models = Vec::new();
    for row in rows(runtime, "spell_effects") {
        let kind = match text(row.get("kind")) {
            "area" => ModSpellEffectKind::Area,
            "channel" => ModSpellEffectKind::Channel,
            "projectile" => ModSpellEffectKind::Projectile,
            _ => continue,
        };
        let (Some(mod_id), Some(image_path)) = (
            row.get("mod_id").and_then(LuaValue::as_str),
            row.get("image_path").and_then(LuaValue::as_str),
        ) else {
            continue;
        };
        let Some(image) = asset(assets, mod_id, image_path, "image") else { continue };
        let model = ModSpellEffectModel {
            frame: frame(row),
            id: integer(row.get("id")),
            image,
            image_size: image_size(row),
            kind,
            radius: finite(row.get("radius")),
            sound: asset(assets, mod_id, text(row.get("sound_path")), "audio"),
            sound_volume: finite(row.get("sound_volume")),
            started_tick: integer(row.get("started_tick")),
            target: Vec2::new(finite(row.get("target_x")), finite(row.get("target_y"))),
            position: position(row),
        };
        if model.id > 0 && model.frame.width > 0.0 && model.frame.height > 0.0 {
            models.push(model);
        }
    }
    models
}

#[must_use]
pub fn project_mod_powerups<'a>(
    runtime: Option<&'a LuaObject>,
    assets: &'a [GameModAsset],
) -> Vec<ModPowerupModel<'a>> {
    let mut models = Vec::new();
    for row in rows(runtime, "powerup_actors") {
        let Some(image) =
            asset(assets, text(row.get("mod_id")), text(row.get("image_path")), "image")
        else {
            continue;
        };
        models.push(ModPowerupModel {
            frame: frame(row),
            id: integer(row.get("id")),
            image,
            image_size: image_size(row),
            position: position(row),
        });
    }
    models
}

#[must_use]
pub fn project_mod_powerup_events<'a>(
    runtime: Option<&'a LuaObject>,
    assets: &'a [GameModAsset],
) -> Vec<ModPowerupEventModel<'a>> {
    let mut models = Vec::new();
    for row in rows(runtime, "powerup_events") {
        models.push(ModPowerupEventModel {
            id: integer(row.get("id")),
            tick: integer(row.get("tick")),
            sound: asset(assets, text(row.get("mod_id")), text(row.get("sound_path")), "audio"),
            sound_volume: finite(row.get("sound_volume")),
            position: position(row),
        });
    }
    models
}

#[must_use]
pub fn project_mod_shop_npcs<'a>(
    runtime: Option<&'a LuaObject>,
    assets: &'a [GameModAsset],
    location: ModWorldLocation,
) -> Vec<ModShopNpcModel<'a>> {
    let mut models = Vec::new();
    for row in rows(runtime, "shops") {
        let mount = row.get("mount").and_then(LuaValue::as_object);
        let scene = text(mount.and_then(|mount| mount.get("scene")));
        if scene == "hub.courtyard" && location != ModWorldLocation::HubCourtyard
            || scene == "boneyard" && location != ModWorldLocation::Boneyard
        {
            continue;
        }
        let (Some(x), Some(y)) = (
            mount.and_then(|mount| mount.get("x")).and_then(LuaValue::as_number),
            mount.and_then(|mount| mount.get("y")).and_then(LuaValue::as_number),
        ) else {
            continue;
        };
        let npc_name = text(row.get("npc").and_then(|npc| npc.get("name")));
        models.push(ModShopNpcModel {
            image: asset(assets, text(row.get("mod_id")), text(row.get("icon_path")), "image"),
            name: if npc_name.is_empty() { text(row.get("name")) } else { npc_name },
            position: Vec2::new(x, y),
        });
    }
    models
}

pub(super) fn rows<'a>(runtime: Option<&'a LuaObject>, key: &str) -> Vec<&'a LuaObject> {
    let values = runtime.and_then(|runtime| runtime.get(key)).and_then(LuaValue::as_array);
    let mut output = Vec::new();
    if let Some(values) = values {
        for value in values {
            if let Some(row) = value.as_object() {
                output.push(row);
            }
        }
    }
    output
}

pub(super) fn text(value: Option<&LuaValue>) -> &str {
    value.and_then(LuaValue::as_str).unwrap_or("")
}

fn finite(value: Option<&LuaValue>) -> f64 {
    value.and_then(LuaValue::as_number).filter(|value| value.is_finite()).unwrap_or(0.0)
}

pub(super) fn integer(value: Option<&LuaValue>) -> i64 {
    let value = finite(value);
    if value.fract() == 0.0 && value.abs() <= 9_007_199_254_740_991.0 { value as i64 } else { 0 }
}

fn frame(row: &LuaObject) -> ModImageFrame {
    ModImageFrame {
        x: finite(row.get("frame_x")),
        y: finite(row.get("frame_y")),
        width: finite(row.get("frame_width")),
        height: finite(row.get("frame_height")),
    }
}

fn image_size(row: &LuaObject) -> Vec2 {
    Vec2::new(finite(row.get("image_width")), finite(row.get("image_height")))
}

fn position(row: &LuaObject) -> Vec2 {
    Vec2::new(finite(row.get("x")), finite(row.get("y")))
}

fn asset<'a>(
    assets: &'a [GameModAsset],
    mod_id: &str,
    path: &str,
    kind: &str,
) -> Option<&'a GameModAsset> {
    if path.is_empty() {
        return None;
    }
    assets.iter().find(|asset| asset.mod_id == mod_id && asset.path == path && asset.kind == kind)
}
