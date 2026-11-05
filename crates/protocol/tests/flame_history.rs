//! Current Source connection replay, byte budget and complete-actor fallback.
use serde_json::{Value, json};
use solomon_protocol::flame_presentation::*;
use solomon_sim::spells::model::PrimarySpellTransient;
use solomon_sim::spells::weld::NativeWeldWorldActor;

#[path = "../../sim/tests/support/website_rows.rs"]
mod website_rows;

fn bytes(birth: &FlamePresentationBirth) -> Result<usize, String> {
    Ok(solomon_assets::json_value::stringify_serializable(birth)?.encode_utf16().count() * 2)
}
fn tick(value: &Value) -> u64 {
    value.as_u64().unwrap()
}
fn birth(value: &Value) -> FlamePresentationBirth {
    serde_json::from_value(value.clone()).unwrap()
}
fn compare_result(result: Result<Value, String>, row: &Value) {
    match result {
        Ok(actual) => {
            assert!(row.get("error").is_none(), "unexpected success; Source: {}", row["error"]);
            website_rows::compare(&actual, &row["result"], "Flame connection operation");
        }
        Err(error) => assert_eq!(error, row["error"].as_str().unwrap()),
    }
}
fn retained_end(birth: &FlamePresentationBirth) -> u64 {
    if birth.kind == FlamePresentationKind::Ribbon {
        birth.birth_tick + 7
    } else {
        birth.end_tick_exclusive
    }
}
fn frame(value: &Value) -> FlamePresentationFrame {
    serde_json::from_value(value.clone()).unwrap()
}
fn actor_row(actor: &PrimarySpellTransient, birth: &FlamePresentationBirth) -> Value {
    let PrimarySpellTransient::Weld(actor) = actor else { unreachable!() };
    let common = actor.common();
    let recipe = serde_json::to_value(&birth.recipe).unwrap();
    let root = &recipe["painterRegistration"];
    let mut row = json!({"ageTicks":common.owner.age_ticks,"birthTick":common.birth_tick,
        "buildId":common.build_id,"direction":{"x":common.direction.x,"y":common.direction.y},
        "id":common.owner.id,"ownerId":common.owner.owner_id,"vector":common.vector,
        "worldKey":common.owner.world_key,"origin":{"x":common.origin.x,"y":common.origin.y},
        "painterRegistrations":[root],"lightRegistration":if common.owner.light_registration.is_some(){root.clone()}else{Value::Null}});
    match actor {
        NativeWeldWorldActor::Channel(channel) => {
            let fields = json!({"kind":"weld-channel","flameVisual":recipe["birth"],"endpoint":channel.endpoint,
                "midpoint":channel.midpoint,"targetId":channel.target_id,"underpowered":channel.underpowered,
                "enhancedEffects":channel.enhanced_effects,"variant":channel.variant});
            row.as_object_mut().unwrap().extend(fields.as_object().unwrap().clone());
        }
        NativeWeldWorldActor::FlameLashFade(fade) => {
            let fields = json!({"kind":"weld-flame-lash-fade","flameVisual":recipe["birth"],"alpha":fade.alpha,
                "alphaStep":fade.alpha_step,"baseScale":fade.base_scale,"wrapperScalar":fade.wrapper_scalar,
                "position":fade.position,"variant":recipe["variant"],"record":fade.record,
                "colorGreen":fade.color_green,"rotationDegrees":fade.rotation_degrees});
            row.as_object_mut().unwrap().extend(fields.as_object().unwrap().clone());
        }
        _ => unreachable!(),
    }
    row
}

