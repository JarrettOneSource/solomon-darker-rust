//! Status, skill, spell and enemy rows from `mod-content-catalog.ts`.

use std::collections::BTreeSet;

use solomon_sim::modding::LuaValue;
use solomon_sim::modding::content::*;
use solomon_sim::modding::definition::{
    ResolvedWebLuaContentReference, WebLuaAssetKind, WebLuaContentKind,
};

use super::empty_array;
use super::fields::*;

pub(super) fn status(
    common: PreparedModContentEntry,
) -> Result<PreparedModStatusDefinition, String> {
    let modifiers =
        optional_object(common.fields.get("modifiers"), &format!("{}.modifiers", common.key))?;
    exact_keys(
        &modifiers,
        &["incoming_damage", "mana_spend"],
        &format!("{}.modifiers", common.key),
    )?;
    validate_modifier_map(&modifiers, &format!("{}.modifiers", common.key))?;
    let duration_ms = duration_ms(
        nullish(common.fields.get("duration")).or(Some(&LuaValue::Number(0.0))),
        &format!("{}.duration", common.key),
    )?;
    let stacking = match common.fields.get("stacking").map(LuaValue::as_str) {
        None | Some(Some("refresh")) => ModStatusStacking::Refresh,
        Some(Some("ignore")) => ModStatusStacking::Ignore,
        Some(Some("replace")) => ModStatusStacking::Replace,
        Some(Some("stack")) => ModStatusStacking::Stack,
        _ => return Err(format!("{}.stacking is not a supported stacking policy", common.key)),
    };
    Ok(PreparedModStatusDefinition { common, duration_ms, modifiers, stacking })
}

pub(super) fn powerup(
    common: PreparedModContentEntry,
) -> Result<PreparedModPowerupDefinition, String> {
    if !has_image(&common, "world") {
        return Err(format!("{}:{} powerup requires image art.world", common.mod_id, common.key));
    }
    if common.art.get("sound").is_some_and(|binding| binding.asset_kind != WebLuaAssetKind::Sound) {
        return Err(format!("{}:{} powerup art.sound must be a sound", common.mod_id, common.key));
    }
    let effect = rule(
        common.fields.get("effect"),
        format!("{}:{} powerup requires an effect rule", common.mod_id, common.key),
    )?;
    let pickup = object_or_empty(common.fields.get("pickup"), &format!("{}.pickup", common.key))?;
    exact_keys(&pickup, &["radius"], &format!("{}.pickup", common.key))?;
    let pickup_radius = number_or(
        pickup.get("radius"),
        32.0,
        1.0,
        256.0,
        &format!("{}.pickup.radius", common.key),
    )?;
    Ok(PreparedModPowerupDefinition { common, effect, pickup_radius })
}

pub(super) fn affix(common: PreparedModContentEntry) -> Result<PreparedModAffixDefinition, String> {
    let modifiers =
        optional_object(common.fields.get("modifiers"), &format!("{}.modifiers", common.key))?;
    exact_keys(
        &modifiers,
        &["incoming_damage", "mana_spend"],
        &format!("{}.modifiers", common.key),
    )?;
    validate_modifier_map(&modifiers, &format!("{}.modifiers", common.key))?;
    let applies_to = equipment_types(&common, "affix")?;
    Ok(PreparedModAffixDefinition { common, applies_to, modifiers })
}

pub(super) fn affix_pool(
    common: PreparedModContentEntry,
    affix_ids: &BTreeSet<String>,
) -> Result<PreparedModAffixPoolDefinition, String> {
    let values = common
        .fields
        .get("entries")
        .and_then(LuaValue::as_array)
        .filter(|values| !values.is_empty() && values.len() <= 256)
        .ok_or_else(|| {
            format!("{}:{} affix pool requires 1..256 entries", common.mod_id, common.key)
        })?;
    let mut entries = Vec::new();
    for (index, value) in values.iter().enumerate() {
        let field = format!("{}.entries[{index}]", common.key);
        let entry = object(Some(value), &field)?;
        exact_keys(entry, &["affix", "weight"], &field)?;
        let affix = optional_reference(
            entry.get("affix"),
            WebLuaContentKind::Affix,
            &format!("{field}.affix"),
        )?
        .filter(|reference| affix_ids.contains(&reference.content_id))
        .ok_or_else(|| {
            format!("{}:{} affix pool entry is unavailable", common.mod_id, common.key)
        })?;
        let weight = number_or(
            entry.get("weight"),
            1.0,
            f64::EPSILON,
            1_000_000.0,
            &format!("{field}.weight"),
        )?;
        entries.push(PreparedModAffixPoolEntry { affix, weight });
    }
    let applies_to = equipment_types(&common, "affix pool")?;
    let rng_domain = optional_text(
        common.fields.get("rng_domain"),
        &common.key,
        128,
        &format!("{}.rng_domain", common.key),
    )?;
    let rolls =
        integer_or(common.fields.get("rolls"), 1, 1, 8, &format!("{}.rolls", common.key))? as u32;
    Ok(PreparedModAffixPoolDefinition { common, applies_to, entries, rng_domain, rolls })
}

