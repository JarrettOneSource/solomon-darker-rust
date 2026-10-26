use serde_json::{Value, json};
use solomon_sim::painter::WorldPainterOrder;
use solomon_sim::profile::primary::{NativePrimarySkillProfileBase, NativeWeldPrimarySkillProfile};
use solomon_sim::profile::weld::{NativeWeldCastKind, NativeWeldPrimaryVector};
use solomon_sim::spells::targeting::{
    PrimarySpellQueryLane, PrimarySpellTarget, PrimarySpellTargetKind,
};
use solomon_sim::spells::weld::*;
use solomon_sim::{NativeRng, Vec2};

#[path = "support/website_rows.rs"]
mod website_rows;

#[path = "support/weld_visual_rows.rs"]
mod weld_visual_rows;

fn number(value: &Value, key: &str) -> f64 {
    value[key].as_f64().unwrap()
}
fn vector(value: &Value) -> Vec2 {
    Vec2::new(number(value, "x"), number(value, "y"))
}
fn point(value: Vec2) -> Value {
    json!({"x":value.x,"y":value.y})
}
fn random(rng: &NativeRng) -> Value {
    json!({"indexA":rng.index_a,"indexB":rng.index_b,"words":rng.words.as_slice()})
}
fn projectile_row(spell: &NativeWeldProjectileState) -> Value {
    let common = &spell.common;
    let lanes = spell.frost_presentation_lanes.map(|lanes| {
        lanes.map(|lane| {
            json!({
                "aspect":lane.aspect,"rotationDegrees":lane.rotation_degrees,"scale":lane.scale,
            })
        })
    });
    json!({
        "ageTicks":common.age_ticks,"ballLightningAcceleration":spell.ball_lightning_acceleration,
        "basePresentationPhaseDegrees":spell.base_presentation_phase_degrees,"buildId":spell.build_id,
        "castPlaybackRate":spell.cast_playback_rate,"castSoundVariant":spell.cast_sound_variant,
        "charge":common.charge,"contactsRemaining":spell.contacts_remaining,"damage":common.damage,
        "direction":point(common.direction),"flightTicks":common.flight_ticks,"headingDegrees":spell.heading_degrees,
        "hitTargetIds":spell.hit_target_ids,"id":common.id,"frostPulseAspect":spell.frost_pulse_aspect,
        "frostPresentationLanes":lanes,"frostTurnDegrees":spell.frost_turn_degrees,
        "groundSparkNativeAgeTicks":spell.ground_spark_native_age_ticks,"groundSparkTurnTicksRemaining":spell.ground_spark_turn_ticks_remaining,
        "kind":"weld","lightRegistration":{"managerLane":"actor","registrationOrdinal":common.registration.registration_ordinal},
        "ownerId":common.owner_id,"phase":"flight","position":point(common.position),"presentationSeed":spell.presentation_seed,
        "projectileIndex":spell.projectile_index,"reacquiresTarget":spell.reacquires_target,
        "secondaryPresentationPhaseDegrees":spell.secondary_presentation_phase_degrees,"speed":spell.speed,
        "targetId":spell.target_id,"turnAccumulator":spell.turn_accumulator,"turnInput":spell.turn_input,
        "underpowered":spell.underpowered,"vector":spell.vector,"velocity":point(common.velocity),"worldKey":common.world_key,
    })
}
fn profile(value: &Value) -> NativeWeldPrimarySkillProfile {
    let build_id = value["buildId"].as_u64().unwrap() as u32;
    NativeWeldPrimarySkillProfile {
        common: NativePrimarySkillProfileBase {
            damage_minimum: number(value, "damageMinimum"),
            damage_maximum: number(value, "damageMaximum"),
            damage_roll_count: number(value, "damageRollCount"),
            mana_cost: number(value, "manaCost"),
            rank: value["rank"].as_u64().unwrap() as u32,
            skill_id: value["skillId"].as_u64().unwrap() as u32,
        },
        build_id,
        cast_kind: NativeWeldCastKind::OneShot,
        damage_factor: number(value, "damageFactor"),
        vector: NativeWeldPrimaryVector {
            build_id,
            cast_kind: NativeWeldCastKind::OneShot,
            values: value["vector"]["values"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_f64().unwrap())
                .collect(),
        },
    }
}
fn targets(value: &Value) -> Vec<PrimarySpellTarget> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|target| PrimarySpellTarget {
            active: true,
            actor_flags: 2,
            attachment: Vec2::ZERO,
            body_radius: 15.0,
            cell_binding_order: target["cellBindingOrder"].as_u64().unwrap() as usize,
            heading_degrees: Some(0.0),
            grid_position: None,
            id: target["id"].as_str().unwrap().into(),
            kind: PrimarySpellTargetKind::Enemy,
            native_priority: 0,
            pending_remove: false,
            position: vector(&target["position"]),
            query_lane: PrimarySpellQueryLane::Grid,
            registration_order: target["registrationOrder"].as_u64().unwrap() as usize,
        })
        .collect()
}

