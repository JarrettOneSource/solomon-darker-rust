//! Nonvisual platform announcements shared by loading and the main GPU UI.

use wasm_bindgen::{JsCast, JsValue};
use web_sys::{Document, HtmlElement};

pub struct ScreenReader {
    document: Document,
    native: Option<js_sys::Function>,
    status: Option<HtmlElement>,
    shared: js_sys::Object,
    loader: bool,
}
impl ScreenReader {
    pub fn attach(document: &Document) -> Result<Self, String> {
        Self::attach_owner(document, false)
    }

    pub fn attach_loader(document: &Document) -> Result<Self, String> {
        Self::attach_owner(document, true)
    }

    fn attach_owner(document: &Document, loader: bool) -> Result<Self, String> {
        // The loader and game are separate WASM modules. A document-owned object
        // shares this lifetime across them without a module-local Rust singleton.
        let key = js_sys::Symbol::for_("solomon.graphics.announcements");
        let previous = js_sys::Reflect::get(document.as_ref(), &key)
            .map_err(|e| format!("accessibility announcement ownership: {e:?}"))?;
        let shared = if previous.is_undefined() {
            let shared = js_sys::Object::new();
            set(&shared, "owners", &0.into())?;
            set(&shared, "loaders", &0.into())?;
            set(&shared, "status", &JsValue::NULL)?;
            shared
        } else {
            previous.dyn_into().map_err(|_| "invalid accessibility announcement ownership")?
        };
        let native = js_sys::Reflect::get(document.as_ref(), &"ariaNotify".into())
            .map_err(|e| format!("accessibility notification capability: {e:?}"))?
            .dyn_into::<js_sys::Function>()
            .ok();
        let existing = get(&shared, "status")?;
        let status = if !existing.is_null() {
            Some(existing.dyn_into().map_err(|_| "invalid accessibility announcement adapter")?)
        } else if native.is_none() {
            let status: HtmlElement = document
                .create_element("div")
                .map_err(|e| format!("accessibility announcement adapter: {e:?}"))?
                .dyn_into()
                .map_err(|_| "accessibility announcement adapter is not HTML")?;
            for (name, value) in [
                ("data-gpu-announcer", ""),
                ("role", "status"),
                ("aria-live", "polite"),
                ("aria-atomic", "true"),
                (
                    "style",
                    "position:fixed;left:-10000px;top:0;width:1px;height:1px;overflow:hidden;clip-path:inset(50%);white-space:nowrap;pointer-events:none",
                ),
            ] {
                status
                    .set_attribute(name, value)
                    .map_err(|e| format!("accessibility announcement attribute: {e:?}"))?;
            }
            document
                .body()
                .ok_or("no body for accessibility announcements")?
                .append_child(&status)
                .map_err(|e| format!("accessibility announcement attachment: {e:?}"))?;
            set(&shared, "status", status.as_ref())?;
            Some(status)
        } else {
            None
        };
        set(&shared, "owners", &(count(&shared, "owners")? + 1).into())?;
        set(&shared, "loaders", &(count(&shared, "loaders")? + u32::from(loader)).into())?;
        js_sys::Reflect::set(document.as_ref(), &key, &shared)
            .map_err(|e| format!("accessibility announcement ownership attachment: {e:?}"))?;
        Ok(Self { document: document.clone(), native, status, shared, loader })
    }

    pub fn is_foreground(&self) -> Result<bool, String> {
        Ok(self.loader || count(&self.shared, "loaders")? == 0)
    }

    pub fn announce_batch(&self, messages: &[String]) -> Result<(), String> {
        // One live-region mutation preserves every message in this frame;
        // sequential textContent writes would leave only the final one.
        self.announce(&messages.join("\n"))
    }

    pub fn announce(&self, text: &str) -> Result<(), String> {
        if !self.is_foreground()? {
            return Ok(());
        }
        if let Some(native) = &self.native {
            native
                .call1(self.document.as_ref(), &JsValue::from_str(text))
                .map_err(|e| format!("accessibility announcement: {e:?}"))?;
        } else if let Some(status) = &self.status {
            status.set_text_content(Some(text));
        }
        Ok(())
    }

    fn release(&self) -> Result<(), String> {
        let owners = count(&self.shared, "owners")? - 1;
        set(&self.shared, "owners", &owners.into())?;
        if self.loader {
            set(&self.shared, "loaders", &(count(&self.shared, "loaders")? - 1).into())?;
        }
        if owners == 0 {
            if let Ok(status) = get(&self.shared, "status")?.dyn_into::<HtmlElement>() {
                status.remove();
            }
            js_sys::Reflect::set(
                self.document.as_ref(),
                &js_sys::Symbol::for_("solomon.graphics.announcements"),
                &JsValue::UNDEFINED,
            )
            .map_err(|e| format!("accessibility announcement ownership release: {e:?}"))?;
        }
        Ok(())
    }
}
impl Drop for ScreenReader {
    fn drop(&mut self) {
        let _ = self.release();
    }
}

fn get(shared: &js_sys::Object, key: &str) -> Result<JsValue, String> {
    js_sys::Reflect::get(shared, &key.into())
        .map_err(|e| format!("accessibility announcement {key}: {e:?}"))
}

fn set(shared: &js_sys::Object, key: &str, value: &JsValue) -> Result<(), String> {
    js_sys::Reflect::set(shared, &key.into(), value)
        .map(|_| ())
        .map_err(|e| format!("accessibility announcement {key}: {e:?}"))
}

fn count(shared: &js_sys::Object, key: &str) -> Result<u32, String> {
    get(shared, key)?
        .as_f64()
        .map(|count| count as u32)
        .ok_or_else(|| format!("invalid accessibility announcement {key}"))
}
