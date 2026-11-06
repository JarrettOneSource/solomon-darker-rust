//! The mod skill quickbar: its slots, Shift+number casting and the cast intent.
//! Accepted runtime rows own bindings/spells; this projection and cast intent own no
//! request sequence, cooldown, player state or gameplay clock.

use std::collections::BTreeMap;

use super::LuaObject;
use super::assets::GameModAsset;
use super::presentation::{integer, rows, text};
use crate::Vec2;
use crate::heading::heading_vector;

/// One mod skill quickbar slot: its slot number, spell and icon.
pub struct ModSkillQuickbarSlot<'a> {
    pub slot: i64,
    pub content_id: &'a str,
    pub name: &'a str,
    pub image: Option<&'a GameModAsset>,
}

/// The local player's position and heading, used to aim a quickbar cast.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModSkillQuickbarPlayer {
    pub position: Vec2,
    pub heading_index: usize,
}

/// The existing hosted sender supplies its one canonical request ID.
#[derive(Clone, Debug, PartialEq)]
pub struct ModCastAction {
    pub content_id: String,
    pub target: Vec2,
}

/// A key press the quickbar may handle: its numeric value and its modifier and repeat flags.
#[derive(Clone, Copy, Debug)]
pub struct ModSkillQuickbarKey {
    /// The key's text as a number; NaN when it is not numeric, as for shifted punctuation.
    pub number: f64,
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
    pub meta: bool,
    pub repeat: bool,
}

#[must_use]
pub fn project_mod_skill_quickbar<'a>(
    runtime: Option<&'a LuaObject>,
    assets: &'a [GameModAsset],
) -> Vec<ModSkillQuickbarSlot<'a>> {
    let mut spells = BTreeMap::new();
    for spell in rows(runtime, "spells") {
        // The map keeps the last spell with a given content ID.
        spells.insert(text(spell.get("content_id")), spell);
    }
    let mut bindings = rows(runtime, "mod_quickbar");
    bindings.sort_by_key(|binding| integer(binding.get("slot")));
    let mut slots = Vec::new();
    for binding in bindings {
        let Some(spell) = spells.get(text(binding.get("content_id"))).copied() else {
            continue;
        };
        let image = assets.iter().find(|asset| {
            Some(asset.mod_id.as_str()) == spell.get("mod_id").and_then(super::LuaValue::as_str)
                && Some(asset.path.as_str())
                    == spell.get("icon_path").and_then(super::LuaValue::as_str)
                && asset.kind == "image"
        });
        slots.push(ModSkillQuickbarSlot {
            slot: integer(binding.get("slot")),
            content_id: text(spell.get("content_id")),
            name: text(spell.get("name")),
            image,
        });
    }
    slots
}

/// The slot a Shift+number press selects. It deliberately uses the key's numeric value,
/// not its physical code, and has no editable-focus or previously-prevented-event exception.
#[must_use]
pub fn mod_skill_quickbar_keyboard_slot(key: ModSkillQuickbarKey) -> Option<i64> {
    if !key.shift || key.control || key.alt || key.meta || key.repeat {
        return None;
    }
    let slot = key.number - 1.0;
    // Every displayed slot is a safe integer, so no other numeric event value can
    // equal it, including fractional, NaN or infinite keys.
    if !slot.is_finite() || slot.fract() != 0.0 || slot.abs() > 9_007_199_254_740_991.0 {
        return None;
    }
    Some(slot as i64)
}

#[must_use]
pub fn cast_mod_skill_quickbar_spell(
    content_id: &str,
    player: Option<ModSkillQuickbarPlayer>,
) -> Option<ModCastAction> {
    let player = player?;
    let heading = heading_vector(player.heading_index);
    Some(ModCastAction {
        content_id: content_id.to_owned(),
        target: Vec2::new(
            player.position.x + heading.x * 300.0,
            player.position.y + heading.y * 300.0,
        ),
    })
}
