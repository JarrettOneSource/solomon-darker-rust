//! Rust endpoint attachment and A* against every preparation shipped with the native catalog.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::Value;
use solomon_assets::boneyard::{BoneyardCatalog, decode_bounds};
use solomon_assets::navigation::decode_navigation;
use solomon_sim::Vec2;
use solomon_sim::boneyard::BoneyardBounds;
use solomon_sim::boneyard::arena::{ArenaPhase, ArenaTransition};
use solomon_sim::boneyard::collision::BoneyardCollisionWorld;
use solomon_sim::boneyard::collision::{
    CollisionPolygon, can_place_boneyard_body, first_boneyard_path_block_progress,
};
use solomon_sim::boneyard::navigation::{
    NavigationMesh, NavigationSet, NavigationTriangle, RouteRequest, find_boneyard_enemy_route,
};
use solomon_sim::enemies::{EnemySpawnIntent, EnemyToken, SpawnPositionPolicy};
use solomon_sim::profile::{NewGameSelection, WizardDiscipline, WizardElement};
use solomon_sim::session::{GameSession, GameSessionOptions, GameSessionTickContext};

fn point(value: &Value) -> Vec2 {
    Vec2::new(value["x"].as_f64().unwrap(), value["y"].as_f64().unwrap())
}

fn asset(source: &str) -> String {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../web/assets/navigation");
    std::fs::read_to_string(directory.join(format!("{source}.json"))).unwrap()
}

#[test]
fn native_boneyard_routes_preserve_recordings_except_verified_endpoint_corrections() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/navigation-goldens.json"
    ))
    .unwrap();
    let catalog = BoneyardCatalog::load_embedded().unwrap();
    let mut prepared = BTreeMap::new();
    for index in 0..=catalog.templates.len() {
        let mut seed = [0; 16];
        seed[..4].copy_from_slice(&(index as u32).to_be_bytes());
        let loaded = if index == catalog.templates.len() {
            catalog.materialize_tutorial(seed)
        } else {
            catalog.materialize(seed)
        };
        let navigation = decode_navigation(&asset(&loaded.source_sha256), &loaded).unwrap();
        let collision = BoneyardCollisionWorld::from_scene(&loaded.scene);
        prepared.insert(loaded.source_sha256, (navigation, collision));
    }
    assert_eq!(fixture["cases"].as_array().unwrap().len(), 63);
    for (index, case) in fixture["cases"].as_array().unwrap().iter().enumerate() {
        let (navigation, collision) = &prepared[case["sourceSha256"].as_str().unwrap()];
        let bounds = decode_bounds(&case["bounds"].to_string()).unwrap();
        let clearance = case["clearance"].as_f64().unwrap();
        for (query_index, query) in case["queries"].as_array().unwrap().iter().enumerate() {
            let ignored: Vec<&str> = query["ignoredSourceIds"]
                .as_array()
                .map_or_else(Vec::new, |ids| ids.iter().map(|id| id.as_str().unwrap()).collect());
            let route = navigation.find_route(
                RouteRequest {
                    body_radius: query["bodyRadius"].as_f64().unwrap(),
                    bounds,
                    clearance,
                    end: point(&query["end"]),
                    end_body_radius: query["endBodyRadius"].as_f64(),
                    ignored_source_ids: &ignored,
                    start: point(&query["start"]),
                },
                collision,
            );
            let expected: Option<Vec<Vec2>> =
                query["route"].as_array().map(|points| points.iter().map(point).collect());
            // These three historical null results selected disconnected first
            // attachments despite collision-safe connections to a shared component.
            // Retain the recording; assert the corrected behavior independently.
            if matches!((index, query_index), (31, 1) | (36, 2) | (38, 1)) {
                assert!(expected.is_none());
                let route = route.expect("the endpoint has a connected alternative");
                assert_eq!(route[0], point(&query["start"]));
                for pair in route.windows(2) {
                    assert_eq!(
                        first_boneyard_path_block_progress(
                            pair[0],
                            pair[1],
                            bounds,
                            collision,
                            query["bodyRadius"].as_f64().unwrap(),
                            &ignored,
                        ),
                        None,
                        "preparation {index} query {query_index} must remain collision-safe"
                    );
                }
            } else {
                assert_eq!(route, expected, "preparation {index} query {query_index}");
            }
        }
    }
}

