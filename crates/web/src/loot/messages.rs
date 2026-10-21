//! `BoneyardScene.tsx` / `NativeLootBitmapText.tsx` pickup text projection.
//! The shared native font painter retains the stock body glyphs and flow placement.

use std::collections::{BTreeMap, BTreeSet, btree_map::Entry};

use solomon_sim::boneyard::loot::messages::NativeLootMessageVisual;
use solomon_sim::native_ui_text::{
    NativeUiFontName, NativeUiTextAlign, NativeUiTextPlacement, NativeUiTextSpec,
};
use web_sys::{Document, HtmlElement};

use crate::ui::text::native_text;
use crate::ui::{append, element, style};

pub struct LootMessages {
    document: Document,
    root: HtmlElement,
    messages: HtmlElement,
    rows: BTreeMap<u64, HtmlElement>,
}

impl LootMessages {
    pub fn attach(document: &Document) -> Result<Self, String> {
        let root = element(document, "div", "loot-message-viewport")?;
        let messages = element(document, "div", "boneyard-loot-messages")?;
        messages.set_attribute("aria-live", "polite").map_err(|_| "loot messages aria-live")?;
        messages.set_attribute("aria-atomic", "false").map_err(|_| "loot messages aria-atomic")?;
        style(
            &messages,
            "position:absolute;left:50%;top:67px;transform:translateX(-50%);transform-origin:center top;display:flex;flex-direction:column;align-items:center;text-align:center",
        )?;
        append(&root, &messages)?;
        let body = document.body().ok_or("page has no body")?;
        append(&body, &root)?;
        style(&root, "display:none")?;
        Ok(Self { document: document.clone(), root, messages, rows: BTreeMap::new() })
    }

    /// Uses the host HUD's logical width and display scale. The source offset is
    /// retained by the message clock; its Website DOM projection uses 18px rows.
    pub fn draw(
        &mut self,
        visuals: &[NativeLootMessageVisual],
        logical_width: f64,
        display_scale: f64,
        visible: bool,
    ) -> Result<(), String> {
        style(
            &self.root,
            &format!(
                "position:fixed;left:50%;top:0;z-index:45;width:{logical_width}px;transform:translateX(-50%) scale({display_scale});transform-origin:center top;pointer-events:none;display:{}",
                if visible { "block" } else { "none" },
            ),
        )?;
        let mut live_ids = BTreeSet::new();
        for visual in visuals {
            let id = visual.message.event_id;
            live_ids.insert(id);
            if let Entry::Vacant(entry) = self.rows.entry(id) {
                let row = element(&self.document, "span", "loot-message")?;
                append(&self.messages, &row)?;
                entry.insert(row);
            }
            let message = &self.rows[&id];
            if message.get_attribute("aria-label").as_deref() != Some(visual.message.text.as_str())
            {
                message.set_text_content(None);
                let spec = NativeUiTextSpec {
                    align: NativeUiTextAlign::Left,
                    placement: NativeUiTextPlacement::Box,
                    tint: visual.message.tint,
                    ..NativeUiTextSpec::new(NativeUiFontName::Body, &visual.message.text, 0.0, 0.0)
                };
                append(message, &native_text(&self.document, &spec, "boneyard-loot-bitmap-text")?)?;
            }
            message
                .set_attribute("aria-label", &visual.message.text)
                .map_err(|_| "loot message label")?;
            message
                .set_attribute("data-event-id", &visual.message.event_id.to_string())
                .map_err(|_| "loot message event")?;
            style(
                message,
                &format!(
                    "display:block;height:18px;white-space:nowrap;transform-origin:center;opacity:{};transform:scale({});filter:drop-shadow(0 2px 0 #000)",
                    visual.alpha, visual.scale,
                ),
            )?;
        }
        self.rows.retain(|id, row| {
            if live_ids.contains(id) {
                return true;
            }
            row.remove();
            false
        });
        Ok(())
    }

    pub fn destroy(&self) {
        self.root.remove();
    }
}
