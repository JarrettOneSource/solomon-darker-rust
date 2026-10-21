//! Item, wearable and potion preparation from `mod-content-catalog.ts`.

use std::collections::BTreeSet;

use solomon_sim::modding::assets::PreparedModSpriteAsset;
use solomon_sim::modding::content::{
    ModPotionLoot, PreparedModContentEntry, PreparedModItemDefinition, PreparedModPotionDefinition,
};
use solomon_sim::modding::definition::{WebLuaAssetKind, WebLuaContentKind};
use solomon_sim::modding::inventory::{
    ModConsumableCatalogEntry, ModConsumableContent, ModConsumableVfx, ModItemCatalogEntry,
    ModItemContent, ModItemIcon, ModWearableContent, ModWearableSlot,
};
use solomon_sim::modding::{LuaObject, LuaValue};

use crate::assets::PreparedModAssetCatalog;

use super::fields::*;

pub(super) fn item(
    common: PreparedModContentEntry,
    assets: &PreparedModAssetCatalog,
) -> Result<PreparedModItemDefinition, String> {
    let icon_binding = common
        .art
        .get("icon")
        .ok_or_else(|| format!("{}:{} item requires art.icon", common.mod_id, common.key))?;
    let sprite = assets.image(&common.mod_id, &icon_binding.key)?;
    let stack = object_or_empty(common.fields.get("stack"), &format!("{}.stack", common.key))?;
    exact_keys(&stack, &["maximum"], &format!("{}.stack", common.key))?;
    let stack_maximum =
        integer_or(stack.get("maximum"), 1, 1, 9_999, &format!("{}.stack.maximum", common.key))?
            as u32;
    let wearable = match common.fields.get("equipment") {
        None => None,
        Some(value) => Some(compile_wearable(
            &common,
            object(Some(value), &format!("{}.equipment", common.key))?,
            assets,
        )?),
    };
    if wearable.is_some() && stack_maximum != 1 {
        return Err(format!("{}:{} wearable equipment cannot stack", common.mod_id, common.key));
    }
    let icon_trim = common
        .art
        .get("icon_trim")
        .map(|binding| assets.image(&common.mod_id, &binding.key))
        .transpose()?;
    let worn_trim_present = wearable.as_ref().is_some_and(|wearable| wearable.worn_trim.is_some());
    if icon_trim.is_some() != worn_trim_present {
        return Err(format!(
            "{}:{} wearable icon and worn trim layers must be paired",
            common.mod_id, common.key
        ));
    }
    if let Some(trim) = icon_trim
        && !same_icon_frame(sprite, trim)
    {
        return Err(format!(
            "{}:{} wearable icon layers must have identical frames",
            common.mod_id, common.key
        ));
    }
    if wearable.is_none()
        && ["worn", "worn_trim", "icon_trim"].iter().any(|slot| common.art.contains_key(*slot))
    {
        return Err(format!("{}:{} wearable art requires equipment", common.mod_id, common.key));
    }
    let item_content = ModItemContent {
        content_id: common.content_id.clone(),
        description: common.description.clone(),
        icon: item_icon(sprite),
        icon_trim_image_path: icon_trim.map(|trim| trim.identity.path.clone()),
        key: common.key.clone(),
        mod_id: common.mod_id.clone(),
        stack_maximum,
        wearable: wearable.as_ref().map(|wearable| wearable.content.clone()),
    };
    let use_rule = match common.fields.get("use") {
        None => None,
        Some(value) => Some(rule(
            Some(value),
            format!("{}:{} item use must be a rule", common.mod_id, common.key),
        )?),
    };
    if wearable.is_some() && use_rule.is_some() {
        return Err(format!(
            "{}:{} wearable equipment cannot be consumed",
            common.mod_id, common.key
        ));
    }
    let catalog = ModItemCatalogEntry {
        content: item_content,
        icon_tints: wearable.and_then(|wearable| wearable.icon_tints),
        name: common.name.clone(),
    };
    Ok(PreparedModItemDefinition { common, catalog, stack_maximum, use_rule })
}

struct Wearable<'a> {
    content: ModWearableContent,
    icon_tints: Option<[u32; 2]>,
    worn_trim: Option<&'a PreparedModSpriteAsset>,
}