#[test]
fn navigation_assets_reject_mismatched_identity_missing_preparations_and_invalid_references() {
    let loaded = BoneyardCatalog::load_embedded().unwrap().materialize([0; 16]);
    let json = asset(&loaded.source_sha256);
    let mut mismatched = loaded.clone();
    mismatched.geometry_sha256 = "0".repeat(64);
    assert!(decode_navigation(&json, &mismatched).is_err());
    let mut mismatched = loaded.clone();
    mismatched.source_sha256 = "0".repeat(64);
    assert!(decode_navigation(&json, &mismatched).is_err());
    let mut document: Value = serde_json::from_str(&json).unwrap();
    document["meshes"].as_array_mut().unwrap().pop();
    assert!(decode_navigation(&document.to_string(), &loaded).is_err());
    let mut document: Value = serde_json::from_str(&json).unwrap();
    document["meshes"][0]["mesh"]["triangles"][0]["vertices"][0] = 999_999.into();
    assert!(decode_navigation(&document.to_string(), &loaded).is_err());
}

#[test]
fn route_endpoints_can_attach_to_a_connected_component_beside_a_small_mesh_island() {
    let catalog = BoneyardCatalog::load_embedded().unwrap();
    let mut seed = [0; 16];
    seed[..4].copy_from_slice(&20_261_028_u32.to_be_bytes());
    let loaded = catalog.materialize(seed);
    assert_eq!(
        loaded.source_sha256,
        "2118053783606f5ef9dc848671d6eecd8e87aa0a3610c8c2119f08452e15a22f"
    );
    let preparations = [
        decode_navigation(&asset(&loaded.source_sha256), &loaded).unwrap(),
        NavigationSet::for_scene(&loaded),
    ];
    let collision = BoneyardCollisionWorld::from_scene(&loaded.scene);
    let bounds =
        ArenaTransition::new(loaded.scene.bounds, loaded.scene.spawn.position).combat_bounds;
    let origin = Vec2::new(276.52716064453125, 1089.437255859375);
    assert!(can_place_boneyard_body(origin, bounds, &collision, 35.0));
    for navigation in preparations {
        for target in
            [Vec2::new(1050.0, 1120.0), Vec2::new(1040.0, 1120.0), Vec2::new(1050.0, 1140.0)]
        {
            assert!(can_place_boneyard_body(target, bounds, &collision, 25.0));
            for (start, end, body_radius, end_body_radius) in
                [(origin, target, 35.0, 25.0), (target, origin, 25.0, 35.0)]
            {
                assert!(
                    navigation
                        .find_route(
                            RouteRequest {
                                start,
                                end,
                                bounds,
                                body_radius,
                                end_body_radius: Some(end_body_radius),
                                clearance: 50.0,
                                ignored_source_ids: &[],
                            },
                            &collision
                        )
                        .is_some(),
                    "a collision-safe alternate attachment connects {start:?} to {end:?}"
                );
            }
        }
    }
}

