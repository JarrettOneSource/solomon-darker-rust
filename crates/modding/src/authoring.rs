//! Source `web-lua-package-authoring.ts` and the native portions of `tools/sdmod/cli.mjs`.
//! Filesystem admission, compilation and package tests use the same compiler/VM as hosts.
//! The existing Node CLI retains its authoring templates, ZIP writer and art tooling.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::rc::Rc;

use base64::{Engine, engine::general_purpose::STANDARD};
use mlua::{Lua, MultiValue, Value};
use solomon_sim::modding::assets::GameModAsset;
use solomon_sim::modding::definition::*;
use solomon_sim::modding::{LuaObject, LuaValue};

use crate::assets::{ModAssetSource, compile_mod_assets, sha256};
use crate::content::compile_mod_content_catalog;
use crate::definition::error::WebLuaDefinitionErrorCode;
use crate::definition::runtime::{WebLuaDefinitionRuntime, WebLuaDefinitionRuntimeOptions};
use crate::definition::scripts::{
    MAXIMUM_INCLUDED_SCRIPTS, MAXIMUM_SCRIPT_BYTES, bundle_web_lua_entry_script, valid_script_path,
};
use crate::definition::{CompileWebLuaDefinitionOptions, compile_web_lua_definition};

const MAXIMUM_PACKAGE_FILES: usize = 256;
const MAXIMUM_PACKAGE_FILE_BYTES: usize = 16 * 1_024 * 1_024;

pub struct CheckedWebLuaPackage {
    pub compiled: CompiledWebLuaMod,
    pub entry_script: String,
    pub entry_script_path: String,
    pub files: BTreeMap<String, Vec<u8>>,
    pub manifest: LuaObject,
    pub root: PathBuf,
    pub scripts: Vec<(String, String)>,
}

pub fn check_web_lua_package(
    root: &Path,
    options: &CompileWebLuaDefinitionOptions,
) -> Result<CheckedWebLuaPackage, String> {
    let root = root.canonicalize().map_err(|error| error.to_string())?;
    let manifest = read_text(&root.join("manifest.json"))?;
    let manifest = serde_json::from_str(&manifest).map_err(|error| error.to_string())?;
    let manifest = manifest_value(&manifest)?;
    let identity = WebLuaModIdentity {
        id: text(&manifest, "id").into(),
        name: text(&manifest, "name").into(),
        version: text(&manifest, "version").into(),
    };
    let runtime = manifest.get("runtime").and_then(LuaValue::as_object).expect("checked runtime");
    let entry_script_path = text(runtime, "entryScript").to_owned();
    let entry_script = read_text(&resolve_inside(&root, &entry_script_path)?)?;
    if entry_script.len() > MAXIMUM_SCRIPT_BYTES {
        return Err(format!("entry script exceeds {MAXIMUM_SCRIPT_BYTES} bytes"));
    }
    let paths = walk_package(&root)?;
    let files = package_files(&root, &paths)?;
    let scripts = package_scripts(&root, &paths, &entry_script_path)?;
    bundle_web_lua_entry_script(&entry_script, &scripts)?;
    let mut runtime = WebLuaDefinitionRuntime::new(WebLuaDefinitionRuntimeOptions {
        entry_script: entry_script_path.clone(),
        identity: identity.clone(),
        scripts: scripts.clone(),
    })?;
    let definition = runtime.run(&entry_script).map_err(|error| error.to_string())?;
    let compiled = compile_web_lua_definition(&identity, &definition, options)
        .map_err(|error| error.to_string())?;
    for asset in &compiled.assets {
        let mut candidates = Vec::new();
        for key in ["file", "image", "path", "source"] {
            if let Some(path) = asset.fields.get(key).and_then(LuaValue::as_str) {
                candidates.push(path);
            }
        }
        if candidates.is_empty() {
            return Err(format!("asset {} has no owned package path", asset.key));
        }
        for path in candidates {
            if !files.contains_key(path) {
                return Err(format!("asset {} references missing package file: {path}", asset.key));
            }
        }
    }
    Ok(CheckedWebLuaPackage {
        compiled,
        entry_script,
        entry_script_path,
        files,
        manifest,
        root,
        scripts,
    })
}

