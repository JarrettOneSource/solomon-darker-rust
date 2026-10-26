use sha2::{Digest, Sha256};
use solomon_storage::DocumentFile;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone)]
pub(super) struct ContentCache(Arc<PathBuf>);

impl ContentCache {
    pub async fn open() -> Result<Self, String> {
        let directory = crate::storage::data_directory()?.join("content-v1");
        blocking::unblock(move || {
            std::fs::create_dir_all(&directory)
                .map_err(|error| format!("Cannot create the content cache: {error}"))?;
            Ok(Self(Arc::new(directory)))
        })
        .await
    }

    fn path(&self, digest: &str) -> Result<PathBuf, String> {
        if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("The content digest is invalid".into());
        }
        Ok(self.0.join(digest))
    }

    pub async fn read(
        &self,
        _url: &str,
        digest: &str,
        expected: u64,
    ) -> Result<Option<Vec<u8>>, String> {
        let path = self.path(digest)?;
        blocking::unblock(move || {
            let metadata = match path.metadata() {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                Err(error) => return Err(format!("Cannot read cached content: {error}")),
            };
            if metadata.len() != expected {
                remove(&path)?;
                return Ok(None);
            }
            let limit = usize::try_from(expected)
                .map_err(|_| "Cached content exceeds the platform limit")?;
            DocumentFile::new(path, limit).load_bytes()
        })
        .await
    }

    pub async fn remove(&self, _url: &str, digest: &str) -> Result<(), String> {
        let path = self.path(digest)?;
        blocking::unblock(move || remove(&path)).await
    }

    pub async fn write(
        &self,
        _url: &str,
        digest: &str,
        bytes: &[u8],
        _content_type: &str,
    ) -> Result<(), String> {
        let path = self.path(digest)?;
        let bytes = bytes.to_vec();
        blocking::unblock(move || DocumentFile::new(path, bytes.len()).commit_bytes(&bytes)).await
    }

    pub async fn verify(&self, expected: &str, bytes: Vec<u8>) -> Result<Option<Vec<u8>>, String> {
        let expected = expected.to_owned();
        blocking::unblock(move || {
            let hash: String =
                Sha256::digest(&bytes).iter().map(|byte| format!("{byte:02x}")).collect();
            Ok((hash == expected).then_some(bytes))
        })
        .await
    }
}

fn remove(path: &std::path::Path) -> Result<(), String> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("Cannot retire invalid cached content: {error}")),
    }
}
