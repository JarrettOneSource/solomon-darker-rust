//! Fabric Dye and Unforge presentation. Mixing, admission, RNG and permanent bonuses
//! remain in `sim::economy`; this surface collects the native transaction's arguments.

use super::chrome::{bitmap_text, button, place};
use super::notices::PlayerPanelNotice;
use super::{DyeSelection, PlayerPanelAction, PlayerPanelSnapshot, PlayerPanels, Target};
use crate::ui::{append, element, native_button, style};
use solomon_sim::economy::{
    HubEconomyResult, HubInventoryAction, NativeDyeLayer, NativeUnforgeOutcome,
    NativeUnforgeOutcomeKind, native_dye_mixed_tint, project_inventory_root_slots,
};
use solomon_sim::hud::HudRect;
use solomon_sim::native_ui_text::{NativeUiFontName, NativeUiTextAlign, NativeUiTextSpec};
use solomon_sim::player_panels::dye::{
    native_dye_modal_opacity, native_dye_selected_pulse, native_dye_swatch_rect,
};
use solomon_sim::player_panels::inventory_slot_rect;
use wasm_bindgen::JsCast;

impl PlayerPanels {
    pub(super) fn open_dye(&mut self, dye_item_id: u64, now: f64) -> Result<(), String> {
        let session_id = self
            .document
            .default_view()
            .ok_or("Fabric Dye window")?
            .crypto()
            .map_err(|_| "Fabric Dye session source")?
            .random_uuid();
        self.sound(solomon_sim::audio::AudioCue::Click);
        self.notice = None;
        self.dye = Some(DyeSelection {
            session_id: session_id.clone(),
            admitted: false,
            opened_at: now,
            closing_at: None,
            selected_at: None,
            selected_row: None,
            path: self.sack_path.clone(),
            swatch_rows: Vec::new(),
            target_item_id: None,
            pending: true,
        });
        self.actions.push(PlayerPanelAction::Inventory(HubInventoryAction::OpenDye {
            dye_item_id,
            session_id,
        }));
        self.dirty = true;
        Ok(())
    }

    pub(super) fn close_dye_session(&mut self) {
        if let Some(dye) = self.dye.take()
            && dye.closing_at.is_none()
        {
            self.actions.push(PlayerPanelAction::Inventory(HubInventoryAction::CloseDye {
                session_id: dye.session_id,
            }));
        }
    }

    pub(super) fn cancel_dye(&mut self, now: f64) {
        let Some(dye) = &mut self.dye else { return };
        if dye.pending || dye.closing_at.is_some() {
            return;
        }
        if dye.target_item_id.take().is_some() {
            return;
        }
        self.actions.push(PlayerPanelAction::Inventory(HubInventoryAction::CloseDye {
            session_id: dye.session_id.clone(),
        }));
        dye.closing_at = Some(now);
    }

    pub(super) fn update_dye_state(&mut self, snapshot: &PlayerPanelSnapshot<'_>, now: f64) {
        let Some(dye) = &mut self.dye else { return };
        if let Some(closing_at) = dye.closing_at {
            if now - closing_at >= 120.0 {
                self.dye = None;
                self.dirty = true;
            }
            return;
        }
        if dye.admitted && snapshot.dye_session_id != Some(dye.session_id.as_str()) {
            self.dye = None;
            self.dirty = true;
            return;
        }
        if !dye.pending
            && dye.target_item_id.is_some_and(|id| {
                solomon_sim::economy::find_inventory_item(&snapshot.economy.backpack, id)
                    .is_none_or(|item| !item.is_dyeable_clothing())
            })
        {
            dye.target_item_id = None;
            self.dirty = true;
        }
        if dye.selected_at.is_some_and(|at| now - at >= 200.0) {
            dye.selected_at = None;
            dye.selected_row = None;
            self.dirty = true;
        }
    }