/// The public check/new/pack/test commands use full production-shaped admission.
/// Source migrate intentionally stops after check_web_lua_package instead.
pub fn admit_prepared_package(checked: &CheckedWebLuaPackage) -> Result<(), String> {
    let mut descriptors = Vec::with_capacity(checked.files.len());
    for (path, bytes) in &checked.files {
        let (kind, content_type) = typed_file(path)?;
        descriptors.push(GameModAsset {
            byte_length: bytes.len() as u64,
            content_type: content_type.into(),
            kind: kind.into(),
            mod_id: checked.compiled.identity.id.clone(),
            path: path.clone(),
            sha256: sha256(bytes),
        });
    }
    let sources = [ModAssetSource {
        files: checked.files.clone(),
        identity: checked.compiled.identity.clone(),
    }];
    let mods = std::slice::from_ref(&checked.compiled);
    let assets = compile_mod_assets(&descriptors, mods, &sources)?;
    compile_mod_content_catalog(mods, &assets)?;
    for content in &checked.compiled.content {
        if content.content_kind != WebLuaContentKind::Boneyard {
            continue;
        }
        let path = content.fields.get("source").and_then(LuaValue::as_str);
        let bytes = path.and_then(|path| checked.files.get(path));
        let (Some(path), Some(bytes)) = (path, bytes) else {
            return Err(format!(
                "{}:{} Boneyard source is not packaged",
                checked.compiled.identity.id, content.key
            ));
        };
        crate::package::project_mod_boneyard(
            &checked.compiled.identity.id,
            &checked.compiled.identity.name,
            path,
            bytes,
        )?;
    }
    Ok(())
}

pub fn bundle_checked_package(checked: &CheckedWebLuaPackage) -> Result<String, String> {
    let bundled = bundle_web_lua_entry_script(&checked.entry_script, &checked.scripts)?;
    if !checked.scripts.is_empty() {
        let mut runtime = WebLuaDefinitionRuntime::new(WebLuaDefinitionRuntimeOptions {
            entry_script: checked.entry_script_path.clone(),
            identity: checked.compiled.identity.clone(),
            scripts: Vec::new(),
        })?;
        let definition = runtime.run(&bundled).map_err(|error| error.to_string())?;
        let compiled = compile_web_lua_definition(
            &checked.compiled.identity,
            &definition,
            &Default::default(),
        )
        .map_err(|error| error.to_string())?;
        if compiled.graph_sha256 != checked.compiled.graph_sha256 {
            return Err("the bundled entry script compiled to a different graph than the package source; report this with the mod source".into());
        }
    }
    Ok(bundled)
}

pub struct PackageTestResult {
    pub output: Vec<String>,
    pub tests: usize,
}

pub fn run_package_tests(checked: &CheckedWebLuaPackage) -> Result<PackageTestResult, String> {
    let directory = checked.root.join("tests");
    let entries = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(PackageTestResult { output: Vec::new(), tests: 0 });
        }
        Err(error) => return Err(error.to_string()),
    };
    let mut names = Vec::new();
    for entry in entries {
        let name =
            entry.map_err(|error| error.to_string())?.file_name().to_string_lossy().into_owned();
        if name.ends_with(".lua") {
            names.push(name);
        }
    }
    names.sort_by(|left, right| left.encode_utf16().cmp(right.encode_utf16()));
    let graph: LuaValue = serde_json::from_str(&checked.compiled.canonical_json)
        .map_err(|error| error.to_string())?;
    let graph_assignment = format!("mod={}", lua_literal(&graph)?);
    let output = Rc::new(RefCell::new(Vec::new()));
    for name in &names {
        let code = read_text(&directory.join(name))?;
        let lua = Lua::new();
        lua.load("io=nil; os=nil; package=nil; require=nil; debug=nil; load=nil; loadfile=nil; dofile=nil")
            .exec().map_err(|error| error.to_string())?;
        // Source constructs a Lua literal, rather than pushing host JSON numbers.
        // This retains integer literals and Source string-escape behavior.
        lua.load(&graph_assignment)
            .set_name(&graph_assignment)
            .exec()
            .map_err(|error| error.to_string())?;
        let printed = Rc::clone(&output);
        let print = lua
            .create_function(move |lua, arguments: MultiValue| {
                let tostring: mlua::Function = lua.globals().get("tostring")?;
                let mut words = Vec::with_capacity(arguments.len());
                for value in arguments {
                    words.push(tostring.call::<String>(value)?);
                }
                printed.borrow_mut().push(words.join("\t"));
                Ok(())
            })
            .map_err(|error| error.to_string())?;
        lua.globals().set("print", print).map_err(|error| error.to_string())?;
        let returned = lua
            .load(&code)
            .set_name(&code)
            .eval::<MultiValue>()
            .map_err(|error| error.to_string())?;
        // Wasmoon distinguishes no returns (undefined) from an explicit nil (null).
        if !returned.is_empty() && !matches!(returned.front(), Some(Value::Boolean(true))) {
            return Err(format!("{name} must return true or no value"));
        }
    }
    let output = output.borrow().clone();
    Ok(PackageTestResult { output, tests: names.len() })
}

