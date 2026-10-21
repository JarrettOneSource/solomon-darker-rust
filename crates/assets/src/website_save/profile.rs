//! Durable profile normalization from `save/game-save-document.ts`.

use serde_json::{Value, json};
use solomon_sim::economy::{
    CompletedRunEconomyArchive, HubEconomyState, NativeHubNpcState, NativeUnforgeBonuses,
    native_hagatha_bundle_state_is_valid, native_hagatha_outcome_state_is_valid,
};
use solomon_sim::profile::{NativeHagathaRuntimeState, WizardElement};
use solomon_sim::session::{GameSession, ParticipantProfile};

use super::contract::{ParsedContinuation, SaveMetadata, integer, only_keys, record};
use super::value::{decode, encode};

#[derive(Clone, Debug)]
pub struct RestoredGameProfile {
    pub advanced_unlocks: [bool; 8],
    pub continuation: Option<ParsedContinuation>,
    pub economy: HubEconomyState,
    pub hagatha_runtime: NativeHagathaRuntimeState,
    pub metadata: SaveMetadata,
}

pub fn restore_profile(document: &str) -> Result<RestoredGameProfile, String> {
    let parsed = super::parse(document)?;
    super::contract::validate_json_tree(
        &serde_json::from_str(document).map_err(|error| error.to_string())?,
    )?;
    let (mut economy, _) =
        normalize_economy(&parsed.profile_economy, parsed.source_schema_version)?;
    if parsed.source_schema_version < 6 {
        let character = parsed.continuation.as_ref().map(|value| &value.summary.character);
        economy.archive_completed_run(CompletedRunEconomyArchive {
            display_name: character.map_or("Wizard", |value| &value.name),
            ground_gold: 0,
            ground_items: &[],
            starter_element: character.map_or(WizardElement::Ether, |value| value.element),
            transfer_carried_items: false,
        });
    }
    let hagatha_runtime = normalize_hagatha_runtime(
        parsed.profile_hagatha_runtime.as_ref(),
        &economy.owned_perk_selectors,
        parsed.source_schema_version,
        0,
    )?;
    Ok(RestoredGameProfile {
        advanced_unlocks: parsed.profile_advanced_unlocks.unwrap_or([false; 8]),
        continuation: parsed.continuation,
        economy,
        hagatha_runtime,
        metadata: parsed.metadata,
    })
}

pub fn hydrate_profile(
    state: &mut GameSession,
    player_id: &str,
    profile: &RestoredGameProfile,
) -> Result<(), String> {
    state.hydrate_participant_profile(
        player_id,
        ParticipantProfile {
            economy: profile.economy.clone(),
            hagatha_runtime: profile.hagatha_runtime,
            advanced_unlocks: profile.advanced_unlocks,
        },
    )
}

