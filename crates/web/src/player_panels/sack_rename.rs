//! Current Website `HubSackRenameDialog.tsx`: an editable, receipt-driven owned-Sack name.
//! Browser focus and drafts live here; the canonical inventory owns name admission.

use std::cell::RefCell;
use std::rc::Rc;

use solomon_sim::economy::{
    HubInventoryAction, HubInventoryItem, INVENTORY_SACK_NAME_MAX_LENGTH,
    normalize_inventory_sack_name,
};
use solomon_sim::hud::HudRect;
use solomon_sim::native_ui_text::{NativeUiFontName, NativeUiTextAlign, NativeUiTextSpec};
use solomon_sim::player_panels::sack_rename::inventory_sack_rename_dialog_layout;
use solomon_sim::player_panels::sack_rename::{
    INVENTORY_SACK_RENAME_BUTTON, INVENTORY_SACK_RENAME_BUTTON_SCALE, InventorySackRenameOwner,
    InventorySackRenameSelection, inventory_sack_rename_target,
};
use wasm_bindgen::{JsCast, prelude::*};
use web_sys::{Document, Element, Event, HtmlElement, HtmlInputElement, KeyboardEvent};

use super::chrome::{bitmap_text, place};
use super::{PlayerPanel, PlayerPanelAction, PlayerPanelSnapshot, PlayerPanels, Target};
use crate::ui::cloud::cloud_text;
use crate::ui::{StockAtlas, append, element, message_frame, native_button, style};

pub(super) enum SackRenameAction {
    Close,
    Submit { item_id: u64, name: String },
}

enum Intent {
    Close,
    Submit(String),
}

type RenameListener = (&'static str, Closure<dyn FnMut(Event)>);

pub(super) struct SackRenameDialog {
    pub item_id: u64,
    pub root: HtmlElement,
    art: HtmlElement,
    text: HtmlElement,
    field: HtmlElement,
    field_art: HtmlElement,
    input: HtmlInputElement,
    error: HtmlElement,
    save: HtmlElement,
    cancel: HtmlElement,
    prior_focus: Option<HtmlElement>,
    events: Rc<RefCell<Vec<Intent>>>,
    listeners: Vec<RenameListener>,
    pending_name: Option<String>,
    viewport_size: (i32, i32),
    field_size: (f64, f64),
}

impl PlayerPanels {
    fn sack_rename_candidate<'a>(
        &self,
        snapshot: &'a PlayerPanelSnapshot<'_>,
    ) -> Option<&'a HubInventoryItem> {
        let selection = self.selected.map(|target| match target {
            Target::Item(item_id) => {
                InventorySackRenameSelection { item_id, owner: InventorySackRenameOwner::Backpack }
            }
            Target::Storage(item_id) => {
                InventorySackRenameSelection { item_id, owner: InventorySackRenameOwner::Storage }
            }
            _ => InventorySackRenameSelection {
                item_id: 0,
                owner: InventorySackRenameOwner::Unowned,
            },
        });
        inventory_sack_rename_target(snapshot.economy, selection, &self.sack_path)
    }

    pub(super) fn draw_sack_rename_open(
        &self,
        snapshot: &PlayerPanelSnapshot<'_>,
    ) -> Result<(), String> {
        if !matches!(self.panel, Some(PlayerPanel::Inventory | PlayerPanel::Service(_)))
            || self.notice.is_some()
            || self.dye.is_some()
            || self.rename_sack.is_some()
            || self.sack_transition.is_some()
        {
            return Ok(());
        }
        let Some(item) = self.sack_rename_candidate(snapshot) else { return Ok(()) };
        let button = native_button(&self.document, "RENAME SACK", "")?;
        button.remove_attribute("data-menu-action").map_err(|_| "Rename Sack producer")?;
        button
            .set_attribute("data-panel-action", "rename-sack")
            .map_err(|_| "Rename Sack open action")?;
        button
            .set_attribute("data-inventory-rename-open", &item.id.to_string())
            .map_err(|_| "Rename Sack target")?;
        button
            .set_attribute("aria-label", &format!("Rename {}", item.name))
            .map_err(|_| "Rename Sack label")?;
        button
            .toggle_attribute_with_force("disabled", self.inventory_transition_locked())
            .map_err(|_| "Rename Sack transition admission")?;
        place_scaled_button(
            &button,
            INVENTORY_SACK_RENAME_BUTTON,
            INVENTORY_SACK_RENAME_BUTTON_SCALE,
        )?;
        append(&self.content, &button)
    }

    pub(super) fn open_sack_rename(
        &mut self,
        snapshot: &PlayerPanelSnapshot<'_>,
    ) -> Result<(), String> {
        let Some(item) = self.sack_rename_candidate(snapshot) else { return Ok(()) };
        let dialog = SackRenameDialog::attach(&self.document, &self.root, item)?;
        self.stage.set_attribute("inert", "").map_err(|_| "Rename Sack underlying input")?;
        self.compact_navigation
            .set_attribute("inert", "")
            .map_err(|_| "Rename Sack underlying sections")?;
        self.rename_sack = Some(dialog);
        self.press = None;
        self.sound(solomon_sim::audio::AudioCue::Click);
        self.navigation.release();
        self.dirty = true;
        Ok(())
    }

    pub(super) fn close_sack_rename(&mut self) -> Result<(), String> {
        // Restore the old surface before the dialog's focus cleanup runs.
        self.stage.remove_attribute("inert").map_err(|_| "Rename Sack underlying input restore")?;
        self.compact_navigation
            .remove_attribute("inert")
            .map_err(|_| "Rename Sack section input restore")?;
        if self.rename_sack.take().is_some() {
            self.dirty = true;
            self.navigation.release();
        }
        Ok(())
    }

    pub(super) fn update_sack_rename(
        &mut self,
        snapshot: &PlayerPanelSnapshot<'_>,
        width: i32,
        height: i32,
    ) -> Result<(), String> {
        let Some(dialog) = &mut self.rename_sack else { return Ok(()) };
        if solomon_sim::economy::find_inventory_item(&snapshot.economy.backpack, dialog.item_id)
            .or_else(|| {
                solomon_sim::economy::find_inventory_item(&snapshot.economy.storage, dialog.item_id)
            })
            .is_none()
        {
            return self.close_sack_rename();
        }
        let action = dialog.update(&self.document, &self.ui, &self.control_panel, width, height)?;
        match action {
            Some(SackRenameAction::Close) => self.close_sack_rename()?,
            Some(SackRenameAction::Submit { item_id, name }) => {
                self.actions.push(PlayerPanelAction::Inventory(HubInventoryAction::RenameSack {
                    item_id,
                    name,
                }))
            }
            None => {}
        }
        Ok(())
    }
}

