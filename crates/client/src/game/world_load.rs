//! The Boneyard world waiting to be installed. Loading runs outside the frame loop: a
//! started load queues the loaded world, the task runner takes it with its generation,
//! prepares its resources and installs it only while that generation is still current.
//! Starting another load or cancelling bumps the generation, so a late preparation is
//! dropped.

use solomon_sim::boneyard::LoadedBoneyard;

/// A queued or in-progress Boneyard load.
#[derive(Default)]
pub(super) struct WorldLoad {
    /// A loaded world waiting for the task runner to prepare it.
    queued: Option<LoadedBoneyard>,
    /// A world is queued or being prepared.
    loading: bool,
    /// Bumped by every start and cancellation; a preparation installs only while its
    /// generation is current.
    generation: u64,
}

impl WorldLoad {
    pub(super) fn is_loading(&self) -> bool {
        self.loading
    }

    /// Queue `world` for preparation, replacing any load in progress.
    pub(super) fn start(&mut self, world: LoadedBoneyard) {
        self.generation += 1;
        self.queued = Some(world);
        self.loading = true;
    }

    /// Drop any queued or in-progress load.
    pub(super) fn cancel(&mut self) {
        self.generation += 1;
        self.queued = None;
        self.loading = false;
    }

    /// The queued world and its generation, for the task runner.
    pub(super) fn take_queued(&mut self) -> Option<(u64, LoadedBoneyard)> {
        self.queued.take().map(|world| (self.generation, world))
    }

    pub(super) fn is_current(&self, generation: u64) -> bool {
        self.generation == generation
    }

    /// The world was installed, or its preparation failed.
    pub(super) fn finish(&mut self) {
        self.loading = false;
    }
}
