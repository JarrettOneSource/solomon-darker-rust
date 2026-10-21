//! `protocol/codecs/mod-content.ts::modContentProjection` and its value/frame checks.

use std::collections::BTreeSet;

use super::*;
use crate::modding::assets::ModSpriteFrame;
use crate::modding::{LuaObject, LuaValue};

impl ModContentProjection {
    /// Decode the actual host projection before applying the client's revision gate.
    pub fn read(source: &LuaObject) -> Result<Self, String> {
        only_keys(
            source,
            "message",
            &["boasts", "content", "manifestSha256", "powerups", "revision", "statuses"],
        )?;
        let mut content = Vec::new();
        let mut content_ids = BTreeSet::new();
        for (index, value) in rows(source.get("content"), "content", 4096)?.iter().enumerate() {
            let field = format!("content[{index}]");
            let entry = read_content(record(Some(value), &field)?, &field, &mut content_ids)?;
            content.push(entry);
        }

        let mut boasts = Vec::new();
        let mut boast_ids = BTreeSet::new();
        for (index, value) in rows(source.get("boasts"), "boasts", 4096)?.iter().enumerate() {
            let field = format!("boasts[{index}]");
            let body = record(Some(value), &field)?;
            only_keys(
                body,
                &field,
                &[
                    "contentId",
                    "failureProducers",
                    "icon",
                    "instruction",
                    "modId",
                    "name",
                    "randomSkillChoices",
                    "response",
                    "scoreMultiplier",
                    "statement",
                    "successWave",
                ],
            )?;
            let content_id = text(body.get("contentId"), &format!("{field}.contentId"), 19)?;
            let mod_id = text(body.get("modId"), &format!("{field}.modId"), 128)?;
            let name = text(body.get("name"), &format!("{field}.name"), 128)?;
            let entry = content.iter().find(|entry| entry.content_id == content_id);
            let Some(entry) = entry.filter(|entry| {
                entry.content_kind == WebLuaContentKind::Boast
                    && entry.mod_id == mod_id
                    && entry.name == name
            }) else {
                return Err(format!("{field} does not match its Boast content entry"));
            };
            if !boast_ids.insert(content_id.clone()) {
                return Err(format!("{field} does not match its Boast content entry"));
            }
            let failure_producers = read_failure_producers(body, &field)?;
            let icon = read_icon(body.get("icon"), &format!("{field}.icon"), entry)?;
            boasts.push(ModBoastProjectionEntry {
                content_id,
                failure_producers,
                icon,
                instruction: bytes(body.get("instruction"), &format!("{field}.instruction"), 1024)?,
                mod_id,
                name,
                random_skill_choices: body
                    .get("randomSkillChoices")
                    .and_then(LuaValue::as_bool)
                    .ok_or_else(|| format!("{field}.randomSkillChoices must be boolean"))?,
                response: bytes(body.get("response"), &format!("{field}.response"), 1024)?,
                score_multiplier: within(
                    body.get("scoreMultiplier"),
                    &format!("{field}.scoreMultiplier"),
                    1.0,
                    10.0,
                )?,
                statement: bytes(body.get("statement"), &format!("{field}.statement"), 1024)?,
                success_wave: integer_within(
                    body.get("successWave"),
                    &format!("{field}.successWave"),
                    1.0,
                    10000.0,
                )? as u32,
            });
        }

        let mut powerups = Vec::new();
        let mut instance_ids = Vec::new();
        for (index, value) in rows(source.get("powerups"), "powerups", 1024)?.iter().enumerate() {
            let field = format!("powerups[{index}]");
            let body = record(Some(value), &field)?;
            only_keys(body, &field, &["contentId", "id", "spawnedTick", "x", "y"])?;
            let content_id = text(body.get("contentId"), &format!("{field}.contentId"), 19)?;
            let id = positive_integer(body.get("id"), &format!("{field}.id"))?;
            if !content.iter().any(|entry| {
                entry.content_id == content_id && entry.content_kind == WebLuaContentKind::Powerup
            }) || instance_ids.contains(&id)
            {
                return Err(format!("{field} is invalid"));
            }
            instance_ids.push(id);
            powerups.push(ModPowerupProjectionEntry {
                content_id,
                id,
                spawned_tick: nonnegative_integer(
                    body.get("spawnedTick"),
                    &format!("{field}.spawnedTick"),
                )?,
                x: finite(body.get("x"), &format!("{field}.x"))?,
                y: finite(body.get("y"), &format!("{field}.y"))?,
            });
        }

        let mut statuses = Vec::new();
        instance_ids.clear();
        for (index, value) in rows(source.get("statuses"), "statuses", 4096)?.iter().enumerate() {
            let field = format!("statuses[{index}]");
            let body = record(Some(value), &field)?;
            only_keys(
                body,
                &field,
                &["contentId", "expiresTick", "instanceId", "startedTick", "targetId"],
            )?;
            let content_id = text(body.get("contentId"), &format!("{field}.contentId"), 19)?;
            let instance_id =
                positive_integer(body.get("instanceId"), &format!("{field}.instanceId"))?;
            let started_tick =
                nonnegative_integer(body.get("startedTick"), &format!("{field}.startedTick"))?;
            let expires_tick =
                positive_integer(body.get("expiresTick"), &format!("{field}.expiresTick"))?;
            if !content_ids.contains(&content_id)
                || instance_ids.contains(&instance_id)
                || expires_tick <= started_tick
            {
                return Err(format!("{field} is invalid"));
            }
            instance_ids.push(instance_id);
            let target_id = text(body.get("targetId"), &format!("{field}.targetId"), 128)?;
            if [
                "constructor",
                "__defineGetter__",
                "__defineSetter__",
                "hasOwnProperty",
                "__lookupGetter__",
                "__lookupSetter__",
                "isPrototypeOf",
                "propertyIsEnumerable",
                "toString",
                "valueOf",
                "__proto__",
                "toLocaleString",
            ]
            .contains(&target_id.as_str())
            {
                return Err(format!("{field}.targetId is reserved"));
            }
            statuses.push(ModStatusProjectionEntry {
                content_id,
                expires_tick,
                instance_id,
                started_tick,
                target_id,
            });
        }
        let manifest_sha256 =
            text(source.get("manifestSha256"), "manifestSha256", 64)?.to_lowercase();
        if manifest_sha256.len() != 64
            || !manifest_sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err("manifestSha256 must be SHA-256 hex".into());
        }
        let revision = nonnegative_integer(source.get("revision"), "revision")?;
        Ok(Self { boasts, content, manifest_sha256, powerups, revision, statuses })
    }
}

