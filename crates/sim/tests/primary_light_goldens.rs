use serde_json::{Value, json};
use solomon_sim::Vec2;
use solomon_sim::boneyard::lighting::*;

#[path = "support/website_rows.rs"]
mod website_rows;

fn row(source: NativeBoneyardLightSource) -> Value {
    json!({
        "castsDirectionalShadow": source.casts_directional_shadow,
        "intensity": source.intensity,
        "position": { "x": source.position.x, "y": source.position.y },
        "radius": source.radius,
    })
}

#[test]
fn primary_light_birth_retirement_and_private_presentation_samples_match_the_source() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/primary-light-goldens.json")).unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let id = case["id"].as_u64().unwrap();
        let frame = case["frame"].as_u64().unwrap();
        let position = Vec2::new(
            case["position"]["x"].as_f64().unwrap(),
            case["position"]["y"].as_f64().unwrap(),
        );
        let shadows = case["multipleShadows"].as_bool().unwrap();
        let actual = json!([
            native_fire_explosion_light_source(
                position,
                frame,
                case["pointGain"].as_f64().unwrap(),
                shadows
            )
            .map(row),
            row(native_fire_ember_light_source(
                id,
                position,
                case["life"].as_f64().unwrap(),
                frame
            )),
            row(native_fire_impact_light_source(position, frame as f64)),
            row(native_fire_good_imp_light_source(
                id,
                position,
                case["lightGlow"].as_f64().unwrap(),
                frame
            )),
            row(native_fireball_light_source(id, position, frame, shadows)),
            row(ether_primary_impact_light_source(position, frame)),
        ]);
        website_rows::compare(&actual, &case["expected"], &format!("light{id} frame{frame}"));
    }
}
