//! Whole current secondary ownership, damage and status transactions against Website.
#![cfg(feature = "persistence")]

#[path = "support/secondary_current.rs"]
mod current;
#[path = "support/native_bits.rs"]
#[allow(dead_code)]
mod native_bits;
#[path = "support/secondary_state.rs"]
#[allow(dead_code)]
mod status_support;

use serde_json::{Value, json};
use solomon_sim::secondary::golem::*;
use solomon_sim::secondary::*;

// Shared source projections are intentionally used in different subsets by host fixtures.
#[allow(dead_code)]
#[path = "support/secondary.rs"]
mod support;
use current::state_snapshot;
use native_bits::source_value;
use support::*;

#[test]
fn secondary_burn_replacement_shield_damage_owner_cleanup_and_golem_death_replay_the_website() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/secondary-state-goldens.json"))
            .unwrap();
    assert_eq!(fixture["sourceRevision"], "fa08ae249ff5563570bdb2a653e2ef9358ce7b83");
    for journey in fixture["journeys"].as_array().unwrap() {
        let mut state = read_state(&journey["initial"]);
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
                    state.apply_fire_burn(NativeSecondaryFireBurnInput {
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
                    state.apply_ether_burn(NativeSecondaryEtherBurnInput {
                        owner_id: input["ownerId"].as_str().unwrap().to_owned(),
                        rank: input["rank"].as_u64().unwrap() as u32,
                        target: target(&input["target"]),
                        world_key: input["worldKey"].as_str().unwrap().to_owned(),
                    });
                }
                "player-damage" => {
                    let applied = state.apply_player_damage(
                        action["playerId"].as_str().unwrap(),
                        action["amount"].as_f64().unwrap(),
                        action["tick"].as_u64().unwrap(),
                        point(&action["position"]),
                        "boneyard:oracle",
                        NativeSecondaryPlayerDamagePresentation::default(),
                    );
                    result = json!({"absorbedDamage":applied.absorbed_damage,"healthDamage":applied.health_damage});
                }
                "golem-damage" => {
                    let damage = &action["damage"];
                    let applied = state.apply_golem_damage(
                        action["actorId"].as_u64().unwrap(),
                        NativeGolemIncomingDamage {
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
                "target-effect" => apply_native_secondary_target_effect(
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
            assert_snapshot(
                &source_value(&state_snapshot(&state)),
                &source_value(&record["state"]),
                &context,
            );
        }
    }
}