fn read_content(
    source: &LuaObject,
    field: &str,
    content_ids: &mut BTreeSet<String>,
) -> Result<ModContentProjectionEntry, String> {
    only_keys(
        source,
        field,
        &["art", "contentId", "contentKind", "description", "key", "modId", "name", "presentation"],
    )?;
    let content_id = text(source.get("contentId"), &format!("{field}.contentId"), 19)?;
    let kind = text(source.get("contentKind"), &format!("{field}.contentKind"), 32)?;
    let mod_id = text(source.get("modId"), &format!("{field}.modId"), 128)?;
    let key = text(source.get("key"), &format!("{field}.key"), 128)?;
    let content_kind = WebLuaContentKind::parse(&kind);
    if !decimal_id(&content_id)
        || content_kind.is_none()
        || !identifier(&mod_id)
        || !identifier(&key)
        || !content_ids.insert(content_id.clone())
    {
        return Err(format!("{field} is invalid"));
    }
    let mut art = Vec::new();
    let mut slots = BTreeSet::new();
    for (index, value) in rows(source.get("art"), &format!("{field}.art"), 32)?.iter().enumerate() {
        let art_field = format!("{field}.art[{index}]");
        let body = record(Some(value), &art_field)?;
        only_keys(body, &art_field, &["path", "slot"])?;
        let slot = text(body.get("slot"), &format!("{art_field}.slot"), 64)?;
        let path = text(body.get("path"), &format!("{art_field}.path"), 240)?;
        if !slots.insert(slot.clone()) || !art_path(&path) {
            return Err(format!("{art_field} is invalid"));
        }
        art.push(ModContentArt { path, slot });
    }
    let description = bytes(source.get("description"), &format!("{field}.description"), 1024)?;
    let name = text(source.get("name"), &format!("{field}.name"), 128)?;
    let presentation = if source.get("presentation") == Some(&LuaValue::Null) {
        None
    } else {
        Some(text(source.get("presentation"), &format!("{field}.presentation"), 64)?)
    };
    Ok(ModContentProjectionEntry {
        art,
        content_id,
        content_kind: content_kind.expect("validated content kind"),
        description,
        key,
        mod_id,
        name,
        presentation,
    })
}

