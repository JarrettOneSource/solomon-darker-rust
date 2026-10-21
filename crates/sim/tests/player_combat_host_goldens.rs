//! Entity-level poison, death, recovery and automatic potion behavior against source.
#[path = "support/player_snapshot.rs"]
mod command_player_snapshot;
#[path = "support/player_combat.rs"]
mod support;
use serde_json::{Value, json};
use solomon_sim::NativeRng;
use solomon_sim::session::player_combat::*;
use support::*;

#[test]
fn entity_damage_mana_and_ordered_resource_death_ticks_replay_the_source() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/player-combat-host-goldens.json"))
            .unwrap();
    for (index, case) in fixture["damages"].as_array().unwrap().iter().enumerate() {
        let (mut player, _, _) = configured_player(&case["spec"]);
        let flags = player.damage_with_result(
            case["amount"].as_f64().unwrap(),
            case["tick"].as_u64().unwrap(),
            case["damageAlreadyScaled"].as_bool().unwrap(),
            case["recordHit"].as_bool().unwrap(),
            case["hitStrength"].as_f64().unwrap(),
            0.0,
        );
        assert_snapshot(
            &json!({"autoHealthPotionUsed":flags.auto_health_potion_used,"cheatDeathTriggered":flags.cheat_death_triggered}),
            &case["flags"],
            &format!("damage:{index} flags"),
        );
        assert_snapshot(&player_snapshot(&player), &case["expected"], &format!("damage:{index}"));
    }
    for (index, case) in fixture["debits"].as_array().unwrap().iter().enumerate() {
        let (mut player, _, _) = configured_player(&case["spec"]);
        let flags = player.try_debit_mana(case["cost"].as_f64().unwrap());
        assert_snapshot(
            &json!({"accepted":flags.accepted,"autoManaPotionUsed":flags.auto_mana_potion_used}),
            &case["flags"],
            &format!("debit:{index} flags"),
        );
        assert_snapshot(&player_snapshot(&player), &case["expected"], &format!("debit:{index}"));
    }
    for journey in fixture["journeys"].as_array().unwrap() {
        let spec = &journey["spec"];
        let (mut player, wizard, primary) = configured_player(spec);
        let mut rng = NativeRng::new(917);
        let mut expected = journey["samples"].as_array().unwrap().iter().peekable();
        for tick in 1..=spec["ticks"].as_u64().unwrap() {
            let flags = step_player_combat(PlayerCombatContext {
                player_id: "local",
                player: &mut player,
                wizard: &wizard,
                primary_cast: &primary,
                acting_staff: spec["actingStaff"].as_bool().unwrap_or(false),
                blindness_ticks_remaining: 0.0,
                tick,
                mana_ceiling: spec["manaCeiling"].as_f64(),
                rng: &mut rng,
            });
            if expected.peek().is_some_and(|sample| sample["tick"].as_u64() == Some(tick)) {
                let expected = expected.next().unwrap();
                let ids = |active: bool| if active { vec!["local"] } else { Vec::new() };
                let poison=flags.poison_responses.iter().map(|response|match response{
                    PlayerPoisonResponse::Deflect{pitch}=>json!({"kind":"deflect","playerId":"local","pitch":pitch}),
                    PlayerPoisonResponse::Flash{response}=>json!({"kind":"flash","playerId":"local","response":flash_snapshot(response)}),
                }).collect::<Vec<_>>();
                let actual = json!({"autoHealthPotionPlayerIds":ids(flags.auto_health_potion_used),"beganDeathEpochPlayerIds":ids(flags.began_death_epoch),"cheatDeathPlayerIds":ids(flags.cheat_death_triggered),"completedDeathPresentationPlayerIds":ids(flags.completed_death_presentation),"deathBurstPlayerIds":ids(flags.emitted_death_burst),"lastWordArchivePlayerIds":ids(flags.last_word_archive),"lastWordBurstPlayerIds":ids(flags.last_word_burst),"poisonResponses":poison});
                let label = format!("{}:{tick}", spec["name"].as_str().unwrap());
                assert_snapshot(&actual, &expected["flags"], &format!("{label} flags"));
                assert_snapshot(&player_snapshot(&player), &expected["expected"], &label);
                assert_snapshot(&rng_snapshot(&rng), &expected["rng"], &format!("{label} rng"));
            }
        }
        assert!(expected.next().is_none());
    }
}

