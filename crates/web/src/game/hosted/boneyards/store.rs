//! The unchanged editor's global local-draft drawer and one-use IndexedDB handoff.
//! Source: `editor/store.ts`, `cloud.ts`, `workshop-request-store.ts`.
use crate::hosted::WebsiteMount;
use crate::storage::{request_result, transaction_done};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use web_sys::{Event, IdbDatabase, IdbTransactionMode, Window};

const INDEX: &str = "sdr:boneyard:drafts";
const DRAFT: &str = "sdr:boneyard:draft:";
const CLOUD_MAP: &str = "sdr:boneyard:cloudmap";
const WORKSHOP_DATABASE: &str = "solomon-dark-boneyard-workshop";
const WORKSHOP_STORE: &str = "requests";
const WORKSHOP_TOKEN: &str = "sdr:boneyard:workshop:";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DraftMeta {
    pub id: String,
    pub name: String,
    pub updated_at: f64,
    pub residents: usize,
}

fn storage(window: &Window) -> Result<web_sys::Storage, String> {
    window
        .local_storage()
        .map_err(|_| "This browser blocked local boneyards")?
        .ok_or_else(|| "This browser has no local draft storage".to_owned())
}
pub(super) fn list(window: &Window) -> Vec<DraftMeta> {
    let read = || -> Result<Vec<DraftMeta>, String> {
        let Some(raw) =
            storage(window)?.get_item(INDEX).map_err(|_| "Draft index could not be read")?
        else {
            return Ok(Vec::new());
        };
        serde_json::from_str(&raw).map_err(|_| "Draft index is not valid".to_owned())
    };
    // Source treats unavailable or malformed local indices as an empty device drawer.
    let mut entries = read().unwrap_or_default();
    entries.sort_by(|a, b| b.updated_at.total_cmp(&a.updated_at));
    entries
}
pub(super) fn load(window: &Window, id: &str) -> Result<Value, String> {
    let raw = storage(window)?
        .get_item(&format!("{DRAFT}{id}"))
        .map_err(|_| "Draft could not be read")?
        .ok_or("Draft is not present on this device")?;
    let value: Value =
        serde_json::from_str(&raw).map_err(|_| "Not JSON. The Archivist has standards.")?;
    validate_file(&value)?;
    Ok(value)
}
pub(super) fn save(window: &Window, id: &str, doc: Value) -> Result<DraftMeta, String> {
    let file = json!({"format":"sdr-boneyard-doc","version":1,"doc":doc});
    validate_file(&file)?;
    let doc = &file["doc"];
    let meta = DraftMeta {
        id: id.to_owned(),
        name: doc["meta"]["name"].as_str().ok_or("Boneyard name is not text")?.to_owned(),
        updated_at: js_sys::Date::now(),
        residents: ["objects", "sprites", "roads", "fences", "terrain"]
            .into_iter()
            .map(|key| doc[key].as_array().expect("validated editor array").len())
            .sum(),
    };
    let mut index = list(window);
    index.retain(|entry| entry.id != id);
    index.insert(0, meta.clone());
    index.truncate(40);
    let storage = storage(window)?;
    storage
        .set_item(&format!("{DRAFT}{id}"), &file.to_string())
        .map_err(|_| "Draft storage is full or unavailable")?;
    storage
        .set_item(
            INDEX,
            &serde_json::to_string(&index).map_err(|_| "Draft index could not be encoded")?,
        )
        .map_err(|_| "Draft index could not be saved")?;
    Ok(meta)
}
pub(super) fn delete(window: &Window, id: &str) -> Result<(), String> {
    let mut entries = list(window);
    entries.retain(|entry| entry.id != id);
    let storage = storage(window)?;
    storage
        .set_item(
            INDEX,
            &serde_json::to_string(&entries).map_err(|_| "Draft index could not be encoded")?,
        )
        .map_err(|_| "Draft index could not be saved")?;
    storage
        .remove_item(&format!("{DRAFT}{id}"))
        .map_err(|_| "Draft could not be removed".to_owned())
}
pub(super) fn validate_file(file: &Value) -> Result<(), String> {
    if file["format"].as_str() != Some("sdr-boneyard-doc")
        || file.get("doc").is_none_or(Value::is_null)
    {
        return Err("Not a boneyard draft. Wrong drawer entirely.".to_owned());
    }
    let doc = &file["doc"];
    for key in ["objects", "sprites", "roads", "fences", "terrain"] {
        if !doc[key].is_array() {
            return Err(format!("Boneyard {key} must be an array."));
        }
    }
    if !doc["meta"]["name"].is_string() {
        return Err("Boneyard name is not text.".to_owned());
    }
    Ok(())
}
pub(super) fn new_id() -> Result<String, String> {
    let time = js_sys::Number::from(js_sys::Date::now())
        .to_string_with_radix(36)
        .map_err(|_| "Draft time could not be encoded")?;
    let random = js_sys::Number::from((js_sys::Math::random() * 1296.0).floor())
        .to_string_with_radix(36)
        .map_err(|_| "Draft identity could not be encoded")?;
    Ok(format!("d{time}{random}"))
}
pub(super) fn set_cloud_id(
    window: &Window,
    draft_id: &str,
    cloud_id: u64,
    pending: &mut Option<Value>,
) {
    let mut map = pending.take().unwrap_or_else(|| {
        storage(window)
            .and_then(|storage| {
                storage.get_item(CLOUD_MAP).map_err(|_| "Cloud draft map unavailable".to_owned())
            })
            .ok()
            .flatten()
            .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
            .filter(Value::is_object)
            .unwrap_or_else(|| json!({}))
    });
    map[draft_id] = cloud_id.into();
    // Source retains this mapping in memory when local quota is full; cloud opening still works.
    if storage(window)
        .and_then(|storage| {
            storage
                .set_item(CLOUD_MAP, &map.to_string())
                .map_err(|_| "Cloud draft mapping could not be saved".to_owned())
        })
        .is_err()
    {
        *pending = Some(map);
    }
}