impl SackRenameDialog {
    pub fn attach(
        document: &Document,
        parent: &HtmlElement,
        item: &HubInventoryItem,
    ) -> Result<Self, String> {
        let prior_focus = document.active_element().and_then(|node| node.dyn_into().ok());
        let root = element(document, "section", "hub-sack-rename-dialog")?;
        let item_id = item.id.to_string();
        for (name, value) in [
            ("aria-label", "Rename Sack"),
            ("aria-modal", "true"),
            ("role", "dialog"),
            ("data-sack-rename-dialog", item_id.as_str()),
        ] {
            root.set_attribute(name, value).map_err(|_| "Sack rename dialog semantics")?;
        }
        let art = element(document, "div", "hub-sack-rename-art")?;
        let text = element(document, "div", "hub-sack-rename-text")?;
        let field = element(document, "div", "hub-sack-rename-field")?;
        let row = element(document, "div", "native-ui-control-field-row")?;
        let label = element(document, "label", "")?;
        label.set_attribute("for", "sack-rename-name").map_err(|_| "Sack name field label")?;
        append(
            &label,
            &cloud_text(document, "Sack name", NativeUiFontName::Medium, 1.0, 0xd9ba70, false)?,
        )?;
        let panel = element(document, "div", "native-ui-control-panel native-ui-control-field")?;
        panel
            .set_attribute("data-native-ui-control-panel", "field")
            .map_err(|_| "Sack name field art")?;
        let field_art = element(document, "span", "native-ui-control-panel-art")?;
        append(&panel, &field_art)?;
        let input: HtmlInputElement =
            element(document, "input", "")?.dyn_into().map_err(|_| "Sack rename input")?;
        input.set_id("sack-rename-name");
        input.set_type("text");
        input.set_autocomplete("off");
        input.set_max_length(INVENTORY_SACK_NAME_MAX_LENGTH as i32);
        input.set_value(&item.name);
        panel.append_child(&input).map_err(|_| "Sack name field input")?;
        append(&row, &label)?;
        append(&row, &panel)?;
        append(&field, &row)?;
        let error = element(document, "p", "hub-sack-rename-error")?;
        error.set_attribute("aria-live", "polite").map_err(|_| "Sack rename error status")?;
        append(&field, &error)?;
        let save = rename_button(document, "SAVE", "save")?;
        let cancel = rename_button(document, "CANCEL", "cancel")?;
        cancel.set_attribute("data-game-back", "").map_err(|_| "Sack rename cancel")?;
        for child in [&art, &text, &field, &save, &cancel] {
            append(&root, child)?;
        }
        append(parent, &root)?;
        let mut dialog = Self {
            item_id: item.id,
            root,
            art,
            text,
            field,
            field_art,
            input,
            error,
            save,
            cancel,
            prior_focus,
            events: Rc::new(RefCell::new(Vec::new())),
            listeners: Vec::new(),
            pending_name: None,
            viewport_size: (0, 0),
            field_size: (0.0, 0.0),
        };
        dialog.attach_events()?;
        dialog.refresh_admission(None)?;
        dialog.input.focus().map_err(|_| "Sack rename focus")?;
        dialog
            .input
            .set_selection_range(0, item.name.encode_utf16().count() as u32)
            .map_err(|_| "Sack rename selection")?;
        Ok(dialog)
    }