use std::cell::RefCell;
use std::collections::BTreeMap;

use solomon_sim::economy::{BoastSelection, HubEquipmentState, HubInventoryItem};
use solomon_sim::hub::interaction::BoastRules;
use solomon_sim::modding::extensions::*;
use solomon_sim::session::{
    GameSession, GameSessionInput, GameSessionOptions, GameSessionTickContext,
};

struct RecordedFilters<'a> {
    spec: &'a Value,
    calls: RefCell<Vec<Value>>,
}

impl GameSimulationExtensions for RecordedFilters<'_> {
    fn create_loot_items(&self, _: GameSimulationLootInput) -> Vec<HubInventoryItem> {
        panic!("a Hub resource epoch cannot create enemy loot")
    }

    fn filter_damage(
        &self,
        input: GameSimulationDamageFilterInput<'_>,
        _: &HubEquipmentState,
    ) -> Result<f64, String> {
        let kind = match input.damage_kind {
            GameSimulationDamageKind::Physical => "physical",
            GameSimulationDamageKind::Magic => "magic",
            GameSimulationDamageKind::Poison => "poison",
        };
        self.calls.borrow_mut().push(json!({"kind":"damage","amount":input.amount,
            "damageKind":kind,"sourceActorId":input.source_actor_id,
            "targetPlayerId":input.target_player_id,"tick":input.tick}));
        if let Some(error) = self.spec["damageError"].as_str() {
            return Err(error.into());
        }
        Ok(input.amount * self.spec["damageScale"].as_f64().unwrap_or(1.0)
            + self.spec["damageAdd"].as_f64().unwrap_or(0.0))
    }

    fn filter_mana(
        &self,
        input: GameSimulationManaFilterInput<'_>,
        _: &HubEquipmentState,
    ) -> Result<f64, String> {
        let source = match input.source {
            GameSimulationManaSource::EnemyContact => "enemy-contact",
            GameSimulationManaSource::Overload => "overload",
            GameSimulationManaSource::PassiveRecovery => "passive-recovery",
            GameSimulationManaSource::PrimaryCast => "primary-cast",
            GameSimulationManaSource::SecondaryCast => "secondary-cast",
            GameSimulationManaSource::StockOrb => "stock-orb",
            GameSimulationManaSource::StockPotion => "stock-potion",
        };
        self.calls.borrow_mut().push(json!({"kind":"mana","currentMana":input.current_mana,
            "delta":input.delta,"maximumMana":input.maximum_mana,"playerId":input.player_id,
            "source":source,"tick":input.tick}));
        if let Some(error) = self.spec["manaError"].as_str() {
            return Err(error.into());
        }
        Ok(input.delta * self.spec["manaScale"].as_f64().unwrap_or(1.0)
            + self.spec["manaAdd"].as_f64().unwrap_or(0.0))
    }

    fn has_consumable(&self, content_id: &str) -> bool {
        self.spec["admitted"].as_bool() == Some(true)
            && self.spec["contentId"].as_str() == Some(content_id)
    }

    fn resolve_boast(&self, selection: &BoastSelection) -> Option<BoastRules<'_>> {
        let BoastSelection::Mod(selection) = selection else { return None };
        if self.spec["admitted"].as_bool() != Some(true)
            || selection.content_id != "123"
            || selection.mod_id != "hosted.content"
        {
            return None;
        }
        Some(BoastRules {
            failure_producers: &[
                solomon_sim::hub::interaction::BoastFailureProducer::PotionUse,
                solomon_sim::hub::interaction::BoastFailureProducer::ManaUnderflow,
            ],
            label: "Careful",
            random_skill_choices: true,
            score_multiplier: 1.75,
            success_wave: 2,
        })
    }
}