#[test]
fn four_welded_projectile_builds_keep_birth_rng_tracking_and_private_turn_state() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/weld-projectile-goldens.json"))
            .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let input = &case["input"];
        let profile = profile(&input["primarySkill"]);
        let targets = targets(&input["targets"]);
        let mut rng = NativeRng::new(case["seed"].as_i64().unwrap());
        let mut order = WorldPainterOrder { next_actor_ordinal: 500, next_transient_ordinal: 70 };
        let mut projectiles = spawn_native_weld_one_shot(
            SpawnNativeWeldOneShotInput {
                aim_direction: vector(&input["aimDirection"]),
                first_id: 100,
                origin: vector(&input["origin"]),
                owner_id: "hero",
                primary_skill: &profile,
                targets: &targets,
                underpowered: input["underpowered"].as_bool().unwrap(),
                world_key: "boneyard:weld",
            },
            &mut rng,
            Some(&mut order),
        );
        for tick in 0..=120 {
            if tick > 0 {
                let mut live = targets.clone();
                for target in &mut live {
                    target.active = target.id != "enemy:1" || tick < 30;
                    target.position.x += tick as f64 / 10.0;
                    target.position.y -= tick as f64 / 10.0;
                }
                if tick >= 60 {
                    live.remove(0);
                }
                for projectile in &mut projectiles {
                    step_native_weld_projectile(&mut projectile.view_mut(), &live);
                    step_native_weld_projectile_presentation(&mut projectile.view_mut(), &mut rng);
                }
            }
            let Some(expected) =
                case["frames"].as_array().unwrap().iter().find(|sample| sample["tick"] == tick)
            else {
                continue;
            };
            let actual = json!({"tick":tick,"projectiles":projectiles.iter().map(projectile_row).collect::<Vec<_>>(),"rng":random(&rng)});
            website_rows::compare(&actual, expected, &format!("{} tick{tick}", case["name"]));
            if let Some(sample) = weld_visual_rows::sample(case, tick) {
                weld_visual_rows::compare(
                    projectiles
                        .iter()
                        .map(|spell| NativeWeldPresentationState::Projectile(spell.view())),
                    sample,
                    "Weld projectile compositor",
                );
            }
        }
        assert_eq!(order.next_actor_ordinal, case["order"]["actor"].as_u64().unwrap() as usize);
        for expected in case["groundSparkFades"].as_array().unwrap() {
            let native_age = expected["nativeAge"].as_u64().unwrap();
            projectiles[0].ground_spark_native_age_ticks = Some(native_age);
            let fades =
                create_native_weld_ground_spark_fade_program(projectiles[0].view(), &mut rng);
            let rows: Vec<Value> = fades.iter().map(|fade|json!({
                "alpha":fade.alpha,"alphaStep":fade.alpha_step,"position":point(fade.position),
                "record":fade.record,"rotationDegrees":fade.rotation_degrees,"scale":fade.scale,
            })).collect();
            website_rows::compare(
                &json!({"nativeAge":native_age,"fades":rows,"rng":random(&rng)}),
                expected,
                "GroundSpark native fade program",
            );
        }
    }
    for words in fixture["privateWords"].as_array().unwrap() {
        assert_eq!(
            native_ground_spark_private_word(words[0].as_u64().unwrap() as u32),
            words[1].as_u64().unwrap() as u32
        );
    }
    for expected in fixture["audioPlans"].as_array().unwrap() {
        let plan = native_weld_audio_plan(expected["buildId"].as_u64().unwrap() as u32);
        let actual = json!({"buildId":plan.build_id,"cue":plan.cue.as_str(),"loop":plan.looping,
            "nativeLoopIds":plan.native_loop_ids,"nativeSoundIds":plan.native_sound_ids,
            "nativeSoundVariantIds":plan.native_sound_variant_ids});
        website_rows::compare(&actual, expected, "Weld native audio ownership");
    }
}