    pub(super) fn update_dye_presentation(&self, now: f64) -> Result<(), String> {
        let Some(dye) = &self.dye else { return Ok(()) };
        let Some(layer) =
            self.root.query_selector(".panel-dye").map_err(|_| "Fabric Dye presentation")?
        else {
            return Ok(());
        };
        let layer: web_sys::HtmlElement = layer.dyn_into().map_err(|_| "Fabric Dye layer")?;
        layer
            .style()
            .set_property(
                "opacity",
                &native_dye_modal_opacity(dye.opened_at, dye.closing_at, now).to_string(),
            )
            .map_err(|_| "Fabric Dye native fade")?;
        if let Some(pulse) = layer
            .query_selector("[data-native-dye-selected-pulse]")
            .map_err(|_| "Fabric Dye pulse")?
        {
            let pulse: web_sys::HtmlElement =
                pulse.dyn_into().map_err(|_| "Fabric Dye pulse layer")?;
            pulse
                .style()
                .set_property(
                    "opacity",
                    &native_dye_selected_pulse(dye.selected_at, now).to_string(),
                )
                .map_err(|_| "Fabric Dye pulse alpha")?;
        }
        Ok(())
    }

    pub(super) fn dye_action_feedback(
        &mut self,
        action: &HubInventoryAction,
        result: &HubEconomyResult,
    ) -> bool {
        let (session_id, opening) = match action {
            HubInventoryAction::OpenDye { session_id, .. } => (session_id, true),
            HubInventoryAction::Dye { session_id, .. } => (session_id, false),
            HubInventoryAction::CloseDye { .. } => return true,
            _ => return false,
        };
        let Some(dye) = self.dye.as_mut().filter(|dye| dye.session_id == *session_id) else {
            return true;
        };
        if opening {
            if result.accepted {
                dye.admitted = true;
                dye.pending = false;
            } else {
                self.dye = None;
                self.sound(solomon_sim::audio::AudioCue::BadAction);
            }
        } else if result.accepted {
            dye.pending = false;
            dye.target_item_id = None;
            self.sound(solomon_sim::audio::AudioCue::Dye);
        } else {
            dye.pending = false;
            self.sound(solomon_sim::audio::AudioCue::BadAction);
        }
        true
    }

    fn prepare_dye_actions(
        &self,
        layer: &web_sys::HtmlElement,
        dye: &DyeSelection,
    ) -> Result<(), String> {
        layer.set_attribute("role", "region").map_err(|_| "Fabric Dye role")?;
        layer.set_attribute("aria-label", "Fabric Dye").map_err(|_| "Fabric Dye label")?;
        let controls =
            layer.query_selector_all("[data-panel-action]").map_err(|_| "Fabric Dye controls")?;
        for index in 0..controls.length() {
            let Some(control) =
                controls.item(index).and_then(|node| node.dyn_into::<web_sys::HtmlElement>().ok())
            else {
                continue;
            };
            let action =
                control.get_attribute("data-panel-action").expect("queried Fabric Dye action");
            if dye.pending || dye.closing_at.is_some() {
                control.set_attribute("disabled", "").map_err(|_| "Fabric Dye pending control")?;
            }
            if let Some(row) = action.strip_prefix("dye-swatch:") {
                control
                    .set_attribute("data-native-dye-swatch", row)
                    .map_err(|_| "Fabric Dye swatch")?;
                let selected = row.parse::<usize>().ok() == dye.selected_row;
                control
                    .set_attribute("data-selected-pulse", if selected { "true" } else { "false" })
                    .map_err(|_| "Fabric Dye swatch pulse")?;
                if dye.target_item_id.is_some() || dye.swatch_rows.len() >= 256 {
                    control
                        .set_attribute("disabled", "")
                        .map_err(|_| "Fabric Dye palette admission")?;
                }
            } else if let Some(item_id) = action.strip_prefix("item:") {
                control
                    .set_attribute("data-native-dye-target", item_id)
                    .map_err(|_| "Fabric Dye target")?;
            } else if action == "dye-cloth" || action == "dye-trim" {
                control
                    .set_attribute(
                        "data-native-dye-layer",
                        action.strip_prefix("dye-").expect("dye layer action"),
                    )
                    .map_err(|_| "Fabric Dye layer")?;
                control
                    .set_attribute(
                        "data-native-dye-target",
                        &dye.target_item_id.expect("selected dye target").to_string(),
                    )
                    .map_err(|_| "Fabric Dye layer target")?;
            } else if action == "dye-cancel" {
                control
                    .set_attribute(
                        "data-native-dye-cancel",
                        if dye.target_item_id.is_some() { "layer" } else { "session" },
                    )
                    .map_err(|_| "Fabric Dye cancel phase")?;
            }
        }
        Ok(())
    }

