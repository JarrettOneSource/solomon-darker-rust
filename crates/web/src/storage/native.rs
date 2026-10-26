//! Native save slots and the small preference document. Every read/modify/write
//! holds a separate OS file lock; replacing the data file cannot release the lock.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::PathBuf;

use solomon_storage::DocumentFile;

use super::StoredSave;

const MAX_SLOT_BYTES: usize = solomon_protocol::MAX_SAVE_BYTES * 2 + 64 * 1024;
const MAX_PREFERENCE_BYTES: usize = 32 * 1024 * 1024;

thread_local! {
    static DIRECTORY: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

/// The desktop owner selects its persistent profile before constructing the client.
pub fn configure_native_storage(directory: PathBuf) -> Result<(), String> {
    std::fs::create_dir_all(&directory)
        .map_err(|error| format!("Cannot create {}: {error}", directory.display()))?;
    let directory = directory
        .canonicalize()
        .map_err(|error| format!("Cannot open {}: {error}", directory.display()))?;
    DIRECTORY.with_borrow_mut(|current| *current = Some(directory));
    Ok(())
}

pub(crate) fn data_directory() -> Result<PathBuf, String> {
    DIRECTORY.with_borrow(|directory| {
        directory.clone().ok_or_else(|| "The native save directory is not configured".into())
    })
}

fn file(name: &str, limit: usize) -> Result<DocumentFile, String> {
    DIRECTORY.with_borrow(|directory| {
        let directory = directory.as_ref().ok_or("The native save directory is not configured")?;
        Ok(DocumentFile::new(directory.join(name), limit))
    })
}

#[derive(Clone)]
pub struct LocalSaveStore(DocumentFile);

impl LocalSaveStore {
    pub async fn open(source: bool) -> Result<Self, String> {
        Ok(Self(file(
            if source { "game-save.json" } else { "standalone-save.json" },
            MAX_SLOT_BYTES,
        )?))
    }

    pub async fn read(&self) -> Result<Option<StoredSave>, String> {
        let file = self.0.clone();
        blocking::unblock(move || decode(file.load()?)).await
    }

    pub async fn write(&self, document: String, expected_revision: u64) -> Result<u64, String> {
        let file = self.0.clone();
        blocking::unblock(move || {
            file.transaction(|locked| {
                let current = decode(locked.load()?)?;
                if current.as_ref().map_or(0, |record| record.revision) != expected_revision {
                    return Err(
                        "The saved game changed in another window. Reload to continue that save."
                            .into(),
                    );
                }
                let revision =
                    expected_revision.checked_add(1).ok_or("Saved-game revision overflow")?;
                let text = serde_json::to_string(&StoredSave { revision, document })
                    .map_err(|error| format!("Could not encode the save slot: {error}"))?;
                locked.commit(&text)?;
                Ok(revision)
            })
        })
        .await
    }
}

fn decode(text: Option<String>) -> Result<Option<StoredSave>, String> {
    text.map(|text| {
        serde_json::from_str(&text)
            .map_err(|error| format!("Could not read the save slot: {error}"))
    })
    .transpose()
}

pub struct Preferences {
    values: BTreeMap<String, String>,
    changed: bool,
}

impl Preferences {
    pub fn get(&self, key: &str) -> Result<Option<String>, String> {
        Ok(self.values.get(key).cloned())
    }

    pub fn set(&mut self, key: &str, value: &str) -> Result<(), String> {
        if self.values.get(key).is_none_or(|current| current != value) {
            self.values.insert(key.to_owned(), value.to_owned());
            self.changed = true;
        }
        Ok(())
    }

    pub fn remove(&mut self, key: &str) -> Result<(), String> {
        self.changed |= self.values.remove(key).is_some();
        Ok(())
    }
}

pub fn with_preferences<T>(
    operation: impl FnOnce(&mut Preferences) -> Result<T, String>,
) -> Result<T, String> {
    file("preferences.json", MAX_PREFERENCE_BYTES)?.transaction(|locked| {
        let values = locked
            .load()?
            .map(|text| serde_json::from_str(&text))
            .transpose()
            .map_err(|error| format!("Could not read saved preferences: {error}"))?
            .unwrap_or_default();
        let mut preferences = Preferences { values, changed: false };
        let result = operation(&mut preferences)?;
        if preferences.changed {
            let text = serde_json::to_string(&preferences.values)
                .map_err(|error| format!("Could not encode saved preferences: {error}"))?;
            locked.commit(&text)?;
        }
        Ok(result)
    })
}

pub fn scoped_key(key: &str) -> Result<String, String> {
    Ok(key.to_owned())
}

/// Atomic replacements change this stamp; the layout subscription polls it only
/// to observe edits made by another native window/process.
pub(crate) fn preference_stamp() -> Result<Option<(std::time::SystemTime, u64)>, String> {
    DIRECTORY.with_borrow(|directory| {
        let path = directory
            .as_ref()
            .ok_or("The native save directory is not configured")?
            .join("preferences.json");
        match std::fs::metadata(&path) {
            Ok(metadata) => metadata
                .modified()
                .map(|time| Some((time, metadata.len())))
                .map_err(|error| format!("Cannot observe {}: {error}", path.display())),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(format!("Cannot observe {}: {error}", path.display())),
        }
    })
}

#[cfg(test)]
mod tests;
