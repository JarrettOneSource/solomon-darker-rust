//! The actual source portrait replacement sequence, including duplicate admission
//! and wrapping external portrait IDs. Serialization keeps the fixture's values.
#![cfg(feature = "persistence")]

use serde_json::Value;
use solomon_sim::hub::memorial::*;

#[test]
fn shared_college_memorial_replaces_oldest_slots_and_preserves_completed_run_identity() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/hub-memorial-goldens.json")).unwrap();
    let mut state = create_hub_memorial_state();
    assert_eq!(
        state,
        serde_json::from_value::<HubMemorialState>(fixture["initial"].clone()).unwrap()
    );
    for (index, step) in fixture["steps"].as_array().unwrap().iter().enumerate() {
        let mut portrait: HubMemorialPortrait =
            serde_json::from_value(step["input"].clone()).unwrap();
        let changed = archive_hub_memorial_portrait(
            &mut state,
            &portrait,
            step["markerDraw"].as_u64().unwrap() as u32,
        );
        assert_eq!(changed, step["changed"].as_bool().unwrap(), "archive {index}");
        // The archived portrait owns its capture; later live profile changes do
        // not repaint the College's completed-run record.
        portrait.config.name = "changed after capture".to_owned();
        // Compare the complete typed state: Source Number has no separate JSON
        // integer/fraction representation for captured tick metadata (900 vs900.0).
        assert_eq!(
            state,
            serde_json::from_value::<HubMemorialState>(step["state"].clone()).unwrap(),
            "archive {index}"
        );
    }
    let mut tie: HubMemorialState =
        serde_json::from_value(fixture["tie"]["before"].clone()).unwrap();
    let portrait: HubMemorialPortrait =
        serde_json::from_value(fixture["tie"]["input"].clone()).unwrap();
    assert!(archive_hub_memorial_portrait(&mut tie, &portrait, 3));
    assert_eq!(
        tie,
        serde_json::from_value::<HubMemorialState>(fixture["tie"]["state"].clone()).unwrap()
    );
    for case in fixture["interactions"].as_array().unwrap() {
        let expected = case["slot"].as_u64().map(|slot| slot as usize);
        assert_eq!(hub_memorial_slot_index_for_interaction(case["id"].as_str().unwrap()), expected);
    }
}
