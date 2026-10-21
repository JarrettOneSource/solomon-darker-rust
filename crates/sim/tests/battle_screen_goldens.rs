//! Screen transforms preserve the source's local-player anchor and signed edge cover.
use serde_json::Value;
use solomon_sim::Vec2;
use solomon_sim::boneyard::presentation::{
    native_arena_displacement_cover, native_enemy_world_feedback_transform,
};

fn point(value: &Value) -> Vec2 {
    Vec2::new(value["x"].as_f64().unwrap(), value["y"].as_f64().unwrap())
}
fn viewport(value: &Value) -> Vec2 {
    Vec2::new(value["width"].as_f64().unwrap(), value["height"].as_f64().unwrap())
}

#[test]
fn battle_camera_pulse_and_light_edge_cover_replay_the_source() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/battle-screen-goldens.json")).unwrap();
    for case in fixture["transforms"].as_array().unwrap() {
        let transform = native_enemy_world_feedback_transform(
            point(&case["camera"]),
            case["camera"]["zoom"].as_f64().unwrap(),
            viewport(&case["viewport"]),
            point(&case["player"]),
            case["magnitude"].as_f64().unwrap(),
        );
        assert_eq!(transform.position, point(&case["result"]["position"]));
        assert_eq!(transform.scale, case["result"]["scale"].as_f64().unwrap());
    }
    for case in fixture["covers"].as_array().unwrap() {
        let cover = native_arena_displacement_cover(
            point(&case["displacement"]),
            viewport(&case["viewport"]),
            case["complexLighting"].as_bool().unwrap(),
        );
        match cover {
            None => assert!(case["result"].is_null()),
            Some(cover) => {
                assert_eq!(cover.position, point(&case["result"]["position"]));
                for (rect, expected) in
                    cover.rectangles.iter().zip(case["result"]["rectangles"].as_array().unwrap())
                {
                    assert_eq!(Vec2::new(rect.x, rect.y), point(expected));
                    assert_eq!(Vec2::new(rect.width, rect.height), viewport(expected));
                }
            }
        }
    }
}
