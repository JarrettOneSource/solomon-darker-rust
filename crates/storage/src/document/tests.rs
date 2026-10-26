use super::*;
use std::fs;
use std::path::PathBuf;

struct Directory(PathBuf);

impl Directory {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let sequence = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let unique =
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        Self(
            std::env::temp_dir()
                .join(format!("solomon-document-{}-{unique}-{sequence}", std::process::id())),
        )
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn rejected_and_failed_replacements_keep_the_committed_document() {
    let directory = Directory::new();
    let file = DocumentFile::new(directory.0.join("save.json"), 32);
    assert_eq!(file.load().unwrap(), None);
    file.commit("complete tick 100").unwrap();
    assert!(file.commit(&"x".repeat(33)).is_err());
    assert_eq!(file.load().unwrap().as_deref(), Some("complete tick 100"));
    let temporary = directory.0.join(format!("save.json.{}.tmp", std::process::id()));
    fs::create_dir(&temporary).unwrap();
    assert!(file.commit("tick 101").is_err());
    assert_eq!(file.load().unwrap().as_deref(), Some("complete tick 100"));
    fs::remove_dir(temporary).unwrap();
    file.commit("complete tick 102").unwrap();
    assert_eq!(file.load().unwrap().as_deref(), Some("complete tick 102"));
    fs::write(directory.0.join("save.json"), "x".repeat(33)).unwrap();
    assert!(file.load().is_err());
}

#[test]
fn independent_writers_lock_the_whole_read_modify_write() {
    let directory = Directory::new();
    let path = directory.0.join("counter");
    DocumentFile::new(path.clone(), 64).commit("0").unwrap();
    let barrier = std::sync::Barrier::new(4);
    std::thread::scope(|scope| {
        for _ in 0..4 {
            let path = &path;
            let barrier = &barrier;
            scope.spawn(move || {
                let file = DocumentFile::new(path.clone(), 64);
                barrier.wait();
                for _ in 0..12 {
                    file.transaction(|locked| {
                        let current: u32 = locked.load()?.unwrap().parse().unwrap();
                        std::thread::yield_now();
                        locked.commit(&(current + 1).to_string())
                    })
                    .unwrap();
                }
            });
        }
    });
    assert_eq!(DocumentFile::new(path, 64).load().unwrap().as_deref(), Some("48"));
}

#[test]
fn binary_content_is_atomic_and_does_not_relax_text_document_validation() {
    let directory = Directory::new();
    let path = directory.0.join("content");
    let file = DocumentFile::new(path.clone(), 4);
    file.commit_bytes(&[0, 255, 128, 1]).unwrap();
    assert_eq!(DocumentFile::new(path, 4).load_bytes().unwrap(), Some(vec![0, 255, 128, 1]));
    assert!(file.load().is_err(), "a text save must still be valid UTF-8");
    assert!(file.commit_bytes(&[0; 5]).is_err());
    assert_eq!(file.load_bytes().unwrap(), Some(vec![0, 255, 128, 1]));
}