#[test]
fn both_endpoints_can_leave_isolated_first_choices_through_a_shared_component() {
    let bounds = BoneyardBounds::new(0.0, 0.0, 1000.0, 1000.0);
    let world = BoneyardCollisionWorld::from_primitives(
        vec![],
        vec![CollisionPolygon {
            points: vec![
                Vec2::new(400.0, 900.0),
                Vec2::new(400.0, 300.0),
                Vec2::new(600.0, 300.0),
                Vec2::new(600.0, 900.0),
            ],
            native_line_mask: 0,
            source_id: None,
        }],
        vec![],
    );
    for joined in [false, true] {
        let mesh = NavigationMesh {
            points: vec![
                Vec2::new(200.0, 650.0),
                Vec2::new(400.0, 650.0),
                Vec2::new(300.0, 850.0),
                Vec2::new(600.0, 650.0),
                Vec2::new(800.0, 650.0),
                Vec2::new(700.0, 850.0),
                Vec2::new(200.0, 50.0),
                Vec2::new(800.0, 50.0),
                Vec2::new(200.0, 200.0),
                Vec2::new(800.0, 200.0),
            ],
            triangles: vec![
                NavigationTriangle {
                    id: 0,
                    center: Vec2::new(300.0, 2150.0 / 3.0),
                    vertices: [0, 1, 2],
                    neighbors: vec![],
                },
                NavigationTriangle {
                    id: 1,
                    center: Vec2::new(700.0, 2150.0 / 3.0),
                    vertices: [3, 4, 5],
                    neighbors: vec![],
                },
                NavigationTriangle {
                    id: 2,
                    center: Vec2::new(400.0, 100.0),
                    vertices: [6, 7, 8],
                    neighbors: if joined { vec![3] } else { vec![] },
                },
                NavigationTriangle {
                    id: 3,
                    center: Vec2::new(600.0, 150.0),
                    vertices: [7, 9, 8],
                    neighbors: if joined { vec![2] } else { vec![] },
                },
            ],
        };
        let route = find_boneyard_enemy_route(
            RouteRequest {
                start: Vec2::new(300.0, 700.0),
                end: Vec2::new(700.0, 700.0),
                body_radius: 25.0,
                end_body_radius: None,
                clearance: 50.0,
                bounds,
                ignored_source_ids: &[],
            },
            &mesh,
            &world,
        );
        assert_eq!(
            route.is_some(),
            joined,
            "endpoint attachment must not invent a graph connection"
        );
        if let Some(route) = route {
            for pair in route.windows(2) {
                assert_eq!(
                    first_boneyard_path_block_progress(pair[0], pair[1], bounds, &world, 25.0, &[]),
                    None
                );
            }
        }
    }
}

#[test]
fn demon_birth_uses_a_connected_endpoint_when_the_wizard_is_beside_scenery() {
    let mut seed = [0; 16];
    seed[..4].copy_from_slice(&20_261_028_u32.to_be_bytes());
    let loaded = BoneyardCatalog::load_embedded().unwrap().materialize(seed);
    let navigation = decode_navigation(&asset(&loaded.source_sha256), &loaded).unwrap();
    let mut game = GameSession::new(
        "wizard".into(),
        NewGameSelection {
            name: "Wizard".into(),
            element: WizardElement::Fire,
            discipline: WizardDiscipline::Arcane,
        },
        GameSessionOptions::default(),
    );
    game.install_boneyard(loaded).unwrap();
    let world = game.boneyard.as_mut().unwrap();
    world.encounter = None;
    world.waves = None;
    world.arena_transition.as_mut().unwrap().phase = ArenaPhase::Sealed;
    game.participants.get_mut("wizard").unwrap().wizard.position = Vec2::new(1050.0, 1120.0);
    let origin = Vec2::new(276.52716064453125, 1089.437255859375);
    let mut birth = EnemySpawnIntent::new(EnemyToken::Demon, origin, 1, 0, 48);
    birth.position_policy = SpawnPositionPolicy::Dark;
    let result = game.step(
        &BTreeMap::new(),
        GameSessionTickContext {
            navigation: Some(&navigation),
            enemy_spawn_intents: &[birth],
            ..Default::default()
        },
    );
    assert!(result.advanced);
    let actors = &game.boneyard.as_ref().unwrap().enemies.actors;
    assert_eq!(actors.len(), 1);
    assert_eq!(actors.get(0).unwrap().position, origin);
}
