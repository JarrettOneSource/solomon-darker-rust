//! Current equipment removal and Sack naming retain actual addressed inventory roots.

mod support;

use serde_json::{Value, json};
use solomon_sim::economy::{
    DOWSING_EQUIPMENT_RECIPES, EquipmentSlot, EquipmentType, FOMENTIUS_STOCK_DEFINITIONS,
    HubEconomyContext, HubEconomyState, HubInventoryAction, HubInventoryItem,
    create_equipment_inventory_item, create_fomentius_inventory_item,
    normalize_inventory_sack_name, project_inventory_items,
};
use solomon_sim::profile::WizardElement;

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/website/current-inventory-goldens.json")).unwrap()
}

fn potion(id: u64) -> HubInventoryItem {
    create_fomentius_inventory_item(&FOMENTIUS_STOCK_DEFINITIONS[0], id, 1)
}

fn sack(id: u64, contents: Vec<HubInventoryItem>) -> HubInventoryItem {
    let mut sack = create_fomentius_inventory_item(&FOMENTIUS_STOCK_DEFINITIONS[5], id, 1);
    sack.contents = Some(contents);
    sack
}

fn equipment(kind: EquipmentType, id: u64) -> HubInventoryItem {
    let recipe =
        DOWSING_EQUIPMENT_RECIPES.iter().find(|recipe| recipe.equipment_type == kind).unwrap();
    create_equipment_inventory_item(recipe, id)
}

fn projected(items: &[HubInventoryItem]) -> Value {
    let mut rows = Vec::new();
    for entry in project_inventory_items(items) {
        rows.push(json!({
            "id": entry.item.id,
            "name": entry.item.name,
            "parentSackId": entry.parent_sack_id,
            "slot": entry.slot,
            "storedSlot": entry.item.inventory_slot,
        }));
    }
    json!(rows)
}

fn snapshot(state: &HubEconomyState) -> Value {
    let equipment =
        [EquipmentSlot::Hat, EquipmentSlot::Robe, EquipmentSlot::Weapon, EquipmentSlot::Amulet]
            .map(|slot| state.equipment.at(slot).as_ref().map(|item| item.id));
    json!({
        "backpack": projected(&state.backpack),
        "storage": projected(&state.storage),
        "revision": state.revision,
        "equipment": equipment,
        "rings": state.equipment.rings.each_ref().map(|item| item.as_ref().map(|item| item.id)),
        "rng": {"indexA": state.rng.index_a, "indexB": state.rng.index_b, "words": &state.rng.words[..]},
        "valid": state.inventory_is_valid(),
    })
}

#[test]
fn current_sack_names_follow_utf16_controls_and_invisible_formatting() {
    let fixture = fixture();
    for field in ["names", "formatNames"] {
        for expected in fixture[field].as_array().unwrap() {
            let value = expected["value"].as_str().unwrap();
            assert_eq!(
                normalize_inventory_sack_name(value),
                expected["result"].as_str(),
                "{field}: {value:?}"
            );
        }
    }
    let mut state = HubEconomyState::new(1, WizardElement::Ether);
    state.backpack = vec![sack(100, vec![sack(101, vec![])]), potion(102)];
    state.storage = vec![sack(200, vec![sack(201, vec![])])];
    support::assert_website_json(&snapshot(&state), &fixture["renameInitial"], "initial", &[]);
    for (index, expected) in fixture["renames"].as_array().unwrap().iter().enumerate() {
        let result = state.apply(
            &HubInventoryAction::RenameSack {
                item_id: expected["itemId"].as_u64().unwrap(),
                name: expected["name"].as_str().unwrap().to_owned(),
            },
            &HubEconomyContext::default(),
        );
        let actual = json!({
            "accepted": result.accepted,
            "reason": result.reason.map(|reason| reason.as_str()),
            "state": snapshot(&state),
        });
        let expected = json!({
            "accepted": expected["accepted"],
            "reason": expected["reason"],
            "state": expected["state"],
        });
        support::assert_website_json(&actual, &expected, &format!("rename[{index}]"), &[]);
    }
}

