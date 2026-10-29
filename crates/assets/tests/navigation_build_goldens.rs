//! Complete runtime graphs and source first-query cache ownership.
//!
//! The mesh fixture imports the unmodified Website triangulator. It includes accepted
//! non-stock scene documents, both native clearances, excluded collision primitives,
//! and the actual collision of a gate after 100 source contact/step transitions.

#[path = "../../sim/tests/support/mod.rs"]
mod support;

use std::sync::Arc;

use serde_json::{Value, json};
use solomon_assets::boneyard::{BoneyardCatalog, ModBoneyardEntry, decode_scene};
use solomon_assets::navigation::CatalogNavigation;
use solomon_sim::Vec2;
use solomon_sim::boneyard::collision::{
    BoneyardCollisionWorld, CollisionCircle, CollisionPolygon, CollisionSegment,
    can_place_boneyard_body, first_boneyard_path_block_progress,
};
use solomon_sim::boneyard::navigation::{
    NavigationMesh, NavigationSet, RouteRequest, boneyard_world_navigation_preparations,
};
use solomon_sim::boneyard::{
    BoneyardBounds, BoneyardChoice, BoneyardScene, BoneyardSource, LoadedBoneyard,
};

fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/navigation-build-goldens.json"
    ))
    .unwrap()
}

fn point(value: &Value) -> Vec2 {
    Vec2::new(value["x"].as_f64().unwrap(), value["y"].as_f64().unwrap())
}

fn point_json(point: Vec2) -> Value {
    json!({ "x": point.x, "y": point.y })
}

fn mesh_json(mesh: &NavigationMesh) -> Value {
    json!({
        "points": mesh.points.iter().copied().map(point_json).collect::<Vec<_>>(),
        "triangles": mesh.triangles.iter().map(|triangle| json!({
            "center": point_json(triangle.center), "id": triangle.id,
            "neighbors": triangle.neighbors, "vertices": triangle.vertices,
        })).collect::<Vec<_>>(),
    })
}

fn collision_world(scene: &BoneyardScene, value: &Value) -> BoneyardCollisionWorld {
    if value.is_null() {
        return BoneyardCollisionWorld::from_scene(scene);
    }
    let mask = |row: &Value| row["nativeLineMask"].as_u64().unwrap_or(0) as u32;
    let source_id = |row: &Value| row["sourceId"].as_str().map(str::to_owned);
    let circles = value["circles"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| CollisionCircle {
            center: point(&row["center"]),
            radius: row["radius"].as_f64().unwrap(),
            native_line_mask: mask(row),
            source_id: source_id(row),
        })
        .collect();
    let polygons = value["polygons"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| CollisionPolygon {
            points: row["points"].as_array().unwrap().iter().map(point).collect(),
            native_line_mask: mask(row),
            source_id: source_id(row),
        })
        .collect();
    let segments = value["segments"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| CollisionSegment {
            start: point(&row["start"]),
            end: point(&row["end"]),
            radius: row["radius"].as_f64().unwrap(),
            native_line_mask: mask(row),
            source_id: source_id(row),
        })
        .collect();
    BoneyardCollisionWorld::from_primitives(circles, polygons, segments)
}

fn find_route(
    navigation: &NavigationSet,
    world: &BoneyardCollisionWorld,
    bounds: BoneyardBounds,
    clearance: f64,
    query: &Value,
) -> Value {
    let route = navigation.find_route(
        RouteRequest {
            start: point(&query["start"]),
            end: point(&query["end"]),
            body_radius: query["bodyRadius"].as_f64().unwrap(),
            end_body_radius: query["endBodyRadius"].as_f64(),
            bounds,
            clearance,
            ignored_source_ids: &[],
        },
        world,
    );
    json!(route.map(|route| route.into_iter().map(point_json).collect::<Vec<_>>()))
}

fn empty_navigation() -> NavigationSet {
    NavigationSet::new("source fixture".into(), "geometry fixture".into(), Vec::new())
}

