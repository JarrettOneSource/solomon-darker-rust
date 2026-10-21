//! `mod-ui/mod-minimap.ts`: the authored minimap projection. Native players and
//! enemies enter as borrowed accepted positions, never as another stepped world.

use crate::Vec2;
use crate::modding::definition::WebLuaContentKind;
use crate::modding::projection::ModContentProjection;
use crate::modding::{LuaObject, LuaValue};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModMinimapPlayer<'a> {
    pub id: &'a str,
    pub position: Vec2,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModMinimapEnemy {
    pub id: f64,
    pub position: Vec2,
}

pub struct ModMinimapSnapshot<'a> {
    /// Preserves the actual snapshot's Object.entries order.
    pub players: &'a [ModMinimapPlayer<'a>],
    pub enemies: &'a [ModMinimapEnemy],
    pub in_boneyard: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModMinimapMarkerKind {
    Enemy,
    Party,
    Powerup,
    SelfPlayer,
}

impl ModMinimapMarkerKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Enemy => "enemy",
            Self::Party => "party",
            Self::Powerup => "powerup",
            Self::SelfPlayer => "self",
        }
    }
}

/// Keep Source Number IDs until the browser's native String conversion makes a
/// DOM key. Authored IDs are not narrowed to an integer machine-word range.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ModMinimapMarkerId<'a> {
    Player(&'a str),
    Enemy(f64),
    ModEnemy(f64),
    Powerup(f64),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModMinimapMarker<'a> {
    pub id: ModMinimapMarkerId<'a>,
    pub kind: ModMinimapMarkerKind,
    pub position: Vec2,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ModMinimapModel<'a> {
    pub accessible_name: &'a str,
    pub actions: Vec<&'a str>,
    pub bindings: Option<&'a LuaObject>,
    pub center: Vec2,
    pub content_id: &'a str,
    pub markers: Vec<ModMinimapMarker<'a>>,
    pub mount: &'a str,
    pub range: f64,
    pub size: Vec2,
}

#[must_use]
pub fn project_mod_minimap<'a>(
    snapshot: &ModMinimapSnapshot<'a>,
    content: &'a ModContentProjection,
    viewer_id: &str,
    runtime: Option<&'a LuaObject>,
) -> Option<ModMinimapModel<'a>> {
    let definition = content.content.iter().find(|entry| {
        entry.content_kind == WebLuaContentKind::Ui
            && entry.presentation.as_deref() == Some("prefab.minimap")
    })?;
    let viewer = snapshot.players.iter().find(|player| player.id == viewer_id)?;
    let surfaces: Vec<_> = runtime
        .and_then(|value| value.get("ui_surfaces"))
        .and_then(LuaValue::as_array)
        .unwrap_or_default()
        .iter()
        .filter_map(LuaValue::as_object)
        .collect();
    let surface = surfaces.iter().copied().find(|surface| {
        surface.get("view").and_then(|view| view.get("operation")).and_then(LuaValue::as_str)
            == Some("prefab.minimap")
    });
    if !surfaces.is_empty() && surface.is_none() {
        return None;
    }
    let fields =
        surface.and_then(|surface| surface.get("view")).and_then(|view| view.get("fields"));
    let layers: Vec<_> = fields
        .and_then(|fields| fields.get("layers"))
        .and_then(LuaValue::as_array)
        .unwrap_or_default()
        .iter()
        .filter_map(|value| {
            value.as_str().or_else(|| value.get("source").and_then(LuaValue::as_str))
        })
        .collect();
    let includes = |name| layers.is_empty() || layers.contains(&name);
    let mut markers = Vec::new();
    for player in snapshot.players {
        let own_player = player.id == viewer_id;
        if includes("party") || own_player {
            markers.push(ModMinimapMarker {
                id: ModMinimapMarkerId::Player(player.id),
                kind: if own_player {
                    ModMinimapMarkerKind::SelfPlayer
                } else {
                    ModMinimapMarkerKind::Party
                },
                position: player.position,
            });
        }
    }
    if snapshot.in_boneyard && includes("visible_hostiles") {
        for enemy in snapshot.enemies {
            markers.push(ModMinimapMarker {
                id: ModMinimapMarkerId::Enemy(enemy.id),
                kind: ModMinimapMarkerKind::Enemy,
                position: enemy.position,
            });
        }
        for value in runtime
            .and_then(|runtime| runtime.get("enemies"))
            .and_then(LuaValue::as_array)
            .unwrap_or_default()
        {
            let (Some(id), Some(x), Some(y)) = (
                value.get("id").and_then(LuaValue::as_number),
                value.get("x").and_then(LuaValue::as_number),
                value.get("y").and_then(LuaValue::as_number),
            ) else {
                continue;
            };
            markers.push(ModMinimapMarker {
                id: ModMinimapMarkerId::ModEnemy(id),
                kind: ModMinimapMarkerKind::Enemy,
                position: Vec2::new(x, y),
            });
        }
    }
    if includes("powerups") {
        for powerup in &content.powerups {
            markers.push(ModMinimapMarker {
                id: ModMinimapMarkerId::Powerup(powerup.id),
                kind: ModMinimapMarkerKind::Powerup,
                position: Vec2::new(powerup.x, powerup.y),
            });
        }
    }
    let size = fields.and_then(|fields| fields.get("size"));
    let scalar_size = size.and_then(LuaValue::as_number).unwrap_or(220.0);
    Some(ModMinimapModel {
        accessible_name: nonempty_text(surface.and_then(|row| row.get("accessible_name")))
            .unwrap_or(&definition.name),
        actions: surface
            .and_then(|row| row.get("actions"))
            .and_then(LuaValue::as_array)
            .unwrap_or_default()
            .iter()
            .filter_map(LuaValue::as_str)
            .collect(),
        bindings: surface.and_then(|row| row.get("bindings")).and_then(LuaValue::as_object),
        center: viewer.position,
        content_id: nonempty_text(surface.and_then(|row| row.get("content_id")))
            .unwrap_or(&definition.content_id),
        markers,
        mount: nonempty_text(surface.and_then(|row| row.get("mount"))).unwrap_or("hud.top_right"),
        range: positive(fields.and_then(|fields| fields.get("range")), 500.0),
        size: Vec2::new(
            positive(size.and_then(|size| size.get("width")), scalar_size),
            positive(size.and_then(|size| size.get("height")), scalar_size),
        ),
    })
}

fn nonempty_text(value: Option<&LuaValue>) -> Option<&str> {
    value.and_then(LuaValue::as_str).filter(|value| !value.is_empty())
}

fn positive(value: Option<&LuaValue>, fallback: f64) -> f64 {
    value
        .and_then(LuaValue::as_number)
        .filter(|value| value.is_finite() && *value > 0.0)
        .unwrap_or(fallback)
}