#[test]
fn authored_resource_filters_follow_public_source_epochs_and_finite_error_boundaries() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/session-extensions-goldens.json"))
            .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let spec = &case["spec"];
        let name = spec["name"].as_str().unwrap();
        let (player, mut wizard, primary) = support::configured_player(spec);
        wizard.primary_cast = primary;
        let mut state = GameSession::new(
            "local".into(),
            player.selection.clone(),
            GameSessionOptions {
                gameplay_rng_seed: 17,
                combat_rng_seed: 917,
                ..GameSessionOptions::default()
            },
        );
        let participant = state.participants.get_mut("local").unwrap();
        *participant.player = player;
        participant.wizard = wizard;
        let filters = RecordedFilters { spec, calls: RefCell::new(Vec::new()) };
        let inputs = BTreeMap::from([("local".into(), GameSessionInput::default())]);
        for sample in case["samples"].as_array().unwrap() {
            let result = state.step_with_extensions(
                &inputs,
                GameSessionTickContext::default(),
                Some(&filters),
            );
            let actual_error = result.as_ref().err().map(String::as_str);
            assert_eq!(actual_error, sample["error"].as_str(), "{name} error");
            let calls = std::mem::take(&mut *filters.calls.borrow_mut());
            support::assert_snapshot(&json!(calls), &sample["calls"], &format!("{name} callbacks"));
            if actual_error.is_some() {
                // Source aborts the epoch and the host closes its clients. No partially
                // mutated Session is published or allowed to run another epoch.
                break;
            }
            assert_eq!(state.ticks, sample["tick"].as_u64().unwrap(), "{name} tick");
            support::assert_snapshot(
                &support::player_snapshot(&state.participants["local"].player),
                &sample["player"],
                &format!("{name} resources at {}", state.ticks),
            );
            support::assert_snapshot(
                &support::rng_snapshot(&state.secondary.rng),
                &sample["secondaryRng"],
                &format!("{name} resource RNG"),
            );
        }
    }
}

#[test]
fn authored_skill_replacement_consumes_only_the_current_native_offer_and_updates_its_barrier() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/session-mod-skill-goldens.json"))
            .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let spec = &case["spec"];
        let (player, mut wizard, primary) = support::configured_player(spec);
        wizard.primary_cast = primary;
        let mut state = GameSession::new(
            "local".into(),
            player.selection.clone(),
            GameSessionOptions {
                gameplay_rng_seed: 17,
                combat_rng_seed: 917,
                ..GameSessionOptions::default()
            },
        );
        let participant = state.participants.get_mut("local").unwrap();
        *participant.player = player;
        participant.wizard = wizard;
        state.grant_experience("local", 900.0);
        for sample in case["samples"].as_array().unwrap() {
            let id = sample["playerId"].as_str().unwrap();
            let sequence = sample["offerSequence"].as_u64().unwrap();
            assert_eq!(
                state.replace_skill_with_mod(id, sequence),
                sample["accepted"].as_bool().unwrap(),
                "authored replacement {id}:{sequence}"
            );
            support::assert_snapshot(
                &command_player_snapshot::snapshot(
                    &state.participants["local"].player,
                    &state.gameplay_rng,
                ),
                &sample["player"],
                "authored replacement player/RNG",
            );
            let progression = &state.participants["local"].player.progression;
            assert_eq!(json!(progression.pending_levels), sample["pendingLevels"]);
            assert_eq!(json!(progression.forced_offer_skill_ids), sample["forcedOfferSkillIds"]);
            assert_eq!(
                json!(progression.sorcerors_charm_available),
                sample["sorcerorsCharmAvailable"]
            );
            let barrier = state.level_up_barrier.as_ref().map(|barrier| {
                json!({"barrierId":barrier.barrier_id,"milestoneLevel":barrier.milestone_level,
                    "milestoneExperience":barrier.milestone_experience,
                    "participantIds":barrier.participant_ids,"pendingPlayerIds":barrier.pending_player_ids,
                    "runId":barrier.run_id,"sourcePlayerId":barrier.source_player_id})
            });
            support::assert_snapshot(
                &json!(barrier),
                &sample["barrier"],
                "authored replacement barrier",
            );
        }
    }
}

