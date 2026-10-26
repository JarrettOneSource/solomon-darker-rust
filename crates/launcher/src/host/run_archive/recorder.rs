//! Source `host/run-archive.ts`. Checkpoint retention is separate from serialization/I/O.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use solomon_sim::boneyard::LoadedBoneyard;
use solomon_sim::combat::PlayerLifeState;
use solomon_sim::economy::NativeInventoryDyeSession;
use solomon_sim::enemies::EnemySpawnIntent;
use solomon_sim::game_run::GameRunPhase;
use solomon_sim::modding::identity::GameContentManifest;
use solomon_sim::session::{GameCheckpoint, GameSession, GameSessionInput};

type Inputs = BTreeMap<String, GameSessionInput>;

/// Diagnostic capture retains the live menu permission separately from the durable
/// checkpoint, whose file serializer intentionally omits that transient field.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArchivedState {
    pub checkpoint: GameCheckpoint,
    pub inventory_dye_sessions: BTreeMap<String, NativeInventoryDyeSession>,
}
impl ArchivedState {
    pub fn capture(state: &GameSession) -> Arc<Self> {
        Arc::new(Self {
            checkpoint: state.checkpoint(),
            inventory_dye_sessions: state.inventory_dye_sessions.clone(),
        })
    }
}

#[derive(Clone)]
pub struct RunArchiveCheckpoint {
    pub state: Arc<ArchivedState>,
    pub inputs: Arc<Inputs>,
    pub enemy_spawn_intents: Arc<Vec<EnemySpawnIntent>>,
    pub tick_ms: f64,
}
impl RunArchiveCheckpoint {
    pub fn new(
        state: Arc<ArchivedState>,
        inputs: Inputs,
        intents: Vec<EnemySpawnIntent>,
        tick_ms: f64,
    ) -> Self {
        Self { state, inputs: Arc::new(inputs), enemy_spawn_intents: Arc::new(intents), tick_ms }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RunArchivePerformanceWindow {
    pub tick: u64,
    pub tick_count: u64,
    pub mean_tick_ms: f64,
    pub maximum_tick_ms: f64,
    pub maximum_behind_ms: f64,
    pub over_budget_ticks: u64,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RunArchivePerformance {
    pub tick_count: u64,
    pub over_budget_ticks: u64,
    pub total_tick_ms: f64,
    pub recent: Vec<RunArchivePerformanceWindow>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RunArchiveEndReason {
    GameOver,
    WorldEnded,
    HostClosed,
    ServerError,
}

pub struct RunArchive {
    pub run_id: String,
    pub session_id: String,
    pub revision: String,
    pub started_at_utc: String,
    pub ended_at_utc: String,
    pub end_reason: RunArchiveEndReason,
    pub content: GameContentManifest,
    pub loaded_boneyard: Arc<LoadedBoneyard>,
    pub last_alive: Option<RunArchiveCheckpoint>,
    /// Source Map insertion order, independent of participant IDs.
    pub last_alive_by_player: Vec<(String, RunArchiveCheckpoint)>,
    pub worst_tick: RunArchiveCheckpoint,
    pub final_state: Arc<ArchivedState>,
    pub performance: RunArchivePerformance,
}

struct ActiveRun {
    loaded: Arc<LoadedBoneyard>,
    started_at_utc: String,
    last_alive: Option<RunArchiveCheckpoint>,
    last_alive_by_player: Vec<(String, RunArchiveCheckpoint)>,
    worst_tick: RunArchiveCheckpoint,
    final_state: Arc<ArchivedState>,
    performance: RunArchivePerformance,
}

pub struct RunArchiveRecorder {
    runs: Vec<ActiveRun>,
    session_id: String,
    revision: String,
    content: GameContentManifest,
    now: fn() -> String,
}

impl RunArchiveRecorder {
    pub fn new(session_id: String, revision: String, content: GameContentManifest) -> Self {
        Self::with_clock(session_id, revision, content, utc_now)
    }
    pub fn with_clock(
        session_id: String,
        revision: String,
        content: GameContentManifest,
        now: fn() -> String,
    ) -> Self {
        Self { runs: Vec::new(), session_id, revision, content, now }
    }
    pub fn loaded_for_run(&self, id: &str) -> Option<Arc<LoadedBoneyard>> {
        self.runs.iter().find(|run| run.loaded.run_id == id).map(|run| Arc::clone(&run.loaded))
    }
    fn active_run(&mut self, state: Arc<ArchivedState>, loaded: &LoadedBoneyard) -> &mut ActiveRun {
        let index = self
            .runs
            .iter()
            .position(|run| run.loaded.run_id == loaded.run_id)
            .unwrap_or_else(|| {
                let worst_tick =
                    RunArchiveCheckpoint::new(Arc::clone(&state), Inputs::new(), Vec::new(), 0.0);
                self.runs.push(ActiveRun {
                    loaded: Arc::new(loaded.clone()),
                    started_at_utc: (self.now)(),
                    last_alive: None,
                    last_alive_by_player: Vec::new(),
                    worst_tick,
                    final_state: state,
                    performance: RunArchivePerformance::default(),
                });
                self.runs.len() - 1
            });
        &mut self.runs[index]
    }
    pub fn checkpoint(&mut self, state: &GameSession, loaded: &LoadedBoneyard) {
        if state.run.phase != GameRunPhase::Active {
            return;
        }
        let state = ArchivedState::capture(state);
        let run = self.active_run(Arc::clone(&state), loaded);
        run.final_state = Arc::clone(&state);
        let inputs = run
            .last_alive
            .as_ref()
            .map_or_else(|| Arc::new(Inputs::new()), |last| Arc::clone(&last.inputs));
        remember_living(
            run,
            RunArchiveCheckpoint {
                state,
                inputs,
                enemy_spawn_intents: Arc::new(Vec::new()),
                tick_ms: 0.0,
            },
        );
    }

    pub fn observe(
        &mut self,
        before: RunArchiveCheckpoint,
        after: Arc<ArchivedState>,
        loaded: &LoadedBoneyard,
        behind_ms: f64,
    ) -> Option<RunArchive> {
        if before.state.checkpoint.run.phase != GameRunPhase::Active {
            return None;
        }
        let run = self.active_run(Arc::clone(&before.state), loaded);
        remember_living(run, before.clone());
        remember_living(run, RunArchiveCheckpoint { state: Arc::clone(&after), ..before.clone() });
        if before.tick_ms > run.worst_tick.tick_ms {
            run.worst_tick = before.clone();
        }
        run.final_state = Arc::clone(&after);
        let performance = &mut run.performance;
        performance.tick_count += 1;
        performance.total_tick_ms += before.tick_ms;
        performance.over_budget_ticks += u64::from(before.tick_ms > 10.0);
        if performance.recent.last().is_none_or(|window| window.tick_count == 100) {
            performance.recent.push(RunArchivePerformanceWindow {
                tick: before.state.checkpoint.ticks,
                ..Default::default()
            });
            if performance.recent.len() > 60 {
                performance.recent.remove(0);
            }
        }
        let window = performance.recent.last_mut().unwrap();
        let count = window.tick_count + 1;
        window.tick = after.checkpoint.ticks;
        window.mean_tick_ms =
            (window.mean_tick_ms * (count - 1) as f64 + before.tick_ms) / count as f64;
        window.tick_count = count;
        window.maximum_tick_ms = window.maximum_tick_ms.max(before.tick_ms);
        window.maximum_behind_ms = window.maximum_behind_ms.max(behind_ms);
        window.over_budget_ticks += u64::from(before.tick_ms > 10.0);
        if after.checkpoint.run.phase == GameRunPhase::GameOver {
            return self.finish(&loaded.run_id, RunArchiveEndReason::GameOver);
        }
        if after.checkpoint.boneyard.is_none() {
            return self.finish(&loaded.run_id, RunArchiveEndReason::WorldEnded);
        }
        None
    }
    pub fn retain(&mut self, active: &[String]) -> Vec<RunArchive> {
        let removed: Vec<_> = self
            .runs
            .iter()
            .filter(|run| !active.contains(&run.loaded.run_id))
            .map(|run| run.loaded.run_id.clone())
            .collect();
        removed
            .into_iter()
            .filter_map(|id| self.finish(&id, RunArchiveEndReason::WorldEnded))
            .collect()
    }
    pub fn close(&mut self, reason: RunArchiveEndReason) -> Vec<RunArchive> {
        let ids: Vec<_> = self.runs.iter().map(|run| run.loaded.run_id.clone()).collect();
        ids.into_iter().filter_map(|id| self.finish(&id, reason)).collect()
    }
    fn finish(&mut self, id: &str, reason: RunArchiveEndReason) -> Option<RunArchive> {
        let index = self.runs.iter().position(|run| run.loaded.run_id == id)?;
        let run = self.runs.remove(index);
        Some(RunArchive {
            run_id: run.loaded.run_id.clone(),
            session_id: self.session_id.clone(),
            revision: self.revision.clone(),
            started_at_utc: run.started_at_utc,
            ended_at_utc: (self.now)(),
            end_reason: reason,
            content: self.content.clone(),
            loaded_boneyard: run.loaded,
            last_alive: run.last_alive,
            last_alive_by_player: run.last_alive_by_player,
            worst_tick: run.worst_tick,
            final_state: run.final_state,
            performance: run.performance,
        })
    }
}

fn remember_living(run: &mut ActiveRun, checkpoint: RunArchiveCheckpoint) {
    let state = &checkpoint.state.checkpoint;
    if state.run.phase != GameRunPhase::Active {
        return;
    }
    for id in &state.participant_order {
        if state.participants.get(id).expect("participant exists").player.combat.life_state
            != PlayerLifeState::Alive
        {
            continue;
        }
        run.last_alive = Some(checkpoint.clone());
        if let Some((_, retained)) =
            run.last_alive_by_player.iter_mut().find(|(player, _)| player == id)
        {
            *retained = checkpoint.clone();
        } else {
            run.last_alive_by_player.push((id.clone(), checkpoint.clone()));
        }
    }
}

pub fn utc_now() -> String {
    super::super::source_utc_timestamp(time::OffsetDateTime::now_utc())
        .expect("the current UTC date is representable")
}
