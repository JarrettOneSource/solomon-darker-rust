//! Disk bytes and trust-boundary behavior from the pinned Website save codecs.

#[path = "../../sim/tests/support/mod.rs"]
mod support;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Value, json};
use solomon_assets::native_save::{
    NativeArchiveFile, NativeChunkNode, NativeGameBoastState, NativeProfilePatch,
    NativeSaveArchive, NativeSyncBuffer, NativeWizardPatch, create_native_save_archive,
    create_portable_game_profile_from_native, create_stored_zip, decode_native_darkdata,
    decode_native_darkdata_profile, decode_native_gamestate_boast, decode_native_gamestate_wizard,
    decode_native_hub_template, encode_native_darkdata, encode_native_sync_buffer,
    encode_portable_game_profile, load_embedded_native_hub_template, native_source_bytes,
    parse_native_sync_buffer, parse_portable_game_profile, patch_native_darkdata,
    patch_native_gamestate, read_native_save_archive, read_zip, validate_native_game_save_source,
};

fn bytes(value: &Value) -> Vec<u8> {
    STANDARD.decode(value.as_str().unwrap()).unwrap()
}

fn assert_source(actual: Value, expected: &Value, path: &str) {
    // JSON.stringify writes 50 for a native f32 widened to 50.0. Compare values
    // exactly, while the shared adapter reports only the first differing field.
    support::assert_website_json(&actual, expected, path, &[]);
}

fn assert_bytes(actual: &[u8], expected: &[u8], path: &str) {
    assert_eq!(actual.len(), expected.len(), "{path}.length");
    if let Some(index) =
        actual.iter().zip(expected).position(|(actual, expected)| actual != expected)
    {
        assert_eq!(actual[index], expected[index], "{path}.bytes[{index}]");
    }
}

fn archive_files(value: &Value) -> Vec<NativeArchiveFile> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|file| NativeArchiveFile {
            bytes: bytes(&file["bytes"]),
            path: file["path"].as_str().unwrap().to_owned(),
        })
        .collect()
}

fn archive_files_json(files: &[NativeArchiveFile]) -> Value {
    json!(
        files
            .iter()
            .map(|file| json!({ "bytes": STANDARD.encode(&file.bytes), "path": file.path }))
            .collect::<Vec<_>>()
    )
}

fn native_archive(value: &Value) -> NativeSaveArchive {
    NativeSaveArchive {
        darkdata: bytes(&value["darkdata"]),
        gamestate: bytes(&value["gamestate"]),
        retained_files: archive_files(&value["retainedFiles"]),
        run_name: value["runName"].as_str().unwrap().to_owned(),
    }
}

#[test]
fn native_zip_output_deflate_input_and_manifest_rejections_match_the_source() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/native-save-archive-goldens.json"
    ))
    .unwrap();
    for case in fixture["zipCases"].as_array().unwrap() {
        let files = archive_files(&case["files"]);
        let zip = create_stored_zip(&files).unwrap();
        assert_bytes(&zip, &bytes(&case["zip"]), case["name"].as_str().unwrap());
        assert_eq!(read_zip(&zip).unwrap(), files);
    }
    for case in fixture["nativeCases"].as_array().unwrap() {
        let save = native_archive(&case["save"]);
        let zip = create_native_save_archive(&save, case["slot"].as_u64().unwrap() as u32).unwrap();
        assert_bytes(&zip, &bytes(&case["zip"]), "native launcher ZIP");
        assert_eq!(read_native_save_archive(&zip).unwrap(), save);
    }
    for case in fixture["compressedCases"].as_array().unwrap() {
        let files = read_zip(&bytes(&case["zip"])).unwrap();
        assert_eq!(archive_files_json(&files), case["files"], "{}", case["name"]);
    }
    for case in fixture["errors"].as_array().unwrap() {
        let error = match case["operation"].as_str().unwrap() {
            "zip" => read_zip(&bytes(&case["input"])).unwrap_err(),
            "native" => read_native_save_archive(&bytes(&case["input"])).unwrap_err(),
            "create" => create_stored_zip(&archive_files(&case["files"])).unwrap_err(),
            "createNative" => create_native_save_archive(
                &native_archive(&case["save"]),
                case["slot"].as_u64().unwrap() as u32,
            )
            .unwrap_err(),
            other => panic!("unknown source archive operation {other}"),
        };
        assert_eq!(error, case["error"].as_str().unwrap(), "{}", case["name"]);
    }
}

