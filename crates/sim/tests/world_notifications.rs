//! The current fa08 shared notification contract, distinct from the retained ed0a loot fixture.
use serde_json::{Value, json};
use solomon_sim::Vec2;
use solomon_sim::boneyard::loot::messages::{
    NativeLootMessagePresentation, NativeWorldNotificationInput, NativeWorldNotificationSource,
    native_world_notifications_visible,
};
use solomon_sim::boneyard::loot::{BoneyardLootEvent, BoneyardLootEventType, BoneyardLootSound};
use solomon_sim::combat::PlayerLifeState;
use solomon_sim::economy::{
    HubActionFeedback, HubActionFeedbackAction, NativeSkillBookFeedbackCursor,
    NativeSkillBookOutcome, native_skill_book_result_text,
};

#[test]
fn the_current_notification_program_replays_the_unmodified_source_exactly() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/world-notifications-goldens.json"))
            .unwrap();
    assert_eq!(fixture["sourceRevision"], "fa08ae249ff5563570bdb2a653e2ef9358ce7b83");
    let scenarios = fixture["scenarios"].as_array().unwrap();
    assert_eq!(scenarios.len(), 9);
    let mut records = 0;
    for scenario in scenarios {
        let name = scenario["name"].as_str().unwrap();
        let mut presentation = NativeLootMessagePresentation::new(0);
        for (index, record) in scenario["records"].as_array().unwrap().iter().enumerate() {
            records += 1;
            if let Some(tick) = record["sample"].as_u64() {
                let visuals = presentation.sample_world_notifications(tick);
                let actual: Vec<_> = visuals
                    .iter()
                    .map(|visual| {
                        let message = &visual.message;
                        let id = message.event_id as i64;
                        let id = if message.key.source == NativeWorldNotificationSource::Book {
                            -id
                        } else {
                            id
                        };
                        json!({
                            "eventId": id, "key": message.key.to_string(),
                            "lifetime": message.lifetime, "offset": message.offset,
                            "text": message.text, "tint": message.tint,
                            "alpha": visual.alpha, "scale": visual.scale,
                        })
                    })
                    .collect();
                assert_source_value(&json!(actual), &record["visuals"], &format!("{name}:{index}"));
            } else {
                let accepted = if let Some(event) = record.get("loot") {
                    presentation.consume(&BoneyardLootEvent {
                        actor_id: 1,
                        event_id: event["eventId"].as_u64().unwrap(),
                        goodie_id: None,
                        phase: None,
                        playback_rate: None,
                        player_id: Some("local".into()),
                        position: Vec2::ZERO,
                        sound: (event["sound"] == "pickup-coin")
                            .then_some(BoneyardLootSound::PickupCoin),
                        text: event["text"].as_str().map(String::from),
                        tick: event["tick"].as_u64().unwrap(),
                        event_type: BoneyardLootEventType::LootPickup,
                    })
                } else {
                    let event = &record["notification"];
                    presentation.consume_text(&NativeWorldNotificationInput {
                        source: match event["source"].as_str().unwrap() {
                            "loot" => NativeWorldNotificationSource::Loot,
                            "book" => NativeWorldNotificationSource::Book,
                            "combat" => NativeWorldNotificationSource::Combat,
                            "secondary" => NativeWorldNotificationSource::Secondary,
                            source => panic!("unknown source {source}"),
                        },
                        event_id: event["eventId"].as_i64().unwrap().unsigned_abs(),
                        tick: event["tick"].as_u64().unwrap(),
                        text: event["text"].as_str().unwrap(),
                        tint: event["tint"].as_u64().unwrap() as u32,
                    })
                };
                assert_eq!(accepted, record["accepted"].as_bool().unwrap(), "{name}:{index}");
            }
        }
    }
    assert_eq!(records, 1960);
    for case in fixture["visibility"].as_array().unwrap() {
        let life = match case["life"].as_str() {
            None => None,
            Some("alive") => Some(PlayerLifeState::Alive),
            Some("lethal-pending") => Some(PlayerLifeState::LethalPending),
            Some("dying") => Some(PlayerLifeState::Dying),
            Some("spectating") => Some(PlayerLifeState::Spectating),
            _ => unreachable!(),
        };
        assert_eq!(
            native_world_notifications_visible(life, case["combat"].as_bool().unwrap()),
            case["visible"].as_bool().unwrap()
        );
    }
    for case in fixture["text"].as_array().unwrap() {
        let actual = native_skill_book_result_text(case["skillId"].as_u64().unwrap() as usize);
        if let Some(text) = case["text"].as_str() {
            assert_eq!(actual.unwrap(), text);
        } else {
            assert_eq!(actual.unwrap_err(), case["error"].as_str().unwrap());
        }
    }
    let mut cursor =
        NativeSkillBookFeedbackCursor::new(feedback(&fixture["cursor"]["initial"]).as_ref());
    for (index, record) in fixture["cursor"]["records"].as_array().unwrap().iter().enumerate() {
        let receipt = cursor.consume(feedback(&record["input"]).as_ref());
        let actual = receipt.map(|receipt| {
            let outcome = match receipt.outcome {
                NativeSkillBookOutcome::Choice => json!({"kind":"choice"}),
                NativeSkillBookOutcome::Rank { skill_id } => {
                    json!({"kind":"rank","skillId":skill_id})
                }
            };
            json!({"sequence":receipt.sequence,"outcome":outcome})
        });
        assert_eq!(json!(actual), record["result"], "cursor:{index}");
    }
}

