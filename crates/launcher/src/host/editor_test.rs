//! Disposable native geometry admission from `host/editor-test-boneyard.ts`.
//! This accepts an editor document, not a published package or saved wizard.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use solomon_assets::boneyard::{ModBoneyardEntry, boneyard_scene_source_value};
use solomon_modding::assets::sha256;
use solomon_modding::definition::scripts::trim_javascript;
use solomon_sim::modding::LuaValue;

pub const MAX_EDITOR_TEST_BYTES: usize = 8 * 1024 * 1024;
pub const EDITOR_TEST_BONEYARD_ID: &str = "editor-test";
pub const EDITOR_TEST_REQUEST_ERROR: &str =
    "The Boneyard could not be tested. Check its native file, bounds and spawn.";

pub fn materialize_editor_test_boneyard(value: &LuaValue) -> Result<ModBoneyardEntry, String> {
    let body = value.as_object().ok_or("Choose a Boneyard document to test.")?;
    let invalid_file = "The test Boneyard must be a valid native file smaller than 8 MiB.";
    let name = body.get("name").and_then(LuaValue::as_str).ok_or(invalid_file)?;
    let encoded = body.get("bytesBase64").and_then(LuaValue::as_str).ok_or(invalid_file)?;
    let encoded_length = encoded.encode_utf16().count();
    if trim_javascript(name).is_empty()
        || name.encode_utf16().count() > 160
        || encoded.is_empty()
        || encoded_length > MAX_EDITOR_TEST_BYTES.div_ceil(3) * 4
        || !encoded_length.is_multiple_of(4)
    {
        return Err(invalid_file.into());
    }
    let invalid_encoding = "The test Boneyard encoding is invalid.";
    let bytes = STANDARD.decode(encoded).map_err(|_| invalid_encoding)?;
    if bytes.len() > MAX_EDITOR_TEST_BYTES || STANDARD.encode(&bytes) != encoded {
        return Err(invalid_encoding.into());
    }
    let mut entry = solomon_modding::package::project_mod_boneyard(
        EDITOR_TEST_BONEYARD_ID,
        "Private editor test",
        "data/levels/editor-test.boneyard",
        &bytes,
    )?;
    let bounds = entry.scene.bounds;
    let spawn = entry.scene.spawn;
    if ![
        bounds.x,
        bounds.y,
        bounds.width,
        bounds.height,
        spawn.position.x,
        spawn.position.y,
        spawn.facing_degrees,
    ]
    .iter()
    .all(|number| number.is_finite())
        || bounds.width <= 0.0
        || bounds.height <= 0.0
        || bounds.width > 131_072.0
        || bounds.height > 131_072.0
    {
        return Err("Choose finite Boneyard bounds and a player spawn before testing.".into());
    }
    let name = trim_javascript(name);
    entry.choice.id = EDITOR_TEST_BONEYARD_ID.into();
    entry.choice.name = name.into();
    entry.scene.name = name.into();
    entry.geometry_sha256 = sha256(
        solomon_assets::json_value::stringify(&boneyard_scene_source_value(&entry.scene))?
            .as_bytes(),
    );
    Ok(entry)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn expand(value: &Value, binaries: &Value) -> Value {
        match value {
            Value::Array(values) => {
                Value::Array(values.iter().map(|value| expand(value, binaries)).collect())
            }
            Value::Object(fields) => {
                if let Some(binary) = fields.get("binary") {
                    return binaries[binary.as_str().unwrap()].clone();
                }
                if let Some(length) = fields.get("zeroBytes") {
                    return Value::String(
                        STANDARD.encode(vec![0; length.as_u64().unwrap() as usize]),
                    );
                }
                Value::Object(
                    fields
                        .iter()
                        .map(|(key, value)| (key.clone(), expand(value, binaries)))
                        .collect(),
                )
            }
            value => value.clone(),
        }
    }

    #[test]
    fn editor_native_admission_matches_current_source_documents() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../sim/tests/fixtures/website/editor-test-goldens.json"
        ))
        .unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let input =
                serde_json::from_value(expand(&case["input"], &fixture["binaries"])).unwrap();
            let actual = materialize_editor_test_boneyard(&input);
            assert_eq!(
                actual.is_ok(),
                case["accepted"].as_bool().unwrap(),
                "{}: {actual:?}",
                case["name"]
            );
            let Ok(entry) = actual else { continue };
            let actual: LuaValue = serde_json::from_value(json!({
                "choice": { "id":entry.choice.id,"name":entry.choice.name,"source":"mod",
                    "modId":entry.choice.mod_id,"modName":entry.choice.mod_name },
                "geometrySha256":entry.geometry_sha256,"sourceSha256":entry.source_sha256,
                "scene":boneyard_scene_source_value(&entry.scene)
            }))
            .unwrap();
            let expected: LuaValue = serde_json::from_value(case["entry"].clone()).unwrap();
            assert_eq!(actual, expected, "{}", case["name"]);
        }
    }
}
