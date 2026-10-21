//! Real multi-party routing, pause isolation, Game Over return and profile transfer.

#[path = "../../sim/tests/support/player_combat.rs"]
#[allow(dead_code)]
mod player_combat;
#[path = "../../sim/tests/support/player_snapshot.rs"]
mod player_snapshot;
#[path = "../../sim/tests/support/secondary_state.rs"]
#[allow(dead_code)]
mod secondary_state;
#[path = "../../sim/tests/support/session.rs"]
mod session_snapshot;
#[path = "../../sim/tests/support/session_tick.rs"]
mod session_tick_rows;
#[path = "../../sim/tests/support/mod.rs"]
mod support;
#[path = "../../sim/tests/support/tutorial_snapshot.rs"]
mod tutorial_snapshot;

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};
use solomon_assets::boneyard::decode_scene;
use solomon_assets::navigation::decode_navigation;
use solomon_sim::Vec2;
use solomon_sim::boneyard::LoadedBoneyard;
use solomon_sim::profile::{NewGameSelection, WizardDiscipline, WizardElement};
use solomon_sim::session::{DetachedSessionParticipant, GameSession, GameSessionOptions};
use solomon_sim::shared_worlds::{SharedGameWorlds, SharedWorldsTickContext};
use solomon_sim::social::{PartyIdentity, PartyVisibility};

fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().unwrap()
}

fn selection(element: &str) -> NewGameSelection {
    NewGameSelection {
        name: "Oracle".into(),
        element: WizardElement::ALL.into_iter().find(|kind| kind.as_str() == element).unwrap(),
        discipline: WizardDiscipline::Arcane,
    }
}

fn identity(id: &str) -> PartyIdentity {
    PartyIdentity {
        id: format!("party-{id}"),
        join_code: format!("CODE-{id}"),
        listing_id: format!("listing-{id}"),
    }
}

fn snapshot(session: &GameSession) -> Value {
    if !session.participants.is_empty() {
        let mut value = session_tick_rows::current_source_snapshot_all(session);
        value["entityIds"] = json!(
            session
                .participant_order
                .iter()
                .map(|id| session.participants[id].entity_id)
                .collect::<Vec<_>>()
        );
        value["nextEntityId"] = json!(session.next_entity_id);
        return value;
    }
    json!({ "ids": [], "entityIds": [], "nextEntityId": session.next_entity_id, "tick": session.ticks,
        "gameRng": player_snapshot::rng_snapshot(&session.gameplay_rng),
        "combatRng": player_snapshot::rng_snapshot(&session.combat_rng),
        "order": { "nextRegistrationOrdinal": { "actor": session.painter_order.next_actor_ordinal,
            "transient": session.painter_order.next_transient_ordinal } },
        "run": session_snapshot::run(&session.run), "traderSeed": session.hub_trader_animation_seed })
}

fn snapshot_worlds(worlds: &SharedGameWorlds) -> Value {
    let mut runs = Vec::new();
    for run in &worlds.runs {
        runs.push(json!({"partyId": run.party_id, "runId": run.loaded_boneyard.run_id,
            "snapshot": snapshot(&run.state)}));
    }
    let mut groups = Vec::new();
    for party in &worlds.parties.parties {
        let visibility = match party.visibility {
            PartyVisibility::Public => "public",
            PartyVisibility::InviteOnly => "invite-only",
            PartyVisibility::Private => "private",
        };
        groups.push(json!([
            party.id,
            party.join_code,
            party.listing_id,
            party.leader_player_id,
            party.member_player_ids,
            visibility
        ]));
    }
    let mut invitations = Vec::new();
    for invitation in &worlds.parties.invitations {
        invitations.push(json!([
            invitation.id,
            invitation.inviter_player_id,
            invitation.invited_player_id,
            invitation.party_id
        ]));
    }
    assert!(worlds.parties.join_requests.is_empty());
    json!({ "hub": snapshot(&worlds.hub), "runs": runs,
        "memorial": serde_json::to_value(&worlds.hub.hub_memorial).unwrap(),
        "parties": {"revision": worlds.parties.revision,
            "nextInvitationOrdinal": worlds.parties.next_invitation_ordinal,
            "groups": groups, "invitations": invitations, "requests": []} })
}

fn collect_trig_paths(state: &Value, prefix: &str, paths: &mut Vec<String>) {
    for id in state["ids"].as_array().unwrap() {
        let id = id.as_str().unwrap();
        for field in ["position.x", "position.y", "velocity.x", "velocity.y"] {
            paths.push(format!("{prefix}.participants.{id}.character.{field}"));
        }
    }
    if let Some(students) = state["shared"]["world"]["students"].as_array() {
        for index in 0..students.len() {
            for axis in 0..2 {
                paths.push(format!("{prefix}.shared.world.students[{index}][1][{axis}]"));
            }
        }
    }
}

fn compare(actual: &Value, expected: &Value, label: &str) {
    let mut paths = Vec::new();
    collect_trig_paths(&expected["hub"], "hub", &mut paths);
    for (index, run) in expected["runs"].as_array().unwrap().iter().enumerate() {
        collect_trig_paths(&run["snapshot"], &format!("runs[{index}].snapshot"), &mut paths);
    }
    let paths: Vec<_> = paths.iter().map(String::as_str).collect();
    support::assert_website_json(actual, expected, label, &paths);
}

