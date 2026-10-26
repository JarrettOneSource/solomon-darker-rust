use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct DocumentFile {
    path: PathBuf,
    max_bytes: usize,
}

/// Holds the separate lock file across the caller's entire revision check and commit.
/// The lock file is retained: unlinking it could admit a second inode and writer.
pub struct LockedDocument<'a> {
    document: &'a DocumentFile,
    _lock: File,
}

impl DocumentFile {
    #[must_use]
    pub fn new(path: PathBuf, max_bytes: usize) -> Self {
        Self { path, max_bytes }
    }

    pub fn load(&self) -> Result<Option<String>, String> {
        self.load_bytes()?
            .map(|bytes| {
                String::from_utf8(bytes)
                    .map_err(|error| format!("Cannot read {}: {error}", self.path.display()))
            })
            .transpose()
    }

    pub fn load_bytes(&self) -> Result<Option<Vec<u8>>, String> {
        let file = match File::open(&self.path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(format!("Cannot open {}: {error}", self.path.display())),
        };
        let mut document = Vec::new();
        file.take(self.max_bytes as u64 + 1)
            .read_to_end(&mut document)
            .map_err(|error| format!("Cannot read {}: {error}", self.path.display()))?;
        if document.len() > self.max_bytes {
            return Err("The saved document exceeds its supported size".into());
        }
        Ok(Some(document))
    }

    pub fn commit(&self, document: &str) -> Result<(), String> {
        self.commit_bytes(document.as_bytes())
    }

    pub fn commit_bytes(&self, document: &[u8]) -> Result<(), String> {
        self.transaction(|locked| locked.commit_bytes(document))
    }

    pub fn transaction<T>(
        &self,
        operation: impl FnOnce(&LockedDocument<'_>) -> Result<T, String>,
    ) -> Result<T, String> {
        let lock = self.open_lock()?;
        lock.lock().map_err(|error| format!("Cannot lock {}: {error}", self.path.display()))?;
        operation(&LockedDocument { document: self, _lock: lock })
    }

    /// A busy document returns None without invoking the operation. Callers that
    /// own cancellable work can wait without leaving a thread inside an OS lock.
    pub fn try_transaction<T>(
        &self,
        operation: impl FnOnce(&LockedDocument<'_>) -> Result<T, String>,
    ) -> Result<Option<T>, String> {
        let lock = self.open_lock()?;
        match lock.try_lock() {
            Ok(()) => operation(&LockedDocument { document: self, _lock: lock }).map(Some),
            Err(TryLockError::WouldBlock) => Ok(None),
            Err(TryLockError::Error(error)) => {
                Err(format!("Cannot lock {}: {error}", self.path.display()))
            }
        }
    }

    fn open_lock(&self) -> Result<File, String> {
        let directory = self.directory();
        fs::create_dir_all(directory)
            .map_err(|error| format!("Cannot create {}: {error}", directory.display()))?;
        let mut lock_name = self.path.as_os_str().to_owned();
        lock_name.push(".lock");
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        options
            .open(Path::new(&lock_name))
            .map_err(|error| format!("Cannot open the save lock: {error}"))
    }

    fn directory(&self) -> &Path {
        self.path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."))
    }

    fn commit_locked(&self, document: &[u8]) -> Result<(), String> {
        if document.len() > self.max_bytes {
            return Err("The saved document exceeds its supported size".into());
        }
        let directory = self.directory();
        let name = self.path.file_name().ok_or("The saved document needs a file name")?;
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
        document: &[u8],
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
        file.write_all(document)
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

impl LockedDocument<'_> {
    pub fn load(&self) -> Result<Option<String>, String> {
        self.document.load()
    }

    /// Return success only after flushing the file and its atomic replacement.
    pub fn commit(&self, document: &str) -> Result<(), String> {
        self.commit_bytes(document.as_bytes())
    }

    pub fn commit_bytes(&self, document: &[u8]) -> Result<(), String> {
        self.document.commit_locked(document)
    }
}

#[cfg(test)]
mod tests;
