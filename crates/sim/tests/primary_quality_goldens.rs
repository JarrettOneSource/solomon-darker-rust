//! Current Source quality retains birth fields and changes only its authored consumers.
use serde_json::{Value, json};
use solomon_sim::Vec2;
use solomon_sim::spells::air::*;
use solomon_sim::spells::fire::{
    native_fire_particle_fade_step, native_fire_particle_lifetime_ticks,
};
use solomon_sim::spells::fire_presentation::native_fire_particle_plan;
use solomon_sim::spells::model::{PrimarySpellFireParticle, PrimarySpellTransientBase};

#[path = "support/website_rows.rs"]
mod website_rows;

fn point(value: &Value) -> Vec2 {
    Vec2::new(value["x"].as_f64().unwrap(), value["y"].as_f64().unwrap())
}
fn light_row(light: NativeAirLightPlan) -> Value {
    json!({"castsDirectionalShadow":light.casts_directional_shadow,"intensity":light.intensity,
        "position":{"x":light.position.x,"y":light.position.y},"radius":light.radius})
}
fn mesh_row(layer: NativeAirRibbonLayer) -> Value {
    let flat = |points: Vec<Vec2>| {
        let mut result = Vec::with_capacity(points.len() * 2);
        for point in points {
            result.extend([point.x, point.y]);
        }
        result
    };
    json!({"alpha":layer.alpha,"indices":layer.mesh.indices,"parameters":layer.parameter_samples,
        "phase":layer.phase_degrees,"offset":layer.phase_offset,"record":layer.texture_record,
        "tint":layer.tint,"uvs":flat(layer.mesh.uvs),"vertices":flat(layer.mesh.vertices),"width":layer.width})
}

#[test]
fn current_air_and_fire_quality_selects_native_lifetimes_meshes_and_lights() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/primary-quality-goldens.json"))
            .unwrap();
    for case in fixture["airCases"].as_array().unwrap() {
        let input = &case["input"];
        let underpowered = input["underpowered"].as_bool().unwrap();
        let chained = input["chained"].as_bool().unwrap();
        let enhanced_effects = input["enhancedEffects"].as_bool().unwrap();
        assert_eq!(
            native_air_contact_lifetime_ticks(underpowered, chained, enhanced_effects),
            case["lifetime"].as_u64().unwrap()
        );
        for contact in case["contacts"].as_array().unwrap() {
            let age = contact["age"].as_u64().unwrap();
            let plan = build_native_air_lightning_plan(NativeAirLightningInput {
                chained,
                enhanced_effects,
                age_ticks: age,
                birth_tick: 17,
                endpoint: point(&input["endpoint"]),
                id: 7,
                midpoint: point(&input["midpoint"]),
                underpowered,
            });
            assert_eq!(plan.contact_corona.alpha, contact["alpha"].as_f64().unwrap());
            website_rows::compare(
                &plan.contact_light.map(light_row).unwrap_or(Value::Null),
                &contact["light"],
                "Air quality contact light",
            );
            if age == 0 && !chained {
                let body: Vec<_> = plan.body.unwrap().into_iter().map(mesh_row).collect();
                website_rows::compare(&json!(body), &case["body"], "Air quality body");
            }
        }
    }
    for case in fixture["paths"].as_array().unwrap() {
        let input = &case["input"];
        let lights: Vec<_> = build_native_air_path_light_sources(
            7,
            17,
            point(&input["origin"]),
            point(&input["midpoint"]),
            point(&input["endpoint"]),
            input["weakCast"].as_bool().unwrap(),
            input["enhancedEffects"].as_bool().unwrap(),
        )
        .into_iter()
        .map(light_row)
        .collect();
        website_rows::compare(&json!(lights), &case["lights"], "Air quality path shadows");
    }
    for case in fixture["fireCases"].as_array().unwrap() {
        let input = &case["state"];
        let id = input["id"].as_u64().unwrap();
        let enhanced_effects = input["enhancedEffects"].as_bool().unwrap();
        assert_eq!(
            native_fire_particle_fade_step(id, enhanced_effects),
            case["fade"].as_f64().unwrap()
        );
        assert_eq!(
            native_fire_particle_lifetime_ticks(id, enhanced_effects),
            case["lifetime"].as_u64().unwrap()
        );
        let plan = native_fire_particle_plan(&PrimarySpellFireParticle {
            common: PrimarySpellTransientBase {
                age_ticks: input["ageTicks"].as_u64().unwrap(),
                id,
                light_registration: None,
                owner_id: "hero".into(),
                painter_registrations: Vec::new(),
                world_key: "quality".into(),
            },
            enhanced_effects,
            direction: point(&input["direction"]),
            origin: point(&input["origin"]),
            variant: 3,
        });
        let draw = &plan.draws[0];
        let actual = json!({"alpha":draw.alpha,"fadeStep":native_fire_particle_fade_step(id, enhanced_effects),
            "frame":draw.record,"position":{"x":plan.position.x,"y":plan.position.y},
            "regionLightPoint":Value::Null,"rotation":draw.rotation,"scale":draw.scale.x,"tint":draw.tint,"worldY":plan.world_y});
        website_rows::compare(&actual, &case["plan"], "Fire quality fade and sprite");
    }
}
