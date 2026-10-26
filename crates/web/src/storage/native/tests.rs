use super::*;
use futures_lite::future::block_on;

struct Directory(PathBuf);

impl Directory {
    fn new() -> Self {
        let unique =
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let directory = Self(
            std::env::temp_dir()
                .join(format!("solomon-native-save-{}-{unique}", std::process::id())),
        );
        configure_native_storage(directory.0.clone()).unwrap();
        directory
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn reopening_preserves_the_whole_schema51_document_and_rejects_stale_writers() {
    let _directory = Directory::new();
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../sim/tests/fixtures/website/current-save-profile-goldens.json"
    ))
    .unwrap();
    let document = fixture["contracts"][0]["document"].to_string();
    assert_eq!(solomon_assets::website_save::parse(&document).unwrap().source_schema_version, 51);
    block_on(async {
        let first = LocalSaveStore::open(true).await.unwrap();
        assert!(first.read().await.unwrap().is_none());
        assert_eq!(first.write(document.clone(), 0).await.unwrap(), 1);
        drop(first);
        let second = LocalSaveStore::open(true).await.unwrap();
        let reopened = second.read().await.unwrap().unwrap();
        assert_eq!(reopened.revision, 1);
        assert_eq!(reopened.document, document);
        let competing = LocalSaveStore::open(true).await.unwrap();
        let (left, right) = futures_util::future::join(
            second.write(document.clone(), 1),
            competing.write(document.clone(), 1),
        )
        .await;
        assert_eq!(usize::from(left.is_ok()) + usize::from(right.is_ok()), 1);
        let error = left.err().or_else(|| right.err()).unwrap();
        assert!(error.contains("changed in another window"), "{error}");
        assert_eq!(second.read().await.unwrap(), Some(StoredSave { revision: 2, document }));
        // The earlier static Rust preview owns a separate slot.
        assert!(LocalSaveStore::open(false).await.unwrap().read().await.unwrap().is_none());
    });
}

#[test]
fn malformed_existing_saves_fail_without_being_replaced() {
    let directory = Directory::new();
    let path = directory.0.join("game-save.json");
    std::fs::write(&path, "interrupted external write").unwrap();
    block_on(async {
        let store = LocalSaveStore::open(true).await.unwrap();
        assert!(store.read().await.is_err());
        assert!(store.write("new wizard".into(), 0).await.is_err());
    });
    assert_eq!(std::fs::read_to_string(path).unwrap(), "interrupted external write");
}

#[test]
fn settings_and_preference_transactions_survive_reopening_and_abort_together() {
    let directory = Directory::new();
    let settings = solomon_sim::settings::GameSettings {
        music_volume_percent: 23,
        ui_scale_percent: 125,
        ..Default::default()
    };
    super::super::save_settings(&settings).unwrap();
    super::super::save_tutorial_declined().unwrap();
    with_preferences(|preferences| preferences.set("sdr.game.playtime.v1", "42000")).unwrap();
    let aborted: Result<(), String> = with_preferences(|preferences| {
        preferences.set("sdr.game.playtime.v1", "99999")?;
        Err("cancelled preference batch".into())
    });
    assert!(aborted.is_err());
    configure_native_storage(directory.0.clone()).unwrap();
    let loaded = super::super::load_settings().unwrap();
    assert_eq!(loaded.music_volume_percent, 23);
    assert_eq!(loaded.ui_scale_percent, 125);
    assert!(super::super::tutorial_declined().unwrap());
    assert_eq!(super::super::read_total_playtime_ms().unwrap(), 42000);
    // Unrecognized values are preserved for their owning feature and later versions.
    with_preferences(|preferences| preferences.set("future-pref", "retained")).unwrap();
    super::super::save_settings(&Default::default()).unwrap();
    assert_eq!(
        with_preferences(|preferences| preferences.get("future-pref")).unwrap().as_deref(),
        Some("retained")
    );
}
