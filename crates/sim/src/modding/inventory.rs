//! Authored inventory metadata for mod items and consumables.
//! These fields travel with the canonical inventory item; views never own a copy
//! of item admission, stack limits or the package's content identity.

use super::assets::ModSpriteFrame;
use crate::economy::{EquipmentType, HubInventoryItem, HubItemKind, base_item};

pub const MOD_ITEM_TYPE_ID: u32 = 7013;

/// A mod item's icon: an atlas frame and the image it comes from.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct ModItemIcon {
    pub atlas_id: String,
    pub frame: ModSpriteFrame,
    pub frame_index: usize,
    pub image_path: String,
}

/// Where a mod wearable is worn: as a hat, robe or staff.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "kebab-case"))]
pub enum ModWearableSlot {
    Hat,
    Robe,
    Staff,
}

/// How a mod wearable looks when worn: its slot, worn and trim images, death shape and whether
/// it can be dyed.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct ModWearableContent {
    pub death_shape: usize,
    pub dyeable: bool,
    pub slot: ModWearableSlot,
    pub worn_image_path: String,
    #[cfg_attr(feature = "persistence", serde(default, skip_serializing_if = "Option::is_none"))]
    pub worn_trim_image_path: Option<String>,
}

/// The package metadata a mod item carries in the inventory: identity, description, icon,
/// stack limit and any wearable look.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct ModItemContent {
    pub content_id: String,
    pub description: String,
    pub icon: ModItemIcon,
    #[cfg_attr(feature = "persistence", serde(default, skip_serializing_if = "Option::is_none"))]
    pub icon_trim_image_path: Option<String>,
    pub key: String,
    pub mod_id: String,
    pub stack_maximum: u32,
    #[cfg_attr(feature = "persistence", serde(default, skip_serializing_if = "Option::is_none"))]
    pub wearable: Option<ModWearableContent>,
}

/// The package metadata a mod consumable carries in the inventory: identity, description,
/// icon, effect duration and use effect.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct ModConsumableContent {
    pub content_id: String,
    pub description: String,
    pub icon: ModItemIcon,
    #[cfg_attr(feature = "persistence", serde(default, skip_serializing_if = "Option::is_none"))]
    pub icon_trim_image_path: Option<String>,
    pub key: String,
    pub mod_id: String,
    #[cfg_attr(feature = "persistence", serde(default, skip_serializing_if = "Option::is_none"))]
    pub wearable: Option<ModWearableContent>,
    pub consume_vfx: Option<ModConsumableVfx>,
    pub duration_ms: f64,
}

/// A consumable's use effect: a spell glow in one RGBA color.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(tag = "kind", rename = "spell_glow"))]
pub struct ModConsumableVfx {
    pub color: [f64; 4],
}

/// A mod consumable as the catalog lists it: its content, name and potion subtype.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct ModConsumableCatalogEntry {
    pub content: ModConsumableContent,
    pub name: String,
    #[cfg_attr(feature = "persistence", serde(rename = "nativeSubtype"))]
    pub subtype: u32,
}

/// A mod item as the catalog lists it: its content, name and icon tints.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct ModItemCatalogEntry {
    pub content: ModItemContent,
    #[cfg_attr(feature = "persistence", serde(default, skip_serializing_if = "Option::is_none"))]
    pub icon_tints: Option<[u32; 2]>,
    pub name: String,
}

/// How a mod equipment modifier combines with a stat: add, multiply or set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "kebab-case"))]
pub enum ModModifierOperation {
    Add,
    Multiply,
    Set,
}

/// One mod equipment modifier: the stat key, the operation and its value.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct ModEquipmentModifier {
    pub key: String,
    pub operation: ModModifierOperation,
    pub value: f64,
}

/// A mod affix on a piece of equipment: its identity, name and modifiers.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct ModEquipmentAffix {
    pub content_id: String,
    pub mod_id: String,
    pub modifiers: Vec<ModEquipmentModifier>,
    pub name: String,
}

/// An active mod consumable effect on a player: its content, use ID, color and the ticks it
/// starts and expires.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct GameSimulationModEffect {
    pub color: [f64; 4],
    pub content_id: String,
    pub expires_tick: u64,
    pub player_id: String,
    pub started_tick: u64,
    pub use_id: u64,
}

/// Accepted inventory consumption returned to the prepared host after the
/// canonical Session transaction.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct GameSimulationModConsumption {
    pub content: ModConsumableContent,
    pub player_id: String,
    pub tick: u64,
    pub use_id: u64,
}

