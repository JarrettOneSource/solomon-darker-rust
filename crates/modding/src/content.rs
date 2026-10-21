//! Immutable catalog preparation from `modding/content/mod-content-catalog.ts`.
//! The phase order is observable when a package has multiple invalid entries.

mod definitions;
mod fields;
mod items;
mod scenes;

use std::collections::{BTreeMap, BTreeSet};

use solomon_sim::modding::LuaValue;
use solomon_sim::modding::content::{
    PreparedModArtBinding, PreparedModContentCatalog, PreparedModContentDefinition,
    PreparedModContentEntry,
};
use solomon_sim::modding::definition::{
    CompiledWebLuaContent, CompiledWebLuaMod, WebLuaAssetKind, WebLuaContentKind,
};

use crate::assets::PreparedModAssetCatalog;
use crate::json::SourceCollator;

use self::fields::{exact_keys, is_token, object, optional_text};

/// Validates every authored family against the same immutable asset catalog.
pub fn compile_mod_content_catalog(
    mods: &[CompiledWebLuaMod],
    assets: &PreparedModAssetCatalog,
) -> Result<PreparedModContentCatalog, String> {
    let count: usize = mods.iter().map(|package| package.content.len()).sum();
    if count > 4_096 {
        return Err("prepared mod catalog exceeds 4096 content entries".into());
    }
    let mut content = Vec::new();
    let mut candidates: BTreeMap<WebLuaContentKind, Vec<PreparedModContentEntry>> = BTreeMap::new();
    for package in mods {
        for definition in &package.content {
            let common = compile_common(package, definition, assets)?;
            if definition.content_kind == WebLuaContentKind::Status {
                content.push(PreparedModContentDefinition::Status(definitions::status(common)?));
            } else {
                candidates.entry(definition.content_kind).or_default().push(common);
            }
        }
    }
    let status_ids = ids(&content, &[WebLuaContentKind::Status]);
    let mut loot_count = 0;
    for (index, common) in
        take_sorted(&mut candidates, WebLuaContentKind::Potion).into_iter().enumerate()
    {
        let potion = items::potion(common, 6 + index as u32, &status_ids, assets)?;
        if potion.loot.is_some() {
            loot_count += 1;
        }
        content.push(PreparedModContentDefinition::Potion(potion));
    }
    for common in take_sorted(&mut candidates, WebLuaContentKind::Item) {
        content.push(PreparedModContentDefinition::Item(items::item(common, assets)?));
    }
    for common in take_sorted(&mut candidates, WebLuaContentKind::Powerup) {
        content.push(PreparedModContentDefinition::Powerup(definitions::powerup(common)?));
    }
    for common in take_sorted(&mut candidates, WebLuaContentKind::Affix) {
        content.push(PreparedModContentDefinition::Affix(definitions::affix(common)?));
    }
    let affix_ids = ids(&content, &[WebLuaContentKind::Affix]);
    for common in take_sorted(&mut candidates, WebLuaContentKind::AffixPool) {
        content.push(PreparedModContentDefinition::AffixPool(definitions::affix_pool(
            common, &affix_ids,
        )?));
    }
    let skill_ids = candidates
        .get(&WebLuaContentKind::Skill)
        .into_iter()
        .flatten()
        .map(|common| common.content_id.clone())
        .collect();
    let declared_ids = mods
        .iter()
        .flat_map(|package| &package.content)
        .map(|definition| definition.content_id.clone())
        .collect();
    for common in take_sorted(&mut candidates, WebLuaContentKind::Skill) {
        content.push(PreparedModContentDefinition::Skill(definitions::skill(
            common,
            &skill_ids,
            &declared_ids,
        )?));
    }
    for common in take_sorted(&mut candidates, WebLuaContentKind::Spell) {
        content.push(PreparedModContentDefinition::Spell(definitions::spell(common)?));
    }
    for common in take_sorted(&mut candidates, WebLuaContentKind::Enemy) {
        content.push(PreparedModContentDefinition::Enemy(definitions::enemy(common)?));
    }
    for common in take_sorted(&mut candidates, WebLuaContentKind::Boneyard) {
        content.push(PreparedModContentDefinition::Boneyard(scenes::boneyard(common)?));
    }
    for common in take_sorted(&mut candidates, WebLuaContentKind::Boast) {
        content.push(PreparedModContentDefinition::Boast(scenes::boast(common, assets)?));
    }
    for common in take_sorted(&mut candidates, WebLuaContentKind::Room) {
        content.push(PreparedModContentDefinition::Room(scenes::room(common)?));
    }
    let room_ids = ids(&content, &[WebLuaContentKind::Room]);
    for common in take_sorted(&mut candidates, WebLuaContentKind::Scene) {
        content.push(PreparedModContentDefinition::Scene(scenes::scene(common, &room_ids)?));
    }
    for common in take_sorted(&mut candidates, WebLuaContentKind::SceneExtension) {
        content
            .push(PreparedModContentDefinition::SceneExtension(scenes::scene_extension(common)?));
    }
    let item_ids = ids(&content, &[WebLuaContentKind::Item, WebLuaContentKind::Potion]);
    let mut wearable_ids = BTreeSet::new();
    for definition in &content {
        if let PreparedModContentDefinition::Item(item) = definition
            && item.catalog.content.wearable.is_some()
        {
            wearable_ids.insert(item.common.content_id.clone());
        }
    }
    let pool_ids = ids(&content, &[WebLuaContentKind::AffixPool]);
    for common in take_sorted(&mut candidates, WebLuaContentKind::Shop) {
        content.push(PreparedModContentDefinition::Shop(scenes::shop(
            common,
            &item_ids,
            &wearable_ids,
            &pool_ids,
        )?));
    }
    for common in take_sorted(&mut candidates, WebLuaContentKind::Ui) {
        content.push(PreparedModContentDefinition::Ui(scenes::ui(common)?));
    }
    if loot_count > 1_024 {
        return Err("prepared mod loot exceeds 1024 rows".into());
    }
    let collator = SourceCollator::default();
    content.sort_by(|left, right| compare_content(left.common(), right.common(), &collator));
    PreparedModContentCatalog::from_prepared(content)
}