pub(super) fn normalize_economy(
    value: &Value,
    version: u64,
) -> Result<(HubEconomyState, Vec<u32>), String> {
    let mut source = record(value, "game save player economy")?.clone();
    const KEYS: &[&str] = &[
        "actionFeedback",
        "backpack",
        "charmCapacity",
        "collegeIntroPending",
        "dowsingFee",
        "dowsingOffers",
        "dowsingRolled",
        "equipment",
        "firstMixedSelectors",
        "fomentiusStock",
        "gold",
        "hagathaBundleSelectors",
        "nextItemId",
        "nextOfferId",
        "npc",
        "ownedPerkSelectors",
        "revision",
        "rng",
        "storage",
        "tonicPurchases",
        "tutorialPending",
        "unforgeBonuses",
    ];
    for key in source.keys() {
        if !KEYS.contains(&key.as_str()) {
            return Err(format!("game save player economy has unexpected field {key}"));
        }
    }
    let has_dowsing_offers = source
        .get("dowsingOffers")
        .and_then(Value::as_array)
        .is_some_and(|offers| !offers.is_empty());
    let dowsing_rolled = if version < 43 {
        has_dowsing_offers
    } else {
        source
            .get("dowsingRolled")
            .and_then(Value::as_bool)
            .ok_or("game save player economy Dowsing phase is invalid")?
    };
    if !dowsing_rolled && has_dowsing_offers {
        return Err("game save player economy Dowsing phase is invalid".into());
    }
    source.insert("dowsingRolled".into(), json!(dowsing_rolled));
    if version >= 13 && source.get("collegeIntroPending").and_then(Value::as_bool).is_none() {
        return Err("game save player economy College intro state is invalid".into());
    }
    if let Some(feedback) = source.get_mut("actionFeedback").and_then(Value::as_object_mut) {
        feedback.entry("unforgeOutcome").or_insert(Value::Null);
        if feedback.get("action").and_then(Value::as_str) == Some("dye") {
            feedback.entry("dyeSessionId").or_insert(json!("historical-dye-result"));
        }
        if version < 42 {
            feedback.insert("skillBookOutcome".into(), Value::Null);
        }
    }
    if version < 42
        && source
            .get("actionFeedback")
            .and_then(|feedback| feedback.get("action"))
            .and_then(Value::as_str)
            == Some("read-skill-book")
    {
        source.insert("actionFeedback".into(), Value::Null);
    }
    let feedback = source.get("actionFeedback");
    let painting = feedback
        .and_then(|value| value.get("action"))
        .and_then(Value::as_str)
        .is_some_and(|action| matches!(action, "dye" | "open-dye" | "close-dye"));
    if (version >= 42 && feedback != Some(&Value::Null)) || painting {
        super::normalize::feedback::validate(
            feedback.unwrap_or(&Value::Null),
            "game save player economy.actionFeedback",
        )?;
    }
    if painting {
        source.insert("actionFeedback".into(), Value::Null);
    }
    let tonics = source.get("tonicPurchases").and_then(Value::as_u64);
    let mut outcomes =
        source.get("ownedPerkSelectors").and_then(Value::as_array).cloned().unwrap_or_default();
    let tonic_entries = outcomes.iter().filter(|value| value.as_u64() == Some(27)).count() as u64;
    if version < 17
        && let Some(tonics) = tonics.filter(|count| *count > tonic_entries)
    {
        // The source rejects a larger tonic count through its canonical outcome validator.
        if tonics > 2 {
            return Err(
                "game save player economy inventory is invalid or Hagatha state is invalid".into(),
            );
        }
        outcomes.extend(std::iter::repeat_n(json!(27), (tonics - tonic_entries) as usize));
    }
    let removed = if version < 24 {
        repair_legacy_hagatha_outcomes(
            &mut outcomes,
            tonics,
            source.get("charmCapacity").and_then(Value::as_u64),
        )
    } else {
        Vec::new()
    };
    source.insert("ownedPerkSelectors".into(), Value::Array(outcomes));
    if version < 24
        && let Some(bundle) = source.get_mut("hagathaBundleSelectors").and_then(Value::as_array_mut)
    {
        let count = bundle.iter().filter(|value| value.as_u64() == Some(27)).count() as u64;
        repair_legacy_hagatha_outcomes(bundle, Some(count), Some(3 + count * 3));
    }
    let intro =
        version >= 13 && source.get("collegeIntroPending").and_then(Value::as_bool) == Some(true);
    let tutorial = source.get("tutorialPending").and_then(Value::as_bool) == Some(true);
    source.insert("collegeIntroPending".into(), json!(intro));
    source.insert("tutorialPending".into(), json!(tutorial));
    source.insert("npc".into(), normalize_npc(source.get("npc"), version >= 11)?);
    if source.get("unforgeBonuses").is_none_or(Value::is_null) {
        source.insert("unforgeBonuses".into(), encode(&NativeUnforgeBonuses::default())?);
    }
    let mut economy: HubEconomyState = decode(Value::Object(source))
        .map_err(|_| "game save player economy inventory is invalid or Hagatha state is invalid")?;
    super::create::normalize_inventory(&mut economy);
    if !economy.inventory_is_valid()
        || !native_hagatha_bundle_state_is_valid(&economy.hagatha_bundle_selectors)
        || !native_hagatha_outcome_state_is_valid(
            &economy.owned_perk_selectors,
            economy.tonic_purchases,
            economy.charm_capacity,
        )
    {
        return Err(
            "game save player economy inventory is invalid or Hagatha state is invalid".into()
        );
    }
    Ok((economy, removed))
}

fn repair_legacy_hagatha_outcomes(
    outcomes: &mut Vec<Value>,
    tonics: Option<u64>,
    capacity: Option<u64>,
) -> Vec<u32> {
    let (Some(tonics), Some(capacity)) = (tonics, capacity) else { return Vec::new() };
    if outcomes.len() as u64 <= capacity
        || tonics > 2
        || capacity != 3 + tonics * 3
        || outcomes.iter().any(|value| value.as_u64().is_none())
        || outcomes.iter().filter(|value| value.as_u64() == Some(27)).count() as u64 != tonics
    {
        return Vec::new();
    }
    let mut retained = Vec::with_capacity(capacity as usize);
    let mut removed = Vec::new();
    let mut remaining_tonics = tonics;
    for value in outcomes.drain(..) {
        if value.as_u64() == Some(27) {
            remaining_tonics -= 1;
            retained.push(value);
        } else if (retained.len() as u64) < capacity - remaining_tonics {
            retained.push(value);
        } else if let Some(selector) = value.as_u64().and_then(|value| value.try_into().ok()) {
            removed.push(selector);
        }
    }
    *outcomes = retained;
    removed
}

