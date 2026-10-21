//! The original controlled native writer output used for a fresh stock export.

use serde_json::Value;

use super::archive::{NativeSaveArchive, run_name_is_valid};
use super::bridge::{
    NATIVE_SOURCE_ATTACHMENT_MAX_BYTES, decode_native_darkdata_profile,
    decode_native_gamestate_boast, decode_native_gamestate_wizard,
};
use super::decode_base64;

pub fn decode_native_hub_template(document: &str) -> Result<NativeSaveArchive, String> {
    let value: Value =
        serde_json::from_str(document).map_err(|_| "native Hub template is not valid JSON")?;
    let file_bytes = |key: &str, claim: &str| -> Result<Vec<u8>, String> {
        let text = value["files"][key]["base64"]
            .as_str()
            .filter(|text| !text.is_empty())
            .ok_or_else(|| format!("{claim} is missing"))?;
        decode_base64(text, claim)
    };
    let darkdata = file_bytes("darkdata", "native darkdata template")?;
    let gamestate = file_bytes("gamestate", "native gamestate template")?;
    if value["files"]["darkdata"]["bytes"].as_f64() != Some(darkdata.len() as f64)
        || value["files"]["gamestate"]["bytes"].as_f64() != Some(gamestate.len() as f64)
        || darkdata.len() + gamestate.len() > NATIVE_SOURCE_ATTACHMENT_MAX_BYTES
    {
        return Err("native Hub template byte contract is invalid".into());
    }
    let run_name = value["expected"]["runName"]
        .as_str()
        .filter(|name| run_name_is_valid(name))
        .ok_or("native Hub template run name is invalid")?;
    decode_native_darkdata_profile(&darkdata)?;
    decode_native_gamestate_wizard(&gamestate)?;
    decode_native_gamestate_boast(&gamestate)?;
    Ok(NativeSaveArchive {
        darkdata,
        gamestate,
        retained_files: Vec::new(),
        run_name: run_name.to_owned(),
    })
}

pub fn load_embedded_native_hub_template() -> Result<NativeSaveArchive, String> {
    decode_native_hub_template(include_str!(
        "../../../../web/assets/native/portable-profile-template.json"
    ))
}
