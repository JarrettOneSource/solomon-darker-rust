//! Actual provisioner packages replay source admission, bytes and scene identity.

mod common;

use base64::{Engine, engine::general_purpose::STANDARD};
use serde::Deserialize;
use serde_json::{Value, json};
use solomon_modding::package::{
    MaterializedWebSessionContent, materialize_web_session_content,
    materialize_web_session_content_value,
};
use solomon_sim::modding::LuaValue;
use solomon_sim::modding::identity::WebSessionContentPayload;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Fixture {
    boneyard_base64: String,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    input: LuaValue,
    output: Option<Value>,
    error: Option<String>,
}

fn expand(value: &mut LuaValue, bytes: &str) {
    match value {
        LuaValue::String(text) if text == "$BONEYARD" => *text = bytes.to_owned(),
        LuaValue::String(text) if text == "$FILE_LIMIT" || text == "$FILE_OVERFLOW" => {
            let length = 16 * 1_024 * 1_024 + usize::from(text == "$FILE_OVERFLOW");
            *text = STANDARD.encode(vec![97; length]);
        }
        LuaValue::Array(values) => {
            for value in values {
                expand(value, bytes);
            }
        }
        LuaValue::Object(object) => {
            let previous = std::mem::take(object);
            for (key, mut value) in previous.into_fields() {
                expand(&mut value, bytes);
                object.insert(key, value);
            }
        }
        _ => {}
    }
}

fn project(content: &MaterializedWebSessionContent) -> Value {
    let boneyards: Vec<_> = content
        .boneyards
        .iter()
        .map(|entry| {
            json!({
                "choice":entry.choice,
                "geometrySha256":entry.geometry_sha256,
                "scene":solomon_assets::boneyard::boneyard_scene_source_value(&entry.scene),
                "sourceSha256":entry.source_sha256,
            })
        })
        .collect();
    json!({
        "assets":content.assets,
        "boneyards":boneyards,
        "compiledMods":content.compiled_mods,
        "manifest":content.manifest,
        "modSources":content.mod_sources,
        "summary":content.summary,
    })
}

#[test]
fn provisioned_packages_retain_typed_assets_source_bytes_and_scene_identity() {
    let fixture: Fixture = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/mod-package-goldens.json"
    ))
    .unwrap();
    for mut case in fixture.cases {
        expand(&mut case.input, &fixture.boneyard_base64);
        match materialize_web_session_content_value(&case.input) {
            Ok(actual) => {
                assert!(case.error.is_none(), "{} unexpectedly accepted", case.name);
                common::assert_json_values(&project(&actual), &case.output.unwrap(), &case.name);
                let input = solomon_assets::json_value::stringify(&case.input).unwrap();
                let typed: WebSessionContentPayload = serde_json::from_str(&input).unwrap();
                assert_eq!(materialize_web_session_content(&typed).unwrap(), actual);
            }
            Err(error) => assert_eq!(Some(error), case.error, "{}", case.name),
        }
    }
}
