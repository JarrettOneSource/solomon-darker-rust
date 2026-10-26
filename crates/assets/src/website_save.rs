//! Current Website save interchange (`save/game-save-document.ts`).
//! Native host checkpoints remain their own durable format; this boundary translates
//! the Website's component rows into the existing simulation owners.

mod contract;
mod create;
mod normalize;
mod profile;
mod projection;
mod restore;
pub(crate) mod value;

pub use create::{CreateSaveOptions, create, create_profile, retire};
pub use profile::{RestoredGameProfile, hydrate_profile, restore_profile};
pub use restore::{RestoredGameSave, restore};

pub use contract::{
    MAX_SAVE_BYTES, NativeSaveFile, NativeSaveSource, ParsedContinuation, ParsedSaveDocument,
    SCHEMA_VERSION, SaveIntegrity, SaveMetadata, SaveSummary, SaveWorldKind, parse,
};

#[cfg(test)]
#[path = "../../sim/tests/support/mod.rs"]
mod support;

#[cfg(test)]
mod tests {
    use super::value::{decode, encode};
    use serde_json::Value;
    use solomon_sim::economy::HubEconomyState;

    fn current_runtime_fixture(family: &str) -> Value {
        use sha2::{Digest, Sha256};
        let bytes = miniz_oxide::inflate::decompress_to_vec_zlib(include_bytes!(
            "../../sim/tests/fixtures/website/current-save-runtime-goldens.json.zlib"
        ))
        .unwrap();
        let digest =
            Sha256::digest(&bytes).iter().map(|byte| format!("{byte:02x}")).collect::<String>();
        assert_eq!(
            digest,
            include_str!(
                "../../sim/tests/fixtures/website/current-save-runtime-goldens.json.sha256"
            )
            .split_whitespace()
            .next()
            .unwrap()
        );
        let mut fixture: Value = serde_json::from_slice(&bytes).unwrap();
        fixture["families"].as_object_mut().unwrap().remove(family).unwrap()
    }

    fn live_simulation(state: &solomon_sim::session::GameSession, player_id: &str) -> Value {
        let mut actual = super::projection::simulation(state, player_id).unwrap();
        // Future ticks include menu authorization that the disk projection omits.
        actual["inventoryDyeSessions"] = encode(&state.inventory_dye_sessions).unwrap();
        actual
    }

    #[test]
    fn restored_enemy_worlds_keep_the_same_controller_rng_and_future_ticks() {
        replay_saved_worlds(&current_runtime_fixture("save-world-migration"));
    }

    #[test]
    fn restored_weld_and_secondary_actors_continue_in_the_shared_spell_queue() {
        replay_saved_worlds(&current_runtime_fixture("save-spell-migration"));
    }

    #[test]
    fn restored_scenery_preserves_independent_tables_through_native_checkpoints() {
        let fixture = current_runtime_fixture("save-scenery-retention");
        for case in fixture["cases"].as_array().unwrap() {
            if case["accepted"] == false {
                continue;
            }
            let mut restored = super::restore(&case["document"].to_string()).unwrap();
            let checkpoint = serde_json::to_string(&restored.session.checkpoint()).unwrap();
            restored.session = solomon_sim::session::GameSession::restore(
                serde_json::from_str(&checkpoint).unwrap(),
                restored.loaded_boneyard.clone(),
            )
            .unwrap();
            let actual = live_simulation(&restored.session, &restored.player_id);
            super::support::assert_website_json(
                &actual,
                &case["frames"][0]["simulation"],
                &format!("{} native checkpoint", case["name"]),
                &[],
            );
        }
        replay_saved_worlds(&fixture);
    }

