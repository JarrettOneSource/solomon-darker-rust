//! Settled portable profile contract (`save/portable-game-profile.ts`).

use std::collections::BTreeSet;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::Serialize;
use serde_json::{Map, Value};
use solomon_sim::economy::{
    native_hagatha_bundle_state_is_valid, native_hagatha_outcome_state_is_valid,
};
use solomon_sim::profile::native_skill_category;

use super::archive::{
    NativeArchiveFile, NativeSaveArchive, retained_path_is_safe, run_name_is_valid,
};
use super::bridge::*;
use super::{decode_base64, safe_integer, sha256, sha256_is_valid};
use crate::native_save_source::{NativeSaveFile, NativeSaveSource};

pub const PORTABLE_GAME_PROFILE_FORMAT: &str = "solomon-dark-portable-profile";
pub const PORTABLE_GAME_PROFILE_VERSION: u32 = 1;
pub const MAX_PORTABLE_GAME_PROFILE_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortableNativeStorageSummary {
    pub child_count: usize,
    pub materialized_in_web: bool,
    pub payload_length: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortableGameProfileState {
    pub boast: NativeGameBoastState,
    pub dowsing_fee: i32,
    pub first_mixed: Vec<bool>,
    pub gold: i32,
    pub hagatha_bundle_selectors: Vec<u32>,
    pub help_pending: Vec<bool>,
    pub librarian_lace_read: bool,
    pub native_storage: PortableNativeStorageSummary,
    pub tutorial_pending: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortableGameWizardState {
    pub advanced_unlocks: Vec<bool>,
    pub cheat_death_charges: i32,
    pub cheat_death_enabled: bool,
    pub concentration_skill_ids: [Option<i32>; 2],
    pub current_health: f64,
    pub current_mana: f64,
    pub deferred_skill_choices: i32,
    pub discipline_root: i32,
    pub element_root: i32,
    pub experience: f64,
    pub experience_bonus: f64,
    pub firewalker_active: bool,
    pub hagatha_ownership: Vec<bool>,
    pub learned_order: Vec<i32>,
    pub level: i32,
    pub mana_cost_reduction: f64,
    pub maximum_health: f64,
    pub maximum_mana: f64,
    pub meditation_idle_delay: i32,
    pub name: String,
    pub next_concentration_slot: i32,
    pub next_threshold: f64,
    pub offer_seed: i32,
    pub offensive_damage_flat: f64,
    pub pending_skill_choices: i32,
    pub perk_capacity: i32,
    pub perk_selectors: Vec<u32>,
    pub permanent_ranks: Vec<u32>,
    pub poison_immunity_ticks: i32,
    pub previous_threshold: f64,
    pub selected_primary_skill_id: i32,
    pub skill_quickbar: Vec<Option<i32>>,
    pub starting_primary: i32,
    pub starting_secondary: i32,
    pub weld_effect: f64,
}

impl From<&PortableGameWizardState> for NativeWizardPatch {
    fn from(wizard: &PortableGameWizardState) -> Self {
        Self {
            cheat_death_charges: wizard.cheat_death_charges,
            cheat_death_enabled: wizard.cheat_death_enabled,
            concentration_skill_ids: wizard.concentration_skill_ids,
            current_health: wizard.current_health,
            current_mana: wizard.current_mana,
            deferred_skill_choices: wizard.deferred_skill_choices,
            discipline_root: wizard.discipline_root,
            element_root: wizard.element_root,
            experience: wizard.experience,
            experience_bonus: wizard.experience_bonus,
            firewalker_active: wizard.firewalker_active,
            hagatha_ownership: wizard.hagatha_ownership.clone(),
            learned_order: wizard.learned_order.clone(),
            level: wizard.level,
            mana_cost_reduction: wizard.mana_cost_reduction,
            maximum_health: wizard.maximum_health,
            maximum_mana: wizard.maximum_mana,
            meditation_idle_delay: wizard.meditation_idle_delay,
            name: wizard.name.clone(),
            next_concentration_slot: wizard.next_concentration_slot,
            next_threshold: wizard.next_threshold,
            offer_seed: wizard.offer_seed,
            offensive_damage_flat: wizard.offensive_damage_flat,
            pending_skill_choices: wizard.pending_skill_choices,
            perk_capacity: wizard.perk_capacity,
            perk_selectors: wizard.perk_selectors.clone(),
            permanent_ranks: wizard.permanent_ranks.clone(),
            poison_immunity_ticks: wizard.poison_immunity_ticks,
            previous_threshold: wizard.previous_threshold,
            selected_primary_skill_id: wizard.selected_primary_skill_id,
            skill_quickbar: wizard.skill_quickbar.clone(),
            starting_primary: wizard.starting_primary,
            starting_secondary: wizard.starting_secondary,
            weld_effect: wizard.weld_effect,
        }
    }
}

impl From<&PortableGameProfileState> for NativeProfilePatch {
    fn from(profile: &PortableGameProfileState) -> Self {
        Self {
            dowsing_fee: profile.dowsing_fee,
            first_mixed: profile.first_mixed.clone(),
            gold: profile.gold,
            hagatha_bundle_selectors: profile.hagatha_bundle_selectors.clone(),
            help_pending: profile.help_pending.clone(),
            librarian_lace_read: profile.librarian_lace_read,
            tutorial_pending: profile.tutorial_pending,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortableGameProfile {
    pub format: String,
    pub native_source: NativeSaveSource,
    pub profile: PortableGameProfileState,
    pub retail_executable_sha256: String,
    pub version: u32,
    pub warnings: Vec<String>,
    pub wizard: PortableGameWizardState,
}

fn record<'a>(value: &'a Value, claim: &str) -> Result<&'a Map<String, Value>, String> {
    value.as_object().ok_or_else(|| format!("{claim} must be an object"))
}

fn only_keys(root: &Map<String, Value>, keys: &[&str], claim: &str) -> Result<(), String> {
    if root.len() != keys.len() || keys.iter().any(|key| !root.contains_key(*key)) {
        return Err(format!("{claim} fields are invalid"));
    }
    Ok(())
}

fn integer(value: &Value, minimum: i64, maximum: i64, claim: &str) -> Result<i64, String> {
    safe_integer(value)
        .filter(|value| *value >= minimum && *value <= maximum)
        .ok_or_else(|| format!("{claim} is invalid"))
}

fn finite(value: &Value, minimum: f64, maximum: f64, claim: &str) -> Result<f64, String> {
    value
        .as_f64()
        .filter(|value| value.is_finite() && *value >= minimum && *value <= maximum)
        .ok_or_else(|| format!("{claim} is invalid"))
}

fn boolean(value: &Value, claim: &str) -> Result<bool, String> {
    value.as_bool().ok_or_else(|| format!("{claim} is invalid"))
}

fn booleans(value: &Value, count: usize, claim: &str) -> Result<Vec<bool>, String> {
    let values = value
        .as_array()
        .filter(|values| values.len() == count)
        .ok_or_else(|| format!("{claim} must contain {count} flags"))?;
    let mut result = Vec::with_capacity(count);
    for (index, value) in values.iter().enumerate() {
        result.push(boolean(value, &format!("{claim}[{index}]"))?);
    }
    Ok(result)
}

fn integers(
    value: &Value,
    count: Option<usize>,
    minimum: i64,
    maximum: i64,
    claim: &str,
    unique: bool,
) -> Result<Vec<i32>, String> {
    let values = value
        .as_array()
        .filter(|values| count.is_none_or(|count| values.len() == count))
        .ok_or_else(|| format!("{claim} is invalid"))?;
    let mut result = Vec::with_capacity(values.len());
    for (index, value) in values.iter().enumerate() {
        result.push(integer(value, minimum, maximum, &format!("{claim}[{index}]"))? as i32);
    }
    if unique && result.iter().collect::<BTreeSet<_>>().len() != result.len() {
        return Err(format!("{claim} contains duplicates"));
    }
    Ok(result)
}

fn nullable_integers(value: &Value, count: usize, claim: &str) -> Result<Vec<Option<i32>>, String> {
    let values = value
        .as_array()
        .filter(|values| values.len() == count)
        .ok_or_else(|| format!("{claim} must contain {count} entries"))?;
    let mut result = Vec::with_capacity(count);
    for (index, value) in values.iter().enumerate() {
        result.push(if value.is_null() {
            None
        } else {
            Some(integer(value, 8, 79, &format!("{claim}[{index}]"))? as i32)
        });
    }
    Ok(result)
}

fn hagatha_outcomes(value: &Value) -> Result<Vec<u32>, String> {
    let outcomes: Vec<u32> = integers(value, None, 0, 27, "portable perk selectors", false)?
        .into_iter()
        .map(|value| value as u32)
        .collect();
    let mut ordinary = BTreeSet::new();
    let mut tonics = 0;
    let mut duplicate = false;
    for &selector in &outcomes {
        if selector == 27 {
            tonics += 1;
        } else if !ordinary.insert(selector) {
            duplicate = true;
        }
    }
    if outcomes.len() > 9 || outcomes.contains(&8) || duplicate || tonics > 2 {
        return Err("portable perk selectors are not a native outcome list".into());
    }
    Ok(outcomes)
}

fn hagatha_bundle(value: &Value) -> Result<Vec<u32>, String> {
    let outcomes: Vec<u32> = integers(value, None, 0, 27, "portable Hagatha bundle", false)?
        .into_iter()
        .map(|value| value as u32)
        .collect();
    if !native_hagatha_bundle_state_is_valid(&outcomes) {
        return Err("portable Hagatha bundle is not a native outcome list".into());
    }
    Ok(outcomes)
}

pub fn create_native_game_save_source(
    save: &NativeSaveArchive,
) -> Result<NativeSaveSource, String> {
    let mut paths = BTreeSet::new();
    let mut total = save.darkdata.len() + save.gamestate.len();
    let mut retained_files = Vec::new();
    for file in &save.retained_files {
        if !retained_path_is_safe(&file.path) || !paths.insert(file.path.to_lowercase()) {
            return Err("native retained save file is invalid".into());
        }
        total += file.bytes.len();
        retained_files.push(NativeSaveFile {
            base64: STANDARD.encode(&file.bytes),
            path: file.path.clone(),
            sha256: sha256(&file.bytes),
        });
    }
    if !run_name_is_valid(&save.run_name)
        || save.darkdata.is_empty()
        || save.gamestate.is_empty()
        || retained_files.len() > 253
        || total > NATIVE_SOURCE_ATTACHMENT_MAX_BYTES
    {
        return Err("native save source is invalid".into());
    }
    Ok(NativeSaveSource {
        darkdata_base64: STANDARD.encode(&save.darkdata),
        darkdata_sha256: sha256(&save.darkdata),
        gamestate_base64: STANDARD.encode(&save.gamestate),
        gamestate_sha256: sha256(&save.gamestate),
        retained_files,
        run_name: save.run_name.clone(),
    })
}

fn parse_native_source(value: &Value) -> Result<NativeSaveSource, String> {
    let root = record(value, "portable native source")?;
    only_keys(
        root,
        &[
            "darkdataBase64",
            "darkdataSha256",
            "gamestateBase64",
            "gamestateSha256",
            "retainedFiles",
            "runName",
        ],
        "portable native source",
    )?;
    let darkdata_base64 = root["darkdataBase64"].as_str();
    let gamestate_base64 = root["gamestateBase64"].as_str();
    let darkdata_sha256 = root["darkdataSha256"].as_str();
    let gamestate_sha256 = root["gamestateSha256"].as_str();
    let run_name = root["runName"].as_str();
    let retained = root["retainedFiles"].as_array();
    if darkdata_base64.is_none()
        || gamestate_base64.is_none()
        || darkdata_sha256.is_none_or(|value| !sha256_is_valid(value))
        || gamestate_sha256.is_none_or(|value| !sha256_is_valid(value))
        || run_name.is_none_or(|name| !run_name_is_valid(name))
        || retained.is_none_or(|files| files.len() > 253)
    {
        return Err("portable native source is invalid".into());
    }
    let darkdata_base64 = darkdata_base64.expect("native source base64 is a string");
    let gamestate_base64 = gamestate_base64.expect("native source base64 is a string");
    let darkdata_sha256 = darkdata_sha256.expect("native source digest was checked");
    let gamestate_sha256 = gamestate_sha256.expect("native source digest was checked");
    let darkdata = decode_base64(darkdata_base64, "portable darkdata")?;
    let gamestate = decode_base64(gamestate_base64, "portable gamestate")?;
    let mut paths = BTreeSet::new();
    let mut total = darkdata.len() + gamestate.len();
    let mut retained_files = Vec::new();
    for (index, file) in
        retained.expect("native source retained membership was checked").iter().enumerate()
    {
        let root = record(file, &format!("portable retained file {index}"))?;
        only_keys(root, &["base64", "path", "sha256"], &format!("portable retained file {index}"))?;
        let base64 = root["base64"].as_str();
        let path = root["path"].as_str();
        let digest = root["sha256"].as_str();
        if base64.is_none()
            || path.is_none_or(|path| {
                !retained_path_is_safe(path) || paths.contains(&path.to_lowercase())
            })
            || digest.is_none_or(|digest| !sha256_is_valid(digest))
        {
            return Err(format!("portable retained file {index} is invalid"));
        }
        let base64 = base64.expect("retained file base64 is a string");
        let path = path.expect("retained file path was checked");
        let digest = digest.expect("retained file digest was checked");
        let bytes = decode_base64(base64, &format!("portable retained file {index}"))?;
        if STANDARD.encode(&bytes) != base64 || sha256(&bytes) != digest {
            return Err(format!("portable retained file {index} integrity is invalid"));
        }
        total += bytes.len();
        paths.insert(path.to_lowercase());
        retained_files.push(NativeSaveFile {
            base64: base64.to_owned(),
            path: path.to_owned(),
            sha256: digest.to_owned(),
        });
    }
    if darkdata.is_empty()
        || gamestate.is_empty()
        || STANDARD.encode(&darkdata) != darkdata_base64
        || STANDARD.encode(&gamestate) != gamestate_base64
        || total > NATIVE_SOURCE_ATTACHMENT_MAX_BYTES
        || sha256(&darkdata) != darkdata_sha256
        || sha256(&gamestate) != gamestate_sha256
    {
        return Err("portable native source integrity is invalid".into());
    }
    decode_native_darkdata_profile(&darkdata)?;
    decode_native_gamestate_wizard(&gamestate)?;
    decode_native_gamestate_boast(&gamestate)?;
    Ok(NativeSaveSource {
        darkdata_base64: darkdata_base64.to_owned(),
        darkdata_sha256: darkdata_sha256.to_owned(),
        gamestate_base64: gamestate_base64.to_owned(),
        gamestate_sha256: gamestate_sha256.to_owned(),
        retained_files,
        run_name: run_name.expect("native source run name was checked").to_owned(),
    })
}

pub fn validate_native_game_save_source(source: &NativeSaveSource) -> Result<(), String> {
    let value = serde_json::to_value(source).map_err(|error| error.to_string())?;
    parse_native_source(&value)?;
    Ok(())
}

pub fn native_source_bytes(source: &NativeSaveSource) -> Result<NativeSaveArchive, String> {
    let mut retained_files = Vec::new();
    for file in &source.retained_files {
        retained_files.push(NativeArchiveFile {
            bytes: decode_base64(&file.base64, &format!("portable retained file {}", file.path))?,
            path: file.path.clone(),
        });
    }
    Ok(NativeSaveArchive {
        darkdata: decode_base64(&source.darkdata_base64, "portable darkdata")?,
        gamestate: decode_base64(&source.gamestate_base64, "portable gamestate")?,
        retained_files,
        run_name: source.run_name.clone(),
    })
}

pub fn parse_portable_game_profile(document: &str) -> Result<PortableGameProfile, String> {
    if document.is_empty() || document.len() > MAX_PORTABLE_GAME_PROFILE_BYTES {
        return Err("portable profile exceeds its size limit".into());
    }
    let value: Value =
        serde_json::from_str(document).map_err(|_| "portable profile is not valid JSON")?;
    let root = record(&value, "portable profile")?;
    only_keys(
        root,
        &[
            "format",
            "nativeSource",
            "profile",
            "retailExecutableSha256",
            "version",
            "warnings",
            "wizard",
        ],
        "portable profile",
    )?;
    if root["format"].as_str() != Some(PORTABLE_GAME_PROFILE_FORMAT)
        || root["version"].as_f64() != Some(f64::from(PORTABLE_GAME_PROFILE_VERSION))
        || root["retailExecutableSha256"].as_str() != Some(RETAIL_SOLOMON_DARK_SHA256)
    {
        return Err("portable profile version or retail identity is unsupported".into());
    }
    let profile = record(&root["profile"], "portable profile state")?;
    only_keys(
        profile,
        &[
            "boast",
            "dowsingFee",
            "firstMixed",
            "gold",
            "hagathaBundleSelectors",
            "helpPending",
            "librarianLaceRead",
            "nativeStorage",
            "tutorialPending",
        ],
        "portable profile state",
    )?;
    let storage = record(&profile["nativeStorage"], "portable native storage")?;
    let boast = record(&profile["boast"], "portable Boast state")?;
    only_keys(boast, &["failed", "selected", "succeeded"], "portable Boast state")?;
    only_keys(
        storage,
        &["childCount", "materializedInWeb", "payloadLength"],
        "portable native storage",
    )?;
    if storage["materializedInWeb"] != Value::Bool(false) {
        return Err("portable native storage materialization claim is invalid".into());
    }
    let wizard = record(&root["wizard"], "portable wizard")?;
    only_keys(
        wizard,
        &[
            "advancedUnlocks",
            "cheatDeathCharges",
            "cheatDeathEnabled",
            "concentrationSkillIds",
            "currentHealth",
            "currentMana",
            "deferredSkillChoices",
            "disciplineRoot",
            "elementRoot",
            "experience",
            "experienceBonus",
            "firewalkerActive",
            "hagathaOwnership",
            "learnedOrder",
            "level",
            "manaCostReduction",
            "maximumHealth",
            "maximumMana",
            "meditationIdleDelay",
            "name",
            "nextConcentrationSlot",
            "nextThreshold",
            "offerSeed",
            "offensiveDamageFlat",
            "pendingSkillChoices",
            "perkCapacity",
            "perkSelectors",
            "permanentRanks",
            "poisonImmunityTicks",
            "previousThreshold",
            "selectedPrimarySkillId",
            "skillQuickbar",
            "startingPrimary",
            "startingSecondary",
            "weldEffect",
        ],
        "portable wizard",
    )?;
    let name = wizard["name"]
        .as_str()
        .filter(|name| {
            !name.is_empty()
                && name.encode_utf16().count() <= 64
                && name.len() <= 255
                && !name.chars().any(|value| value < '\u{20}')
        })
        .ok_or("portable wizard name is invalid")?;
    let warnings = root["warnings"]
        .as_array()
        .filter(|values| values.iter().all(Value::is_string))
        .ok_or("portable profile warnings are invalid")?;
    let result = PortableGameProfile {
        format: PORTABLE_GAME_PROFILE_FORMAT.into(),
        native_source: parse_native_source(&root["nativeSource"])?,
        profile: PortableGameProfileState {
            boast: NativeGameBoastState {
                failed: boolean(&boast["failed"], "portable Boast failure")?,
                selected: if boast["selected"].is_null() {
                    None
                } else {
                    Some(integer(&boast["selected"], 0, 4, "portable selected Boast")? as u32)
                },
                succeeded: boolean(&boast["succeeded"], "portable Boast success")?,
            },
            dowsing_fee: integer(
                &profile["dowsingFee"],
                0,
                i64::from(i32::MAX),
                "portable Dowsing fee",
            )? as i32,
            first_mixed: booleans(
                &profile["firstMixed"],
                NATIVE_FIRST_MIX_COUNT,
                "portable first-mix state",
            )?,
            gold: integer(&profile["gold"], 0, i64::from(i32::MAX), "portable gold")? as i32,
            hagatha_bundle_selectors: hagatha_bundle(&profile["hagathaBundleSelectors"])?,
            help_pending: booleans(&profile["helpPending"], 10, "portable help state")?,
            librarian_lace_read: boolean(&profile["librarianLaceRead"], "portable Lace state")?,
            native_storage: PortableNativeStorageSummary {
                child_count: integer(
                    &storage["childCount"],
                    0,
                    1_000_000,
                    "portable storage child count",
                )? as usize,
                materialized_in_web: false,
                payload_length: integer(
                    &storage["payloadLength"],
                    0,
                    64 * 1024 * 1024,
                    "portable storage bytes",
                )? as usize,
            },
            tutorial_pending: boolean(&profile["tutorialPending"], "portable Tutorial state")?,
        },
        retail_executable_sha256: RETAIL_SOLOMON_DARK_SHA256.into(),
        version: PORTABLE_GAME_PROFILE_VERSION,
        warnings: warnings
            .iter()
            .map(|value| value.as_str().expect("warning strings were checked").to_owned())
            .collect(),
        wizard: PortableGameWizardState {
            advanced_unlocks: booleans(&wizard["advancedUnlocks"], 8, "portable advanced unlocks")?,
            cheat_death_charges: integer(
                &wizard["cheatDeathCharges"],
                0,
                1_000,
                "portable cheat-death charges",
            )? as i32,
            cheat_death_enabled: boolean(
                &wizard["cheatDeathEnabled"],
                "portable cheat-death state",
            )?,
            concentration_skill_ids: nullable_integers(
                &wizard["concentrationSkillIds"],
                2,
                "portable concentrations",
            )?
            .try_into()
            .expect("two concentration entries were checked"),
            current_health: finite(
                &wizard["currentHealth"],
                0.0,
                1_000_000.0,
                "portable current health",
            )?,
            current_mana: finite(
                &wizard["currentMana"],
                0.0,
                1_000_000.0,
                "portable current mana",
            )?,
            deferred_skill_choices: integer(
                &wizard["deferredSkillChoices"],
                0,
                1_000,
                "portable deferred choices",
            )? as i32,
            discipline_root: integer(&wizard["disciplineRoot"], 0, 7, "portable discipline root")?
                as i32,
            element_root: integer(&wizard["elementRoot"], 0, 4, "portable element root")? as i32,
            experience: finite(&wizard["experience"], 0.0, 10_000_000.0, "portable experience")?,
            experience_bonus: finite(
                &wizard["experienceBonus"],
                -1_000.0,
                1_000.0,
                "portable experience bonus",
            )?,
            firewalker_active: boolean(&wizard["firewalkerActive"], "portable Firewalker state")?,
            hagatha_ownership: booleans(
                &wizard["hagathaOwnership"],
                NATIVE_HAGATHA_OWNERSHIP_COUNT,
                "portable Hagatha ownership",
            )?,
            learned_order: integers(
                &wizard["learnedOrder"],
                None,
                8,
                79,
                "portable learned order",
                true,
            )?,
            level: integer(&wizard["level"], 1, 75, "portable level")? as i32,
            mana_cost_reduction: finite(
                &wizard["manaCostReduction"],
                -1_000_000.0,
                1_000_000.0,
                "portable mana reduction",
            )?,
            maximum_health: finite(
                &wizard["maximumHealth"],
                1.0,
                1_000_000.0,
                "portable maximum health",
            )?,
            maximum_mana: finite(
                &wizard["maximumMana"],
                1.0,
                1_000_000.0,
                "portable maximum mana",
            )?,
            meditation_idle_delay: integer(
                &wizard["meditationIdleDelay"],
                -1,
                10_000_000,
                "portable Meditation delay",
            )? as i32,
            name: name.to_owned(),
            next_concentration_slot: integer(
                &wizard["nextConcentrationSlot"],
                0,
                1,
                "portable concentration cursor",
            )? as i32,
            next_threshold: finite(
                &wizard["nextThreshold"],
                0.0,
                10_000_000.0,
                "portable next threshold",
            )?,
            offer_seed: integer(&wizard["offerSeed"], 0, 999_999, "portable offer seed")? as i32,
            offensive_damage_flat: finite(
                &wizard["offensiveDamageFlat"],
                -1_000_000.0,
                1_000_000.0,
                "portable damage flat",
            )?,
            pending_skill_choices: integer(
                &wizard["pendingSkillChoices"],
                0,
                1_000,
                "portable pending choices",
            )? as i32,
            perk_capacity: integer(&wizard["perkCapacity"], 0, 64, "portable perk capacity")?
                as i32,
            perk_selectors: hagatha_outcomes(&wizard["perkSelectors"])?,
            permanent_ranks: integers(
                &wizard["permanentRanks"],
                Some(NATIVE_WIZARD_SKILL_ROW_COUNT),
                0,
                0xffff,
                "portable permanent ranks",
                false,
            )?
            .into_iter()
            .map(|value| value as u32)
            .collect(),
            poison_immunity_ticks: integer(
                &wizard["poisonImmunityTicks"],
                0,
                10_000_000,
                "portable poison immunity",
            )? as i32,
            previous_threshold: finite(
                &wizard["previousThreshold"],
                0.0,
                10_000_000.0,
                "portable previous threshold",
            )?,
            selected_primary_skill_id: integer(
                &wizard["selectedPrimarySkillId"],
                8,
                80,
                "portable selected primary",
            )? as i32,
            skill_quickbar: nullable_integers(&wizard["skillQuickbar"], 8, "portable quickbar")?,
            starting_primary: integer(
                &wizard["startingPrimary"],
                8,
                79,
                "portable starting primary",
            )? as i32,
            starting_secondary: integer(
                &wizard["startingSecondary"],
                8,
                79,
                "portable starting secondary",
            )? as i32,
            weld_effect: finite(&wizard["weldEffect"], 0.0, 1_000_000.0, "portable weld effect")?,
        },
    };
    let wizard = &result.wizard;
    if wizard.current_health > wizard.maximum_health
        || wizard.current_mana > wizard.maximum_mana
        || wizard.learned_order.iter().any(|id| wizard.permanent_ranks[*id as usize] == 0)
    {
        return Err("portable wizard progression is internally inconsistent".into());
    }
    if (wizard.selected_primary_skill_id != 80
        && native_skill_category(wizard.selected_primary_skill_id as usize) != Some(1))
        || wizard
            .concentration_skill_ids
            .iter()
            .flatten()
            .any(|id| native_skill_category(*id as usize) != Some(3))
        || (wizard.concentration_skill_ids[0].is_some()
            && wizard.concentration_skill_ids[0] == wizard.concentration_skill_ids[1])
        || (wizard.concentration_skill_ids[1].is_some() && !wizard.perk_selectors.contains(&21))
        || wizard
            .skill_quickbar
            .iter()
            .flatten()
            .any(|id| !matches!(native_skill_category(*id as usize), Some(1 | 2)))
    {
        return Err("portable selected-skill state is invalid".into());
    }
    let tonics = wizard.perk_selectors.iter().filter(|selector| **selector == 27).count() as u32;
    let ordinary: BTreeSet<u32> =
        wizard.perk_selectors.iter().copied().filter(|selector| *selector != 27).collect();
    if !native_hagatha_outcome_state_is_valid(
        &wizard.perk_selectors,
        tonics,
        wizard.perk_capacity as u32,
    ) || wizard.hagatha_ownership[..27]
        .iter()
        .enumerate()
        .any(|(selector, owned)| selector != 8 && *owned != ordinary.contains(&(selector as u32)))
        || wizard.hagatha_ownership[27] != (tonics > 0)
    {
        return Err("portable Hagatha outcomes and ownership are inconsistent".into());
    }
    let boast = &result.profile.boast;
    if (boast.selected.is_none() && (boast.failed || boast.succeeded))
        || (boast.selected == Some(3) && boast.failed)
        || (boast.failed && boast.succeeded)
    {
        return Err("portable Boast lifecycle is inconsistent".into());
    }
    Ok(result)
}

pub fn encode_portable_game_profile(profile: &PortableGameProfile) -> Result<String, String> {
    let document = crate::json_value::stringify_serializable(profile)?;
    if document.len() > MAX_PORTABLE_GAME_PROFILE_BYTES {
        return Err("portable profile exceeds its size limit".into());
    }
    Ok(document)
}

pub fn create_portable_game_profile_from_native(
    save: &NativeSaveArchive,
    additional_warnings: &[String],
) -> Result<PortableGameProfile, String> {
    let profile = decode_native_darkdata_profile(&save.darkdata)?;
    let wizard = decode_native_gamestate_wizard(&save.gamestate)?;
    let boast = decode_native_gamestate_boast(&save.gamestate)?;
    if !wizard.experience_enabled || wizard.random_boast_active != (boast.selected == Some(3)) {
        return Err("native local wizard and Boast progression flags disagree".into());
    }
    let ranks: Vec<u32> = wizard.rows.iter().map(|row| u32::from(row.permanent_rank)).collect();
    let effective_only: Vec<i32> = wizard
        .learned_order
        .iter()
        .copied()
        .filter(|id| usize::try_from(*id).ok().and_then(|id| ranks.get(id)) == Some(&0))
        .collect();
    if effective_only.iter().any(|id| wizard.rows[*id as usize].effective_rank == 0) {
        return Err("native learned order contains an inactive row".into());
    }
    let learned_order: Vec<i32> = wizard
        .learned_order
        .iter()
        .copied()
        .filter(|id| {
            usize::try_from(*id).ok().and_then(|id| ranks.get(id)).is_some_and(|rank| *rank > 0)
        })
        .collect();
    let mut warnings = additional_warnings.to_vec();
    warnings.extend([
        "Native Luthacus storage is retained byte-for-byte but is not materialized in the web inventory bridge.",
        "Machinimbus purchase-only unlocks are not stored by retail; only already learned advanced rows can cross.",
        "Serendipity and Reverie active-until-hurt flags are not retail disk members and start inactive after import.",
        "Retail omits Unforge base HP/MP bonuses; import rebuilds maximum vitals and preserves only the saved current/max ratios.",
    ].into_iter().map(str::to_owned));
    if wizard.selected_primary_skill_id == 52 || wizard.skill_quickbar.contains(&Some(52)) {
        warnings.push("Retail omits the active synthetic Weld build; selected or belted Spell Welding cannot be reconstructed after disk load.".into());
    }
    if wizard.selected_primary_skill_id == 80 {
        warnings.push(
            "Plane Orb is a live Planewalker override and resets in the settled portable Hub."
                .into(),
        );
    }
    if !effective_only.is_empty() {
        let ids = effective_only.iter().map(i32::to_string).collect::<Vec<_>>().join(", ");
        warnings.push(format!("Native learned/visible row(s) {ids} depend only on effective equipment ranks; they remain in the native attachment but are unavailable in the settled web Hub."));
    }
    warnings.push("The portable wizard starts in a settled Hub; in-flight native Arena and Region objects remain in the native attachment.".into());
    if !save.retained_files.is_empty() {
        warnings.push(format!("{} opaque native slot file(s) will be retained for stock export but are not web authority.", save.retained_files.len()));
    }
    if wizard.hagatha_ownership[8] || wizard.hagatha_ownership[28..].iter().any(|owned| *owned) {
        warnings.push("Unavailable native Hagatha ownership rows 8 and 28..49 remain byte-preserved but are not materialized in web play.".into());
    }
    let candidate = PortableGameProfile {
        format: PORTABLE_GAME_PROFILE_FORMAT.into(),
        native_source: create_native_game_save_source(save)?,
        profile: PortableGameProfileState {
            boast,
            dowsing_fee: profile.dowsing_fee,
            first_mixed: profile.first_mixed,
            gold: profile.gold,
            hagatha_bundle_selectors: profile.hagatha_bundle_selectors,
            help_pending: profile.help_pending,
            librarian_lace_read: profile.librarian_lace_read,
            native_storage: PortableNativeStorageSummary {
                child_count: profile.storage_child_count,
                materialized_in_web: false,
                payload_length: profile.storage_payload_length,
            },
            tutorial_pending: profile.tutorial_pending,
        },
        retail_executable_sha256: RETAIL_SOLOMON_DARK_SHA256.into(),
        version: PORTABLE_GAME_PROFILE_VERSION,
        warnings,
        wizard: PortableGameWizardState {
            advanced_unlocks: ranks[72..80].iter().map(|rank| *rank > 0).collect(),
            cheat_death_charges: wizard.cheat_death_charges,
            cheat_death_enabled: wizard.cheat_death_enabled,
            concentration_skill_ids: wizard.concentration_skill_ids,
            current_health: wizard.current_health,
            current_mana: wizard.current_mana,
            deferred_skill_choices: wizard.deferred_skill_choices,
            discipline_root: wizard.discipline_root,
            element_root: wizard.element_root,
            experience: wizard.experience,
            experience_bonus: wizard.experience_bonus,
            firewalker_active: wizard.firewalker_active,
            hagatha_ownership: wizard.hagatha_ownership,
            learned_order,
            level: wizard.level,
            mana_cost_reduction: wizard.mana_cost_reduction,
            maximum_health: wizard.maximum_health,
            maximum_mana: wizard.maximum_mana,
            meditation_idle_delay: wizard.meditation_idle_delay,
            name: wizard.name,
            next_concentration_slot: wizard.next_concentration_slot,
            next_threshold: wizard.next_threshold,
            offer_seed: wizard.offer_seed,
            offensive_damage_flat: wizard.offensive_damage_flat,
            pending_skill_choices: wizard.pending_skill_choices,
            perk_capacity: wizard.perk_capacity,
            perk_selectors: wizard.perk_selectors,
            permanent_ranks: ranks,
            poison_immunity_ticks: wizard.poison_immunity_ticks,
            previous_threshold: wizard.previous_threshold,
            selected_primary_skill_id: wizard.selected_primary_skill_id,
            skill_quickbar: wizard.skill_quickbar,
            starting_primary: wizard.starting_primary,
            starting_secondary: wizard.starting_secondary,
            weld_effect: wizard.weld_effect,
        },
    };
    parse_portable_game_profile(&encode_portable_game_profile(&candidate)?)
}
