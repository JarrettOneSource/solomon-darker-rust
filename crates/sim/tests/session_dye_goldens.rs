#![cfg(feature = "persistence")]
//! Current live-menu dye ownership through public Session inventory and tick calls.

mod support;

use serde_json::{Value, json};
use solomon_sim::Vec2;
use solomon_sim::economy::{
    FOMENTIUS_STOCK_DEFINITIONS, HubInventoryAction, NativeDyeLayer,
    create_fomentius_inventory_item,
};
use solomon_sim::profile::{NewGameSelection, WizardDiscipline, WizardElement};
use solomon_sim::session::{
    GameCheckpoint, GameSession, GameSessionInput, GameSessionOptions, GameSessionTickContext,
};
use std::collections::BTreeMap;

fn initial() -> GameSession {
    let mut state = GameSession::new_participants(
        [("local", "Local"), ("other", "Other")]
            .into_iter()
            .map(|(id, name)| {
                (
                    id.into(),
                    NewGameSelection {
                        name: name.into(),
                        discipline: WizardDiscipline::Arcane,
                        element: WizardElement::Ether,
                    },
                )
            })
            .collect(),
        GameSessionOptions { gameplay_rng_seed: 17, ..GameSessionOptions::default() },
    )
    .unwrap();
    for (id, first) in [("local", 100), ("other", 200)] {
        let player = &mut state.participants.get_mut(id).unwrap().player;
        let economy = &mut player.economy;
        let mut hat = economy.equipment.hat.take().unwrap();
        hat.id = first + 1;
        let mut robe = economy.equipment.robe.take().unwrap();
        robe.id = first + 2;
        economy.backpack = vec![
            create_fomentius_inventory_item(&FOMENTIUS_STOCK_DEFINITIONS[3], first, 2),
            hat,
            robe,
        ];
    }
    state
}

fn snapshot(state: &GameSession) -> Value {
    let sessions: BTreeMap<_, _> = state
        .inventory_dye_sessions
        .iter()
        .map(|(id, row)| {
            (id, json!({"id":row.id,"dyeItemId":row.dye_item_id,"kitConsumed":row.kit_consumed}))
        })
        .collect();
    let players:BTreeMap<_,_>=state.participants.iter().map(|(id,participant)| {
        let economy=&participant.player.economy;
        let backpack:Vec<_>=economy.backpack.iter().map(|item|json!({"id":item.id,"quantity":item.quantity,"iconTints":item.icon_tints})).collect();
        let feedback=economy.action_feedback.as_ref().map(|feedback|json!({
            "accepted":feedback.accepted,"action":feedback.action.as_str(),"sequence":feedback.sequence,
            "reason":feedback.reason.map(|reason|reason.as_str()),"dyeSessionId":feedback.dye_session_id
        }));
        (id,json!({"backpack":backpack,"feedback":feedback}))
    }).collect();
    json!({"sessions":sessions,"players":players})
}

fn action(value: &Value) -> HubInventoryAction {
    let session = || value["sessionId"].as_str().unwrap().into();
    match value["type"].as_str().unwrap() {
        "open-dye" => HubInventoryAction::OpenDye {
            dye_item_id: value["dyeItemId"].as_u64().unwrap(),
            session_id: session(),
        },
        "close-dye" => HubInventoryAction::CloseDye { session_id: session() },
        "dye" => HubInventoryAction::Dye {
            session_id: session(),
            target_item_id: value["targetItemId"].as_u64().unwrap(),
            layer: if value["layer"] == "cloth" {
                NativeDyeLayer::Cloth
            } else {
                NativeDyeLayer::Trim
            },
            swatch_rows: value["swatchRows"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_u64().unwrap() as usize)
                .collect(),
        },
        "rename-sack" => HubInventoryAction::RenameSack {
            item_id: value["itemId"].as_u64().unwrap(),
            name: value["name"].as_str().unwrap().into(),
        },
        value => panic!("unrecorded dye fixture action {value}"),
    }
}

#[test]
fn open_paint_close_and_input_lifetimes_match_current_two_player_source() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/session-dye-goldens.json")).unwrap();
    let mut state = initial();
    support::assert_website_json(&snapshot(&state), &fixture["initial"], "initial", &[]);
    for (index, row) in fixture["rows"].as_array().unwrap().iter().enumerate() {
        let step = &row["step"];
        let mut accepted = None;
        let mut reason = None;
        if step["action"].is_object() {
            let result =
                state.apply_inventory(step["playerId"].as_str().unwrap(), &action(&step["action"]));
            accepted = Some(result.transaction.accepted);
            reason = result.transaction.reason.map(|reason| reason.as_str());
        } else if let Some(inputs) = step["input"].as_object() {
            let inputs = inputs
                .iter()
                .map(|(id, input)| {
                    (
                        id.clone(),
                        GameSessionInput {
                            movement: serde_json::from_value(input["movement"].clone()).unwrap(),
                            primary: input["cast"]["primary"].as_bool().unwrap(),
                            quickbar: input["cast"]["quickbar"].as_u64().map(|slot| slot as usize),
                            ..GameSessionInput::default()
                        },
                    )
                })
                .collect();
            state.step(&inputs, GameSessionTickContext::default());
        } else if step["returnHub"] == true {
            state.return_to_hub();
        } else {
            assert!(state.remove_participant(step["remove"].as_str().unwrap()));
        }
        assert_eq!(json!(accepted), row["accepted"], "step {index}");
        assert_eq!(json!(reason), row["reason"], "step {index}");
        support::assert_website_json(
            &snapshot(&state),
            &row["expected"],
            &format!("step{index}"),
            &[],
        );
    }
}

#[test]
fn painting_permission_is_live_in_snapshots_and_absent_from_durable_checkpoints() {
    let mut state = initial();
    assert!(
        state
            .apply_inventory(
                "local",
                &HubInventoryAction::OpenDye { dye_item_id: 100, session_id: "live-menu".into() }
            )
            .transaction
            .accepted
    );
    let frame = serde_json::to_value(state.snapshot()).unwrap();
    let received: GameCheckpoint = serde_json::from_value(frame).unwrap();
    assert_eq!(received.inventory_dye_sessions["local"].id, "live-menu");
    let remote = GameSession::restore(received, None).unwrap();
    assert_eq!(remote.inventory_dye_session("local").unwrap().id, "live-menu");
    let checkpoint = state.checkpoint();
    assert!(checkpoint.inventory_dye_sessions.is_empty());
    assert!(serde_json::to_value(&checkpoint).unwrap().get("inventory_dye_sessions").is_none());
    assert!(state.inventory_dye_session("local").is_some());
    let mut inputs = BTreeMap::new();
    inputs.insert(
        "local".into(),
        GameSessionInput { movement: Vec2::new(1.0, 0.0), ..GameSessionInput::default() },
    );
    state.step(&inputs, GameSessionTickContext::default());
    assert!(state.inventory_dye_session("local").is_none());
}
