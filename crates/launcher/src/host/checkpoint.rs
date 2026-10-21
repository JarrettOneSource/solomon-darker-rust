//! Durable native host documents. Gameplay encoding is the shared
//! `solomon_assets::save_document` codec; this module only commits its completed document.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

// A host document wraps the escaped complete save plus bounded credential metadata.
const MAX_HOST_DOCUMENT_BYTES: u64 = (solomon_protocol::MAX_SAVE_BYTES * 2 + 64 * 1024) as u64;

pub struct CheckpointFile {
    path: PathBuf,
}

impl CheckpointFile {
    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn load(&self) -> Result<Option<String>, String> {
        let file = match File::open(&self.path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(format!("Cannot open {}: {error}", self.path.display())),
        };
        let mut document = String::new();
        file.take(MAX_HOST_DOCUMENT_BYTES + 1)
            .read_to_string(&mut document)
            .map_err(|error| format!("Cannot read {}: {error}", self.path.display()))?;
        if document.len() as u64 > MAX_HOST_DOCUMENT_BYTES {
            return Err("The native host checkpoint exceeds its supported size".into());
        }
        Ok(Some(document))
    }

    /// A failed write leaves the last committed document in place. The ACK producer must
    /// call this before reporting SaveCommitted; accepted socket delivery is not a save.
    pub fn commit(&self, document: &str) -> Result<(), String> {
        if document.len() as u64 > MAX_HOST_DOCUMENT_BYTES {
            return Err("The native host checkpoint exceeds its supported size".into());
        }
        let directory = self.path.parent().filter(|path| !path.as_os_str().is_empty());
        let directory = directory.unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(directory)
            .map_err(|error| format!("Cannot create {}: {error}", directory.display()))?;
        let name = self.path.file_name().ok_or("The host checkpoint needs a file name")?;
        let mut temporary_name = name.to_os_string();
        temporary_name.push(format!(".{}.tmp", std::process::id()));
        let temporary = directory.join(temporary_name);
        let result = self.commit_temporary(document, &temporary, directory);
        if result.is_err() {
            // Cleanup failure cannot obscure the original write/rename failure. A stale
            // temporary is never read as a checkpoint and is safely truncated next time.
            let _ = fs::remove_file(&temporary);
        }
        result
    }

    fn commit_temporary(
        &self,
        document: &str,
        temporary: &Path,
        directory: &Path,
    ) -> Result<(), String> {
        let mut options = OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            // Resume credentials are private host state, beside the save they authorize.
            options.mode(0o600);
        }
        let mut file = options
            .open(temporary)
            .map_err(|error| format!("Cannot create {}: {error}", temporary.display()))?;
        file.write_all(document.as_bytes())
            .and_then(|()| file.sync_all())
            .map_err(|error| format!("Cannot flush {}: {error}", temporary.display()))?;
        drop(file);
        fs::rename(temporary, &self.path)
            .map_err(|error| format!("Cannot replace {}: {error}", self.path.display()))?;
        // Unix needs the containing directory synced to retain the rename across power
        // loss. Windows' standard rename supplies its platform replacement operation.
        #[cfg(unix)]
        File::open(directory)
            .and_then(|file| file.sync_all())
            .map_err(|error| format!("Cannot flush {}: {error}", directory.display()))?;
        #[cfg(not(unix))]
        let _ = directory;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failed_atomic_write_preserves_the_last_committed_host_document() {
        let directory = std::env::temp_dir().join(format!(
            "solomon-host-checkpoint-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        let path = directory.join("host.json");
        let checkpoint = CheckpointFile::new(path);
        assert!(checkpoint.load().unwrap().is_none());
        checkpoint.commit("completed tick 100").unwrap();
        let temporary = directory.join(format!("host.json.{}.tmp", std::process::id()));
        fs::create_dir(&temporary).unwrap();
        assert!(checkpoint.commit("failed tick 101").is_err());
        assert_eq!(checkpoint.load().unwrap().as_deref(), Some("completed tick 100"));
        fs::remove_dir(&temporary).unwrap();
        checkpoint.commit("completed tick 102").unwrap();
        assert_eq!(checkpoint.load().unwrap().as_deref(), Some("completed tick 102"));
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
        fs::remove_dir_all(directory).unwrap();
    }
}
