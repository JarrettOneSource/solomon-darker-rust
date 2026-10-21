//! Current Website profile contracts and genuine historical profile inputs.

use serde_json::{Value, json};
use solomon_assets::website_save;
use solomon_sim::profile::{NewGameSelection, WizardDiscipline, WizardElement};
use solomon_sim::session::{GameSession, GameSessionOptions};

#[path = "../../sim/tests/support/mod.rs"]
mod support;

#[test]
fn current_profile_envelopes_preserve_advanced_unlocks_and_hydrate_historical_saves() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/current-save-profile-goldens.json"
    ))
    .unwrap();
    for case in fixture["contracts"].as_array().unwrap() {
        let result = website_save::parse(&case["document"].to_string());
        let name = case["name"].as_str().unwrap();
        if case["accepted"] == false {
            assert_eq!(result.unwrap_err(), case["error"].as_str().unwrap(), "{name}");
        } else {
            let parsed = result.unwrap_or_else(|error| panic!("{name}: {error}"));
            assert_eq!(parsed.source_schema_version, case["sourceSchemaVersion"].as_u64().unwrap());
        }
    }
    let player_id = fixture["playerId"].as_str().unwrap();
    let config = NewGameSelection {
        name: fixture["config"]["displayName"].as_str().unwrap().into(),
        element: WizardElement::ALL
            .into_iter()
            .find(|element| element.as_str() == fixture["config"]["element"])
            .unwrap(),
        discipline: WizardDiscipline::ALL
            .into_iter()
            .find(|discipline| discipline.as_str() == fixture["config"]["discipline"])
            .unwrap(),
    };
    for case in fixture["profiles"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let result = website_save::restore_profile(&case["document"].to_string());
        if case["accepted"] == false {
            assert_eq!(result.unwrap_err(), case["error"].as_str().unwrap(), "{name}");
            continue;
        }
        let profile = result.unwrap_or_else(|error| panic!("{name}: {error}"));
        let mut session = GameSession::new(
            player_id.to_owned(),
            config.clone(),
            GameSessionOptions { gameplay_rng_seed: 17, combat_rng_seed: 91, ..Default::default() },
        );
        website_save::hydrate_profile(&mut session, player_id, &profile).unwrap();
        let actual: Value = serde_json::from_str(
            &website_save::create_profile(&session, player_id, &profile.metadata).unwrap(),
        )
        .unwrap();
        support::assert_website_json(&actual, &case["reencoded"], name, &[]);
        assert_eq!(
            session.participants[player_id].player.book.advanced_unlocks.as_slice(),
            serde_json::from_value::<Vec<bool>>(case["advancedUnlocks"].clone()).unwrap(),
            "{name} permanent unlocks"
        );
        support::assert_website_json(
            &json!({"indexA":session.gameplay_rng.index_a,"indexB":session.gameplay_rng.index_b,
                "words":session.gameplay_rng.words.to_vec()}),
            &case["gameRng"],
            &format!("{name} hydrated RNG"),
            &[],
        );
    }
}
