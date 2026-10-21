//! Saved player books, belts and runtime reconstruction from `normalizePlayerStore`.

use serde_json::{Value, json};
use solomon_sim::belt::{
    NATIVE_BELT_SLOT_COUNT, PLAYER_BELT_SLOT_COUNT, PlayerBeltComponent, is_native_belt_skill,
    native_belt_owned_item, native_inventory_item_can_bind_to_belt,
};
use solomon_sim::combat::PlayerCombatState;
use solomon_sim::economy::HubEconomyState;
use solomon_sim::profile::{
    NativeSkillBookState, PlayerConcentrationSlot, PlayerProgressionState, PlayerSkillRuntimeState,
    PlayerStatBookComponent, native_weld_build, player_skill_derived_stats_with_stat_book,
    refresh_player_combat_from_skill_stats,
};
use solomon_sim::spells::casting::PlayerPrimaryCastState;

use super::super::contract::record;
use super::super::profile::{normalize_economy, normalize_hagatha_runtime};
use super::super::value::{decode, encode};
use super::{array, finite, integer_within, numeric_or_null, puppet_hit, reject_unexpected_keys};

pub(super) fn normalize(value: &Value, version: u64, tick: u64) -> Result<Value, String> {
    let mut source = record(value, "game save players")?.clone();
    reject_unexpected_keys(
        &source,
        "game save players",
        &[
            "configs",
            "economies",
            "entityIds",
            "identities",
            "lightings",
            "locomotions",
            "nextEntityId",
            "primaryCasts",
            "progressions",
            "skillBooks",
            "skillRuntimes",
            "statBooks",
        ],
        if version >= 18 { &["belts"] } else { &[] },
    )?;
    let count = array(&value["configs"], "game save player components are invalid")?.len();
    for field in ["economies", "primaryCasts", "skillBooks", "statBooks"] {
        if array(&value[field], "game save player components are invalid")?.len() != count {
            return Err("game save player component cardinality drifted".into());
        }
    }
    if version >= 18
        && array(&value["belts"], "game save player components are invalid")?.len() != count
    {
        return Err("game save player component cardinality drifted".into());
    }
    let persisted = value["skillRuntimes"].as_array();
    if persisted.is_some_and(|runtimes| runtimes.len() != count) {
        return Err("game save skill runtime cardinality drifted".into());
    }
    let mut economies = Vec::new();
    let mut removed_outcomes = Vec::new();
    for economy in array(&value["economies"], "game save player economies")? {
        let (economy, removed) = normalize_economy(economy, version)?;
        economies.push(economy);
        removed_outcomes.push(removed);
    }
    let mut books = Vec::new();
    let mut runtimes = Vec::new();
    let mut belts = Vec::new();
    let mut casts = Vec::new();
    let mut stat_books = Vec::new();
    for index in 0..count {
        let legacy = record(&value["skillBooks"][index], &format!("game save skill book {index}"))?;
        let mut book = skill_book(&value["skillBooks"][index], index)?;
        let saved_book = book.clone();
        let stat_book: PlayerStatBookComponent = decode(value["statBooks"][index].clone())?;
        let economy = &economies[index];
        let mut runtime =
            PlayerSkillRuntimeState::new_with_stat_book(&mut book, &stat_book, economy);
        if let Some(persisted) = persisted {
            let persisted = &persisted[index];
            record(persisted, &format!("game save skill runtime {index}"))?;
            let (armor, coating) = if version < 30 {
                (0.0, 0.0)
            } else {
                record(&persisted["harden"], "game save Harden state")?;
                (
                    finite(&persisted["harden"]["armor"], "game save Harden armor")?,
                    finite(&persisted["harden"]["coating"], "game save Harden coating")?,
                )
            };
            if armor < 0.0 || !(0.0..=1.0).contains(&coating) || (coating == 0.0 && armor != 0.0) {
                return Err("game save Harden state is out of range".into());
            }
            runtime.next_concentration_replacement_slot =
                match persisted["nextConcentrationReplacementSlot"].as_str() {
                    Some("a") => PlayerConcentrationSlot::A,
                    Some("b") => PlayerConcentrationSlot::B,
                    _ => return Err("game save concentration replacement slot is invalid".into()),
                };
            runtime.concentration_skill_id_a =
                decode(numeric_or_null(&persisted["concentrationSkillIdA"]))?;
            runtime.concentration_skill_id_b =
                decode(numeric_or_null(&persisted["concentrationSkillIdB"]))?;
            runtime.harden.armor = armor;
            runtime.harden.coating = coating;
            runtime.hurricane_charge =
                finite(&persisted["hurricaneCharge"], "game save Hurricane charge")?;
            runtime.hurricane_refreshed = persisted["hurricaneRefreshed"].as_bool() == Some(true);
            runtime.staff_melee_alternate =
                persisted["staffMeleeAlternate"].as_bool() == Some(true);
        } else {
            let concentrations = legacy.get("concentrationSkillIds").and_then(Value::as_array);
            runtime.concentration_skill_id_a = decode(numeric_or_null(
                concentrations.and_then(|values| values.first()).unwrap_or(&Value::Null),
            ))?;
            runtime.concentration_skill_id_b = decode(numeric_or_null(
                concentrations.and_then(|values| values.get(1)).unwrap_or(&Value::Null),
            ))?;
            runtime.next_concentration_replacement_slot =
                if legacy.get("nextConcentrationSlot").and_then(Value::as_f64) == Some(1.0) {
                    PlayerConcentrationSlot::B
                } else {
                    PlayerConcentrationSlot::A
                };
        }
        runtime.refresh_with_stat_book(&mut book, &stat_book, economy);
        let mut belt = if version >= 18 {
            saved_belt(&value["belts"][index], index, &book, economy, version)?
        } else {
            let quickbar = legacy
                .get("skillQuickbar")
                .filter(|value| value.is_array())
                .or_else(|| legacy.get("secondaryBelt"))
                .unwrap_or(&Value::Null);
            let quickbar = array(quickbar, "game save secondary belt")?;
            let entries: [Option<usize>; 8] = decode(Value::Array(quickbar.clone()))?;
            PlayerBeltComponent::from_skill_quickbar(entries)
                .ok_or("game save skill quickbar entry is invalid")?
        };
        belt.autofill_newly_learned(&saved_book, &book);
        let mut cast =
            record(&encode(&PlayerPrimaryCastState::default())?, "idle primary cast")?.clone();
        cast.extend(record(&value["primaryCasts"][index], "game save primary cast")?.clone());
        cast.insert(
            "oneShotAttackPoseHeld".into(),
            json!(value["primaryCasts"][index]["oneShotAttackPoseHeld"].as_bool() == Some(true)),
        );
        if !value["primaryCasts"][index]["selectedPrimaryId"].is_number() {
            let selected = if book.primary_skill_id == 52 {
                book.weld_build_id.map_or(-1, i64::from)
            } else {
                book.primary_skill_id as i64
            };
            cast.insert("selectedPrimaryId".into(), json!(selected));
        }
        belts.push(encode(&belt)?);
        casts.push(Value::Object(cast));
        books.push(book);
        runtimes.push(runtime);
        stat_books.push(stat_book);
    }
    let mut progressions = Vec::new();
    for (index, value) in
        array(&value["progressions"], "game save player progressions")?.iter().enumerate()
    {
        let mut progression =
            record(value, &format!("game save player progression {index}"))?.clone();
        if version >= 48
            && (value["corpseConsumed"].as_bool().is_none()
                || (value["corpseConsumed"] == true
                    && !matches!(value["lifeState"].as_str(), Some("dying" | "spectating"))))
        {
            return Err(format!(
                "game save player progression {index} corpseConsumed requires a dead player and a boolean"
            ));
        }
        progression.insert(
            "corpseConsumed".into(),
            if version < 48 { json!(false) } else { value["corpseConsumed"].clone() },
        );
        progression.insert(
            "rescueProtection".into(),
            match value.get("rescueProtection") {
                Some(protection) => super::visual_state::rescue_protection(
                    protection,
                    &format!("game save player {index} rescue protection"),
                )?,
                None => json!({"fraction":0,"nextParticleId":1,"particles":[]}),
            },
        );
        if version >= 31 && value["poisonBeforeCold"].as_bool().is_none() {
            return Err(format!(
                "game save player progression {index} poison/cold order is invalid"
            ));
        }
        let economy =
            economies.get(index).ok_or("game save player component cardinality drifted")?;
        let hagatha = normalize_hagatha_runtime(
            progression.get("hagathaRuntime"),
            &economy.owned_perk_selectors,
            version,
            index,
        )?;
        progression.insert("hagathaRuntime".into(), encode(&hagatha)?);
        progression.insert(
            "disciplineOfferBias".into(),
            json!(economy.owned_perk_selectors.contains(&14)),
        );
        progression.insert(
            "hitFeedback".into(),
            puppet_hit(
                &value["hitFeedback"],
                &value["lastDamageTick"],
                tick,
                version,
                &format!("game save player {index} hit feedback"),
            )?,
        );
        if version < 31 {
            progression.insert("poisonBeforeCold".into(), json!(false));
        }
        if version < 34 {
            progression.insert("circleSlowTicksRemaining".into(), json!(0));
        }
        if !removed_outcomes[index].is_empty() {
            let state: PlayerProgressionState = decode(Value::Object(progression.clone()))?;
            let mut combat: PlayerCombatState = decode(Value::Object(progression.clone()))?;
            let derived = player_skill_derived_stats_with_stat_book(
                &runtimes[index],
                &books[index],
                &stat_books[index],
                &state,
                economy,
            );
            refresh_player_combat_from_skill_stats(&mut combat, &derived);
            progression.extend(record(&encode(&combat)?, "rebuilt player combat")?.clone());
        }
        progressions.push(Value::Object(progression));
    }
    let mut locomotions = array(&value["locomotions"], "game save player locomotions")?.clone();
    for (index, locomotion) in locomotions.iter_mut().enumerate() {
        record(locomotion, &format!("game save player locomotion {index}"))?;
        let heading = integer_within(
            &locomotion["headingIndex"],
            &format!("game save player locomotion {index} headingIndex"),
            0,
            23,
        )?;
        let degrees = if version < 49 {
            heading as f64 * 15.0
        } else {
            finite(
                &locomotion["headingDegrees"],
                &format!("game save player locomotion {index} headingDegrees"),
            )?
        };
        let degrees = solomon_sim::float::fround(degrees);
        if solomon_sim::heading::heading_index(degrees) != heading as usize {
            return Err(format!(
                "game save player locomotion {index} heading invariant is invalid"
            ));
        }
        locomotion["headingDegrees"] = json!(degrees);
    }
    source.insert("locomotions".into(), Value::Array(locomotions));
    if version < 34 {
        let mut lightings = array(&value["lightings"], "game save player lightings")?.clone();
        for (index, lighting) in lightings.iter_mut().enumerate() {
            record(lighting, &format!("game save player lighting {index}"))?;
            lighting["blindnessTicksRemaining"] = json!(0);
        }
        source.insert("lightings".into(), Value::Array(lightings));
    }
    source.insert("belts".into(), Value::Array(belts));
    source.insert("primaryCasts".into(), Value::Array(casts));
    source.insert("skillBooks".into(), encode(&books)?);
    source.insert("skillRuntimes".into(), encode(&runtimes)?);
    source.insert("economies".into(), encode(&economies)?);
    source.insert("progressions".into(), Value::Array(progressions));
    Ok(Value::Object(source))
}