    pub fn update(
        &mut self,
        document: &Document,
        ui: &StockAtlas,
        controls: &StockAtlas,
        width: i32,
        height: i32,
    ) -> Result<Option<SackRenameAction>, String> {
        if self.viewport_size != (width, height) {
            self.viewport_size = (width, height);
            let layout = inventory_sack_rename_dialog_layout(
                f64::from(width),
                f64::from(height),
                f64::from(ui.record(18)?.logical_width),
                f64::from(ui.record(8)?.logical_height),
            );
            self.art.set_text_content(None);
            let frame = message_frame(
                document,
                ui,
                layout.body.width / layout.art_scale,
                layout.body.height / layout.art_scale,
            )?;
            style(
                &frame,
                &format!(
                    "position:absolute;left:{}px;top:{}px;transform:scale({});transform-origin:top left",
                    layout.body.x, layout.body.y, layout.art_scale
                ),
            )?;
            append(&self.art, &frame)?;
            self.text.set_text_content(None);
            for (label, font, baseline) in [
                ("Rename Sack".to_owned(), NativeUiFontName::Menu, layout.title_baseline),
                (
                    format!("Up to {INVENTORY_SACK_NAME_MAX_LENGTH} characters"),
                    NativeUiFontName::Medium,
                    layout.limit_baseline,
                ),
            ] {
                bitmap_text(
                    document,
                    &self.text,
                    &NativeUiTextSpec {
                        align: NativeUiTextAlign::Left,
                        ..NativeUiTextSpec::new(font, &label, layout.body.x + 48.0, baseline)
                    },
                )?;
            }
            style(
                &self.field,
                &format!(
                    "position:absolute;left:{}px;top:{}px;width:{}px",
                    layout.field.x, layout.field.y, layout.field.width
                ),
            )?;
            place_scaled_button(&self.save, layout.save, layout.button_scale)?;
            place_scaled_button(&self.cancel, layout.cancel, layout.button_scale)?;
        }
        let bounds = self.field_art.get_bounding_client_rect();
        if bounds.width() > 0.0
            && bounds.height() > 0.0
            && self.field_size != (bounds.width(), bounds.height())
        {
            self.field_size = (bounds.width(), bounds.height());
            self.field_art.set_text_content(None);
            append(
                &self.field_art,
                &controls.frame(document, 5, bounds.width(), bounds.height(), 3.0)?,
            )?;
        }
        let events = std::mem::take(&mut *self.events.borrow_mut());
        for event in events {
            if self.pending_name.is_some() {
                continue;
            }
            match event {
                Intent::Close => return Ok(Some(SackRenameAction::Close)),
                Intent::Submit(name) => {
                    let Some(name) = normalize_inventory_sack_name(&name) else { continue };
                    self.pending_name = Some(name.to_owned());
                    self.refresh_admission(None)?;
                    return Ok(Some(SackRenameAction::Submit {
                        item_id: self.item_id,
                        name: name.to_owned(),
                    }));
                }
            }
        }
        Ok(None)
    }

    /// An accepted receipt closes the modal; rejection leaves the exact draft available.
    pub fn action_feedback(
        &mut self,
        action: &HubInventoryAction,
        accepted: bool,
    ) -> Result<bool, String> {
        let HubInventoryAction::RenameSack { item_id, name } = action else { return Ok(false) };
        if *item_id != self.item_id || self.pending_name.as_deref() != Some(name.as_str()) {
            return Ok(false);
        }
        if accepted {
            return Ok(true);
        }
        self.pending_name = None;
        self.refresh_admission(Some("That Sack could not be renamed. Please try again."))?;
        Ok(false)
    }

    pub fn can_cancel(&self) -> bool {
        self.pending_name.is_none()
    }

    fn refresh_admission(&self, error: Option<&str>) -> Result<(), String> {
        let pending = self.pending_name.is_some();
        self.input.set_disabled(pending);
        self.cancel
            .toggle_attribute_with_force("disabled", pending)
            .map_err(|_| "Sack rename Cancel admission")?;
        refresh_value(&self.input, &self.save, &self.error, error)
    }

