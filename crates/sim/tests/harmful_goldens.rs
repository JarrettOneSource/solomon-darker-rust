//! Incoming damage, admitted Flash and complete Harden shard lifetimes against Website.

use serde_json::{Value, json};
use solomon_sim::economy::HubEconomyState;
use solomon_sim::harden_effects::*;
use solomon_sim::harmful::*;
use solomon_sim::profile::*;
use solomon_sim::{NativeRng, Vec2};

fn point(value: Vec2) -> Value {
    json!({"x":value.x,"y":value.y})
}
fn rng(value: &NativeRng) -> Value {
    json!({"indexA":value.index_a,"indexB":value.index_b,"words":&value.words[..]})
}
fn body(value: NativeHardenShardBody) -> Value {
    json!({"bounceVelocity":value.bounce_velocity,"height":value.height,"life":value.life,"position":point(value.position),"record":value.record,"rotationDegrees":value.rotation_degrees,"rotationStepDegrees":value.rotation_step_degrees,"velocity":point(value.velocity),"verticalVelocity":value.vertical_velocity})
}
fn chip(value: NativeHardenChip) -> Value {
    json!({"pitch":value.pitch,"shard":body(value.shard)})
}
fn flash(value: &PlayerFlashResponse) -> Value {
    json!({"cameraDisplacement":point(value.camera_displacement),"durationTicks":value.duration_ticks,"growScales":value.grow_scales,"pitch":value.pitch})
}
fn effect(value: &NativeHardenEffect) -> Value {
    let (owner, kind, mut value) = match value {
        NativeHardenEffect::Shard { owner, body: shard } => (owner, "harden-shard", body(*shard)),
        NativeHardenEffect::Burst { owner, alpha, position } => {
            (owner, "harden-burst", json!({"alpha":alpha,"position":point(*position)}))
        }
    };
    value["kind"] = json!(kind);
    value["ageTicks"] = json!(owner.common.age_ticks);
    value["birthTick"] = json!(owner.birth_tick);
    value["id"] = json!(owner.common.id);
    value["ownerId"] = json!(owner.common.owner_id);
    value["worldKey"] = json!(owner.common.world_key);
    value
}
fn assert_snapshot(actual: &Value, expected: &Value, label: &str) {
    match (actual, expected) {
        (Value::Number(a), Value::Number(b)) => assert_eq!(a.as_f64(), b.as_f64(), "{label}"),
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len(), "{label}");
            for (index, (a, b)) in a.iter().zip(b).enumerate() {
                assert_snapshot(a, b, &format!("{label}[{index}]"));
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(a.len(), b.len(), "{label}");
            for (key, b) in b {
                assert_snapshot(&a[key], b, &format!("{label}.{key}"));
            }
        }
        _ => assert_eq!(actual, expected, "{label}"),
    }
}

#[test]
fn harmful_contact_flash_and_harden_effects_replay_native_draw_order() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/harmful-goldens.json")).unwrap();
    let mut book = NativeSkillBookState::new(WizardElement::Ether, WizardDiscipline::Arcane);
    let economy = HubEconomyState::new(77, WizardElement::Ether);
    let base_runtime = PlayerSkillRuntimeState::new(&mut book, &economy);
    let base_progression = PlayerProgressionState::new(1);
    let base_stats = player_skill_derived_stats(&base_runtime, &book, &base_progression, &economy);
    let position = Vec2::new(123.25, -54.5);
    for (index, case) in fixture["contacts"].as_array().unwrap().iter().enumerate() {
        let spec = &case["spec"];
        let mut runtime = base_runtime.clone();
        let mut progression = base_progression.clone();
        let mut stats = base_stats;
        runtime.harden = NativeHardenState {
            armor: spec["armor"].as_f64().unwrap(),
            coating: spec["coating"].as_f64().unwrap(),
        };
        runtime.concentration_skill_id_a = spec["concentration"].as_u64().map(|id| id as usize);
        progression.mind_chug_ticks_remaining = spec["mindChug"].as_u64().unwrap();
        stats.damage_resistance = spec["resistance"].as_f64().unwrap();
        stats.magic_resistance = spec["magic"].as_f64().unwrap();
        stats.incoming_damage_factor = spec["incoming"].as_f64().unwrap();
        stats.deflect_chance_percent = spec["deflect"].as_f64().unwrap();
        let mut random = NativeRng::new(case["seed"].as_i64().unwrap());
        let result = resolve_player_harmful_contact(
            PlayerHarmfulContactContext {
                runtime: &runtime,
                derived: &stats,
                progression: &progression,
                rescue_protection:
                    &solomon_sim::player_rescue::NativePlayerRescueProtection::default(),
                reflection_source_in_range: case["inRange"].as_bool().unwrap(),
                position,
                enhanced_effects: true,
            },
            PlayerIncomingDamage {
                physical_damage: case["physicalDamage"].as_f64().unwrap(),
                magic_damage: case["magicDamage"].as_f64().unwrap(),
            },
            &mut random,
        );
        let actual = json!({"physicalDamage":result.physical_damage,"magicDamage":result.magic_damage,"shieldDamage":result.shield_damage,
            "reflectedDamage":result.reflected_damage,"hardenChip":result.harden_chip.map(chip),"deflectPitch":result.deflect_pitch,"deflected":result.deflect_pitch.is_some(),"rng":rng(&random)});
        assert_snapshot(&actual, &case["result"], &format!("contact {index}"));
    }
    for case in fixture["flashes"].as_array().unwrap() {
        let mut stats = base_stats;
        stats.flash_chance_percent = case["chance"].as_f64().unwrap();
        stats.flash_duration_ticks = 400;
        let mut random = NativeRng::new(case["seed"].as_i64().unwrap());
        let result = resolve_player_flash_response(&stats, &mut random);
        assert_snapshot(
            &json!({"flash":result.as_ref().map(flash),"rng":rng(&random)}),
            &case["result"],
            "Flash",
        );
    }
    for case in fixture["breakups"].as_array().unwrap() {
        let mut random = NativeRng::new(case["seed"].as_i64().unwrap());
        let result = create_native_harden_breakup(
            NativeHardenBreakupInput {
                coating: case["coating"].as_f64().unwrap(),
                position,
                owner_id: "player",
                world_key: "boneyard:oracle",
                tick: 100,
                first_id: 7,
                enhanced_effects: true,
            },
            &mut random,
        );
        let mut effects = result.effects;
        assert_snapshot(
            &json!({"effects":effects.iter().map(effect).collect::<Vec<_>>(),"rng":rng(&random),"nextId":result.next_id,"pitch":result.pitch}),
            &case["initial"],
            "Harden birth",
        );
        let blocked = case["blocked"].as_bool().unwrap();
        let mut samples = case["samples"].as_array().unwrap().iter().peekable();
        for tick in 101..=850 {
            effects.retain_mut(|effect| {
                step_native_harden_effect(effect, tick, &mut random, |_| !blocked)
            });
            if samples.peek().is_some_and(|sample| sample["tick"].as_u64() == Some(tick)) {
                let expected = samples.next().unwrap();
                assert_snapshot(
                    &json!({"tick":tick,"effects":effects.iter().map(effect).collect::<Vec<_>>(),"rng":rng(&random)}),
                    expected,
                    &format!("Harden {tick}"),
                );
            }
        }
    }
    for case in fixture["ranges"].as_array().unwrap() {
        assert_eq!(
            player_deflect_reflection_source_in_range(
                Vec2::ZERO,
                25.0,
                Vec2::new(case["x"].as_f64().unwrap(), 0.0),
                25.0
            ),
            case["result"].as_bool().unwrap()
        );
    }
}
