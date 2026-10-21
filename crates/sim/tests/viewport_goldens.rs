use serde_json::Value;
use solomon_sim::Vec2;
use solomon_sim::viewport::bounded_game_viewport;

#[test]
fn gameplay_viewport_matches_source_on_desktop_phone_and_bounded_rooms() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/viewport-goldens.json")).unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let layout = bounded_game_viewport(
            Vec2::new(case["size"][0].as_f64().unwrap(), case["size"][1].as_f64().unwrap()),
            Vec2::new(
                case["world"]["width"].as_f64().unwrap(),
                case["world"]["height"].as_f64().unwrap(),
            ),
            case["zoom"].as_f64().unwrap(),
        );
        assert_eq!(layout.display_scale, case["result"]["displayScale"].as_f64().unwrap());
        assert_eq!(
            layout.logical_size,
            Vec2::new(
                case["result"]["width"].as_f64().unwrap(),
                case["result"]["height"].as_f64().unwrap()
            )
        );
        assert_eq!(layout.world_zoom, case["result"]["worldZoom"].as_f64().unwrap());
    }
}
