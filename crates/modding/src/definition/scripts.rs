//! Packed includes from `web-lua-script-bundle.ts`.

use solomon_sim::modding::LuaValue;

pub const BUNDLE_MARKER: &str = "--@sd-bundle ";
pub const MAXIMUM_SCRIPT_BYTES: usize = 256 * 1_024;
pub const MAXIMUM_INCLUDED_SCRIPTS: usize = 64;

/// Authoring preserves entry line numbers by appending the includes as a Lua comment.
pub fn bundle_web_lua_entry_script(
    entry_script: &str,
    scripts: &[(String, String)],
) -> Result<String, String> {
    validate_web_lua_script_set(scripts)?;
    let mut bundled = entry_script.to_owned();
    if !scripts.is_empty() {
        let mut ordered = scripts.iter().collect::<Vec<_>>();
        ordered.sort_by(|(left, _), (right, _)| left.encode_utf16().cmp(right.encode_utf16()));
        let mut values = solomon_sim::modding::LuaObject::new();
        for (path, text) in ordered {
            values.insert(path.clone(), LuaValue::String(text.clone()));
        }
        if !bundled.ends_with('\n') {
            bundled.push('\n');
        }
        bundled.push_str(BUNDLE_MARKER);
        bundled.push_str(&crate::json::stringify(&LuaValue::Object(values))?);
        bundled.push('\n');
    }
    if bundled.len() > MAXIMUM_SCRIPT_BYTES {
        return Err(format!(
            "the entry script and its included scripts total {} bytes, above the {MAXIMUM_SCRIPT_BYTES} byte limit; move large tables into assets or trim the scripts",
            bundled.len()
        ));
    }
    Ok(bundled)
}

pub fn read_web_lua_script_bundle(code: &str) -> Result<Option<Vec<(String, String)>>, String> {
    let mut bundle_line = "";
    for line in code.split('\n').rev() {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if trim_javascript(line).is_empty() {
            continue;
        }
        bundle_line = line;
        break;
    }
    let Some(json) = bundle_line.strip_prefix(BUNDLE_MARKER) else { return Ok(None) };
    let value: LuaValue = serde_json::from_str(json)
        .map_err(|_| "the packed script bundle is not valid JSON".to_owned())?;
    let Some(object) = value.as_object() else {
        return Err("the packed script bundle must be an object of script paths to Lua text".into());
    };
    let mut scripts = Vec::new();
    for (path, value) in object.iter() {
        let Some(text) = value.as_str().filter(|_| valid_script_path(path)) else {
            return Err(format!("the packed script bundle contains an invalid entry: {path}"));
        };
        scripts.push((path.to_owned(), text.to_owned()));
    }
    validate_web_lua_script_set(&scripts)?;
    Ok(Some(scripts))
}

pub fn validate_web_lua_script_set(scripts: &[(String, String)]) -> Result<(), String> {
    if scripts.len() > MAXIMUM_INCLUDED_SCRIPTS {
        return Err(format!(
            "a package may include at most {MAXIMUM_INCLUDED_SCRIPTS} extra scripts"
        ));
    }
    let mut total = 0;
    for (path, text) in scripts {
        if !valid_script_path(path) {
            return Err(format!("included script path is invalid: {path}"));
        }
        total += text.len();
    }
    if total > MAXIMUM_SCRIPT_BYTES {
        return Err(format!("included scripts exceed {MAXIMUM_SCRIPT_BYTES} bytes in total"));
    }
    Ok(())
}

pub fn valid_script_path(path: &str) -> bool {
    path.strip_prefix("scripts/").is_some_and(|path| {
        path.len() > ".lua".len()
            && path.ends_with(".lua")
            && !path.contains(['\n', '\r', '\u{2028}', '\u{2029}'])
    })
}

pub fn trim_javascript(value: &str) -> &str {
    value.trim_matches(|character| {
        matches!(character,
        '\u{0009}'..='\u{000d}' | '\u{0020}' | '\u{00a0}' | '\u{1680}'
        | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}'
        | '\u{205f}' | '\u{3000}' | '\u{feff}')
    })
}
