//! Local browser persistence, following `save/game-save-store.ts`: one IndexedDB slot and
//! an atomic revision check. Preferences stay small and use the browser's local storage.

mod playtime;
pub use playtime::{PlaytimeRecorder, read_total_playtime_ms};

use js_sys::Promise;
use serde::{Deserialize, Serialize};
use solomon_sim::settings::GameSettings;
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use wasm_bindgen_futures::JsFuture;
use web_sys::{Event, IdbDatabase, IdbRequest, IdbTransaction, IdbTransactionMode};

const DATABASE: &str = "solomon-darker-rust-saves";
const SOURCE_DATABASE: &str = "solomon-dark-game-saves";
const STORE: &str = "slots";
const SETTINGS_KEY: &str = "solomon-darker-rust-settings-v1";

#[derive(Clone)]
pub enum SaveStore {
    Local(IdbDatabase),
    Source(IdbDatabase),
    Cloud(crate::hosted::WebsiteApi),
}

#[derive(Serialize, Deserialize)]
pub struct StoredSave {
    pub revision: u64,
    pub document: String,
}

impl SaveStore {
    /// Account admission chooses this store explicitly. A cloud failure never selects
    /// the local slot or acknowledges a write that the Website has not committed.
    #[must_use]
    pub fn cloud(api: crate::hosted::WebsiteApi) -> Self {
        Self::Cloud(api)
    }

    pub async fn open() -> Result<Self, String> {
        Self::open_local(false).await
    }

    /// Current Website and desktop use the existing object record and slot key. Opening
    /// the same database preserves installed saves and revision conflicts across the cutover.
    pub async fn open_source() -> Result<Self, String> {
        Self::open_local(true).await
    }

    async fn open_local(source: bool) -> Result<Self, String> {
        let factory = web_sys::window()
            .ok_or("No browser window")?
            .indexed_db()
            .map_err(describe)?
            .ok_or("This browser does not provide saved-game storage")?;
        let request = factory
            .open_with_u32(&scoped_key(if source { SOURCE_DATABASE } else { DATABASE })?, 1)
            .map_err(describe)?;
        let upgrading = request.clone();
        let upgrade = Closure::<dyn FnMut(Event)>::new(move |_| {
            let result = upgrading
                .result()
                .and_then(|value| value.dyn_into::<IdbDatabase>())
                .and_then(|database| {
                    if source {
                        let options = web_sys::IdbObjectStoreParameters::new();
                        options.set_key_path(&JsValue::from_str("slot"));
                        database.create_object_store_with_optional_parameters(STORE, &options)
                    } else {
                        database.create_object_store(STORE)
                    }
                });
            if let Err(error) = result {
                // Aborting the version-change transaction reports failure through the same
                // request future. A half-created database must never look like a good open.
                web_sys::console::error_1(&error);
                if let Some(transaction) = upgrading.transaction()
                    && let Err(error) = transaction.abort()
                {
                    web_sys::console::error_1(&error);
                }
            }
        });
        request.set_onupgradeneeded(Some(upgrade.as_ref().unchecked_ref()));
        let result = request_result(&request).await;
        request.set_onupgradeneeded(None);
        let database = result?.dyn_into().map_err(|_| "Saved-game database did not open")?;
        Ok(if source { Self::Source(database) } else { Self::Local(database) })
    }

    pub async fn read(&self) -> Result<Option<StoredSave>, String> {
        let database = match self {
            Self::Local(database) | Self::Source(database) => database,
            Self::Cloud(api) => {
                return api
                    .read_save()
                    .await
                    .map(|record| {
                        record.map(|record| StoredSave {
                            revision: record.revision,
                            document: record.document,
                        })
                    })
                    .map_err(|error| error.to_string());
            }
        };
        let transaction = database.transaction_with_str(STORE).map_err(describe)?;
        let store = transaction.object_store(STORE).map_err(describe)?;
        let value = request_result(&store.get(&JsValue::from(0)).map_err(describe)?).await?;
        decode_record(value, matches!(self, Self::Source(_)))
    }