fn lua_literal(value: &LuaValue) -> Result<String, String> {
    match value {
        LuaValue::Null => Ok("nil".into()),
        LuaValue::Array(values) => {
            let mut items = Vec::with_capacity(values.len());
            for value in values {
                items.push(lua_literal(value)?);
            }
            Ok(format!("{{{}}}", items.join(",")))
        }
        LuaValue::Object(values) => {
            let mut fields = Vec::with_capacity(values.len());
            for (key, value) in values.iter() {
                let key = crate::json::stringify(&LuaValue::String(key.into()))?;
                fields.push(format!("[{key}]={}", lua_literal(value)?));
            }
            Ok(format!("{{{}}}", fields.join(",")))
        }
        _ => crate::json::stringify(value),
    }
}

/// JSON contract for the retained Node authoring frontend, not a second compiler.
pub fn checked_package_value(checked: &CheckedWebLuaPackage) -> Result<LuaValue, String> {
    let mut files = LuaObject::new();
    for (path, bytes) in &checked.files {
        files.insert(path.clone(), LuaValue::String(STANDARD.encode(bytes)));
    }
    let mut scripts = LuaObject::new();
    for (path, code) in &checked.scripts {
        scripts.insert(path.clone(), LuaValue::String(code.clone()));
    }
    Ok(LuaValue::Object(
        [
            ("compiled".into(), crate::json::lua_value(&checked.compiled)?),
            ("entryScript".into(), LuaValue::String(checked.entry_script.clone())),
            ("entryScriptPath".into(), LuaValue::String(checked.entry_script_path.clone())),
            ("files".into(), LuaValue::Object(files)),
            ("manifest".into(), LuaValue::Object(checked.manifest.clone())),
            ("root".into(), LuaValue::String(checked.root.to_string_lossy().into_owned())),
            ("scripts".into(), LuaValue::Object(scripts)),
        ]
        .into_iter()
        .collect(),
    ))
}

pub fn package_summary(checked: &CheckedWebLuaPackage) -> Result<LuaValue, String> {
    let compiled = &checked.compiled;
    crate::json::lua_value(&serde_json::json!({
        "apiVersion": compiled.api_version,
        "assets": compiled.assets.len(),
        "capabilities": compiled.capabilities,
        "content": compiled.content.len(),
        "graphSha256": compiled.graph_sha256,
        "id": compiled.identity.id,
        "reducers": compiled.reducers.len(),
        "rules": compiled.rules.len(),
        "version": compiled.identity.version,
    }))
}

pub fn authoring_reference_inventory() -> Result<LuaValue, String> {
    let mut fields = LuaObject::new();
    let mut art_slots = LuaObject::new();
    for kind in WebLuaContentKind::ALL {
        let (allowed, required) = crate::definition::content_schema(kind);
        let mut allowed = allowed.to_vec();
        allowed.extend(["description", "name"]);
        allowed.sort_unstable();
        allowed.dedup();
        let mut required = required.to_vec();
        required.sort_unstable();
        fields.insert(
            kind.as_str().into(),
            crate::json::lua_value(&serde_json::json!({
                "allowed": allowed, "required": required,
            }))?,
        );
        art_slots.insert(kind.as_str().into(), crate::json::lua_value(&kind.art_slots())?);
    }
    let mut inventory = match crate::json::lua_value(&serde_json::json!({
        "apiVersion": WEB_LUA_DEFINITION_API_VERSION,
        "contentKinds": WebLuaContentKind::ALL.map(WebLuaContentKind::as_str),
        "definitionErrorCodes": WebLuaDefinitionErrorCode::ALL.map(WebLuaDefinitionErrorCode::as_str),
        "maximumIncludedScripts": MAXIMUM_INCLUDED_SCRIPTS,
        "maximumScriptBytes": MAXIMUM_SCRIPT_BYTES,
        "ruleEventNames": WEB_LUA_RULE_EVENT_NAMES,
        "scopeKinds": WebLuaScopeKind::ALL.map(WebLuaScopeKind::as_str),
        "artAliases": WEB_LUA_ART_ALIAS_NAMES,
        "effects": WEB_LUA_EFFECT_NAMES,
        "prefabs": WEB_LUA_PREFAB_NAMES,
        "rules": WEB_LUA_RULE_NAMES,
        "schemas": WEB_LUA_SCHEMA_NAMES,
        "predicateFields": WEB_LUA_PREDICATE_FIELDS,
    }))? {
        LuaValue::Object(value) => value,
        _ => unreachable!("the reference inventory is an object"),
    };
    inventory.insert("contentSchemaFields".into(), LuaValue::Object(fields));
    inventory.insert("contentArtSlots".into(), LuaValue::Object(art_slots));
    Ok(LuaValue::Object(inventory))
}