#[test]
fn current_unequip_preserves_destination_swaps_capacity_and_rejection_order() {
    let fixture = fixture();
    for expected in fixture["unequips"].as_array().unwrap() {
        let mut state = HubEconomyState::new(1, WizardElement::Ether);
        state.backpack = vec![sack(100, vec![potion(103)]), potion(102)];
        if expected["mode"] == "full-visible" {
            let recipe = DOWSING_EQUIPMENT_RECIPES
                .iter()
                .find(|recipe| recipe.equipment_type == EquipmentType::Ring)
                .unwrap();
            let mut contents = Vec::new();
            for slot in 0..87 {
                let mut item = create_equipment_inventory_item(recipe, 1000 + slot as u64);
                item.inventory_slot = Some(slot);
                contents.push(item);
            }
            state.backpack = vec![sack(100, contents)];
        }
        let name = expected["name"].as_str().unwrap();
        support::assert_website_json(&snapshot(&state), &expected["initial"], name, &[]);
        let slot = EquipmentSlot::ALL
            .into_iter()
            .find(|slot| Some(slot.as_str()) == expected["slot"].as_str())
            .unwrap();
        let result = state.apply(
            &HubInventoryAction::Unequip {
                slot,
                destination_sack_id: expected["destinationSackId"].as_u64(),
                destination_slot: expected["destinationSlot"].as_u64().map(|slot| slot as usize),
            },
            &HubEconomyContext::default(),
        );
        let actual = json!({
            "accepted": result.accepted,
            "reason": result.reason.map(|reason| reason.as_str()),
            "state": snapshot(&state),
        });
        let expected = json!({
            "accepted": expected["accepted"],
            "reason": expected["reason"],
            "state": expected["state"],
        });
        support::assert_website_json(&actual, &expected, name, &[]);
    }
}

#[test]
fn current_equipped_rings_swap_without_changing_inventory_addresses() {
    let fixture = fixture();
    for expected in fixture["equips"].as_array().unwrap() {
        let mut state = HubEconomyState::new(1, WizardElement::Ether);
        let mode = expected["mode"].as_str().unwrap();
        state.owned_perk_selectors = if mode == "third-ring" { vec![19] } else { vec![] };
        state.equipment.rings = [
            Some(equipment(EquipmentType::Ring, 900)),
            if mode == "empty-ring" { None } else { Some(equipment(EquipmentType::Ring, 901)) },
            if mode == "third-ring" { Some(equipment(EquipmentType::Ring, 902)) } else { None },
        ];
        let name = expected["name"].as_str().unwrap();
        support::assert_website_json(&snapshot(&state), &expected["initial"], name, &[]);
        let slot = EquipmentSlot::ALL
            .into_iter()
            .find(|slot| Some(slot.as_str()) == expected["destinationSlot"].as_str())
            .unwrap();
        let result = state.apply(
            &HubInventoryAction::Equip { item_id: expected["itemId"].as_u64().unwrap(), slot },
            &HubEconomyContext { player_level: 1000, ..HubEconomyContext::default() },
        );
        support::assert_website_json(
            &json!({"accepted": result.accepted, "reason": result.reason.map(|reason| reason.as_str()), "state": snapshot(&state)}),
            &json!({"accepted": expected["accepted"], "reason": expected["reason"], "state": expected["state"]}),
            name,
            &[],
        );
    }
}

#[test]
fn current_sack_equip_uses_addressed_order_and_does_not_reselect_the_displaced_wand() {
    let fixture = fixture();
    for expected in fixture["bulk"].as_array().unwrap() {
        let mut state = HubEconomyState::new(1, WizardElement::Ether);
        let mode = expected["mode"].as_str().unwrap();
        let mut contents = vec![equipment(EquipmentType::Staff, 101)];
        if mode == "addressed-child-order" {
            contents.clear();
            for (kind, id, slot) in [
                (EquipmentType::Staff, 101, 7),
                (EquipmentType::Staff, 102, 2),
                (EquipmentType::Robe, 103, 5),
                (EquipmentType::Ring, 104, 9),
                (EquipmentType::Ring, 105, 1),
            ] {
                let mut item = equipment(kind, id);
                item.inventory_slot = Some(slot);
                contents.push(item);
            }
        }
        state.backpack = vec![sack(100, contents)];
        state.equipment.weapon = Some(equipment(EquipmentType::Wand, 900));
        support::assert_website_json(&snapshot(&state), &expected["initial"], mode, &[]);
        let result = state.apply(
            &HubInventoryAction::EquipSackContents { item_id: 100 },
            &HubEconomyContext { player_level: 1000, ..HubEconomyContext::default() },
        );
        support::assert_website_json(
            &json!({"accepted": result.accepted, "reason": result.reason.map(|reason| reason.as_str()), "state": snapshot(&state)}),
            &json!({"accepted": expected["accepted"], "reason": expected["reason"], "state": expected["state"]}),
            mode,
            &[],
        );
    }
}
