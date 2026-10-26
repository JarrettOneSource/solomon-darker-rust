//! Native private run diagnostics, following Source recorder/store lifecycles.
//! The gzip payload uses a documented Rust checkpoint table rather than V8 objects.
mod encoding;
mod recorder;
mod store;
#[cfg(test)]
mod tests;

pub use encoding::{ArchiveMetadata, ArchivePlayer, RunArchiveFile, StoredCheckpoint};
pub use recorder::{
    ArchivedState, RunArchive, RunArchiveCheckpoint, RunArchiveEndReason, RunArchivePerformance,
    RunArchivePerformanceWindow, RunArchiveRecorder,
};
pub use store::{
    RunArchiveReceipt, RunArchiveSink, RunArchiveStore, prune_directory, read_run_archive,
    write_archive,
};

use solomon_sim::boneyard::LoadedBoneyard;
use solomon_sim::enemies::EnemySpawnIntent;
use solomon_sim::game_run::GameRunPhase;
use solomon_sim::session::{GameSession, GameSessionInput};
use solomon_sim::shared_worlds::SharedWorldsTickObserver;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Instant;

struct PendingObservation {
    before: RunArchiveCheckpoint,
    loaded: Arc<LoadedBoneyard>,
    started: Instant,
    behind_ms: f64,
}

pub(super) struct HostRunArchives {
    recorder: RunArchiveRecorder,
    sink: RunArchiveSink,
    pending: Option<PendingObservation>,
    pub deadline: Option<Instant>,
    next_retain_ms: f64,
}
impl HostRunArchives {
    pub fn new(recorder: RunArchiveRecorder, sink: RunArchiveSink, now_ms: f64) -> Self {
        Self { recorder, sink, pending: None, deadline: None, next_retain_ms: now_ms + 1000.0 }
    }
    pub fn checkpoint(&mut self, state: &GameSession) {
        if let Some(world) = &state.boneyard {
            self.recorder.checkpoint(state, &world.loaded);
        }
    }
    pub fn before_private_step(
        &mut self,
        state: &GameSession,
        inputs: &BTreeMap<String, GameSessionInput>,
        intents: &[EnemySpawnIntent],
    ) {
        if let Some(world) = &state.boneyard {
            self.before_run_step(state, &world.loaded, inputs, intents);
        }
    }
    pub fn after_private_step(&mut self, state: &GameSession) {
        self.finish_observation(state);
    }
    fn finish_observation(&mut self, state: &GameSession) {
        let Some(mut pending) = self.pending.take() else {
            return;
        };
        pending.before.tick_ms = pending.started.elapsed().as_secs_f64() * 1000.0;
        let after = ArchivedState::capture(state);
        if let Some(archive) =
            self.recorder.observe(pending.before, after, &pending.loaded, pending.behind_ms)
        {
            self.save(archive);
        }
    }
    fn save(&self, archive: RunArchive) {
        if let Err(reason) = self.sink.save(archive) {
            eprintln!("run_archive.failed: {reason}");
        }
    }
    pub fn retain(&mut self, active: &[String], now_ms: f64) {
        if now_ms < self.next_retain_ms {
            return;
        }
        self.next_retain_ms = now_ms + 1000.0;
        for archive in self.recorder.retain(active) {
            self.save(archive);
        }
    }
    pub fn close(&mut self, reason: RunArchiveEndReason) {
        for archive in self.recorder.close(reason) {
            self.save(archive);
        }
    }
}
impl SharedWorldsTickObserver for HostRunArchives {
    fn before_run_step(
        &mut self,
        state: &GameSession,
        loaded: &LoadedBoneyard,
        inputs: &BTreeMap<String, GameSessionInput>,
        intents: &[EnemySpawnIntent],
    ) {
        self.pending = None;
        if state.run.phase != GameRunPhase::Active {
            return;
        }
        let before = RunArchiveCheckpoint::new(
            ArchivedState::capture(state),
            inputs
                .iter()
                .filter(|(id, _)| state.participants.contains_key(id))
                .map(|(id, input)| (id.clone(), *input))
                .collect(),
            intents.to_vec(),
            0.0,
        );
        let loaded = self
            .recorder
            .loaded_for_run(&loaded.run_id)
            .unwrap_or_else(|| Arc::new(loaded.clone()));
        // Retention/copy overhead is outside the kernel timing, like Source's
        // immutable pre-step reference. Wall-time lag still exposes that overhead.
        let started = Instant::now();
        let behind_ms = self.deadline.map_or(0.0, |deadline| {
            started.saturating_duration_since(deadline).as_secs_f64() * 1000.0
        });
        self.pending = Some(PendingObservation { before, loaded, started, behind_ms });
    }
    fn after_run_step(&mut self, state: &GameSession, _: &LoadedBoneyard) {
        self.finish_observation(state);
    }
}
