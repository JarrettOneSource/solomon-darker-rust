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
            session
                .participants
                .get(player_id)
                .expect("participant exists")
                .player
                .book
                .advanced_unlocks
                .as_slice(),
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

#[test]
fn portable_item_rarities_keep_source_spelling_and_read_earlier_rust_exports() {
    use solomon_assets::website_save::{CreateSaveOptions, SaveIntegrity, SaveMetadata};
    use solomon_sim::economy::{
        DOWSING_EQUIPMENT_RECIPES, FOMENTIUS_STOCK_DEFINITIONS, HubItemKind,
        create_equipment_inventory_item, create_fomentius_inventory_item,
    };
    use solomon_sim::modding::LuaObject;

    let mut session = GameSession::new(
        "local".into(),
        NewGameSelection {
            name: "Felix".into(),
            element: WizardElement::Earth,
            discipline: WizardDiscipline::Arcane,
        },
        GameSessionOptions::default(),
    );
    let sack =
        FOMENTIUS_STOCK_DEFINITIONS.iter().find(|item| item.kind == HubItemKind::Sack).unwrap();
    let mut inner = create_fomentius_inventory_item(sack, 1001, 1);
    inner.contents = Some(vec![
        create_equipment_inventory_item(&DOWSING_EQUIPMENT_RECIPES[0], 1002),
        create_equipment_inventory_item(&DOWSING_EQUIPMENT_RECIPES[1], 1003),
    ]);
    let mut outer = create_fomentius_inventory_item(sack, 1000, 1);
    outer.contents = Some(vec![inner]);
    {
        let participant = session.participants.get_mut("local").unwrap();
        let economy = &mut participant.player.economy;
        economy.storage = vec![outer];
        economy.equipment.robe =
            Some(create_equipment_inventory_item(&DOWSING_EQUIPMENT_RECIPES[1], 1004));
        economy.equipment.rings[0] =
            Some(create_equipment_inventory_item(&DOWSING_EQUIPMENT_RECIPES[0], 1005));
        economy.backpack.push(create_equipment_inventory_item(&DOWSING_EQUIPMENT_RECIPES[0], 1006));
        economy.next_item_id = 1010;
    }
    let metadata = SaveMetadata {
        integrity: SaveIntegrity::LocalOnly,
        mods: Vec::new(),
        mod_state: LuaObject::default(),
        native_source: None,
    };
    let profile = website_save::create_profile(&session, "local", &metadata).unwrap();
    let continuation = website_save::create(CreateSaveOptions {
        state: &session,
        player_id: "local",
        loaded_boneyard: None,
        metadata: &metadata,
        party_rejoin_token: None,
    })
    .unwrap();
    for text in [profile, continuation] {
        let encoded: Value = serde_json::from_str(&text).unwrap();
        let inventory = &encoded["profile"]["economy"]["storage"][0]["contents"][0]["contents"];
        assert_eq!(inventory[0]["rarity"], "Rare");
        assert_eq!(inventory[1]["rarity"], "Epic");
        let canonical = website_save::restore_profile(&text).unwrap();
        let legacy = text
            .replace("\"rarity\":\"Rare\"", "\"rarity\":\"rare\"")
            .replace("\"rarity\":\"Epic\"", "\"rarity\":\"epic\"");
        assert_ne!(text, legacy);
        let restored = website_save::restore_profile(&legacy).unwrap();
        assert_eq!(restored.economy, canonical.economy);
        let mut fresh = session.clone();
        website_save::hydrate_profile(&mut fresh, "local", &restored).unwrap();
        let reencoded: Value = serde_json::from_str(
            &website_save::create_profile(&fresh, "local", &restored.metadata).unwrap(),
        )
        .unwrap();
        assert_eq!(
            reencoded["profile"]["economy"]["storage"],
            encoded["profile"]["economy"]["storage"]
        );
        if !encoded["continuation"].is_null() {
            let current = website_save::restore(&text).unwrap();
            let old = website_save::restore(&legacy).unwrap();
            assert_eq!(
                old.session.participants.get("local").unwrap().player.economy,
                current.session.participants.get("local").unwrap().player.economy
            );
        }
        let invalid = legacy.replace("\"rarity\":\"rare\"", "\"rarity\":\"legendary\"");
        assert!(website_save::restore_profile(&invalid).is_err());
    }
}
