//! Borrowed rows from Source `mod-ui/ModSkillBook.tsx` and `ModSkillPicker.tsx`.
//! The accepted runtime owns ranks, offers, spells and bindings. These functions
//! preserve the different first/last-row lookups used by the two Source views.

use crate::modding::{LuaObject, LuaValue};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModSkillIcon<'a> {
    pub mod_id: &'a str,
    pub path: &'a str,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModLearnedSkill<'a> {
    pub content_id: &'a str,
    pub name: &'a str,
    pub description: &'a str,
    pub icon: ModSkillIcon<'a>,
    pub rank: f64,
    pub maximum_rank: f64,
    pub subskill: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModSpellBinding<'a> {
    pub content_id: &'a str,
    pub name: &'a str,
    pub icon: ModSkillIcon<'a>,
    /// A bound row keeps its actual safe-integer slot, including out-of-range
    /// values. Only the authority admits a binding change.
    pub slot: Option<f64>,
    pub bound: bool,
}

#[derive(Debug, PartialEq)]
pub struct ModSkillBookModel<'a> {
    pub learned: Vec<ModLearnedSkill<'a>>,
    pub spells: Vec<ModSpellBinding<'a>>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModOfferedSkill<'a> {
    pub content_id: &'a str,
    pub name: &'a str,
    pub description: &'a str,
    pub icon: ModSkillIcon<'a>,
    pub next_rank: f64,
}

#[derive(Debug, PartialEq)]
pub struct ModSkillPickerModel<'a> {
    pub sequence: f64,
    pub skills: Vec<ModOfferedSkill<'a>>,
}

#[must_use]
pub fn project_mod_skill_book(runtime: Option<&LuaObject>) -> Option<ModSkillBookModel<'_>> {
    let runtime = runtime?;
    let ranks: Vec<_> = rows(runtime.get("skill_ranks")).collect();
    let mut learned = Vec::new();
    for skill in rows(runtime.get("skills")) {
        let content_id = text(skill.get("content_id"));
        // Source builds a Map: duplicate rank rows are last-wins here.
        let rank = ranks
            .iter()
            .rev()
            .find(|rank| text(rank.get("content_id")) == content_id)
            .map_or(0.0, |rank| integer(rank.get("rank")));
        if rank > 0.0 {
            learned.push(ModLearnedSkill {
                content_id,
                name: text(skill.get("name")),
                description: text(skill.get("description")),
                icon: icon(skill),
                rank,
                maximum_rank: integer(skill.get("maximum_rank")),
                subskill: truthy(skill.get("parent_content_id")),
            });
        }
    }
    let bindings: Vec<_> = rows(runtime.get("mod_quickbar")).collect();
    let open_slot = (0..8).find(|slot| {
        !bindings.iter().any(|binding| integer(binding.get("slot")) == f64::from(*slot))
    });
    let mut spells = Vec::new();
    for spell in rows(runtime.get("spells")) {
        let content_id = text(spell.get("content_id"));
        let binding = bindings.iter().find(|binding| {
            binding.get("content_id").and_then(LuaValue::as_str) == Some(content_id)
        });
        spells.push(ModSpellBinding {
            content_id,
            name: text(spell.get("name")),
            icon: icon(spell),
            slot: binding.map(|row| integer(row.get("slot"))).or(open_slot.map(f64::from)),
            bound: binding.is_some(),
        });
    }
    if learned.is_empty() && spells.is_empty() {
        return None;
    }
    Some(ModSkillBookModel { learned, spells })
}

#[must_use]
pub fn project_mod_skill_picker<'a>(
    runtime: Option<&'a LuaObject>,
    player_id: &str,
) -> Option<ModSkillPickerModel<'a>> {
    let runtime = runtime?;
    let offer = rows(runtime.get("skill_offers"))
        .find(|row| row.get("player_id").and_then(LuaValue::as_str) == Some(player_id))?;
    let sequence = integer(offer.get("sequence"));
    if sequence < 1.0 {
        return None;
    }
    let offered = offer.get("content_ids").and_then(LuaValue::as_array).unwrap_or_default();
    let mut skills = Vec::new();
    for skill in rows(runtime.get("skills")) {
        let content_id = text(skill.get("content_id"));
        if !offered.iter().any(|id| id.as_str() == Some(content_id)) {
            continue;
        }
        // Source Picker uses find rather than the Book's last-wins Map.
        let rank = rows(runtime.get("skill_ranks"))
            .find(|rank| rank.get("content_id") == skill.get("content_id"))
            .map_or(0.0, |rank| integer(rank.get("rank")));
        skills.push(ModOfferedSkill {
            content_id,
            name: text(skill.get("name")),
            description: text(skill.get("description")),
            icon: icon(skill),
            next_rank: rank + 1.0,
        });
    }
    if skills.is_empty() {
        return None;
    }
    Some(ModSkillPickerModel { sequence, skills })
}

fn rows(value: Option<&LuaValue>) -> impl Iterator<Item = &LuaObject> {
    value.and_then(LuaValue::as_array).unwrap_or_default().iter().filter_map(LuaValue::as_object)
}

fn text(value: Option<&LuaValue>) -> &str {
    value.and_then(LuaValue::as_str).unwrap_or_default()
}

fn icon(row: &LuaObject) -> ModSkillIcon<'_> {
    ModSkillIcon { mod_id: text(row.get("mod_id")), path: text(row.get("icon_path")) }
}

fn integer(value: Option<&LuaValue>) -> f64 {
    match value.and_then(LuaValue::as_number) {
        Some(number) if number.fract() == 0.0 && number.abs() <= 9_007_199_254_740_991.0 => number,
        _ => 0.0,
    }
}

fn truthy(value: Option<&LuaValue>) -> bool {
    match value {
        None | Some(LuaValue::Null) => false,
        Some(LuaValue::Boolean(value)) => *value,
        Some(LuaValue::Number(value)) => *value != 0.0 && !value.is_nan(),
        Some(LuaValue::String(value)) => !value.is_empty(),
        Some(LuaValue::Array(_) | LuaValue::Object(_)) => true,
    }
}
