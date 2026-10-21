use std::collections::BTreeSet;

use serde_json::{Value, json};
use solomon_sim::economy::NativeHubNpcState;
use solomon_sim::hub::interaction::{
    HubInteractionId, HubNpcChatChoice, HubNpcChatContent, HubNpcChatNext, consume_hub_npc_speech,
    create_hub_npc_chat_content, hub_dialogue_record, hub_npc_chat_choices, hub_npc_question,
};
use solomon_sim::native_ui_text::{NativeUiFontName, wrap_native_ui_text_runs};
use solomon_sim::player_panels::dialogue::{hub_chat_scroll_frame, hub_chat_text_runs};

#[test]
fn current_dialogue_one_shot_history_emphasis_and_accelerated_scroll_match_website() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/hub-dialogue-goldens.json")).unwrap();
    for expected in fixture["records"].as_array().unwrap() {
        let row = hub_dialogue_record(expected["key"].as_str().unwrap()).unwrap();
        assert_eq!(row.one_shot, expected["oneShot"].as_bool().unwrap_or(false), "{}", row.key);
        assert_eq!(row.label, expected["label"].as_str().unwrap());
        assert_eq!(json!(row.lines), expected["lines"]);
    }
    for family in fixture["families"].as_array().unwrap() {
        let id = HubInteractionId::from_id(family["id"].as_str().unwrap()).unwrap();
        let story = family["storyOffice"].as_bool().unwrap();
        let npc = NativeHubNpcState::default();
        let mut consumed = BTreeSet::new();
        let initial = create_hub_npc_chat_content(id, &npc, 1, None, story, &consumed);
        assert_eq!(
            content_json(&initial, id, story, &consumed),
            family["initial"],
            "initial {id:?}"
        );
        consume(&mut consumed, id, story, &initial);
        assert_eq!(json!(consumed), family["afterIntro"]);
        for step in family["questions"].as_array().unwrap() {
            assert_eq!(choices_json(id, story, &consumed), step["choices"]);
            let key = step["key"].as_str().unwrap();
            let answer = hub_npc_question(id, key, story, &consumed);
            let actual = answer.as_ref().map(|row| content_json(row, id, story, &consumed));
            assert_eq!(json!(actual), step["answer"], "answer {id:?} {key}");
            if let Some(answer) = answer {
                consume(&mut consumed, id, story, &answer);
            }
            assert_eq!(json!(consumed), step["consumed"]);
            let again = hub_npc_question(id, key, story, &consumed);
            let actual = again.as_ref().map(|row| content_json(row, id, story, &consumed));
            assert_eq!(json!(actual), step["again"]);
        }
        consume_hub_npc_speech(&mut consumed, id, "UNKNOWN_ROW", story);
        assert_eq!(json!(consumed), family["consumed"]);
        assert_eq!(choices_json(id, story, &consumed), family["finalChoices"]);
        let reopened = create_hub_npc_chat_content(id, &npc, 1, None, story, &consumed);
        assert_eq!(content_json(&reopened, id, story, &consumed), family["reopened"]);
    }
    for case in fixture["text"].as_array().unwrap() {
        let source = case["source"].as_str().unwrap();
        let runs = hub_chat_text_runs(source);
        let actual: Vec<_> =
            runs.iter().map(|run| json!({"text":run.text,"italic":run.italic})).collect();
        assert_eq!(json!(actual), case["runs"]);
        let lines = wrap_native_ui_text_runs(&runs, NativeUiFontName::Menu, 477.0);
        let actual: Vec<Vec<_>> = lines
            .iter()
            .map(|line| {
                line.iter()
                    .flat_map(|run| {
                        run.text
                            .chars()
                            .map(|text| json!({"text":text.to_string(),"italic":run.italic}))
                    })
                    .collect()
            })
            .collect();
        assert_eq!(json!(actual), case["lines"], "paragraph {source}");
    }
    for case in fixture["frames"].as_array().unwrap() {
        let frame = hub_chat_scroll_frame(
            1000.0,
            case["acceleratedAt"].as_f64(),
            case["now"].as_f64().unwrap(),
            case["height"].as_f64().unwrap(),
        );
        assert_eq!(frame.content_y, case["y"].as_f64().unwrap());
        assert_eq!(frame.complete, case["complete"].as_bool().unwrap());
    }
}

fn consume(
    consumed: &mut BTreeSet<String>,
    id: HubInteractionId,
    story: bool,
    content: &HubNpcChatContent,
) {
    if let HubNpcChatContent::Speech { key, .. } = content {
        consume_hub_npc_speech(consumed, id, key, story);
    }
}

fn choices_json(id: HubInteractionId, story: bool, consumed: &BTreeSet<String>) -> Value {
    let choices: Vec<_> = hub_npc_chat_choices(id, story, consumed)
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
    json!(choices)
}

fn content_json(
    content: &HubNpcChatContent,
    id: HubInteractionId,
    story: bool,
    consumed: &BTreeSet<String>,
) -> Value {
    match content {
        HubNpcChatContent::Choices => {
            json!({"kind":"choices","choices":choices_json(id,story,consumed)})
        }
        HubNpcChatContent::Selector(selector) => {
            json!({"kind":"selector","selector":selector.as_str()})
        }
        HubNpcChatContent::Speech { key, lines, next } => {
            json!({"kind":"speech","key":key,"lines":lines,"next":match next {HubNpcChatNext::Choices=>"choices",HubNpcChatNext::Close=>"close"}})
        }
    }
}