#[test]
fn current_source_flame_connection_history_and_retained_replay_match() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/flame-history-goldens.json"
    ))
    .unwrap();
    for case in fixture["historyCases"].as_array().unwrap() {
        let options = &case["options"];
        let mut history = FlamePresentationHistory::new(
            FlamePresentationHistoryOptions {
                world_key: options["worldKey"].as_str().unwrap().into(),
                tick: tick(&options["tick"]),
                maximum_births: tick(&options["maximumBirths"]) as usize,
                maximum_bytes: options["maximumBytes"].as_u64().map(|v| v as usize),
                initial_epoch: tick(&options["initialEpoch"]),
            },
            bytes,
        )
        .unwrap();
        for row in case["rows"].as_array().unwrap() {
            let input = &row["input"];
            let result = match input["op"].as_str().unwrap() {
                "record" => {
                    let incoming =
                        input["births"].as_array().unwrap().iter().map(birth).collect::<Vec<_>>();
                    history.record_tick(tick(&input["tick"]), &incoming).map(|()| Value::Null)
                }
                "retire" => history
                    .retire_birth(&birth(&input["birth"]), tick(&input["tick"]))
                    .map(|()| Value::Null),
                "owner" => history
                    .retire_owner(input["owner"].as_str().unwrap(), tick(&input["tick"]))
                    .map(|()| Value::Null),
                "discard" => {
                    history.discard_through_tick(tick(&input["tick"])).map(|()| Value::Null)
                }
                "reset" => history
                    .reset(input["worldKey"].as_str().unwrap().into(), tick(&input["tick"]))
                    .map(|()| Value::Null),
                "frame" => {
                    let baseline = if input["baseline"].is_null() {
                        None
                    } else {
                        Some(serde_json::from_value(input["baseline"].clone()).unwrap())
                    };
                    history
                        .frame(baseline, tick(&input["tick"]))
                        .map(|frame| serde_json::to_value(frame).unwrap())
                }
                _ => unreachable!(),
            };
            compare_result(result, row);
        }
    }
    for case in fixture["replayCases"].as_array().unwrap() {
        let camera = case["camera"].as_bool().unwrap();
        let mut replay = FlamePresentationReplay::new(
            tick(&case["maximumBirths"]) as usize,
            if camera { Some(retained_end) } else { None },
        );
        for row in case["rows"].as_array().unwrap() {
            let input = &row["input"];
            let result = match input["op"].as_str().unwrap() {
                "accept" => replay.accept(&frame(&input["frame"])).map(|()| Value::Null),
                "sample" => replay
                    .sample(input["tick"].as_f64().unwrap())
                    .map(|births| serde_json::to_value(births).unwrap()),
                "history" => {
                    Ok(serde_json::to_value(replay.history_at(input["tick"].as_f64().unwrap()))
                        .unwrap())
                }
                "discard" => replay.discard_before_tick(tick(&input["tick"])).map(|()| Value::Null),
                "retained" => {
                    Ok(serde_json::to_value(replay.retained(&birth(&input["birth"]))).unwrap())
                }
                _ => unreachable!(),
            };
            compare_result(result, row);
            assert_eq!(replay.through_tick(), tick(&row["throughTick"]));
        }
    }
    let mut host_history = FlamePresentationHistory::new(
        FlamePresentationHistoryOptions {
            world_key: "hub".into(),
            tick: 0,
            maximum_births: MAX_FLAME_PRESENTATION_BIRTHS,
            maximum_bytes: None,
            initial_epoch: 1,
        },
        bytes,
    )
    .unwrap();
    for expected in fixture["advances"].as_array().unwrap() {
        let before = expected["before"].as_array().unwrap().iter().map(birth).collect::<Vec<_>>();
        let after = expected["after"].as_array().unwrap().iter().map(birth).collect::<Vec<_>>();
        let tick = tick(&expected["tick"]);
        host_history
            .advance(expected["worldKey"].as_str().unwrap(), tick, &before, &after)
            .unwrap();
        website_rows::compare(
            &serde_json::to_value(host_history.frame(None, tick).unwrap()).unwrap(),
            &expected["reset"],
            "Flame actual host replacement boundary",
        );
        website_rows::compare(
            &serde_json::to_value(
                host_history
                    .frame(
                        Some(FlamePresentationBaseline {
                            epoch: 1,
                            world_tick: tick.saturating_sub(1),
                        }),
                        tick,
                    )
                    .unwrap(),
            )
            .unwrap(),
            &expected["frame"],
            "Flame actual host commit boundary",
        );
    }
    for expected in fixture["sizes"].as_array().unwrap() {
        assert_eq!(bytes(&birth(&expected["birth"])).unwrap(), tick(&expected["bytes"]) as usize);
    }
    for case in fixture["materialized"].as_array().unwrap() {
        let birth = birth(&case["birth"]);
        for row in case["frames"].as_array().unwrap() {
            let actor = materialize_flame_presentation(&birth, row["tick"].as_f64().unwrap());
            let actual =
                actor.as_ref().map(|actor| actor_row(actor, &birth)).unwrap_or(Value::Null);
            website_rows::compare(&actual, &row["actor"], "Flame display-only actor");
            if let Some(actor) = actor {
                assert_eq!(flame_presentation_birth(&actor).unwrap(), Some(birth.clone()));
            }
        }
    }
}
