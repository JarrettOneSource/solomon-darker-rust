//! Owner-only disk projections from `save/game-save-document.ts`.

use serde::Serialize;
use serde_json::{Value, json};
use solomon_sim::boneyard::LoadedBoneyard;
use solomon_sim::economy::{HubEconomyState, project_inventory_root_slots};
use solomon_sim::game_run::GameRunPhase;
use solomon_sim::modding::LuaObject;
use solomon_sim::modding::identity::GameContentIdentity;
use solomon_sim::session::GameSession;

use super::contract::{
    MAX_SAVE_BYTES, NativeSaveSource, SCHEMA_VERSION, SaveIntegrity, SaveMetadata,
};
use super::projection;
use super::value::encode;

pub struct CreateSaveOptions<'a> {
    pub state: &'a GameSession,
    pub player_id: &'a str,
    pub loaded_boneyard: Option<&'a LoadedBoneyard>,
    pub metadata: &'a SaveMetadata,
    pub party_rejoin_token: Option<&'a str>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Document<'a> {
    continuation: Option<Value>,
    integrity: SaveIntegrity,
    mods: &'a [GameContentIdentity],
    mod_state: &'a LuaObject,
    native_source: &'a Option<NativeSaveSource>,
    profile: Value,
    schema_version: u64,
}

pub fn create(options: CreateSaveOptions<'_>) -> Result<String, String> {
    let mut state = owner_projection(options.state, options.player_id)?;
    if matches!(state.run.phase, GameRunPhase::GameOver | GameRunPhase::Loadout) {
        return Err(format!(
            "game save cannot checkpoint {}",
            encode(&state.run.phase)?.as_str().unwrap()
        ));
    }
    let loaded = match (&state.boneyard, options.loaded_boneyard) {
        (None, None) => Value::Null,
        (None, Some(_)) => return Err("Hub game save carries a Boneyard".into()),
        (Some(_), None) => return Err("Boneyard game save is missing its loaded content".into()),
        (Some(world), Some(loaded)) => {
            if state.run.phase != GameRunPhase::Active
                || state.run.run_id.as_ref() != Some(&loaded.run_id)
                || world.loaded.run_id != loaded.run_id
            {
                return Err("Boneyard run ownership is inconsistent".into());
            }
            projection::loaded_boneyard(loaded)?
        }
    };
    if let Some(token) = options.party_rejoin_token
        && (!token.starts_with("sdrpr2.")
            || !super::contract::valid_party_token(token, true)
            || state.boneyard.is_none()
            || state.run.phase != GameRunPhase::Active)
    {
        return Err("game save party rejoin token requires an active Boneyard".into());
    }
    let profile = profile(&state, options.player_id, false)?;
    disk_projection(&mut state);
    let owner = &state.participants[options.player_id];
    let world_kind = if state.boneyard.is_some() { "boneyard" } else { "hub" };
    let continuation = json!({
        "loadedBoneyard": loaded,
        "simulation": projection::simulation(&state, options.player_id)?,
        "summary": {
            "activeRun": state.boneyard.is_some() && state.run.phase == GameRunPhase::Active,
            "character": encode(&owner.player.selection)?,
            "partyRejoinToken": options.party_rejoin_token,
            "phase": encode(&state.run.phase)?, "playerId": options.player_id,
            "savedAtTick": state.ticks, "worldKind": world_kind,
        },
    });
    encode_document(Some(continuation), profile, options.metadata)
}

pub fn create_profile(
    state: &GameSession,
    player_id: &str,
    metadata: &SaveMetadata,
) -> Result<String, String> {
    let owner = owner_projection(state, player_id)?;
    encode_document(None, profile(&owner, player_id, true)?, metadata)
}

