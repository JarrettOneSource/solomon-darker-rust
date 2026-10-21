//! Validates Website provisioner packages before any definition VM is created.
//! Source: `host/web-mod-content.ts::materializeWebSessionContent`.

use std::collections::{BTreeMap, HashSet};

use base64::{Engine, engine::general_purpose::STANDARD};
use solomon_assets::boneyard::{ModBoneyardEntry, boneyard_scene_source_value, decode_boneyard};
use solomon_sim::boneyard::{BoneyardChoice, BoneyardSource};
use solomon_sim::modding::assets::GameModAsset;
use solomon_sim::modding::definition::WebLuaModIdentity;
use solomon_sim::modding::identity::{
    GameContentIdentity, GameContentManifest, WebSessionContentPayload, WebSessionContentSummary,
    WebSessionModSummary,
};
use solomon_sim::modding::{LuaObject, LuaValue};

use super::{MaterializedWebSessionContent, WebLuaModSource};
use crate::assets::sha256;

const MAXIMUM_MODS: usize = 128;
const MAXIMUM_LUA_MODS: usize = 8;
const MAXIMUM_SCRIPT_BYTES: usize = 256 * 1_024;
const MAXIMUM_BONEYARD_BYTES: usize = 8 * 1_024 * 1_024;
const MAXIMUM_FILE_BYTES: usize = 16 * 1_024 * 1_024;
const MAXIMUM_FILES: usize = 256;
const MAXIMUM_CONTENT_BYTES: usize = 32 * 1_024 * 1_024;

struct PackageFile {
    asset: GameModAsset,
    bytes: Vec<u8>,
}

struct PackageBoneyard {
    bytes_base64: String,
    target: String,
}

struct PackageMod {
    boneyards: Vec<PackageBoneyard>,
    content_sha256: String,
    entry_script: Option<String>,
    files: Vec<PackageFile>,
    id: String,
    name: String,
    slug: String,
    version: String,
}

/// Typed hosted callers and the raw trust boundary share one source validator.
/// Lua callbacks are deliberately absent from the returned admission document.
pub fn materialize_web_session_content(
    payload: &WebSessionContentPayload,
) -> Result<MaterializedWebSessionContent, String> {
    let text = serde_json::to_string(payload).map_err(|error| error.to_string())?;
    let value = serde_json::from_str(&text).map_err(|error| error.to_string())?;
    materialize_web_session_content_value(&value)
}

