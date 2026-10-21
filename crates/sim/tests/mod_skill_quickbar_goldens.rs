//! Actual Source TSX render/click and keyboard-controller observations.

use serde_json::{Value, json};
use solomon_sim::Vec2;
use solomon_sim::modding::assets::GameModAsset;
use solomon_sim::modding::skill_quickbar::{
    ModCastAction, ModSkillQuickbarKey, ModSkillQuickbarPlayer, cast_mod_skill_quickbar_spell,
    mod_skill_quickbar_keyboard_slot, project_mod_skill_quickbar,
};
use solomon_sim::modding::{LuaObject, LuaValue};

fn golden() -> Value {
    serde_json::from_str(include_str!("fixtures/website/mod-skill-quickbar-goldens.json")).unwrap()
}

#[test]
fn mod_quickbar_rows_and_click_targets_match_the_actual_source_component() {
    for case in golden()["components"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let runtime = lua(&case["runtime"]);
        let assets: Vec<_> = case["assets"]
            .as_array()
            .unwrap()
            .iter()
            .map(|asset| GameModAsset {
                byte_length: asset["byteLength"].as_u64().unwrap(),
                content_type: asset["contentType"].as_str().unwrap().into(),
                kind: asset["kind"].as_str().unwrap().into(),
                mod_id: asset["modId"].as_str().unwrap().into(),
                path: asset["path"].as_str().unwrap().into(),
                sha256: asset["sha256"].as_str().unwrap().into(),
            })
            .collect();
        let slots = project_mod_skill_quickbar(runtime.as_object(), &assets);
        assert_eq!(!slots.is_empty(), case["visible"].as_bool().unwrap(), "{name}");
        let rows: Vec<_> = slots
            .iter()
            .map(|slot| {
                json!({
                    "slot": slot.slot,
                    "label": slot.slot + 1,
                    "title": format!("{} (Shift+{})", slot.name, slot.slot + 1),
                    "image": slot.image.map(|image| format!("/api/game/content/{}", image.sha256)),
                    "cast": cast_json(cast_mod_skill_quickbar_spell(slot.content_id, player(&case["player"]))),
                })
            })
            .collect();
        assert_source_values(&json!(rows), &case["slots"], name);
    }
}

#[test]
fn mod_quickbar_keyboard_admission_and_all_headings_match_source() {
    let fixture = golden();
    for case in fixture["castTargets"].as_array().unwrap() {
        assert_source_values(
            &cast_json(cast_mod_skill_quickbar_spell("spell-1", player(&case["player"]))),
            &case["cast"],
            "heading target",
        );
    }
    let actor =
        Some(ModSkillQuickbarPlayer { position: Vec2::new(10.5, -20.25), heading_index: 7 });
    for case in fixture["events"].as_array().unwrap() {
        let input = &case["input"];
        let key = ModSkillQuickbarKey {
            number: case["keyNumber"].as_f64().unwrap_or(f64::NAN),
            shift: input["shiftKey"].as_bool().unwrap_or(false),
            control: input["ctrlKey"].as_bool().unwrap_or(false),
            alt: input["altKey"].as_bool().unwrap_or(false),
            meta: input["metaKey"].as_bool().unwrap_or(false),
            repeat: input["repeat"].as_bool().unwrap_or(false),
        };
        let selected = match mod_skill_quickbar_keyboard_slot(key) {
            Some(0) => Some("spell-1"),
            Some(7) => Some("spell-8"),
            _ => None,
        };
        let prevented = input["defaultPrevented"].as_bool().unwrap_or(false) || selected.is_some();
        assert_eq!(prevented, case["defaultPrevented"].as_bool().unwrap(), "{input}");
        let actor = if input["missingPlayer"].as_bool() == Some(true) { None } else { actor };
        let cast = selected.and_then(|content_id| cast_mod_skill_quickbar_spell(content_id, actor));
        assert_source_values(&cast_json(cast), &case["cast"], &input.to_string());
    }
}

fn player(value: &Value) -> Option<ModSkillQuickbarPlayer> {
    value.as_object()?;
    Some(ModSkillQuickbarPlayer {
        position: Vec2::new(
            value["position"]["x"].as_f64().unwrap(),
            value["position"]["y"].as_f64().unwrap(),
        ),
        heading_index: value["headingIndex"].as_u64().unwrap() as usize,
    })
}

fn cast_json(cast: Option<ModCastAction>) -> Value {
    cast.map_or(Value::Null, |cast| {
        json!({"contentId": cast.content_id, "target": {"x": cast.target.x, "y": cast.target.y}})
    })
}

fn lua(value: &Value) -> LuaValue {
    match value {
        Value::Null => LuaValue::Null,
        Value::Bool(value) => LuaValue::Boolean(*value),
        Value::Number(value) => LuaValue::Number(value.as_f64().unwrap()),
        Value::String(value) => LuaValue::String(value.clone()),
        Value::Array(values) => LuaValue::Array(values.iter().map(lua).collect()),
        Value::Object(fields) => LuaValue::Object(
            fields.iter().map(|(key, value)| (key.clone(), lua(value))).collect::<LuaObject>(),
        ),
    }
}

fn assert_source_values(actual: &Value, expected: &Value, path: &str) {
    match (actual, expected) {
        (Value::Number(actual), Value::Number(expected)) => {
            assert_eq!(actual.as_f64(), expected.as_f64(), "{path}");
        }
        (Value::Array(actual), Value::Array(expected)) => {
            assert_eq!(actual.len(), expected.len(), "{path}");
            for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
                assert_source_values(actual, expected, &format!("{path}[{index}]"));
            }
        }
        (Value::Object(actual), Value::Object(expected)) => {
            assert_eq!(actual.len(), expected.len(), "{path}");
            for (key, expected) in expected {
                assert_source_values(actual.get(key).unwrap(), expected, &format!("{path}.{key}"));
            }
        }
        _ => assert_eq!(actual, expected, "{path}"),
    }
}
