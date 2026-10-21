use serde_json::{Value, json};
use solomon_sim::Vec2;
use solomon_sim::belt::NativeBeltCommand;
use solomon_sim::economy::{
    DOWSING_EQUIPMENT_RECIPES, EquipmentSlot, FOMENTIUS_STOCK_DEFINITIONS, HubEconomyState,
    HubInventoryAction, HubInventoryItem, TransferDirection, create_equipment_inventory_item,
    create_fomentius_inventory_item,
};
use solomon_sim::hud::HudRect;
use solomon_sim::player_panels::inventory_drop::{
    InventoryDropContext, InventoryDropEffect, InventoryDropSource, drop_inventory_source,
};
use solomon_sim::profile::WizardElement;

#[test]
fn inventory_drop_commands_restore_swaps_addresses_and_sink_order_match_current_website() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/inventory-drop-goldens.json")).unwrap();
    let belt_rects = std::array::from_fn(|index| {
        let row = &fixture["beltRects"][index];
        HudRect::new(
            number(&row["x"]),
            number(&row["y"]),
            number(&row["width"]),
            number(&row["height"]),
        )
    });
    for case in fixture["cases"].as_array().unwrap() {
        let input = &case["input"];
        let mut economy = HubEconomyState::new(1, WizardElement::Ether);
        economy.backpack = input["backpack"].as_array().unwrap().iter().map(item).collect();
        economy.equipment = Default::default();
        for entry in input["equipment"].as_array().unwrap() {
            economy.equipment.set(slot(&entry["slot"]), Some(item(&entry["item"])));
        }
        economy.owned_perk_selectors =
            if input["unlocked"].as_bool().unwrap() { vec![19] } else { Vec::new() };
        let path: Vec<_> =
            input["path"].as_array().unwrap().iter().map(|value| value.as_u64().unwrap()).collect();
        let context = InventoryDropContext {
            economy: &economy,
            sack_path: &path,
            belt_rects: &belt_rects,
            belt_start: input["bankStart"].as_u64().unwrap() as usize,
            companion: input["companion"].as_bool().unwrap(),
            storage_drop_rect: input["storage"]
                .as_bool()
                .unwrap()
                .then(|| rect(&fixture["storageRect"])),
            reference_drop_rect: input["reference"]
                .as_bool()
                .unwrap()
                .then(|| rect(&fixture["referenceRect"])),
        };
        let source = &input["source"];
        let item_id = source["itemId"].as_u64().unwrap();
        let source = if source["owner"] == "backpack" {
            InventoryDropSource::Backpack { item_id }
        } else {
            InventoryDropSource::Equipment { item_id, slot: slot(&source["equipmentSlot"]) }
        };
        let actual: Vec<_> = drop_inventory_source(source, point(&input["point"]), &context)
            .iter()
            .map(effect)
            .collect();
        assert_wire(&Value::Array(actual), &case["expected"], input["name"].as_str().unwrap());
    }
}

fn item(spec: &Value) -> HubInventoryItem {
    let id = spec["id"].as_u64().unwrap();
    let mut built = if let Some(recipe) = spec["recipe"].as_u64() {
        create_equipment_inventory_item(&DOWSING_EQUIPMENT_RECIPES[recipe as usize], id)
    } else {
        let definition = FOMENTIUS_STOCK_DEFINITIONS
            .iter()
            .find(|row| row.kind.as_str() == spec["kind"].as_str().unwrap())
            .unwrap();
        create_fomentius_inventory_item(definition, id, 1)
    };
    built.inventory_slot = spec["slot"].as_u64().map(|slot| slot as usize);
    if let Some(contents) = spec["contents"].as_array() {
        built.contents = Some(contents.iter().map(item).collect());
    }
    built
}

fn slot(value: &Value) -> EquipmentSlot {
    EquipmentSlot::ALL.into_iter().find(|slot| slot.as_str() == value.as_str().unwrap()).unwrap()
}

