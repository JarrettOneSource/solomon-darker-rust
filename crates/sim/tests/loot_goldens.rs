//! Complete loot construction against the unmodified Website kernels.

#![recursion_limit = "256"]

#[path = "loot_goldens/store.rs"]
mod store_cases;
mod support;

use serde_json::{Value, json};
use solomon_sim::economy::{EquipmentType, HubInventoryItem};
use solomon_sim::equipment::NativeEquipmentEffect;
use solomon_sim::equipment::random::generate_native_random_equipment_effects;
use solomon_sim::loot::items::{resolve_native_goodie_contents, select_enemy_item};
use solomon_sim::loot::*;
use solomon_sim::{NativeRng, Vec2};

#[test]
fn current_goodie_equipment_avoids_recipes_already_on_the_ground() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/loot-goldens.json")).unwrap();
    for (index, expected) in fixture["currentGoodies"].as_array().unwrap().iter().enumerate() {
        let input = &expected["input"];
        let mut rng = read_rng(&input["sharedRng"]);
        let mut ids = NativeLootItemIds::new(17);
        let result = resolve_native_goodie_contents(
            &mut rng,
            &mut ids,
            10,
            12,
            &flags(&input["advancedUnlocks"]),
            &indexes(&input["ownedRecipeIndexes"]),
            &indexes(&input["groundRecipeIndexes"]),
        );
        support::assert_website_json(
            &json!({"gold":result.gold,"items":result.items.iter().map(item_snapshot).collect::<Vec<_>>(),"sharedRng":rng_snapshot(&rng)}),
            &expected["expected"],
            &format!("currentGoodies[{index}]"),
            &[],
        );
        assert_eq!(ids.next_id, expected["nextItemId"].as_u64().unwrap());
    }
}

#[test]
fn current_named_enemy_items_and_original_faculty_pools_match_source() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/loot-goldens.json")).unwrap();
    for (index, expected) in fixture["currentItems"].as_array().unwrap().iter().enumerate() {
        let source = &expected["input"];
        let advanced = flags(&source["participant"]["advancedUnlocks"]);
        let owned = indexes(&source["participant"]["ownedRecipeIndexes"]);
        let ground = indexes(&source["groundRecipeIndexes"]);
        let pool = source["itemRecipePool"].as_array().map(|_| indexes(&source["itemRecipePool"]));
        let mut input = selection_input(source, &advanced, &owned, &NativeLootOpenPlacement);
        input.ground_recipe_indexes = &ground;
        input.item_recipe_pool = pool.as_deref();
        let mut rng = read_rng(&source["sharedRng"]);
        let mut ids = NativeLootItemIds::new(1);
        let item = select_enemy_item(&mut rng, &mut ids, &input, input.item_recipe_pool);
        support::assert_website_json(
            &json!({"item":item.as_ref().map(item_snapshot),"sharedRng":rng_snapshot(&rng)}),
            &expected["expected"],
            &format!("currentItems[{index}]"),
            &[],
        );
        assert_eq!(ids.next_id, expected["nextItemId"].as_u64().unwrap());
    }
    for (index, expected) in fixture["facultyPools"].as_array().unwrap().iter().enumerate() {
        let token = if expected["token"] == "DIREFACULTY" {
            solomon_sim::enemies::EnemyToken::DireFaculty
        } else {
            solomon_sim::enemies::EnemyToken::Skeleton
        };
        let pool = dire_boss_item_recipe_pool(
            expected["sourceSha256"].as_str(),
            token,
            expected["recipeUid"].as_u64().map(|uid| uid as usize),
        );
        support::assert_website_json(
            &json!(pool),
            &expected["expected"],
            &format!("facultyPools[{index}]"),
            &[],
        );
    }
}

fn rng_snapshot(rng: &NativeRng) -> Value {
    json!({"indexA":rng.index_a,"indexB":rng.index_b,"words":rng.words.as_slice()})
}

fn read_rng(value: &Value) -> NativeRng {
    let mut rng = NativeRng::new(1);
    rng.index_a = value["indexA"].as_u64().unwrap() as usize;
    rng.index_b = value["indexB"].as_u64().unwrap() as usize;
    for (word, source) in rng.words.iter_mut().zip(value["words"].as_array().unwrap()) {
        *word = source.as_u64().unwrap() as u32;
    }
    rng
}

