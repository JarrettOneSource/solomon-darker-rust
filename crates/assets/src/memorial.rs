//! The durable shared College Memorial boundary from
//! `host/game-memorial-persistence.ts` and `protocol/codecs/hub.ts`.
//! The runtime owns the ten slots; this codec validates external documents before
//! replacing them and preserves Source Number metadata without narrowing it.

use std::collections::BTreeSet;

use solomon_sim::appearance::equipment::{
    PlayerEquipmentTintedSelector, PlayerLivingEquipmentAppearance, PlayerWeaponAppearance,
};
use solomon_sim::hub::memorial::{
    HUB_MEMORIAL_FIRST_EXTERNAL_PORTRAIT_ID, HUB_MEMORIAL_INITIAL_MARKERS,
    HUB_MEMORIAL_INITIAL_SLOT_AGES, HUB_MEMORIAL_LAST_EXTERNAL_PORTRAIT_ID,
    HUB_MEMORIAL_SLOT_COUNT, HubMemorialPortrait, HubMemorialSlot, HubMemorialState,
};
use solomon_sim::modding::{LuaObject, LuaValue};
use solomon_sim::profile::{NewGameSelection, WizardDiscipline, WizardElement};
use solomon_sim::spells::casting::NativePlayerWeaponKind;

pub const MAX_GAME_MEMORIAL_BYTES: usize = 256 * 1024;
pub const MEMORIAL_SIZE_ERROR: &str = "Game memorial state exceeds its bounded document size";

/// The outer envelope deliberately tolerates extra fields, as Source does.
pub fn decode_memorial_document(document: &str) -> Result<HubMemorialState, String> {
    if document.len() > MAX_GAME_MEMORIAL_BYTES {
        return Err(MEMORIAL_SIZE_ERROR.into());
    }
    let value: LuaValue = serde_json::from_str(document).map_err(|error| error.to_string())?;
    let source = value.as_object().ok_or("Game memorial document must be an object")?;
    decode_memorial_state(source.get("state"), "gameMemorial.state")
}

/// Returns the exact Source document before its file's final newline.
pub fn encode_memorial_document(state: &HubMemorialState) -> Result<String, String> {
    let source = crate::website_save::value::encode(state)?;
    let state: LuaValue = serde_json::from_value(source).map_err(|error| error.to_string())?;
    let document = crate::json_value::stringify(&LuaValue::Object(
        [("state".into(), state)].into_iter().collect(),
    ))?;
    if document.len() > MAX_GAME_MEMORIAL_BYTES {
        return Err(MEMORIAL_SIZE_ERROR.into());
    }
    Ok(document)
}

fn decode_memorial_state(
    value: Option<&LuaValue>,
    field: &str,
) -> Result<HubMemorialState, String> {
    let source = record(value, field, &["nextAge", "nextPortraitId", "slots"])?;
    let next_age = integer(source.get("nextAge"), &format!("{field}.nextAge"))?;
    if next_age < 1.0 {
        return Err(format!("{field}.nextAge must be positive"));
    }
    let next_portrait_id =
        within(source.get("nextPortraitId"), &format!("{field}.nextPortraitId"), 100, 109)? as u32;
    let slots_field = format!("{field}.slots");
    let raw_slots = source
        .get("slots")
        .and_then(LuaValue::as_array)
        .ok_or_else(|| format!("{slots_field} must be an array"))?;
    if raw_slots.len() != HUB_MEMORIAL_SLOT_COUNT {
        return Err(format!("{slots_field} must contain ten Painting slots"));
    }
    let mut ages = Vec::new();
    let mut dynamic_ages = Vec::new();
    let mut identities = BTreeSet::new();
    let mut portrait_ids = BTreeSet::new();
    let mut slots = Vec::new();
    for (index, value) in raw_slots.iter().enumerate() {
        let slot_field = format!("{slots_field}[{index}]");
        let slot = record(Some(value), &slot_field, &["age", "marker", "portrait", "portraitId"])?;
        let age = nonnegative(slot.get("age"), &format!("{slot_field}.age"))?;
        if ages.contains(&age) {
            return Err(format!("{slots_field} duplicates age {}", js_number(age)));
        }
        ages.push(age);
        let marker_field = format!("{slot_field}.marker");
        let marker = match slot.get("marker") {
            Some(LuaValue::Boolean(value)) => *value,
            _ => return Err(format!("{marker_field} must be boolean")),
        };
        let portrait_id = nonnegative(slot.get("portraitId"), &format!("{slot_field}.portraitId"))?;
        let portrait = if matches!(slot.get("portrait"), Some(LuaValue::Null)) {
            if portrait_id != index as f64
                || age != HUB_MEMORIAL_INITIAL_SLOT_AGES[index] as f64
                || marker != HUB_MEMORIAL_INITIAL_MARKERS[index]
            {
                return Err(format!("{slot_field} does not match its stock resident"));
            }
            None
        } else {
            if portrait_id < f64::from(HUB_MEMORIAL_FIRST_EXTERNAL_PORTRAIT_ID)
                || portrait_id > f64::from(HUB_MEMORIAL_LAST_EXTERNAL_PORTRAIT_ID)
                || !portrait_ids.insert(portrait_id as u32)
            {
                return Err(format!("{slot_field}.portraitId is not a unique external id"));
            }
            let portrait = decode_portrait(
                slot.get("portrait"),
                &format!("{slot_field}.portrait"),
                &slots_field,
                &mut identities,
            )?;
            dynamic_ages.push(age);
            Some(portrait)
        };
        slots.push(HubMemorialSlot {
            age: age as u64,
            marker,
            portrait,
            portrait_id: portrait_id as u32,
        });
    }
    let completion_count = next_age - 1001.0;
    if completion_count < 0.0 {
        return Err(format!("{field}.nextAge precedes stock defaults"));
    }
    if dynamic_ages.len() as f64 != completion_count.min(HUB_MEMORIAL_SLOT_COUNT as f64) {
        return Err(format!("{slots_field} do not match the portrait age counter"));
    }
    dynamic_ages.sort_by(f64::total_cmp);
    if dynamic_ages
        .iter()
        .enumerate()
        .any(|(index, age)| *age != next_age - dynamic_ages.len() as f64 + index as f64)
    {
        return Err(format!("{slots_field} do not retain the newest FIFO ages"));
    }
    if f64::from(next_portrait_id) != 100.0 + completion_count % HUB_MEMORIAL_SLOT_COUNT as f64 {
        return Err(format!("{field}.nextPortraitId does not match the ten-id ring"));
    }
    // A valid ten-distinct-age FIFO is below the integer-precision boundary, so
    // these state counters are representable. Captured tick metadata is independent.
    Ok(HubMemorialState {
        next_age: next_age as u64,
        next_portrait_id,
        slots: slots.try_into().expect("ten Memorial slots checked above"),
    })
}

