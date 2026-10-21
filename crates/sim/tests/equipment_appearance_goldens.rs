use serde_json::{Value, json};
use solomon_sim::appearance::equipment::*;
use solomon_sim::economy::{
    DOWSING_EQUIPMENT_RECIPES, EquipmentSlot, EquipmentType, HubEconomyState, HubEquipmentState,
    create_equipment_inventory_item,
};
use solomon_sim::modding::assets::ModSpriteFrame;
use solomon_sim::modding::inventory::{
    ModItemContent, ModItemIcon, ModWearableContent, ModWearableSlot,
};
use solomon_sim::profile::WizardElement;
use solomon_sim::spells::casting::NativePlayerWeaponKind;

fn garment(value: PlayerEquipmentTintedSelector) -> Value {
    json!([value.primary_tint, value.secondary_tint, value.selector])
}

#[test]
fn custom_living_art_retains_identity_and_uses_authored_stock_death_shapes() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/equipment-appearance-goldens.json"))
            .unwrap();
    for case in fixture["modCases"].as_array().unwrap() {
        let mut equipment = HubEquipmentState::default();
        let mut item = HubEconomyState::new(7, WizardElement::Ether).equipment.hat.unwrap();
        let content = mod_content(&case["content"]);
        item.mod_item_content = Some(content.clone());
        item.recipe_index = None;
        item.native_selector = None;
        item.equipment_type = Some(match case["equipmentType"].as_str().unwrap() {
            "hat" => EquipmentType::Hat,
            "robe" => EquipmentType::Robe,
            "staff" => EquipmentType::Staff,
            "wand" => EquipmentType::Wand,
            _ => unreachable!(),
        });
        item.icon_tints = case["tints"].as_array().map(|values| {
            std::array::from_fn(|index| values[index].as_u64().map(|value| value as u32))
        });
        match case["slot"].as_str().unwrap() {
            "hat" => equipment.hat = Some(item),
            "robe" => equipment.robe = Some(item),
            "weapon" => equipment.weapon = Some(item),
            _ => unreachable!(),
        }
        let living = std::panic::catch_unwind(|| {
            let appearance = player_living_equipment_appearance(WizardElement::Ether, &equipment);
            let garment = |value: PlayerRenderableGarmentAppearance| match value {
                PlayerRenderableGarmentAppearance::Native(value) => {
                    json!(["native", value.primary_tint, value.secondary_tint, value.selector])
                }
                PlayerRenderableGarmentAppearance::Mod(value) => {
                    assert_eq!(value.content, content);
                    json!([
                        "mod",
                        value.content.content_id,
                        value.primary_tint,
                        value.secondary_tint
                    ])
                }
            };
            let weapon = appearance.weapon.map(|value| match value {
                PlayerRenderableWeaponAppearance::Native(value) => {
                    let kind =
                        if value.kind == NativePlayerWeaponKind::Staff { "staff" } else { "wand" };
                    json!(["native", kind, value.selector])
                }
                PlayerRenderableWeaponAppearance::ModStaff(value) => {
                    assert_eq!(*value, content);
                    json!(["mod", "staff", value.content_id])
                }
            });
            json!([appearance.hat.map(garment), appearance.robe.map(garment), weapon])
        });
        assert_outcome(living, &case["living"]);
        let native = std::panic::catch_unwind(|| {
            let value = player_living_native_equipment_appearance(WizardElement::Ether, &equipment);
            json!([value.hat.map(garment), value.robe.map(garment), value.weapon.map(weapon)])
        });
        assert_outcome(native, &case["native"]);
        let death = std::panic::catch_unwind(|| {
            let value = player_death_equipment_appearance(WizardElement::Ether, &equipment);
            json!([garment(value.hat), garment(value.robe), weapon(value.weapon)])
        });
        assert_outcome(death, &case["death"]);
    }
}

fn assert_outcome(actual: Result<Value, Box<dyn std::any::Any + Send>>, expected: &Value) {
    if expected["error"] == true {
        assert!(actual.is_err(), "invalid custom appearance was accepted");
    } else {
        assert_eq!(actual.unwrap(), expected["value"]);
    }
}