    pub(super) fn begin_unforge(&mut self, item_id: u64, snapshot: &PlayerPanelSnapshot<'_>) {
        let Some(item) =
            solomon_sim::economy::find_inventory_item(&snapshot.economy.backpack, item_id)
        else {
            return;
        };
        if !item.can_unforge() {
            return;
        }
        self.unforge_item = Some(item_id);
        self.notice = Some(PlayerPanelNotice::UnforgeConfirmation);
        self.dirty = true;
    }

    pub fn show_unforge_result(&mut self, outcome: &NativeUnforgeOutcome) {
        let amount = outcome.amount.unwrap_or(0.0);
        let body = match outcome.kind {
            NativeUnforgeOutcomeKind::Experience => format!("+{amount}% faster experience gain"),
            NativeUnforgeOutcomeKind::Fizzle => "No bonus".into(),
            NativeUnforgeOutcomeKind::FullRejuvenation => "Full rejuvenation".into(),
            NativeUnforgeOutcomeKind::Gold => format!("Transmuted to {amount} gold coins"),
            NativeUnforgeOutcomeKind::ManaCost => format!("-{amount} mana cost for all spells"),
            NativeUnforgeOutcomeKind::MaximumHealth => format!("+{amount} to maximum health"),
            NativeUnforgeOutcomeKind::MaximumMana => format!("+{amount} to maximum mana"),
            NativeUnforgeOutcomeKind::MindDredge => {
                "Transmuted to Mind Dredge (+1 skill points at next level)".into()
            }
            NativeUnforgeOutcomeKind::OffensiveDamage => {
                format!("+{amount} damage for all offensive spells")
            }
        };
        let failed = outcome.kind == NativeUnforgeOutcomeKind::Fizzle;
        let title = if failed {
            "FAILED UNFORGING!".into()
        } else {
            format!("{} UNFORGED", outcome.item_name.to_uppercase())
        };
        self.notice = Some(PlayerPanelNotice::UnforgeResult {
            title,
            body,
            summary: if failed {
                "Spellbreaking fizzles!".into()
            } else {
                "Unforging bonus:".into()
            },
            outcome_tint: if failed { 0xff4040 } else { 0x40ff40 },
        });
        self.dirty = true;
    }

    pub(super) fn activate_dye(
        &mut self,
        target: Target,
        snapshot: &PlayerPanelSnapshot<'_>,
        now: f64,
    ) {
        let Some(dye) = &mut self.dye else {
            return;
        };
        if dye.pending || dye.closing_at.is_some() {
            return;
        }
        match target {
            Target::DyeSwatch(row)
                if row < 18 && dye.swatch_rows.len() < 256 && dye.target_item_id.is_none() =>
            {
                dye.swatch_rows.push(row);
                dye.selected_at = Some(now);
                dye.selected_row = Some(row);
                self.sound(solomon_sim::audio::AudioCue::Click);
            }
            Target::DyeCancel => {
                self.cancel_dye(now);
            }
            Target::Item(item_id) if !dye.swatch_rows.is_empty() => {
                if solomon_sim::economy::find_inventory_item(&snapshot.economy.backpack, item_id)
                    .is_some_and(|item| item.is_dyeable_clothing())
                {
                    dye.target_item_id = Some(item_id);
                    self.sound(solomon_sim::audio::AudioCue::Click);
                }
            }
            Target::DyeCloth | Target::DyeTrim => {
                if let Some(target_item_id) = dye.target_item_id {
                    let layer = if target == Target::DyeCloth {
                        NativeDyeLayer::Cloth
                    } else {
                        NativeDyeLayer::Trim
                    };
                    self.actions.push(PlayerPanelAction::Inventory(HubInventoryAction::Dye {
                        session_id: dye.session_id.clone(),
                        target_item_id,
                        layer,
                        swatch_rows: dye.swatch_rows.clone(),
                    }));
                    dye.pending = true;
                }
            }
            _ => {}
        }
    }