    fn replay_saved_worlds(fixture: &Value) {
        for case in fixture["cases"].as_array().unwrap() {
            let result = super::restore(&case["document"].to_string());
            if case["accepted"].as_bool() == Some(false) {
                let error = result.err().unwrap_or_else(|| panic!("{} was accepted", case["name"]));
                assert_eq!(error, case["error"].as_str().unwrap(), "{}", case["name"]);
                continue;
            }
            let mut restored = result.unwrap_or_else(|error| panic!("{}: {error}", case["name"]));
            let navigation = restored
                .loaded_boneyard
                .as_ref()
                .map(solomon_sim::boneyard::navigation::NavigationSet::for_scene);
            let mut elapsed = 0;
            for frame in case["frames"].as_array().unwrap() {
                while elapsed < frame["ticks"].as_u64().unwrap() {
                    restored.session.step(
                        &std::collections::BTreeMap::new(),
                        solomon_sim::session::GameSessionTickContext {
                            navigation: navigation.as_ref(),
                            ..Default::default()
                        },
                    );
                    elapsed += 1;
                }
                let actual = live_simulation(&restored.session, &restored.player_id);
                super::support::assert_website_json(
                    &actual,
                    &frame["simulation"],
                    &format!("{} tick{elapsed}", case["name"]),
                    &[],
                );
            }
        }
    }

    #[test]
    fn saved_components_rebuild_runtime_and_reject_invalid_persistent_choices() {
        let fixture = current_runtime_fixture("save-migration");
        for case in fixture["cases"].as_array().unwrap() {
            let result = super::restore(&case["document"].to_string());
            if case["accepted"].as_bool() == Some(false) {
                assert_eq!(
                    result.unwrap_err(),
                    case["error"].as_str().unwrap(),
                    "{}",
                    case["name"]
                );
                continue;
            }
            let restored = result.unwrap_or_else(|error| panic!("{}: {error}", case["name"]));
            let projected =
                super::projection::simulation(&restored.session, &restored.player_id).unwrap();
            super::support::assert_website_json(
                &projected["world"]["participants"],
                &case["restored"]["hubParticipants"],
                &format!("{} Hub participants", case["name"]),
                &[],
            );
            for field in [
                "playerEntities",
                "primarySpells",
                "secondaryAbilities",
                "run",
                "worldManagerOrder",
                "gameRng",
            ] {
                super::support::assert_website_json(
                    &projected[field],
                    &case["restored"][field],
                    &format!("{} {field}", case["name"]),
                    &[],
                );
            }
        }
    }

    #[test]
    fn action_feedback_follows_real_transactions_and_clears_only_on_run_archive() {
        use solomon_sim::economy::{CompletedRunEconomyArchive, EquipmentSlot, HubInventoryAction};
        use solomon_sim::hub::interaction::HubInteractionId;
        use solomon_sim::profile::WizardElement;

        let fixture = current_runtime_fixture("save-profile");
        let owner = fixture["playerId"].as_str().unwrap();
        let mut restored = super::restore(&fixture["feedbackDocument"].to_string()).unwrap();
        for case in fixture["feedbackActions"].as_array().unwrap() {
            let command = &case["command"];
            let item_id = || command["itemId"].as_u64().unwrap();
            let action = match command["type"].as_str().unwrap() {
                "buy-fomentius" => HubInventoryAction::BuyFomentius { item_id: item_id() },
                "close-hagatha" => HubInventoryAction::CloseHagatha,
                "dowse" => HubInventoryAction::Dowse { reference_item_id: None },
                "close-dowsing" => HubInventoryAction::CloseDowsing,
                "transfer" => HubInventoryAction::Transfer {
                    item_id: item_id(),
                    direction: decode(command["direction"].clone()).unwrap(),
                    gesture: decode(command["gesture"].clone()).unwrap(),
                },
                "acknowledge-npc-hint" => HubInventoryAction::AcknowledgeNpcHint {
                    interaction_id: HubInteractionId::Teacher,
                },
                "unequip" => HubInventoryAction::Unequip {
                    slot: EquipmentSlot::Hat,
                    destination_sack_id: None,
                    destination_slot: None,
                },
                "unforge" => HubInventoryAction::Unforge { item_id: item_id() },
                kind => panic!("unknown source feedback command {kind}"),
            };
            let result = restored.session.apply_inventory(owner, &action);
            assert_eq!(
                result.transaction.accepted,
                case["accepted"].as_bool().unwrap(),
                "{command}"
            );
            let economy = &restored
                .session
                .participants
                .get(owner)
                .expect("participant exists")
                .player
                .economy;
            super::support::assert_website_json(
                &encode(&economy.action_feedback).unwrap(),
                &case["feedback"],
                "actionFeedback",
                &[],
            );
            assert_eq!(economy.revision, case["revision"].as_u64().unwrap(), "{command}");
            super::support::assert_website_json(
                &encode(&economy.rng).unwrap(),
                &case["rng"],
                "economy RNG",
                &[],
            );
        }
        let economy = &mut restored.session.participants.get_mut(owner).unwrap().player.economy;
        economy.archive_completed_run(CompletedRunEconomyArchive {
            display_name: "Profile Oracle",
            ground_gold: 0,
            ground_items: &[],
            starter_element: WizardElement::Fire,
            transfer_carried_items: false,
        });
        assert_eq!(encode(&economy.action_feedback).unwrap(), fixture["archivedFeedback"]);
    }