fn read_text(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|error| error.to_string())?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn manifest_value(value: &LuaValue) -> Result<LuaObject, String> {
    let source = object(value, "manifest")?;
    exact_keys(
        source,
        &["$schema", "id", "name", "overlays", "priority", "requiredMods", "runtime", "version"],
        "manifest",
    )?;
    if !source.get("id").and_then(LuaValue::as_str).is_some_and(valid_web_lua_content_key) {
        return Err("manifest.id must be a canonical lowercase package id".into());
    }
    if !source
        .get("name")
        .and_then(LuaValue::as_str)
        .is_some_and(|name| (1..=80).contains(&name.encode_utf16().count()))
    {
        return Err("manifest.name must contain 1..80 characters".into());
    }
    if !source.get("version").and_then(LuaValue::as_str).is_some_and(valid_version) {
        return Err("manifest.version is invalid".into());
    }
    if let Some(priority) = source.get("priority")
        && !priority
            .as_number()
            .is_some_and(|value| value.fract() == 0.0 && value.abs() <= 100_000.0)
    {
        return Err("manifest.priority is invalid".into());
    }
    if source.get("overlays").is_some_and(|value| value.as_array().is_none()) {
        return Err("manifest.overlays must be an array".into());
    }
    if let Some(required) = source.get("requiredMods") {
        let mut seen = HashSet::new();
        let valid = required.as_array().is_some_and(|values| {
            values.iter().all(|value| {
                value.as_str().is_some_and(|id| valid_web_lua_content_key(id) && seen.insert(id))
            })
        });
        if !valid {
            return Err("manifest.requiredMods is invalid".into());
        }
    }
    let runtime = source.get("runtime").ok_or("manifest.runtime must be an object")?;
    let runtime = object(runtime, "manifest.runtime")?;
    exact_keys(runtime, &["apiVersion", "entryScript"], "manifest.runtime")?;
    if runtime.get("apiVersion").and_then(LuaValue::as_str) != Some(WEB_LUA_DEFINITION_API_VERSION)
    {
        return Err(format!(
            "manifest.runtime.apiVersion must be {WEB_LUA_DEFINITION_API_VERSION}"
        ));
    }
    if !runtime.get("entryScript").and_then(LuaValue::as_str).is_some_and(valid_script_path) {
        return Err("manifest.runtime.entryScript must name a Lua file under scripts/".into());
    }
    let mut manifest = LuaObject::new();
    for key in
        ["$schema", "id", "name", "overlays", "priority", "requiredMods", "runtime", "version"]
    {
        if let Some(value) = source.get(key) {
            if key == "$schema" && value.as_str().is_none() {
                continue;
            }
            manifest.insert(key.into(), value.clone());
        }
    }
    Ok(manifest)
}

fn valid_version(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 64
        && bytes[0].is_ascii_alphanumeric()
        && bytes.iter().all(|value| value.is_ascii_alphanumeric() || b"._+-".contains(value))
}

fn object<'a>(value: &'a LuaValue, field: &str) -> Result<&'a LuaObject, String> {
    value.as_object().ok_or_else(|| format!("{field} must be an object"))
}

fn text<'a>(source: &'a LuaObject, key: &str) -> &'a str {
    source.get(key).and_then(LuaValue::as_str).expect("validated manifest text")
}

fn exact_keys(source: &LuaObject, allowed: &[&str], field: &str) -> Result<(), String> {
    let unknown = source
        .iter()
        .filter_map(|(key, _)| (!allowed.contains(&key)).then_some(key))
        .collect::<Vec<_>>();
    if !unknown.is_empty() {
        return Err(format!("{field} contains unknown fields: {}", unknown.join(", ")));
    }
    Ok(())
}