/// `materializeWebSessionContent`: reject missing or extra fields at the actual
/// untrusted boundary, including the required nullable `entryScript` property.
pub fn materialize_web_session_content_value(
    value: &LuaValue,
) -> Result<MaterializedWebSessionContent, String> {
    let source = object(Some(value), "session content")?;
    exact_keys(source, &["manifestSha256", "mods"], "session content")?;
    let manifest_sha256 = digest(source.get("manifestSha256"), "session manifest")?;
    let Some(mods) = source.get("mods").and_then(LuaValue::as_array) else {
        return Err(format!(
            "session content mods must be an array of at most {MAXIMUM_MODS} entries"
        ));
    };
    if mods.len() > MAXIMUM_MODS {
        return Err(format!(
            "session content mods must be an array of at most {MAXIMUM_MODS} entries"
        ));
    }
    // The Website parses every row before checking duplicate IDs or decoding
    // legacy Boneyards. Preserve that order for rejected provisioner documents.
    let mut packages = Vec::with_capacity(mods.len());
    for (index, value) in mods.iter().enumerate() {
        packages.push(parse_mod(value, index)?);
    }

    let mut ids = HashSet::new();
    let mut boneyards = Vec::new();
    let mut boneyard_indices = BTreeMap::new();
    let mut assets = Vec::new();
    let mut mod_sources = Vec::new();
    let mut manifest_mods = Vec::new();
    let mut summary_mods = Vec::new();
    let mut aggregate_bytes = 0;
    for package in packages {
        if !ids.insert(package.id.to_lowercase()) {
            return Err(format!("duplicate session mod id: {}", package.id));
        }
        let mut package_bytes = BTreeMap::new();
        let mut package_assets = Vec::with_capacity(package.files.len());
        for file in package.files {
            aggregate_bytes += file.bytes.len();
            package_bytes.insert(file.asset.path.clone(), file.bytes);
            package_assets.push(file.asset.clone());
            assets.push(file.asset);
        }
        if let Some(entry_script) = package.entry_script {
            aggregate_bytes += entry_script.len();
            mod_sources.push(WebLuaModSource {
                entry_script,
                files: package_bytes,
                identity: WebLuaModIdentity {
                    id: package.id.clone(),
                    name: package.name.clone(),
                    version: package.version.clone(),
                },
            });
        }
        for boneyard in package.boneyards {
            let bytes = decode_base64(
                &boneyard.bytes_base64,
                &format!("{} {}", package.id, boneyard.target),
                MAXIMUM_BONEYARD_BYTES,
            )?;
            aggregate_bytes += bytes.len();
            let entry = project_mod_boneyard(&package.id, &package.name, &boneyard.target, &bytes)?;
            let normalized_target = boneyard.target.to_lowercase();
            if let Some(&index) = boneyard_indices.get(&normalized_target) {
                // Map.set replaces the value while retaining its first position.
                boneyards[index] = entry;
            } else {
                boneyard_indices.insert(normalized_target, boneyards.len());
                boneyards.push(entry);
            }
        }
        manifest_mods.push(GameContentIdentity {
            content_sha256: package.content_sha256.clone(),
            id: package.id.clone(),
            version: package.version.clone(),
        });
        summary_mods.push(WebSessionModSummary {
            assets: package_assets,
            content_sha256: package.content_sha256,
            graph_sha256: None,
            id: package.id,
            name: package.name,
            slug: package.slug,
            version: package.version,
        });
    }
    if aggregate_bytes > MAXIMUM_CONTENT_BYTES {
        return Err("session content exceeds its aggregate byte limit".into());
    }
    if mod_sources.len() > MAXIMUM_LUA_MODS {
        return Err(format!("session content has more than {MAXIMUM_LUA_MODS} Lua mods"));
    }
    Ok(MaterializedWebSessionContent {
        assets,
        boneyards,
        // Source materialization precedes its separate Lua definition stage.
        compiled_mods: Vec::new(),
        manifest: GameContentManifest {
            manifest_sha256: manifest_sha256.clone(),
            mods: manifest_mods,
        },
        mod_sources,
        summary: WebSessionContentSummary { manifest_sha256, mods: summary_mods },
    })
}

/// `projectModBoneyard`: identity includes the original target spelling and
/// source bytes; geometry uses the pinned JavaScript property's insertion order.
pub fn project_mod_boneyard(
    mod_id: &str,
    mod_name: &str,
    target: &str,
    bytes: &[u8],
) -> Result<ModBoneyardEntry, String> {
    let source_sha256 = sha256(bytes);
    let scene = decode_boneyard(bytes)?;
    let basename = target.rsplit('/').next().unwrap_or(target);
    let name = basename.strip_suffix(".boneyard").unwrap_or(basename);
    let mut slug = String::new();
    let mut previous_separator = false;
    for character in name.chars() {
        if character.is_ascii_alphanumeric() || matches!(character, '_' | '-') {
            slug.push(character.to_ascii_lowercase());
            previous_separator = false;
        } else if !previous_separator {
            slug.push('-');
            previous_separator = true;
        }
    }
    slug.truncate(64);
    let identity = sha256(format!("{target}\0{source_sha256}").as_bytes());
    let geometry = solomon_assets::json_value::stringify(&boneyard_scene_source_value(&scene))?;
    Ok(ModBoneyardEntry {
        choice: BoneyardChoice {
            id: format!("mod:{mod_id}:{slug}:{}", &identity[..12]),
            name: name.to_owned(),
            source: BoneyardSource::Mod,
            mod_id: Some(mod_id.to_owned()),
            mod_name: Some(mod_name.to_owned()),
        },
        geometry_sha256: sha256(geometry.as_bytes()),
        scene,
        source_sha256,
        web_lua_content_id: None,
    })
}

