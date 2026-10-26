//! Browser account save tools from Website `save/game-save-files.ts` and
//! `save/game-save-portability.ts`. The canonical asset readers own validation and
//! conversion; this module owns browser Files and JavaScript result objects.
//!
//! Importing this independent module has no game startup hook. Account controls
//! explicitly call the two asynchronous operations after initializing the WASM module.

use js_sys::{Array, Error, Object, Reflect, Uint8Array};
use solomon_assets::native_save::{
    GameSaveImportPreview, GameSaveImportSource, GameSaveInputFile, encode_portable_game_profile,
};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::File;

struct ReadSaveFile {
    name: String,
    relative_path: String,
    bytes: Vec<u8>,
}

fn save_error(message: String) -> JsValue {
    Error::new(&message).into()
}

fn warnings_array(warnings: &[String]) -> Array {
    let array = Array::new();
    for warning in warnings {
        array.push(&JsValue::from_str(warning));
    }
    array
}

fn browser_preview(preview: &GameSaveImportPreview) -> Result<JsValue, JsValue> {
    let object = Object::new();
    let source = match preview.source {
        GameSaveImportSource::Game => "browser",
        GameSaveImportSource::Stock => "stock",
    };
    for (key, value) in [
        ("discipline", JsValue::from_str(preview.discipline.as_str())),
        ("displayName", JsValue::from_str(&preview.display_name)),
        ("document", JsValue::from_str(&preview.document)),
        ("element", JsValue::from_str(preview.element.as_str())),
        ("gold", JsValue::from_f64(preview.gold as f64)),
        ("hagathaPerks", JsValue::from_f64(preview.hagatha_perks as f64)),
        ("learnedRows", JsValue::from_f64(preview.learned_rows as f64)),
        ("level", JsValue::from_f64(f64::from(preview.level))),
        ("source", JsValue::from_str(source)),
        ("warnings", warnings_array(&preview.warnings).into()),
    ] {
        Reflect::set(&object, &JsValue::from_str(key), &value)?;
    }
    Ok(Object::freeze(&object).into())
}

/// Accept the Account input's FileList or File[] and preserve directory-relative run names.
#[wasm_bindgen(js_name = readGameSaveFileSelection)]
pub async fn read_game_save_file_selection(selection: JsValue) -> Result<JsValue, JsValue> {
    let iterator = js_sys::try_iter(&selection)?
        .ok_or_else(|| save_error("Save selection must contain browser Files.".into()))?;
    let mut files = Vec::new();
    for value in iterator {
        let file = value?
            .dyn_into::<File>()
            .map_err(|_| save_error("Save selection must contain browser Files.".into()))?;
        files.push(file);
    }
    // Source checks a JSON File's size before reading its ArrayBuffer. The asset
    // reader repeats the byte-length check once the browser has supplied its bytes.
    if files.len() == 1
        && files[0].name().to_lowercase().ends_with(".json")
        && files[0].size() > solomon_assets::website_save::MAX_SAVE_BYTES as f64
    {
        return Err(save_error("The save is too large.".into()));
    }
    let mut read_files = Vec::with_capacity(files.len());
    for file in files {
        let name = file.name();
        let relative_path = Reflect::get(file.as_ref(), &JsValue::from_str("webkitRelativePath"))?
            .as_string()
            .unwrap_or_default();
        let buffer = JsFuture::from(file.array_buffer()).await?;
        read_files.push(ReadSaveFile {
            name,
            relative_path,
            bytes: Uint8Array::new(&buffer).to_vec(),
        });
    }
    let inputs: Vec<_> = read_files
        .iter()
        .map(|file| GameSaveInputFile {
            name: &file.name,
            relative_path: &file.relative_path,
            bytes: &file.bytes,
        })
        .collect();
    let preview =
        solomon_assets::native_save::read_game_save_file_selection(&inputs).map_err(save_error)?;
    browser_preview(&preview)
}

/// Export archive bytes and Source warnings; the Account owns confirmation and download.
#[wasm_bindgen(js_name = exportWebGameSaveToNativeArchive)]
pub async fn export_web_game_save_to_native_archive(document: String) -> Result<JsValue, JsValue> {
    let exported = solomon_assets::native_save::export_web_game_save_to_native_archive(&document)
        .map_err(save_error)?;
    let portable = encode_portable_game_profile(&exported.portable).map_err(save_error)?;
    let object = Object::new();
    for (key, value) in [
        ("archive", Uint8Array::from(exported.archive.as_slice()).into()),
        ("portable", js_sys::JSON::parse(&portable)?),
        ("warnings", warnings_array(&exported.warnings).into()),
    ] {
        Reflect::set(&object, &JsValue::from_str(key), &value)?;
    }
    Ok(Object::freeze(&object).into())
}
