use serde_json::Value;
use solomon_sim::Vec2;
use solomon_sim::player_panels::flyby::{hub_inventory_flyby_frame, hub_inventory_flyby_point};

#[test]
fn inventory_flyby_discrete_travel_and_independent_copy_lifetimes_match_current_website() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/inventory-flyby-goldens.json"))
            .unwrap();
    for case in fixture["frames"].as_array().unwrap() {
        let frame = hub_inventory_flyby_frame(1000.0, 1000.0 + case["elapsed"].as_f64().unwrap());
        let expected = &case["expected"];
        assert_eq!(frame.tick, expected["tick"].as_u64().unwrap());
        assert_eq!(frame.complete, expected["complete"].as_bool().unwrap());
        assert_eq!(frame.main_visible, expected["mainVisible"].as_bool().unwrap());
        assert_eq!(frame.travel_complete, expected["travelComplete"].as_bool().unwrap());
        assert_eq!(frame.main_progress, expected["mainProgress"].as_f64().unwrap());
        let copies = expected["afterimages"].as_array().unwrap();
        assert_eq!(frame.afterimages.len(), copies.len());
        for (copy, expected) in frame.afterimages.iter().zip(copies) {
            assert_eq!(copy.alpha, expected["alpha"].as_f64().unwrap());
            assert_eq!(copy.progress, expected["progress"].as_f64().unwrap());
            assert_eq!(copy.spawn_tick, expected["spawnTick"].as_u64().unwrap());
        }
    }
    for case in fixture["points"].as_array().unwrap() {
        let actual = hub_inventory_flyby_point(
            point(&case["from"]),
            point(&case["to"]),
            case["progress"].as_f64().unwrap(),
        );
        assert_eq!(actual, point(&case["expected"]));
    }
}

fn point(value: &Value) -> Vec2 {
    Vec2::new(value["x"].as_f64().unwrap(), value["y"].as_f64().unwrap())
}