fn skill_book(value: &Value, index: usize) -> Result<NativeSkillBookState, String> {
    let permanent = array(&value["permanentRanks"], "game save permanent skill ranks")?;
    let effective = array(&value["effectiveRanks"], "game save effective skill ranks")?;
    let learned =
        value.get("learnedSkillOrder").filter(|value| value.is_array()).cloned().unwrap_or_else(
            || {
                json!(
                    permanent
                        .iter()
                        .enumerate()
                        .filter(|(id, value)| (8..=79).contains(id)
                            && value.as_f64().is_some_and(|rank| rank > 0.0))
                        .map(|(id, _)| id)
                        .collect::<Vec<_>>()
                )
            },
        );
    let weld_id = numeric_or_null(
        value
            .get("weldBuildId")
            .filter(|value| !value.is_null())
            .unwrap_or(&value["activeWeldBuildId"]),
    );
    let build = weld_id.as_u64().and_then(|id| u32::try_from(id).ok()).and_then(native_weld_build);
    let components = if let Some(components) = value.get("weldComponentRanks") {
        components.clone()
    } else {
        match build {
            Some(build) => json!(
                build
                    .component_skill_ids
                    .map(|id| effective.get(id).cloned().unwrap_or(Value::Null))
            ),
            None => Value::Null,
        }
    };
    if build.is_none() != components.is_null() {
        return Err(format!("game save player skill book {index} Weld cache is invalid"));
    }
    if !components.is_null()
        && components.as_array().is_none_or(|values| {
            values.len() != 6
                || values.iter().any(|value| value.as_u64().is_none_or(|rank| rank > 255))
        })
    {
        return Err(format!("game save player skill book {index} Weld ranks are invalid"));
    }
    let advanced = value.get("advancedUnlocks").cloned().unwrap_or(json!(vec![false; 8]));
    if advanced
        .as_array()
        .is_none_or(|values| values.len() != 8 || values.iter().any(|value| !value.is_boolean()))
    {
        return Err("game save advanced unlocks are invalid".into());
    }
    decode(json!({"advancedUnlocks":advanced,
        "disciplineRoot":integer_within(&value["disciplineRoot"], "game save discipline root", 0, 7)?,
        "effectiveRanks":effective,"elementRoot":integer_within(&value["elementRoot"], "game save element root", 0, 4)?,
        "learnedSkillOrder":learned,"permanentRanks":permanent,
        "primarySkillId":value["primarySkillId"],
        "weldBuildId":weld_id,"weldComponentRanks":components}))
}

