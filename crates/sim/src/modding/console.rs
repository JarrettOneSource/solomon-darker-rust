//! Enabled game Lua API values from `host/lua/web-lua-contract.ts`.
//! The native VM owns callbacks/timers; frame rows borrow actual Session state.

mod catalog;
mod commands;

pub use catalog::*;
pub use commands::{AppliedWebLuaCommands, apply_web_lua_commands};

use crate::Vec2;
use crate::combat::PlayerLifeState;
use crate::enemies::EnemyToken;
use crate::enemies::model::EnemyLifeState;
use crate::game_run::GameRunPhase;
use crate::session::GameSession;

use super::{LuaObject, LuaValue};

pub const WEB_LUA_API_VERSION: &str = "1.0.0";
pub const WEB_LUA_EVENT_NAMES: [&str; 10] = [
    "enemy.death",
    "enemy.spawned",
    "gold.changed",
    "item.consumed",
    "level.up",
    "run.ended",
    "run.started",
    "runtime.tick",
    "wave.completed",
    "wave.started",
];
pub const WEB_LUA_CAPABILITIES: [&str; 17] = [
    "enemies.read",
    "enemies.spawn",
    "events.read",
    "gameplay.read",
    "hub.read",
    "player.read",
    "player.resources.write",
    "player.resources.owner",
    "rng.run.seed",
    "runtime.read",
    "scene.read",
    "state.session",
    "timer.local.scheduler",
    "timer.session",
    "waves.read",
    "waves.spawn",
    "world.read",
];

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct WebLuaPlayerState {
    pub current_health: f64,
    pub current_mana: f64,
    pub death_tick: u64,
    pub discipline: String,
    pub display_name: String,
    pub element: String,
    pub experience: f64,
    pub gold: u64,
    pub id: String,
    pub level: u32,
    pub life_state: String,
    pub maximum_health: f64,
    pub maximum_mana: f64,
    pub pending_level_up: bool,
    pub position: Vec2,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct WebLuaEnemyState {
    pub health: f64,
    pub id: u64,
    pub life_state: String,
    pub maximum_health: f64,
    pub position: Vec2,
    pub token: String,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct WebLuaFrameState {
    pub authority_player_id: Option<String>,
    pub boneyard_seed: Option<String>,
    pub enemies: Vec<WebLuaEnemyState>,
    pub multiplayer: bool,
    pub phase: String,
    pub player_count: usize,
    pub players: Vec<WebLuaPlayerState>,
    pub run_id: Option<String>,
    pub scene: String,
    pub tick: u64,
    pub waves: Option<LuaObject>,
    pub world: String,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "persistence",
    serde(tag = "type", rename_all = "kebab-case", rename_all_fields = "camelCase")
)]
pub enum WebLuaCommand {
    GrantExperience {
        amount: f64,
        player_id: String,
    },
    GrantGold {
        amount: f64,
        player_id: String,
    },
    GrantItem {
        item_key: String,
        player_id: String,
        quantity: f64,
    },
    GrantSkill {
        player_id: String,
        ranks: f64,
        skill_id: u32,
    },
    GrantWeld {
        build_id: u32,
        player_id: String,
    },
    RestoreHealth {
        amount: f64,
        player_id: String,
    },
    RestoreMana {
        amount: f64,
        player_id: String,
    },
    SetGold {
        player_id: String,
        value: u64,
    },
    SetMana {
        player_id: String,
        value: f64,
    },
    SetNextRunSeed {
        seed: u32,
    },
    SpawnNextWave {
        run_id: String,
        wave_event_id: u64,
    },
    SpawnEnemy {
        request_id: u64,
        #[cfg_attr(
            feature = "persistence",
            serde(serialize_with = "serialize_token", deserialize_with = "deserialize_token")
        )]
        token: EnemyToken,
        x: f64,
        y: f64,
    },
}

impl WebLuaCommand {
    #[must_use]
    pub fn player_id(&self) -> Option<&str> {
        match self {
            Self::GrantExperience { player_id, .. }
            | Self::GrantGold { player_id, .. }
            | Self::GrantItem { player_id, .. }
            | Self::GrantSkill { player_id, .. }
            | Self::GrantWeld { player_id, .. }
            | Self::RestoreHealth { player_id, .. }
            | Self::RestoreMana { player_id, .. }
            | Self::SetGold { player_id, .. }
            | Self::SetMana { player_id, .. } => Some(player_id),
            Self::SetNextRunSeed { .. } | Self::SpawnNextWave { .. } | Self::SpawnEnemy { .. } => {
                None
            }
        }
    }
}

