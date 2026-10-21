use serde_json::{Value, json};
use solomon_sim::Vec2;
use solomon_sim::economy::{EquipmentType, HubInventoryItem, HubItemKind, ItemRarity};
use solomon_sim::equipment::NativeEquipmentEffect;
use solomon_sim::modding::assets::ModSpriteFrame;
use solomon_sim::modding::inventory::{
    ModConsumableContent, ModEquipmentAffix, ModItemContent, ModItemIcon,
};
use solomon_sim::native_ui_text::NativeUiFontName;
use solomon_sim::player_panels::item_text::{
    HagathaTooltipOptions, HubTooltipLine, HubTooltipOptions, hub_contextual_hover_layout,
    hub_hagatha_tooltip_lines, hub_inventory_item_info_text, hub_item_tooltip_lines,
    hub_native_equipment_effect_text,
};

#[test]
fn inventory_item_effect_perk_rows_and_native_hover_geometry_match_current_website() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/item-text-goldens.json")).unwrap();
    for case in fixture["effects"].as_array().unwrap() {
        assert_eq!(
            hub_native_equipment_effect_text(read_effect(&case["effect"])),
            case["expected"].as_str().unwrap(),
            "effect {}",
            case["effect"]
        );
    }
    for case in fixture["hagatha"].as_array().unwrap() {
        let options = &case["options"];
        let selectors: Vec<_> = options["bundleSelectors"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_u64().unwrap() as u32)
            .collect();
        let actual = hub_hagatha_tooltip_lines(&HagathaTooltipOptions {
            bundle_selectors: &selectors,
            cheat_death_charges: options["cheatDeathCharges"].as_u64().map(|value| value as u32),
            first_mixed: options["firstMixed"].as_bool().unwrap(),
            price: options["price"].as_u64(),
            selector: options["selector"].as_i64().unwrap() as i32,
        });
        assert_eq!(lines_json(&actual), case["expected"], "Hagatha {options}");
    }
    for case in fixture["items"].as_array().unwrap() {
        let item = read_item(&case["item"]);
        let info = hub_inventory_item_info_text(&item);
        assert_eq!(
            json!({"description":info.description,"instruction":info.instruction,"title":info.title}),
            case["info"]
        );
        let options = &case["options"];
        let recipes: Vec<_> = options["ownedRecipeIndexes"]
            .as_array()
            .map(|values| values.iter().map(|value| value.as_u64().unwrap() as usize).collect())
            .unwrap_or_default();
        let actual = hub_item_tooltip_lines(
            &item,
            &HubTooltipOptions {
                creativity_rank: options["creativityRank"].as_u64().unwrap_or(0) as u32,
                owned_recipe_indices: &recipes,
                player_level: options["playerLevel"].as_u64().map(|value| value as u32),
                price: options["price"].as_u64(),
            },
        );
        assert_eq!(lines_json(&actual), case["expected"], "item {} options {options}", item.name);
    }
    for case in fixture["layouts"].as_array().unwrap() {
        let lines: Vec<_> = case["lines"].as_array().unwrap().iter().map(read_line).collect();
        let actual = hub_contextual_hover_layout(
            &lines,
            Vec2::new(case["source"]["x"].as_f64().unwrap(), case["source"]["y"].as_f64().unwrap()),
            case["gap"].as_f64().unwrap(),
            None,
        );
        let expected = &case["expected"];
        assert_eq!(
            [actual.rect.x, actual.rect.y, actual.rect.width, actual.rect.height],
            ["x", "y", "width", "height"].map(|field| expected[field].as_f64().unwrap())
        );
        assert_eq!(actual.rendered.len(), expected["rendered"].as_array().unwrap().len());
        for (line, expected) in actual.rendered.iter().zip(expected["rendered"].as_array().unwrap())
        {
            assert_eq!(json!(line.sources), expected["sources"]);
            assert_eq!(line.width, expected["width"].as_f64().unwrap());
            assert_eq!(line.height, expected["height"].as_f64().unwrap());
        }
    }
}

fn lines_json(lines: &[HubTooltipLine]) -> Value {
    json!(
        lines
            .iter()
            .map(|line| {
                let mut result =
                    json!({"font":line.font.as_str(),"text":line.text,"tint":line.tint});
                if line.sack_name_browser_font {
                    result["sackNameBrowserFont"] = json!(true);
                }
                result
            })
            .collect::<Vec<_>>()
    )
}

fn read_line(value: &Value) -> HubTooltipLine {
    HubTooltipLine {
        font: if value["font"] == "menu" { NativeUiFontName::Menu } else { NativeUiFontName::Body },
        text: value["text"].as_str().unwrap().into(),
        tint: value["tint"].as_u64().unwrap() as u32,
        sack_name_browser_font: value["sackNameBrowserFont"].as_bool().unwrap_or(false),
    }
}