fn point(value: &Value) -> Vec2 {
    Vec2::new(number(&value["x"]), number(&value["y"]))
}
fn rect(value: &Value) -> HudRect {
    HudRect::new(number(&value[0]), number(&value[1]), number(&value[2]), number(&value[3]))
}
fn number(value: &Value) -> f64 {
    value.as_f64().unwrap()
}

fn effect(effect: &InventoryDropEffect) -> Value {
    match effect {
        InventoryDropEffect::Action(value) => json!({"kind":"action", "action":action(value)}),
        InventoryDropEffect::BindBelt(NativeBeltCommand::BindItem { item_id, slot }) => {
            json!({"kind":"belt", "itemId":item_id, "slot":slot})
        }
        InventoryDropEffect::BindBelt(_) => panic!("Source inventory drop does not bind a skill"),
        InventoryDropEffect::Flyby(request) => json!({
            "kind":"flyby", "action":request.action.as_ref().map(action),
            "lanes":request.lanes.iter().map(|lane| json!({"itemId":lane.item.id, "from":{"x":lane.from.x,"y":lane.from.y}, "to":{"x":lane.to.x,"y":lane.to.y}})).collect::<Vec<_>>()
        }),
        InventoryDropEffect::Sound { cue, playback_rate } => {
            json!({"kind":"sound", "cue":cue.as_str(), "playbackRate":playback_rate})
        }
        InventoryDropEffect::SelectReference(item_id) => {
            json!({"kind":"reference", "itemId":item_id})
        }
        InventoryDropEffect::ConfirmUnforge(item_id) => {
            json!({"kind":"confirm-unforge", "itemId":item_id})
        }
        InventoryDropEffect::RequiredClothing(slot) => {
            json!({"kind":"required-clothing", "slot":slot.as_str()})
        }
    }
}

fn action(value: &HubInventoryAction) -> Value {
    match value {
        HubInventoryAction::Move { item_id, destination_sack_id, destination_slot } => {
            json!({"type":"move-inventory-item", "itemId":item_id,"destinationSackId":destination_sack_id,"destinationSlot":destination_slot})
        }
        HubInventoryAction::Unequip { slot, destination_sack_id, destination_slot } => {
            json!({"type":"unequip", "slot":slot.as_str(),"destinationSackId":destination_sack_id,"destinationSlot":destination_slot})
        }
        HubInventoryAction::Equip { item_id, slot } => {
            json!({"type":"equip", "itemId":item_id,"slot":slot.as_str()})
        }
        HubInventoryAction::Unforge { item_id } => json!({"type":"unforge", "itemId":item_id}),
        HubInventoryAction::Transfer { item_id, direction, .. } => {
            json!({"type":"transfer", "itemId":item_id, "direction":match direction {TransferDirection::ToBackpack=>"to-backpack",TransferDirection::ToStorage=>"to-storage"},"gesture":"drag"})
        }
        _ => panic!("unexpected inventory drop action {value:?}"),
    }
}

// JSON does not preserve whether a source number was written with a decimal point.
// Compare every numeric value as an exact double, retaining all object/array members.
fn assert_wire(actual: &Value, expected: &Value, name: &str) {
    match (actual, expected) {
        (Value::Number(_), Value::Number(_)) => {
            assert_eq!(actual.as_f64(), expected.as_f64(), "{name}")
        }
        (Value::Array(actual), Value::Array(expected)) => {
            assert_eq!(actual.len(), expected.len(), "{name}: {actual:?} != {expected:?}");
            for (actual, expected) in actual.iter().zip(expected) {
                assert_wire(actual, expected, name);
            }
        }
        (Value::Object(actual), Value::Object(expected)) => {
            assert_eq!(actual.len(), expected.len(), "{name}: {actual:?} != {expected:?}");
            for (key, expected) in expected {
                assert_wire(&actual[key], expected, name);
            }
        }
        _ => assert_eq!(actual, expected, "{name}"),
    }
}