fn assert_source_value(actual: &Value, expected: &Value, path: &str) {
    match (actual, expected) {
        (Value::Number(actual), Value::Number(expected)) => {
            assert_eq!(actual.as_f64(), expected.as_f64(), "{path}");
        }
        (Value::Array(actual), Value::Array(expected)) => {
            assert_eq!(actual.len(), expected.len(), "{path} length");
            for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
                assert_source_value(actual, expected, &format!("{path}[{index}]"));
            }
        }
        (Value::Object(actual), Value::Object(expected)) => {
            assert_eq!(actual.len(), expected.len(), "{path} fields");
            for (key, expected) in expected {
                assert_source_value(&actual[key], expected, &format!("{path}.{key}"));
            }
        }
        _ => assert_eq!(actual, expected, "{path}"),
    }
}

fn feedback(value: &Value) -> Option<HubActionFeedback> {
    value.as_object()?;
    Some(HubActionFeedback {
        accepted: value["accepted"].as_bool().unwrap(),
        action: if value["action"] == "read-skill-book" {
            HubActionFeedbackAction::ReadSkillBook
        } else {
            assert_eq!(value["action"], "consume");
            HubActionFeedbackAction::Consume
        },
        sequence: value["sequence"].as_u64().unwrap(),
        skill_book_outcome: value["skillBookOutcome"]["kind"].as_str().map(|kind| match kind {
            "choice" => NativeSkillBookOutcome::Choice,
            "rank" => NativeSkillBookOutcome::Rank {
                skill_id: value["skillBookOutcome"]["skillId"].as_u64().map(|id| id as usize),
            },
            _ => unreachable!(),
        }),
        dowsing_pitch: None,
        dye_session_id: None,
        reason: None,
        transfer_direction: None,
        transfer_gesture: None,
        unforge_outcome: None,
    })
}

#[test]
fn equal_ids_from_distinct_notification_lanes_keep_the_newest_row_active() {
    let mut presentation = NativeLootMessagePresentation::new(0);
    for (source, tick, text) in [
        (NativeWorldNotificationSource::Loot, 1, "Health Potion"),
        (NativeWorldNotificationSource::Combat, 2, "CHEAT DEATH!"),
        (NativeWorldNotificationSource::Secondary, 3, "Overloaded Mana!"),
    ] {
        assert!(presentation.consume_text(&NativeWorldNotificationInput {
            source,
            event_id: 1,
            tick,
            text,
            tint: 0xffffff,
        }));
        presentation.sample_world_notifications(tick);
    }
    let rows = presentation.sample_world_notifications(3);
    assert_eq!(
        rows.iter().map(|row| row.message.key.to_string()).collect::<Vec<_>>(),
        ["loot:1", "combat:1", "secondary:1"]
    );
    assert_eq!(rows.last().unwrap().message.offset, -17.0);
}

#[test]
fn the_current_projection_rounds_scale_without_changing_the_historical_loot_api() {
    let mut presentation = NativeLootMessagePresentation::new(0);
    for (event_id, tick, text) in [(1, 1, "Health Potion"), (2, 2, "DAMAGE x4")] {
        presentation.consume_text(&NativeWorldNotificationInput {
            source: NativeWorldNotificationSource::Loot,
            event_id,
            tick,
            text,
            tint: 0xffffff,
        });
    }
    assert_eq!(presentation.sample(2)[0].scale, 0.984000027179718);
    assert_eq!(presentation.sample_world_notifications(2)[0].scale, 0.9800000190734863);
    assert_eq!(presentation.sample_world_notifications(303).len(), 0);
}

#[test]
fn notification_visibility_admits_lethal_pending_but_respects_death_and_combat_access() {
    for (life, expected) in [
        (Some(PlayerLifeState::Alive), true),
        (Some(PlayerLifeState::LethalPending), true),
        (Some(PlayerLifeState::Dying), false),
        (Some(PlayerLifeState::Spectating), false),
        (None, false),
    ] {
        assert_eq!(native_world_notifications_visible(life, true), expected);
        assert!(!native_world_notifications_visible(life, false));
    }
}
