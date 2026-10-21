//! Current three-bank belt ownership, effective grants and legacy migration.

use serde_json::{Value, json};
use solomon_sim::belt::*;
use solomon_sim::economy::*;
use solomon_sim::profile::{NativeSkillBookState, WizardDiscipline, WizardElement};

fn belt_snapshot(belt: &PlayerBeltComponent) -> Value {
    json!(belt.entries.map(|entry| entry.map(|entry| match entry {
        NativeBeltEntry::Skill { skill_id } => json!({"kind":"skill","skillId":skill_id}),
        NativeBeltEntry::HealthPotion => json!({"kind":"health-potion"}),
        NativeBeltEntry::ManaPotion => json!({"kind":"mana-potion"}),
        NativeBeltEntry::Item { item_id, native_type_id } =>
            json!({"kind":"item","itemId":item_id,"nativeTypeId":native_type_id}),
    })))
}
fn equipment_type(value: &Value) -> Option<EquipmentType> {
    value.as_str().map(|value| match value {
        "amulet" => EquipmentType::Amulet,
        "hat" => EquipmentType::Hat,
        "ring" => EquipmentType::Ring,
        "robe" => EquipmentType::Robe,
        "staff" => EquipmentType::Staff,
        "wand" => EquipmentType::Wand,
        other => panic!("equipment type {other}"),
    })
}

#[test]
fn native_belt_bindings_follow_owned_items_learned_skills_and_live_potion_stacks() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/belt-goldens.json")).unwrap();
    for case in fixture["creations"].as_array().unwrap() {
        let element = WizardElement::ALL
            .into_iter()
            .find(|element| Some(element.as_str()) == case["element"].as_str())
            .unwrap();
        let discipline = WizardDiscipline::ALL
            .into_iter()
            .find(|discipline| Some(discipline.as_str()) == case["discipline"].as_str())
            .unwrap();
        assert_eq!(
            belt_snapshot(&PlayerBeltComponent::new(&NativeSkillBookState::new(
                element, discipline
            ))),
            case["belt"]
        );
    }
    let mut book = NativeSkillBookState::new(WizardElement::Ether, WizardDiscipline::Arcane);
    let mut economy = HubEconomyState::new(1, WizardElement::Ether);
    let mut sack = create_fomentius_inventory_item(&FOMENTIUS_STOCK_DEFINITIONS[5], 100, 1);
    sack.contents = Some(vec![
        create_fomentius_inventory_item(&FOMENTIUS_STOCK_DEFINITIONS[0], 101, 3),
        create_fomentius_inventory_item(&FOMENTIUS_STOCK_DEFINITIONS[6], 102, 1),
        create_fomentius_inventory_item(&FOMENTIUS_STOCK_DEFINITIONS[3], 103, 1),
        create_equipment_inventory_item(&DOWSING_EQUIPMENT_RECIPES[35], 104),
    ]);
    economy.backpack.push(sack);
    let mut belt = PlayerBeltComponent::new(&book);
    assert_eq!(belt_snapshot(&belt), fixture["initial"]["belt"]);
    for record in fixture["records"].as_array().unwrap() {
        let operation = &record["operation"];
        let accepted = match operation["type"].as_str().unwrap() {
            "bind-item" => belt.apply(
                NativeBeltCommand::BindItem {
                    item_id: operation["itemId"].as_u64().unwrap(),
                    slot: operation["slot"].as_u64().unwrap() as usize,
                },
                &book,
                &economy,
            ),
            "bind-skill" => belt.apply(
                NativeBeltCommand::BindSkill {
                    skill_id: operation["skillId"].as_u64().map(|value| value as usize),
                    slot: operation["slot"].as_u64().unwrap() as usize,
                },
                &book,
                &economy,
            ),
            "learn" => {
                let previous = book.clone();
                book.grant_ranks(operation["skillId"].as_u64().unwrap() as usize, 1);
                belt.autofill_newly_learned(&previous, &book);
                true
            }
            "temporary-skill" => {
                let previous = book.clone();
                let skill_id = operation["skillId"].as_u64().unwrap() as usize;
                book.effective_ranks[skill_id] = operation["rank"].as_u64().unwrap() as u32;
                book.learned_skill_order.push(skill_id);
                belt.autofill_newly_learned(&previous, &book);
                true
            }
            "expire-temporary-skill" => {
                let skill_id = operation["skillId"].as_u64().unwrap() as usize;
                book.effective_ranks[skill_id] = 0;
                book.learned_skill_order.retain(|id| *id != skill_id);
                belt.refresh(&book, &economy);
                true
            }
            "store" => {
                assert!(
                    economy
                        .apply(
                            &HubInventoryAction::Transfer {
                                item_id: operation["itemId"].as_u64().unwrap(),
                                direction: TransferDirection::ToStorage,
                                gesture: solomon_sim::economy::HubTransferGesture::Drag,
                            },
                            &HubEconomyContext::default()
                        )
                        .accepted
                );
                belt.refresh(&book, &economy);
                true
            }
            "consume" => {
                assert!(
                    economy
                        .apply(
                            &HubInventoryAction::Consume {
                                item_id: operation["itemId"].as_u64().unwrap()
                            },
                            &HubEconomyContext::default()
                        )
                        .accepted
                );
                belt.refresh(&book, &economy);
                true
            }
            other => panic!("belt operation {other}"),
        };
        assert_eq!(accepted, record["accepted"].as_bool().unwrap());
        assert_eq!(belt_snapshot(&belt), record["belt"]);
        assert_eq!(json!(belt.skill_projection()), record["skills"]);
        for (key, kind) in
            [("health", NativeBeltPotionKind::Health), ("mana", NativeBeltPotionKind::Mana)]
        {
            let projection = native_belt_potion_projection(&economy.backpack, kind);
            assert_eq!(
                json!({"count":projection.count,"itemId":projection.item.map(|item|item.id)}),
                record[key]
            );
        }
        let items = belt.entries.map(|entry| {
            entry.and_then(|entry| native_belt_entry_item(entry, &economy)).map(|item| item.id)
        });
        assert_eq!(json!(items), record["entryItems"]);
    }
    for case in fixture["migrations"].as_array().unwrap() {
        let quickbar: [Option<usize>; 8] = case["quickbar"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_u64().map(|value| value as usize))
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();
        assert_eq!(
            belt_snapshot(&PlayerBeltComponent::from_skill_quickbar(quickbar).unwrap()),
            case["belt"]
        );
    }
    for case in fixture["admission"].as_array().unwrap() {
        let mut item = economy.equipment.hat.clone().unwrap();
        item.kind = match case["item"]["kind"].as_str().unwrap() {
            "equipment" => HubItemKind::Equipment,
            "mod-item" => HubItemKind::ModItem,
            "mod-potion" => HubItemKind::ModPotion,
            other => panic!("belt admission item kind {other}"),
        };
        item.native_type_id = case["item"]["nativeTypeId"].as_u64().unwrap() as u32;
        item.native_subtype = case["item"]["nativeSubtype"].as_u64().map(|value| value as u32);
        assert_eq!(
            native_inventory_item_can_bind_to_belt(&item),
            case["accepted"].as_bool().unwrap()
        );
    }
    for case in fixture["equipmentSlots"].as_array().unwrap() {
        let mut item = economy.equipment.hat.clone().unwrap();
        item.equipment_type = equipment_type(&case["equipmentType"]);
        let slots: Vec<_> = native_belt_equipment_slots(&item, case["unlocked"].as_bool().unwrap())
            .into_iter()
            .map(EquipmentSlot::as_str)
            .collect();
        assert_eq!(json!(slots), case["slots"]);
    }
}
