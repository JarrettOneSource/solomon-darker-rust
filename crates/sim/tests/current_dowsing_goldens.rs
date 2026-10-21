//! Current Shlorio reference draws, native pricing and rolled service lifetime.

mod support;

use serde_json::{Value, json};
use solomon_sim::NativeRng;
use solomon_sim::economy::{
    DOWSING_EQUIPMENT_RECIPES, FOMENTIUS_STOCK_DEFINITIONS, HubEconomyContext, HubEconomyState,
    HubInventoryAction, HubInventoryItem, create_equipment_inventory_item,
    create_fomentius_inventory_item, project_inventory_items,
};
use solomon_sim::profile::WizardElement;

fn item(index: usize) -> HubInventoryItem {
    create_equipment_inventory_item(&DOWSING_EQUIPMENT_RECIPES[index], 90000 + index as u64)
}

fn source_for(mode: &str) -> HubEconomyState {
    let mut economy = HubEconomyState::new(1, WizardElement::Ether);
    economy.gold = 20000;
    if mode != "ordinary" {
        economy.rng = NativeRng::new(if mode == "missing-37" { 1394 } else { 1 });
    }
    match mode {
        "hood" => economy.backpack = vec![item(16)],
        "all" | "missing-37" => {
            economy.backpack = DOWSING_EQUIPMENT_RECIPES
                .iter()
                .filter(|recipe| mode != "missing-37" || recipe.source_index != 37)
                .map(|recipe| item(recipe.source_index))
                .collect();
        }
        "nested" => {
            let mut sack =
                create_fomentius_inventory_item(&FOMENTIUS_STOCK_DEFINITIONS[5], 99000, 1);
            sack.contents = Some(
                DOWSING_EQUIPMENT_RECIPES
                    .iter()
                    .filter(|recipe| ![5, 16, 17, 19].contains(&recipe.source_index))
                    .map(|recipe| item(recipe.source_index))
                    .collect(),
            );
            economy.backpack = vec![item(16), sack];
            economy.storage = vec![item(19)];
            economy.equipment.hat = Some(item(5));
        }
        "ordinary" | "potion" => {}
        other => panic!("unknown Source setup {other}"),
    }
    economy
}

fn snapshot(economy: &HubEconomyState) -> Value {
    json!({
        "gold": economy.gold,
        "dowsingFee": economy.dowsing_fee,
        "dowsingOffers": economy.dowsing_offers.iter().map(|offer| json!({
            "id": offer.id, "price": offer.price, "recipeIndex": offer.recipe_index,
        })).collect::<Vec<_>>(),
        "dowsingRolled": economy.dowsing_rolled,
        "nextItemId": economy.next_item_id,
        "nextOfferId": economy.next_offer_id,
        "revision": economy.revision,
        "rng": {"indexA": economy.rng.index_a, "indexB": economy.rng.index_b, "words": &economy.rng.words[..]},
        "backpack": project_inventory_items(&economy.backpack).iter()
            .map(|entry| json!([entry.item.id, entry.item.recipe_index])).collect::<Vec<_>>(),
        "storage": project_inventory_items(&economy.storage).iter()
            .map(|entry| json!([entry.item.id, entry.item.recipe_index])).collect::<Vec<_>>(),
    })
}

#[test]
fn current_shlorio_selects_all_recipes_before_pricing() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/current-dowsing-goldens.json"))
            .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let mut economy = source_for(case["mode"].as_str().unwrap());
        support::assert_website_json(&snapshot(&economy), &case["initial"], "initial", &[]);
        for (index, expected) in case["records"].as_array().unwrap().iter().enumerate() {
            let input = &expected["input"];
            let action = match input["type"].as_str().unwrap() {
                "dowse" => HubInventoryAction::Dowse {
                    reference_item_id: input["referenceItemId"].as_u64(),
                },
                "buy" => {
                    HubInventoryAction::BuyDowsing { offer_id: input["offerId"].as_u64().unwrap() }
                }
                "close" => HubInventoryAction::CloseDowsing,
                other => panic!("unknown Source command {other}"),
            };
            let before = economy.clone();
            let result = economy.apply(&action, &HubEconomyContext::default());
            if !result.accepted {
                assert_eq!(economy, before, "rejected Shlorio transactions are atomic");
            }
            support::assert_website_json(
                &json!({
                    "input": input,
                    "accepted": result.accepted,
                    "reason": result.reason.map(|reason| reason.as_str()),
                    "dowsingPitch": result.dowsing_pitch,
                    "state": snapshot(&economy),
                }),
                expected,
                &format!("{} command {index}", case["name"]),
                &[],
            );
        }
    }
}
