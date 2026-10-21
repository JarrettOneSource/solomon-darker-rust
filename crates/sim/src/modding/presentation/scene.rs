//! The authored SVG scene, from `mod-ui/mod-scene-model.ts`.

use super::{integer, rows, text};
use crate::modding::{LuaObject, LuaValue};

#[derive(Clone, Debug, PartialEq)]
pub struct ModRoomModel<'a> {
    pub content_id: &'a str,
    pub description: &'a str,
    pub floor: &'a str,
    pub height: f64,
    pub name: &'a str,
    pub props: Vec<&'a LuaObject>,
    pub walls: Vec<&'a LuaObject>,
    pub width: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ModSceneModel<'a> {
    pub content_id: &'a str,
    pub epoch: i64,
    pub room_index: usize,
    pub rooms: Vec<ModRoomModel<'a>>,
}

#[must_use]
pub fn project_mod_scene<'a>(
    runtime: Option<&'a LuaObject>,
    owner_id: &str,
) -> Option<ModSceneModel<'a>> {
    let source = rows(runtime, "scenes")
        .into_iter()
        .find(|scene| scene.get("owner_id").and_then(LuaValue::as_str) == Some(owner_id))?;
    let mut rooms = Vec::new();
    for room in rows(Some(source), "rooms") {
        let geometry = room.get("geometry").and_then(LuaValue::as_object);
        let name = text(room.get("name"));
        rooms.push(ModRoomModel {
            content_id: text(room.get("content_id")),
            description: text(room.get("description")),
            floor: mod_scene_color(geometry.and_then(|geometry| geometry.get("floor")), "#201c2b"),
            height: bounded(geometry.and_then(|geometry| geometry.get("height")), 720.0),
            name: if name.is_empty() { "Room" } else { name },
            props: rows(Some(room), "props"),
            walls: rows(geometry, "walls"),
            width: bounded(geometry.and_then(|geometry| geometry.get("width")), 1120.0),
        });
    }
    let index = integer(source.get("room_index"));
    let content_id = text(source.get("scene_content_id"));
    if content_id.is_empty() || rooms.is_empty() || index < 0 || index as usize >= rooms.len() {
        return None;
    }
    Some(ModSceneModel {
        content_id,
        epoch: integer(source.get("epoch")),
        room_index: index as usize,
        rooms,
    })
}

#[must_use]
pub fn mod_scene_color<'a>(value: Option<&'a LuaValue>, fallback: &'a str) -> &'a str {
    let color = text(value);
    if color.len() == 7
        && color.starts_with('#')
        && color[1..].bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        color
    } else {
        fallback
    }
}

#[must_use]
pub fn mod_scene_number(value: Option<&LuaValue>, fallback: f64) -> f64 {
    value.and_then(LuaValue::as_number).filter(|value| value.is_finite()).unwrap_or(fallback)
}

fn bounded(value: Option<&LuaValue>, fallback: f64) -> f64 {
    let number = mod_scene_number(value, fallback);
    if (160.0..=4096.0).contains(&number) { number } else { fallback }
}