fn effects_snapshot(effects: &[NativeEquipmentEffect]) -> Value {
    json!(
        effects
            .iter()
            .map(|effect| json!({"kind":effect.kind,"magnitude":effect.magnitude,
        "operator":effect.operator,"target":effect.target}))
            .collect::<Vec<_>>()
    )
}

fn item_snapshot(item: &HubInventoryItem) -> Value {
    let mut result = json!({
        "equipmentType":item.equipment_type.map(EquipmentType::as_str), "iconRecords":item.icon_records,
        "id":item.id,"kind":item.kind.as_str(),"name":item.name,"nativeSubtype":item.native_subtype,
        "nativeTypeId":item.native_type_id,"quantity":item.quantity,"rarity":item.rarity.map(|rarity| match rarity {
            solomon_sim::economy::ItemRarity::Epic => "Epic", solomon_sim::economy::ItemRarity::Rare => "Rare" }),
        "recipeIndex":item.recipe_index,
    });
    if let Some(contents) = &item.contents {
        result["contents"] = json!(contents.iter().map(item_snapshot).collect::<Vec<_>>());
    }
    if let Some(level) = item.generated_level {
        result["generatedLevel"] = json!(level);
    }
    if let Some(tints) = item.icon_tints {
        result["iconTints"] = json!(tints);
    }
    if let Some(slot) = item.inventory_slot {
        result["inventorySlot"] = json!(slot);
    }
    if let Some(selector) = item.native_selector {
        result["nativeSelector"] = json!(selector);
    }
    if let Some(effects) = &item.native_effects {
        result["nativeEffects"] = effects_snapshot(effects);
    }
    result
}

fn category_name(category: NativeLootCategory) -> &'static str {
    match category {
        NativeLootCategory::Key => "key",
        NativeLootCategory::Orb => "orb",
        NativeLootCategory::Gold => "gold",
        NativeLootCategory::Item => "item",
        NativeLootCategory::Potion => "potion",
        NativeLootCategory::Powerup => "powerup",
    }
}

fn drop_snapshot(drop: &NativeLootDropSpec) -> Value {
    let mut result = json!({"activationDelayTicks":drop.activation_delay_ticks,"id":drop.id,
        "kind":match drop.kind { NativeLootDropKind::Bonus => "bonus", NativeLootDropKind::Gold => "gold", NativeLootDropKind::Orb => "orb", NativeLootDropKind::Sack => "sack" },
        "nativeTypeId":drop.native_type_id,"phase":drop.phase,"position":{"x":drop.position.x,"y":drop.position.y},
        "source":match drop.source { NativeLootDropSource::Enemy => "enemy", NativeLootDropSource::Goodie => "goodie", NativeLootDropSource::Script => "script" }});
    if let Some(amount) = drop.amount {
        result["amount"] = json!(amount);
    }
    if let Some(bonus) = drop.bonus_kind {
        result["bonusKind"] = json!(match bonus {
            NativeBonusKind::SkillChoice => 0,
            NativeBonusKind::SkillRank => 1,
            NativeBonusKind::DamageX4 => 2,
        });
    }
    if let Some(item) = &drop.item {
        result["item"] = item_snapshot(item);
    }
    if let Some(orb) = drop.orb_kind {
        result["orbKind"] = json!(match orb {
            NativeOrbKind::Health => "health",
            NativeOrbKind::Mana => "mana",
        });
    }
    if let Some(rotation) = drop.rotation_deg {
        result["rotationDeg"] = json!(rotation);
    }
    if let Some(seed) = drop.scatter_seed {
        result["scatterSeed"] = json!(seed);
    }
    if let Some(tier) = drop.tier {
        result["tier"] = json!(tier);
    }
    if let Some(value) = drop.value {
        result["value"] = json!(value);
    }
    result
}

fn materialization_snapshot(value: &NativeLootMaterialization, rng: &NativeRng) -> Value {
    json!({"drops":value.drops.iter().map(drop_snapshot).collect::<Vec<_>>(),
        "lastSuccessfulItemLevel":value.last_successful_item_level,"nextKeyDropLevel":value.next_key_drop_level,
        "sharedRng":rng_snapshot(rng)})
}

fn flags(value: &Value) -> [bool; 8] {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|flag| flag.as_bool().unwrap())
        .collect::<Vec<_>>()
        .try_into()
        .unwrap()
}

fn indexes(value: &Value) -> Vec<usize> {
    value.as_array().unwrap().iter().map(|index| index.as_u64().unwrap() as usize).collect()
}

