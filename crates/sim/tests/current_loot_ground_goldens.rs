//! Actual loot-store materialization uses its retained carriers at every selection.
#[path = "support/native_bits.rs"]
#[allow(dead_code)]
mod native_bits;

use serde_json::{Value, json};
use solomon_sim::Vec2;
use solomon_sim::boneyard::loot::*;
use solomon_sim::economy::{
    DOWSING_EQUIPMENT_RECIPES, HubInventoryItem, create_equipment_inventory_item,
};
use solomon_sim::loot::*;
use solomon_sim::painter::WorldPainterOrder;

fn item_identity(item: &HubInventoryItem) -> Value {
    json!({"id":item.id,"nativeTypeId":item.native_type_id,"recipeIndex":item.recipe_index,
        "contents":item.contents.as_ref().map(|contents|contents.iter().map(item_identity).collect::<Vec<_>>())})
}
fn snapshot(store: &BoneyardLootStore, order: WorldPainterOrder) -> Value {
    let items: Vec<_> = store.actors.iter().map(|actor|json!({"id":actor.id,"nativeTypeId":actor.native_type_id,
        "item":actor.item.as_ref().map(item_identity),"itemId":actor.item.as_ref().map(|item|item.id),"recipeIndex":actor.item.as_ref().and_then(|item|item.recipe_index),
        "amount":actor.amount,"source":match actor.source {NativeLootDropSource::Enemy=>"enemy",NativeLootDropSource::Goodie=>"goodie",NativeLootDropSource::Script=>"script"}})).collect();
    json!({"items":items,"rng":{"indexA":store.shared_rng.index_a,"indexB":store.shared_rng.index_b,"words":store.shared_rng.words.as_slice()},
        "nextActorId":store.next_actor_id,"nextItemId":store.next_item_id,"lastSuccessfulItemLevel":store.last_successful_item_level,
        "nextKeyDropLevel":store.next_key_drop_level,"order":{"nextRegistrationOrdinal":{"actor":order.next_actor_ordinal,"transient":order.next_transient_ordinal}}})
}
#[test]
fn sequential_enemy_and_goodie_selections_exclude_live_direct_ground_recipes() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/current-loot-ground-goldens.json"))
            .unwrap();
    assert_eq!(fixture["sourceRevision"], "fa08ae249ff5563570bdb2a653e2ef9358ce7b83");
    for case in fixture["cases"].as_array().unwrap() {
        let goodies = if case["mode"] == "goodies" {
            ["first", "second"]
                .into_iter()
                .map(|eid| BoneyardGoodieInput {
                    eid: eid.into(),
                    position: Vec2::new(100.0, 100.0),
                    reward_seed: Some(10),
                    scenery_registration_ordinal: None,
                    subtype: None,
                })
                .collect()
        } else {
            Vec::new()
        };
        let mut store = BoneyardLootStore::new(case["seed"].as_str().unwrap(), &goodies);
        let mut order = WorldPainterOrder::default();
        let items: Vec<_> = case["ground"]
            .as_array()
            .unwrap()
            .iter()
            .map(|index| {
                let index = index.as_u64().unwrap() as usize;
                create_equipment_inventory_item(&DOWSING_EQUIPMENT_RECIPES[index], index as u64 + 1)
            })
            .collect();
        store.spawn_custom_items(&items, Vec2::new(2000.0, 2000.0), 0, &mut order);
        let advanced = [false; 8];
        if case["mode"] == "goodies" {
            for goodie in &goodies {
                store.activate_goodie(&goodie.eid, "owner");
            }
            let participant = BoneyardLootParticipant {
                advanced_unlocks: &advanced,
                alive: true,
                connected: true,
                heading_index: 0,
                level: 31,
                modifiers: NativeLootModifiers::default(),
                owned_recipe_indexes: &[],
                player_id: "owner",
                position: Vec2::new(-10000.0, -10000.0),
            };
            for tick in 1..=250 {
                store.step(
                    std::slice::from_ref(&participant),
                    &NativeLootOpenPlacement,
                    tick,
                    &mut order,
                );
            }
            support_compare(&store, order, &case["frames"][0]["expected"], case, 0);
            continue;
        }
        for round in 0..2 {
            store.materialize_enemy_loot(
                NativeLootSelectionInput {
                    actor_seed: 77,
                    ground_recipe_indexes: &[],
                    item_recipe_pool: None,
                    arena: NativeLootArenaInput {
                        disable_mask: 0,
                        item_level_maximum: 100,
                        item_level_minimum: 0,
                        last_successful_item_level: -1,
                        level: 31,
                        mode: 0,
                        special_suppression: false,
                    },
                    explicit_gold_amount: None,
                    drop_delay_context: 0,
                    inventory_has_health_potion: false,
                    key: NativeLootKeyInput { current: 1, level: 31, remaining: 0 },
                    nearby_mask_two_count: 0,
                    participant: NativeLootParticipantInput {
                        advanced_unlocks: &advanced,
                        level: 31,
                        modifiers: NativeLootModifiers::default(),
                        owned_recipe_indexes: &[],
                        slot: 0,
                    },
                    placement: &NativeLootOpenPlacement,
                    policies: NativeLootPolicies {
                        item: NativeLootPolicy::Forced,
                        specific_item: 4,
                        ..NativeLootPolicies::DISABLED
                    },
                    scene_forces_health_potion: false,
                    source_position: Vec2::new(100.0, 200.0),
                    world_badguy_count: 100,
                    world_has_health_potion_sack: false,
                },
                if case["mode"] == "miniboss" {
                    Some(BoneyardLootDeathProgram::MinibossDie)
                } else {
                    None
                },
                5 + round as u64,
                &mut order,
            );
            support_compare(&store, order, &case["frames"][round]["expected"], case, round);
        }
    }
}
fn support_compare(
    store: &BoneyardLootStore,
    order: WorldPainterOrder,
    expected: &Value,
    case: &Value,
    round: usize,
) {
    native_bits::compare(
        &native_bits::source_value(&snapshot(store, order)),
        &native_bits::source_value(expected),
        &format!("{} {} round{round}", case["mode"], case["seed"]),
    );
}
