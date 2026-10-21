//! Admission executes actual Lua, resolves dependencies and materializes worlds.

mod common;

use serde::Deserialize;
use serde_json::{Value, json};
use solomon_modding::package::{
    compile_web_session_content_definitions, materialize_web_session_content_value,
};
use solomon_sim::modding::LuaValue;

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
        LuaValue::String(text) if text == "$BONEYARD" => *text = bytes.into(),
        LuaValue::Array(values) => {
            for value in values {
                expand(value, bytes);
            }
        }
        LuaValue::Object(fields) => {
            let previous = std::mem::take(fields);
            for (key, mut value) in previous.into_fields() {
                expand(&mut value, bytes);
                fields.insert(key, value);
            }
        }
        _ => {}
    }
}

#[test]
fn admitted_packages_execute_lua_dependencies_and_authored_scene_overrides() {
    let fixture: Fixture = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/mod-provisioning-goldens.json"
    ))
    .unwrap();
    for mut case in fixture.cases {
        expand(&mut case.input, &fixture.boneyard_base64);
        let result = materialize_web_session_content_value(&case.input)
            .and_then(compile_web_session_content_definitions);
        match result {
            Ok(content) => {
                assert!(case.error.is_none(), "{} unexpectedly accepted", case.name);
                let boneyards: Vec<_> = content.boneyards.iter().map(|entry| json!({
                    "choice":entry.choice,"geometrySha256":entry.geometry_sha256,
                    "sourceSha256":entry.source_sha256,"webLuaContentId":entry.web_lua_content_id,
                    "environmentMode":entry.scene.environment_mode,
                })).collect();
                let actual = json!({
                    "assets":content.assets,"compiledMods":content.compiled_mods,
                    "manifest":content.manifest,"summary":content.summary,"boneyards":boneyards,
                });
                common::assert_json_values(
                    &actual,
                    &case.output.expect("accepted source content"),
                    &case.name,
                );
                fn assert_send<T: Send>(_: &T) {}
                assert_send(&content);
            }
            Err(error) => assert_eq!(Some(error), case.error, "{} rejection", case.name),
        }
    }
}
