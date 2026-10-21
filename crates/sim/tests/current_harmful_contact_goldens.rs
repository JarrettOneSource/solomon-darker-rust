//! Current rescue protection scales the received lanes after armor, without
//! skipping deflection or Harden draws from the accepted shared stream.

use serde_json::{Value, json};
use solomon_sim::economy::HubEconomyState;
use solomon_sim::harden_effects::*;
use solomon_sim::harmful::*;
use solomon_sim::player_rescue::NativePlayerRescueProtection;
use solomon_sim::profile::*;
use solomon_sim::{NativeRng, Vec2};

fn random_snapshot(rng: &NativeRng) -> Value {
    json!({"indexA":rng.index_a,"indexB":rng.index_b,"words":&rng.words[..]})
}

fn shard_snapshot(body: NativeHardenShardBody) -> Value {
    json!({"bounceVelocity":body.bounce_velocity,"enhancedShadow":body.enhanced_shadow,
        "height":body.height,"life":body.life,"position":body.position,"record":body.record,
        "rotationDegrees":body.rotation_degrees,"rotationStepDegrees":body.rotation_step_degrees,
        "velocity":body.velocity,"verticalVelocity":body.vertical_velocity})
}

fn effect_snapshot(effect: &NativeHardenEffect) -> Value {
    let (owner, kind, mut value) = match effect {
        NativeHardenEffect::Shard { owner, body } => (owner, "harden-shard", shard_snapshot(*body)),
        NativeHardenEffect::Burst { owner, alpha, position } => {
            (owner, "harden-burst", json!({"alpha":alpha,"position":position}))
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

fn assert_source(actual: &Value, expected: &Value, path: &str) {
    match (actual, expected) {
        (Value::Number(a), Value::Number(b)) => {
            assert_eq!(a.as_f64().unwrap().to_bits(), b.as_f64().unwrap().to_bits(), "{path}");
        }
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len(), "{path} length");
            for (index, (a, b)) in a.iter().zip(b).enumerate() {
                assert_source(a, b, &format!("{path}[{index}]"));
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(a.len(), b.len(), "{path} fields");
            for (key, expected) in b {
                assert_source(&a[key], expected, &format!("{path}.{key}"));
            }
        }
        _ => assert_eq!(actual, expected, "{path}"),
    }
}

#[test]
fn current_harmful_contacts_keep_rescue_and_harden_random_order() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/current-harmful-contact-goldens.json"))
            .unwrap();
    assert_eq!(fixture["sourceRevision"], "fa08ae249ff5563570bdb2a653e2ef9358ce7b83");
    let mut book = NativeSkillBookState::new(WizardElement::Ether, WizardDiscipline::Arcane);
    let economy = HubEconomyState::new(77, WizardElement::Ether);
    let base_runtime = PlayerSkillRuntimeState::new(&mut book, &economy);
    let progression = PlayerProgressionState::new(1);
    let base_stats = player_skill_derived_stats(&base_runtime, &book, &progression, &economy);
    for (index, case) in fixture["contacts"].as_array().unwrap().iter().enumerate() {
        let spec = &case["spec"];
        let mut runtime = base_runtime.clone();
        runtime.harden = NativeHardenState {
            armor: spec["armor"].as_f64().unwrap(),
            coating: spec["coating"].as_f64().unwrap(),
        };
        runtime.concentration_skill_id_a = spec["concentration"].as_u64().map(|id| id as usize);
        let mut stats = base_stats;
        stats.damage_resistance = 0.3;
        stats.magic_resistance = 0.2;
        stats.incoming_damage_factor = 1.25;
        stats.deflect_chance_percent = spec["deflect"].as_f64().unwrap();
        let protection = NativePlayerRescueProtection {
            fraction: case["fraction"].as_f64().unwrap(),
            ..NativePlayerRescueProtection::default()
        };
        let mut rng = NativeRng::new(case["seed"].as_i64().unwrap());
        let result = resolve_player_harmful_contact(
            PlayerHarmfulContactContext {
                runtime: &runtime,
                derived: &stats,
                progression: &progression,
                rescue_protection: &protection,
                reflection_source_in_range: true,
                position: Vec2::new(123.25, -54.5),
                enhanced_effects: case["enhancedEffects"].as_bool().unwrap(),
            },
            PlayerIncomingDamage {
                physical_damage: case["physicalDamage"].as_f64().unwrap(),
                magic_damage: case["magicDamage"].as_f64().unwrap(),
            },
            &mut rng,
        );
        assert_source(
            &json!({"physicalDamage":result.physical_damage,"magicDamage":result.magic_damage,
                "shieldDamage":result.shield_damage,"reflectedDamage":result.reflected_damage,
                "deflectPitch":result.deflect_pitch,"deflected":result.deflect_pitch.is_some(),
                "rng":random_snapshot(&rng)}),
            &case["damage"],
            &format!("current harmful contact {index}"),
        );
        let chip = result
            .harden_chip
            .map(|chip| json!({"pitch":chip.pitch,"shard":shard_snapshot(chip.shard)}));
        assert_source(&json!(chip), &case["hardenChip"], &format!("current chip {index}"));
    }
    for (index, case) in fixture["breakups"].as_array().unwrap().iter().enumerate() {
        let mut rng = NativeRng::new(case["seed"].as_i64().unwrap());
        let birth = create_native_harden_breakup(
            NativeHardenBreakupInput {
                coating: case["coating"].as_f64().unwrap(),
                position: Vec2::new(123.25, -54.5),
                owner_id: "player",
                world_key: "boneyard:current-contact",
                tick: 100,
                first_id: 7,
                enhanced_effects: case["enhancedEffects"].as_bool().unwrap(),
            },
            &mut rng,
        );
        let mut effects = birth.effects;
        assert_source(
            &json!({"effects":effects.iter().map(effect_snapshot).collect::<Vec<_>>(),
                "rng":random_snapshot(&rng),"nextId":birth.next_id,"pitch":birth.pitch}),
            &case["initial"],
            &format!("current Harden {index} birth"),
        );
        // The actual Source view keeps the inherited black shadow additive. It
        // shares the body's rotation and has no normal-blend darkening pass.
        let plan = native_harden_effect_plan(&effects[0]);
        let enhanced = case["enhancedEffects"].as_bool().unwrap();
        assert_eq!(plan.draws.len(), if enhanced { 2 } else { 1 });
        if enhanced {
            assert_eq!(plan.draws[0].tint, 0);
            assert_eq!(plan.draws[0].offset, Vec2::new(0.0, 2.0));
            assert_eq!(plan.draws[0].scale, Vec2::new(1.0, 0.75));
            assert_eq!(plan.draws[0].blend, solomon_sim::spells::presentation::SpellBlend::Add);
        }
        let mut samples = case["samples"].as_array().unwrap().iter().peekable();
        for tick in 101..=850 {
            effects.retain_mut(|effect| {
                step_native_harden_effect(effect, tick, &mut rng, |_| {
                    !case["blocked"].as_bool().unwrap()
                })
            });
            if samples.peek().is_some_and(|sample| sample["tick"].as_u64() == Some(tick)) {
                assert_source(
                    &json!({"tick":tick,"effects":effects.iter().map(effect_snapshot).collect::<Vec<_>>(),
                        "rng":random_snapshot(&rng)}),
                    samples.next().unwrap(),
                    &format!("current Harden {index} tick{tick}"),
                );
            }
        }
        assert!(samples.next().is_none());
    }
}
