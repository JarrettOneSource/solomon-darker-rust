//! Current cache units, native repeated-float fade and final Earth charge clamp.
#[path = "support/mod.rs"]
#[allow(dead_code)]
mod support;
use serde_json::{Value, json};
use solomon_sim::profile::offensive::NativeOffensiveSpellFactors;
use solomon_sim::profile::primary::{NativePrimarySkillProfile, native_primary_skill_profile};
use solomon_sim::profile::{NativeSkillBookState, WizardDiscipline, WizardElement};
use solomon_sim::spells::air_water::{
    create_native_water_aura_actor, native_water_aura_visual_plan,
};
use solomon_sim::spells::earth::{
    NativeEarthBoulderContactInput, consume_native_earth_boulder_contact,
};
use solomon_sim::spells::model::PrimarySpellTransientBase;
use solomon_sim::{NativeRng, Vec2};
fn fixture() -> Value {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/current-primary-caches-goldens.json"))
            .unwrap();
    assert_eq!(fixture["sourceRevision"], "fa08ae249ff5563570bdb2a653e2ef9358ce7b83");
    fixture
}
fn compare(actual: Value, expected: &Value, label: &str) {
    support::assert_website_json(&actual, expected, label, &[]);
}
#[test]
fn current_water_hail_and_earth_cache_units_match_source() {
    for case in fixture()["profiles"].as_array().unwrap() {
        let water = case["element"] == "water";
        let mut book = NativeSkillBookState::new(
            if water { WizardElement::Water } else { WizardElement::Earth },
            WizardDiscipline::Arcane,
        );
        for id in if water { &[37, 38][..] } else { &[47][..] } {
            book.effective_ranks[*id] = case["rank"].as_u64().unwrap() as u32;
        }
        let actual =
            match native_primary_skill_profile(&book, &NativeOffensiveSpellFactors::default()) {
                NativePrimarySkillProfile::Water(profile) => {
                    json!([profile.aura_radius_scale, profile.hail_threshold])
                }
                NativePrimarySkillProfile::Earth(profile) => json!([profile.maximum_charge]),
                _ => unreachable!(),
            };
        compare(actual, &case["expected"], &format!("{} rank{}", case["element"], case["rank"]));
    }
}
#[test]
fn cold_aura_constructor_rng_and_native_fade_stores_match_source() {
    for case in fixture()["auras"].as_array().unwrap() {
        let radius_scale =
            solomon_sim::cold_aura::native_cold_aura_radius_scale(case["feet"].as_f64().unwrap());
        compare(json!(radius_scale), &case["radiusScale"], "Cold Aura feet cache");
        compare(
            json!(solomon_sim::cold_aura::native_cold_aura_query_radius(radius_scale)),
            &case["queryRadius"],
            "Cold Aura query radius",
        );
        let mut rng = NativeRng::new(case["seed"].as_i64().unwrap());
        let mut actor = create_native_water_aura_actor(
            PrimarySpellTransientBase {
                age_ticks: 0,
                id: 1,
                light_registration: None,
                owner_id: "owner".into(),
                painter_registrations: Vec::new(),
                world_key: "boneyard:oracle".into(),
            },
            0,
            Vec2::new(10.0, 20.0),
            case["radiusScale"].as_f64().unwrap(),
            &mut rng,
        );
        compare(
            json!([
                actor.alpha_decay,
                actor.duration_ticks,
                actor.initial_rotation_degrees,
                actor.rotation_step_degrees
            ]),
            &case["fields"],
            "Cold Aura constructor",
        );
        compare(
            json!({"indexA":rng.index_a,"indexB":rng.index_b,"words":&rng.words[..]}),
            &case["rng"],
            "Cold Aura stream",
        );
        for frame in case["frames"].as_array().unwrap() {
            actor.common.age_ticks = frame["ageTicks"].as_f64().unwrap().max(0.0).trunc() as u64;
            let draw = native_water_aura_visual_plan(&actor);
            compare(
                json!({"alpha":draw.alpha,"rotationRadians":draw.rotation_radians,"scale":draw.scale,"tint":draw.tint}),
                &frame["visual"],
                "Cold Aura native stores",
            );
        }
    }
}
#[test]
fn earth_contact_never_rounds_above_its_release_charge() {
    for case in fixture()["pools"].as_array().unwrap() {
        let value = &case["input"];
        let result = consume_native_earth_boulder_contact(NativeEarthBoulderContactInput {
            release_base_damage: value["releaseBaseDamage"].as_f64().unwrap(),
            release_charge: value["releaseCharge"].as_f64().unwrap(),
            remaining_pool: value["remainingPool"].as_f64().unwrap(),
            target_health: value["targetHealth"].as_f64().unwrap(),
            toughness: value["toughness"].as_f64().unwrap(),
        });
        compare(
            json!({"charge":result.charge,"continueTraversal":result.continue_traversal,"damage":result.damage,
            "depleted":result.depleted,"remainingPool":result.remaining_pool}),
            &case["expected"],
            "Earth final clamp",
        );
    }
}