fn equipment_types(common: &PreparedModContentEntry, family: &str) -> Result<Vec<String>, String> {
    let values = text_array(
        nullish(common.fields.get("applies_to")).or(Some(&empty_array())),
        &format!("{}.applies_to", common.key),
    )?;
    for value in &values {
        if !matches!(value.as_str(), "amulet" | "hat" | "ring" | "robe" | "staff" | "wand") {
            return Err(format!(
                "{}:{} {family} applies_to contains an unknown equipment type",
                common.mod_id, common.key
            ));
        }
    }
    Ok(values)
}

pub(super) fn skill(
    common: PreparedModContentEntry,
    skill_ids: &BTreeSet<String>,
    declared_ids: &BTreeSet<String>,
) -> Result<PreparedModSkillDefinition, String> {
    if !has_image(&common, "icon") {
        return Err(format!("{}:{} skill requires image art.icon", common.mod_id, common.key));
    }
    let values = common
        .fields
        .get("ranks")
        .and_then(LuaValue::as_array)
        .filter(|values| !values.is_empty() && values.len() <= 99)
        .ok_or_else(|| {
            format!("{}:{} skill requires 1..99 rank definitions", common.mod_id, common.key)
        })?;
    let mut ranks = Vec::new();
    for (index, value) in values.iter().enumerate() {
        ranks.push(object(Some(value), &format!("{}.ranks[{index}]", common.key))?.clone());
    }
    for (index, rank) in ranks.iter().enumerate() {
        exact_keys(
            rank,
            &["grant", "grants", "modify"],
            &format!("{}.ranks[{index}]", common.key),
        )?;
    }
    for (index, rank) in ranks.iter().enumerate() {
        if let Some(value) = rank.get("modify") {
            let field = format!("{}.ranks[{index}].modify", common.key);
            validate_modifier_map(object(Some(value), &field)?, &field)?;
        }
    }
    let maximum_rank = integer_or(
        nullish(common.fields.get("maximum_rank")),
        ranks.len() as u64,
        1,
        ranks.len() as u64,
        &format!("{}.maximum_rank", common.key),
    )? as u32;
    let offer = object_or_empty(common.fields.get("offer"), &format!("{}.offer", common.key))?;
    exact_keys(&offer, &["minimum_level", "weight"], &format!("{}.offer", common.key))?;
    let parent = optional_reference(
        common.fields.get("parent"),
        WebLuaContentKind::Skill,
        &format!("{}.parent", common.key),
    )?;
    let prerequisites = match common.fields.get("prerequisites") {
        None => Vec::new(),
        Some(value) => reference_array(
            Some(value),
            WebLuaContentKind::Skill,
            &format!("{}.prerequisites", common.key),
        )?,
    };
    if parent.as_ref().is_some_and(|reference| !skill_ids.contains(&reference.content_id))
        || prerequisites.iter().any(|reference| !skill_ids.contains(&reference.content_id))
    {
        return Err(format!("{}:{} skill dependency is unavailable", common.mod_id, common.key));
    }
    let grants =
        skill_grants(common.fields.get("grants"), declared_ids, &format!("{}.grants", common.key))?;
    let mut rank_grants = Vec::new();
    ranks.truncate(maximum_rank as usize);
    for (index, rank) in ranks.iter().enumerate() {
        rank_grants.push(skill_grants(
            nullish(rank.get("grants")).or_else(|| rank.get("grant")),
            declared_ids,
            &format!("{}.ranks[{index}].grant", common.key),
        )?);
    }
    let minimum_level = integer_or(
        offer.get("minimum_level"),
        1,
        1,
        999,
        &format!("{}.offer.minimum_level", common.key),
    )? as u32;
    let offer_weight = number_or(
        offer.get("weight"),
        1.0,
        f64::EPSILON,
        1_000_000.0,
        &format!("{}.offer.weight", common.key),
    )?;
    Ok(PreparedModSkillDefinition {
        common,
        grants,
        maximum_rank,
        minimum_level,
        offer_weight,
        parent,
        prerequisites,
        rank_grants,
        ranks,
    })
}

fn skill_grants(
    value: Option<&LuaValue>,
    declared_ids: &BTreeSet<String>,
    field: &str,
) -> Result<Vec<ResolvedWebLuaContentReference>, String> {
    let Some(value) = nullish(value) else { return Ok(Vec::new()) };
    let values = value.as_array().unwrap_or_else(|| std::slice::from_ref(value));
    if values.len() > 32
        || values.iter().any(|value| !is_token(value, "resolved-content-reference"))
    {
        return Err(format!("{field} must contain resolved content references"));
    }
    let mut grants = Vec::new();
    for value in values {
        let grant: ResolvedWebLuaContentReference = decode(value)?;
        if !declared_ids.contains(&grant.content_id)
            || !matches!(grant.target_kind, WebLuaContentKind::Spell | WebLuaContentKind::Ui)
        {
            return Err(format!("{field} may grant only declared spells or UI"));
        }
        grants.push(grant);
    }
    Ok(grants)
}