fn saved_belt(
    value: &Value,
    index: usize,
    book: &NativeSkillBookState,
    economy: &HubEconomyState,
    version: u64,
) -> Result<PlayerBeltComponent, String> {
    let entries = array(value, &format!("game save player belt {index}"))?;
    if entries.len() != PLAYER_BELT_SLOT_COUNT
        && !(version < 50 && entries.len() == NATIVE_BELT_SLOT_COUNT)
    {
        return Err(format!("game save player belt {index} is invalid"));
    }
    for (slot, entry) in entries.iter().enumerate() {
        if entry.is_null() {
            continue;
        }
        let field = format!("game save player belt {index} slot {slot}");
        let source = record(entry, &field)?;
        match entry["kind"].as_str() {
            Some("skill") => {
                reject_unexpected_keys(source, &field, &["kind", "skillId"], &[])?;
                let id = entry["skillId"].as_u64().and_then(|id| usize::try_from(id).ok());
                if id.is_none_or(|id| {
                    !is_native_belt_skill(id)
                        || book.effective_ranks.get(id).is_none_or(|rank| *rank < 1)
                }) {
                    return Err(format!("{field} skill is invalid"));
                }
            }
            Some("health-potion" | "mana-potion") => {
                reject_unexpected_keys(source, &field, &["kind"], &[])?
            }
            Some("item") => {
                reject_unexpected_keys(source, &field, &["itemId", "kind", "nativeTypeId"], &[])?;
                let item =
                    entry["itemId"].as_u64().and_then(|id| native_belt_owned_item(economy, id));
                if item.is_none_or(|item| {
                    Some(u64::from(item.native_type_id)) != entry["nativeTypeId"].as_u64()
                        || !native_inventory_item_can_bind_to_belt(item)
                }) {
                    return Err(format!("{field} item is invalid"));
                }
            }
            _ => return Err(format!("{field} kind is invalid")),
        }
    }
    decode(value.clone())
}
