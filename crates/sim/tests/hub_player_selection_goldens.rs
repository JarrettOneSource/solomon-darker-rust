//! Source College actor hit targets, controller range and direct party-invite admission.

use serde_json::Value;
use solomon_sim::Vec2;
use solomon_sim::hub::regions::HubRegion;
use solomon_sim::social::{
    HubPlayerSelection, PartyMembership, PartyVisibility, hub_player_party_invite_available,
    nearest_hub_player, select_hub_player_at_point,
};

fn number(value: &Value) -> f64 {
    value.as_f64().unwrap_or_else(|| value.as_str().unwrap().parse().unwrap())
}

fn players(case: &Value) -> Vec<HubPlayerSelection<'_>> {
    case["players"]
        .as_array()
        .unwrap()
        .iter()
        .map(|player| HubPlayerSelection {
            player_id: player["id"].as_str().unwrap(),
            position: Vec2::new(number(&player["position"][0]), number(&player["position"][1])),
            region: player["region"].as_str().map(|region| HubRegion::from_id(region).unwrap()),
        })
        .collect()
}

#[test]
fn local_college_actor_selection_and_invitation_match_the_website() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/hub-player-selection-goldens.json"))
            .unwrap();
    for case in fixture["pointer"].as_array().unwrap() {
        let players = players(case);
        assert_eq!(
            select_hub_player_at_point(
                &players,
                case["local"].as_str().unwrap(),
                Vec2::new(number(&case["point"][0]), number(&case["point"][1])),
            ),
            case["expected"].as_str(),
            "{case}",
        );
    }
    for case in fixture["nearest"].as_array().unwrap() {
        let players = players(case);
        assert_eq!(
            nearest_hub_player(&players, case["local"].as_str().unwrap(), number(&case["maximum"])),
            case["expected"].as_str(),
            "{case}",
        );
    }
    for case in fixture["invites"].as_array().unwrap() {
        let party = PartyMembership {
            enhanced_effects: true,
            id: "actual-party".into(),
            join_code: None,
            leader_player_id: if case["leader"].as_bool().unwrap() { "local" } else { "other" }
                .into(),
            listing_id: None,
            member_player_ids: if case["alreadyTogether"].as_bool().unwrap() {
                vec!["local".into(), "bot-or-player".into()]
            } else {
                vec!["local".into()]
            },
            visibility: PartyVisibility::Public,
        };
        assert_eq!(
            hub_player_party_invite_available(
                case["partyPresent"].as_bool().unwrap().then_some(&party),
                "local",
                "bot-or-player",
            ),
            case["expected"].as_bool().unwrap(),
            "{case}",
        );
    }
}
