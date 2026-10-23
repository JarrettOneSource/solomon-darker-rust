//! Nonvisual screen-reader announcements for GPU controls and Tutorial narration.

use wasm_bindgen::{JsCast, JsValue};
use web_sys::{Document, HtmlElement};

pub(super) struct Accessibility {
    document: Document,
    native: Option<js_sys::Function>,
    status: Option<HtmlElement>,
    last_narration: String,
}

impl Accessibility {
    pub fn attach(document: &Document) -> Result<Self, String> {
        let native = js_sys::Reflect::get(document.as_ref(), &JsValue::from_str("ariaNotify"))
            .map_err(|error| format!("accessibility notification capability: {error:?}"))?
            .dyn_into::<js_sys::Function>()
            .ok();
        let status = if native.is_none() {
            let status: HtmlElement = document
                .create_element("div")
                .map_err(|error| format!("accessibility announcement adapter: {error:?}"))?
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
                    .map_err(|error| format!("accessibility announcement attribute: {error:?}"))?;
            }
            document
                .body()
                .ok_or("no body for accessibility announcements")?
                .append_child(&status)
                .map_err(|error| format!("accessibility announcement attachment: {error:?}"))?;
            Some(status)
        } else {
            None
        };
        Ok(Self { document: document.clone(), native, status, last_narration: String::new() })
    }

    pub fn update(
        &mut self,
        output: &egui::PlatformOutput,
        narration: &[String],
    ) -> Result<(), String> {
        for text in narration.iter().filter(|text| !text.is_empty()) {
            if *text != self.last_narration {
                self.speak(text)?;
                self.last_narration.clone_from(text);
            }
        }
        let description = output.events_description();
        if !description.is_empty() {
            self.speak(&description)?;
        }
        Ok(())
    }

    fn speak(&self, text: &str) -> Result<(), String> {
        if let Some(native) = &self.native {
            native
                .call1(self.document.as_ref(), &JsValue::from_str(text))
                .map_err(|error| format!("accessibility announcement: {error:?}"))?;
        } else if let Some(status) = &self.status {
            status.set_text_content(Some(text));
        }
        Ok(())
    }
}

impl Drop for Accessibility {
    fn drop(&mut self) {
        if let Some(status) = &self.status {
            status.remove();
        }
    }
}