fn decode_portrait(
    value: Option<&LuaValue>,
    field: &str,
    slots_field: &str,
    identities: &mut BTreeSet<String>,
) -> Result<HubMemorialPortrait, String> {
    let source = record(
        value,
        field,
        &[
            "accountUsername",
            "awesomeness",
            "awesomestKill",
            "capturedAtTick",
            "config",
            "elapsedTicks",
            "equipment",
            "headingIndex",
            "level",
            "monstersKilled",
            "playerId",
            "portraitScale",
            "runId",
            "wave",
        ],
    )?;
    let player_id = string(source.get("playerId"), &format!("{field}.playerId"), 128)?;
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
    .contains(&player_id.as_str())
    {
        return Err(format!("{field}.playerId is reserved"));
    }
    let run_id = string(source.get("runId"), &format!("{field}.runId"), 128)?;
    if !identities.insert(format!("{run_id}\0{player_id}")) {
        return Err(format!("{slots_field} duplicates a completed run participant"));
    }
    let portrait_scale = finite(source.get("portraitScale"), &format!("{field}.portraitScale"))?;
    if portrait_scale <= 0.0 {
        return Err(format!("{field}.portraitScale must be positive"));
    }
    if !(0.85_f32 as f64..=1.0).contains(&portrait_scale) {
        return Err(format!("{field}.portraitScale is outside its native range"));
    }
    Ok(HubMemorialPortrait {
        account_username: nullable_string(
            source.get("accountUsername"),
            &format!("{field}.accountUsername"),
        )?,
        awesomeness: within(
            source.get("awesomeness"),
            &format!("{field}.awesomeness"),
            0,
            2_000_000_000,
        )?,
        awesomest_kill: nullable_string(
            source.get("awesomestKill"),
            &format!("{field}.awesomestKill"),
        )?,
        captured_at_tick: nonnegative(
            source.get("capturedAtTick"),
            &format!("{field}.capturedAtTick"),
        )?,
        config: config(source.get("config"), &format!("{field}.config"))?,
        elapsed_ticks: within(
            source.get("elapsedTicks"),
            &format!("{field}.elapsedTicks"),
            0,
            60_480_000,
        )?,
        equipment: equipment(source.get("equipment"), &format!("{field}.equipment"))?,
        heading_index: within(source.get("headingIndex"), &format!("{field}.headingIndex"), 0, 23)?
            as usize,
        level: within(source.get("level"), &format!("{field}.level"), 1, 10_000)? as u32,
        monsters_killed: within(
            source.get("monstersKilled"),
            &format!("{field}.monstersKilled"),
            0,
            2_000_000_000,
        )?,
        player_id,
        portrait_scale,
        run_id,
        wave: within(source.get("wave"), &format!("{field}.wave"), 0, 1_000_000)?,
    })
}

fn config(value: Option<&LuaValue>, field: &str) -> Result<NewGameSelection, String> {
    let source = record(value, field, &["discipline", "displayName", "element"])?;
    let discipline = string(source.get("discipline"), &format!("{field}.discipline"), 32)?;
    let discipline = WizardDiscipline::ALL
        .into_iter()
        .find(|value| value.as_str() == discipline)
        .ok_or_else(|| format!("{field}.discipline is not supported"))?;
    let element = string(source.get("element"), &format!("{field}.element"), 32)?;
    let element = WizardElement::ALL
        .into_iter()
        .find(|value| value.as_str() == element)
        .ok_or_else(|| format!("{field}.element is not supported"))?;
    Ok(NewGameSelection {
        name: string(source.get("displayName"), &format!("{field}.displayName"), 64)?,
        discipline,
        element,
    })
}