#[test]
fn accepted_scene_meshes_preserve_graphs_and_verified_route_corrections() {
    let fixture = fixture();
    for (index, case) in fixture["cases"].as_array().unwrap().iter().enumerate() {
        let scene = decode_scene(&case["scene"].to_string()).unwrap();
        let bounds = scene.bounds;
        let clearance = case["clearance"].as_f64().unwrap();
        let world = collision_world(&scene, &case["world"]);
        let navigation = if case["world"].is_null() {
            // Imported content is decoded through the actual scene boundary and
            // prepared from its geometry, without a stock source-hash lookup.
            NavigationSet::for_scene(&LoadedBoneyard {
                choice_id: "imported:source-fixture".into(),
                choice_source: BoneyardSource::Mod,
                mod_id: Some("source fixture package".into()),
                mod_name: Some("Source fixture".into()),
                name: scene.name.clone(),
                run_id: "source fixture run".into(),
                seed: "00112233445566778899aabbccddeeff".into(),
                source_sha256: format!("accepted scene {index}"),
                geometry_sha256: format!("accepted geometry {index}"),
                scene: scene.into(),
            })
        } else {
            let navigation = empty_navigation();
            navigation.prepare(bounds, &world, clearance);
            navigation
        };
        let prepared = navigation.prepared_for(bounds, clearance).unwrap();
        support::assert_website_json(
            &mesh_json(&prepared.mesh),
            &case["mesh"],
            &format!("runtime mesh {index}"),
            &[],
        );
        for (query_index, query) in case["queries"].as_array().unwrap().iter().enumerate() {
            let actual = find_route(&navigation, &world, bounds, clearance, query);
            if index == 2 && query_index == 0 {
                // This recorded null selected disconnected first attachments in
                // the imported overlapping-footprint scene. Retain the recording
                // and independently check the newly reachable route.
                assert_eq!(case["scene"]["name"], "overlapping authored footprints");
                assert_eq!(clearance, 25.0);
                assert!(query["route"].is_null());
                let radius = query["bodyRadius"].as_f64().unwrap();
                assert!(can_place_boneyard_body(point(&query["start"]), bounds, &world, radius));
                assert!(can_place_boneyard_body(point(&query["end"]), bounds, &world, radius));
                let route: Vec<_> = actual.as_array().unwrap().iter().map(point).collect();
                assert_eq!(route[0], point(&query["start"]));
                for pair in route.windows(2) {
                    assert_eq!(
                        first_boneyard_path_block_progress(
                            pair[0],
                            pair[1],
                            bounds,
                            &world,
                            radius,
                            &[],
                        ),
                        None,
                        "the corrected imported-scene route must remain collision-safe",
                    );
                }
                continue;
            }
            support::assert_website_json(
                &actual,
                &query["route"],
                &format!("runtime route {index}/{query_index}"),
                &[],
            );
        }
    }
    let catalog = BoneyardCatalog::load_embedded().unwrap();
    for case in fixture["preparations"].as_array().unwrap() {
        let mut loaded = if case["tutorial"] == true {
            catalog.materialize_tutorial([0; 16])
        } else {
            catalog.materialize([0; 16])
        };
        loaded.choice_source =
            if case["source"] == "mod" { BoneyardSource::Mod } else { BoneyardSource::Default };
        let preparations = boneyard_world_navigation_preparations(&loaded);
        let actual: Vec<_> = preparations
            .iter()
            .map(|(bounds, clearance)| json!({
                "bounds": { "x": bounds.x, "y": bounds.y, "w": bounds.width, "h": bounds.height },
                "clearance": clearance,
            }))
            .collect();
        support::assert_website_json(
            &json!(actual),
            &case["expected"],
            &format!("eager domains {} {}", case["source"], case["choiceId"]),
            &[],
        );
    }
}

#[test]
fn moving_gates_retain_the_first_mesh_only_within_the_same_geometry_owner() {
    let fixture = fixture();
    let case = &fixture["cache"];
    let scene = decode_scene(&case["scene"].to_string()).unwrap();
    let bounds = scene.bounds;
    let clearance = case["clearance"].as_f64().unwrap();
    let first_world = collision_world(&scene, &case["firstWorld"]);
    let moved_world = collision_world(&scene, &case["movedWorld"]);
    let navigation = empty_navigation();
    assert!(navigation.prepared_for(bounds, clearance).is_none());
    support::assert_website_json(
        &find_route(&navigation, &first_world, bounds, clearance, &case["directQuery"]),
        &case["directRoute"],
        "clear route before mesh construction",
        &[],
    );
    assert!(navigation.prepared_for(bounds, clearance).is_none());
    support::assert_website_json(
        &find_route(&navigation, &first_world, bounds, clearance, &case["blockedQuery"]),
        &case["firstRoute"],
        "first blocked gate route",
        &[],
    );
    let first = navigation.prepared_for(bounds, clearance).unwrap();
    support::assert_website_json(&mesh_json(&first.mesh), &case["firstMesh"], "first mesh", &[]);

    let cloned_navigation = navigation.clone();
    support::assert_website_json(
        &find_route(&cloned_navigation, &moved_world, bounds, clearance, &case["blockedQuery"]),
        &case["retainedRoute"],
        "same geometry after gate movement",
        &[],
    );
    let retained = cloned_navigation.prepared_for(bounds, clearance).unwrap();
    assert!(Arc::ptr_eq(&first, &retained));

    let fresh_navigation = empty_navigation();
    support::assert_website_json(
        &find_route(&fresh_navigation, &moved_world, bounds, clearance, &case["blockedQuery"]),
        &case["freshRoute"],
        "independent accepted geometry",
        &[],
    );
    let fresh = fresh_navigation.prepared_for(bounds, clearance).unwrap();
    support::assert_website_json(&mesh_json(&fresh.mesh), &case["movedMesh"], "fresh mesh", &[]);
    assert_ne!(first.mesh, fresh.mesh);
}

