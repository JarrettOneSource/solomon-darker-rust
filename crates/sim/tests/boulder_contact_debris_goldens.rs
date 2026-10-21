use serde_json::{Value, json};
use solomon_sim::NativeRng;
use solomon_sim::spells::earth_debris::*;

#[path = "support/website_rows.rs"]
mod website_rows;
use website_rows::compare;

fn random_state(rng: &NativeRng) -> Value {
    json!({ "indexA": rng.index_a, "indexB": rng.index_b, "words": rng.words.as_slice() })
}
fn seed_row(seed: NativeWeldMeteorDebrisSeed) -> Value {
    json!({ "alpha": seed.alpha, "colorGreen": seed.color_green, "height": seed.height,
        "index": seed.index, "position": { "x": seed.position.x, "y": seed.position.y }, "record": seed.record,
        "rotationDegrees": seed.rotation_degrees, "rotationStepDegrees": seed.rotation_step_degrees,
        "scale": seed.scale, "velocity": { "x": seed.velocity.x, "y": seed.velocity.y }, "verticalVelocity": seed.vertical_velocity })
}
fn particle_row(particle: NativeWeldBoulderDebrisParticleState) -> Value {
    let mut row = seed_row(particle.common);
    row["bounceVelocity"] = json!(particle.bounce_velocity);
    row["enhancedShadow"] = json!(particle.enhanced_shadow);
    row
}

#[test]
fn boulder_contact_draws_bounces_and_fade_clock_replay_the_website() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/boulder-contact-debris-goldens.json"))
            .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let scale = case["scale"].as_f64().unwrap_or(f64::NAN);
        let mut born_rng = NativeRng::new(7);
        let born = create_native_weld_boulder_contact_debris_program(scale, &mut born_rng);
        compare(
            &json!({ "debris": [seed_row(born)], "rng": random_state(&born_rng) }),
            &case["born"],
            "native boulder contact factory",
        );
        for lifecycle in case["lifecycles"].as_array().unwrap() {
            let enhanced = lifecycle["enhanced"].as_bool().unwrap();
            let mut particle = Some(create_native_weld_boulder_debris_particle(born, enhanced));
            let mut rng = born_rng.clone();
            let mut age = 0;
            for frame in lifecycle["frames"].as_array().unwrap() {
                while age < frame["age"].as_u64().unwrap() {
                    age += 1;
                    particle = step_native_weld_boulder_debris_particle(
                        particle.unwrap(),
                        17 + age,
                        &mut rng,
                    );
                }
                compare(
                    &particle.map(particle_row).unwrap_or(Value::Null),
                    &frame["particle"],
                    &format!("boulder debris age{age}"),
                );
                compare(&random_state(&rng), &frame["rng"], "boulder debris bounce RNG");
            }
        }
    }
}
