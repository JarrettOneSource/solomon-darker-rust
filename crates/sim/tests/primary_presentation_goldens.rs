use serde_json::{Value, json};
use solomon_sim::painter::{PainterRegistration, WorldManagerLane};
use solomon_sim::spells::air::*;
use solomon_sim::spells::earth::EARTH_BOULDER_IDENTITY_ORIENTATION;
use solomon_sim::spells::earth_presentation::earth_boulder_sprite_plan;
use solomon_sim::spells::ether_presentation::*;
use solomon_sim::spells::fire::*;
use solomon_sim::spells::fire_presentation::*;
use solomon_sim::spells::model::*;
use solomon_sim::spells::presentation::*;
use solomon_sim::{NativeRng, Vec2};

#[path = "support/website_rows.rs"]
mod website_rows;

fn point(value: Vec2) -> [f64; 2] {
    [value.x, value.y]
}
fn read_point(value: &Value) -> Vec2 {
    Vec2::new(value["x"].as_f64().unwrap(), value["y"].as_f64().unwrap())
}
fn sprite_row(draw: NativeSpellSprite) -> Value {
    json!([
        draw.alpha,
        u32::from(draw.atlas == SpellAtlas::DeadHawg),
        u32::from(draw.blend == SpellBlend::Add),
        draw.offset.x,
        draw.offset.y,
        match draw.queue {
            SpellDrawQueue::World => 0,
            SpellDrawQueue::PreWorld => 1,
            SpellDrawQueue::PostWorld => 2,
        },
        draw.record,
        draw.rotation,
        draw.scale.x,
        draw.scale.y,
        draw.tint
    ])
}
fn plan_row(plan: NativeSpellSpritePlan) -> Value {
    let draws: Vec<Value> = plan.draws.into_iter().map(sprite_row).collect();
    json!({ "position": point(plan.position), "world_y": plan.world_y, "sort_bias": plan.sort_bias, "light": plan.region_light_point.map(point), "draws": draws })
}
fn mesh_row(mesh: &NativeAirMesh) -> Value {
    let vertices: Vec<[f64; 2]> = mesh.vertices.iter().copied().map(point).collect();
    let uvs: Vec<[f64; 2]> = mesh.uvs.iter().copied().map(point).collect();
    json!({ "indices": mesh.indices, "uvs": uvs, "vertices": vertices })
}
fn corona_row(corona: Option<NativeAirCoronaPlan>) -> Value {
    corona.map_or(Value::Null, |corona| {
        let draws: Vec<Value> = corona.draws.into_iter().map(sprite_row).collect();
        json!({ "alpha": corona.alpha, "center": point(corona.center), "draws": draws })
    })
}
fn light_row(light: NativeAirLightPlan) -> Value {
    json!([light.casts_directional_shadow, light.intensity, point(light.position), light.radius])
}

// These final compositor operations remain doubles in Pixi. The native float32
// ribbon geometry is compared separately without this libm rounding tolerance.
fn compare_draws(actual: &Value, expected: &Value, path: &str) {
    match (actual, expected) {
        (Value::Number(actual), Value::Number(expected)) => {
            let actual = actual.as_f64().unwrap();
            let expected = expected.as_f64().unwrap();
            assert!((actual - expected).abs() < 1e-9, "{path}: {actual} != {expected}");
        }
        (Value::Array(actual), Value::Array(expected)) => {
            assert_eq!(actual.len(), expected.len(), "{path}");
            for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
                compare_draws(actual, expected, &format!("{path}[{index}]"));
            }
        }
        (Value::Object(actual), Value::Object(expected)) => {
            assert_eq!(actual.len(), expected.len(), "{path}");
            for (key, expected) in expected {
                compare_draws(&actual[key], expected, &format!("{path}.{key}"));
            }
        }
        _ => assert_eq!(actual, expected, "{path}"),
    }
}

