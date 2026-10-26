//! One revision-checked save contract over IndexedDB, native files, or the Website.
//! Preferences retain the Website formats and legacy migration rules.

#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg(not(target_arch = "wasm32"))]
mod native;
mod playtime;
pub use playtime::{PlaytimeRecorder, read_total_playtime_ms};

#[cfg(target_arch = "wasm32")]
use browser::{LocalSaveStore, Preferences};
#[cfg(target_arch = "wasm32")]
pub(crate) use browser::{request_result, transaction_done};
#[cfg(target_arch = "wasm32")]
pub(crate) use browser::{scoped_key, with_preferences};
#[cfg(not(target_arch = "wasm32"))]
pub use native::configure_native_storage;
#[cfg(not(target_arch = "wasm32"))]
use native::{LocalSaveStore, Preferences};
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use native::{data_directory, preference_stamp, scoped_key, with_preferences};

use serde::{Deserialize, Serialize};
use solomon_sim::settings::GameSettings;

#[derive(Clone)]
pub enum SaveStore {
    Local(LocalSaveStore),
    Cloud(crate::hosted::WebsiteApi),
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredSave {
    pub revision: u64,
    pub document: String,
}

impl SaveStore {
    /// A cloud failure never selects the local slot or acknowledges an uncommitted write.
    #[must_use]
    pub fn cloud(api: crate::hosted::WebsiteApi) -> Self {
        Self::Cloud(api)
    }

    #[cfg(target_arch = "wasm32")]
    pub async fn open() -> Result<Self, String> {
        LocalSaveStore::open(false).await.map(Self::Local)
    }

    pub async fn open_source() -> Result<Self, String> {
        LocalSaveStore::open(true).await.map(Self::Local)
    }

    pub async fn read(&self) -> Result<Option<StoredSave>, String> {
        match self {
            Self::Local(store) => store.read().await,
            Self::Cloud(api) => api
                .read_save()
                .await
                .map(|record| {
                    record.map(|record| StoredSave {
                        revision: record.revision,
                        document: record.document,
                    })
                })
                .map_err(|error| error.to_string()),
        }
    }

    pub async fn write(&self, document: String, expected_revision: u64) -> Result<u64, String> {
        match self {
            Self::Local(store) => store.write(document, expected_revision).await,
            Self::Cloud(api) => api
                .write_save(&document, expected_revision)
                .await
                .map(|record| record.revision)
                .map_err(|error| error.to_string()),
        }
    }
}

pub(crate) fn read_preference(key: &str) -> Result<Option<String>, String> {
    with_preferences(|storage| storage.get(key))
}

pub(crate) fn write_preference(key: &str, value: Option<&str>) -> Result<(), String> {
    with_preferences(|storage| match value {
        Some(value) => storage.set(key, value),
        None => storage.remove(key),
    })
}

pub fn load_settings() -> Result<GameSettings, String> {
    if let Some(text) =
        read_preference(&scoped_key(solomon_assets::browser_settings::STORAGE_KEY)?)?
    {
        return Ok(solomon_assets::browser_settings::parse(&text));
    }
    // The earlier Rust standalone used a different key and snake_case fields.
    let Some(text) = read_preference(&scoped_key("solomon-darker-rust-settings-v1")?)? else {
        return Ok(GameSettings::default());
    };
    let mut settings: GameSettings = serde_json::from_str(&text)
        .map_err(|error| format!("Could not read saved preferences: {error}"))?;
    if !settings.controls.migrate_missing_bindings() {
        return Ok(GameSettings::default());
    }
    if !(75..=125).contains(&settings.camera_fov_percent)
        || !(75..=150).contains(&settings.ui_scale_percent)
        || !(24..=100).contains(&settings.light_quality_percent)
        || settings.music_volume_percent > 100
        || settings.sound_volume_percent > 100
    {
        return Err("Saved preferences contain a value outside its supported range".into());
    }
    Ok(settings)
}

pub fn save_settings(settings: &GameSettings) -> Result<(), String> {
    write_preference(
        &scoped_key(solomon_assets::browser_settings::STORAGE_KEY)?,
        Some(&solomon_assets::browser_settings::serialize(settings)?),
    )
}

pub fn tutorial_declined() -> Result<bool, String> {
    Ok(read_preference(&scoped_key("solomon-darker-rust-tutorial-declined")?)?.as_deref()
        == Some("true"))
}

pub fn save_tutorial_declined() -> Result<(), String> {
    write_preference(&scoped_key("solomon-darker-rust-tutorial-declined")?, Some("true"))
}

fn hall_from_preferences(
    storage: &Preferences,
) -> Result<Vec<solomon_sim::hall_of_fame::HallOfFameEntry>, String> {
    use solomon_assets::browser_hall;
    let current = storage.get(&scoped_key(browser_hall::STORAGE_KEY)?)?;
    let legacy = storage.get(&scoped_key(browser_hall::LEGACY_STORAGE_KEY)?)?;
    if current.is_some() || legacy.is_some() {
        return Ok(browser_hall::read(current.as_deref(), legacy.as_deref()));
    }
    let Some(text) = storage.get(&scoped_key("solomon-darker-rust-hall-v1")?)? else {
        return Ok(Vec::new());
    };
    let entries: Vec<solomon_sim::hall_of_fame::HallOfFameEntry> = serde_json::from_str(&text)
        .map_err(|error| format!("Could not read the Hall of Fame: {error}"))?;
    Ok(browser_hall::read(Some(&browser_hall::serialize(&entries)?), None))
}

pub fn read_hall() -> Result<Vec<solomon_sim::hall_of_fame::HallOfFameEntry>, String> {
    with_preferences(|storage| hall_from_preferences(storage))
}

pub fn record_hall(
    entry: solomon_sim::hall_of_fame::HallOfFameEntry,
) -> Result<Vec<solomon_sim::hall_of_fame::HallOfFameEntry>, String> {
    use solomon_assets::browser_hall;
    with_preferences(|storage| {
        let current = browser_hall::serialize(&hall_from_preferences(storage)?)?;
        let entries = browser_hall::record(entry, Some(&current), None);
        storage
            .set(&scoped_key(browser_hall::STORAGE_KEY)?, &browser_hall::serialize(&entries)?)?;
        Ok(entries)
    })
}