    pub(super) fn draw_dye(&self, snapshot: &PlayerPanelSnapshot<'_>) -> Result<(), String> {
        if self.compact {
            return self.draw_compact_dye(snapshot);
        }
        let dye = self.dye.as_ref().expect("dye surface is open");
        let layer = element(&self.document, "div", "panel-dye")?;
        layer
            .set_attribute("data-native-dye-phase", dye_phase(dye))
            .map_err(|_| "Fabric Dye phase")?;
        let curtain = element(&self.document, "div", "panel-dye-curtain")?;
        append(&layer, &curtain)?;
        let panel = element(&self.document, "div", "panel-dye-box")?;
        place(&panel, HudRect::new(480.0, 80.0, 640.0, 360.0))?;
        append(&layer, &panel)?;
        bitmap_text(
            &self.document,
            &layer,
            &NativeUiTextSpec {
                align: NativeUiTextAlign::Center,
                tint: 0xe4c56d,
                ..NativeUiTextSpec::new(NativeUiFontName::Menu, "FABRIC DYE", 800.0, 121.0)
            },
        )?;
        bitmap_text(
            &self.document,
            &layer,
            &NativeUiTextSpec {
                align: NativeUiTextAlign::Center,
                ..NativeUiTextSpec::new(
                    NativeUiFontName::Medium,
                    dye_instruction(dye),
                    800.0,
                    151.0,
                )
            },
        )?;
        for row in 0..18 {
            let rect = native_dye_swatch_rect(row);
            let swatch = button(
                &self.document,
                &layer,
                &format!("dye-swatch:{row}"),
                &format!("Add dye color {}", row + 1),
                rect,
            )?;
            let color = native_dye_mixed_tint(&[row]).expect("native dye row exists");
            swatch
                .style()
                .set_property("background", &format!("#{color:06x}"))
                .map_err(|_| "dye swatch")?;
            swatch
                .style()
                .set_property("border", "2px solid #201c13")
                .map_err(|_| "dye swatch edge")?;
            swatch
                .toggle_attribute_with_force(
                    "disabled",
                    dye.pending
                        || dye.closing_at.is_some()
                        || dye.target_item_id.is_some()
                        || dye.swatch_rows.len() >= 256,
                )
                .map_err(|_| "Fabric Dye swatch admission")?;
            if dye.selected_row == Some(row) {
                let pulse = element(&self.document, "span", "panel-dye-selected-pulse")?;
                pulse
                    .set_attribute("data-native-dye-selected-pulse", "true")
                    .map_err(|_| "Fabric Dye selected pulse")?;
                place(
                    &pulse,
                    HudRect::new(rect.x - 4.0, rect.y - 4.0, rect.width + 8.0, rect.height + 8.0),
                )?;
                append(&layer, &pulse)?;
            }
        }
        let tub = element(&self.document, "div", "panel-dye-tub")?;
        place(&tub, HudRect::new(960.0, 198.0, 96.0, 96.0))?;
        let color = native_dye_mixed_tint(&dye.swatch_rows)
            .map_or_else(|| "rgba(255,255,255,.2)".to_owned(), |color| format!("#{color:06x}"));
        tub.style().set_property("background", &color).map_err(|_| "mixed dye")?;
        append(&layer, &tub)?;
        bitmap_text(
            &self.document,
            &layer,
            &NativeUiTextSpec {
                align: NativeUiTextAlign::Center,
                tint: 0xe4c56d,
                ..NativeUiTextSpec::new(NativeUiFontName::Medium, "DYE TUB", 1008.0, 319.0)
            },
        )?;
        let items = solomon_sim::economy::inventory_items_at_sack_path(
            &snapshot.economy.backpack,
            &dye.path,
        )
        .unwrap_or(&[]);
        for entry in project_inventory_root_slots(items) {
            if !entry.item.is_dyeable_clothing() {
                continue;
            }
            let visible_slot = entry.slot + usize::from(!dye.path.is_empty());
            if visible_slot >= 88 {
                continue;
            }
            let rect = inventory_slot_rect(visible_slot);
            let edge = element(&self.document, "span", "panel-dye-target-edge")?;
            place(
                &edge,
                HudRect::new(rect.x + 2.0, rect.y + 2.0, rect.width - 4.0, rect.height - 4.0),
            )?;
            edge.style()
                .set_property(
                    "border-color",
                    if dye.target_item_id == Some(entry.item.id) { "white" } else { "#d8ba70" },
                )
                .map_err(|_| "Fabric Dye target outline")?;
            append(&layer, &edge)?;
            if dye.target_item_id == Some(entry.item.id) {
                for (action, label, top, height, baseline) in [
                    ("dye-cloth", "CLOTH", rect.y, 40.0, 26.0),
                    ("dye-trim", "TRIM", rect.y + 40.0, 32.0, 22.0),
                ] {
                    let action = button(
                        &self.document,
                        &layer,
                        action,
                        &format!("Dye {}", label.to_lowercase()),
                        HudRect::new(rect.x, top, rect.width, height),
                    )?;
                    action.set_class_name("panel-action panel-dye-layer");
                    action
                        .toggle_attribute_with_force(
                            "disabled",
                            dye.pending || dye.closing_at.is_some(),
                        )
                        .map_err(|_| "Fabric Dye layer admission")?;
                    bitmap_text(
                        &self.document,
                        &action,
                        &NativeUiTextSpec {
                            align: NativeUiTextAlign::Center,
                            ..NativeUiTextSpec::new(NativeUiFontName::Body, label, 36.0, baseline)
                        },
                    )?;
                }
            } else if !dye.swatch_rows.is_empty() && dye.target_item_id.is_none() {
                button(
                    &self.document,
                    &layer,
                    &format!("item:{}", entry.item.id),
                    &format!("Dye {}", entry.item.name),
                    rect,
                )?;
            }
        }
        let label = if dye.target_item_id.is_some() { "CANCEL" } else { "DONE" };
        let cancel = button(
            &self.document,
            &layer,
            "dye-cancel",
            if dye.target_item_id.is_some() { "Cancel layer choice" } else { "Done Fabric Dye" },
            HudRect::new(690.0, 390.0, 220.0, 44.0),
        )?;
        cancel.class_list().add_1("panel-dye-cancel").map_err(|_| "Fabric Dye Done art")?;
        cancel
            .toggle_attribute_with_force("disabled", dye.pending || dye.closing_at.is_some())
            .map_err(|_| "Fabric Dye cancel admission")?;
        bitmap_text(
            &self.document,
            &cancel,
            &NativeUiTextSpec {
                align: NativeUiTextAlign::Center,
                ..NativeUiTextSpec::new(NativeUiFontName::Menu, label, 110.0, 31.0)
            },
        )?;
        self.prepare_dye_actions(&layer, dye)?;
        append(&self.content, &layer)
    }