fn read_failure_producers(
    source: &LuaObject,
    field: &str,
) -> Result<Vec<BoastFailureProducer>, String> {
    let field = format!("{field}.failureProducers");
    let values = rows(source.get("failureProducers"), &field, 4)?;
    let mut names = Vec::new();
    for (index, value) in values.iter().enumerate() {
        names.push(text(Some(value), &format!("{field}[{index}]"), 32)?);
    }
    let mut producers = Vec::new();
    for name in names {
        let producer = match name.as_str() {
            "magical-equipment" => BoastFailureProducer::MagicalEquipment,
            "mana-underflow" => BoastFailureProducer::ManaUnderflow,
            "potion-use" => BoastFailureProducer::PotionUse,
            "secondary-cast" => BoastFailureProducer::SecondaryCast,
            _ => return Err(format!("{field} is invalid")),
        };
        if producers.contains(&producer) {
            return Err(format!("{field} is invalid"));
        }
        producers.push(producer);
    }
    Ok(producers)
}

fn read_icon(
    value: Option<&LuaValue>,
    field: &str,
    content: &ModContentProjectionEntry,
) -> Result<PreparedModBoastIcon, String> {
    let source = record(value, field)?;
    match text(source.get("kind"), &format!("{field}.kind"), 8)?.as_str() {
        "stock" => {
            only_keys(source, field, &["kind", "record", "style"])?;
            let style =
                integer_within(source.get("style"), &format!("{field}.style"), 0.0, 7.0)? as usize;
            let record =
                integer_within(source.get("record"), &format!("{field}.record"), 90.0, 97.0)?
                    as usize;
            if record != 90 + style || content.art.iter().any(|art| art.slot == "icon") {
                return Err(format!("{field} is inconsistent"));
            }
            Ok(PreparedModBoastIcon::Stock { record, style })
        }
        "mod" => {
            only_keys(source, field, &["frame", "imageHeight", "imagePath", "imageWidth", "kind"])?;
            let frame = read_frame(source.get("frame"), &format!("{field}.frame"))?;
            let image_width = integer_within(
                source.get("imageWidth"),
                &format!("{field}.imageWidth"),
                1.0,
                4096.0,
            )? as u32;
            let image_height = integer_within(
                source.get("imageHeight"),
                &format!("{field}.imageHeight"),
                1.0,
                4096.0,
            )? as u32;
            if frame.logical_width > 128.0
                || frame.logical_height > 128.0
                || frame.x + frame.width > f64::from(image_width)
                || frame.y + frame.height > f64::from(image_height)
                || !content.art.iter().any(|art| {
                    art.slot == "icon"
                        && Some(art.path.as_str())
                            == source.get("imagePath").and_then(LuaValue::as_str)
                })
            {
                return Err(format!("{field} is inconsistent"));
            }
            let image_path = text(source.get("imagePath"), &format!("{field}.imagePath"), 240)?;
            Ok(PreparedModBoastIcon::Mod { frame, image_height, image_path, image_width })
        }
        _ => Err(format!("{field}.kind is invalid")),
    }
}

fn read_frame(value: Option<&LuaValue>, field: &str) -> Result<ModSpriteFrame, String> {
    let source = record(value, field)?;
    only_keys(
        source,
        field,
        &[
            "centerOffsetX",
            "centerOffsetY",
            "contentHeight",
            "contentWidth",
            "height",
            "logicalHeight",
            "logicalWidth",
            "width",
            "x",
            "y",
        ],
    )?;
    let frame = ModSpriteFrame {
        center_offset_x: finite(source.get("centerOffsetX"), &format!("{field}.centerOffsetX"))?,
        center_offset_y: finite(source.get("centerOffsetY"), &format!("{field}.centerOffsetY"))?,
        content_height: positive(source.get("contentHeight"), &format!("{field}.contentHeight"))?,
        content_width: positive(source.get("contentWidth"), &format!("{field}.contentWidth"))?,
        height: positive(source.get("height"), &format!("{field}.height"))?,
        logical_height: positive_integer(
            source.get("logicalHeight"),
            &format!("{field}.logicalHeight"),
        )?,
        logical_width: positive_integer(
            source.get("logicalWidth"),
            &format!("{field}.logicalWidth"),
        )?,
        width: positive(source.get("width"), &format!("{field}.width"))?,
        x: finite(source.get("x"), &format!("{field}.x"))?,
        y: finite(source.get("y"), &format!("{field}.y"))?,
    };
    let components = [
        frame.center_offset_x,
        frame.center_offset_y,
        frame.content_height,
        frame.content_width,
        frame.height,
        frame.logical_height,
        frame.logical_width,
        frame.width,
        frame.x,
        frame.y,
    ];
    if components.iter().any(|component| component.abs() > 16384.0)
        || frame.x < 0.0
        || frame.y < 0.0
    {
        return Err(format!("{field} exceeds mod sprite geometry bounds"));
    }
    Ok(frame)
}

