//! Source `NativeUiNotebox.tsx`: a non-modal, dismissible instruction/failure.
//! Its clock belongs to presentation; it never pauses or mutates the session.

use std::cell::Cell;
use std::rc::Rc;

use solomon_sim::native_ui_notebox::{
    NativeNoteboxKind, native_notebox_duration_ms, native_notebox_layout, native_notebox_opacity,
};
use solomon_sim::native_ui_text::{NativeUiFontName, NativeUiTextSpec};
use wasm_bindgen::{JsCast, prelude::*};
use web_sys::{Document, Element, Event, HtmlElement};

use super::chrome::tint;
use crate::ui::{StockAtlas, append, element, style, text::native_text};

struct Notice {
    kind: NativeNoteboxKind,
    started_at: f64,
}

pub(super) struct NpcNotebox {
    document: Document,
    root: HtmlElement,
    stage: HtmlElement,
    status: HtmlElement,
    panel: Option<HtmlElement>,
    active: Option<Notice>,
    sequence: u64,
    elapsed: Rc<Cell<f64>>,
    dismissed: Rc<Cell<Option<f64>>>,
    pointer: Closure<dyn FnMut(Event)>,
}

impl NpcNotebox {
    pub(super) fn new(document: &Document) -> Result<Self, String> {
        let root = element(document, "div", "native-notebox-overlay")?;
        root.set_hidden(true);
        let css = element(document, "style", "native-notebox-styles")?;
        css.set_text_content(Some(include_str!("../../../../web/native-notebox.css")));
        append(&root, &css)?;
        let stage = element(document, "div", "native-notebox-stage")?;
        append(&root, &stage)?;
        let status = element(document, "span", "native-notebox-status")?;
        status.set_attribute("aria-atomic", "true").map_err(|_| "Boast notice announcement")?;
        append(&root, &status)?;
        append(&document.body().ok_or("Boast notices need the game document")?, &root)?;
        let elapsed = Rc::new(Cell::new(0.0));
        let dismissed = Rc::new(Cell::new(None));
        let current_elapsed = elapsed.clone();
        let dismiss = dismissed.clone();
        let pointer = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
            let panel = event
                .target()
                .and_then(|target| target.dyn_into::<Element>().ok())
                .and_then(|target| {
                    target.closest(".native-notebox-panel").expect("valid notice selector")
                });
            if panel.is_none() {
                return;
            }
            event.prevent_default();
            event.stop_propagation();
            if dismiss.get().is_none() {
                dismiss.set(Some(current_elapsed.get()));
            }
        });
        root.add_event_listener_with_callback("pointerdown", pointer.as_ref().unchecked_ref())
            .map_err(|_| "Could not listen to Boast notice dismissal")?;
        Ok(Self {
            document: document.clone(),
            root,
            stage,
            status,
            panel: None,
            active: None,
            sequence: 0,
            elapsed,
            dismissed,
            pointer,
        })
    }

    pub(super) fn show(
        &mut self,
        ui: &StockAtlas,
        kind: NativeNoteboxKind,
        text: &str,
        now: f64,
    ) -> Result<(), String> {
        self.sequence += 1;
        self.elapsed.set(0.0);
        self.dismissed.set(None);
        self.stage.set_text_content(None);
        let layout = native_notebox_layout(text);
        let panel = element(&self.document, "div", "native-notebox-panel")?;
        style(
            &panel,
            &format!(
                "left:{}px;top:{}px;width:{}px;height:{}px;opacity:0",
                layout.panel_left, layout.panel_top, layout.panel_width, layout.panel_height
            ),
        )?;
        for class in ["native-notebox-frame-primary", "native-notebox-frame-secondary"] {
            let frame =
                ui.nine_slice(&self.document, 64, layout.panel_width, layout.panel_height)?;
            frame.set_class_name(&format!("native-notebox-frame {class}"));
            if kind == NativeNoteboxKind::Failure {
                tint(&self.document, &frame, 0xff4040)?;
            }
            append(&panel, &frame)?;
        }
        let mut spec = NativeUiTextSpec::new(
            NativeUiFontName::Menu,
            text,
            layout.panel_width / 2.0,
            layout.text_baseline,
        );
        spec.tint = if kind == NativeNoteboxKind::Failure { 0xff4040 } else { 0xd9ba70 };
        append(&panel, &native_text(&self.document, &spec, "native-notebox-text")?)?;
        append(&self.stage, &panel)?;
        self.root
            .set_attribute(
                "data-native-notebox-kind",
                if kind == NativeNoteboxKind::Failure { "failure" } else { "instruction" },
            )
            .map_err(|_| "Boast notice kind")?;
        self.root
            .set_attribute("data-native-notebox-sequence", &self.sequence.to_string())
            .map_err(|_| "Boast notice sequence")?;
        self.root
            .set_attribute("data-native-notebox-text", text)
            .map_err(|_| "Boast notice text")?;
        self.status
            .set_attribute(
                "role",
                if kind == NativeNoteboxKind::Failure { "alert" } else { "status" },
            )
            .map_err(|_| "Boast notice role")?;
        self.status.set_text_content(Some(text));
        self.root.set_hidden(false);
        self.active = Some(Notice { kind, started_at: now });
        self.panel = Some(panel);
        Ok(())
    }

    pub(super) fn update(&mut self, now: f64, width: i32, height: i32) -> Result<(), String> {
        let Some(notice) = &self.active else { return Ok(()) };
        let elapsed = (now - notice.started_at).max(0.0);
        self.elapsed.set(elapsed);
        let dismissed = self.dismissed.get();
        let expires =
            dismissed.map_or_else(|| native_notebox_duration_ms(notice.kind), |at| at + 200.0);
        if elapsed >= expires {
            self.reset();
            return Ok(());
        }
        let scale = (f64::from(width) / 1600.0).min(f64::from(height) / 900.0);
        let left = (f64::from(width) - 1600.0 * scale) / 2.0;
        let top = (f64::from(height) - 900.0 * scale) / 2.0;
        style(
            &self.stage,
            &format!(
                "position:absolute;left:{left}px;top:{top}px;width:1600px;height:900px;transform:scale({scale});transform-origin:top left"
            ),
        )?;
        let opacity = native_notebox_opacity(notice.kind, elapsed, dismissed);
        self.root
            .set_attribute("data-native-notebox-opacity", &opacity.to_string())
            .map_err(|_| "Boast notice opacity")?;
        let panel = self.panel.as_ref().expect("active Boast notice has its panel");
        panel
            .style()
            .set_property("opacity", &opacity.to_string())
            .map_err(|_| "Boast notice fade")?;
        panel
            .set_attribute(
                "data-native-notebox-dismissed",
                if dismissed.is_some() { "true" } else { "false" },
            )
            .map_err(|_| "Boast notice dismissal")?;
        Ok(())
    }

    pub(super) fn reset(&mut self) {
        self.root.set_hidden(true);
        self.stage.set_text_content(None);
        self.status.set_text_content(None);
        self.active = None;
        self.panel = None;
        self.dismissed.set(None);
    }
}

impl Drop for NpcNotebox {
    fn drop(&mut self) {
        self.root
            .remove_event_listener_with_callback(
                "pointerdown",
                self.pointer.as_ref().unchecked_ref(),
            )
            .expect("remove installed Boast notice listener");
        self.root.remove();
    }
}