fn parse_mod(value: &LuaValue, index: usize) -> Result<PackageMod, String> {
    let field = format!("session content mods[{index}]");
    let source = object(Some(value), &field)?;
    exact_keys(
        source,
        &[
            "boneyards",
            "contentSha256",
            "entryScript",
            "files",
            "id",
            "name",
            "priority",
            "slug",
            "version",
        ],
        &field,
    )?;
    let id = text(source.get("id"), &format!("{field}.id"), 128)?;
    if !id.as_bytes()[0].is_ascii_alphanumeric()
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(format!("{field}.id is invalid"));
    }
    let name = text(source.get("name"), &format!("{field}.name"), 80)?;
    let slug = text(source.get("slug"), &format!("{field}.slug"), 80)?;
    let version = text(source.get("version"), &format!("{field}.version"), 64)?;
    let content_sha256 = digest(source.get("contentSha256"), &format!("{field}.contentSha256"))?;
    let priority = source.get("priority").and_then(LuaValue::as_number);
    if priority
        .is_none_or(|value| value.fract() != 0.0 || !(-100_000.0..=100_000.0).contains(&value))
    {
        return Err(format!("{field}.priority is invalid"));
    }
    let entry_script = match source.get("entryScript") {
        Some(LuaValue::Null) => None,
        value => Some(text(value, &format!("{field}.entryScript"), MAXIMUM_SCRIPT_BYTES)?),
    };
    if entry_script.as_ref().is_some_and(|script| script.len() > MAXIMUM_SCRIPT_BYTES) {
        return Err(format!("{field}.entryScript exceeds its byte limit"));
    }
    let Some(files) = source.get("files").and_then(LuaValue::as_array) else {
        return Err(format!("{field}.files is invalid"));
    };
    if files.len() > MAXIMUM_FILES {
        return Err(format!("{field}.files is invalid"));
    }
    let mut portable_paths = HashSet::new();
    let mut parsed_files = Vec::with_capacity(files.len());
    for (index, value) in files.iter().enumerate() {
        parsed_files.push(parse_file(value, index, &field, &id, &mut portable_paths)?);
    }
    let Some(boneyards) = source.get("boneyards").and_then(LuaValue::as_array) else {
        return Err(format!("{field}.boneyards is invalid"));
    };
    if boneyards.len() > 256 {
        return Err(format!("{field}.boneyards is invalid"));
    }
    let mut parsed_boneyards = Vec::with_capacity(boneyards.len());
    for (index, value) in boneyards.iter().enumerate() {
        let field = format!("{field}.boneyards[{index}]");
        let source = object(Some(value), &field)?;
        exact_keys(source, &["bytesBase64", "target"], &field)?;
        let target = text(source.get("target"), &format!("{field}.target"), 240)?;
        if !package_path(&target, &["data/levels/", "sandbox/DarkCloud/mylevels/"], &["boneyard"]) {
            return Err(format!("{field}.target is invalid"));
        }
        let bytes_base64 = text(
            source.get("bytesBase64"),
            &format!("{field}.bytesBase64"),
            base64_limit(MAXIMUM_BONEYARD_BYTES),
        )?;
        parsed_boneyards.push(PackageBoneyard { bytes_base64, target });
    }
    Ok(PackageMod {
        boneyards: parsed_boneyards,
        content_sha256,
        entry_script,
        files: parsed_files,
        id,
        name,
        slug,
        version,
    })
}

fn parse_file(
    value: &LuaValue,
    index: usize,
    mod_field: &str,
    mod_id: &str,
    portable_paths: &mut HashSet<String>,
) -> Result<PackageFile, String> {
    let field = format!("{mod_field}.files[{index}]");
    let source = object(Some(value), &field)?;
    exact_keys(
        source,
        &["byteLength", "bytesBase64", "contentType", "kind", "path", "sha256"],
        &field,
    )?;
    let path = text(source.get("path"), &format!("{field}.path"), 240)?;
    if !package_path(
        &path,
        &["sprites/", "art/", "audio/", "levels/", "scenes/"],
        &["boneyard", "bundle", "json", "mp3", "ogg", "png", "wav"],
    ) || !portable_paths.insert(path.to_lowercase())
    {
        return Err(format!("{field}.path is invalid or duplicated"));
    }
    let kind = text(source.get("kind"), &format!("{field}.kind"), 64)?;
    let content_type = text(source.get("contentType"), &format!("{field}.contentType"), 128)?;
    if expected_package_file(&path) != Some((kind.as_str(), content_type.as_str())) {
        return Err(format!("{field} kind or contentType does not match its typed package path"));
    }
    let bytes_base64 = text(
        source.get("bytesBase64"),
        &format!("{field}.bytesBase64"),
        base64_limit(MAXIMUM_FILE_BYTES),
    )?;
    let bytes = decode_base64(&bytes_base64, &format!("{mod_field} {path}"), MAXIMUM_FILE_BYTES)?;
    if source.get("byteLength").and_then(LuaValue::as_number) != Some(bytes.len() as f64) {
        return Err(format!("{field}.byteLength does not match its package bytes"));
    }
    let digest = digest(source.get("sha256"), &format!("{field}.sha256"))?;
    if sha256(&bytes) != digest {
        return Err(format!("{field}.sha256 does not match its package bytes"));
    }
    Ok(PackageFile {
        asset: GameModAsset {
            byte_length: bytes.len() as u64,
            content_type,
            kind,
            mod_id: mod_id.to_owned(),
            path,
            sha256: digest,
        },
        bytes,
    })
}