fn common(age: u64) -> PrimarySpellTransientBase {
    PrimarySpellTransientBase {
        age_ticks: age,
        id: 7,
        light_registration: None,
        owner_id: "view".into(),
        painter_registrations: Vec::new(),
        world_key: "boneyard".into(),
    }
}
fn projectile(age: u64) -> PrimarySpellProjectileBase {
    PrimarySpellProjectileBase {
        age_ticks: age,
        charge: 1.0,
        damage: 0.0,
        direction: Vec2::new(0.6, -0.8),
        flight_ticks: 0,
        id: 7,
        registration: PainterRegistration {
            manager_lane: WorldManagerLane::Actor,
            registration_ordinal: 49,
        },
        owner_id: "view".into(),
        phase: PrimarySpellProjectilePhase::Flight,
        position: Vec2::new(100.0, 200.0),
        velocity: Vec2::ZERO,
        world_key: "boneyard".into(),
    }
}

#[test]
fn stock_compositor_records_transforms_and_passes_match_the_website() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/primary-presentation-goldens.json"))
            .unwrap();
    for sample in fixture["sprites"].as_array().unwrap() {
        let kind = sample["kind"].as_str().unwrap();
        let age = sample["age"].as_u64().unwrap_or(0);
        let origin = Vec2::new(100.0, 200.0);
        let plan = match kind {
            "ether" => ether_primary_flight_plan(7, age as f64, 3.0, 0.75, age == 7),
            "ether-impact" => ether_primary_impact_plan(&PrimarySpellEtherImpact {
                common: common(age),
                birth_tick: 17,
                origin,
                visual_scale: 0.75,
            }),
            "earth" => {
                let charge = sample["charge"].as_f64().unwrap();
                let mut common = projectile(17);
                common.charge = charge;
                earth_boulder_sprite_plan(
                    &PrimarySpellEarthProjectile {
                        common,
                        assembly_charge: charge - 0.001,
                        hit_target_ids: Vec::new(),
                        maximum_charge: 2.0,
                        orientation: EARTH_BOULDER_IDENTITY_ORIENTATION,
                        remaining_damage: 0.0,
                        shell_charge: charge - 0.002,
                        toughness: 1.0,
                    },
                    17.0,
                )
            }
            "fireball" => native_fireball_plan(
                NativeFireballPresentationState {
                    age_ticks: age as f64,
                    direction: Vec2::new(0.6, -0.8),
                    id: 7,
                    position: origin,
                    underpowered: age == 7,
                },
                99,
            ),
            "fire-impact" => {
                native_fire_impact_plan(&PrimarySpellFireImpact { common: common(age), origin })
            }
            "fire-particle" => native_fire_particle_plan(&PrimarySpellFireParticle {
                enhanced_effects: true,
                common: common(age),
                direction: Vec2::new(0.6, -0.8),
                origin,
                variant: 3,
            }),
            "fire-explosion" => native_fire_explosion_plan(
                age as f64,
                origin,
                if age == 10 {
                    FireExplosionPresentation::Steam
                } else {
                    FireExplosionPresentation::Fire
                },
                1.9,
                0.7,
            ),
            "fire-ember" => native_fire_ember_plan(
                common(age).id,
                -20.0,
                if age == 0 { 2.0 } else { 0.4 },
                1.5,
                origin,
                99,
                true,
            ),
            "fire-patch" => native_fire_patch_plan(
                &PrimarySpellFirePatch {
                    common: common(10),
                    atlas_phase: sample["phase"].as_f64().unwrap(),
                    atlas_phase_step: 0.25,
                    burn_damage: 0.0,
                    damage: 0.0,
                    draw_alpha: 1.0,
                    fade_alpha: 0.7,
                    life: 1.5,
                    native_type: FirePatchType::Fire,
                    position: origin,
                    horizontal_sign: -1.0,
                    scale: 0.8,
                    supplemental_contact: false,
                    velocity: Vec2::ZERO,
                    velocity_multiplier: Vec2::new(1.0, 1.0),
                },
                99,
            ),
            "fire-good-imp" => {
                let mut imp = spawn_native_fire_good_imp(
                    SpawnNativeFireGoodImpOptions {
                        burn_damage: 10.0,
                        damage: 12.0,
                        id: 7,
                        lifetime_ticks: 300,
                        owner_id: "view".into(),
                        position: origin,
                        world_key: "boneyard".into(),
                    },
                    &mut NativeRng::new(7),
                );
                imp.heading_degrees = 253.4;
                imp.vertical_offset = -20.0;
                imp.effect_alpha = 0.7;
                imp.effect_phase = 9.2;
                imp.contact_age_ticks = Some(5);
                imp.contact_origin = Some(Vec2::new(105.0, 190.0));
                native_fire_good_imp_plan(&imp)
            }
            _ => panic!("unrecognized native presentation {kind}"),
        };
        compare_draws(&plan_row(plan), &sample["plan"], &format!("{kind}.age{age}"));
    }
}