#[test]
fn native_profile_wizard_and_boast_patches_match_the_complete_original_disk_bytes() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/native-save-bridge-goldens.json"
    ))
    .unwrap();
    let darkdata = bytes(&fixture["base"]["darkdata"]);
    let gamestate = bytes(&fixture["base"]["gamestate"]);
    assert_source(
        serde_json::to_value(decode_native_darkdata_profile(&darkdata).unwrap()).unwrap(),
        &fixture["base"]["profile"],
        "base.profile",
    );
    assert_source(
        serde_json::to_value(decode_native_gamestate_wizard(&gamestate).unwrap()).unwrap(),
        &fixture["base"]["wizard"],
        "base.wizard",
    );
    assert_source(
        serde_json::to_value(decode_native_gamestate_boast(&gamestate).unwrap()).unwrap(),
        &fixture["base"]["boast"],
        "base.boast",
    );
    for case in fixture["profiles"].as_array().unwrap() {
        let patch: NativeProfilePatch = serde_json::from_value(case["patch"].clone()).unwrap();
        let output = patch_native_darkdata(&darkdata, &patch).unwrap();
        assert_eq!(output, bytes(&case["output"]), "{}", case["name"]);
        assert_source(
            serde_json::to_value(decode_native_darkdata_profile(&output).unwrap()).unwrap(),
            &case["decoded"],
            case["name"].as_str().unwrap(),
        );
    }
    for case in fixture["wizards"].as_array().unwrap() {
        let patch: NativeWizardPatch = serde_json::from_value(case["patch"].clone()).unwrap();
        let boast: Option<NativeGameBoastState> =
            serde_json::from_value(case["boast"].clone()).unwrap();
        let input = if case["input"].is_null() { gamestate.clone() } else { bytes(&case["input"]) };
        let output = patch_native_gamestate(&input, &patch, boast.as_ref()).unwrap();
        assert_eq!(output, bytes(&case["output"]), "{}", case["name"]);
        assert_source(
            serde_json::to_value(decode_native_gamestate_wizard(&output).unwrap()).unwrap(),
            &case["decoded"],
            case["name"].as_str().unwrap(),
        );
        assert_source(
            serde_json::to_value(decode_native_gamestate_boast(&output).unwrap()).unwrap(),
            &case["decodedBoast"],
            case["name"].as_str().unwrap(),
        );
    }
    for case in fixture["errors"].as_array().unwrap() {
        let input = bytes(&case["input"]);
        let error = match case["operation"].as_str().unwrap() {
            "profile" => decode_native_darkdata_profile(&input).unwrap_err(),
            "wizard" => decode_native_gamestate_wizard(&input).unwrap_err(),
            "boast" => decode_native_gamestate_boast(&input).unwrap_err(),
            "patchProfile" => {
                let patch = serde_json::from_value(case["patch"].clone()).unwrap();
                patch_native_darkdata(&input, &patch).unwrap_err()
            }
            "patchWizard" => {
                let patch = serde_json::from_value(case["patch"].clone()).unwrap();
                let boast: Option<NativeGameBoastState> =
                    serde_json::from_value(case["boast"].clone()).unwrap();
                patch_native_gamestate(&input, &patch, boast.as_ref()).unwrap_err()
            }
            other => panic!("unknown source operation {other}"),
        };
        assert_eq!(error, case["error"].as_str().unwrap(), "{}", case["name"]);
    }
}

fn chunk_json(node: &NativeChunkNode) -> Value {
    json!({
        "children": node.children.iter().map(chunk_json).collect::<Vec<_>>(),
        "offset": node.offset,
        "payload": STANDARD.encode(&node.payload),
        "payloadOffset": node.payload_offset,
    })
}

fn buffer_json(buffer: &NativeSyncBuffer) -> Value {
    json!({
        "endOffset": buffer.end_offset,
        "namedBuffers": buffer.named_buffers.iter().map(|item| json!({
            "buffer": buffer_json(&item.buffer), "name": item.name,
            "nameOffset": item.name_offset,
        })).collect::<Vec<_>>(),
        "offset": buffer.offset, "root": chunk_json(&buffer.root),
    })
}