/// A mod consumable as an inventory item; address assignment belongs to the inventory
/// transaction. Custom potion subtypes begin at six in the prepared catalog.
#[must_use]
pub fn mod_consumable_inventory_item(catalog: &ModConsumableCatalogEntry) -> HubInventoryItem {
    let mut item =
        base_item(0, HubItemKind::ModPotion, &catalog.name, 7001, Some(catalog.subtype), &[]);
    item.mod_content = Some(catalog.content.clone());
    item
}

/// A mod item as an inventory item. Wearables and stackable items use the same inventory
/// item model, with actual package metadata attached to the inventory tree.
pub fn mod_item_inventory_item(
    catalog: &ModItemCatalogEntry,
    quantity: u32,
) -> Result<HubInventoryItem, String> {
    let wearable = catalog.content.wearable.as_ref();
    if wearable.is_some() && quantity != 1 {
        return Err("mod wearable quantity must be one".into());
    }
    if quantity < 1 || quantity > catalog.content.stack_maximum {
        return Err(format!(
            "mod item quantity must be within 1..{}",
            catalog.content.stack_maximum
        ));
    }
    let kind = if wearable.is_some() { HubItemKind::Equipment } else { HubItemKind::ModItem };
    let mut item = base_item(0, kind, &catalog.name, MOD_ITEM_TYPE_ID, None, &[]);
    item.equipment_type = wearable.map(|wearable| match wearable.slot {
        ModWearableSlot::Hat => EquipmentType::Hat,
        ModWearableSlot::Robe => EquipmentType::Robe,
        ModWearableSlot::Staff => EquipmentType::Staff,
    });
    item.icon_tints = catalog.icon_tints.map(|tints| tints.map(Some));
    item.mod_item_content = Some(catalog.content.clone());
    item.quantity = quantity;
    Ok(item)
}

/// These are the economy's identity invariants for mod items, independent of whether
/// the owning package is currently installed (reconciliation is a separate step).
#[must_use]
pub fn mod_item_inventory_identity_is_valid(item: &HubInventoryItem) -> bool {
    let Some(content) = &item.mod_item_content else { return item.kind != HubItemKind::ModItem };
    let Some(wearable) = &content.wearable else {
        return item.kind == HubItemKind::ModItem
            && item.equipment_type.is_none()
            && item.type_id == MOD_ITEM_TYPE_ID
            && item.subtype.is_none()
            && item.icon_records.is_empty()
            && item.quantity <= content.stack_maximum;
    };
    let equipment_type = match wearable.slot {
        ModWearableSlot::Hat => EquipmentType::Hat,
        ModWearableSlot::Robe => EquipmentType::Robe,
        ModWearableSlot::Staff => EquipmentType::Staff,
    };
    item.kind == HubItemKind::Equipment
        && item.equipment_type == Some(equipment_type)
        && item.type_id == MOD_ITEM_TYPE_ID
        && item.subtype.is_none()
        && item.icon_records.is_empty()
        && item.quantity == 1
        && item.rarity.is_none()
        && item.recipe_index.is_none()
        && item.generated_level.is_none()
        && item.selector.is_none()
        && item.native_effects.is_none()
        && content.stack_maximum == 1
        && mod_wearable_content_is_valid(wearable, content.icon_trim_image_path.as_deref())
        && if wearable.slot == ModWearableSlot::Staff {
            item.icon_tints.is_none()
        } else {
            item.icon_tints.is_some_and(|tints| tints.iter().all(Option::is_some))
        }
}

#[must_use]
pub fn mod_wearable_content_is_valid(
    wearable: &ModWearableContent,
    icon_trim_image_path: Option<&str>,
) -> bool {
    let maximum_shape = match wearable.slot {
        ModWearableSlot::Hat => 3,
        ModWearableSlot::Robe => 2,
        ModWearableSlot::Staff => 5,
    };
    let has_trim = wearable.worn_trim_image_path.is_some();
    wearable.death_shape <= maximum_shape
        && wearable_image_path(&wearable.worn_image_path)
        && wearable.worn_trim_image_path.as_deref().is_none_or(wearable_image_path)
        && icon_trim_image_path.is_some() == has_trim
        && (!wearable.dyeable || has_trim)
        && (wearable.slot != ModWearableSlot::Staff || !has_trim)
}

fn wearable_image_path(path: &str) -> bool {
    let remainder = path.strip_prefix("art/").or_else(|| path.strip_prefix("sprites/"));
    remainder.is_some_and(|remainder| {
        remainder.len() > ".png".len()
            && remainder.ends_with(".png")
            && !remainder.contains(['\n', '\r', '\u{2028}', '\u{2029}'])
    })
}