fn read_effect(value: &Value) -> NativeEquipmentEffect {
    NativeEquipmentEffect {
        kind: value["kind"].as_u64().unwrap() as u32,
        magnitude: value["magnitude"].as_f64().unwrap(),
        operator: value["operator"].as_u64().unwrap() as u32,
        target: value["target"].as_u64().unwrap() as u32,
    }
}

fn read_item(value: &Value) -> HubInventoryItem {
    let kind = match value["kind"].as_str().unwrap() {
        "antidote" => HubItemKind::Antidote,
        "dye" => HubItemKind::Dye,
        "equipment" => HubItemKind::Equipment,
        "health-potion" => HubItemKind::HealthPotion,
        "key" => HubItemKind::Key,
        "mana-potion" => HubItemKind::ManaPotion,
        "mind-chug" => HubItemKind::MindChug,
        "mod-item" => HubItemKind::ModItem,
        "mod-potion" => HubItemKind::ModPotion,
        "rejuvenation-potion" => HubItemKind::RejuvenationPotion,
        "sack" => HubItemKind::Sack,
        "skill-book" => HubItemKind::SkillBook,
        "wizard-chug" => HubItemKind::WizardChug,
        _ => panic!("fixture item kind"),
    };
    let equipment_type = value["equipmentType"].as_str().map(|kind| match kind {
        "hat" => EquipmentType::Hat,
        "robe" => EquipmentType::Robe,
        "staff" => EquipmentType::Staff,
        "wand" => EquipmentType::Wand,
        "ring" => EquipmentType::Ring,
        "amulet" => EquipmentType::Amulet,
        _ => panic!("fixture equipment kind"),
    });
    let icon_tints = value["iconTints"].as_array().map(|values| {
        [values[0].as_u64().map(|value| value as u32), values[1].as_u64().map(|value| value as u32)]
    });
    let mod_item_content = value["modItemContent"].as_object().map(|_| ModItemContent {
        content_id: value["modItemContent"]["contentId"].as_str().unwrap().into(),
        description: value["modItemContent"]["description"].as_str().unwrap().into(),
        icon: unused_icon(),
        icon_trim_image_path: None,
        key: "item".into(),
        mod_id: "example".into(),
        stack_maximum: 10,
        wearable: None,
    });
    let mod_content = value["modContent"].as_object().map(|_| ModConsumableContent {
        content_id: value["modContent"]["contentId"].as_str().unwrap().into(),
        description: value["modContent"]["description"].as_str().unwrap().into(),
        icon: unused_icon(),
        icon_trim_image_path: None,
        key: "potion".into(),
        mod_id: "example".into(),
        wearable: None,
        consume_vfx: None,
        duration_ms: 2000.0,
    });
    let mod_affixes = value["modAffixes"].as_array().map(|values| {
        values
            .iter()
            .map(|affix| ModEquipmentAffix {
                content_id: affix["contentId"].as_str().unwrap().into(),
                mod_id: affix["modId"].as_str().unwrap().into(),
                name: affix["name"].as_str().unwrap().into(),
                modifiers: vec![],
            })
            .collect()
    });
    HubInventoryItem {
        contents: value["contents"].as_array().map(|values| values.iter().map(read_item).collect()),
        equipment_type,
        generated_level: value["generatedLevel"].as_u64().map(|value| value as u32),
        icon_records: value["iconRecords"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_u64().unwrap() as usize)
            .collect(),
        icon_tints,
        id: value["id"].as_u64().unwrap(),
        inventory_slot: value["inventorySlot"].as_u64().map(|value| value as usize),
        kind,
        mod_content,
        mod_item_content,
        mod_affixes,
        name: value["name"].as_str().unwrap().into(),
        native_subtype: value["nativeSubtype"].as_u64().map(|value| value as u32),
        native_selector: value["nativeSelector"].as_u64().map(|value| value as u32),
        native_effects: value["nativeEffects"]
            .as_array()
            .map(|values| values.iter().map(read_effect).collect()),
        native_type_id: value["nativeTypeId"].as_u64().unwrap() as u32,
        quantity: value["quantity"].as_u64().unwrap() as u32,
        rarity: value["rarity"]
            .as_str()
            .map(|value| if value == "Epic" { ItemRarity::Epic } else { ItemRarity::Rare }),
        recipe_index: value["recipeIndex"].as_u64().map(|value| value as usize),
    }
}

// Icon bytes do not enter this text-only source function. Use one valid inert
// descriptor while retaining every actual identity/name/description/affix field.
fn unused_icon() -> ModItemIcon {
    ModItemIcon {
        atlas_id: "fixture".into(),
        frame_index: 0,
        image_path: "icon.png".into(),
        frame: ModSpriteFrame {
            center_offset_x: 0.0,
            center_offset_y: 0.0,
            content_height: 8.0,
            content_width: 8.0,
            height: 8.0,
            logical_height: 8.0,
            logical_width: 8.0,
            width: 8.0,
            x: 0.0,
            y: 0.0,
        },
    }
}