fn compile_wearable<'a>(
    common: &PreparedModContentEntry,
    equipment: &LuaObject,
    assets: &'a PreparedModAssetCatalog,
) -> Result<Wearable<'a>, String> {
    let field = format!("{}.equipment", common.key);
    exact_keys(equipment, &["death_shape", "dyeable", "slot", "tints"], &field)?;
    let slot = match equipment.get("slot").and_then(LuaValue::as_str) {
        Some("hat") => ModWearableSlot::Hat,
        Some("robe") => ModWearableSlot::Robe,
        Some("staff") => ModWearableSlot::Staff,
        _ => {
            return Err(format!(
                "{}:{} equipment slot must be hat, robe, or staff",
                common.mod_id, common.key
            ));
        }
    };
    let worn_binding = common.art.get("worn").ok_or_else(|| {
        format!("{}:{} wearable equipment requires art.worn", common.mod_id, common.key)
    })?;
    let worn = assets.image(&common.mod_id, &worn_binding.key)?;
    let maximum_rows = match slot {
        ModWearableSlot::Hat => 1,
        ModWearableSlot::Robe => 5,
        ModWearableSlot::Staff => 10,
    };
    validate_wearable_sheet(
        worn,
        maximum_rows,
        &format!("{}:{}.art.worn", common.mod_id, common.key),
    )?;
    let worn_trim = common
        .art
        .get("worn_trim")
        .map(|binding| assets.image(&common.mod_id, &binding.key))
        .transpose()?;
    if let Some(trim) = worn_trim {
        validate_wearable_sheet(
            trim,
            maximum_rows,
            &format!("{}:{}.art.worn_trim", common.mod_id, common.key),
        )?;
        if worn.width != trim.width || worn.height != trim.height {
            return Err(format!(
                "{}:{} wearable worn layers must have identical dimensions",
                common.mod_id, common.key
            ));
        }
    }
    let dyeable = match equipment.get("dyeable") {
        None => false,
        Some(value) => boolean(Some(value), &format!("{field}.dyeable"))?,
    };
    if slot == ModWearableSlot::Staff && (dyeable || worn_trim.is_some()) {
        return Err(format!(
            "{}:{} staff equipment cannot declare dye layers",
            common.mod_id, common.key
        ));
    }
    if dyeable && worn_trim.is_none() {
        return Err(format!(
            "{}:{} dyeable equipment requires worn and icon trim layers",
            common.mod_id, common.key
        ));
    }
    let maximum_death_shape = match slot {
        ModWearableSlot::Hat => 3,
        ModWearableSlot::Robe => 2,
        ModWearableSlot::Staff => 5,
    };
    let death_shape = integer_or(
        equipment.get("death_shape"),
        0,
        0,
        maximum_death_shape,
        &format!("{field}.death_shape"),
    )? as usize;
    let icon_tints = if slot == ModWearableSlot::Staff {
        None
    } else {
        let tints = object_or_empty(equipment.get("tints"), &format!("{field}.tints"))?;
        exact_keys(&tints, &["cloth", "trim"], &format!("{field}.tints"))?;
        Some([
            integer_or(
                tints.get("cloth"),
                0xff_ffff,
                0,
                0xff_ffff,
                &format!("{field}.tints.cloth"),
            )? as u32,
            integer_or(tints.get("trim"), 0xff_ffff, 0, 0xff_ffff, &format!("{field}.tints.trim"))?
                as u32,
        ])
    };
    if slot == ModWearableSlot::Staff && equipment.get("tints").is_some() {
        return Err(format!(
            "{}:{} staff equipment cannot declare tints",
            common.mod_id, common.key
        ));
    }
    Ok(Wearable {
        content: ModWearableContent {
            death_shape,
            dyeable,
            slot,
            worn_image_path: worn.identity.path.clone(),
            worn_trim_image_path: worn_trim.map(|trim| trim.identity.path.clone()),
        },
        icon_tints,
        worn_trim,
    })
}

fn validate_wearable_sheet(
    asset: &PreparedModSpriteAsset,
    maximum_rows: usize,
    field: &str,
) -> Result<(), String> {
    if asset.asset_kind != WebLuaAssetKind::Sheet
        || asset.width != 24 * 170
        || !asset.height.is_multiple_of(170)
    {
        return Err(format!("{field} must be a 24-column sheet of 170 px frames"));
    }
    let rows = asset.height as usize / 170;
    if rows < 1 || rows > maximum_rows || asset.frames.len() != rows * 24 {
        return Err(format!("{field} must contain 1..{maximum_rows} pose rows"));
    }
    Ok(())
}

