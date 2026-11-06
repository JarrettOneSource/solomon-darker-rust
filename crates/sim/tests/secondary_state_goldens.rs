//! Whole current secondary ownership, damage and status transactions, against recordings
//! from the Website client.
#![cfg(feature = "persistence")]

#[path = "support/secondary_current.rs"]
mod current;
#[path = "support/exact_bits.rs"]
#[allow(dead_code)]
mod exact_bits;
#[path = "support/secondary_state.rs"]
#[allow(dead_code)]
mod status_support;

use serde_json::{Value, json};
use solomon_sim::painter::WorldPainterOrder;
use solomon_sim::secondary::golem::*;
use solomon_sim::secondary::*;

// Each test crate uses a different subset of these shared row projections.
#[allow(dead_code)]
#[path = "support/secondary.rs"]
mod support;
use current::state_snapshot;
use exact_bits::source_value;
use support::*;

#[test]
fn secondary_burn_replacement_shield_damage_owner_cleanup_and_golem_death_replay_the_website() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/secondary-state-goldens.json"))
            .unwrap();
    assert_eq!(fixture["sourceRevision"], "fa08ae249ff5563570bdb2a653e2ef9358ce7b83");
    for journey in fixture["journeys"].as_array().unwrap() {
        let mut state = read_state(&journey["initial"]);
        let mut order = WorldPainterOrder::default();
        assert_snapshot(
            &source_value(&state_snapshot(&state)),
            &source_value(&journey["initial"]),
            "initial secondary state",
        );
        for (index, record) in journey["records"].as_array().unwrap().iter().enumerate() {
            let action = &record["action"];
            let mut result = Value::Null;
            match action["type"].as_str().unwrap() {
                "fire-burn" => {
                    let input = &action["input"];
                    state.apply_fire_burn(SecondaryFireBurnInput {
                        damage: input["damage"].as_f64().unwrap(),
                        owner_id: input["ownerId"].as_str().unwrap().to_owned(),
                        rank: input["rank"].as_u64().unwrap() as u32,
                        skill_id: input["skillId"].as_u64().unwrap() as usize,
                        target: target(&input["target"]),
                        world_key: input["worldKey"].as_str().unwrap().to_owned(),
                    });
                }
                "ether-burn" => {
                    let input = &action["input"];
                    state.apply_ether_burn(SecondaryEtherBurnInput {
                        owner_id: input["ownerId"].as_str().unwrap().to_owned(),
                        rank: input["rank"].as_u64().unwrap() as u32,
                        target: target(&input["target"]),
                        world_key: input["worldKey"].as_str().unwrap().to_owned(),
                    });
                }
                "player-damage" => {
                    let applied = state.apply_player_damage(
                        SecondaryPlayerDamageInput {
                            player_id: action["playerId"].as_str().unwrap(),
                            amount: action["amount"].as_f64().unwrap(),
                            tick: action["tick"].as_u64().unwrap(),
                            position: point(&action["position"]),
                            world_key: "boneyard:oracle",
                            presentation: SecondaryPlayerDamagePresentation::default(),
                        },
                        &mut order,
                    );
                    result = json!({"absorbedDamage":applied.absorbed_damage,"healthDamage":applied.health_damage});
                }
                "golem-damage" => {
                    let damage = &action["damage"];
                    let applied = state.apply_golem_damage(
                        action["actorId"].as_u64().unwrap(),
                        GolemIncomingDamage {
                            primary_damage: damage["primaryDamage"].as_f64().unwrap(),
                            secondary_damage: damage["secondaryDamage"].as_f64().unwrap(),
                            reflectable_physical_source_in_range:
                                damage["reflectablePhysicalSourceInRange"].as_bool().unwrap(),
                        },
                        action["tick"].as_u64().unwrap(),
                        true,
                        None,
                    );
                    result = json!({"ignored":applied.ignored,"killed":applied.killed,"ownerId":applied.owner_id,"reflectedDamage":applied.reflected_damage});
                }
                "actor-age" => {
                    let actor = state
                        .actors
                        .iter_mut()
                        .find(|actor| Some(actor.id) == action["id"].as_u64())
                        .unwrap();
                    actor.age_ticks = action["age"].as_u64().unwrap();
                }
                "player-patch" => {
                    state
                        .players
                        .get_mut(action["id"].as_str().unwrap())
                        .unwrap()
                        .stoneskin_ticks_remaining =
                        action["patch"]["stoneskinTicksRemaining"].as_u64().unwrap();
                }
                "target-effect" => apply_secondary_target_effect(
                    &mut state.target_effects,
                    action["world"].as_str().unwrap(),
                    action["target"].as_u64().unwrap() as usize,
                    status_support::patch(&action["patch"]),
                ),
                "remove-owner" => state.remove_owner(action["id"].as_str().unwrap()),
                "reset" => state.reset_world(),
                other => panic!("unknown secondary state action {other}"),
            }
            let context = format!("{} record {index}", journey["name"]);
            assert_snapshot(&result, &record["result"], &context);
            let mut expected_state = record["state"].clone();
            if journey["name"].as_str().is_some_and(|name| name.starts_with("shield-")) {
                // The retained direct-damage fixture predates callback-owned painter
                // enrollment. Its actor mechanics stay exact; only birth ownership moves.
                for (ordinal, actor) in
                    expected_state["actors"].as_array_mut().unwrap().iter_mut().enumerate()
                {
                    if matches!(
                        actor["kind"].as_str(),
                        Some("shield-break" | "shield-explosion" | "shockwave")
                    ) {
                        assert_eq!(actor["painterRegistrations"], json!([]));
                        actor["painterRegistrations"] = json!([{
                            "managerLane": "transient", "registrationOrdinal": ordinal,
                        }]);
                        if actor["kind"] == "shockwave" {
                            assert_eq!(actor["lightRegistration"], Value::Null);
                            actor["lightRegistration"] = json!({
                                "managerLane": "actor", "registrationOrdinal": 0,
                            });
                        }
                    }
                }
            }
            assert_snapshot(
                &source_value(&state_snapshot(&state)),
                &source_value(&expected_state),
                &context,
            );
        }
    }
}