/// `createWebLuaFrameState`: Source identity/actor order, with no simulation step.
#[must_use]
pub fn create_web_lua_frame_state(
    game: &GameSession,
    authority_player_id: Option<&str>,
) -> WebLuaFrameState {
    let mut players = Vec::new();
    for id in &game.participant_order {
        let participant = &game.participants.get(id).expect("participant exists");
        let player = &participant.player;
        let life = match player.combat.life_state {
            PlayerLifeState::Alive => "alive",
            PlayerLifeState::LethalPending => "lethal-pending",
            PlayerLifeState::Dying => "dying",
            PlayerLifeState::Spectating => "spectating",
        };
        players.push(WebLuaPlayerState {
            current_health: player.combat.current_health,
            current_mana: player.combat.current_mana,
            death_tick: player.combat.death_tick,
            discipline: player.selection.discipline.as_str().into(),
            display_name: player.selection.name.clone(),
            element: player.selection.element.as_str().into(),
            experience: player.progression.experience,
            gold: player.economy.gold,
            id: id.clone(),
            level: player.progression.level,
            life_state: life.into(),
            maximum_health: player.combat.maximum_health,
            maximum_mana: player.combat.maximum_mana,
            pending_level_up: player.progression.pending_offer.is_some(),
            position: participant.wizard.position,
        });
    }
    let mut enemies = Vec::new();
    let (world, scene, seed, waves) = if let Some(world) = &game.boneyard {
        for enemy in world.enemies.actors.iter() {
            enemies.push(WebLuaEnemyState {
                health: enemy.current_health,
                id: enemy.id as u64,
                life_state: if enemy.life_state == EnemyLifeState::Alive {
                    "alive"
                } else {
                    "dying"
                }
                .into(),
                maximum_health: enemy.config.maximum_health,
                position: enemy.position,
                token: enemy.config.enemy_token.id().into(),
            });
        }
        let waves = world.waves.as_ref().map(|waves| {
            let mut fields = LuaObject::new();
            for (key, value) in [
                ("live_enemy_count", LuaValue::Number(world.enemies.actors.len() as f64)),
                ("pending_spawn_budget", LuaValue::Number(waves.state.pending_spawn_budget as f64)),
                ("phase", LuaValue::String(waves.state.phase.id().into())),
                ("schedule_index", LuaValue::Number(waves.state.schedule_index as f64)),
                ("spawn_next_available", LuaValue::Boolean(next_wave_available(game))),
                ("wave_event_id", LuaValue::Number(waves.state.wave_event_id as f64)),
                ("wave_ordinal", LuaValue::Number(waves.state.wave_ordinal as f64)),
            ] {
                fields.insert(key.into(), value);
            }
            fields
        });
        ("boneyard", world.loaded.name.clone(), Some(world.loaded.seed.clone()), waves)
    } else {
        let region = authority_player_id
            .and_then(|id| game.participants.get(id))
            .map_or("courtyard", |player| player.wizard.region.id());
        ("hub", format!("hub.{region}"), None, None)
    };
    WebLuaFrameState {
        authority_player_id: authority_player_id.map(str::to_owned),
        boneyard_seed: seed,
        enemies,
        multiplayer: players.len() > 1,
        phase: game.run.phase.id().into(),
        player_count: players.len(),
        players,
        run_id: game.run.run_id.clone(),
        scene,
        tick: game.ticks,
        waves,
        world: world.into(),
    }
}

fn next_wave_available(game: &GameSession) -> bool {
    game.run.phase == GameRunPhase::Active
        && game.level_up_barrier.is_none()
        && game.boneyard.as_ref().is_some_and(|world| {
            world.tutorial.is_none()
                && world.waves.as_ref().is_some_and(|waves| waves.can_spawn_next_wave())
        })
}

#[cfg(feature = "persistence")]
fn serialize_token<S: serde::Serializer>(
    token: &EnemyToken,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(token.id())
}
#[cfg(feature = "persistence")]
fn deserialize_token<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<EnemyToken, D::Error> {
    let value = <String as serde::Deserialize>::deserialize(deserializer)?;
    EnemyToken::from_id(&value).ok_or_else(|| serde::de::Error::custom("unknown stock enemy token"))
}