pub fn retire(document: &str) -> Result<String, String> {
    let restored = super::restore(document)?;
    let mut profile = restored
        .session
        .retired_participant_profile(&restored.player_id)
        .ok_or("game save profile owner is absent")?;
    disk_economy(&mut profile.economy);
    encode_document(
        None,
        json!({"economy":encode(&profile.economy)?, "hagathaRuntime":encode(&profile.hagatha_runtime)?,
            "advancedUnlocks":profile.advanced_unlocks}),
        &restored.metadata,
    )
}

pub(super) fn normalize_inventory(economy: &mut HubEconomyState) {
    fn materialize(
        items: &[solomon_sim::economy::HubInventoryItem],
    ) -> Vec<solomon_sim::economy::HubInventoryItem> {
        project_inventory_root_slots(items)
            .into_iter()
            .map(|entry| {
                let mut item = entry.item.clone();
                item.inventory_slot = Some(entry.slot);
                if let Some(contents) = item.contents.as_mut() {
                    *contents = materialize(contents);
                }
                item
            })
            .collect()
    }
    economy.backpack = materialize(&economy.backpack);
    economy.storage = materialize(&economy.storage);
}

fn disk_economy(economy: &mut HubEconomyState) {
    normalize_inventory(economy);
    if economy.action_feedback.as_ref().is_some_and(|feedback| {
        feedback.dye_session_id.is_some()
            || feedback.action == solomon_sim::economy::HubActionFeedbackAction::RenameSack
    }) {
        economy.action_feedback = None;
    }
}

fn owner_projection(state: &GameSession, player_id: &str) -> Result<GameSession, String> {
    if !state.participants.contains_key(player_id) {
        return Err("game save owner is absent from authoritative state".into());
    }
    let mut owner = state.clone();
    for id in &state.participant_order {
        if id != player_id {
            owner.remove_participant(id);
        }
    }
    if !owner.participants[player_id].player.economy.inventory_is_valid() {
        return Err("game save owner inventory is invalid".into());
    }
    Ok(owner)
}

fn disk_projection(state: &mut GameSession) {
    for participant in state.participants.values_mut() {
        let player = &mut participant.player;
        disk_economy(&mut player.economy);
        player.progression.damage_x4_ticks_remaining = 0;
        player.progression.mind_chug_ticks_remaining = 0;
        player.skill_runtime.meditation_activity_ramp_ticks = 0;
        player.skill_runtime.meditation_idle_elapsed_ticks = 0;
        player.skill_runtime.mindstar_active = false;
        player.skill_runtime.refresh_with_stat_book(
            &mut player.book,
            &player.stat_book,
            &player.economy,
        );
    }
    for player in state.secondary.players.values_mut() {
        player.cast_spin_ticks_remaining = 0;
        player.held_slot = None;
        player.last_skill_id = None;
        player.mindstar = false;
        player.plane_orb_held = false;
        player.regenerate = false;
        player.reserved_mana = if player.firewalker { 50.0 } else { 0.0 };
        player.cast_action = None;
    }
}

fn profile(state: &GameSession, player_id: &str, durable: bool) -> Result<Value, String> {
    let mut economy = if durable {
        state.durable_profile_economy(player_id)
    } else {
        state.retired_wizard_economy(player_id)
    }
    .ok_or("game save profile owner is absent")?;
    disk_economy(&mut economy);
    Ok(
        json!({"economy":encode(&economy)?, "hagathaRuntime":encode(&state.participants[player_id].player.progression.hagatha_runtime)?,
            "advancedUnlocks":state.participants[player_id].player.book.advanced_unlocks}),
    )
}

fn encode_document(
    continuation: Option<Value>,
    profile: Value,
    metadata: &SaveMetadata,
) -> Result<String, String> {
    let text = serde_json::to_string(&Document {
        continuation,
        profile,
        integrity: metadata.integrity,
        mods: &metadata.mods,
        mod_state: &metadata.mod_state,
        native_source: &metadata.native_source,
        schema_version: SCHEMA_VERSION,
    })
    .map_err(|error| error.to_string())?;
    if text.len() > MAX_SAVE_BYTES {
        return Err("game save exceeds its size limit".into());
    }
    Ok(text)
}
