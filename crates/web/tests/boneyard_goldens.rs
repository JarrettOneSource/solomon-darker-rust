//! Offline Boneyard geometry and movement against the pinned Website narrow phase.

use serde_json::Value;
use solomon_assets::boneyard::{BoneyardCatalog, decode_scene};
use solomon_sim::Vec2;
use solomon_sim::boneyard::collision::{
    BoneyardCollisionWorld, first_boneyard_line_obstruction, first_boneyard_path_block_progress,
    resolve_boneyard_movement,
};
use solomon_sim::boneyard::{
    BoneyardWorld, arena::ArenaTransition, boneyard_camera_origin, gate::create_gate_leaves,
};

fn point(value: &Value) -> Vec2 {
    Vec2::new(value["x"].as_f64().unwrap(), value["y"].as_f64().unwrap())
}

#[test]
fn offline_boneyards_replay_the_website_geometry_movement_gates_and_entrance() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/boneyard-goldens.json"
    ))
    .unwrap();
    let catalog = BoneyardCatalog::load_embedded().unwrap();
    assert_eq!(catalog.templates.len(), 12, "all retained stock templates ship offline");
    assert_eq!(catalog.tutorial.scene.objects.len(), 92);

    for (case_index, case) in fixture["collisionCases"].as_array().unwrap().iter().enumerate() {
        let scene = decode_scene(&case["scene"].to_string()).unwrap();
        let world = BoneyardCollisionWorld::from_scene(&scene);
        for (index, circle) in world.circles().iter().enumerate() {
            let expected = &case["world"]["circles"][index];
            assert_eq!(circle.center, point(&expected["center"]), "circle in case {case_index}");
            assert_eq!(circle.radius, expected["radius"].as_f64().unwrap());
            assert_eq!(
                circle.native_line_mask,
                expected["nativeLineMask"].as_u64().unwrap_or(0) as u32
            );
            assert_eq!(circle.source_id.as_deref(), expected["sourceId"].as_str());
        }
        assert_eq!(world.circles().len(), case["world"]["circles"].as_array().unwrap().len());
        assert_eq!(world.polygons().len(), case["world"]["polygons"].as_array().unwrap().len());
        assert_eq!(world.segments().count(), case["world"]["segments"].as_array().unwrap().len());
        for (index, polygon) in world.polygons().iter().enumerate() {
            let expected: Vec<_> = case["world"]["polygons"][index]["points"]
                .as_array()
                .unwrap()
                .iter()
                .map(point)
                .collect();
            assert_eq!(polygon.points, expected, "polygon in case {case_index}");
        }
        for movement in case["movements"].as_array().unwrap() {
            let resolved = resolve_boneyard_movement(
                point(&movement["start"]),
                point(&movement["requested"]),
                scene.bounds,
                &world,
                movement["radius"].as_f64().unwrap(),
            );
            assert_eq!(resolved, point(&movement["position"]), "movement in case {case_index}");
        }
        for (index, segment) in world.segments().enumerate() {
            let expected = &case["world"]["segments"][index];
            assert_eq!(segment.start, point(&expected["start"]));
            assert_eq!(segment.end, point(&expected["end"]));
            assert_eq!(segment.radius, expected["radius"].as_f64().unwrap());
            assert_eq!(
                segment.native_line_mask,
                expected["nativeLineMask"].as_u64().unwrap_or(0) as u32
            );
        }
        for line in case["lines"].as_array().unwrap() {
            let actual = first_boneyard_line_obstruction(
                point(&line["start"]),
                point(&line["end"]),
                scene.bounds,
                &world,
                None,
                line["nativeExclusionMask"].as_u64().unwrap() as u32,
                &[],
            );
            let expected = if line["point"].is_null() { None } else { Some(point(&line["point"])) };
            assert_eq!(actual, expected, "line obstruction in case {case_index}");
        }
        for path in case["paths"].as_array().unwrap() {
            let ignored: Vec<&str> = path["ignoredSourceIds"]
                .as_array()
                .unwrap()
                .iter()
                .map(|id| id.as_str().unwrap())
                .collect();
            let actual = first_boneyard_path_block_progress(
                point(&path["start"]),
                point(&path["end"]),
                scene.bounds,
                &world,
                25.0,
                &ignored,
            );
            assert_eq!(actual, path["progress"].as_f64(), "path obstruction in case {case_index}");
        }
    }

    for walk in fixture["walks"].as_array().unwrap() {
        let template_index = walk["templateIndex"].as_u64().unwrap() as usize;
        let mut seed = [0_u8; 16];
        seed[..4].copy_from_slice(&(template_index as u32).to_be_bytes());
        let mut loaded = catalog.materialize(seed);
        loaded.seed = walk["seed"].as_str().unwrap().into();
        let mut world = BoneyardWorld::new(loaded);
        let mut wizard = world.spawn_wizard();
        let mut tick = 0;
        for stage in walk["stages"].as_array().unwrap() {
            let movement = Vec2::new(
                stage["movement"][0].as_f64().unwrap(),
                stage["movement"][1].as_f64().unwrap(),
            );
            for _ in 0..stage["ticks"].as_u64().unwrap() {
                world.step_movement_only(&mut wizard, movement);
                let expected: Vec<f64> =
                    serde_json::from_value(walk["ticks"][tick].clone()).unwrap();
                assert_eq!(
                    vec![
                        wizard.position.x,
                        wizard.position.y,
                        wizard.velocity.x,
                        wizard.velocity.y,
                        wizard.heading_index as f64,
                        wizard.gait_degrees,
                        wizard.walk_cycle_primary
                    ],
                    expected,
                    "template {template_index} diverged at tick {tick}",
                );
                tick += 1;
            }
        }
        assert_eq!(world.ticks, tick as u64);
    }

    let scene = decode_scene(&fixture["collisionCases"][33]["scene"].to_string()).unwrap();
    let mut leaves = create_gate_leaves(&scene.fences, fixture["seed"].as_str().unwrap());
    for (tick, expected) in fixture["gateTicks"].as_array().unwrap().iter().enumerate() {
        for (index, leaf) in leaves.iter_mut().enumerate() {
            if tick == 0 || tick == 70 {
                leaf.apply_contact(Vec2::new(1.0, if tick == 0 { 1.0 } else { -1.0 }));
            }
            leaf.step();
            let expected = &expected[index];
            let tip = point(&expected["tip"]);
            assert!(
                (leaf.tip.x - tip.x).abs() < 1e-9 && (leaf.tip.y - tip.y).abs() < 1e-9,
                "gate tip at tick {tick}"
            );
            assert_eq!(leaf.velocity, point(&expected["velocity"]));
            assert_eq!(leaf.damping, expected["damping"].as_f64().unwrap());
        }
    }
    assert_eq!(leaves.len(), 2);

    for case in fixture["arenaCases"].as_array().unwrap() {
        let mut state = ArenaTransition::new(
            solomon_assets::boneyard::decode_bounds(&case["bounds"].to_string()).unwrap(),
            point(&case["spawn"]),
        );
        for (index, expected) in case["ticks"].as_array().unwrap().iter().enumerate() {
            if index == 1 {
                state.start();
            }
            if index > 1 {
                state.step();
            }
            assert_eq!(state.camera_bounds.x, expected["cameraBounds"]["x"].as_f64().unwrap());
            assert_eq!(state.camera_bounds.y, expected["cameraBounds"]["y"].as_f64().unwrap());
            assert_eq!(state.camera_bounds.width, expected["cameraBounds"]["w"].as_f64().unwrap());
            assert_eq!(state.camera_bounds.height, expected["cameraBounds"]["h"].as_f64().unwrap());
            assert_eq!(state.blend_factor, expected["blendFactor"].as_f64().unwrap());
            assert_eq!(
                state.seal_ticks_remaining,
                expected["sealTicksRemaining"].as_u64().unwrap()
            );
        }
    }

    for case in fixture["cameraCases"].as_array().unwrap() {
        let bounds = solomon_assets::boneyard::decode_bounds(&case["bounds"].to_string()).unwrap();
        let view = Vec2::new(
            case["view"]["width"].as_f64().unwrap(),
            case["view"]["height"].as_f64().unwrap(),
        );
        assert_eq!(
            boneyard_camera_origin(
                point(&case["position"]),
                bounds,
                view,
                case["scale"].as_f64().unwrap()
            ),
            point(&case["origin"])
        );
    }
}