fn expected_package_file(path: &str) -> Option<(&'static str, &'static str)> {
    let (_, extension) = path.rsplit_once('.')?;
    if path.starts_with("sprites/") || path.starts_with("art/") {
        return match extension {
            "png" => Some(("image", "image/png")),
            "bundle" => Some(("sprite-bundle", "application/vnd.solomon-dark.sprite-bundle")),
            "json" => Some(("art-metadata", "application/json")),
            _ => None,
        };
    }
    if path.starts_with("audio/") {
        let content_type = match extension {
            "ogg" => "audio/ogg",
            "wav" => "audio/wav",
            "mp3" => "audio/mpeg",
            _ => return None,
        };
        return Some(("audio", content_type));
    }
    if path.starts_with("levels/") {
        return match extension {
            "boneyard" => Some(("boneyard", "application/vnd.solomon-dark.boneyard")),
            "json" => Some(("level-metadata", "application/json")),
            _ => None,
        };
    }
    if path.starts_with("scenes/") && extension == "json" {
        return Some(("scene", "application/json"));
    }
    None
}

fn package_path(path: &str, prefixes: &[&str], extensions: &[&str]) -> bool {
    let Some(remainder) = prefixes.iter().find_map(|prefix| path.strip_prefix(prefix)) else {
        return false;
    };
    let Some((stem, extension)) = remainder.rsplit_once('.') else { return false };
    !stem.is_empty()
        && extensions.contains(&extension)
        && !remainder.contains(['\n', '\r', '\u{2028}', '\u{2029}'])
}

fn decode_base64(value: &str, field: &str, maximum: usize) -> Result<Vec<u8>, String> {
    let invalid = || format!("{field} has invalid package bytes");
    let bytes = STANDARD.decode(value).map_err(|_| invalid())?;
    if bytes.is_empty() || bytes.len() > maximum || STANDARD.encode(&bytes) != value {
        return Err(invalid());
    }
    Ok(bytes)
}

fn base64_limit(bytes: usize) -> usize {
    bytes.div_ceil(3) * 4
}

fn object<'a>(value: Option<&'a LuaValue>, field: &str) -> Result<&'a LuaObject, String> {
    value.and_then(LuaValue::as_object).ok_or_else(|| format!("{field} must be an object"))
}

fn exact_keys(source: &LuaObject, expected: &[&str], field: &str) -> Result<(), String> {
    if source.iter().any(|(key, _)| !expected.contains(&key))
        || expected.iter().any(|key| source.get(key).is_none())
    {
        return Err(format!("{field} has invalid fields"));
    }
    Ok(())
}

fn text(value: Option<&LuaValue>, field: &str, maximum: usize) -> Result<String, String> {
    let Some(text) = value.and_then(LuaValue::as_str) else {
        return Err(format!("{field} must be a nonempty string of at most {maximum} characters"));
    };
    if text.is_empty() || text.encode_utf16().count() > maximum {
        return Err(format!("{field} must be a nonempty string of at most {maximum} characters"));
    }
    Ok(text.to_owned())
}

fn digest(value: Option<&LuaValue>, field: &str) -> Result<String, String> {
    let normalized = text(value, field, 64)?.to_lowercase();
    if normalized.len() != 64 || !normalized.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!("{field} must be a SHA-256 digest"));
    }
    Ok(normalized)
}