fn mod_content(value: &Value) -> ModItemContent {
    let frame = &value["icon"]["frame"];
    ModItemContent {
        content_id: value["contentId"].as_str().unwrap().into(),
        description: value["description"].as_str().unwrap().into(),
        icon: ModItemIcon {
            atlas_id: value["icon"]["atlasId"].as_str().unwrap().into(),
            frame_index: value["icon"]["frameIndex"].as_u64().unwrap() as usize,
            image_path: value["icon"]["imagePath"].as_str().unwrap().into(),
            frame: ModSpriteFrame {
                x: frame["x"].as_f64().unwrap(),
                y: frame["y"].as_f64().unwrap(),
                width: frame["width"].as_f64().unwrap(),
                height: frame["height"].as_f64().unwrap(),
                logical_width: frame["logicalWidth"].as_f64().unwrap(),
                logical_height: frame["logicalHeight"].as_f64().unwrap(),
                center_offset_x: frame["centerOffsetX"].as_f64().unwrap(),
                center_offset_y: frame["centerOffsetY"].as_f64().unwrap(),
                content_width: frame["contentWidth"].as_f64().unwrap(),
                content_height: frame["contentHeight"].as_f64().unwrap(),
            },
        },
        icon_trim_image_path: None,
        key: value["key"].as_str().unwrap().into(),
        mod_id: value["modId"].as_str().unwrap().into(),
        stack_maximum: value["stackMaximum"].as_u64().unwrap() as u32,
        wearable: Some(ModWearableContent {
            death_shape: value["wearable"]["deathShape"].as_u64().unwrap() as usize,
            dyeable: value["wearable"]["dyeable"].as_bool().unwrap(),
            slot: match value["wearable"]["slot"].as_str().unwrap() {
                "hat" => ModWearableSlot::Hat,
                "robe" => ModWearableSlot::Robe,
                "staff" => ModWearableSlot::Staff,
                _ => unreachable!(),
            },
            worn_image_path: value["wearable"]["wornImagePath"].as_str().unwrap().into(),
            worn_trim_image_path: None,
        }),
    }
}
fn weapon(value: PlayerWeaponAppearance) -> Value {
    json!([
        if value.kind == NativePlayerWeaponKind::Staff { "staff" } else { "wand" },
        value.selector
    ])
}

#[test]
fn starter_recipe_dye_and_generated_equipment_appearance_replay_the_website() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/equipment-appearance-goldens.json"))
            .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let element = match case["element"].as_str().unwrap() {
            "air" => WizardElement::Air,
            "earth" => WizardElement::Earth,
            "fire" => WizardElement::Fire,
            "water" => WizardElement::Water,
            _ => WizardElement::Ether,
        };
        let mode = case["mode"].as_str().unwrap();
        let mut equipment = if mode == "fresh" {
            HubEconomyState::new(7, element).equipment
        } else {
            HubEquipmentState::default()
        };
        if mode == "recipe" || mode == "dyed" {
            let index = case["recipe"].as_u64().unwrap() as usize;
            let recipe = DOWSING_EQUIPMENT_RECIPES
                .iter()
                .find(|recipe| recipe.source_index == index)
                .unwrap();
            let mut item = create_equipment_inventory_item(recipe, 100);
            if mode == "dyed" {
                item.icon_tints = Some([Some(0x123456), Some(0xabcdef)]);
            }
            let slot = match recipe.equipment_type {
                EquipmentType::Hat => EquipmentSlot::Hat,
                EquipmentType::Robe => EquipmentSlot::Robe,
                _ => EquipmentSlot::Weapon,
            };
            equipment.set(slot, Some(item));
        } else if mode == "generated" {
            let mut item = HubEconomyState::new(7, element).equipment.hat.unwrap();
            let (kind, slot) = match case["type"].as_str().unwrap() {
                "hat" => (EquipmentType::Hat, EquipmentSlot::Hat),
                "robe" => (EquipmentType::Robe, EquipmentSlot::Robe),
                "staff" => (EquipmentType::Staff, EquipmentSlot::Weapon),
                _ => (EquipmentType::Wand, EquipmentSlot::Weapon),
            };
            item.equipment_type = Some(kind);
            item.native_selector = Some(case["selector"].as_u64().unwrap() as u32);
            item.icon_tints = Some([Some(0x123456), Some(0xabcdef)]);
            equipment.set(slot, Some(item));
        }
        let living = player_living_native_equipment_appearance(element, &equipment);
        let death = player_death_equipment_appearance(element, &equipment);
        assert_eq!(
            json!([living.hat.map(garment), living.robe.map(garment), living.weapon.map(weapon)]),
            case["living"],
            "{mode} living"
        );
        assert_eq!(
            json!([garment(death.hat), garment(death.robe), weapon(death.weapon)]),
            case["death"],
            "{mode} death"
        );
    }
}