#[test]
fn lightning_float32_ribbons_coronas_and_light_sources_match_the_website() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/primary-presentation-goldens.json"))
            .unwrap();
    for sample in fixture["airCases"].as_array().unwrap() {
        let input = &sample["input"];
        let plan = build_native_air_lightning_plan(NativeAirLightningInput {
            chained: false,
            enhanced_effects: true,
            age_ticks: input["ageTicks"].as_u64().unwrap(),
            birth_tick: 17,
            endpoint: read_point(&input["endpoint"]),
            id: 7,
            midpoint: read_point(&input["midpoint"]),
            underpowered: input["underpowered"].as_bool().unwrap(),
        });
        let body = plan.body.as_ref().map(|layers| layers.iter().map(|layer| {
            let branch = layer.branch.as_ref().map(|branch| json!({ "geometry_record": branch.geometry_record,
                "mirror_x": branch.mirror_x, "scale": branch.scale, "texture_record": branch.texture_record, "mesh": mesh_row(&branch.mesh) }));
            json!({ "alpha": layer.alpha, "mesh": mesh_row(&layer.mesh), "parameters": layer.parameter_samples,
                "phase": layer.phase_degrees, "offset": layer.phase_offset, "record": layer.texture_record,
                "tint": layer.tint, "width": layer.width, "branch": branch })
        }).collect::<Vec<_>>());
        if let Some(body) = &body {
            for (index, layer) in body.iter().enumerate() {
                website_rows::compare(
                    &layer["mesh"],
                    &sample["plan"]["body"][index]["mesh"],
                    "native ribbon float32 mesh",
                );
                website_rows::compare(
                    &layer["parameters"],
                    &sample["plan"]["body"][index]["parameters"],
                    "native ribbon parameter stores",
                );
                if !layer["branch"].is_null() {
                    website_rows::compare(
                        &layer["branch"]["mesh"],
                        &sample["plan"]["body"][index]["branch"]["mesh"],
                        "native branch float32 mesh",
                    );
                }
            }
        }
        let actual = json!({ "body": body, "source": corona_row(plan.source_corona),
            "contact": corona_row(Some(plan.contact_corona)), "light": plan.contact_light.map(light_row) });
        compare_draws(&actual, &sample["plan"], "Air plan");
    }
    for sample in fixture["pathLights"].as_array().unwrap() {
        let lights = build_native_air_path_light_sources(
            7,
            17,
            Vec2::new(100.0, 200.0),
            Vec2::new(400.0, 350.0),
            Vec2::new(700.0, 800.0),
            sample["weak"].as_bool().unwrap(),
            true,
        );
        let actual: Vec<Value> = lights.into_iter().map(light_row).collect();
        website_rows::compare(&json!(actual), &sample["lights"], "native Air path lights");
    }
}