#[test]
fn native_disk_codecs_match_original_bytes_trees_and_malformed_input_errors() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/native-save-codec-goldens.json"
    ))
    .unwrap();
    for case in fixture["syncCases"].as_array().unwrap() {
        let raw = bytes(&case["raw"]);
        let darkdata = bytes(&case["darkdata"]);
        let parsed = parse_native_sync_buffer(&raw).unwrap();
        assert_eq!(buffer_json(&parsed), case["parsed"], "{}", case["name"]);
        assert_eq!(encode_native_sync_buffer(&parsed).unwrap(), raw, "{}", case["name"]);
        assert_eq!(encode_native_darkdata(&parsed).unwrap(), darkdata, "{}", case["name"]);
        assert_eq!(decode_native_darkdata(&darkdata).unwrap(), parsed, "{}", case["name"]);
    }
    for case in fixture["errors"].as_array().unwrap() {
        let raw = bytes(&case["raw"]);
        let result = if case["kind"] == "sync" {
            parse_native_sync_buffer(&raw)
        } else {
            decode_native_darkdata(&raw)
        };
        assert_eq!(result.unwrap_err(), case["error"].as_str().unwrap(), "{}", case["name"]);
    }
}

fn portable_case_document(base: &str, case: &Value) -> String {
    if let Some(document) = case["document"].as_str() {
        return document.to_owned();
    }
    let mut value: Value = serde_json::from_str(base).unwrap();
    for change in case["changes"].as_array().unwrap() {
        let path = change["path"].as_array().unwrap();
        let mut owner = &mut value;
        for key in &path[..path.len() - 1] {
            owner = if let Some(key) = key.as_str() {
                &mut owner[key]
            } else {
                &mut owner[key.as_u64().unwrap() as usize]
            };
        }
        let key = path.last().unwrap();
        if change["remove"] == true {
            owner.as_object_mut().unwrap().remove(key.as_str().unwrap());
        } else if let Some(key) = key.as_str() {
            owner[key] = change["value"].clone();
        } else {
            owner[key.as_u64().unwrap() as usize] = change["value"].clone();
        }
    }
    serde_json::to_string(&value).unwrap()
}

#[test]
fn portable_profiles_attachments_and_template_match_source_bytes_and_validation() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/portable-game-profile-goldens.json"
    ))
    .unwrap();
    assert_eq!(fixture["sourceRevision"], "fa08ae249ff5563570bdb2a653e2ef9358ce7b83");
    for case in fixture["creates"].as_array().unwrap() {
        let save = native_archive(&case["input"]);
        let warnings: Vec<String> = serde_json::from_value(case["warnings"].clone()).unwrap();
        let result = create_portable_game_profile_from_native(&save, &warnings);
        if let Some(error) = case["error"].as_str() {
            assert_eq!(result.unwrap_err(), error, "{}", case["name"]);
            continue;
        }
        let profile = result.unwrap();
        assert_eq!(
            encode_portable_game_profile(&profile).unwrap(),
            case["encoded"].as_str().unwrap(),
            "{}",
            case["name"]
        );
        validate_native_game_save_source(&profile.native_source).unwrap();
        assert_eq!(native_source_bytes(&profile.native_source).unwrap(), save);
    }
    for case in fixture["cases"].as_array().unwrap() {
        let document = portable_case_document(fixture["baseDocument"].as_str().unwrap(), case);
        let result = parse_portable_game_profile(&document);
        if let Some(error) = case["error"].as_str() {
            match result {
                Err(actual) => assert_eq!(actual, error, "{}", case["name"]),
                Ok(_) => panic!("{}: expected Source rejection: {error}", case["name"]),
            }
        } else {
            let profile = result.unwrap();
            let expected = case["encoded"].as_str().unwrap();
            assert_source(
                serde_json::to_value(&profile).unwrap(),
                &serde_json::from_str(expected).unwrap(),
                case["name"].as_str().unwrap(),
            );
            assert_eq!(
                encode_portable_game_profile(&profile).unwrap(),
                expected,
                "{}",
                case["name"]
            );
        }
    }
    for case in fixture["templateCases"].as_array().unwrap() {
        let result = decode_native_hub_template(&serde_json::to_string(&case["input"]).unwrap());
        if let Some(error) = case["error"].as_str() {
            assert_eq!(result.unwrap_err(), error, "{}", case["name"]);
        } else {
            let expected = native_archive(&case["output"]);
            assert_eq!(result.unwrap(), expected, "{}", case["name"]);
            assert_eq!(load_embedded_native_hub_template().unwrap(), expected);
        }
    }
}
