//! Native constructor placement through the source's actual complete-world callback boundary.

use std::cell::Cell;
use std::path::PathBuf;

use serde_json::Value;
use solomon_assets::boneyard::{BoneyardCatalog, decode_bounds};
use solomon_assets::navigation::decode_navigation;
use solomon_sim::boneyard::collision::BoneyardCollisionWorld;
use solomon_sim::boneyard::feedback::NativeEnemyWorldFeedback;
use solomon_sim::boneyard::gate::create_gate_leaves;
use solomon_sim::boneyard::projectile_world::BoneyardProjectileViewport;
use solomon_sim::boneyard::world_geometry::{BoneyardEnemyGeometry, BoneyardGeometryInput};
use solomon_sim::boneyard::world_light::{
    BoneyardWorldLightQuery, WorldLightPlayer, WorldLightScene,
};
use solomon_sim::enemies::config::EnemyArenaScalars;
use solomon_sim::enemies::model::{EnemyTarget, EnemyTerminalOutput};
use solomon_sim::enemies::world::{EnemyGeometry, EnemySpawnContext, EnemyWorld};
use solomon_sim::enemies::{EnemySpawnIntent, EnemyToken, SpawnPositionPolicy};
use solomon_sim::painter::{PainterRegistration, WorldManagerLane, WorldPainterOrder};
use solomon_sim::{NativeRng, Vec2};

fn point(value: &Value) -> Vec2 {
    Vec2::new(value["x"].as_f64().unwrap(), value["y"].as_f64().unwrap())
}