async fn open_workshop_database(
    window: &Window,
    mount: &WebsiteMount,
) -> Result<IdbDatabase, String> {
    let factory = window
        .indexed_db()
        .map_err(|_| "Workshop storage is blocked")?
        .ok_or("Workshop storage is unavailable")?;
    let request = factory
        .open_with_u32(&mount.storage_key(WORKSHOP_DATABASE), 1)
        .map_err(|_| "Workshop storage could not be opened")?;
    let upgrading = request.clone();
    let upgrade = Closure::<dyn FnMut(Event)>::new(move |_| {
        let result = upgrading
            .result()
            .and_then(|value| value.dyn_into::<IdbDatabase>())
            .and_then(|database| database.create_object_store(WORKSHOP_STORE));
        if let Err(error) = result {
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
    result?.dyn_into().map_err(|_| "Workshop storage did not open".to_owned())
}
async fn write_workshop(
    database: &IdbDatabase,
    id: &str,
    value: Option<&Value>,
) -> Result<(), String> {
    let transaction = database
        .transaction_with_str_and_mode(WORKSHOP_STORE, IdbTransactionMode::Readwrite)
        .map_err(|_| "Workshop transaction could not start")?;
    let store =
        transaction.object_store(WORKSHOP_STORE).map_err(|_| "Workshop store could not be read")?;
    if let Some(value) = value {
        let value = js_sys::JSON::parse(&value.to_string())
            .map_err(|_| "Workshop document could not be encoded")?;
        store
            .add_with_key(&value, &JsValue::from_str(id))
            .map_err(|_| "Workshop document could not be saved")?;
    } else {
        store
            .delete(&JsValue::from_str(id))
            .map_err(|_| "Workshop document could not be retired")?;
    }
    transaction_done(&transaction).await
}
pub(super) async fn workshop_url(
    window: &Window,
    mount: &WebsiteMount,
    request: Value,
) -> Result<String, String> {
    let id = window.crypto().map_err(|_| "Workshop random source unavailable")?.random_uuid();
    let database = open_workshop_database(window, mount).await?;
    let result = async {
        write_workshop(&database, &id, Some(&request)).await?;
        let token = (|| {
            window
                .session_storage()
                .map_err(|_| "Workshop tab storage is blocked")?
                .ok_or("Workshop tab storage is unavailable")?
                .set_item(&mount.storage_key(&format!("{WORKSHOP_TOKEN}{id}")), &id)
                .map_err(|_| "Workshop tab token could not be saved")
        })();
        if token.is_err() {
            write_workshop(&database, &id, None).await?;
            return Err("Workshop tab token could not be saved".to_owned());
        }
        Ok(format!("{}?workshop={id}", mount.path("/boneyard")))
    }
    .await;
    database.close();
    result
}
