//! Actual inventory book transactions publish current choice/rank receipts.

mod support;

#[path = "support/player_snapshot.rs"]
mod player_snapshot;

use serde_json::{Value, json};
use solomon_sim::economy::{
    HubInventoryAction, NATIVE_SKILL_BOOK_DEFINITIONS, NativeSkillBookOutcome,
    create_native_skill_book_inventory_item, find_inventory_item,
};
use solomon_sim::profile::{
    NATIVE_SKILL_CATALOG, NewGameSelection, WizardDiscipline, WizardElement,
};
use solomon_sim::session::{GameSession, GameSessionOptions};

#[test]
fn current_book_receipts_follow_choice_rank_and_exhaustion() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/skill-book-outcome-goldens.json"))
            .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let mut session = GameSession::new(
            "owner".into(),
            NewGameSelection {
                name: "Book Reader".into(),
                element: WizardElement::Fire,
                discipline: WizardDiscipline::Body,
            },
            GameSessionOptions {
                gameplay_rng_seed: case["seed"].as_i64().unwrap(),
                ..GameSessionOptions::default()
            },
        );
        let player = &mut session.participants.get_mut("owner").unwrap().player;
        if case["pending"].as_bool().unwrap() {
            player.grant_experience(95.0, &mut session.gameplay_rng);
        }
        if case["capped"].as_bool().unwrap() {
            for (id, definition) in NATIVE_SKILL_CATALOG.iter().enumerate().take(80).skip(8) {
                if id != 52 && definition.maximum_level > 0 {
                    player.book.grant_ranks(id, 1000);
                }
            }
        }
        let definition = NATIVE_SKILL_BOOK_DEFINITIONS
            .iter()
            .find(|definition| {
                Some(u64::from(definition.native_subtype)) == case["subtype"].as_u64()
            })
            .unwrap();
        player.economy.backpack.push(create_native_skill_book_inventory_item(definition, 1000));
        for (index, expected) in case["records"].as_array().unwrap().iter().enumerate() {
            // A choice book opens the actual Session barrier. A second command
            // rejected there retains its first receipt, just as the Source Game API.
            let result = session
                .apply_inventory("owner", &HubInventoryAction::ReadSkillBook { item_id: 1000 });
            let player = &session.participants.get("owner").expect("participant exists").player;
            let feedback = player.economy.action_feedback.as_ref().unwrap();
            let state = player_snapshot::snapshot(player, &session.gameplay_rng);
            let actual = json!({
                "accepted": result.transaction.accepted,
                "feedback": {
                    "accepted": feedback.accepted,
                    "action": feedback.action.as_str(),
                    "dowsingPitch": feedback.dowsing_pitch,
                    "reason": feedback.reason.map(|reason| reason.as_str()),
                    "sequence": feedback.sequence,
                    "skillBookOutcome": feedback.skill_book_outcome.map(|outcome| match outcome {
                        NativeSkillBookOutcome::Choice => json!({"kind": "choice"}),
                        NativeSkillBookOutcome::Rank { skill_id } => json!({"kind": "rank", "skillId": skill_id}),
                    }),
                    "transferDirection": null,
                    "transferGesture": null,
                    "unforgeOutcome": null,
                },
                "book": state["book"],
                "gameRng": state["gameRng"],
                "offerSeed": player.progression.offer_seed,
                "pendingOffer": state["progression"]["pendingOffer"],
                "itemRemaining": find_inventory_item(&player.economy.backpack, 1000).is_some(),
            });
            support::assert_website_json(
                &actual,
                expected,
                &format!("{} read {index}", case["name"]),
                &[],
            );
        }
    }
}
