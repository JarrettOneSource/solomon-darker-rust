//! Website `mobile-ui-layout.ts` / `mobile-ui-sharing.ts` storage edge.
//! The complete validated document is shared by the editor and live controls.

use std::cell::Cell;
use std::rc::Rc;

use solomon_sim::mobile_ui::{
    MobileUiLayout, MobileUiLayoutDocument, MobileUiLayoutState, legacy_mobile_ui_layout,
    mobile_ui_layout_document, mobile_ui_layout_from_document,
};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::{JsCast, closure::Closure};
#[cfg(target_arch = "wasm32")]
use web_sys::{Event, StorageEvent, Window};

use crate::platform::Surface;

pub const MOBILE_UI_LAYOUT_STORAGE_KEY: &str = "solomon-dark-mobile-ui-layout-v1";
/// Same-page controls subscribe to this native event. Other browser tabs receive
/// the ordinary storage event and read the same mount-scoped key.
#[cfg(target_arch = "wasm32")]
pub const MOBILE_UI_LAYOUT_CHANGED_EVENT: &str = "solomon-dark-mobile-ui-layout-changed";

pub struct MobileUiLayoutStore {
    #[cfg(target_arch = "wasm32")]
    window: Window,
    key: String,
    changed: Rc<Cell<bool>>,
    #[cfg(not(target_arch = "wasm32"))]
    stamp: Cell<Option<(std::time::SystemTime, u64)>>,
    #[cfg(not(target_arch = "wasm32"))]
    next_check: Cell<std::time::Instant>,
    #[cfg(target_arch = "wasm32")]
    layout_change: Closure<dyn FnMut(Event)>,
    #[cfg(target_arch = "wasm32")]
    storage_change: Closure<dyn FnMut(Event)>,
}

impl MobileUiLayoutStore {
    pub fn new(surface: &Surface) -> Result<Self, String> {
        let key = crate::storage::scoped_key(MOBILE_UI_LAYOUT_STORAGE_KEY)?;
        let changed = Rc::new(Cell::new(true));
        #[cfg(target_arch = "wasm32")]
        {
            let window = surface
                .canvas()
                .owner_document()
                .and_then(|document| document.default_view())
                .ok_or("The game has no window")?;
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
            let store =
                Self { window: window.clone(), key, changed, layout_change, storage_change };
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
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = surface;
            Ok(Self {
                key,
                changed,
                stamp: Cell::new(crate::storage::preference_stamp()?),
                next_check: Cell::new(std::time::Instant::now()),
            })
        }
    }

    pub fn take_changed(&self) -> Result<bool, String> {
        #[cfg(not(target_arch = "wasm32"))]
        if std::time::Instant::now() >= self.next_check.get() {
            self.next_check.set(std::time::Instant::now() + std::time::Duration::from_millis(500));
            let stamp = crate::storage::preference_stamp()?;
            if self.stamp.replace(stamp) != stamp {
                self.changed.set(true);
            }
        }
        Ok(self.changed.replace(false))
    }

    #[must_use]
    pub fn storage_key(&self) -> &str {
        &self.key
    }

    pub fn read(&self) -> Result<MobileUiLayoutState, String> {
        let Some(serialized) = crate::storage::read_preference(&self.key)? else {
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
        crate::storage::write_preference(&self.key, Some(&serialized))?;
        let state = MobileUiLayoutState {
            customized: true,
            layout: mobile_ui_layout_from_document(&document)
                .expect("normalized complete mobile layout"),
        };
        self.notify()?;
        Ok(state)
    }

    pub fn reset(&self) -> Result<MobileUiLayoutState, String> {
        crate::storage::write_preference(&self.key, None)?;
        self.notify()?;
        Ok(MobileUiLayoutState::default())
    }

    fn notify(&self) -> Result<(), String> {
        self.changed.set(true);
        #[cfg(target_arch = "wasm32")]
        self.window
            .dispatch_event(
                &web_sys::Event::new(MOBILE_UI_LAYOUT_CHANGED_EVENT)
                    .map_err(|_| "Mobile layout notification could not be created.")?,
            )
            .map_err(|_| "Mobile layout controls could not be notified.")?;
        Ok(())
    }
}

#[cfg(target_arch = "wasm32")]
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
