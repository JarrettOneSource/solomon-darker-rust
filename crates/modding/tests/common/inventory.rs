//! Fixture-only spelling bridge. Website JSON and native checkpoints deliberately
//! use different inventory field/enum names; domain values remain unchanged.

use serde_json::{Value, json};
use solomon_sim::economy::{
    EquipmentType, HubEquipmentState, HubInventoryItem, HubItemKind, ItemRarity,
};

pub fn item_value(item: &HubInventoryItem) -> Value {
    let mut value = json!({"equipmentType":item.equipment_type.map(EquipmentType::as_str),
        "iconRecords":item.icon_records,"id":item.id,"kind":item.kind.as_str(),"name":item.name,
        "nativeSubtype":item.native_subtype,"nativeTypeId":item.native_type_id,"quantity":item.quantity,
        "rarity":item.rarity.map(|rarity| match rarity {ItemRarity::Epic=>"Epic",ItemRarity::Rare=>"Rare"}),
        "recipeIndex":item.recipe_index});
    for (key, field) in [
        (
            "contents",
            item.contents
                .as_ref()
                .map(|items| json!(items.iter().map(item_value).collect::<Vec<_>>())),
        ),
        ("generatedLevel", item.generated_level.map(|value| json!(value))),
        ("iconTints", item.icon_tints.map(|value| json!(value))),
        ("inventorySlot", item.inventory_slot.map(|value| json!(value))),
        ("nativeSelector", item.native_selector.map(|value| json!(value))),
        ("nativeEffects", item.native_effects.as_ref().map(|value| json!(value))),
        ("modContent", item.mod_content.as_ref().map(|value| json!(value))),
        ("modItemContent", item.mod_item_content.as_ref().map(|value| json!(value))),
        ("modAffixes", item.mod_affixes.as_ref().map(|value| json!(value))),
    ] {
        if let Some(field) = field {
            value[key] = field;
        }
    }
    value
}

// Some fixture targets only project items; filter fixtures also decode equipment.
#[allow(dead_code)]
pub fn equipment_from_source(value: Value) -> HubEquipmentState {
    fn item(value: &Value) -> Option<HubInventoryItem> {
        let mut fields = value.as_object()?.clone();
        assert_eq!(fields.get("kind"), Some(&json!("equipment")));
        fields.insert("kind".into(), json!(HubItemKind::Equipment));
        let equipment_type = match fields.remove("equipmentType").unwrap().as_str().unwrap() {
            "amulet" => EquipmentType::Amulet,
            "hat" => EquipmentType::Hat,
            "ring" => EquipmentType::Ring,
            "robe" => EquipmentType::Robe,
            "staff" => EquipmentType::Staff,
            "wand" => EquipmentType::Wand,
            value => panic!("unsupported fixture equipment type: {value}"),
        };
        fields.insert("equipment_type".into(), json!(equipment_type));
        for (source, native) in [
            ("generatedLevel", "generated_level"),
            ("iconRecords", "icon_records"),
            ("iconTints", "icon_tints"),
            ("inventorySlot", "inventory_slot"),
            ("modContent", "mod_content"),
            ("modItemContent", "mod_item_content"),
            ("modAffixes", "mod_affixes"),
            ("nativeSubtype", "native_subtype"),
            ("nativeSelector", "native_selector"),
            ("nativeEffects", "native_effects"),
            ("nativeTypeId", "native_type_id"),
            ("recipeIndex", "recipe_index"),
        ] {
            if let Some(value) = fields.remove(source) {
                fields.insert(native.into(), value);
            }
        }
        Some(serde_json::from_value(Value::Object(fields)).unwrap())
    }
    HubEquipmentState {
        amulet: item(&value["amulet"]),
        hat: item(&value["hat"]),
        rings: [item(&value["rings"][0]), item(&value["rings"][1]), item(&value["rings"][2])],
        robe: item(&value["robe"]),
        weapon: item(&value["weapon"]),
    }
}