fn normalize_npc(value: Option<&Value>, required: bool) -> Result<Value, String> {
    let Some(value) = value else {
        if required {
            return Err("game save Hub NPC state is missing".into());
        }
        return encode(&NativeHubNpcState {
            help_flags: [false; 10],
            ..NativeHubNpcState::default()
        });
    };
    let mut source = record(value, "game save Hub NPC state")?.clone();
    if required && !source.contains_key("helpFlags") {
        return Err("game save Hub NPC help flags are missing".into());
    }
    only_keys(
        &source,
        "game save Hub NPC state",
        if source.contains_key("helpFlags") {
            &["boast", "helpFlags", "librarianLaceRead"]
        } else {
            &["boast", "librarianLaceRead"]
        },
    )?;
    let boast = record(&source["boast"], "game save Boast state")?;
    only_keys(
        boast,
        "game save Boast state",
        &["failed", "failureSequence", "selected", "succeeded"],
    )?;
    if !boast["selected"].is_null() {
        if boast["selected"].is_number() {
            if !boast["selected"].as_u64().is_some_and(|id| id <= 4) {
                return Err("game save Boast selection is invalid".into());
            }
        } else {
            let selected = record(&boast["selected"], "game save mod Boast selection")?;
            only_keys(selected, "game save mod Boast selection", &["contentId", "kind", "modId"])?;
            let valid = selected.get("kind").and_then(Value::as_str) == Some("mod")
                && selected
                    .get("contentId")
                    .and_then(Value::as_str)
                    .zip(selected.get("modId").and_then(Value::as_str))
                    .is_some_and(|(content, package)| {
                        solomon_sim::modding::content::ModBoastSelection::new(
                            content.into(),
                            package.into(),
                        )
                        .is_ok()
                    });
            if !valid {
                return Err("game save mod Boast selection is invalid".into());
            }
        }
    }
    let failed = boast["failed"].as_bool();
    let succeeded = boast["succeeded"].as_bool();
    let sequence = boast["failureSequence"].as_u64();
    if failed.is_none()
        || succeeded.is_none()
        || sequence.is_none_or(|value| value > 1)
        || failed != Some(sequence == Some(1))
        || (failed == Some(true) && succeeded == Some(true))
        || (boast["selected"].as_u64() == Some(3) && failed == Some(true))
        || (boast["selected"].is_null() && (failed == Some(true) || succeeded == Some(true)))
        || source["librarianLaceRead"].as_bool().is_none()
    {
        return Err("game save Hub NPC state is invalid".into());
    }
    let flags = source.entry("helpFlags").or_insert_with(|| json!(vec![false; 10]));
    if flags.as_array().is_none_or(|values| {
        values.len() != 10 || values.iter().any(|value| value.as_bool().is_none())
    }) {
        return Err("game save Hub NPC state is invalid".into());
    }
    Ok(Value::Object(source))
}

pub(super) fn normalize_hagatha_runtime(
    value: Option<&Value>,
    owned: &[u32],
    version: u64,
    index: usize,
) -> Result<NativeHagathaRuntimeState, String> {
    let runtime = if let Some(value) = value {
        let field = format!("game save Hagatha runtime {index}");
        let source = record(value, &field)?;
        only_keys(source, &field, &["cheatDeathCharges", "reverieActive", "serendipityActive"])?;
        if integer(&source["cheatDeathCharges"], &field).is_err()
            || source["cheatDeathCharges"].as_u64().is_none_or(|value| value > 1)
            || source["reverieActive"].as_bool().is_none()
            || source["serendipityActive"].as_bool().is_none()
        {
            return Err(format!("{field} is invalid"));
        }
        decode(value.clone())?
    } else {
        let mut runtime = NativeHagathaRuntimeState::default();
        runtime.purchase(owned);
        runtime
    };
    let mut normalized = runtime;
    for selector in [7, 24, 25] {
        if !owned.contains(&selector) {
            normalized.remove(selector);
        }
    }
    if version >= 24 && normalized != runtime {
        return Err(format!("game save Hagatha runtime {index} has no matching ownership"));
    }
    Ok(normalized)
}