pub(super) fn spell(common: PreparedModContentEntry) -> Result<PreparedModSpellDefinition, String> {
    if !has_image(&common, "icon") {
        return Err(format!("{}:{} spell requires image art.icon", common.mod_id, common.key));
    }
    if common.art.contains_key("effect") && !has_image(&common, "effect") {
        return Err(format!("{}:{} spell art.effect must be an image", common.mod_id, common.key));
    }
    if common.art.get("sound").is_some_and(|binding| binding.asset_kind != WebLuaAssetKind::Sound) {
        return Err(format!("{}:{} spell art.sound must be a sound", common.mod_id, common.key));
    }
    let behavior = rule(
        common.fields.get("behavior"),
        format!("{}:{} spell requires a behavior prefab", common.mod_id, common.key),
    )?;
    let slot = match common.fields.get("slot").and_then(LuaValue::as_str) {
        Some("primary") => ModSpellSlot::Primary,
        Some("secondary") => ModSpellSlot::Secondary,
        _ => {
            return Err(format!(
                "{}:{} spell slot must be primary or secondary",
                common.mod_id, common.key
            ));
        }
    };
    let cooldown_ms = duration_ms(
        nullish(common.fields.get("cooldown")).or(Some(&LuaValue::Number(0.0))),
        &format!("{}.cooldown", common.key),
    )?;
    let mana = number_or(
        common.fields.get("mana"),
        0.0,
        0.0,
        1_000_000.0,
        &format!("{}.mana", common.key),
    )?;
    Ok(PreparedModSpellDefinition { common, behavior, cooldown_ms, mana, slot })
}

pub(super) fn enemy(common: PreparedModContentEntry) -> Result<PreparedModEnemyDefinition, String> {
    if !has_image(&common, "atlas") {
        return Err(format!("{}:{} enemy requires image art.atlas", common.mod_id, common.key));
    }
    for slot in ["attack_sound", "death_sound", "sound"] {
        if common.art.get(slot).is_some_and(|binding| binding.asset_kind != WebLuaAssetKind::Sound)
        {
            return Err(format!(
                "{}:{} enemy art.{slot} must be a sound",
                common.mod_id, common.key
            ));
        }
    }
    let field = format!("{}.stats", common.key);
    let stats = object_or_empty(common.fields.get("stats"), &field)?;
    exact_keys(
        &stats,
        &[
            "attack_cooldown",
            "attack_range",
            "collision_radius",
            "damage",
            "health",
            "scale",
            "speed",
        ],
        &field,
    )?;
    let loot = object_or_empty(common.fields.get("loot"), &format!("{}.loot", common.key))?;
    exact_keys(&loot, &["experience", "gold"], &format!("{}.loot", common.key))?;
    let gold = object_or_empty(loot.get("gold"), &format!("{}.loot.gold", common.key))?;
    exact_keys(&gold, &["maximum", "minimum"], &format!("{}.loot.gold", common.key))?;
    let gold_minimum = integer_or(
        gold.get("minimum"),
        0,
        0,
        1_000_000,
        &format!("{}.loot.gold.minimum", common.key),
    )? as u32;
    let gold_maximum = integer_or(
        gold.get("maximum"),
        u64::from(gold_minimum),
        u64::from(gold_minimum),
        1_000_000,
        &format!("{}.loot.gold.maximum", common.key),
    )? as u32;
    let experience = number_or(
        loot.get("experience"),
        0.0,
        0.0,
        1_000_000.0,
        &format!("{}.loot.experience", common.key),
    )?;
    let attack_cooldown_ms = duration_ms(
        nullish(stats.get("attack_cooldown")).or(Some(&LuaValue::String("1s".into()))),
        &format!("{field}.attack_cooldown"),
    )?;
    let attack_damage =
        number_or(stats.get("damage"), 10.0, 0.0, 1_000_000.0, &format!("{field}.damage"))?;
    let attack_range =
        number_or(stats.get("attack_range"), 36.0, 1.0, 2_048.0, &format!("{field}.attack_range"))?;
    let collision_radius = number_or(
        stats.get("collision_radius"),
        24.0,
        4.0,
        128.0,
        &format!("{field}.collision_radius"),
    )?;
    let health =
        number_or(stats.get("health"), 100.0, 1.0, 1_000_000_000.0, &format!("{field}.health"))?;
    let scale = number_or(stats.get("scale"), 1.0, 0.1, 10.0, &format!("{field}.scale"))?;
    let speed = number_or(stats.get("speed"), 2.0, 0.0, 100.0, &format!("{field}.speed"))?;
    Ok(PreparedModEnemyDefinition {
        common,
        attack_cooldown_ms,
        attack_damage,
        attack_range,
        health,
        collision_radius,
        gold_maximum,
        gold_minimum,
        experience,
        scale,
        speed,
    })
}

pub(super) fn has_image(common: &PreparedModContentEntry, slot: &str) -> bool {
    common.art.get(slot).is_some_and(|binding| {
        matches!(binding.asset_kind, WebLuaAssetKind::Sprite | WebLuaAssetKind::Sheet)
    })
}
