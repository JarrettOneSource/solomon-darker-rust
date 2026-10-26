//! Durable host checkpoints use the same atomic document owner as native client saves.
//! Gameplay encoding remains in `solomon_assets::save_document`.

use solomon_storage::DocumentFile;
use std::path::PathBuf;

// A host document wraps the escaped complete save plus bounded credential metadata.
const MAX_HOST_DOCUMENT_BYTES: usize = solomon_protocol::MAX_SAVE_BYTES * 2 + 64 * 1024;

pub struct CheckpointFile(DocumentFile);

impl CheckpointFile {
    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        Self(DocumentFile::new(path, MAX_HOST_DOCUMENT_BYTES))
    }

    pub fn load(&self) -> Result<Option<String>, String> {
        self.0.load()
    }

    pub fn commit(&self, document: &str) -> Result<(), String> {
        self.0.commit(document)
    }
}
