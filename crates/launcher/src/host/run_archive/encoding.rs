//! Private Rust archive encoding: one canonical checkpoint table plus references.
//! Source lifecycle/summary semantics are retained; V8 serialization is not a runtime dependency.

use std::cell::Cell;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Instant;

use serde::{Deserialize, Serialize, Serializer};
use solomon_sim::boneyard::LoadedBoneyard;
use solomon_sim::enemies::EnemySpawnIntent;
use solomon_sim::modding::identity::GameContentManifest;
use solomon_sim::session::GameSessionInput;

use super::recorder::{
    ArchivedState, RunArchive, RunArchiveCheckpoint, RunArchiveEndReason, RunArchivePerformance,
};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoredCheckpoint {
    pub state: usize,
    pub inputs: BTreeMap<String, GameSessionInput>,
    pub enemy_spawn_intents: Vec<EnemySpawnIntent>,
    pub tick_ms: f64,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArchiveMetadata {
    pub schema_version: u32,
    pub id: String,
    pub run_id: String,
    pub session_id: String,
    pub revision: String,
    pub protocol_version: u32,
    pub runtime_version: String,
    pub started_at_utc: String,
    pub ended_at_utc: String,
    pub end_reason: RunArchiveEndReason,
    pub content: GameContentManifest,
    pub loaded_boneyard: LoadedBoneyard,
    pub last_alive: Option<StoredCheckpoint>,
    pub last_alive_by_player: Vec<(String, StoredCheckpoint)>,
    pub worst_tick: StoredCheckpoint,
    pub final_state: usize,
    pub performance: RunArchivePerformance,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RunArchiveFile<S = ArchivedState, M = ArchiveMetadata> {
    pub encoding: String,
    pub encoding_version: u32,
    pub states: Vec<S>,
    pub archive: M,
}

#[derive(Clone, Copy, Default)]
pub struct SerializationMetrics {
    pub total_ms: f64,
    pub maximum_slice_ms: f64,
}
struct Timed<'a, T>(&'a T, &'a Cell<SerializationMetrics>);
impl<T: Serialize> Serialize for Timed<'_, T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // This worker owns the queue. Yield between the same complete-world slices
        // that Source preloads into V8's shared object table.
        std::thread::yield_now();
        let started = Instant::now();
        let result = self.0.serialize(serializer);
        let elapsed = started.elapsed().as_secs_f64() * 1000.0;
        let previous = self.1.get();
        self.1.set(SerializationMetrics {
            total_ms: previous.total_ms + elapsed,
            maximum_slice_ms: previous.maximum_slice_ms.max(elapsed),
        });
        result
    }
}

fn intern<'a>(states: &mut Vec<&'a ArchivedState>, state: &'a Arc<ArchivedState>) -> usize {
    if let Some(index) =
        states.iter().position(|candidate| std::ptr::eq(*candidate, state.as_ref()))
    {
        return index;
    }
    states.push(state.as_ref());
    states.len() - 1
}
fn checkpoint<'a>(
    states: &mut Vec<&'a ArchivedState>,
    value: &'a RunArchiveCheckpoint,
) -> StoredCheckpoint {
    StoredCheckpoint {
        state: intern(states, &value.state),
        inputs: (*value.inputs).clone(),
        enemy_spawn_intents: (*value.enemy_spawn_intents).clone(),
        tick_ms: value.tick_ms,
    }
}

pub fn serialize_archive(
    archive: &RunArchive,
    id: &str,
) -> Result<(Vec<u8>, SerializationMetrics), String> {
    let mut states = Vec::new();
    let worst_tick = checkpoint(&mut states, &archive.worst_tick);
    let last_alive_by_player = archive
        .last_alive_by_player
        .iter()
        .map(|(id, value)| (id.clone(), checkpoint(&mut states, value)))
        .collect();
    let final_state = intern(&mut states, &archive.final_state);
    let last_alive = archive.last_alive.as_ref().map(|value| checkpoint(&mut states, value));
    let metadata = ArchiveMetadata {
        schema_version: 1,
        id: id.into(),
        run_id: archive.run_id.clone(),
        session_id: archive.session_id.clone(),
        revision: archive.revision.clone(),
        protocol_version: solomon_protocol::PROTOCOL_VERSION,
        runtime_version: format!("solomon-rust/{}", env!("CARGO_PKG_VERSION")),
        started_at_utc: archive.started_at_utc.clone(),
        ended_at_utc: archive.ended_at_utc.clone(),
        end_reason: archive.end_reason,
        content: archive.content.clone(),
        loaded_boneyard: (*archive.loaded_boneyard).clone(),
        last_alive,
        last_alive_by_player,
        worst_tick,
        final_state,
        performance: archive.performance.clone(),
    };
    let metrics = Cell::new(SerializationMetrics::default());
    let file = RunArchiveFile {
        encoding: "solomon-rust-checkpoint-table".into(),
        encoding_version: 1,
        states: states.iter().map(|state| Timed(*state, &metrics)).collect(),
        archive: Timed(&metadata, &metrics),
    };
    let bytes = serde_json::to_vec(&file).map_err(|error| error.to_string())?;
    Ok((bytes, metrics.get()))
}

pub fn decode_archive(bytes: &[u8]) -> Result<RunArchiveFile, String> {
    let file: RunArchiveFile = serde_json::from_slice(bytes)
        .map_err(|error| format!("Invalid private Rust run archive: {error}"))?;
    let metadata = &file.archive;
    if file.encoding != "solomon-rust-checkpoint-table"
        || file.encoding_version != 1
        || metadata.schema_version != 1
        // Protocol 2 changes live snapshot encoding, not the archived checkpoint schema.
        || !matches!(metadata.protocol_version, 1 | 2)
        || metadata.revision.len() != 40
        || !metadata
            .revision
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        || metadata.loaded_boneyard.run_id != metadata.run_id
        || file
            .states
            .get(metadata.worst_tick.state)
            .is_none_or(|state| state.checkpoint.boneyard.is_none())
        || metadata.final_state >= file.states.len()
        || metadata.last_alive.as_ref().is_some_and(|value| value.state >= file.states.len())
        || metadata.last_alive_by_player.iter().any(|(_, value)| value.state >= file.states.len())
    {
        return Err("Run archive metadata or checkpoint is invalid".into());
    }
    Ok(file)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchivePlayer {
    pub player_id: String,
    pub display_name: String,
    pub last_alive_tick: Option<u64>,
}
pub fn archive_players(archive: &RunArchive) -> Vec<ArchivePlayer> {
    let mut players = Vec::new();
    for (id, checkpoint) in &archive.last_alive_by_player {
        players.push(ArchivePlayer {
            player_id: id.clone(),
            display_name: checkpoint
                .state
                .checkpoint
                .participants
                .get(id)
                .expect("participant exists")
                .player
                .selection
                .name
                .clone(),
            last_alive_tick: Some(checkpoint.state.checkpoint.ticks),
        });
    }
    for id in &archive.final_state.checkpoint.participant_order {
        let name = archive
            .final_state
            .checkpoint
            .participants
            .get(id)
            .expect("participant exists")
            .player
            .selection
            .name
            .clone();
        if let Some(player) = players.iter_mut().find(|player| player.player_id == *id) {
            player.display_name = name;
        } else {
            players.push(ArchivePlayer {
                player_id: id.clone(),
                display_name: name,
                last_alive_tick: None,
            });
        }
    }
    players
}