#[test]
fn independent_parties_keep_the_shared_college_and_other_runs_live() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/shared-worlds-goldens.json"
    ))
    .unwrap();
    let source = &fixture["loaded"];
    let template = LoadedBoneyard {
        choice_id: text(&source["choice"], "id").into(),
        choice_source: solomon_sim::boneyard::BoneyardSource::Default,
        mod_id: None,
        mod_name: None,
        name: text(&source["choice"], "name").into(),
        run_id: "fixture".into(),
        seed: text(source, "seed").into(),
        source_sha256: text(source, "sourceSha256").into(),
        geometry_sha256: text(source, "geometrySha256").into(),
        scene: decode_scene(&source["scene"].to_string()).unwrap(),
    };
    let mut worlds = SharedGameWorlds::new(0);
    let mut detached: Option<DetachedSessionParticipant> = None;
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../web/assets/navigation")
        .join(format!("{}.json", template.source_sha256));
    let navigation = decode_navigation(&std::fs::read_to_string(path).unwrap(), &template).unwrap();
    let navigation_by_party = BTreeMap::from([
        ("party-zeta".into(), navigation.clone()),
        ("party-omega".into(), navigation),
    ]);
    for (index, row) in fixture["rows"].as_array().unwrap().iter().enumerate() {
        let request = &row["request"];
        let action = text(row, "action");
        let mut extra = Value::Null;
        let result = match action {
            "create" => Ok(()),
            "add" => {
                let id = text(request, "id");
                worlds.add_hub_player(id.into(), selection(text(request, "element")), identity(id));
                Ok(())
            }
            "invite" => worlds.invite(text(request, "id"), text(request, "target"), 4),
            "accept" => {
                worlds.accept_invitation(text(request, "id"), text(request, "invitation"), 4)
            }
            "start" => {
                let mut loaded = template.clone();
                loaded.run_id = text(request, "runId").into();
                worlds.start_party_run(text(request, "id"), loaded)
            }
            "step" => {
                let paused: BTreeSet<String> =
                    serde_json::from_value(request["paused"].clone()).unwrap();
                for _ in 0..request["count"].as_u64().unwrap() {
                    worlds.step(
                        &BTreeMap::new(),
                        SharedWorldsTickContext {
                            paused_party_ids: Some(&paused),
                            navigation_by_party: Some(&navigation_by_party),
                            ..Default::default()
                        },
                    );
                }
                Ok(())
            }
            "save-state" => {
                extra = worlds
                    .party_save_state_for_player(text(request, "id"))
                    .unwrap()
                    .map_or(Value::Null, |state| snapshot(&state));
                Ok(())
            }
            "loaded" => {
                extra = json!(
                    worlds
                        .loaded_boneyard_for_player(text(request, "id"))
                        .map(|loaded| &loaded.run_id)
                );
                Ok(())
            }
            "detach" => {
                let id = text(request, "id");
                detached =
                    Some(worlds.state_for_player(id).unwrap().detach_participant(id).unwrap());
                worlds.detach_player(id).unwrap();
                Ok(())
            }
            "rejoin" => worlds.rejoin_party_run_player(
                detached.as_ref().unwrap().clone(),
                text(request, "partyId"),
                identity(&format!("{}-rejoined", text(request, "id"))),
                request["maximum"].as_u64().unwrap() as usize,
                None,
            ),
            "partition" => {
                let selected: Vec<String> = serde_json::from_value(request["ids"].clone()).unwrap();
                extra = match worlds.hub.partition(&selected) {
                    Ok(partition) => {
                        json!({ "selected": snapshot(&partition.selected), "remaining": snapshot(&partition.remaining) })
                    }
                    Err(error) => json!({ "error": error }),
                };
                Ok(())
            }
            "damage" => {
                let state = worlds.state_for_player_mut(text(request, "id")).unwrap();
                state.participants.get_mut(text(request, "id")).unwrap().player.damage_with_result(
                    request["amount"].as_f64().unwrap(),
                    state.ticks,
                    true,
                    false,
                    1.0,
                    0.0,
                );
                Ok(())
            }
            "continue" => worlds.continue_game_over(
                text(request, "id"),
                text(request, "runId"),
                request["eventId"].as_u64().unwrap(),
            ),
            "confirm" => {
                worlds.confirm_loadout(text(request, "id"), selection(text(request, "element")))
            }
            "remove" => {
                worlds.remove_player(text(request, "id")).unwrap();
                Ok(())
            }
            "restore" => {
                let id = text(request, "id");
                let mut restored = GameSession::new(
                    id.into(),
                    selection(text(request, "element")),
                    GameSessionOptions { gameplay_rng_seed: 73, ..Default::default() },
                );
                let wizard = &mut restored.participants.get_mut(id).unwrap().wizard;
                wizard.position = Vec2::new(1234.0, 567.0);
                wizard.velocity = Vec2::new(4.0, -3.0);
                worlds.restore_player(restored, None, id, identity(id)).unwrap();
                Ok(())
            }
            _ => panic!("unknown source action {action}"),
        };
        let label = format!("row {index} {action}");
        assert_eq!(result.is_ok(), row["accepted"].as_bool().unwrap(), "{label}");
        assert_eq!(json!(result.err().map(|reason| reason.id())), row["reason"], "{label}");
        compare(&snapshot_worlds(&worlds), &row["expected"], &label);
        if action == "save-state" && !extra.is_null() {
            let mut paths = Vec::new();
            collect_trig_paths(&row["extra"], "", &mut paths);
            let paths: Vec<_> = paths.iter().map(String::as_str).collect();
            support::assert_website_json(&extra, &row["extra"], &label, &paths);
        } else if action == "partition" && extra.get("error").is_none() {
            for key in ["selected", "remaining"] {
                let mut paths = Vec::new();
                collect_trig_paths(&row["extra"][key], "", &mut paths);
                let paths: Vec<_> = paths.iter().map(String::as_str).collect();
                support::assert_website_json(&extra[key], &row["extra"][key], key, &paths);
            }
        } else {
            assert_eq!(extra, row["extra"], "{label}");
        }
    }
}
