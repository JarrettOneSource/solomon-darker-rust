//! All authored equipment, complete sets and every native effect lane against the Website.

use serde_json::{Value, json};
use solomon_sim::equipment::{
    NATIVE_EQUIPMENT_RECIPES, NATIVE_EQUIPMENT_SETS, NativeEquipmentEffect,
    NativeEquipmentEffectSource, NativeEquipmentModifiers, NativeEquipmentScalarTransform,
    resolve_native_equipment_effects,
};

fn transform(value: NativeEquipmentScalarTransform) -> Value {
    json!({"offset":value.offset,"scale":value.scale})
}

fn modifiers(value: &NativeEquipmentModifiers) -> Value {
    json!({
        "castSpeedFlat":value.cast_speed_flat,"castSpeedMultiplier":value.cast_speed_multiplier,
        "classCastSpeedFlat":value.class_cast_speed_flat,"classCastSpeedMultiplier":value.class_cast_speed_multiplier,
        "classDamageFlat":value.class_damage_flat,"classDamageMultiplier":value.class_damage_multiplier,
        "classManaCostFlat":value.class_mana_cost_flat,"classManaCostMultiplier":value.class_mana_cost_multiplier,
        "classRecharge":value.class_recharge.map(transform),"damageResistance":value.damage_resistance,
        "featureBits":value.feature_bits,"globalDamageFlat":value.global_damage_flat,
        "globalDamageMultiplier":value.global_damage_multiplier,"globalManaCostFlat":value.global_mana_cost_flat,
        "globalManaCostMultiplier":value.global_mana_cost_multiplier,"goldMultiplier":value.gold_multiplier,
        "healthRecovery":transform(value.health_recovery),"magicResistance":value.magic_resistance,
        "manaRecovery":transform(value.mana_recovery),"maximumHealth":transform(value.maximum_health),
        "maximumMana":transform(value.maximum_mana),"meleeDamageFlat":value.melee_damage_flat,
        "meleeDamageMultiplier":value.melee_damage_multiplier,"orbPullMultiplier":value.orb_pull_multiplier,
        "poisonResistance":value.poison_resistance,"recharge":transform(value.recharge),
        "skillDamageFlat":&value.skill_damage_flat[..],"skillDamageMultiplier":&value.skill_damage_multiplier[..],
        "walkSpeed":transform(value.walk_speed),"weldEffect":value.weld_effect,
    })
}

fn assert_snapshot(actual: &Value, expected: &Value, path: &str) {
    match (actual, expected) {
        (Value::Number(actual), Value::Number(expected)) => {
            assert_eq!(actual.as_f64(), expected.as_f64(), "{path}")
        }
        (Value::Array(actual), Value::Array(expected)) => {
            assert_eq!(actual.len(), expected.len(), "{path} length");
            for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
                assert_snapshot(actual, expected, &format!("{path}[{index}]"));
            }
        }
        (Value::Object(actual), Value::Object(expected)) => {
            assert_eq!(actual.len(), expected.len(), "{path} fields");
            for (key, expected) in expected {
                assert_snapshot(&actual[key], expected, &format!("{path}.{key}"));
            }
        }
        _ => assert_eq!(actual, expected, "{path}"),
    }
}

#[test]
fn authored_equipment_sets_and_all_native_stat_lanes_replay_the_website() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/equipment-goldens.json")).unwrap();
    for (actual, expected) in
        NATIVE_EQUIPMENT_RECIPES.iter().zip(fixture["catalog"]["items"].as_array().unwrap())
    {
        assert_eq!(actual.name, expected["name"].as_str().unwrap());
        assert_eq!(actual.description, expected["description"].as_str().unwrap());
    }
    for (actual, expected) in
        NATIVE_EQUIPMENT_SETS.iter().zip(fixture["catalog"]["sets"].as_array().unwrap())
    {
        assert_eq!(actual.name, expected["name"].as_str().unwrap());
        assert_eq!(
            actual.member_recipe_indices,
            expected["memberRecipeIndices"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_u64().unwrap() as usize)
                .collect::<Vec<_>>()
        );
    }
    for case in fixture["cases"].as_array().unwrap() {
        let ranks: [u32; 83] = case["ranks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_u64().unwrap() as u32)
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();
        let sources: Vec<_> = case["sources"]
            .as_array()
            .unwrap()
            .iter()
            .map(|source| NativeEquipmentEffectSource {
                effects: source["effects"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|effect| NativeEquipmentEffect {
                        kind: effect["kind"].as_u64().unwrap() as u32,
                        magnitude: effect["magnitude"].as_f64().unwrap(),
                        operator: effect["operator"].as_u64().unwrap() as u32,
                        target: effect["target"].as_u64().unwrap() as u32,
                    })
                    .collect(),
                recipe_index: source["recipeIndex"].as_u64().map(|value| value as usize),
            })
            .collect();
        let owned: Vec<_> = case["owned"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_u64().unwrap() as u32)
            .collect();
        let actual = resolve_native_equipment_effects(&ranks, &sources, &owned);
        assert_snapshot(
            &json!({"effectiveRanks":&actual.effective_ranks[..],"grantedSkillOrder":actual.granted_skill_order,"modifiers":modifiers(&actual.modifiers)}),
            &case["result"],
            case["name"].as_str().unwrap(),
        );
    }
}
