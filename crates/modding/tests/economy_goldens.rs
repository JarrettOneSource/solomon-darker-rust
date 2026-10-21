//! Authored items stay in the actual economy and Boast state through transactions.

use serde::Deserialize;
use serde_json::{Value, json};
use solomon_modding::{assets::PreparedModAssetCatalog, content::compile_mod_content_catalog};
use solomon_sim::economy::*;
use solomon_sim::hub::interaction::BoastFailureProducer;
use solomon_sim::modding::assets::PreparedModAsset;
use solomon_sim::modding::content::{PreparedModContentCatalog, PreparedModContentDefinition};
use solomon_sim::modding::definition::CompiledWebLuaMod;
use solomon_sim::modding::inventory::*;
use solomon_sim::profile::WizardElement;

#[derive(Deserialize)]
struct CatalogFixture {
    cases: Vec<CatalogCase>,
}
#[derive(Deserialize)]
struct CatalogCase {
    name: String,
    mods: Vec<CompiledWebLuaMod>,
    assets: Vec<PreparedModAsset>,
}

fn catalog() -> PreparedModContentCatalog {
    let fixture: CatalogFixture = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/mod-content-goldens.json"
    ))
    .unwrap();
    let source =
        fixture.cases.into_iter().find(|case| case.name == "all-sixteen-families").unwrap();
    compile_mod_content_catalog(&source.mods, &PreparedModAssetCatalog::new(source.assets).unwrap())
        .unwrap()
}

fn id(catalog: &PreparedModContentCatalog, key: &str) -> String {
    catalog.all().iter().find(|row| row.common().key == key).unwrap().common().content_id.clone()
}

fn item_value(item: &HubInventoryItem) -> Value {
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

fn assert_value(actual: &Value, expected: &Value, path: &str) {
    match (actual, expected) {
        (Value::Number(actual), Value::Number(expected)) => {
            assert_eq!(actual.as_f64(), expected.as_f64(), "{path}")
        }
        (Value::Array(actual), Value::Array(expected)) => {
            assert_eq!(actual.len(), expected.len(), "{path} length");
            for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
                assert_value(actual, expected, &format!("{path}[{index}]"));
            }
        }
        (Value::Object(actual), Value::Object(expected)) => {
            assert_eq!(actual.len(), expected.len(), "{path} fields");
            for (key, expected) in expected {
                assert_value(&actual[key], expected, &format!("{path}.{key}"));
            }
        }
        _ => assert_eq!(actual, expected, "{path}"),
    }
}

fn state_value(state: &HubEconomyState) -> Value {
    let equipment: serde_json::Map<_, _> = [
        ("amulet", state.equipment.amulet.as_ref()),
        ("hat", state.equipment.hat.as_ref()),
        ("robe", state.equipment.robe.as_ref()),
        ("weapon", state.equipment.weapon.as_ref()),
    ]
    .into_iter()
    .map(|(key, item)| (key.to_owned(), item.map_or(Value::Null, item_value)))
    .collect();
    let mut equipment = Value::Object(equipment);
    equipment["rings"] = json!(
        state
            .equipment
            .rings
            .iter()
            .map(|item| item.as_ref().map_or(Value::Null, item_value))
            .collect::<Vec<_>>()
    );
    json!({"backpack":state.backpack.iter().map(item_value).collect::<Vec<_>>(),
        "storage":state.storage.iter().map(item_value).collect::<Vec<_>>(),"equipment":equipment,
        "nextItemId":state.next_item_id,"revision":state.revision,
        "rng":{"indexA":state.rng.index_a,"indexB":state.rng.index_b,"words":&state.rng.words[..]},
        "boast":{"failed":state.npc.boast.failed,"failureSequence":state.npc.boast.failure_sequence,
            "selected":state.npc.boast.selected,"succeeded":state.npc.boast.succeeded},
        "valid":state.inventory_is_valid()})
}

fn owned_id(state: &HubEconomyState, key: &str) -> u64 {
    state
        .backpack
        .iter()
        .find(|item| item.mod_item_content.as_ref().is_some_and(|content| content.key == key))
        .unwrap()
        .id
}

