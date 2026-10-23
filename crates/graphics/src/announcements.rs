//! Nonvisual platform announcements shared by loading and the main GPU UI.

use wasm_bindgen::{JsCast, JsValue};
use web_sys::{Document, HtmlElement};

pub struct ScreenReader {
    document: Document,
    native: Option<js_sys::Function>,
    status: Option<HtmlElement>,
}
impl ScreenReader {
    pub fn attach(document: &Document) -> Result<Self, String> {
        let native = js_sys::Reflect::get(document.as_ref(), &"ariaNotify".into())
            .map_err(|e| format!("accessibility notification capability: {e:?}"))?
            .dyn_into::<js_sys::Function>()
            .ok();
        let status = if native.is_none() {
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
            Some(status)
        } else {
            None
        };
        Ok(Self { document: document.clone(), native, status })
    }
    pub fn announce(&self, text: &str) -> Result<(), String> {
        if let Some(native) = &self.native {
            native
                .call1(self.document.as_ref(), &JsValue::from_str(text))
                .map_err(|e| format!("accessibility announcement: {e:?}"))?;
        } else if let Some(status) = &self.status {
            status.set_text_content(Some(text));
        }
        Ok(())
    }
}
impl Drop for ScreenReader {
    fn drop(&mut self) {
        if let Some(status) = &self.status {
            status.remove();
        }
    }
}
