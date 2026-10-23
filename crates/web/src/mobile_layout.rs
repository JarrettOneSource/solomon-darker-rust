//! Website `mobile-ui-layout.ts` / `mobile-ui-sharing.ts` storage edge.
//! The complete validated document is shared by the editor and live controls.

use std::cell::Cell;
use std::rc::Rc;

use solomon_sim::mobile_ui::{
    MobileUiLayout, MobileUiLayoutDocument, MobileUiLayoutState, legacy_mobile_ui_layout,
    mobile_ui_layout_document, mobile_ui_layout_from_document,
};
use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{Event, Storage, StorageEvent, Window};

use crate::hosted::WebsiteMount;

pub const MOBILE_UI_LAYOUT_STORAGE_KEY: &str = "solomon-dark-mobile-ui-layout-v1";
/// Same-page controls subscribe to this native event. Other browser tabs receive
/// the ordinary storage event and read the same mount-scoped key.
pub const MOBILE_UI_LAYOUT_CHANGED_EVENT: &str = "solomon-dark-mobile-ui-layout-changed";

pub struct MobileUiLayoutStore {
    window: Window,
    key: String,
    changed: Rc<Cell<bool>>,
    layout_change: Closure<dyn FnMut(Event)>,
    storage_change: Closure<dyn FnMut(Event)>,
}

impl MobileUiLayoutStore {
    pub fn new(window: &Window, mount: &WebsiteMount) -> Result<Self, String> {
        let key = mount.storage_key(MOBILE_UI_LAYOUT_STORAGE_KEY);
        let changed = Rc::new(Cell::new(true));
        let dirty = changed.clone();
        let layout_change = Closure::<dyn FnMut(Event)>::new(move |_| dirty.set(true));
        let dirty = changed.clone();
        let storage_key = key.clone();
        let storage_change = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
            if event
                .dyn_ref::<StorageEvent>()
                .is_some_and(|event| event.key().is_none_or(|key| key == storage_key))
            {
                dirty.set(true);
            }
        });
        let store = Self { window: window.clone(), key, changed, layout_change, storage_change };
        window
            .add_event_listener_with_callback(
                MOBILE_UI_LAYOUT_CHANGED_EVENT,
                store.layout_change.as_ref().unchecked_ref(),
            )
            .map_err(|_| "Mobile layout changes could not be observed.")?;
        window
            .add_event_listener_with_callback(
                "storage",
                store.storage_change.as_ref().unchecked_ref(),
            )
            .map_err(|_| "Mobile layout storage changes could not be observed.")?;
        Ok(store)
    }

    pub fn take_changed(&self) -> bool {
        self.changed.replace(false)
    }

    #[must_use]
    pub fn storage_key(&self) -> &str {
        &self.key
    }

    pub fn read(&self) -> Result<MobileUiLayoutState, String> {
        let Some(serialized) = self
            .storage()?
            .get_item(&self.key)
            .map_err(|_| "The mobile layout could not be read.")?
        else {
            return Ok(MobileUiLayoutState::default());
        };
        let document = parse_document(&serialized);
        let layout = document
            .as_ref()
            .and_then(mobile_ui_layout_from_document)
            .or_else(|| document.as_ref().and_then(legacy_mobile_ui_layout));
        Ok(layout.map_or_else(MobileUiLayoutState::default, |layout| MobileUiLayoutState {
            customized: true,
            layout,
        }))
    }

    pub fn current_document(&self) -> Result<MobileUiLayoutDocument, String> {
        let current = self.read()?;
        if !current.customized {
            return Err("Customize and save a mobile layout before submitting it.".to_owned());
        }
        Ok(mobile_ui_layout_document(&current.layout))
    }

    pub fn install_shared(&self, value: &serde_json::Value) -> Result<MobileUiLayoutState, String> {
        let document = serde_json::from_value::<MobileUiLayoutDocument>(value.clone()).ok();
        let layout = document
            .as_ref()
            .and_then(mobile_ui_layout_from_document)
            .ok_or("That shared layout is not a supported mobile UI document.")?;
        self.set_layout(&layout)
    }

    pub fn set_layout(&self, layout: &MobileUiLayout) -> Result<MobileUiLayoutState, String> {
        let document = mobile_ui_layout_document(layout);
        let serialized = serde_json::to_string(&document)
            .map_err(|_| "The mobile layout could not be encoded.")?;
        self.storage()?
            .set_item(&self.key, &serialized)
            .map_err(|_| "The mobile layout could not be saved.")?;
        let state = MobileUiLayoutState {
            customized: true,
            layout: mobile_ui_layout_from_document(&document)
                .expect("normalized complete mobile layout"),
        };
        self.notify()?;
        Ok(state)
    }

    pub fn reset(&self) -> Result<MobileUiLayoutState, String> {
        self.storage()?
            .remove_item(&self.key)
            .map_err(|_| "The mobile layout could not be reset.")?;
        self.notify()?;
        Ok(MobileUiLayoutState::default())
    }

    fn storage(&self) -> Result<Storage, String> {
        self.window
            .local_storage()
            .map_err(|_| "Mobile layout storage is unavailable.")?
            .ok_or_else(|| "Mobile layout storage is unavailable.".to_owned())
    }

    fn notify(&self) -> Result<(), String> {
        self.window
            .dispatch_event(
                &web_sys::Event::new(MOBILE_UI_LAYOUT_CHANGED_EVENT)
                    .map_err(|_| "Mobile layout notification could not be created.")?,
            )
            .map_err(|_| "Mobile layout controls could not be notified.")?;
        Ok(())
    }
}

impl Drop for MobileUiLayoutStore {
    fn drop(&mut self) {
        let _ = self.window.remove_event_listener_with_callback(
            MOBILE_UI_LAYOUT_CHANGED_EVENT,
            self.layout_change.as_ref().unchecked_ref(),
        );
        let _ = self.window.remove_event_listener_with_callback(
            "storage",
            self.storage_change.as_ref().unchecked_ref(),
        );
    }
}

fn parse_document(serialized: &str) -> Option<MobileUiLayoutDocument> {
    // JSON.parse keeps the last duplicate object key. Deserialize through Value
    // before the closed schema so legacy browser storage has the same semantics.
    let value: serde_json::Value = serde_json::from_str(serialized).ok()?;
    serde_json::from_value(value).ok()
}
