//! Actual host pause ownership and indefinite/countdown visual boundaries.
use serde_json::Value;
use solomon_sim::menu::shared::{
    GameplayPausePresentation, gameplay_pause_presentation, gameplay_resume_grace_progress,
};

#[test]
fn shared_hold_copy_and_visual_countdown_match_the_website() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/shared-gameplay-goldens.json"))
            .expect("shared gameplay fixture");
    for case in fixture["pauses"].as_array().expect("pause cases") {
        let state = &case["state"];
        let activity = match state["source"].as_str().expect("pause source") {
            "inventory" => Some("Inventory"),
            "skill-book" => Some("the Skill Book"),
            "skill-selector" => Some("the skill selector"),
            "pause-menu" => None,
            _ => panic!("source pause kind"),
        };
        let actual = gameplay_pause_presentation(
            state["ownerPlayerId"].as_str().expect("owner id"),
            case["localPlayerId"].as_str().expect("local id"),
            state["ownerDisplayName"].as_str().expect("owner name"),
            activity,
        );
        let expected = &case["presentation"];
        match actual {
            GameplayPausePresentation::Owner => assert_eq!(expected["kind"], "owner"),
            GameplayPausePresentation::Waiting { label, detail } => {
                assert_eq!(expected["kind"], "waiting");
                assert_eq!(label, expected["label"].as_str().expect("waiting label"));
                assert_eq!(detail, expected["detail"].as_str().expect("waiting detail"));
            }
        }
    }
    for case in fixture["progress"].as_array().expect("resume cases") {
        let actual = gameplay_resume_grace_progress(
            case["remainingMs"].as_f64(),
            case["elapsedMs"].as_f64().expect("visual elapsed time"),
        );
        assert_eq!(actual, case["progress"].as_f64());
    }
}