    #[test]
    fn website_profiles_normalize_legacy_economies_and_hydrate_fresh_wizards() {
        use solomon_sim::session::{GameSession, GameSessionOptions};
        let fixture = current_runtime_fixture("save-profile");
        let owner = fixture["playerId"].as_str().unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let result = super::restore_profile(&case["document"].to_string());
            if !case["accepted"].as_bool().unwrap() {
                assert_eq!(
                    result.unwrap_err(),
                    case["error"].as_str().unwrap(),
                    "{}",
                    case["name"]
                );
                continue;
            }
            let profile = result.unwrap_or_else(|error| panic!("{}: {error}", case["name"]));
            super::support::assert_website_json(
                &encode(&profile.economy).unwrap(),
                &case["economy"],
                &format!("{} economy", case["name"]),
                &[],
            );
            super::support::assert_website_json(
                &encode(&profile.hagatha_runtime).unwrap(),
                &case["hagathaRuntime"],
                "hagathaRuntime",
                &[],
            );
            let mut state = GameSession::new(
                owner.into(),
                decode(fixture["config"].clone()).unwrap(),
                GameSessionOptions {
                    gameplay_rng_seed: 17,
                    combat_rng_seed: 91,
                    ..GameSessionOptions::default()
                },
            );
            super::hydrate_profile(&mut state, owner, &profile).unwrap();
            let actual = super::projection::simulation(&state, owner).unwrap();
            super::support::assert_website_json(
                &actual["playerEntities"],
                &case["players"],
                &format!("{} hydrated", case["name"]),
                &[],
            );
        }
    }

    #[test]
    fn current_website_continuations_restore_canonical_session_owners() {
        let fixture = current_runtime_fixture("save-document");
        let active = current_runtime_fixture("save-active");
        for case in
            fixture["cases"].as_array().unwrap().iter().chain(active["cases"].as_array().unwrap())
        {
            if case["accepted"] == false {
                assert_eq!(
                    super::restore(&case["document"].to_string()).unwrap_err(),
                    case["error"].as_str().unwrap(),
                    "{}",
                    case["name"]
                );
                continue;
            }
            let restored = super::restore(&case["document"].to_string())
                .unwrap_or_else(|error| panic!("{}: {error}", case["name"]));
            let expected = &case["restored"];
            let reencoded = super::create(super::CreateSaveOptions {
                state: &restored.session,
                player_id: &restored.player_id,
                loaded_boneyard: restored.loaded_boneyard.as_ref(),
                metadata: &restored.metadata,
                party_rejoin_token: None,
            })
            .unwrap_or_else(|error| panic!("{} encode: {error}", case["name"]));
            // Restoring the Hub reconstructs its 900-tick student population. Only these
            // independently recomputed trigonometric trajectories admit the named tolerance;
            // saved values, discrete ownership and all Boneyard state remain exact.
            let student_count = case["reencoded"]["continuation"]["simulation"]["world"]["studentPopulation"]["students"].as_array().map_or(0, Vec::len);
            let trig_paths: Vec<String> = (0..student_count)
                .flat_map(|index| {
                    [
                        "wander.x",
                        "wander.y",
                        "position.x",
                        "position.y",
                        "framePhase",
                        "gaitDegrees",
                        "profile.pushResistance",
                    ]
                    .map(|field| format!(".world.studentPopulation.students[{index}].{field}"))
                })
                .collect();
            super::support::assert_website_json(
                &serde_json::from_str(&reencoded).unwrap(),
                &case["reencoded"],
                &format!("{} reencoded", case["name"]),
                &trig_paths.iter().map(String::as_str).collect::<Vec<_>>(),
            );
            assert_eq!(restored.session.ticks, expected["tick"].as_u64().unwrap());
            super::support::assert_website_json(
                &encode(&restored.session.gameplay_rng).unwrap(),
                &expected["gameRng"],
                "gameRng",
                &[],
            );
            super::support::assert_website_json(
                &encode(&restored.session.combat_rng).unwrap(),
                &expected["combatRng"],
                "combatRng",
                &[],
            );
            let player = &restored
                .session
                .participants
                .get(&restored.player_id)
                .expect("participant exists");
            super::support::assert_website_json(
                &encode(&player.player.economy).unwrap(),
                &expected["playerEntities"]["economies"][0],
                "economy",
                &[],
            );
            super::support::assert_website_json(
                &encode(&player.player.skill_runtime).unwrap(),
                &expected["playerEntities"]["skillRuntimes"][0],
                "skillRuntime",
                &[],
            );
            super::support::assert_website_json(
                &encode(&player.lighting).unwrap(),
                &expected["playerEntities"]["lightings"][0],
                "lighting",
                &[],
            );
        }
    }

    #[test]
    fn website_save_envelopes_preserve_legacy_summaries_and_rejections() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../sim/tests/fixtures/website/save-document-goldens.json"
        ))
        .unwrap();
        for case in fixture["contracts"].as_array().unwrap() {
            let result = super::parse(case["text"].as_str().unwrap());
            if !case["accepted"].as_bool().unwrap() {
                assert_eq!(
                    result.unwrap_err(),
                    case["error"].as_str().unwrap(),
                    "{}",
                    case["name"]
                );
                continue;
            }
            let parsed = result.unwrap_or_else(|error| panic!("{}: {error}", case["name"]));
            assert_eq!(parsed.source_schema_version, case["sourceSchemaVersion"].as_u64().unwrap());
            let summary = parsed.continuation.as_ref().map(|value| &value.summary);
            super::support::assert_website_json(
                &encode(&summary).unwrap(),
                &case["summary"],
                "summary",
                &[],
            );
            assert_eq!(serde_json::to_value(parsed.metadata.integrity).unwrap(), case["integrity"]);
            assert_eq!(serde_json::to_value(parsed.metadata.mods).unwrap(), case["mods"]);
            let source_state: solomon_sim::modding::LuaValue =
                serde_json::from_str(case["text"].as_str().unwrap()).unwrap();
            if let Some(state) = source_state.as_object().unwrap().get("modState") {
                assert_eq!(
                    crate::json_value::stringify(&solomon_sim::modding::LuaValue::Object(
                        parsed.metadata.mod_state
                    ))
                    .unwrap(),
                    crate::json_value::stringify(state).unwrap()
                );
            }
            assert_eq!(
                serde_json::to_value(parsed.metadata.native_source).unwrap(),
                case["nativeSource"]
            );
        }
    }

    #[test]
    fn website_saved_economies_decode_without_changing_item_identity() {
        let fixture = current_runtime_fixture("save-document");
        for case in fixture["cases"].as_array().unwrap() {
            for profile in ["profileDocument", "retired"] {
                let source = &case[profile]["profile"]["economy"];
                let economy: HubEconomyState = decode(source.clone())
                    .unwrap_or_else(|error| panic!("{} {profile}: {error}", case["name"]));
                assert_eq!(economy.gold, source["gold"].as_u64().unwrap());
                assert_eq!(economy.next_item_id, source["nextItemId"].as_u64().unwrap());
                assert_eq!(economy.backpack.len(), source["backpack"].as_array().unwrap().len());
                assert_eq!(
                    economy.rng.words.to_vec(),
                    serde_json::from_value::<Vec<u32>>(source["rng"]["words"].clone()).unwrap()
                );
                super::support::assert_website_json(
                    &encode(&economy).unwrap(),
                    source,
                    "economy",
                    &[],
                );
            }
        }
    }
}