fn equipment(
    value: Option<&LuaValue>,
    field: &str,
) -> Result<PlayerLivingEquipmentAppearance, String> {
    let source = record(value, field, &["hat", "robe", "weapon"])?;
    Ok(PlayerLivingEquipmentAppearance {
        hat: tinted_selector(source.get("hat"), &format!("{field}.hat"), 3)?,
        robe: tinted_selector(source.get("robe"), &format!("{field}.robe"), 2)?,
        weapon: match source.get("weapon") {
            Some(LuaValue::Null) => None,
            value => {
                let field = format!("{field}.weapon");
                let source = record(value, &field, &["kind", "selector"])?;
                let kind = string(source.get("kind"), &format!("{field}.kind"), 8)?;
                let kind = match kind.as_str() {
                    "staff" => NativePlayerWeaponKind::Staff,
                    "wand" => NativePlayerWeaponKind::Wand,
                    _ => return Err(format!("{field}.kind is not supported")),
                };
                Some(PlayerWeaponAppearance {
                    kind,
                    selector: within(source.get("selector"), &format!("{field}.selector"), 0, 5)?
                        as usize,
                })
            }
        },
    })
}

fn tinted_selector(
    value: Option<&LuaValue>,
    field: &str,
    maximum: u64,
) -> Result<Option<PlayerEquipmentTintedSelector>, String> {
    if matches!(value, Some(LuaValue::Null)) {
        return Ok(None);
    }
    let source = record(value, field, &["primaryTint", "secondaryTint", "selector"])?;
    Ok(Some(PlayerEquipmentTintedSelector {
        primary_tint: within(
            source.get("primaryTint"),
            &format!("{field}.primaryTint"),
            0,
            0xffffff,
        )? as u32,
        secondary_tint: within(
            source.get("secondaryTint"),
            &format!("{field}.secondaryTint"),
            0,
            0xffffff,
        )? as u32,
        selector: within(source.get("selector"), &format!("{field}.selector"), 0, maximum)?
            as usize,
    }))
}

fn record<'a>(
    value: Option<&'a LuaValue>,
    field: &str,
    keys: &[&str],
) -> Result<&'a LuaObject, String> {
    let source =
        value.and_then(LuaValue::as_object).ok_or_else(|| format!("{field} must be an object"))?;
    if let Some((key, _)) = source.iter().find(|(key, _)| !keys.contains(key)) {
        return Err(format!("{field}.{key} is not allowed"));
    }
    Ok(source)
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

fn nonnegative(value: Option<&LuaValue>, field: &str) -> Result<f64, String> {
    let value = integer(value, field)?;
    if value < 0.0 {
        return Err(format!("{field} must be nonnegative"));
    }
    Ok(value)
}

fn within(
    value: Option<&LuaValue>,
    field: &str,
    minimum: u64,
    maximum: u64,
) -> Result<u64, String> {
    let value = integer(value, field)?;
    if value < minimum as f64 || value > maximum as f64 {
        return Err(format!("{field} must be within [{minimum},{maximum}]"));
    }
    Ok(value as u64)
}

fn string(value: Option<&LuaValue>, field: &str, maximum: usize) -> Result<String, String> {
    value
        .and_then(LuaValue::as_str)
        .filter(|value| !value.is_empty() && value.encode_utf16().count() <= maximum)
        .map(str::to_owned)
        .ok_or_else(|| format!("{field} must be a nonempty string of at most {maximum} characters"))
}

fn nullable_string(value: Option<&LuaValue>, field: &str) -> Result<Option<String>, String> {
    if matches!(value, Some(LuaValue::Null)) {
        return Ok(None);
    }
    string(value, field, 64).map(Some)
}

fn js_number(value: f64) -> String {
    ryu_js::Buffer::new().format(value).to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durable_memorial_documents_follow_the_actual_source_reader() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../sim/tests/fixtures/website/memorial-persistence-goldens.json"
        ))
        .unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let result = decode_memorial_document(&case["document"].to_string());
            if !case["accepted"].as_bool().unwrap() {
                assert_eq!(
                    result.unwrap_err(),
                    case["error"].as_str().unwrap(),
                    "{}",
                    case["name"]
                );
                continue;
            }
            let state = result.unwrap_or_else(|error| panic!("{}: {error}", case["name"]));
            let document = encode_memorial_document(&state).unwrap();
            assert_eq!(document, case["encoded"].as_str().unwrap(), "{}", case["name"]);
            let encoded: LuaValue = serde_json::from_str(&document).unwrap();
            let expected: LuaValue =
                serde_json::from_value(serde_json::json!({ "state":case["state"] })).unwrap();
            assert_eq!(encoded, expected, "{}", case["name"]);
            assert_eq!(decode_memorial_document(&document).unwrap(), state, "{}", case["name"]);
        }
    }
}
