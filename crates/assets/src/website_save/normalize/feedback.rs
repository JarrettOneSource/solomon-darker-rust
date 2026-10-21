//! Saved action receipts use the current `protocol/codecs/economy.ts` contract.

use serde_json::Value;

use super::protocol_values::{boolean, finite, integer, object, string};

pub(in crate::website_save) fn validate(value: &Value, field: &str) -> Result<(), String> {
    let source = object(
        value,
        field,
        &[
            "accepted",
            "action",
            "dowsingPitch",
            "reason",
            "sequence",
            "transferDirection",
            "transferGesture",
            "unforgeOutcome",
            "skillBookOutcome",
            "dyeSessionId",
        ],
    )?;
    let action = string(&value["action"], &format!("{field}.action"), 32)?;
    if ![
        "activate-belt-slot",
        "bind-belt-item",
        "buy-dowsing",
        "buy-fomentius",
        "buy-hagatha",
        "buy-teacher-spell",
        "close-dowsing",
        "close-dye",
        "close-hagatha",
        "consume",
        "dye",
        "dowse",
        "equip",
        "interact-goodie",
        "move-inventory-item",
        "open-dye",
        "read-librarian-book",
        "read-skill-book",
        "rename-sack",
        "remove-hagatha",
        "select-boast",
        "transfer",
        "unforge",
        "unequip",
    ]
    .contains(&action)
    {
        return Err(format!("{field}.action is not supported"));
    }
    let painting = matches!(action, "open-dye" | "dye" | "close-dye");
    if painting {
        string(&value["dyeSessionId"], &format!("{field}.dyeSessionId"), 64)?;
    } else if source.contains_key("dyeSessionId") {
        return Err(format!("{field}.dyeSessionId requires a painting action"));
    }
    let reason = nullable_string(source.get("reason"), &format!("{field}.reason"))?;
    if let Some(reason) = reason
        && ![
            "capacity-full",
            "ineligible-item",
            "insufficient-gold",
            "invalid-inventory",
            "invalid-name",
            "invalid-offer",
            "invalid-slot",
            "invalid-target",
            "item-not-found",
            "offers-active",
            "perk-capacity-full",
            "required-clothing",
            "slot-empty",
            "slot-locked",
        ]
        .contains(&reason)
    {
        return Err(format!("{field}.reason is not supported"));
    }
    let direction =
        nullable_string(source.get("transferDirection"), &format!("{field}.transferDirection"))?;
    let gesture =
        nullable_string(source.get("transferGesture"), &format!("{field}.transferGesture"))?;
    if direction.is_some_and(|value| !matches!(value, "to-backpack" | "to-storage")) {
        return Err(format!("{field}.transferDirection is not supported"));
    }
    if gesture.is_some_and(|value| !matches!(value, "double-activation" | "drag")) {
        return Err(format!("{field}.transferGesture is not supported"));
    }
    if (action == "transfer") != (direction.is_some() && gesture.is_some()) {
        return Err(format!("{field} transfer metadata does not match action"));
    }
    boolean(&value["accepted"], &format!("{field}.accepted"))?;
    let accepted = value["accepted"] == true;
    if accepted != reason.is_none() {
        return Err(format!("{field}.accepted does not match reason"));
    }
    if direction == Some("to-storage") && gesture != Some("drag") {
        return Err(format!("{field}.transferGesture must be drag for to-storage"));
    }
    let pitch = match source.get("dowsingPitch") {
        Some(Value::Null) => None,
        _ => Some(finite(&value["dowsingPitch"], &format!("{field}.dowsingPitch"))?),
    };
    if (accepted && matches!(action, "dowse" | "buy-dowsing")) != pitch.is_some()
        || pitch.is_some_and(|pitch| !(0.8..=1.1_f32 as f64).contains(&pitch))
    {
        return Err(format!("{field}.dowsingPitch does not match action"));
    }
    let unforge = source.get("unforgeOutcome") != Some(&Value::Null);
    if unforge {
        unforge_outcome(&value["unforgeOutcome"], &format!("{field}.unforgeOutcome"))?;
    }
    if (accepted && action == "unforge") != unforge {
        return Err(format!("{field}.unforgeOutcome does not match action"));
    }
    let book = source.get("skillBookOutcome") != Some(&Value::Null);
    if book {
        skill_book_outcome(&value["skillBookOutcome"], &format!("{field}.skillBookOutcome"))?;
    }
    if (accepted && action == "read-skill-book") != book {
        return Err(format!("{field}.skillBookOutcome does not match action"));
    }
    integer(&value["sequence"], &format!("{field}.sequence"), true)?;
    Ok(())
}

fn nullable_string<'a>(value: Option<&'a Value>, field: &str) -> Result<Option<&'a str>, String> {
    if value == Some(&Value::Null) {
        Ok(None)
    } else {
        string(value.unwrap_or(&Value::Null), field, 32).map(Some)
    }
}

fn skill_book_outcome(value: &Value, field: &str) -> Result<(), String> {
    if value["kind"] == "choice" {
        object(value, field, &["kind"])?;
        return Ok(());
    }
    let source = object(value, field, &["kind", "skillId"])?;
    if value["kind"] != "rank" {
        return Err(format!("{field}.kind is not supported"));
    }
    if source.get("skillId") != Some(&Value::Null) {
        let id = integer(&value["skillId"], &format!("{field}.skillId"), true)?;
        if !(8..=81).contains(&id) {
            return Err(format!("{field}.skillId is outside the native rank-award rows"));
        }
    }
    Ok(())
}

fn unforge_outcome(value: &Value, field: &str) -> Result<(), String> {
    let source = object(value, field, &["amount", "itemName", "kind"])?;
    let kind = string(&value["kind"], &format!("{field}.kind"), 32)?;
    if ![
        "experience",
        "fizzle",
        "full-rejuvenation",
        "gold",
        "mana-cost",
        "maximum-health",
        "maximum-mana",
        "mind-dredge",
        "offensive-damage",
    ]
    .contains(&kind)
    {
        return Err(format!("{field}.kind is not supported"));
    }
    let amount = match source.get("amount") {
        Some(Value::Null) => None,
        _ => Some(integer(&value["amount"], &format!("{field}.amount"), true)?),
    };
    if matches!(kind, "fizzle" | "full-rejuvenation") != amount.is_none() {
        return Err(format!("{field}.amount does not match kind"));
    }
    if kind == "mind-dredge" && amount != Some(1) {
        return Err(format!("{field}.amount does not match Mind Dredge"));
    }
    string(&value["itemName"], &format!("{field}.itemName"), 128)?;
    Ok(())
}
