//! Rust endpoint attachment and A* against every preparation shipped with the native catalog.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::Value;
use solomon_assets::boneyard::{BoneyardCatalog, decode_bounds};
use solomon_assets::navigation::decode_navigation;
use solomon_sim::Vec2;
use solomon_sim::boneyard::collision::BoneyardCollisionWorld;
use solomon_sim::boneyard::navigation::RouteRequest;

fn point(value: &Value) -> Vec2 {
    Vec2::new(value["x"].as_f64().unwrap(), value["y"].as_f64().unwrap())
}

fn asset(source: &str) -> String {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../web/assets/navigation");
    std::fs::read_to_string(directory.join(format!("{source}.json"))).unwrap()
}

#[test]
fn native_boneyard_route_queries_match_all_source_preparations_exactly() {
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
            assert_eq!(route, expected, "preparation {index} query {query_index}");
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