#[test]
fn authored_item_factories_stacks_clothing_reforge_and_reconciliation_match_source() {
    let catalog = catalog();
    let fixture: Value = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/mod-economy-goldens.json"
    ))
    .unwrap();
    for case in fixture["factories"].as_array().unwrap() {
        let definition = catalog.item(&id(&catalog, case["key"].as_str().unwrap())).unwrap();
        match mod_item_inventory_item(
            &definition.catalog,
            case["quantity"].as_u64().unwrap() as u32,
        ) {
            Ok(item) => {
                assert!(case["error"].is_null());
                assert_value(&item_value(&item), &case["item"], "factory");
            }
            Err(error) => assert_eq!(Some(error.as_str()), case["error"].as_str()),
        }
    }
    let potion_catalog = &catalog.potion(&id(&catalog, "tonic")).unwrap().catalog;
    assert_value(
        &item_value(&mod_consumable_inventory_item(potion_catalog)),
        &fixture["potion"],
        "potion factory",
    );
    for case in fixture["loot"].as_array().unwrap() {
        let items = catalog
            .create_loot_items(case["seed"].as_u64().unwrap(), case["boss"].as_bool().unwrap())
            .unwrap();
        assert_value(
            &json!(items.iter().map(item_value).collect::<Vec<_>>()),
            &case["items"],
            "content loot",
        );
    }
    for case in fixture["identities"].as_array().unwrap() {
        let value = &case["item"];
        let content: ModItemContent =
            serde_json::from_value(value["modItemContent"].clone()).unwrap();
        let definition = ModItemCatalogEntry {
            content,
            icon_tints: value["iconTints"].as_array().map(|tints| {
                [tints[0].as_u64().unwrap() as u32, tints[1].as_u64().unwrap() as u32]
            }),
            name: value["name"].as_str().unwrap().into(),
        };
        let mut item = mod_item_inventory_item(&definition, 1).unwrap();
        item.quantity = value["quantity"].as_u64().unwrap() as u32;
        item.native_effects =
            value.get("nativeEffects").map(|value| serde_json::from_value(value.clone()).unwrap());
        assert_eq!(
            mod_item_inventory_identity_is_valid(&item),
            case["valid"].as_bool().unwrap(),
            "{}",
            case["name"]
        );
    }
    let token = &catalog.item(&id(&catalog, "token")).unwrap().catalog;
    let robe = &catalog.item(&id(&catalog, "robe")).unwrap().catalog;
    let affix: ModEquipmentAffix = serde_json::from_value(fixture["affix"].clone()).unwrap();
    let foreign: ModEquipmentAffix = serde_json::from_value(fixture["foreign"].clone()).unwrap();
    let mut state = HubEconomyState::new(77, WizardElement::Ether);
    let mut robe_id = 0;
    let mut dye_session = None;
    for step in fixture["steps"].as_array().unwrap() {
        let label = step["label"].as_str().unwrap();
        let accepted = match label {
            "token-six" | "token-twelve" => {
                state.insert_loot_item(mod_item_inventory_item(token, 6).unwrap())
            }
            "token-overflow" => state.insert_loot_item(mod_item_inventory_item(token, 1).unwrap()),
            "potion" => state.insert_loot_item(mod_consumable_inventory_item(potion_catalog)),
            "same-content-stale-subtype" => {
                let mut item = mod_consumable_inventory_item(potion_catalog);
                item.native_subtype = item.native_subtype.map(|subtype| subtype + 17);
                state.insert_loot_item(item)
            }
            "robe-one" | "robe-two" => {
                let result = state.insert_loot_item(mod_item_inventory_item(robe, 1).unwrap());
                robe_id = owned_id(&state, "robe");
                result
            }
            "dye" => {
                let definition = FOMENTIUS_STOCK_DEFINITIONS
                    .iter()
                    .find(|definition| definition.kind == HubItemKind::Dye)
                    .unwrap();
                state.insert_loot_item(create_fomentius_inventory_item(definition, 0, 2))
            }
            "open-authored-dye" => {
                let dye =
                    state.backpack.iter().find(|item| item.kind == HubItemKind::Dye).unwrap().id;
                let result = state.begin_inventory_dye_session(dye, "authored-paint");
                dye_session = result.dye_session;
                result.accepted
            }
            "dye-authored-clothing" => {
                state
                    .dye_inventory_clothing(
                        dye_session.as_ref().unwrap(),
                        robe_id,
                        NativeDyeLayer::Trim,
                        &[1, 8],
                    )
                    .accepted
            }
            "reforge-authored-clothing" => {
                state.reforge_mod_equipment(robe_id, &[affix.clone(), foreign.clone()]).accepted
            }
            "reforge-reject-duplicate" => {
                state.reforge_mod_equipment(robe_id, &[affix.clone(), affix.clone()]).accepted
            }
            "equip-authored-clothing" => {
                state
                    .apply(
                        &HubInventoryAction::Equip { item_id: robe_id, slot: EquipmentSlot::Robe },
                        &HubEconomyContext::default(),
                    )
                    .accepted
            }
            "consume-authored-potion" => {
                let potion = state
                    .backpack
                    .iter()
                    .find(|item| item.kind == HubItemKind::ModPotion)
                    .unwrap()
                    .id;
                state
                    .apply(
                        &HubInventoryAction::Consume { item_id: potion },
                        &HubEconomyContext::default(),
                    )
                    .accepted
            }
            "transfer-authored-item" => {
                let token = owned_id(&state, "token");
                state
                    .apply(
                        &HubInventoryAction::Transfer {
                            item_id: token,
                            direction: TransferDirection::ToStorage,
                            gesture: solomon_sim::economy::HubTransferGesture::Drag,
                        },
                        &HubEconomyContext::default(),
                    )
                    .accepted
            }
            "reconcile-foreign-affix" => {
                let definition = catalog.boast(&id(&catalog, "vow")).unwrap();
                state.npc.boast.select(BoastSelection::Mod(definition.selection.clone()));
                state.reconcile_mod_packages(&["ASSET.TEST".into()]);
                true
            }
            "reconcile-missing-owner" => {
                state.reconcile_mod_packages(&[]);
                true
            }
            _ => panic!("unknown recorded transaction {label}"),
        };
        assert_eq!(accepted, step["accepted"].as_bool().unwrap(), "{label}");
        assert_value(&state_value(&state), &step["state"], label);
    }
}

