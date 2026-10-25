//! Complete policy geometry blocks with actual placement and moving-gate primitives.

use serde_json::Value;
use solomon_sim::Vec2;
use solomon_sim::boneyard::BoneyardBounds;
use solomon_sim::boneyard::collision::{
    BoneyardCollisionWorld, CollisionCircle, CollisionPolygon, CollisionSegment,
};
use solomon_sim::ml_bot::observe_ml_bot_policy_geometry;

fn point(value: &Value) -> Vec2 {
    Vec2::new(value["x"].as_f64().unwrap(), value["y"].as_f64().unwrap())
}

#[test]
fn placement_rays_and_nearest_obstacle_rows_match_source() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/ml-bot-geometry-goldens.json"))
            .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let bounds = &case["bounds"];
        let bounds = BoneyardBounds::new(
            bounds["x"].as_f64().unwrap(),
            bounds["y"].as_f64().unwrap(),
            bounds["w"].as_f64().unwrap(),
            bounds["h"].as_f64().unwrap(),
        );
        let mut circles = Vec::new();
        let mut polygons = Vec::new();
        let mut segments = Vec::new();
        let source_id = |value: &Value| value["sourceId"].as_str().map(str::to_owned);
        for row in case["collision"]["circles"].as_array().unwrap() {
            circles.push(CollisionCircle {
                center: point(&row["center"]),
                radius: row["radius"].as_f64().unwrap(),
                native_line_mask: 0,
                source_id: source_id(row),
            });
        }
        for row in case["collision"]["segments"].as_array().unwrap() {
            segments.push(CollisionSegment {
                start: point(&row["start"]),
                end: point(&row["end"]),
                radius: row["radius"].as_f64().unwrap(),
                native_line_mask: 0,
                source_id: source_id(row),
            });
        }
        for row in case["collision"]["polygons"].as_array().unwrap() {
            polygons.push(CollisionPolygon {
                points: row["points"].as_array().unwrap().iter().map(point).collect(),
                native_line_mask: 0,
                source_id: source_id(row),
            });
        }
        let collision = BoneyardCollisionWorld::from_primitives(circles, polygons, segments);
        let destructible_ids = case["destructibleIds"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().to_owned())
            .collect::<Vec<_>>();
        let actual = observe_ml_bot_policy_geometry(
            bounds,
            &collision,
            &destructible_ids,
            point(&case["position"]),
        );
        for (key, values) in
            [("obstacles", actual.obstacles), ("patchAndRays", actual.patch_and_rays)]
        {
            let expected = case["output"][key].as_array().unwrap();
            assert_eq!(values.len(), expected.len());
            for (index, (actual, expected)) in values.iter().zip(expected).enumerate() {
                assert_eq!(
                    actual.to_bits(),
                    (expected.as_f64().unwrap() as f32).to_bits(),
                    "{} {:?} {key}[{index}]",
                    case["name"],
                    case["position"],
                );
            }
        }
    }
}