fn resolve_inside(root: &Path, path: &str) -> Result<PathBuf, String> {
    if path.is_empty()
        || path.contains('\\')
        || path.starts_with('/')
        || path.split('/').any(|part| part == "..")
    {
        return Err(format!("package path is invalid: {path}"));
    }
    let mut result = root.to_owned();
    for component in Path::new(path).components() {
        if let Component::Normal(value) = component {
            result.push(value);
        }
    }
    if result == root {
        return Err(format!("package path escapes its root: {path}"));
    }
    Ok(result)
}

fn walk_package(root: &Path) -> Result<Vec<String>, String> {
    fn walk(root: &Path, directory: &Path, output: &mut Vec<String>) -> Result<(), String> {
        for entry in fs::read_dir(directory).map_err(|error| error.to_string())? {
            let entry = entry.map_err(|error| error.to_string())?;
            if entry.file_name() == ".git" || entry.file_name() == "node_modules" {
                continue;
            }
            let kind = entry.file_type().map_err(|error| error.to_string())?;
            if kind.is_dir() {
                walk(root, &entry.path(), output)?;
            } else if kind.is_file() {
                output.push(
                    entry
                        .path()
                        .strip_prefix(root)
                        .expect("walked package root")
                        .components()
                        .map(|part| part.as_os_str().to_string_lossy())
                        .collect::<Vec<_>>()
                        .join("/"),
                );
            }
        }
        Ok(())
    }
    let mut output = Vec::new();
    walk(root, root, &mut output)?;
    output.sort_by(|left, right| left.encode_utf16().cmp(right.encode_utf16()));
    Ok(output)
}

fn package_files(root: &Path, paths: &[String]) -> Result<BTreeMap<String, Vec<u8>>, String> {
    let paths = paths.iter().filter(|path| asset_path(path)).collect::<Vec<_>>();
    if paths.len() > MAXIMUM_PACKAGE_FILES {
        return Err(format!("package has more than {MAXIMUM_PACKAGE_FILES} typed asset files"));
    }
    let mut files = BTreeMap::new();
    for path in paths {
        let bytes = fs::read(resolve_inside(root, path)?).map_err(|error| error.to_string())?;
        if bytes.is_empty() || bytes.len() > MAXIMUM_PACKAGE_FILE_BYTES {
            return Err(format!("package asset has invalid size: {path}"));
        }
        files.insert(path.clone(), bytes);
    }
    Ok(files)
}

fn asset_path(path: &str) -> bool {
    let Some((prefix, rest)) = path.split_once('/') else { return false };
    let Some((stem, extension)) = rest.rsplit_once('.') else { return false };
    ["sprites", "art", "audio", "levels", "scenes"].contains(&prefix)
        && !stem.is_empty()
        && !rest.contains(['\n', '\r', '\u{2028}', '\u{2029}'])
        && ["boneyard", "bundle", "json", "mp3", "ogg", "png", "wav"].contains(&extension)
}

fn package_scripts(
    root: &Path,
    paths: &[String],
    entry: &str,
) -> Result<Vec<(String, String)>, String> {
    let mut scripts = Vec::new();
    for path in paths {
        if path == entry || !valid_script_path(path) {
            continue;
        }
        let bytes = fs::read(resolve_inside(root, path)?).map_err(|error| error.to_string())?;
        if bytes.len() > MAXIMUM_SCRIPT_BYTES {
            return Err(format!("package script exceeds {MAXIMUM_SCRIPT_BYTES} bytes: {path}"));
        }
        scripts.push((path.clone(), String::from_utf8_lossy(&bytes).into_owned()));
    }
    Ok(scripts)
}

fn typed_file(path: &str) -> Result<(&'static str, &'static str), String> {
    if path.ends_with(".png") {
        return Ok(("image", "image/png"));
    }
    if path.ends_with(".ogg") {
        return Ok(("audio", "audio/ogg"));
    }
    if path.ends_with(".wav") {
        return Ok(("audio", "audio/wav"));
    }
    if path.ends_with(".mp3") {
        return Ok(("audio", "audio/mpeg"));
    }
    if path.ends_with(".boneyard") {
        return Ok(("boneyard", "application/vnd.solomon-dark.boneyard"));
    }
    if path.starts_with("scenes/") && path.ends_with(".json") {
        return Ok(("scene", "application/json"));
    }
    if path.starts_with("art/") && path.ends_with(".json") {
        return Ok(("art-metadata", "application/json"));
    }
    if path.starts_with("levels/") && path.ends_with(".json") {
        return Ok(("level-metadata", "application/json"));
    }
    if path.ends_with(".bundle") {
        return Ok(("sprite-bundle", "application/vnd.solomon-dark.sprite-bundle"));
    }
    Err(format!("unsupported typed package file: {path}"))
}