fn selection_input<'a>(
    value: &Value,
    advanced: &'a [bool; 8],
    owned: &'a [usize],
    placement: &'a dyn NativeLootPlacement,
) -> NativeLootSelectionInput<'a> {
    let arena = &value["arena"];
    let participant = &value["participant"];
    let modifiers = &participant["modifiers"];
    let policies = &value["policies"];
    let policy =
        |key: &str| NativeLootPolicy::from_id(policies[key].as_u64().unwrap() as u32).unwrap();
    NativeLootSelectionInput {
        actor_seed: value["actorSeed"].as_u64().unwrap() as u32,
        ground_recipe_indexes: &[],
        item_recipe_pool: None,
        arena: NativeLootArenaInput {
            disable_mask: arena["disableMask"].as_u64().unwrap() as u32,
            item_level_maximum: arena["itemLevelMaximum"].as_i64().unwrap() as i32,
            item_level_minimum: arena["itemLevelMinimum"].as_i64().unwrap() as i32,
            last_successful_item_level: arena["lastSuccessfulItemLevel"].as_i64().unwrap() as i32,
            level: arena["level"].as_i64().unwrap() as i32,
            mode: arena["mode"].as_i64().unwrap() as i32,
            special_suppression: arena["specialSuppression"].as_bool().unwrap(),
        },
        explicit_gold_amount: value["explicitGoldAmount"].as_u64(),
        drop_delay_context: value["dropDelayContext"].as_i64().unwrap(),
        inventory_has_health_potion: value["inventoryHasHealthPotion"].as_bool().unwrap(),
        key: NativeLootKeyInput {
            current: value["key"]["current"].as_u64().unwrap() as u32,
            level: value["key"]["level"].as_u64().unwrap() as u32,
            remaining: value["key"]["remaining"].as_u64().unwrap() as usize,
        },
        nearby_mask_two_count: value["nearbyMaskTwoCount"].as_u64().unwrap() as usize,
        participant: NativeLootParticipantInput {
            advanced_unlocks: advanced,
            level: participant["level"].as_u64().unwrap() as u32,
            modifiers: NativeLootModifiers {
                gold_amount: modifiers["goldAmount"].as_f64().unwrap(),
                gold_chance: modifiers["goldChance"].as_f64().unwrap(),
                item_chance: modifiers["itemChance"].as_f64().unwrap(),
                orb_chance: modifiers["orbChance"].as_f64().unwrap(),
                orb_pull: modifiers["orbPull"].as_f64().unwrap(),
                orb_value_bonus: modifiers["orbValueBonus"].as_bool().unwrap(),
                pickup_factor: modifiers["pickupFactor"].as_f64().unwrap(),
                powerup_chance: modifiers["powerupChance"].as_f64().unwrap(),
            },
            owned_recipe_indexes: owned,
            slot: participant["slot"].as_u64().unwrap() as usize,
        },
        placement,
        policies: NativeLootPolicies {
            gold: policy("gold"),
            item: policy("item"),
            orb: policy("orb"),
            potion: policy("potion"),
            powerup: policy("powerup"),
            specific_item: policies["specificItem"].as_u64().unwrap() as u32,
        },
        scene_forces_health_potion: value["sceneForcesHealthPotion"].as_bool().unwrap(),
        source_position: Vec2::new(
            value["sourcePosition"]["x"].as_f64().unwrap(),
            value["sourcePosition"]["y"].as_f64().unwrap(),
        ),
        world_badguy_count: value["worldBadguyCount"].as_u64().unwrap() as usize,
        world_has_health_potion_sack: value["worldHasHealthPotionSack"].as_bool().unwrap(),
    }
}

fn action(value: &Value) -> NativeLootScriptAction {
    match value["kind"].as_str().unwrap() {
        "drop-gold" => {
            NativeLootScriptAction::DropGold { amount: value["amount"].as_i64().unwrap() }
        }
        "drop-random-gold" => NativeLootScriptAction::DropRandomGold {
            minimum: value["minimum"].as_i64().unwrap() as i32,
            maximum: value["maximum"].as_i64().unwrap() as i32,
        },
        "drop-item" => NativeLootScriptAction::DropItem {
            recipe_index: value["recipeIndex"].as_u64().unwrap() as usize,
        },
        "drop-key" => NativeLootScriptAction::DropKey,
        "drop-potion" => NativeLootScriptAction::DropPotion {
            subtype: value["subtype"].as_u64().unwrap() as u32,
        },
        "drop-random-item" => {
            NativeLootScriptAction::DropRandomItem { mode: value["mode"].as_u64().unwrap() as u32 }
        }
        other => panic!("unknown script action {other}"),
    }
}

