//! Actual portable/Website conversion, binary projection and file-selection contracts.

#[path = "../../sim/tests/support/mod.rs"]
mod support;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Value, json};
use solomon_assets::native_save::{
    GameSaveImportSource, GameSaveInputFile, WEB_GAME_SAVE_SUPPORT_ARCHIVE_PATH,
    create_web_game_save_from_portable_profile, encode_portable_game_profile,
    export_web_game_save_to_native_archive, parse_portable_game_profile,
    read_game_save_file_selection, read_native_save_archive,
};

fn fixture() -> Value {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/native-save-runtime-goldens.json"
    ))
    .unwrap();
    assert_eq!(fixture["sourceRevision"], "fa08ae249ff5563570bdb2a653e2ef9358ce7b83");
    fixture
}

fn assert_source(actual: &Value, expected: &Value, path: &str) {
    support::assert_website_json(actual, expected, path, &[]);
}

fn bytes(value: &Value) -> Vec<u8> {
    STANDARD.decode(value.as_str().unwrap()).unwrap()
}

fn assert_bytes(actual: &[u8], expected: &[u8], path: &str) {
    assert_eq!(actual.len(), expected.len(), "{path}.length");
    if let Some(index) = actual.iter().zip(expected).position(|(left, right)| left != right) {
        assert_eq!(actual[index], expected[index], "{path}.bytes[{index}]");
    }
}

#[test]
fn portable_imports_create_the_source_settled_hub_and_warnings() {
    let fixture = fixture();
    for case in fixture["imports"].as_array().unwrap() {
        let result = parse_portable_game_profile(case["input"].as_str().unwrap())
            .and_then(|profile| create_web_game_save_from_portable_profile(&profile));
        if let Some(error) = case["error"].as_str() {
            assert_eq!(result.unwrap_err(), error, "{}", case["name"]);
            continue;
        }
        let result = result.unwrap_or_else(|error| panic!("{}: {error}", case["name"]));
        let actual = json!({
            "character": {
                "discipline": result.character.discipline.as_str(),
                "displayName": result.character.name,
                "element": result.character.element.as_str(),
            },
            "document": serde_json::from_str::<Value>(&result.document).unwrap(),
            "playerId": result.player_id,
            "warnings": result.warnings,
        });
        assert_source(&actual, &case["output"], case["name"].as_str().unwrap());
    }
}

#[test]
fn native_exports_preserve_exact_disk_bytes_and_full_browser_support_state() {
    let fixture = fixture();
    for case in fixture["exports"].as_array().unwrap() {
        let result = export_web_game_save_to_native_archive(case["input"].as_str().unwrap());
        if let Some(error) = case["error"].as_str() {
            assert_eq!(result.unwrap_err(), error, "{}", case["name"]);
            continue;
        }
        let result = result.unwrap_or_else(|error| panic!("{}: {error}", case["name"]));
        let native = read_native_save_archive(&result.archive).unwrap();
        assert_bytes(&native.darkdata, &bytes(&case["native"]["darkdata"]), "export.darkdata");
        assert_bytes(&native.gamestate, &bytes(&case["native"]["gamestate"]), "export.gamestate");
        assert_eq!(native.run_name, case["native"]["runName"].as_str().unwrap());
        let support: Vec<_> = native
            .retained_files
            .iter()
            .filter(|file| file.path == WEB_GAME_SAVE_SUPPORT_ARCHIVE_PATH)
            .collect();
        assert_eq!(support.len(), 1, "{} support document count", case["name"]);
        assert_source(
            &serde_json::from_slice(&support[0].bytes).unwrap(),
            &case["support"],
            case["name"].as_str().unwrap(),
        );
        let others: Vec<_> = native
            .retained_files
            .iter()
            .filter(|file| file.path != WEB_GAME_SAVE_SUPPORT_ARCHIVE_PATH)
            .collect();
        let expected_files = case["native"]["retainedFiles"].as_array().unwrap();
        assert_eq!(others.len(), expected_files.len(), "{} retained file count", case["name"]);
        for (actual, expected) in others.iter().zip(expected_files) {
            assert_eq!(actual.path, expected["path"].as_str().unwrap());
            assert_bytes(&actual.bytes, &bytes(&expected["bytes"]), &actual.path);
        }
        let actual_portable = encode_portable_game_profile(&result.portable).unwrap();
        let expected_portable = case["portable"].as_str().unwrap();
        assert_source(
            &serde_json::from_str(&actual_portable).unwrap(),
            &serde_json::from_str(expected_portable).unwrap(),
            "export.portable",
        );
        assert_bytes(
            actual_portable.as_bytes(),
            expected_portable.as_bytes(),
            "export.portable JSON",
        );
        assert_eq!(serde_json::to_value(result.warnings).unwrap(), case["warnings"]);
    }
}

#[test]
fn native_file_selection_admission_and_metadata_match_the_source() {
    let fixture = fixture();
    for case in fixture["selections"].as_array().unwrap() {
        let inputs = case["input"].as_array().unwrap();
        let contents: Vec<Vec<u8>> = inputs.iter().map(|input| bytes(&input["bytes"])).collect();
        let files: Vec<_> = inputs
            .iter()
            .zip(&contents)
            .map(|(input, bytes)| GameSaveInputFile {
                name: input["name"].as_str().unwrap(),
                relative_path: input["relativePath"].as_str().unwrap(),
                bytes,
            })
            .collect();
        let result = read_game_save_file_selection(&files);
        if let Some(error) = case["error"].as_str() {
            // The format is shared by native and browser clients. Keep the
            // captured data unchanged while checking the current platform-neutral copy.
            let expected = error.replace("Choose a browser save JSON", "Choose a game save JSON");
            assert_eq!(result.unwrap_err(), expected, "{}", case["name"]);
            continue;
        }
        let result = result.unwrap_or_else(|error| panic!("{}: {error}", case["name"]));
        let actual = json!({
            "discipline": result.discipline.as_str(), "displayName": result.display_name,
            "document": serde_json::from_str::<Value>(&result.document).unwrap(),
            "element": result.element.as_str(), "gold": result.gold,
            "hagathaPerks": result.hagatha_perks, "learnedRows": result.learned_rows,
            "level": result.level,
            "source": if result.source == GameSaveImportSource::Stock { "stock" } else { "browser" },
            "warnings": result.warnings,
        });
        assert_source(&actual, &case["output"], case["name"].as_str().unwrap());
    }
}
