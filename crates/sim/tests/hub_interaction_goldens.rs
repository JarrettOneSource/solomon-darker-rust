//! Every stock College conversation, range, marker and selector against the Website.

use serde_json::{Value, json};
use solomon_sim::Vec2;
use solomon_sim::economy::NativeHubNpcState;
use solomon_sim::hub::interaction::*;
use solomon_sim::hub::regions::HubRegion;
use std::collections::BTreeSet;

fn point(value: &Value) -> Vec2 {
    Vec2::new(value["x"].as_f64().unwrap(), value["y"].as_f64().unwrap())
}
fn id(value: &Value) -> HubInteractionId {
    HubInteractionId::from_id(value.as_str().unwrap()).unwrap()
}
fn next(value: HubNpcChatNext) -> &'static str {
    match value {
        HubNpcChatNext::Choices => "choices",
        HubNpcChatNext::Close => "close",
    }
}
fn content(value: &HubNpcChatContent) -> Value {
    match value {
        HubNpcChatContent::Choices => json!({"kind":"choices"}),
        HubNpcChatContent::Selector(selector) => {
            json!({"kind":"selector","selector":selector.as_str()})
        }
        HubNpcChatContent::Speech { key, lines, next: continuation } => {
            json!({"kind":"speech","key":key,"lines":lines,"next":next(*continuation)})
        }
    }
}
fn surface(value: &Value) -> Option<HubNpcMarkerSurface> {
    value.as_str().map(|value| match value {
        "dialogue" => HubNpcMarkerSurface::Dialogue,
        "inventory" => HubNpcMarkerSurface::Inventory,
        "modal" => HubNpcMarkerSurface::Modal,
        "service" => HubNpcMarkerSurface::Service,
        other => panic!("surface {other}"),
    })
}
fn availability(value: &Value) -> HubInteractionAvailability {
    HubInteractionAvailability {
        skorcha_position: if value["skorchaPosition"].is_null() {
            None
        } else {
            Some(point(&value["skorchaPosition"]))
        },
        skorcha_variant: value["skorchaVariant"].as_u64().map(|value| value as usize),
        story_office: value["storyOffice"].as_bool().unwrap_or(false),
    }
}
fn assert_snapshot(actual: &Value, expected: &Value, path: &str) {
    match (actual, expected) {
        (Value::Number(actual), Value::Number(expected)) => {
            let actual = actual.as_f64().unwrap();
            let expected = expected.as_f64().unwrap();
            if path.ends_with(".alpha") {
                assert!((actual - expected).abs() < 1e-9, "{path}: {actual} vs {expected}");
            } else {
                assert_eq!(actual, expected, "{path}");
            }
        }
        (Value::Array(actual), Value::Array(expected)) => {
            assert_eq!(actual.len(), expected.len(), "{path} length");
            for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
                assert_snapshot(actual, expected, &format!("{path}[{index}]"));
            }
        }
        (Value::Object(actual), Value::Object(expected)) => {
            assert_eq!(actual.len(), expected.len(), "{path} fields");
            for (key, expected) in expected {
                assert_snapshot(&actual[key], expected, &format!("{path}.{key}"));
            }
        }
        _ => assert_eq!(actual, expected, "{path}"),
    }
}

