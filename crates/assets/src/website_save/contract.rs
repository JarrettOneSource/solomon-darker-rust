//! The public envelope and validation from Website `save/game-save-contract.ts`.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use solomon_sim::game_run::GameRunPhase;
use solomon_sim::modding::LuaObject;
use solomon_sim::modding::identity::GameContentIdentity;
use solomon_sim::profile::NewGameSelection;

pub const SCHEMA_VERSION: u64 = 51;
pub const MAX_SAVE_BYTES: usize = 96 * 1024 * 1024;
pub const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SaveIntegrity {
    GlobalClean,
    LocalOnly,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SaveWorldKind {
    Hub,
    Boneyard,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SaveSummary {
    pub active_run: bool,
    pub character: NewGameSelection,
    pub party_rejoin_token: Option<String>,
    pub phase: GameRunPhase,
    pub player_id: String,
    pub saved_at_tick: u64,
    pub world_kind: SaveWorldKind,
}

pub use crate::native_save_source::{NativeSaveFile, NativeSaveSource};

#[derive(Clone, Debug)]
pub struct SaveMetadata {
    pub integrity: SaveIntegrity,
    pub mods: Vec<GameContentIdentity>,
    pub mod_state: LuaObject,
    pub native_source: Option<NativeSaveSource>,
}

#[derive(Clone, Debug)]
pub struct ParsedContinuation {
    pub loaded_boneyard: Value,
    pub simulation: Value,
    pub summary: SaveSummary,
}

#[derive(Clone, Debug)]
pub struct ParsedSaveDocument {
    pub continuation: Option<ParsedContinuation>,
    pub profile_advanced_unlocks: Option<[bool; 8]>,
    pub profile_economy: Value,
    pub profile_hagatha_runtime: Option<Value>,
    pub metadata: SaveMetadata,
    pub source_schema_version: u64,
}

pub fn parse(document: &str) -> Result<ParsedSaveDocument, String> {
    if document.is_empty() || document.len() > MAX_SAVE_BYTES {
        return Err("game save exceeds its size limit".into());
    }
    let raw: Value = serde_json::from_str(document).map_err(|_| "game save is not valid JSON")?;
    let root = record(&raw, "game save")?;
    let version = integer(
        root.get("schemaVersion").ok_or("game save schema version is invalid")?,
        "game save schema version",
    )?;
    if version == 0 {
        return Err("game save schema version is invalid".into());
    }
    let current = version >= 5;
    let mut keys = if current {
        vec!["continuation", "integrity", "mods", "modState", "profile", "schemaVersion"]
    } else {
        vec!["loadedBoneyard", "schemaVersion", "simulation", "summary"]
    };
    if version >= 17 {
        keys.push("nativeSource");
    }
    if (2..=4).contains(&version) {
        keys.extend(["mods", "modState"]);
    }
    if version == 4 {
        keys.push("integrity");
    }
    only_keys(root, "game save", &keys)?;
    let continuation = if current {
        if root["continuation"].is_null() {
            None
        } else {
            let continuation = record(&root["continuation"], "game save continuation")?;
            only_keys(
                continuation,
                "game save continuation",
                &["loadedBoneyard", "simulation", "summary"],
            )?;
            Some(parse_continuation(continuation, version)?)
        }
    } else {
        Some(parse_continuation(root, version)?)
    };
    let mut profile_advanced_unlocks = None;
    let (profile_economy, profile_hagatha_runtime) = if current {
        let profile = record(&root["profile"], "game save profile")?;
        let keys = if version >= 45 {
            &["advancedUnlocks", "economy", "hagathaRuntime"][..]
        } else {
            &["economy", "hagathaRuntime"][..]
        };
        only_keys(profile, "game save profile", keys)?;
        if version >= 45 {
            let flags = profile["advancedUnlocks"]
                .as_array()
                .filter(|flags| flags.len() == 8)
                .ok_or("game save profile advanced unlocks must contain eight flags")?;
            let mut unlocks = [false; 8];
            for (index, value) in flags.iter().enumerate() {
                unlocks[index] = value.as_bool().ok_or_else(|| {
                    format!("game save profile advanced unlocks[{index}] is invalid")
                })?;
            }
            profile_advanced_unlocks = Some(unlocks);
        }
        record(&profile["economy"], "game save profile economy")?;
        record(&profile["hagathaRuntime"], "game save profile Hagatha runtime")?;
        (profile["economy"].clone(), Some(profile["hagathaRuntime"].clone()))
    } else {
        legacy_profile(continuation.as_ref().expect("legacy save has a continuation"))?
    };
    let integrity = if version >= 4 {
        serde_json::from_value(root["integrity"].clone())
            .map_err(|_| "game save integrity is invalid")?
    } else {
        SaveIntegrity::LocalOnly
    };
    let mods = if version >= 2 { parse_mods(&root["mods"])? } else { Vec::new() };
    // LuaObject's own visitor preserves JS property order. Parsing the whole envelope
    // through serde_json::Value would sort those arbitrary state-cell keys.
    #[derive(Deserialize)]
    struct OrderedState {
        #[serde(rename = "modState", default)]
        mod_state: LuaObject,
    }
    let mod_state = serde_json::from_str::<OrderedState>(document)
        .map_err(|_| "game save mod state is invalid")?
        .mod_state;
    if version >= 2 {
        validate_mod_state(&mod_state)?;
    }
    let native_source = if version >= 17 && !root["nativeSource"].is_null() {
        Some(parse_native_source(&root["nativeSource"])?)
    } else {
        None
    };
    Ok(ParsedSaveDocument {
        continuation,
        profile_advanced_unlocks,
        profile_economy,
        profile_hagatha_runtime,
        metadata: SaveMetadata { integrity, mods, mod_state, native_source },
        source_schema_version: version,
    })
}

fn parse_continuation(
    root: &Map<String, Value>,
    version: u64,
) -> Result<ParsedContinuation, String> {
    Ok(ParsedContinuation {
        loaded_boneyard: root["loadedBoneyard"].clone(),
        simulation: root["simulation"].clone(),
        summary: parse_summary(&root["summary"], version)?,
    })
}

fn legacy_profile(continuation: &ParsedContinuation) -> Result<(Value, Option<Value>), String> {
    let simulation = record(&continuation.simulation, "game save simulation")?;
    let players = record(
        simulation.get("playerEntities").ok_or("game save players must be an object")?,
        "game save players",
    )?;
    let identities = players
        .get("identities")
        .and_then(Value::as_array)
        .ok_or("game save player profile is invalid")?;
    let economies = players
        .get("economies")
        .and_then(Value::as_array)
        .ok_or("game save player profile is invalid")?;
    let mut owner_index = None;
    for (index, identity) in identities.iter().enumerate() {
        let identity = record(identity, "game save player identity")?;
        if identity.get("playerId").and_then(Value::as_str) == Some(&continuation.summary.player_id)
        {
            owner_index = Some(index);
            break;
        }
    }
    let index = owner_index.ok_or("game save profile owner is absent")?;
    let economy = economies.get(index).ok_or("game save profile owner is absent")?;
    record(economy, "game save profile economy")?;
    let runtime = players
        .get("progressions")
        .and_then(Value::as_array)
        .and_then(|rows| rows.get(index))
        .and_then(|row| row.get("hagathaRuntime"))
        .cloned();
    Ok((economy.clone(), runtime))
}

fn parse_summary(value: &Value, version: u64) -> Result<SaveSummary, String> {
    let summary = record(value, "game save summary")?;
    let mut keys = vec!["character", "phase", "playerId", "savedAtTick", "worldKind"];
    if version >= 6 {
        keys.push("activeRun");
    }
    if version >= 10 {
        keys.push("partyRejoinToken");
    }
    only_keys(summary, "game save summary", &keys)?;
    let character = record(&summary["character"], "game save character")?;
    only_keys(character, "game save character", &["discipline", "displayName", "element"])?;
    let valid_discipline = matches!(
        character["discipline"].as_str(),
        Some("arcane" | "mind" | "body" | "air" | "earth" | "ether" | "fire" | "water")
    );
    if !valid_discipline {
        return Err("game save character discipline is invalid".into());
    }
    let valid_element =
        matches!(character["element"].as_str(), Some("air" | "earth" | "ether" | "fire" | "water"));
    if !valid_element {
        return Err("game save character element is invalid".into());
    }
    if character["displayName"].as_str().is_none() {
        return Err("game save character display name is invalid".into());
    }
    let character: NewGameSelection = super::value::decode(summary["character"].clone())?;
    if character.name.is_empty() || character.name.encode_utf16().count() > 64 {
        return Err("game save character display name is invalid".into());
    }
    let player_id = summary["playerId"].as_str().ok_or("game save owner is invalid")?;
    if player_id.is_empty() || player_id.encode_utf16().count() > 128 {
        return Err("game save owner is invalid".into());
    }
    let phase: GameRunPhase =
        super::value::decode(summary["phase"].clone()).map_err(|_| "game save phase is invalid")?;
    let world_kind: SaveWorldKind = serde_json::from_value(summary["worldKind"].clone())
        .map_err(|_| "game save world kind is invalid")?;
    let active_run = if version >= 6 {
        summary["activeRun"].as_bool().ok_or("game save active run is invalid")?
    } else {
        world_kind == SaveWorldKind::Boneyard && phase == GameRunPhase::Active
    };
    let party_rejoin_token = if version < 10 || summary["partyRejoinToken"].is_null() {
        None
    } else {
        let token = summary["partyRejoinToken"]
            .as_str()
            .ok_or("game save party rejoin token is invalid")?;
        if !valid_party_token(token, version >= 12) {
            return Err("game save party rejoin token is invalid".into());
        }
        Some(token.into())
    };
    Ok(SaveSummary {
        active_run,
        character,
        party_rejoin_token,
        phase,
        player_id: player_id.into(),
        saved_at_tick: integer(&summary["savedAtTick"], "game save tick")?,
        world_kind,
    })
}

pub(super) fn valid_party_token(token: &str, signed: bool) -> bool {
    let segment = |value: &str| {
        value.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    };
    if !signed {
        return token.len() == 43 && segment(token);
    }
    if token.len() > 8192 {
        return false;
    }
    let parts: Vec<_> = token.split('.').collect();
    parts.len() == 3
        && matches!(parts[0], "sdrpr1" | "sdrpr2")
        && !parts[1].is_empty()
        && segment(parts[1])
        && parts[2].len() == 43
        && segment(parts[2])
}

fn valid_mod_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.as_bytes()[0].is_ascii_alphanumeric()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn parse_mods(value: &Value) -> Result<Vec<GameContentIdentity>, String> {
    let values = value.as_array().ok_or("game save mods are invalid")?;
    if values.len() > 128 {
        return Err("game save mods are invalid".into());
    }
    let mut identities = Vec::with_capacity(values.len());
    let mut seen = BTreeSet::new();
    for value in values {
        let record = record(value, "game save mod")?;
        only_keys(record, "game save mod", &["contentSha256", "id", "version"])?;
        let entry: GameContentIdentity = serde_json::from_value(value.clone())
            .map_err(|_| "game save mod identity is invalid")?;
        if !valid_mod_id(&entry.id)
            || !seen.insert(entry.id.to_ascii_lowercase())
            || entry.version.is_empty()
            || entry.version.encode_utf16().count() > 64
            || !valid_sha256(&entry.content_sha256)
        {
            return Err("game save mod identity is invalid".into());
        }
        identities.push(entry);
    }
    Ok(identities)
}

fn validate_mod_state(state: &LuaObject) -> Result<(), String> {
    use solomon_sim::modding::LuaValue;
    if state.len() > 128 {
        return Err("game save mod state is too large".into());
    }
    for (id, state) in state.iter() {
        if !valid_mod_id(id) {
            return Err("game save mod state id is invalid".into());
        }
        let fields = state.as_object().ok_or("game save mod state must be an object")?;
        if fields.len() > 256 {
            return Err("game save mod state has too many keys".into());
        }
        if fields.iter().any(|(key, _)| key.is_empty() || key.encode_utf16().count() > 128) {
            return Err("game save mod state key is invalid".into());
        }
        if crate::json_value::stringify(state)?.len() > 60 * 1024 {
            return Err("game save mod state exceeds its size limit".into());
        }
        let mut pending = vec![(state, 0)];
        while let Some((value, depth)) = pending.pop() {
            if depth > 16 {
                return Err("game save mod state is too deeply nested".into());
            }
            match value {
                LuaValue::Number(value) if !value.is_finite() => {
                    return Err("game save mod state number is invalid".into());
                }
                LuaValue::Array(values) => {
                    if values.len() > 2048 {
                        return Err("game save mod state array is too large".into());
                    }
                    pending.extend(values.iter().map(|value| (value, depth + 1)));
                }
                LuaValue::Object(values) => {
                    if values.len() > 2048 {
                        return Err("game save mod state object is too large".into());
                    }
                    for (key, value) in values.iter() {
                        if matches!(key, "__proto__" | "constructor" | "prototype") {
                            return Err(format!("game save mod state has unsafe field {key}"));
                        }
                        pending.push((value, depth + 1));
                    }
                }
                _ => {}
            }
        }
    }
    Ok(())
}

fn bounded_base64(value: &str) -> bool {
    if value.is_empty() || !value.len().is_multiple_of(4) || value.len() > 11 * 1024 * 1024 {
        return false;
    }
    let unpadded = value.trim_end_matches('=');
    value.len() - unpadded.len() <= 2
        && unpadded.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/'))
}

fn native_path_is_safe(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    let game_state = lower.starts_with("solomondark/savegames/")
        && lower.ends_with("/gamestate.sav")
        && lower.len() > "solomondark/savegames//gamestate.sav".len();
    !path.is_empty()
        && path.encode_utf16().count() <= 512
        && lower.starts_with("solomondark/")
        && !matches!(lower.as_str(), "solomondark/darkdata.cfg" | "solomondark/settings.txt")
        && !game_state
        && !path.contains(['\\', ':'])
        && !path.starts_with('/')
        && !path.ends_with('/')
        && path.split('/').all(|part| !part.is_empty() && part != "." && part != "..")
        && !path
            .chars()
            .any(|letter| letter <= '\u{001f}' || ('\u{007f}'..='\u{009f}').contains(&letter))
}

fn parse_native_source(value: &Value) -> Result<NativeSaveSource, String> {
    let source: NativeSaveSource =
        serde_json::from_value(value.clone()).map_err(|_| "game save native source is invalid")?;
    if source.retained_files.len() > 253 {
        return Err("game save native source retained files are invalid".into());
    }
    let mut paths = BTreeSet::new();
    let mut retained_length = 0;
    for file in &source.retained_files {
        if (!file.base64.is_empty() && !bounded_base64(&file.base64))
            || !native_path_is_safe(&file.path)
            || !valid_sha256(&file.sha256)
            || !paths.insert(file.path.to_lowercase())
        {
            return Err("game save native retained file is invalid".into());
        }
        retained_length += file.base64.len();
    }
    if !bounded_base64(&source.darkdata_base64)
        || !bounded_base64(&source.gamestate_base64)
        || source.darkdata_base64.len() + source.gamestate_base64.len() + retained_length
            > 11 * 1024 * 1024
        || !valid_sha256(&source.darkdata_sha256)
        || !valid_sha256(&source.gamestate_sha256)
        || source.run_name.is_empty()
        || source.run_name.len() > 64
        || !source
            .run_name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err("game save native source is invalid".into());
    }
    Ok(source)
}

pub(super) fn validate_json_tree(value: &Value) -> Result<(), String> {
    let mut count = 0;
    let mut pending = vec![(value, 0)];
    while let Some((value, depth)) = pending.pop() {
        count += 1;
        if count > 3_000_000 {
            return Err("game save has too many values".into());
        }
        if depth > 80 {
            return Err("game save is too deeply nested".into());
        }
        match value {
            Value::Array(values) => pending.extend(values.iter().map(|value| (value, depth + 1))),
            Value::Object(values) => {
                for (key, value) in values {
                    if matches!(key.as_str(), "__proto__" | "constructor" | "prototype") {
                        return Err(format!("game save has unsafe field {key}"));
                    }
                    pending.push((value, depth + 1));
                }
            }
            _ => {}
        }
    }
    Ok(())
}

pub(super) fn record<'a>(value: &'a Value, field: &str) -> Result<&'a Map<String, Value>, String> {
    value.as_object().ok_or_else(|| format!("{field} must be an object"))
}

pub(super) fn only_keys(
    source: &Map<String, Value>,
    field: &str,
    keys: &[&str],
) -> Result<(), String> {
    for key in source.keys() {
        if !keys.contains(&key.as_str()) {
            return Err(format!("{field} has unexpected field {key}"));
        }
    }
    for key in keys {
        if !source.contains_key(*key) {
            return Err(format!("{field} is missing field {key}"));
        }
    }
    Ok(())
}

pub(super) fn integer(value: &Value, field: &str) -> Result<u64, String> {
    let value = value.as_f64().ok_or_else(|| format!("{field} is invalid"))?;
    if value < 0.0 || value.fract() != 0.0 || value > MAX_SAFE_INTEGER as f64 {
        return Err(format!("{field} is invalid"));
    }
    Ok(value as u64)
}