fn same_icon_frame(left: &PreparedModSpriteAsset, right: &PreparedModSpriteAsset) -> bool {
    let left = &left.frames[0];
    let right = &right.frames[0];
    left.width == right.width
        && left.height == right.height
        && left.logical_width == right.logical_width
        && left.logical_height == right.logical_height
        && left.center_offset_x == right.center_offset_x
        && left.center_offset_y == right.center_offset_y
}

fn item_icon(asset: &PreparedModSpriteAsset) -> ModItemIcon {
    ModItemIcon {
        atlas_id: asset.identity.id.clone(),
        frame: asset.frames[0],
        frame_index: 0,
        image_path: asset.identity.path.clone(),
    }
}

pub(super) fn potion(
    common: PreparedModContentEntry,
    native_subtype: u32,
    status_ids: &BTreeSet<String>,
    assets: &PreparedModAssetCatalog,
) -> Result<PreparedModPotionDefinition, String> {
    let binding = common
        .art
        .get("icon")
        .ok_or_else(|| format!("{}:{} potion requires art.icon", common.mod_id, common.key))?;
    let sprite = assets.image(&common.mod_id, &binding.key)?;
    let duration_ms =
        duration_ms(common.fields.get("duration"), &format!("{}.duration", common.key))?;
    let on_use = rule(
        common.fields.get("on_use"),
        format!("{}:{} potion requires an on_use rule", common.mod_id, common.key),
    )?;
    let status = optional_reference(
        common.fields.get("status"),
        WebLuaContentKind::Status,
        &format!("{}.status", common.key),
    )?;
    if let Some(status) = &status
        && !status_ids.contains(&status.content_id)
    {
        return Err(format!(
            "{}:{} potion status is unavailable: {}",
            common.mod_id, common.key, status.content_id
        ));
    }
    let loot = match nullish(common.fields.get("loot")) {
        None => None,
        Some(value) => {
            let loot = object(Some(value), &format!("{}.loot", common.key))?;
            Some(ModPotionLoot {
                boss: number_or(
                    nullish(loot.get("boss")),
                    0.0,
                    0.0,
                    1.0,
                    &format!("{}.loot.boss", common.key),
                )?,
                ordinary: number_or(
                    nullish(loot.get("ordinary")),
                    0.0,
                    0.0,
                    1.0,
                    &format!("{}.loot.ordinary", common.key),
                )?,
            })
        }
    };
    let content = ModConsumableContent {
        content_id: common.content_id.clone(),
        description: common.description.clone(),
        icon: item_icon(sprite),
        icon_trim_image_path: None,
        key: common.key.clone(),
        mod_id: common.mod_id.clone(),
        wearable: None,
        consume_vfx: Some(consume_vfx(common.fields.get("presentation"))?),
        duration_ms,
    };
    let catalog = ModConsumableCatalogEntry { content, name: common.name.clone(), native_subtype };
    Ok(PreparedModPotionDefinition { common, catalog, loot, on_use, status })
}

fn consume_vfx(value: Option<&LuaValue>) -> Result<ModConsumableVfx, String> {
    if let Some(value) = nullish(value) {
        let presentation = object(Some(value), "potion.presentation")?;
        if let Some(value) = nullish(presentation.get("consume_vfx")) {
            let descriptor = object(Some(value), "potion.presentation.consume_vfx")?;
            let color = descriptor.get("color").and_then(LuaValue::as_array);
            if descriptor.get("kind").and_then(LuaValue::as_str) != Some("spell_glow")
                || color.is_none_or(|color| color.len() != 4)
            {
                return Err("potion presentation consume_vfx is invalid".into());
            }
            let mut components = [0.0; 4];
            for (index, value) in color.expect("color length was validated").iter().enumerate() {
                components[index] = number(
                    Some(value),
                    0.0,
                    1.0,
                    &format!("potion.presentation.consume_vfx.color[{index}]"),
                )?;
            }
            return Ok(ModConsumableVfx { color: components });
        }
    }
    Ok(ModConsumableVfx { color: [0.15, 1.0, 0.25, 1.0] })
}