#[test]
fn college_dialogue_geometry_markers_and_every_stock_selector_replay_the_website() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/hub-interaction-goldens.json"))
            .unwrap();
    for case in fixture["dialogues"].as_array().unwrap() {
        let interaction = id(&case["id"]);
        let story = case["storyOffice"].as_bool().unwrap();
        let npc = NativeHubNpcState::default();
        let consumed = BTreeSet::new();
        let choices: Vec<_> = hub_npc_chat_choices(interaction, story, &consumed)
            .into_iter()
            .map(|choice| match choice {
                HubNpcChatChoice::Command { label, selector } => {
                    json!({"kind":"command","label":label,"selector":selector.as_str()})
                }
                HubNpcChatChoice::Question { key, label } => {
                    json!({"kind":"question","key":key,"label":label})
                }
            })
            .collect();
        let initial = create_hub_npc_chat_content(interaction, &npc, 1, None, story, &consumed);
        let intro = if initial == HubNpcChatContent::Choices {
            json!({"kind":"choices","choices":choices})
        } else {
            content(&initial)
        };
        assert_snapshot(&intro, &case["intro"], "intro");
        assert_snapshot(&json!(choices), &case["choices"], "choices");
        for question in case["questions"].as_array().unwrap() {
            assert_snapshot(
                &hub_npc_question(interaction, question["key"].as_str().unwrap(), story, &consumed)
                    .as_ref()
                    .map(content)
                    .unwrap(),
                &question["content"],
                "question",
            );
        }
        assert_snapshot(
            &hub_npc_dismissal(interaction, -7, story).as_ref().map(content).unwrap_or(Value::Null),
            &case["dismissal"],
            "dismissal",
        );
    }
    for case in fixture["geometry"].as_array().unwrap() {
        let region = HubRegion::from_id(case["region"].as_str().unwrap()).unwrap();
        let at = point(&case["point"]);
        let available = availability(&case["availability"]);
        assert_eq!(
            nearest_hub_interaction(region, at, available).map(HubInteractionId::as_str),
            case["nearest"].as_str()
        );
        assert_eq!(
            hub_interaction_at_point(region, at, available).map(HubInteractionId::as_str),
            case["atPoint"].as_str()
        );
        assert_eq!(
            hub_interaction_within_range(id(&case["id"]), region, at, available),
            case["withinRange"].as_bool().unwrap()
        );
    }
    for case in fixture["markerFrames"].as_array().unwrap() {
        let suppression: [bool; 3] = case["suppression"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_bool().unwrap())
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();
        let actual = hub_npc_marker_frame(
            id(&case["id"]),
            case["tick"].as_u64().unwrap(),
            case["seed"].as_u64().unwrap() as u32,
            suppression,
            availability(&case["options"]),
            surface(&case["options"]["surface"]),
        );
        let side = match actual.side {
            HubNpcMarkerSide::Left => "left",
            HubNpcMarkerSide::Right => "right",
        };
        let style = match actual.style {
            HubNpcMarkerStyle::Help => "help",
            HubNpcMarkerStyle::Talk => "talk",
        };
        assert_snapshot(
            &json!({"alpha":actual.alpha,"interactionId":actual.interaction_id.as_str(),"phaseDegrees":actual.phase_degrees,
            "position":{"x":actual.position.x,"y":actual.position.y},"record":actual.record,"region":actual.region.id(),"side":side,"style":style,"visible":actual.visible}),
            &case["frame"],
            "marker",
        );
    }
    for case in fixture["selectors"].as_array().unwrap() {
        let selector = match case["selector"].as_str().unwrap() {
            "boast" => HubNpcListSelector::Boast,
            "books" => HubNpcListSelector::Books,
            _ => HubNpcListSelector::TeacherSpells,
        };
        let npc = NativeHubNpcState {
            librarian_lace_read: case["read"].as_bool().unwrap(),
            ..NativeHubNpcState::default()
        };
        assert_eq!(hub_npc_selector_title(selector), case["title"].as_str().unwrap());
        let rows: Vec<_> = hub_npc_selector_rows(
            selector,
            &npc,
            &[true, false, false, true, false, false, false, false],
        )
        .into_iter()
        .map(|row| {
            let mut value =
                json!({"detail":row.detail,"id":row.id,"label":row.label,"price":row.price});
            if let Some(record) = row.boast_icon_record {
                value["boastIcon"] = json!({"kind":"stock","record":record,"style":record-90});
            }
            value
        })
        .collect();
        assert_snapshot(&json!(rows), &case["rows"], "selector rows");
    }
    for case in fixture["selectorResponses"].as_array().unwrap() {
        let selector = match case["selector"].as_str().unwrap() {
            "boast" => HubNpcListSelector::Boast,
            "books" => HubNpcListSelector::Books,
            _ => HubNpcListSelector::TeacherSpells,
        };
        let id = case["id"].as_u64().unwrap() as u32;
        assert_snapshot(
            &hub_npc_selector_response(selector, id).as_ref().map(content).unwrap(),
            &case["response"],
            "selector response",
        );
        use solomon_sim::economy::HubInventoryAction;
        let action = match hub_npc_selector_action(selector, id) {
            HubInventoryAction::SelectBoast { boast_id } => {
                json!({"type":"select-boast","boastId":boast_id.native_id().unwrap()})
            }
            HubInventoryAction::ReadLibrarianBook { book_id } => {
                json!({"type":"read-librarian-book","bookId":book_id})
            }
            HubInventoryAction::BuyTeacherSpell { skill_id } => {
                json!({"type":"buy-teacher-spell","skillId":skill_id})
            }
            _ => panic!("a native NPC selector action"),
        };
        assert_snapshot(&action, &case["action"], "selector action");
    }
    for case in fixture["onboarding"].as_array().unwrap() {
        let flags: [bool; 10] = case["flags"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_bool().unwrap())
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();
        let plans:Vec<_>=hub_npc_onboarding_plan(&flags,case["tick"].as_u64().unwrap(),surface(&case["surface"])).into_iter().map(|plan|match plan {
            HubNpcOnboardingPlan::WalkToTalk {target,arrow_offset,arrow_record,arrow_rotation_degrees,text,text_offset}=>json!({"kind":"walk-to-talk","target":target.as_str(),
                "arrowOffset":{"x":arrow_offset.x,"y":arrow_offset.y},"arrowRecord":arrow_record,"arrowRotationDegrees":arrow_rotation_degrees,
                "text":text,"textOffset":{"x":text_offset.x,"y":text_offset.y}}),
            HubNpcOnboardingPlan::Directional {target,offset,record}=>json!({"kind":"directional","target":target.as_str(),"offset":{"x":offset.x,"y":offset.y},"record":record}),
        }).collect();
        assert_snapshot(&json!(plans), &case["plans"], "onboarding");
    }
}
