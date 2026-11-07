//! Plays one package's session content the way the hosted authority would for a player who
//! enabled it.
//!
//! The supervisor admits content with `materialize_web_session_content_value` and
//! `compile_web_session_content_definitions`; `HostedGameHost::new` then compiles its assets,
//! its content catalog and the mods' Lua runtime for a private College. A sweep enters every
//! Boneyard a player of the package could choose: the stock Random Boneyard when the package
//! has Lua, and each Boneyard it provides. Each tick follows the private College's order in
//! `HostedGameHost::step_prepared_worlds`: the mods' player controls and actors, the session
//! tick with the mods' extensions, then the mods' rules over the events that tick produced.
//! Connections, the developer Console, bots and run archives take no part.

use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::Arc;

use solomon_launcher::host::HostContent;
use solomon_launcher::host::hosted::{HostedGameHost, HostedWorlds};
use solomon_modding::host::PreparedModHost;
use solomon_modding::package::{
    MaterializedWebSessionContent, compile_web_session_content_definitions,
    materialize_web_session_content_value,
};
use solomon_sim::Vec2;
use solomon_sim::boneyard::BoneyardSource;
use solomon_sim::constants::TICK_RATE_HZ;
use solomon_sim::modding::events::{GameSimulationEventSnapshot, derive_web_lua_events};
use solomon_sim::modding::extensions::GameSimulationExtensions;
use solomon_sim::modding::{LuaObject, LuaValue};
use solomon_sim::profile::{NewGameSelection, WizardDiscipline, WizardElement};
use solomon_sim::session::{GameSessionInput, GameSessionTickContext};
use solomon_sim::social::{GameSessionKind, PartyIdentity};

use crate::Stage;

const PLAYER: &str = "sweep-player";
const MAXIMUM_PLAYERS: usize = 16;
const WORLD_SEED: u32 = 0x5eed;
const BONEYARD_SEED: [u8; 16] = [0; 16];
const RUN_ID: &str = "content-sweep-run";

/// One Boneyard the sweep entered and how many session ticks it advanced there.
pub struct Play {
    pub choice_id: String,
    pub name: String,
    pub stock: bool,
    pub source_sha256: String,
    pub ticks: u64,
}

/// Admits, prepares and plays a session content document. `stage` names the step in progress,
/// so a failure or panic reports where it happened.
pub fn play_package(
    content: &str,
    assets: &Path,
    ticks: u64,
    stage: &Cell<Stage>,
) -> Result<Vec<Play>, String> {
    stage.set(Stage::Admission);
    let value: LuaValue = serde_json::from_str(content).map_err(|error| error.to_string())?;
    let content = materialize_web_session_content_value(&value)?;
    stage.set(Stage::Definitions);
    let content = Arc::new(compile_web_session_content_definitions(content)?);
    stage.set(Stage::Preparation);
    // The College the supervisor provisions for this content. Building it compiles the mods'
    // assets, content catalog and Lua runtime; the sweep then plays fresh worlds of its own.
    HostedGameHost::new(
        "content-sweep".into(),
        GameSessionKind::PrivateCollege,
        MAXIMUM_PLAYERS,
        Arc::clone(&content),
        assets,
        "content-sweep-recovery-secret".into(),
        "0".repeat(40),
    )?;
    let catalog = HostContent::load_with_mod_entries(assets, content.boneyards.clone())?;
    let mut choices = Vec::new();
    if !content.mod_sources.is_empty() {
        choices.push("default-random".to_owned());
    }
    choices.extend(content.boneyards.iter().map(|entry| entry.choice.id.clone()));
    let mut plays = Vec::with_capacity(choices.len());
    for choice in &choices {
        plays.push(play_boneyard(&content, &catalog, choice, ticks, stage)?);
    }
    Ok(plays)
}