    pub async fn write(&self, document: String, expected_revision: u64) -> Result<u64, String> {
        let database = match self {
            Self::Local(database) | Self::Source(database) => database,
            Self::Cloud(api) => {
                return api
                    .write_save(&document, expected_revision)
                    .await
                    .map(|record| record.revision)
                    .map_err(|error| error.to_string());
            }
        };
        let transaction = database
            .transaction_with_str_and_mode(STORE, IdbTransactionMode::Readwrite)
            .map_err(describe)?;
        let store = transaction.object_store(STORE).map_err(describe)?;
        let current = request_result(&store.get(&JsValue::from(0)).map_err(describe)?).await?;
        let current = decode_record(current, matches!(self, Self::Source(_)))?;
        if current.as_ref().map_or(0, |record| record.revision) != expected_revision {
            transaction.abort().map_err(describe)?;
            return Err(
                "The saved game changed in another tab. Reload to continue that save.".into()
            );
        }
        let revision = expected_revision.checked_add(1).ok_or("Saved-game revision overflow")?;
        if matches!(self, Self::Source(_)) {
            let record =
                serde_json::json!({ "slot": 0, "revision": revision, "document": document });
            store
                .put(&js_sys::JSON::parse(&record.to_string()).map_err(describe)?)
                .map_err(describe)?;
        } else {
            let record = serde_json::to_string(&StoredSave { revision, document })
                .map_err(|error| format!("Could not encode the save slot: {error}"))?;
            store.put_with_key(&JsValue::from(record), &JsValue::from(0)).map_err(describe)?;
        }
        transaction_done(&transaction).await?;
        Ok(revision)
    }
}

fn decode_record(value: JsValue, source: bool) -> Result<Option<StoredSave>, String> {
    if value.is_undefined() {
        return Ok(None);
    }
    let text = if source {
        js_sys::JSON::stringify(&value).map_err(describe)?.as_string()
    } else {
        value.as_string()
    }
    .ok_or("The saved-game slot has an invalid record")?;
    serde_json::from_str(&text)
        .map(Some)
        .map_err(|error| format!("Could not read the save slot: {error}"))
}

pub(crate) async fn request_result(request: &IdbRequest) -> Result<JsValue, String> {
    let mut success = None;
    let mut failure = None;
    let promise = Promise::new(&mut |resolve, reject| {
        let completed = request.clone();
        let success_reject = reject.clone();
        let on_success = Closure::<dyn FnMut(Event)>::new(move |_| match completed.result() {
            Ok(value) => {
                let _ = resolve.call1(&JsValue::UNDEFINED, &value);
            }
            Err(error) => {
                let _ = success_reject.call1(&JsValue::UNDEFINED, &error);
            }
        });
        let failed = request.clone();
        let on_error = Closure::<dyn FnMut(Event)>::new(move |_| {
            let error = match failed.error() {
                Ok(Some(error)) => JsValue::from(error),
                Ok(None) => JsValue::from("Saved-game request failed"),
                Err(error) => error,
            };
            let _ = reject.call1(&JsValue::UNDEFINED, &error);
        });
        request.set_onsuccess(Some(on_success.as_ref().unchecked_ref()));
        request.set_onerror(Some(on_error.as_ref().unchecked_ref()));
        success = Some(on_success);
        failure = Some(on_error);
    });
    let result = JsFuture::from(promise).await.map_err(describe);
    request.set_onsuccess(None);
    request.set_onerror(None);
    drop((success, failure));
    result
}

pub(crate) async fn transaction_done(transaction: &IdbTransaction) -> Result<(), String> {
    let mut complete = None;
    let mut failure = None;
    let promise = Promise::new(&mut |resolve, reject| {
        let on_complete = Closure::<dyn FnMut(Event)>::new(move |_| {
            let _ = resolve.call0(&JsValue::UNDEFINED);
        });
        let failed = transaction.clone();
        let on_failure = Closure::<dyn FnMut(Event)>::new(move |_| {
            let error = failed
                .error()
                .map_or_else(|| JsValue::from("Saved-game transaction was aborted"), JsValue::from);
            let _ = reject.call1(&JsValue::UNDEFINED, &error);
        });
        transaction.set_oncomplete(Some(on_complete.as_ref().unchecked_ref()));
        transaction.set_onabort(Some(on_failure.as_ref().unchecked_ref()));
        transaction.set_onerror(Some(on_failure.as_ref().unchecked_ref()));
        complete = Some(on_complete);
        failure = Some(on_failure);
    });
    let result = JsFuture::from(promise).await.map(|_| ()).map_err(describe);
    transaction.set_oncomplete(None);
    transaction.set_onabort(None);
    transaction.set_onerror(None);
    drop((complete, failure));
    result
}

