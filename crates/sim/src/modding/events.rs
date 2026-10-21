//! Semantic events from `host/lua/web-lua-game-api.ts::deriveWebLuaEvents`.
//! The host retains these small facts across an epoch, never a cloned Session.

use crate::Vec2;
use crate::boneyard::waves::director::WavePhase;
use crate::enemies::model::EnemyEventKind;
use crate::game_run::GameRunPhase;
use crate::session::GameSession;

use super::{LuaObject, LuaValue};

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct WebLuaDerivedEvent {
    pub name: String,
    pub payload: LuaValue,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct GameSimulationEventSnapshot {
    pub run_id: Option<String>,
    pub phase: GameRunPhase,
    pub tick: u64,
    pub world: Option<BoneyardEventSnapshot>,
    pub players: Vec<PlayerEventSnapshot>,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct PlayerEventSnapshot {
    pub id: String,
    pub gold: u64,
    pub level: u32,
    pub experience: f64,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct BoneyardEventSnapshot {
    pub waves: Option<WaveEventSnapshot>,
    pub last_enemy_event_id: usize,
    pub enemy_events: Vec<NativeEnemyEventSnapshot>,
    pub actors: Vec<NativeEnemyActorEventSnapshot>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct WaveEventSnapshot {
    pub event_id: u64,
    pub ordinal: u64,
    pub phase: WavePhase,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NativeEnemyEventSnapshot {
    pub actor_id: usize,
    pub event_id: usize,
    pub kind: NativeEnemySemanticEventKind,
    pub target_player_id: Option<String>,
    pub tick: u64,
    pub source_position: Option<Vec2>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum NativeEnemySemanticEventKind {
    Spawned,
    Death,
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NativeEnemyActorEventSnapshot {
    pub id: usize,
    pub native_type_id: u16,
    pub position: Vec2,
}

impl GameSimulationEventSnapshot {
    #[must_use]
    pub fn capture(state: &GameSession) -> Self {
        let mut players = Vec::with_capacity(state.participant_order.len());
        for id in &state.participant_order {
            let player = &state.participants[id].player;
            players.push(PlayerEventSnapshot {
                id: id.clone(),
                gold: player.economy.gold,
                level: player.progression.level,
                experience: player.progression.experience,
            });
        }
        let world = state.boneyard.as_ref().map(|world| {
            let mut actors = Vec::with_capacity(world.enemies.actors.len());
            for actor in &world.enemies.actors {
                actors.push(NativeEnemyActorEventSnapshot {
                    id: actor.id,
                    native_type_id: actor.config.native_type_id,
                    position: actor.position,
                });
            }
            let mut enemy_events = Vec::new();
            for event in &world.enemy_events {
                let kind = match event.kind {
                    EnemyEventKind::EnemySpawned => NativeEnemySemanticEventKind::Spawned,
                    EnemyEventKind::EnemyDeath => NativeEnemySemanticEventKind::Death,
                    _ => continue,
                };
                enemy_events.push(NativeEnemyEventSnapshot {
                    actor_id: event.actor_id,
                    event_id: event.event_id,
                    kind,
                    target_player_id: event.target_player_id.clone().flatten(),
                    tick: event.tick,
                    source_position: event.source_position,
                });
            }
            BoneyardEventSnapshot {
                waves: world.waves.as_ref().map(|waves| WaveEventSnapshot {
                    event_id: waves.state.wave_event_id,
                    ordinal: waves.state.wave_ordinal,
                    phase: waves.state.phase,
                }),
                last_enemy_event_id: world.enemy_events.last().map_or(0, |event| event.event_id),
                enemy_events,
                actors,
            }
        });
        Self {
            run_id: state.run.run_id.clone(),
            phase: state.run.phase,
            tick: state.ticks,
            world,
            players,
        }
    }
}

/// Event and participant order are the source traversal order. `includes` lets the
/// Console omit events it does not subscribe to; prepared content requests all events.
#[must_use]
pub fn derive_web_lua_events(
    previous: &GameSimulationEventSnapshot,
    current: &GameSimulationEventSnapshot,
    includes: impl Fn(&str) -> bool,
) -> Vec<WebLuaDerivedEvent> {
    let mut events = Vec::new();
    if includes("run.started")
        && previous.run_id != current.run_id
        && let Some(run_id) = &current.run_id
    {
        events.push(event(
            "run.started",
            [
                ("event", text("run.started")),
                ("run_id", text(run_id)),
                ("tick", number(current.tick)),
            ],
        ));
    }
    if includes("run.ended")
        && current.run_id.is_none()
        && matches!(current.phase, GameRunPhase::Loadout | GameRunPhase::Hub)
        && let Some(run_id) = &previous.run_id
    {
        let reason = if previous.phase == GameRunPhase::GameOver { "game-over" } else { "ended" };
        events.push(event(
            "run.ended",
            [
                ("event", text("run.ended")),
                ("reason", text(reason)),
                ("run_id", text(run_id)),
                ("tick", number(current.tick)),
            ],
        ));
    }
    if let (Some(before), Some(after)) = (&previous.world, &current.world) {
        if includes("wave.started")
            && let Some(wave) = after.waves
            && before.waves.is_none_or(|previous| wave.event_id > previous.event_id)
        {
            events.push(event(
                "wave.started",
                [
                    ("event", text("wave.started")),
                    ("tick", number(current.tick)),
                    ("wave_event_id", number(wave.event_id)),
                    ("wave_ordinal", number(wave.ordinal)),
                    ("wave", number(wave.ordinal)),
                ],
            ));
        }
        if includes("wave.completed")
            && before.waves.is_some_and(|wave| wave.phase == WavePhase::WaveThreshold)
            && let Some(wave) = after.waves.filter(|wave| wave.phase == WavePhase::WaveLullDelay)
        {
            events.push(event(
                "wave.completed",
                [
                    ("event", text("wave.completed")),
                    ("tick", number(current.tick)),
                    ("wave", number(wave.ordinal)),
                    ("wave_ordinal", number(wave.ordinal)),
                ],
            ));
        }
        for row in &after.enemy_events {
            if row.event_id <= before.last_enemy_event_id {
                continue;
            }
            let name = match row.kind {
                NativeEnemySemanticEventKind::Spawned => "enemy.spawned",
                NativeEnemySemanticEventKind::Death => "enemy.death",
            };
            if !includes(name) {
                continue;
            }
            let actor = after
                .actors
                .iter()
                .find(|actor| actor.id == row.actor_id)
                .or_else(|| before.actors.iter().find(|actor| actor.id == row.actor_id));
            let position = row
                .source_position
                .or_else(|| actor.map(|actor| actor.position))
                .unwrap_or(Vec2::ZERO);
            events.push(event(
                name,
                [
                    ("actor_id", number(row.actor_id as u64)),
                    ("content_id", number(0)),
                    (
                        "enemy_type",
                        number(u64::from(actor.map_or(0, |actor| actor.native_type_id))),
                    ),
                    ("event", text(name)),
                    ("event_id", number(row.event_id as u64)),
                    (
                        "target_player_id",
                        row.target_player_id.as_deref().map_or(LuaValue::Null, text),
                    ),
                    ("tick", number(row.tick)),
                    ("x", LuaValue::Number(position.x)),
                    ("y", LuaValue::Number(position.y)),
                ],
            ));
        }
    }
    for player in &current.players {
        let Some(before) = previous.players.iter().find(|previous| previous.id == player.id) else {
            continue;
        };
        if includes("gold.changed") && player.gold != before.gold {
            events.push(event(
                "gold.changed",
                [
                    ("delta", LuaValue::Number(player.gold as f64 - before.gold as f64)),
                    ("event", text("gold.changed")),
                    ("gold", number(player.gold)),
                    ("player_id", text(&player.id)),
                    ("source", text("web-authority")),
                ],
            ));
        }
        if includes("level.up") && player.level > before.level {
            events.push(event(
                "level.up",
                [
                    ("event", text("level.up")),
                    ("level", number(u64::from(player.level))),
                    ("player_id", text(&player.id)),
                    ("xp", LuaValue::Number(player.experience)),
                ],
            ));
        }
    }
    events
}

fn event<const N: usize>(name: &str, fields: [(&str, LuaValue); N]) -> WebLuaDerivedEvent {
    let mut payload = LuaObject::new();
    for (key, value) in fields {
        payload.insert(key.into(), value);
    }
    WebLuaDerivedEvent { name: name.into(), payload: LuaValue::Object(payload) }
}

fn text(value: &str) -> LuaValue {
    LuaValue::String(value.into())
}
fn number(value: u64) -> LuaValue {
    LuaValue::Number(value as f64)
}
