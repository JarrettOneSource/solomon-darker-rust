use serde::Deserialize;
use serde_json::{Value, json};
use solomon_sim::observer_activity::{
    ObserverSkillPlayer, derive_observer_skill_events, observer_skill_offers,
};
use solomon_sim::profile::{PlayerSkillOffer, PlayerSkillOfferOption};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SourceOffer {
    automatic_choice_index: Option<usize>,
    level: u32,
    options: Vec<SourceOption>,
    sequence: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SourceOption {
    insight: bool,
    skill_id: usize,
    target_rank: u32,
    weld_build_id: Option<u32>,
}

struct Player {
    id: String,
    name: String,
    pending_offer: Option<PlayerSkillOffer>,
    learned_ranks: [u32; 83],
    deferred_choices: usize,
}

fn players(snapshot: &Value) -> Vec<Player> {
    // serde_json's map is sorted; the fixture records a separate source insertion order.
    let mut players = Vec::new();
    for id in snapshot["playerOrder"].as_array().unwrap() {
        let id = id.as_str().unwrap();
        let source = &snapshot["players"][id];
        let progression = &source["progression"];
        let pending_offer: Option<SourceOffer> =
            serde_json::from_value(progression["pendingOffer"].clone()).unwrap();
        let pending_offer = pending_offer.map(|offer| PlayerSkillOffer {
            automatic_choice_index: offer.automatic_choice_index,
            level: offer.level,
            options: offer
                .options
                .into_iter()
                .map(|option| PlayerSkillOfferOption {
                    insight: option.insight,
                    skill_id: option.skill_id,
                    target_rank: option.target_rank,
                    weld_build_id: option.weld_build_id,
                })
                .collect(),
            sequence: offer.sequence,
        });
        let mut learned_ranks = [0; 83];
        for rank in progression["learnedSkills"].as_array().unwrap() {
            learned_ranks[rank[0].as_u64().unwrap() as usize] = rank[1].as_u64().unwrap() as u32;
        }
        players.push(Player {
            id: id.to_owned(),
            name: source["config"]["displayName"].as_str().unwrap().to_owned(),
            pending_offer,
            learned_ranks,
            deferred_choices: progression["deferredSkillChoices"].as_u64().unwrap() as usize,
        });
    }
    players
}

fn borrowed(players: &[Player]) -> Vec<ObserverSkillPlayer<'_>> {
    players
        .iter()
        .map(|player| ObserverSkillPlayer {
            id: &player.id,
            name: &player.name,
            pending_offer: player.pending_offer.as_ref(),
            learned_ranks: &player.learned_ranks,
            deferred_choices: player.deferred_choices,
        })
        .collect()
}

#[test]
fn accepted_skill_choices_and_roster_order_match_the_source_observer() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/observer-activity-goldens.json"))
            .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let before = players(&case["before"]);
        let after = players(&case["after"]);
        let before = borrowed(&before);
        let after = borrowed(&after);
        let offers: Vec<_> = observer_skill_offers(&after)
            .into_iter()
            .map(|offer| {
                json!({"id": offer.id, "options": offer.options,
                    "playerName": offer.player_name, "title": offer.title})
            })
            .collect();
        let events: Vec<_> =
            derive_observer_skill_events(&before, &after, case["after"]["tick"].as_u64().unwrap())
                .into_iter()
                .map(|event| {
                    json!({"detail": event.detail, "id": event.id,
                        "playerName": event.player_name, "title": event.title})
                })
                .collect();
        assert_eq!(json!(offers), case["offers"], "offers {}", case["name"]);
        assert_eq!(json!(events), case["events"], "events {}", case["name"]);
    }
}