#[test]
fn loot_categories_items_goodies_and_random_equipment_replay_the_website() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/loot-goldens.json")).unwrap();
    for (index, row) in fixture["equipment"].as_array().unwrap().iter().enumerate() {
        let equipment_type = match row["type"].as_str().unwrap() {
            "hat" => EquipmentType::Hat,
            "robe" => EquipmentType::Robe,
            "staff" => EquipmentType::Staff,
            "wand" => EquipmentType::Wand,
            "ring" => EquipmentType::Ring,
            "amulet" => EquipmentType::Amulet,
            other => panic!("unknown equipment {other}"),
        };
        let mut rng = NativeRng::new(row["seed"].as_i64().unwrap());
        let actual = generate_native_random_equipment_effects(
            &mut rng,
            equipment_type,
            row["level"].as_i64().unwrap() as i32,
            &flags(&row["advancedUnlocks"]),
        );
        support::assert_website_json(
            &json!({"effects":effects_snapshot(&actual.effects),"itemLevel":actual.item_level,"name":actual.name,"sharedRng":rng_snapshot(&rng)}),
            &row["expected"],
            &format!("equipment[{index}]"),
            &[],
        );
    }
    for family in ["rolls", "scripts"] {
        for (index, row) in fixture[family].as_array().unwrap().iter().enumerate() {
            let value = &row["input"];
            let advanced = flags(&value["participant"]["advancedUnlocks"]);
            let owned = indexes(&value["participant"]["ownedRecipeIndexes"]);
            let placement = |point: Vec2, _: f64| {
                row["blocked"] != true
                    || point.x < 70.0
                    || point.x > 125.0
                    || point.y < 165.0
                    || point.y > 230.0
            };
            let input = selection_input(value, &advanced, &owned, &placement);
            let (category, private_rng) = select_native_loot_candidate(&input);
            let path = format!("{family}[{index}]");
            support::assert_website_json(
                &json!({"category":category.map(category_name),"privateRng":rng_snapshot(&private_rng)}),
                &row["selected"],
                &path,
                &[],
            );
            let mut rng = read_rng(&value["sharedRng"]);
            let mut ids = NativeLootItemIds::new(1);
            let actual = if family == "scripts" {
                materialization_snapshot(
                    &materialize_native_loot_script_action(
                        &input,
                        action(&row["action"]),
                        &mut rng,
                        &mut ids,
                    ),
                    &rng,
                )
            } else {
                let result = roll_native_enemy_loot(&input, &mut rng, &mut ids);
                let mut actual = materialization_snapshot(&result.materialization, &rng);
                actual["emergencyPotionAttempted"] = json!(result.emergency_potion_attempted);
                actual["selectedCategory"] = json!(result.selected_category.map(category_name));
                actual
            };
            support::assert_website_json(&actual, &row["expected"], &path, &[]);
            assert_eq!(ids.next_id, row["nextItemId"].as_u64().unwrap(), "{path} next item");
        }
    }
    for (index, row) in fixture["goodies"].as_array().unwrap().iter().enumerate() {
        let input = &row["input"];
        let mut rng = read_rng(&input["sharedRng"]);
        let mut ids = NativeLootItemIds::new(17);
        let result = resolve_native_goodie_contents(
            &mut rng,
            &mut ids,
            input["selector"].as_u64().unwrap() as u32,
            input["playerLevel"].as_u64().unwrap() as u32,
            &flags(&input["advancedUnlocks"]),
            &indexes(&input["ownedRecipeIndexes"]),
            &[],
        );
        support::assert_website_json(
            &json!({"gold":result.gold,"items":result.items.iter().map(item_snapshot).collect::<Vec<_>>(),"sharedRng":rng_snapshot(&rng)}),
            &row["expected"],
            &format!("goodies[{index}]"),
            &[],
        );
        assert_eq!(ids.next_id, row["nextItemId"].as_u64().unwrap());
    }
    for row in fixture["keys"].as_array().unwrap() {
        let mut rng = NativeRng::new(row["seed"].as_i64().unwrap());
        let level =
            advance_native_key_drop_level(&mut rng, row["current"].as_u64().unwrap() as u32);
        support::assert_website_json(
            &json!({"level":level,"sharedRng":rng_snapshot(&rng)}),
            &row["expected"],
            "key schedule",
            &[],
        );
    }
}
