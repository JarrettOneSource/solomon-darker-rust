//! The save slot of an offline game: when a write is wanted, at most one write at a time,
//! and the revision each write replaces. A failed read or write stops saving until the
//! next new game, so a damaged slot is never overwritten. Only the browser has this slot;
//! the desktop app saves through its native authority.

use crate::storage::SaveStore;

/// Request an autosave after thirty seconds of active simulation, the host's autosave interval.
const AUTOSAVE_TICKS: u64 = 3000;

/// A save write to run asynchronously: the document, its store and the expected revision.
pub struct PendingSave {
    store: SaveStore,
    revision: u64,
    document: String,
}

impl PendingSave {
    pub async fn write(self) -> Result<u64, String> {
        self.store.write(self.document, self.revision).await
    }
}

/// The offline game's save slot and the state of its writes.
#[derive(Default)]
pub(super) struct LocalSave {
    /// Absent until the browser storage opens, and when it cannot.
    store: Option<SaveStore>,
    /// The stored revision the next write replaces.
    revision: u64,
    /// A write was requested after the last one began.
    wanted: bool,
    /// A write is running; the next one waits for its result.
    writing: bool,
    /// The session tick of the last write or restore, which starts the autosave interval.
    last_saved_tick: u64,
    /// The failure that stopped saving.
    error: Option<String>,
}

impl LocalSave {
    #[cfg(target_arch = "wasm32")]
    pub(super) fn open(&mut self, store: SaveStore) {
        self.store = Some(store);
    }

    pub(super) fn has_store(&self) -> bool {
        self.store.is_some()
    }

    /// Record the revision of the document read at startup.
    #[cfg(target_arch = "wasm32")]
    pub(super) fn set_revision(&mut self, revision: u64) {
        self.revision = revision;
    }

    /// Stop saving after a failed read, restore or write.
    #[cfg(target_arch = "wasm32")]
    pub(super) fn fail(&mut self, error: String) {
        self.error = Some(error);
    }

    /// Ask for a write, or withdraw the request when the game may not save.
    pub(super) fn request(&mut self, allowed: bool) {
        self.wanted = allowed;
    }

    pub(super) fn autosave_due(&self, tick: u64) -> bool {
        tick.saturating_sub(self.last_saved_tick) >= AUTOSAVE_TICKS
    }

    /// A restored game matches its stored document, so the autosave interval starts again.
    #[cfg(target_arch = "wasm32")]
    pub(super) fn restored_at(&mut self, tick: u64) {
        self.last_saved_tick = tick;
    }

    /// A new game may save again after an earlier failure, and autosaves count from its
    /// first tick.
    pub(super) fn start_new_game(&mut self) {
        if self.store.is_some() {
            self.error = None;
        }
        self.last_saved_tick = 0;
    }

    /// The store to write to, when a write is wanted, none is running and saving has not
    /// failed.
    pub(super) fn ready_store(&self) -> Option<SaveStore> {
        if !self.wanted || self.writing || self.error.is_some() {
            return None;
        }
        self.store.clone()
    }

    /// Start writing `document`, encoded from the game at `tick`.
    pub(super) fn begin(&mut self, store: SaveStore, document: String, tick: u64) -> PendingSave {
        self.wanted = false;
        self.writing = true;
        self.last_saved_tick = tick;
        PendingSave { store, revision: self.revision, document }
    }

    /// Record a finished write. Returns the error when it failed and saving has stopped.
    pub(super) fn finish(&mut self, result: Result<u64, String>) -> Option<String> {
        self.writing = false;
        match result {
            Ok(revision) => {
                self.revision = revision;
                None
            }
            Err(error) => {
                self.error = Some(error.clone());
                Some(error)
            }
        }
    }
}
