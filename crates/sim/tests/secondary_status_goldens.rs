//! Ordered target effects and material tint replays from the unmodified secondary kernel.

use serde_json::{Value, json};
use solomon_sim::secondary::*;

fn electric(value: &Value) -> NativeSecondaryElectricBurnEffectState {
    NativeSecondaryElectricBurnEffectState {
        arc_count: value["arcCount"].as_u64().unwrap() as usize,
        damage_per_tick: value["damagePerTick"].as_f64().unwrap(),
        owner_id: value["ownerId"].as_str().unwrap().to_owned(),
        source_actor_id: value["sourceActorId"].as_u64().unwrap(),
        stun_factor: value["stunFactor"].as_f64().unwrap(),
        ticks: value["ticks"].as_u64().unwrap(),
    }
}
fn steamed(value: &Value) -> NativeSecondarySteamedEffectState {
    NativeSecondarySteamedEffectState {
        damage_per_tick: value["damagePerTick"].as_f64().unwrap(),
        ember_damage: value["emberDamage"].as_f64().unwrap(),
        ember_fragments: value["emberFragments"].as_u64().unwrap() as usize,
        explode_damage: value["explodeDamage"].as_f64().unwrap(),
        explode_radius: value["explodeRadius"].as_f64().unwrap(),
        owner_id: value["ownerId"].as_str().unwrap().to_owned(),
        source_actor_id: value["sourceActorId"].as_u64().unwrap(),
        ticks: value["ticks"].as_u64().unwrap(),
    }
}
fn patch(value: &Value) -> NativeSecondaryTargetEffectPatch {
    NativeSecondaryTargetEffectPatch {
        circle_slow_factor: value["circleSlowFactor"].as_f64(),
        circle_slow_ticks: value["circleSlowTicks"].as_u64(),
        cold_slow_factor: value["coldSlowFactor"].as_f64(),
        cold_slow_material: value["coldSlowMaterial"].as_bool(),
        cold_slow_ticks: value["coldSlowTicks"].as_u64(),
        dazzle_ticks: value["dazzleTicks"].as_u64(),
        disrupted_ticks: value["disruptedTicks"].as_u64(),
        electric_burn: value
            .get("electricBurn")
            .map(|value| if value.is_null() { None } else { Some(electric(value)) }),
        flee_ticks: value["fleeTicks"].as_u64(),
        frost_burn_damage_per_tick: value["frostBurnDamagePerTick"].as_f64(),
        frost_burn_owner_id: value["frostBurnOwnerId"].as_str().map(str::to_owned),
        frost_burn_skill_id: value["frostBurnSkillId"].as_u64().map(|id| id as usize),
        frost_burn_source_actor_id: value["frostBurnSourceActorId"].as_u64(),
        frost_burn_ticks: value["frostBurnTicks"].as_u64(),
        frozen_ticks: value["frozenTicks"].as_u64(),
        frozen_time_scale: value["frozenTimeScale"].as_f64(),
        prismatic_ticks: value["prismaticTicks"].as_u64(),
        stun_factor: value["stunFactor"].as_f64(),
        stun_ticks: value["stunTicks"].as_u64(),
        steamed: value
            .get("steamed")
            .map(|value| if value.is_null() { None } else { Some(steamed(value)) }),
        weaken_factor: value["weakenFactor"].as_f64(),
    }
}
fn snapshot(effect: &NativeSecondaryTargetEffectState) -> Value {
    let electric=effect.electric_burn.as_ref().map(|burn|json!({"arcCount":burn.arc_count,"damagePerTick":burn.damage_per_tick,
        "ownerId":burn.owner_id,"sourceActorId":burn.source_actor_id,"stunFactor":burn.stun_factor,"ticks":burn.ticks}));
    let steamed=effect.steamed.as_ref().map(|burn|json!({"damagePerTick":burn.damage_per_tick,"emberDamage":burn.ember_damage,
        "emberFragments":burn.ember_fragments,"explodeDamage":burn.explode_damage,"explodeRadius":burn.explode_radius,
        "ownerId":burn.owner_id,"sourceActorId":burn.source_actor_id,"ticks":burn.ticks}));
    let mut order = Vec::new();
    for kind in &effect.movement_modifier_order {
        order.push(match kind {
            NativeSecondaryMovementModifierKind::ColdSlow => "cold-slow",
            NativeSecondaryMovementModifierKind::CircleSlow => "circle-slow",
            NativeSecondaryMovementModifierKind::Frozen => "frozen",
            NativeSecondaryMovementModifierKind::Stun => "stun",
            NativeSecondaryMovementModifierKind::Dazzle => "dazzle",
        });
    }
    json!({"circleSlowFactor":effect.circle_slow_factor,"circleSlowTicks":effect.circle_slow_ticks,
        "coldSlowFactor":effect.cold_slow_factor,"coldSlowMaterial":effect.cold_slow_material,"coldSlowTicks":effect.cold_slow_ticks,
        "dazzleMaximumTicks":effect.dazzle_maximum_ticks,"dazzleTicks":effect.dazzle_ticks,"disruptedTicks":effect.disrupted_ticks,
        "electricBurn":electric,"fleeTicks":effect.flee_ticks,"frostBurnDamagePerTick":effect.frost_burn_damage_per_tick,
        "frostBurnOwnerId":effect.frost_burn_owner_id,"frostBurnSkillId":effect.frost_burn_skill_id,
        "frostBurnSourceActorId":effect.frost_burn_source_actor_id,"frostBurnTicks":effect.frost_burn_ticks,
        "frozenTicks":effect.frozen_ticks,"frozenTimeScale":effect.frozen_time_scale,"movementModifierOrder":order,
        "prismaticTicks":effect.prismatic_ticks,"stunFactor":effect.stun_factor,"stunTicks":effect.stun_ticks,
        "steamed":steamed,"targetId":effect.target_id,"timeScale":effect.time_scale,"weakenFactor":effect.weaken_factor,
        "worldKey":effect.world_key})
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
fn every_movement_order_burn_priority_thaw_and_material_tint_replays_the_website() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/secondary-status-goldens.json"))
            .unwrap();
    for journey in fixture["journeys"].as_array().unwrap() {
        let mut effects = Vec::new();
        for (index, record) in journey["records"].as_array().unwrap().iter().enumerate() {
            let action = &record["action"];
            match action["type"].as_str().unwrap() {
                "apply" => apply_native_secondary_target_effect(
                    &mut effects,
                    action["world"].as_str().unwrap_or("boneyard:oracle"),
                    action["target"].as_u64().unwrap_or(7) as usize,
                    patch(&action["patch"]),
                ),
                "dazzle" => apply_native_secondary_dazzle(
                    &mut effects,
                    "boneyard:oracle",
                    7,
                    action["ticks"].as_u64().unwrap(),
                ),
                "step" => {
                    for _ in 0..action["ticks"].as_u64().unwrap() {
                        effects.retain_mut(|effect| {
                            effect.step();
                            effect.is_active()
                        });
                    }
                }
                other => panic!("unknown secondary status action {other}"),
            }
            let mut actual = Vec::new();
            for effect in &effects {
                actual.push(snapshot(effect));
            }
            assert_snapshot(
                &json!(actual),
                &record["effects"],
                &format!("{} record {index}", journey["name"]),
            );
        }
    }
    for case in fixture["tints"].as_array().unwrap() {
        let mut effects = Vec::new();
        if !case["patch"].is_null() {
            apply_native_secondary_target_effect(
                &mut effects,
                "boneyard:oracle",
                7,
                patch(&case["patch"]),
            );
        }
        let effect = native_secondary_target_effect(&effects, "boneyard:oracle", 7);
        let actual = native_secondary_target_material_tint(
            case["worldTint"].as_u64().unwrap() as u32,
            effect,
        );
        assert_eq!(u64::from(actual), case["result"].as_u64().unwrap(), "{case}");
    }
}