#[test]
fn native_and_authored_boasts_share_failure_success_and_score_rules() {
    let catalog = catalog();
    let fixture: Value = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/mod-economy-goldens.json"
    ))
    .unwrap();
    for case in fixture["boasts"].as_array().unwrap() {
        let selection: BoastSelection = serde_json::from_value(case["selection"].clone()).unwrap();
        let mut state = BoastState::default();
        state.select(selection);
        for row in case["rows"].as_array().unwrap() {
            let rules = catalog.boast_rules(state.selected.as_ref().unwrap());
            if row["event"]["type"] == "wave" {
                state
                    .complete_wave_with_rules(row["event"]["wave"].as_u64().unwrap() as u32, rules);
            } else {
                let producer = match row["event"]["producer"].as_str().unwrap() {
                    "potion-use" => BoastFailureProducer::PotionUse,
                    "secondary-cast" => BoastFailureProducer::SecondaryCast,
                    _ => unreachable!(),
                };
                state.fail_with_rules(producer, rules);
            }
            let actual = json!({"failed":state.failed,"failureSequence":state.failure_sequence,"selected":state.selected,"succeeded":state.succeeded});
            assert_value(&actual, &row["state"], "boast state");
            assert_eq!(state.score_with_rules(16_777_217, rules), row["score"].as_u64().unwrap());
            assert_eq!(
                state.uses_random_skill_choices_with_rules(rules),
                row["random"].as_bool().unwrap()
            );
            assert_eq!(state.failure_text_with_rules(rules).as_deref(), row["failure"].as_str());
        }
    }
    // Keeping an authored selection does not grant another package's definition.
    let definition = catalog
        .all()
        .iter()
        .find_map(|row| match row {
            PreparedModContentDefinition::Boast(value) => Some(value),
            _ => None,
        })
        .unwrap();
    let mut selection = definition.selection.clone();
    selection.mod_id = "another.mod".into();
    assert!(catalog.boast_rules(&BoastSelection::Mod(selection)).is_none());
}