fn record<'a>(value: Option<&'a LuaValue>, field: &str) -> Result<&'a LuaObject, String> {
    value.and_then(LuaValue::as_object).ok_or_else(|| format!("{field} must be an object"))
}

fn only_keys(source: &LuaObject, field: &str, allowed: &[&str]) -> Result<(), String> {
    for (key, _) in source.iter() {
        if !allowed.contains(&key) {
            return Err(format!("{field}.{key} is not allowed"));
        }
    }
    Ok(())
}

fn rows<'a>(
    value: Option<&'a LuaValue>,
    field: &str,
    maximum: usize,
) -> Result<&'a [LuaValue], String> {
    let rows =
        value.and_then(LuaValue::as_array).ok_or_else(|| format!("{field} must be an array"))?;
    if rows.len() > maximum {
        return Err(format!("{field} may contain at most {maximum} entries"));
    }
    Ok(rows)
}

fn text(value: Option<&LuaValue>, field: &str, maximum: usize) -> Result<String, String> {
    let value = value
        .and_then(LuaValue::as_str)
        .filter(|value| !value.is_empty() && value.encode_utf16().count() <= maximum);
    value
        .map(str::to_owned)
        .ok_or_else(|| format!("{field} must be a nonempty string of at most {maximum} characters"))
}

fn bytes(value: Option<&LuaValue>, field: &str, maximum: usize) -> Result<String, String> {
    value
        .and_then(LuaValue::as_str)
        .filter(|value| value.len() <= maximum)
        .map(str::to_owned)
        .ok_or_else(|| format!("{field} must be a string of at most {maximum} bytes"))
}

fn finite(value: Option<&LuaValue>, field: &str) -> Result<f64, String> {
    value
        .and_then(LuaValue::as_number)
        .filter(|value| value.is_finite())
        .ok_or_else(|| format!("{field} must be finite"))
}

fn integer(value: Option<&LuaValue>, field: &str) -> Result<f64, String> {
    let value = finite(value, field)?;
    if value.fract() != 0.0 {
        return Err(format!("{field} must be an integer"));
    }
    Ok(value)
}

fn positive(value: Option<&LuaValue>, field: &str) -> Result<f64, String> {
    let value = finite(value, field)?;
    if value <= 0.0 {
        return Err(format!("{field} must be positive"));
    }
    Ok(value)
}

fn positive_integer(value: Option<&LuaValue>, field: &str) -> Result<f64, String> {
    let value = integer(value, field)?;
    if value < 1.0 {
        return Err(format!("{field} must be positive"));
    }
    Ok(value)
}

fn nonnegative_integer(value: Option<&LuaValue>, field: &str) -> Result<f64, String> {
    let value = integer(value, field)?;
    if value < 0.0 {
        return Err(format!("{field} must be nonnegative"));
    }
    Ok(value)
}

fn within(
    value: Option<&LuaValue>,
    field: &str,
    minimum: f64,
    maximum: f64,
) -> Result<f64, String> {
    let value = finite(value, field)?;
    if value < minimum || value > maximum {
        return Err(format!("{field} must be within [{minimum},{maximum}]"));
    }
    Ok(value)
}

fn integer_within(
    value: Option<&LuaValue>,
    field: &str,
    minimum: f64,
    maximum: f64,
) -> Result<f64, String> {
    let value = integer(value, field)?;
    if value < minimum || value > maximum {
        return Err(format!("{field} is outside [{minimum},{maximum}]"));
    }
    Ok(value)
}

fn decimal_id(value: &str) -> bool {
    !value.starts_with('0') && value.bytes().all(|byte| byte.is_ascii_digit())
}

fn identifier(value: &str) -> bool {
    let valid_end = |byte: u8| byte.is_ascii_lowercase() || byte.is_ascii_digit();
    valid_end(value.as_bytes()[0])
        && valid_end(*value.as_bytes().last().expect("nonempty identifier"))
        && value.bytes().all(|byte| valid_end(byte) || matches!(byte, b'.' | b'_' | b'-'))
}

fn art_path(value: &str) -> bool {
    let Some((folder, rest)) = value.split_once('/') else { return false };
    ["art", "audio", "levels", "scenes", "sprites"].contains(&folder)
        && rest
            .chars()
            .next()
            .is_some_and(|character| !matches!(character, '\n' | '\r' | '\u{2028}' | '\u{2029}'))
}
