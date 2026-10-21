//! Source run archive capture at real admission, world, disconnect and host boundaries.
use super::{HostedGameHost, HostedWorlds};
use crate::host::run_archive::{
    HostRunArchives, RunArchiveEndReason, RunArchiveRecorder, RunArchiveSink,
};
use std::time::Instant;

impl HostedGameHost {
    pub(in crate::host) fn install_run_archives(&mut self, sink: RunArchiveSink, now_ms: f64) {
        if self.editor_test {
            return;
        }
        self.run_archives = Some(HostRunArchives::new(
            RunArchiveRecorder::new(
                self.id.clone(),
                self.recovery_revision.clone(),
                self.content.manifest.clone(),
            ),
            sink,
            now_ms,
        ));
    }
    pub(in crate::host) fn set_archive_deadline(&mut self, deadline: Instant) {
        if let Some(archives) = &mut self.run_archives {
            archives.deadline = Some(deadline);
        }
    }
    pub(in crate::host) fn checkpoint_run_for_player(&mut self, id: &str) {
        if let (Some(archives), Some(state)) =
            (&mut self.run_archives, self.worlds.state_for_player(id))
        {
            archives.checkpoint(state);
        }
    }
    pub(in crate::host) fn retain_run_archives(&mut self, now_ms: f64) {
        let active: Vec<_> = match &self.worlds {
            HostedWorlds::Global(worlds) => {
                worlds.runs.iter().map(|run| run.loaded_boneyard.run_id.clone()).collect()
            }
            HostedWorlds::Private { state, .. } => {
                state.boneyard.iter().map(|world| world.loaded.run_id.clone()).collect()
            }
        };
        if let Some(archives) = &mut self.run_archives {
            archives.retain(&active, now_ms);
        }
    }
    pub(in crate::host) fn close_run_archives(&mut self, reason: RunArchiveEndReason) {
        let Some(mut archives) = self.run_archives.take() else {
            return;
        };
        if reason == RunArchiveEndReason::HostClosed {
            match &self.worlds {
                HostedWorlds::Global(worlds) => {
                    for run in &worlds.runs {
                        archives.checkpoint(&run.state);
                    }
                }
                HostedWorlds::Private { state, .. } => archives.checkpoint(state),
            }
        }
        archives.close(reason);
    }
}