fn play_boneyard(
    content: &MaterializedWebSessionContent,
    catalog: &HostContent,
    choice_id: &str,
    ticks: u64,
    stage: &Cell<Stage>,
) -> Result<Play, String> {
    stage.set(Stage::Preparation);
    let mut worlds = HostedWorlds::new(GameSessionKind::PrivateCollege, WORLD_SEED)?;
    let mut runtime = match &mut worlds {
        HostedWorlds::Private { state, .. } if !content.mod_sources.is_empty() => {
            Some(PreparedModHost::prepare(content, state)?)
        }
        _ => None,
    };
    let identity = PartyIdentity {
        id: "sweep-party".into(),
        join_code: "SWEEPPARTY".into(),
        listing_id: "sweep-listing".into(),
    };
    let selection = NewGameSelection {
        name: "Sweep".into(),
        element: WizardElement::Ether,
        discipline: WizardDiscipline::Arcane,
    };
    worlds.add_player(PLAYER.into(), selection, identity, MAXIMUM_PLAYERS)?;
    let HostedWorlds::Private { state, .. } = &mut worlds else {
        unreachable!("a private College keeps one session")
    };

    stage.set(Stage::Boneyard);
    let (loaded, navigation) = catalog.prepare_choice(BONEYARD_SEED, choice_id, RUN_ID)?;
    let name = loaded.name.clone();
    let stock = loaded.choice_source != BoneyardSource::Mod;
    let source_sha256 = loaded.source_sha256.clone();
    let lua_content_id = content
        .boneyards
        .iter()
        .find(|entry| entry.choice.id == choice_id)
        .and_then(|entry| entry.web_lua_content_id.clone());
    let entered = GameSimulationEventSnapshot::capture(state);
    state.install_boneyard(loaded)?;

    stage.set(Stage::Session);
    let mut pending = Vec::new();
    if let Some(runtime) = &mut runtime {
        runtime.activate_boneyard(lua_content_id.as_deref(), false, state)?;
        pending =
            derive_web_lua_events(&entered, &GameSimulationEventSnapshot::capture(state), |_| true);
    }
    let first_tick = state.ticks;
    let intro_ready = BTreeSet::new();
    for index in 0..ticks {
        let mut inputs = BTreeMap::from([(PLAYER.to_owned(), walking(index))]);
        let mut spawns = Vec::new();
        let mut before = None;
        if let Some(runtime) = &mut runtime {
            // An open mod scene holds a private College's clock until the player leaves it.
            if let Some(run_id) = &state.run.run_id
                && runtime.active_scene(run_id)?.is_some()
            {
                break;
            }
            before = Some(GameSimulationEventSnapshot::capture(state));
            // A control that releases a skill picker pauses the College for its grace.
            let now_ms = index as f64 * 1_000.0 / f64::from(TICK_RATE_HZ);
            if runtime.apply_player_controls(&mut inputs, now_ms, state)? {
                continue;
            }
            runtime.tick(state.ticks + 1, state)?;
            spawns = runtime.drain_enemy_spawns()?;
        }
        state.step_with_extensions(
            &inputs,
            GameSessionTickContext {
                navigation: Some(&navigation),
                college_intro_ready_player_ids: Some(&intro_ready),
                enemy_spawn_intents: &spawns,
            },
            runtime.as_ref().map(|runtime| runtime.runtime() as &dyn GameSimulationExtensions),
        )?;
        if let (Some(runtime), Some(before)) = (&mut runtime, before) {
            let mut events = std::mem::take(&mut pending);
            events.extend(derive_web_lua_events(
                &before,
                &GameSimulationEventSnapshot::capture(state),
                |_| true,
            ));
            let scope = state.run.run_id.clone().unwrap_or_else(|| "private-session".into());
            runtime.step(&events, state.ticks, &scope, &LuaObject::new(), state)?;
            // The host logs these and keeps running; for the sweep a failing rule is a failure.
            let failures = runtime.drain_logs();
            if !failures.is_empty() {
                return Err(format!(
                    "Mod rules failed at tick {}: {}",
                    state.ticks,
                    failures.join("; ")
                ));
            }
        }
    }
    if let Some(runtime) = &runtime {
        // What the host sends a player's client, and what it keeps in the player's save.
        runtime.project()?;
        runtime.runtime_projection(Some(PLAYER), state)?;
        runtime.save_state()?;
    }
    Ok(Play {
        choice_id: choice_id.into(),
        name,
        stock,
        source_sha256,
        ticks: state.ticks - first_tick,
    })
}

/// Standing still for a second, then walking right, down, left and up for a second each, so
/// movement and collision run against the Boneyard's own geometry.
fn walking(index: u64) -> GameSessionInput {
    let movement = match (index / u64::from(TICK_RATE_HZ)) % 5 {
        0 => Vec2::ZERO,
        1 => Vec2::new(1.0, 0.0),
        2 => Vec2::new(0.0, 1.0),
        3 => Vec2::new(-1.0, 0.0),
        _ => Vec2::new(0.0, -1.0),
    };
    GameSessionInput { movement, ..GameSessionInput::default() }
}