fn take_sorted(
    candidates: &mut BTreeMap<WebLuaContentKind, Vec<PreparedModContentEntry>>,
    kind: WebLuaContentKind,
) -> Vec<PreparedModContentEntry> {
    let mut entries = candidates.remove(&kind).unwrap_or_default();
    let collator = SourceCollator::default();
    entries.sort_by(|left, right| compare_content(left, right, &collator));
    entries
}

fn compare_content(
    left: &PreparedModContentEntry,
    right: &PreparedModContentEntry,
    collator: &SourceCollator,
) -> std::cmp::Ordering {
    collator.compare(&left.mod_id, &right.mod_id).then_with(|| {
        // Definition compilation generated these decimal u64 identities. Source
        // sorts their numeric BigInt values, rather than decimal string order.
        let left = left.content_id.parse::<u64>().expect("compiled content has a numeric identity");
        let right =
            right.content_id.parse::<u64>().expect("compiled content has a numeric identity");
        left.cmp(&right)
    })
}

fn ids(content: &[PreparedModContentDefinition], kinds: &[WebLuaContentKind]) -> BTreeSet<String> {
    content
        .iter()
        .filter(|definition| kinds.contains(&definition.kind()))
        .map(|definition| definition.common().content_id.clone())
        .collect()
}

fn compile_common(
    package: &CompiledWebLuaMod,
    definition: &CompiledWebLuaContent,
    assets: &PreparedModAssetCatalog,
) -> Result<PreparedModContentEntry, String> {
    let field = format!("{}:{}.art", package.identity.id, definition.key);
    if let Some(value) = definition.fields.get("art") {
        exact_keys(object(Some(value), &field)?, definition.content_kind.art_slots(), &field)?;
    }
    let art = compile_art(&package.identity.id, definition.fields.get("art"), assets, &field)?;
    for slot in ["ambience", "loop", "music"] {
        if let Some(binding) = art.get(slot)
            && !matches!(binding.asset_kind, WebLuaAssetKind::Music | WebLuaAssetKind::Sound)
        {
            return Err(format!(
                "{}:{} art.{slot} must be audio",
                package.identity.id, definition.key
            ));
        }
    }
    let description = optional_text(
        definition.fields.get("description"),
        "",
        1_024,
        &format!("{}.description", definition.key),
    )?;
    let name = optional_text(
        definition.fields.get("name"),
        &humanize(&definition.key),
        128,
        &format!("{}.name", definition.key),
    )?;
    Ok(PreparedModContentEntry {
        art,
        content_id: definition.content_id.clone(),
        description,
        fields: definition.fields.clone(),
        key: definition.key.clone(),
        mod_id: package.identity.id.clone(),
        name,
    })
}

fn compile_art(
    mod_id: &str,
    value: Option<&LuaValue>,
    assets: &PreparedModAssetCatalog,
    field: &str,
) -> Result<BTreeMap<String, PreparedModArtBinding>, String> {
    let mut art = BTreeMap::new();
    let Some(value) = fields::nullish(value) else { return Ok(art) };
    for (slot, candidate) in object(Some(value), field)?.iter() {
        if !is_token(candidate, "asset-reference")
            || candidate.get("key").and_then(LuaValue::as_str).is_none()
        {
            return Err(format!("{field}.{slot} must reference a declared asset"));
        }
        let key = candidate.get("key").and_then(LuaValue::as_str).expect("asset key was validated");
        let asset = assets
            .get(mod_id, key)
            .ok_or_else(|| format!("{field}.{slot} references an unavailable asset: {key}"))?;
        let identity = asset.identity();
        art.insert(
            slot.to_owned(),
            PreparedModArtBinding {
                asset_id: identity.id.clone(),
                asset_kind: asset.asset_kind(),
                key: identity.key.clone(),
                path: identity.path.clone(),
            },
        );
    }
    Ok(art)
}

fn humanize(key: &str) -> String {
    let mut words = Vec::new();
    for word in key.split(['.', '_', '-']).filter(|word| !word.is_empty()) {
        let mut characters = word.chars();
        let initial = characters.next().expect("empty words were excluded");
        words.push(format!("{}{}", initial.to_uppercase(), characters.as_str()));
    }
    words.join(" ")
}

fn empty_array() -> LuaValue {
    LuaValue::Array(Vec::new())
}