#[test]
fn actual_world_births_use_reachable_native_policy_placement_and_current_world_lights() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/world-geometry-goldens.json"
    ))
    .unwrap();
    let catalog = BoneyardCatalog::load_embedded().unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let index = case["templateIndex"].as_u64().unwrap() as usize;
        let mut seed = [0; 16];
        seed[..4].copy_from_slice(&(index as u32).to_be_bytes());
        let mut loaded = catalog.materialize(seed);
        loaded.seed = case["seed"].as_str().unwrap().into();
        let directory =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../web/assets/navigation");
        let json =
            std::fs::read_to_string(directory.join(format!("{}.json", loaded.source_sha256)))
                .unwrap();
        let navigation = decode_navigation(&json, &loaded).unwrap();
        let collision = BoneyardCollisionWorld::from_scene(&loaded.scene);
        let mut dynamic = collision.clone();
        let mut leaves = create_gate_leaves(&loaded.scene.fences, &loaded.seed);
        for leaf in &mut leaves {
            leaf.step();
        }
        dynamic.set_gate_leaves(&leaves);
        let player = &case["player"];
        let position = point(&player["position"]);
        let active_bounds = decode_bounds(&case["activeBounds"].to_string()).unwrap();
        let camera_bounds = decode_bounds(&case["cameraBounds"].to_string()).unwrap();
        let world_light = BoneyardWorldLightQuery::new(
            WorldLightScene {
                bounds: loaded.scene.bounds,
                camera_bounds,
                lantern_position: Some(loaded.scene.solomon_dig.as_ref().unwrap().lantern_position),
                lantern_light_registration: Some(PainterRegistration {
                    manager_lane: WorldManagerLane::Actor,
                    registration_ordinal: 5,
                }),
            },
            &[WorldLightPlayer {
                id: "local",
                position,
                heading_index: player["headingIndex"].as_u64().unwrap() as usize,
                weapon_pulse: 0.0,
                overlay_effect_phase: 0.0,
                light_registration: None,
                viewport: None,
            }],
            &[],
            &[],
            1,
        );
        let focuses = [position];
        let viewports = [BoneyardProjectileViewport {
            position,
            viewport: Vec2::new(1600.0, 900.0),
            alternate_player: false,
        }];
        let mut initial_feedback = NativeEnemyWorldFeedback::default();
        initial_feedback.step(1);
        let feedback = Cell::new(initial_feedback);
        let geometry = BoneyardEnemyGeometry::new(
            BoneyardGeometryInput {
                active_bounds,
                spawn_bounds: active_bounds,
                spawn_camera_bounds: active_bounds,
                static_collision: &collision,
                collision: &dynamic,
                navigation: &navigation,
                world_light: &world_light,
                spawn_policy_focuses: &focuses,
                spawn_domain: None,
                spawn_retry_bounds: None,
                projectile_viewports: &viewports,
                feedback: &feedback,
            },
            Vec::new(),
        );
        let target = EnemyTarget {
            id: "local".into(),
            alive: true,
            eligible: true,
            connected: true,
            summoned: false,
            collision_radius: 25.0,
            position,
            heading_deg: Some(player["headingIndex"].as_f64().unwrap() * 15.0),
            velocity_per_tick: Vec2::ZERO,
        };
        let intent = &case["intent"];
        let mut intent_row =
            EnemySpawnIntent::new(EnemyToken::Zombie, point(&intent["position"]), 1, 1, 0);
        intent_row.position_policy = match intent["positionPolicy"].as_str().unwrap() {
            "direct" => SpawnPositionPolicy::Direct,
            "dark" => SpawnPositionPolicy::Dark,
            "light" => SpawnPositionPolicy::Light,
            "offscreen" => SpawnPositionPolicy::Offscreen,
            value => panic!("unknown source world spawn policy {value}"),
        };
        let mut enemies = EnemyWorld::new(
            &loaded.seed,
            case["nativeRegistrationBase"].as_u64().unwrap() as usize,
        );
        let mut painters = WorldPainterOrder { next_actor_ordinal: 10, next_transient_ordinal: 0 };
        let input_rng = &case["sharedRngState"];
        let words: Vec<u32> = serde_json::from_value(input_rng["words"].clone()).unwrap();
        let mut shared_rng = NativeRng {
            index_a: input_rng["indexA"].as_u64().unwrap() as usize,
            index_b: input_rng["indexB"].as_u64().unwrap() as usize,
            words: words.try_into().unwrap(),
            divisor: 100_000,
        };
        let mut roll_loot_seed = |bound| shared_rng.draw_integer(bound as i32) as u32;
        let ids = enemies.spawn(
            &[intent_row],
            EnemySpawnContext {
                players: &[target],
                tick: 1,
                arena_scalars: EnemyArenaScalars::default(),
                roll_loot_seed: Some(&mut roll_loot_seed),
            },
            &geometry,
            Some(&mut painters),
        );
        assert_eq!(ids, [1]);
        let expected = &case["expected"];
        assert_eq!(
            enemies.actors.get(0).unwrap().position,
            point(&expected["position"]),
            "world {index} birth"
        );
        assert_eq!(enemies.rng_state, expected["actorRngState"].as_u64().unwrap() as u32);
        assert_eq!(
            enemies.actors.get(0).unwrap().loot_seed,
            expected["lootSeed"].as_u64().unwrap() as u32
        );
        assert_eq!(
            shared_rng.index_a,
            expected["sharedRngState"]["indexA"].as_u64().unwrap() as usize
        );
        assert_eq!(
            shared_rng.index_b,
            expected["sharedRngState"]["indexB"].as_u64().unwrap() as usize
        );
        let shared_words: Vec<u32> =
            serde_json::from_value(expected["sharedRngState"]["words"].clone()).unwrap();
        assert_eq!(shared_rng.words.as_slice(), shared_words);
        let rng = &expected["steeringRngState"];
        assert_eq!(enemies.steering_rng.index_a, rng["indexA"].as_u64().unwrap() as usize);
        assert_eq!(enemies.steering_rng.index_b, rng["indexB"].as_u64().unwrap() as usize);
        let words: Vec<u32> = serde_json::from_value(rng["words"].clone()).unwrap();
        assert_eq!(enemies.steering_rng.words.as_slice(), words);
        assert_eq!(
            painters.next_actor_ordinal,
            expected["painterOrder"]["nextRegistrationOrdinal"]["actor"].as_u64().unwrap() as usize
        );
        assert_eq!(
            painters.next_transient_ordinal,
            expected["painterOrder"]["nextRegistrationOrdinal"]["transient"].as_u64().unwrap()
                as usize
        );
        let native_bounds = geometry.native_view_bounds().unwrap();
        assert_eq!(native_bounds, decode_bounds(&case["nativeViewBounds"].to_string()).unwrap());
        for record in case["visibilityQueries"].as_array().unwrap() {
            let position = point(&record["position"]);
            let visibility = geometry.native_visibility(position);
            assert_eq!(visibility.admitted, record["native"]["admitted"].as_bool().unwrap());
            assert_eq!(visibility.intensity, record["native"]["intensity"].as_f64().unwrap());
            assert_eq!(
                geometry.projected_point_visible(position),
                record["projected"].as_bool().unwrap()
            );
        }
        for record in case["feedbackEvents"].as_array().unwrap() {
            if record["explosion"].is_object() {
                geometry.on_projectile_explosion(point(&record["explosion"]));
            } else {
                let output = match record["output"].as_str().unwrap() {
                    "wraith-fragments" => EnemyTerminalOutput::WraithFragments,
                    "imp-split" => EnemyTerminalOutput::ImpSplit,
                    "faculty-break" => EnemyTerminalOutput::FacultyBreak,
                    value => panic!("unknown retained feedback output {value}"),
                };
                geometry.on_terminal_output(
                    output,
                    record["count"].as_u64().map(|count| count as usize),
                );
            }
            assert_eq!(
                feedback.get().accumulator,
                record["expected"]["accumulator"].as_f64().unwrap()
            );
            assert_eq!(feedback.get().magnitude, record["expected"]["magnitude"].as_f64().unwrap());
        }
    }
}