pub fn load_settings() -> Result<GameSettings, String> {
    let storage = local_storage()?;
    if let Some(text) = storage
        .get_item(&scoped_key(solomon_assets::browser_settings::STORAGE_KEY)?)
        .map_err(describe)?
    {
        return Ok(solomon_assets::browser_settings::parse(&text));
    }
    // The earlier Rust standalone used a different key and snake_case fields.
    // Read it only when the current Website document has never been written.
    let Some(text) = storage.get_item(&scoped_key(SETTINGS_KEY)?).map_err(describe)? else {
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
    let text = solomon_assets::browser_settings::serialize(settings)?;
    local_storage()?
        .set_item(&scoped_key(solomon_assets::browser_settings::STORAGE_KEY)?, &text)
        .map_err(describe)
}

pub fn tutorial_declined() -> Result<bool, String> {
    Ok(local_storage()?
        .get_item(&scoped_key("solomon-darker-rust-tutorial-declined")?)
        .map_err(describe)?
        .as_deref()
        == Some("true"))
}

pub fn save_tutorial_declined() -> Result<(), String> {
    local_storage()?
        .set_item(&scoped_key("solomon-darker-rust-tutorial-declined")?, "true")
        .map_err(describe)
}

const EARLIER_RUST_HALL_KEY: &str = "solomon-darker-rust-hall-v1";

pub fn read_hall() -> Result<Vec<solomon_sim::hall_of_fame::HallOfFameEntry>, String> {
    use solomon_assets::browser_hall;
    let storage = local_storage()?;
    let current = storage.get_item(&scoped_key(browser_hall::STORAGE_KEY)?).map_err(describe)?;
    let legacy =
        storage.get_item(&scoped_key(browser_hall::LEGACY_STORAGE_KEY)?).map_err(describe)?;
    if current.is_some() || legacy.is_some() {
        return Ok(browser_hall::read(current.as_deref(), legacy.as_deref()));
    }
    let Some(text) = storage.get_item(&scoped_key(EARLIER_RUST_HALL_KEY)?).map_err(describe)?
    else {
        return Ok(Vec::new());
    };
    let entries: Vec<solomon_sim::hall_of_fame::HallOfFameEntry> = serde_json::from_str(&text)
        .map_err(|error| format!("Could not read the Hall of Fame: {error}"))?;
    // Preserve the earlier standalone's records through the current admission rules.
    Ok(browser_hall::read(Some(&browser_hall::serialize(&entries)?), None))
}

pub fn record_hall(
    entry: solomon_sim::hall_of_fame::HallOfFameEntry,
) -> Result<Vec<solomon_sim::hall_of_fame::HallOfFameEntry>, String> {
    use solomon_assets::browser_hall;
    let current = browser_hall::serialize(&read_hall()?)?;
    let entries = browser_hall::record(entry, Some(&current), None);
    local_storage()?
        .set_item(&scoped_key(browser_hall::STORAGE_KEY)?, &browser_hall::serialize(&entries)?)
        .map_err(describe)?;
    Ok(entries)
}

fn local_storage() -> Result<web_sys::Storage, String> {
    web_sys::window()
        .ok_or("No browser window")?
        .local_storage()
        .map_err(describe)?
        .ok_or_else(|| "This browser does not provide preference storage".into())
}

pub fn scoped_key(key: &str) -> Result<String, String> {
    let window = web_sys::window().ok_or("No browser window")?;
    Ok(crate::hosted::WebsiteMount::configured(&window)?.unwrap_or_default().storage_key(key))
}

fn describe(error: JsValue) -> String {
    if let Some(exception) = error.dyn_ref::<web_sys::DomException>() {
        return exception.message();
    }
    error.as_string().unwrap_or_else(|| format!("Saved-game storage failed: {error:?}"))
}
