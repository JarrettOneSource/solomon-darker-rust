//! Shared Hub Memorial durability from `host/game-memorial-persistence.ts`.
//! The existing checkpoint file owns atomic fsync/rename; this owner adds the
//! Memorial's smaller external bound and exact Source document validation.

use std::fs::{self, File};
use std::io::Read;
use std::path::PathBuf;

use solomon_assets::memorial::{
    MAX_GAME_MEMORIAL_BYTES, MEMORIAL_SIZE_ERROR, decode_memorial_document,
    encode_memorial_document,
};
use solomon_sim::hub::memorial::{HubMemorialState, create_hub_memorial_state};

use super::checkpoint::CheckpointFile;

pub(super) struct MemorialPersistence {
    file: CheckpointFile,
}

impl MemorialPersistence {
    pub fn open(path: PathBuf) -> Result<(Self, HubMemorialState), String> {
        if !path.is_absolute() {
            return Err("Game memorial path must be absolute".into());
        }
        let directory = path.parent().ok_or("Game memorial path needs a file name")?;
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o750);
        }
        builder
            .create(directory)
            .map_err(|error| format!("Cannot create {}: {error}", directory.display()))?;
        let initial = match File::open(&path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                create_hub_memorial_state()
            }
            Err(error) => return Err(format!("Cannot open {}: {error}", path.display())),
            Ok(file) => {
                if file.metadata().map_err(|error| error.to_string())?.len()
                    > MAX_GAME_MEMORIAL_BYTES as u64
                {
                    return Err(MEMORIAL_SIZE_ERROR.into());
                }
                let mut bytes = Vec::new();
                file.take(MAX_GAME_MEMORIAL_BYTES as u64 + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|error| format!("Cannot read {}: {error}", path.display()))?;
                decode_memorial_document(&String::from_utf8_lossy(&bytes))?
            }
        };
        Ok((Self { file: CheckpointFile::new(path) }, initial))
    }

    pub fn persist(&self, state: &HubMemorialState) -> Result<(), String> {
        let mut document = encode_memorial_document(state)?;
        document.push('\n');
        self.file.commit(&document)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shared_memorial_reopens_exact_slots_and_rejects_oversized_or_relative_files() {
        assert_eq!(
            MemorialPersistence::open("relative.json".into()).err().unwrap(),
            "Game memorial path must be absolute"
        );
        let directory = std::env::temp_dir().join(format!(
            "solomon-memorial-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        let path = directory.join("memorial.json");
        let (file, initial) = MemorialPersistence::open(path.clone()).unwrap();
        assert_eq!(initial, create_hub_memorial_state());
        assert!(!path.exists(), "opening a missing Memorial must not invent a durable update");
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../sim/tests/fixtures/website/memorial-persistence-goldens.json"
        ))
        .unwrap();
        let case = fixture["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|case| case["name"] == "retained-ten")
            .unwrap();
        let retained = decode_memorial_document(&case["document"].to_string()).unwrap();
        file.persist(&retained).unwrap();
        assert_eq!(MemorialPersistence::open(path.clone()).unwrap().1, retained);
        assert!(fs::read_to_string(&path).unwrap().ends_with('\n'));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
            assert_eq!(fs::metadata(&directory).unwrap().permissions().mode() & 0o777, 0o750);
        }
        fs::write(&path, vec![b' '; MAX_GAME_MEMORIAL_BYTES + 1]).unwrap();
        assert_eq!(MemorialPersistence::open(path).err().unwrap(), MEMORIAL_SIZE_ERROR);
        fs::remove_dir_all(directory).unwrap();
    }
}