#[test]
fn beam_endpoints_clip_to_the_same_bounds_as_the_website() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/boneyard-goldens.json"
    ))
    .unwrap();
    for case in fixture["lineClipCases"].as_array().unwrap() {
        let bounds = solomon_assets::boneyard::decode_bounds(&case["bounds"].to_string()).unwrap();
        let actual = solomon_sim::line_obstruction::clip_line_to_bounds(
            point(&case["start"]),
            point(&case["end"]),
            bounds,
        );
        let expected = if case["clipped"].is_null() {
            None
        } else {
            Some((point(&case["clipped"]["start"]), point(&case["clipped"]["end"])))
        };
        assert_eq!(actual, expected);
    }
}

#[test]
fn the_run_seed_selects_the_same_stock_template_as_the_website() {
    let catalog = BoneyardCatalog::load_embedded().unwrap();
    let mut seed = [0_u8; 16];
    assert_eq!(catalog.materialize(seed).source_sha256, catalog.templates[0].source_sha256);
    seed[3] = 13;
    assert_eq!(catalog.materialize(seed).source_sha256, catalog.templates[1].source_sha256);
    seed[3] = 0;
    seed[0] = 1;
    // The first big-endian word is 2^24; its remainder modulo twelve is four.
    assert_eq!(catalog.materialize(seed).source_sha256, catalog.templates[4].source_sha256);
}