    fn attach_events(&mut self) -> Result<(), String> {
        let events = self.events.clone();
        let input = self.input.clone();
        let click = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
            event.stop_propagation();
            if input.disabled() {
                return;
            }
            let action = event
                .target()
                .and_then(|target| target.dyn_into::<Element>().ok())
                .and_then(|target| target.closest("[data-sack-rename-action]").ok().flatten())
                .and_then(|target| target.get_attribute("data-sack-rename-action"));
            match action.as_deref() {
                Some("save") => events.borrow_mut().push(Intent::Submit(input.value())),
                Some("cancel") => events.borrow_mut().push(Intent::Close),
                _ => {}
            }
        });
        self.listen("click", click)?;
        let events = self.events.clone();
        let input = self.input.clone();
        let root = self.root.clone();
        let key = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
            event.stop_propagation();
            let Some(key) = event.dyn_ref::<KeyboardEvent>() else { return };
            if key.key() == "Escape" {
                event.prevent_default();
                if !input.disabled() {
                    events.borrow_mut().push(Intent::Close);
                }
            } else if key.key() == "Enter"
                && key.target().is_some_and(|target| target.is_instance_of::<HtmlInputElement>())
            {
                event.prevent_default();
                if !input.disabled() && !key.repeat() && !key.is_composing() {
                    events.borrow_mut().push(Intent::Submit(input.value()));
                }
            } else if key.key() == "Tab" {
                trap_tab(&root, key);
            }
        });
        self.listen("keydown", key)?;
        let input = self.input.clone();
        let save = self.save.clone();
        let error = self.error.clone();
        let changed = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
            event.stop_propagation();
            if let Err(message) = refresh_value(&input, &save, &error, None) {
                web_sys::console::error_1(&message.into());
            }
        });
        self.listen("input", changed)?;
        for name in ["pointerdown", "pointermove", "pointerup", "pointercancel", "wheel"] {
            self.listen(
                name,
                Closure::<dyn FnMut(Event)>::new(|event: Event| event.stop_propagation()),
            )?;
        }
        Ok(())
    }

    fn listen(
        &mut self,
        name: &'static str,
        listener: Closure<dyn FnMut(Event)>,
    ) -> Result<(), String> {
        self.root
            .add_event_listener_with_callback(name, listener.as_ref().unchecked_ref())
            .map_err(|_| format!("Sack rename {name} listener"))?;
        self.listeners.push((name, listener));
        Ok(())
    }
}

impl Drop for SackRenameDialog {
    fn drop(&mut self) {
        for (name, listener) in &self.listeners {
            let _ = self
                .root
                .remove_event_listener_with_callback(name, listener.as_ref().unchecked_ref());
        }
        self.root.remove();
        if let Some(prior) = &self.prior_focus
            && prior.is_connected()
        {
            let _ = prior.focus();
        }
    }
}

fn refresh_value(
    input: &HtmlInputElement,
    save: &HtmlElement,
    error: &HtmlElement,
    rejection: Option<&str>,
) -> Result<(), String> {
    let value = input.value();
    let valid = normalize_inventory_sack_name(&value).is_some();
    save.toggle_attribute_with_force("disabled", input.disabled() || !valid)
        .map_err(|_| "Sack rename Save admission")?;
    error.set_text_content(Some(rejection.unwrap_or(if valid {
        "\u{a0}"
    } else {
        "Enter a name using 1–32 characters."
    })));
    Ok(())
}

fn rename_button(document: &Document, label: &str, action: &str) -> Result<HtmlElement, String> {
    let button = native_button(document, label, "")?;
    button.remove_attribute("data-menu-action").map_err(|_| "Sack rename button producer")?;
    button.set_attribute("data-sack-rename-action", action).map_err(|_| "Sack rename action")?;
    Ok(button)
}

fn place_scaled_button(button: &HtmlElement, bounds: HudRect, scale: f64) -> Result<(), String> {
    place(button, HudRect::new(bounds.x, bounds.y, bounds.width / scale, bounds.height / scale))?;
    button
        .style()
        .set_property("transform", &format!("scale({scale})"))
        .map_err(|_| "Sack rename button scale")?;
    button
        .style()
        .set_property("transform-origin", "top left")
        .map_err(|_| "Sack rename button origin".into())
}

fn trap_tab(root: &HtmlElement, key: &KeyboardEvent) {
    let Ok(controls) = root.query_selector_all("input:not(:disabled),button:not(:disabled)") else {
        return;
    };
    let first = controls.item(0);
    let last = controls.item(controls.length().saturating_sub(1));
    let edge = if key.shift_key() { first.as_ref() } else { last.as_ref() };
    let active = root.owner_document().and_then(|document| document.active_element());
    if edge
        .is_some_and(|edge| active.as_ref().is_some_and(|active| edge.is_same_node(Some(active))))
    {
        key.prevent_default();
        if let Some(target) = if key.shift_key() { last } else { first }
            && let Ok(target) = target.dyn_into::<HtmlElement>()
        {
            let _ = target.focus();
        }
    }
}