#[test]
// This journey decodes the actual admitted catalog and serializes its consumption,
// effects and authored selection. The default simulation keeps serde optional.
#[cfg(feature = "persistence")]
fn inventory_filters_and_authored_consumption_replay_public_source_transactions() {
    use solomon_sim::economy::{HubInventoryAction, find_inventory_item};
    use solomon_sim::modding::inventory::{
        ModConsumableCatalogEntry, mod_consumable_inventory_item,
    };

    let fixture: Value = serde_json::from_str(include_str!(
        "fixtures/website/session-inventory-extensions-goldens.json"
    ))
    .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let mut spec = case["spec"].clone();
        let catalog: ModConsumableCatalogEntry =
            serde_json::from_value(case["catalog"].clone()).unwrap();
        spec["contentId"] = json!(catalog.content.content_id);
        let name = spec["name"].as_str().unwrap();
        let (player, mut wizard, primary) =
            support::configured_player(&json!({"combat":{"currentMana":10}}));
        wizard.primary_cast = primary;
        let mut state = GameSession::new(
            "local".into(),
            player.selection.clone(),
            GameSessionOptions { gameplay_rng_seed: 17, ..GameSessionOptions::default() },
        );
        let participant = state.participants.get_mut("local").unwrap();
        *participant.player = player;
        participant.wizard = wizard;
        if spec["mod"].as_bool() == Some(true) {
            for _ in 0..spec["repeats"].as_u64().unwrap() {
                assert!(
                    participant
                        .player
                        .economy
                        .insert_loot_item(mod_consumable_inventory_item(&catalog))
                );
            }
        }
        let filters = RecordedFilters { spec: &spec, calls: RefCell::new(Vec::new()) };
        for step in case["steps"].as_array().unwrap() {
            let action = match step["action"]["type"].as_str().unwrap() {
                "consume" => {
                    let item_id = step["action"]["itemId"].as_u64().unwrap();
                    assert!(
                        find_inventory_item(
                            &state.participants["local"].player.economy.backpack,
                            item_id,
                        )
                        .is_some()
                    );
                    HubInventoryAction::Consume { item_id }
                }
                "select-boast" => HubInventoryAction::SelectBoast {
                    boast_id: serde_json::from_value(step["action"]["boastId"].clone()).unwrap(),
                },
                action => panic!("unrecorded source inventory action {action}"),
            };
            let result = state.apply_inventory_with_extensions("local", &action, Some(&filters));
            let actual_error = result.as_ref().err().map(String::as_str);
            assert_eq!(actual_error, step["error"].as_str(), "{name} error");
            let calls = std::mem::take(&mut *filters.calls.borrow_mut());
            support::assert_snapshot(&json!(calls), &step["calls"], &format!("{name} filters"));
            let Ok(result) = result else { break };
            assert_eq!(result.transaction.accepted, step["accepted"].as_bool().unwrap(), "{name}");
            assert_eq!(
                result.transaction.reason.map(|reason| reason.as_str()),
                step["reason"].as_str(),
                "{name}"
            );
            support::assert_snapshot(
                &serde_json::to_value(&result.mod_consumption).unwrap(),
                &step["modConsumption"],
                &format!("{name} accepted consumption"),
            );
            let player = &state.participants["local"].player;
            let mut snapshot = support::player_snapshot(player);
            snapshot["economy"]["boast"]["selected"] =
                serde_json::to_value(&player.economy.npc.boast.selected).unwrap();
            support::assert_snapshot(&snapshot, &step["player"], &format!("{name} player"));
            support::assert_snapshot(
                &serde_json::to_value(&state.mod_effects).unwrap(),
                &step["modEffects"],
                &format!("{name} effects"),
            );
            assert_eq!(
                state.next_mod_consumable_use_id,
                step["nextUseId"].as_u64().unwrap(),
                "{name}"
            );
        }
    }
}