    fn draw_compact_dye(&self, snapshot: &PlayerPanelSnapshot<'_>) -> Result<(), String> {
        let dye = self.dye.as_ref().expect("open dye surface");
        let layer = element(&self.document, "div", "panel-dye panel-compact-dye")?;
        layer
            .set_attribute("data-native-dye-phase", dye_phase(dye))
            .map_err(|_| "compact Fabric Dye phase")?;
        let sheet = element(&self.document, "div", "panel-compact-dye-sheet")?;
        let width = f64::from(self.viewport_size.0 - 24).min(640.0);
        let center = width / 2.0;
        let mut items = Vec::new();
        let source_items = solomon_sim::economy::inventory_items_at_sack_path(
            &snapshot.economy.backpack,
            &dye.path,
        )
        .unwrap_or(&[]);
        for entry in project_inventory_root_slots(source_items) {
            if !entry.item.is_dyeable_clothing() {
                continue;
            }
            if dye.target_item_id.is_some_and(|id| id != entry.item.id) {
                continue;
            }
            items.push(entry);
        }
        let item_rows = items.len().div_ceil(3);
        let actions_top = 400.0 + item_rows as f64 * 100.0;
        let height = actions_top + if dye.target_item_id.is_some() { 132.0 } else { 78.0 };
        style(&sheet, &format!("width:{width}px;height:{height}px"))?;
        append(&layer, &sheet)?;
        let leather = self.ui.tiles(&self.document, 49, width, height)?;
        append(&sheet, &leather)?;
        let frame = self.ui.nine_slice(&self.document, 17, width, height)?;
        append(&sheet, &frame)?;
        bitmap_text(
            &self.document,
            &sheet,
            &NativeUiTextSpec {
                tint: 0xe4c56d,
                ..NativeUiTextSpec::new(NativeUiFontName::Menu, "FABRIC DYE", center, 48.0)
            },
        )?;
        let instruction = dye_instruction(dye);
        bitmap_text(
            &self.document,
            &sheet,
            &NativeUiTextSpec {
                max_width: Some(width - 36.0),
                ..NativeUiTextSpec::new(NativeUiFontName::Medium, instruction, center, 88.0)
            },
        )?;
        for row in 0..18 {
            let swatch = button(
                &self.document,
                &sheet,
                &format!("dye-swatch:{row}"),
                &format!("Add dye color {}", row + 1),
                HudRect::new(
                    center - 142.0 + (row % 6) as f64 * 48.0,
                    128.0 + (row / 6) as f64 * 50.0,
                    44.0,
                    44.0,
                ),
            )?;
            let color = native_dye_mixed_tint(&[row]).expect("native dye color");
            style(
                &swatch,
                &format!(
                    "position:absolute;left:{}px;top:{}px;width:44px;height:44px;background:#{color:06x};border:2px solid #201c13",
                    center - 142.0 + (row % 6) as f64 * 48.0,
                    128.0 + (row / 6) as f64 * 50.0
                ),
            )?;
            if dye.selected_row == Some(row) {
                let pulse = element(&self.document, "span", "panel-dye-selected-pulse")?;
                pulse
                    .set_attribute("data-native-dye-selected-pulse", "true")
                    .map_err(|_| "compact Fabric Dye pulse")?;
                place(
                    &pulse,
                    HudRect::new(
                        center - 146.0 + (row % 6) as f64 * 48.0,
                        124.0 + (row / 6) as f64 * 50.0,
                        52.0,
                        52.0,
                    ),
                )?;
                append(&sheet, &pulse)?;
            }
        }
        let tub = element(&self.document, "div", "panel-dye-tub")?;
        place(&tub, HudRect::new(center - 36.0, 294.0, 72.0, 72.0))?;
        let color = native_dye_mixed_tint(&dye.swatch_rows)
            .map_or_else(|| "rgba(255,255,255,.2)".to_owned(), |color| format!("#{color:06x}"));
        tub.style().set_property("background", &color).map_err(|_| "compact mixed dye")?;
        append(&sheet, &tub)?;
        bitmap_text(
            &self.document,
            &sheet,
            &NativeUiTextSpec {
                tint: 0xe4c56d,
                ..NativeUiTextSpec::new(NativeUiFontName::Medium, "DYE TUB", center, 390.0)
            },
        )?;
        for (index, entry) in items.iter().enumerate() {
            let target = button(
                &self.document,
                &sheet,
                &format!("item:{}", entry.item.id),
                &format!("Dye {}", entry.item.name),
                HudRect::new(
                    center - 120.0 + (index % 3) as f64 * 84.0,
                    410.0 + (index / 3) as f64 * 100.0,
                    72.0,
                    72.0,
                ),
            )?;
            let frame = self.inventory.raw_sprite(&self.document, 10)?;
            append(&target, &frame)?;
            self.draw_item(&target, entry.item, 36.0, 36.0, 1, snapshot.mod_assets)?;
            target
                .set_attribute(
                    "data-selected",
                    if dye.target_item_id == Some(entry.item.id) { "true" } else { "false" },
                )
                .map_err(|_| "compact dye target")?;
            if dye.swatch_rows.is_empty() {
                target.set_attribute("disabled", "").map_err(|_| "dye requires a color")?;
            }
        }
        if dye.target_item_id.is_some() {
            for (action, label, left) in
                [("dye-cloth", "CLOTH", 20.0), ("dye-trim", "TRIM", center + 8.0)]
            {
                let button = native_button(&self.document, label, action)?;
                button
                    .set_attribute("data-panel-action", action)
                    .map_err(|_| "compact dye layer")?;
                place(&button, HudRect::new(left, actions_top + 16.0, center - 28.0, 44.0))?;
                append(&sheet, &button)?;
            }
        }
        let cancel = native_button(
            &self.document,
            if dye.target_item_id.is_some() { "CANCEL" } else { "DONE" },
            "dye-cancel",
        )?;
        cancel
            .set_attribute("data-panel-action", "dye-cancel")
            .map_err(|_| "compact dye cancel")?;
        place(&cancel, HudRect::new(20.0, height - 64.0, width - 40.0, 44.0))?;
        append(&sheet, &cancel)?;
        self.prepare_dye_actions(&layer, dye)?;
        append(&self.overlay, &layer)
    }
}

fn dye_phase(dye: &DyeSelection) -> &'static str {
    if dye.target_item_id.is_some() {
        "layer"
    } else if dye.swatch_rows.is_empty() {
        "mix"
    } else {
        "target"
    }
}

fn dye_instruction(dye: &DyeSelection) -> &'static str {
    if dye.pending {
        if dye.admitted { "DYEING..." } else { "OPENING..." }
    } else if dye.target_item_id.is_some() {
        "CHOOSE DYE CLOTH OR DYE TRIM"
    } else if dye.swatch_rows.is_empty() {
        "MIX ONE OR MORE COLORS"
    } else {
        "CHOOSE A HAT OR ROBE FROM YOUR BACKPACK"
    }
}
