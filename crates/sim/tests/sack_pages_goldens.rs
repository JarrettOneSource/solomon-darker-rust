use serde_json::Value;
use solomon_sim::player_panels::sack_pages::*;

#[test]
fn sack_pages_keep_the_native_clip_and_every_transition_tick() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/sack-pages-goldens.json")).unwrap();
    assert_eq!(SACK_PAGE_HEIGHT, fixture["transition"]["pageHeight"].as_f64().unwrap());
    assert_eq!(u64::from(SACK_PAGE_TICKS), fixture["transition"]["ticks"].as_u64().unwrap());
    for (actual, field) in [
        (SACK_PAGE_CLIP.x, "x"),
        (SACK_PAGE_CLIP.y, "y"),
        (SACK_PAGE_CLIP.width, "width"),
        (SACK_PAGE_CLIP.height, "height"),
    ] {
        assert_eq!(actual, fixture["clip"][field].as_f64().unwrap());
    }
    for case in fixture["heights"].as_array().unwrap() {
        assert_eq!(
            native_inventory_sack_page_height(case["height"].as_f64().unwrap()),
            case["expected"].as_f64().unwrap(),
            "height {}",
            case["height"]
        );
    }
    for case in fixture["frames"].as_array().unwrap() {
        let direction = match case["direction"].as_str().unwrap() {
            "open" => SackPageDirection::Open,
            "back" => SackPageDirection::Back,
            _ => unreachable!(),
        };
        let actual = hub_sack_page_offsets(
            direction,
            case["startedAtMs"].as_f64().unwrap(),
            case["nowMs"].as_f64().unwrap(),
        );
        let expected = &case["expected"];
        assert_eq!(actual.incoming_y, expected["incomingY"].as_f64().unwrap(), "{case}");
        assert_eq!(actual.outgoing_y, expected["outgoingY"].as_f64().unwrap(), "{case}");
        assert_eq!(actual.settled, expected["settled"].as_bool().unwrap(), "{case}");
        assert_eq!(u64::from(actual.ticks), expected["ticks"].as_u64().unwrap(), "{case}");
    }
}