#[test]
fn catalog_materializations_retain_the_scene_owner_until_the_entry_is_replaced() {
    let fixture = fixture();
    let expected = &fixture["catalogIdentity"];
    let catalog = BoneyardCatalog::load_embedded().unwrap();
    let loaded = catalog.materialize_choice("default-random", [0; 16], "first").unwrap();
    let bounds = loaded.scene.bounds;
    let first = NavigationSet::for_scene(&loaded);
    let mut owners = CatalogNavigation::default();
    assert_eq!(first.prepared_for(bounds, 25.0).is_some(), expected["stockBefore"]);
    first.prepare(bounds, &BoneyardCollisionWorld::from_scene(&loaded.scene), 25.0);
    let original_mesh = first.prepared_for(bounds, 25.0).unwrap();
    owners.retain(&catalog, &loaded, first).unwrap();

    // Stock source templates are module-scoped even across source catalog objects.
    let second_catalog = BoneyardCatalog::load_embedded().unwrap();
    let repeated = second_catalog.materialize_choice("default-random", [0; 16], "second").unwrap();
    assert_eq!(loaded.scene == repeated.scene, expected["stockSameScene"]);
    let retained = owners.get(&second_catalog, &repeated).unwrap().unwrap();
    assert_eq!(retained.prepared_for(bounds, 25.0).is_some(), expected["stockRepeated"]);
    assert!(Arc::ptr_eq(&original_mesh, &retained.prepared_for(bounds, 25.0).unwrap()));
    let decoded = NavigationSet::for_scene(&loaded);
    assert_eq!(decoded.prepared_for(bounds, 25.0).is_some(), expected["stockDecoded"]);

    let entry = ModBoneyardEntry {
        choice: BoneyardChoice {
            id: "mod:fixture:gate".into(),
            name: "first query moving gate cache".into(),
            source: BoneyardSource::Mod,
            mod_id: Some("fixture".into()),
            mod_name: Some("Fixture".into()),
        },
        geometry_sha256: "fixture geometry".into(),
        scene: decode_scene(&fixture["cache"]["scene"].to_string()).unwrap(),
        source_sha256: "fixture source".into(),
        web_lua_content_id: None,
    };
    let catalog = catalog.with_mod_entries(vec![entry.clone()]).unwrap();
    let loaded = catalog.materialize_choice(&entry.choice.id, [0; 16], "mod first").unwrap();
    let bounds = loaded.scene.bounds;
    assert_eq!(owners.get(&catalog, &loaded).unwrap().is_some(), expected["modBefore"]);
    let first = NavigationSet::for_scene(&loaded);
    let original_mesh = first.prepared_for(bounds, 25.0).unwrap();
    owners.retain(&catalog, &loaded, first).unwrap();
    let repeated_catalog = catalog.clone();
    assert_eq!(
        Arc::ptr_eq(
            catalog.mod_entry(&entry.choice.id).unwrap(),
            repeated_catalog.mod_entry(&entry.choice.id).unwrap()
        ),
        expected["modSameScene"],
    );
    let repeated =
        repeated_catalog.materialize_choice(&entry.choice.id, [0; 16], "mod second").unwrap();
    let retained = owners.get(&repeated_catalog, &repeated).unwrap().unwrap();
    assert_eq!(retained.prepared_for(bounds, 25.0).is_some(), expected["modRepeated"]);
    assert!(Arc::ptr_eq(&original_mesh, &retained.prepared_for(bounds, 25.0).unwrap()));

    // Equal bytes in a newly accepted entry do not inherit a previous entry's graph.
    let replacement = catalog.with_mod_entries(vec![entry.clone()]).unwrap();
    let loaded = replacement.materialize_choice(&entry.choice.id, [0; 16], "replaced").unwrap();
    assert_eq!(owners.get(&replacement, &loaded).unwrap().is_some(), expected["modReplaced"]);
    let fresh = NavigationSet::for_scene(&loaded);
    owners.retain(&replacement, &loaded, fresh.clone()).unwrap();
    assert!(!Arc::ptr_eq(&original_mesh, &fresh.prepared_for(bounds, 25.0).unwrap()));
}
