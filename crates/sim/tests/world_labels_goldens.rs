//! Pinned world-label metrics and actual accepted-chat receipt lifetimes.

use serde_json::{Value, json};
use solomon_sim::Vec2;
use solomon_sim::native_ui_text::NativeUiGlyphLayout;
use solomon_sim::social::*;
use solomon_sim::world_labels::*;

fn point(value: &Value) -> Vec2 {
    Vec2::new(value["x"].as_f64().unwrap(), value["y"].as_f64().unwrap())
}

fn glyphs(glyphs: &[NativeUiGlyphLayout]) -> Value {
    json!(
        glyphs
            .iter()
            .map(|glyph| json!({
                "record":glyph.glyph.record,"centerX":glyph.center_x,"centerY":glyph.center_y,
                "scale":glyph.scale,
            }))
            .collect::<Vec<_>>()
    )
}

fn speech(value: &GameWorldSpeech) -> Value {
    json!({
        "channel":value.channel.label().to_lowercase(),"expiresAtMs":value.expires_at_ms,
        "holdUntilMs":value.hold_until_ms,"playerId":value.player_id,"sequence":value.sequence,
        "startedAtMs":value.started_at_ms,"text":value.text,
    })
}

fn assert_source(actual: &Value, expected: &Value) {
    match (actual, expected) {
        (Value::Number(actual), Value::Number(expected)) => {
            assert_eq!(actual.as_f64(), expected.as_f64());
        }
        (Value::Array(actual), Value::Array(expected)) => {
            assert_eq!(actual.len(), expected.len());
            for (actual, expected) in actual.iter().zip(expected) {
                assert_source(actual, expected);
            }
        }
        (Value::Object(actual), Value::Object(expected)) => {
            assert_eq!(actual.len(), expected.len());
            for (key, expected) in expected {
                assert_source(&actual[key], expected);
            }
        }
        _ => assert_eq!(actual, expected),
    }
}

#[test]
fn nameplate_and_speech_layouts_match_the_source_bitmap_metrics() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/world-labels-goldens.json")).unwrap();
    for case in fixture["plates"].as_array().unwrap() {
        let actual = world_nameplate_visual_layout(case["name"].as_str().unwrap());
        assert_source(&json!(actual.width), &case["width"]);
        assert_source(&json!(actual.glyph_offset.x), &case["glyphOffsetX"]);
        assert_source(&json!(actual.glyph_offset.y), &case["glyphOffsetY"]);
        assert_source(&json!(actual.cap_box[0]), &case["capBox"]["top"]);
        assert_source(&json!(actual.cap_box[1]), &case["capBox"]["bottom"]);
        assert_source(&json!(actual.glyph_bounds[0]), &case["glyphBounds"]["left"]);
        assert_source(&json!(actual.glyph_bounds[1]), &case["glyphBounds"]["right"]);
        assert_source(&glyphs(&actual.glyphs), &case["glyphs"]);
    }
    for case in fixture["messages"].as_array().unwrap() {
        let actual = layout_native_world_speech(case["text"].as_str().unwrap());
        assert_source(&json!(actual.width), &case["width"]);
        assert_source(&json!(actual.height), &case["height"]);
        assert_source(&json!(actual.glyph_count), &case["glyphCount"]);
        let actual = actual.lines.iter().map(|line| {
            json!({"text":line.text,"advance":line.advance,"glyphs":glyphs(&line.glyphs)})
        }).collect::<Vec<_>>();
        assert_source(&json!(actual), &case["lines"]);
    }
    for case in fixture["health"].as_array().unwrap() {
        assert_source(
            &json!(native_world_nameplate_health_ratio(
                case["current"].as_f64().unwrap(),
                case["maximum"].as_f64().unwrap(),
            )),
            &case["expected"],
        );
    }
    for case in fixture["projections"].as_array().unwrap() {
        let transform = &case["transform"];
        let viewport = &case["viewport"];
        let actual = project_native_world_point(
            point(&case["point"]),
            point(&transform["position"]),
            transform["scale"].as_f64().unwrap(),
            Vec2::new(viewport["width"].as_f64().unwrap(), viewport["height"].as_f64().unwrap()),
        );
        assert_source(
            &json!(actual.map(|point| json!({"x":point.x,"y":point.y}))),
            &case["expected"],
        );
    }
}

#[test]
fn accepted_world_speech_replaces_speakers_and_keeps_source_hold_fade_and_limit() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/world-labels-goldens.json")).unwrap();
    let mut speeches = Vec::new();
    for case in fixture["journey"].as_array().unwrap() {
        let source = &case["message"];
        let sender = &source["sender"];
        let message = GameChatMessage {
            activity: None,
            channel: match source["channel"].as_str().unwrap() {
                "global" => GameChatChannel::Global,
                "party" => GameChatChannel::Party,
                "whisper" => GameChatChannel::Whisper,
                other => panic!("unexpected source channel {other}"),
            },
            recipient: None,
            sender: GameChatSender {
                display_name: sender["displayName"].as_str().unwrap().into(),
                player_id: sender["playerId"].as_str().unwrap().into(),
                player_reference: sender["playerReference"].as_str().unwrap().into(),
            },
            sequence: source["sequence"].as_u64().unwrap(),
            text: source["text"].as_str().unwrap().into(),
        };
        assert_eq!(
            append_game_world_speech(
                &mut speeches,
                &message,
                case["receivedAtMs"].as_f64().unwrap()
            ),
            case["accepted"].as_bool().unwrap(),
        );
        if let Some(expected) = case.get("speeches") {
            assert_source(&json!(speeches.iter().map(speech).collect::<Vec<_>>()), expected);
        }
    }
    for case in fixture["samples"].as_array().unwrap() {
        let source = &case["speech"];
        let value = GameWorldSpeech {
            channel: GameChatChannel::Whisper,
            expires_at_ms: source["expiresAtMs"].as_f64().unwrap(),
            hold_until_ms: source["holdUntilMs"].as_f64().unwrap(),
            player_id: source["playerId"].as_str().unwrap().into(),
            sequence: source["sequence"].as_u64().unwrap(),
            started_at_ms: source["startedAtMs"].as_f64().unwrap(),
            text: source["text"].as_str().unwrap().into(),
        };
        let actual = sample_game_world_speech_alpha(&value, case["nowMs"].as_f64().unwrap());
        assert_source(
            &json!(actual),
            &case["expected"].get("alpha").cloned().unwrap_or(Value::Null),
        );
    }
}
