//! Fabric Dye and Unforge. The canonical economy retains mixing, admission,
//! randomness and bonuses; GPU controls collect the same transaction arguments.
use super::chrome::PanelCanvas;
use super::notices::PlayerPanelNotice;
use super::{DyeSelection, PlayerPanelAction, PlayerPanelSnapshot, PlayerPanels, Target};
use crate::gpu_ui::UiFrame;
use solomon_sim::economy::{
    HubEconomyResult, HubInventoryAction, NativeDyeLayer, NativeUnforgeOutcome,
    NativeUnforgeOutcomeKind, native_dye_mixed_tint, project_inventory_root_slots,
};
use solomon_sim::hud::HudRect;
use solomon_sim::native_ui_text::{NativeUiFontName, NativeUiTextSpec};
use solomon_sim::player_panels::dye::{
    native_dye_modal_opacity, native_dye_selected_pulse, native_dye_swatch_rect,
};
use solomon_sim::player_panels::inventory_slot_rect;
pub(super) const DYE_MODAL_SCOPE: &str = "fabric-dye";

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
            }
            return;
        }
        if dye.admitted && snapshot.dye_session_id != Some(dye.session_id.as_str()) {
            self.dye = None;

            return;
        }
        if !dye.pending
            && dye.target_item_id.is_some_and(|id| {
                solomon_sim::economy::find_inventory_item(&snapshot.economy.backpack, id)
                    .is_none_or(|item| !item.is_dyeable_clothing())
            })
        {
            dye.target_item_id = None;
        }
        if dye.selected_at.is_some_and(|at| now - at >= 200.0) {
            dye.selected_at = None;
            dye.selected_row = None;
        }
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
    pub(super) fn paint_dye_modal(
        &mut self,
        frame: &mut UiFrame<'_>,
        s: &PlayerPanelSnapshot<'_>,
    ) -> Result<(), String> {
        let compact = self.compact;
        let scale = if compact { 1.0 } else { self.view_scale as f32 };
        let origin = if compact {
            egui::Pos2::ZERO
        } else {
            egui::pos2(self.stage_origin.x as f32, self.stage_origin.y as f32)
        };
        self.modal_canvas(frame, DYE_MODAL_SCOPE, scale, origin, 0.75, |panels, c| {
            if compact { panels.draw_compact_dye(c, s) } else { panels.draw_dye(c, s) }
        })
    }
    fn dye_canvas<'a, 'gpu>(&self, c: &PanelCanvas<'a, 'gpu>) -> PanelCanvas<'a, 'gpu> {
        let dye = self.dye.as_ref().expect("dye owner");
        c.faded(native_dye_modal_opacity(dye.opened_at, dye.closing_at, self.now) as f32)
            .admitted(!dye.pending && dye.closing_at.is_none())
    }
    fn dye_swatch(&self, c: &PanelCanvas<'_, '_>, row: usize, r: HudRect) -> Result<(), String> {
        let dye = self.dye.as_ref().expect("dye owner");
        let swatch = c
            .admitted(dye.target_item_id.is_none() && dye.swatch_rows.len() < 256)
            .button(&format!("dye-swatch:{row}"), &format!("Add dye color {}", row + 1), r);
        swatch.fill(
            HudRect::new(0.0, 0.0, r.width, r.height),
            native_dye_mixed_tint(&[row]).expect("native swatch"),
            1.0,
        );
        swatch.stroke(HudRect::new(0.0, 0.0, r.width, r.height), 0x201c13, 2.0);
        if dye.selected_row == Some(row) {
            c.faded(native_dye_selected_pulse(dye.selected_at, self.now) as f32).stroke(
                HudRect::new(r.x - 4.0, r.y - 4.0, r.width + 8.0, r.height + 8.0),
                0xffffff,
                3.0,
            );
        }
        Ok(())
    }
    fn draw_dye(&self, c: &PanelCanvas<'_, '_>, s: &PlayerPanelSnapshot<'_>) -> Result<(), String> {
        let dye = self.dye.as_ref().expect("dye owner");
        let c = self.dye_canvas(c);
        let box_rect = HudRect::new(480.0, 80.0, 640.0, 360.0);
        c.fill(box_rect, 0x090908, 0.96);
        c.stroke(box_rect, 0xd8ba70, 3.0);
        c.stroke(HudRect::new(488.0, 88.0, 624.0, 344.0), 0xeadab3, 1.0);
        c.semantic_value(
            "panel:dye",
            "Fabric Dye",
            "dialog",
            box_rect,
            &dye_presentation(dye, self.now),
        );
        c.text(&NativeUiTextSpec {
            tint: 0xe4c56d,
            ..NativeUiTextSpec::new(NativeUiFontName::Menu, "FABRIC DYE", 800.0, 121.0)
        });
        c.text(&NativeUiTextSpec::new(
            NativeUiFontName::Medium,
            dye_instruction(dye),
            800.0,
            151.0,
        ));
        for row in 0..18 {
            self.dye_swatch(&c, row, native_dye_swatch_rect(row))?;
        }
        let tub = HudRect::new(960.0, 198.0, 96.0, 96.0);
        c.fill(
            tub,
            native_dye_mixed_tint(&dye.swatch_rows).unwrap_or(0xffffff),
            if dye.swatch_rows.is_empty() { 0.2 } else { 1.0 },
        );
        c.stroke(tub, 0xeadab3, 3.0);
        c.text(&NativeUiTextSpec {
            tint: 0xe4c56d,
            ..NativeUiTextSpec::new(NativeUiFontName::Medium, "DYE TUB", 1008.0, 319.0)
        });
        let items =
            solomon_sim::economy::inventory_items_at_sack_path(&s.economy.backpack, &dye.path)
                .unwrap_or(&[]);
        for entry in project_inventory_root_slots(items) {
            if !entry.item.is_dyeable_clothing() {
                continue;
            }
            let visible = entry.slot + usize::from(!dye.path.is_empty());
            if visible >= 88 {
                continue;
            }
            let r = inventory_slot_rect(visible);
            c.stroke(
                HudRect::new(r.x + 2.0, r.y + 2.0, r.width - 4.0, r.height - 4.0),
                if dye.target_item_id == Some(entry.item.id) { 0xffffff } else { 0xd8ba70 },
                3.0,
            );
            if dye.target_item_id == Some(entry.item.id) {
                for (action, label, top, height, baseline) in [
                    ("dye-cloth", "CLOTH", r.y, 40.0, 26.0),
                    ("dye-trim", "TRIM", r.y + 40.0, 32.0, 22.0),
                ] {
                    let target = c.button(
                        action,
                        &format!("Dye {}", label.to_lowercase()),
                        HudRect::new(r.x, top, r.width, height),
                    );
                    target.fill(HudRect::new(0.0, 0.0, r.width, height), 0, 0.38);
                    target.stroke(HudRect::new(0.0, 0.0, r.width, height), 0xffffff, 2.0);
                    target.text(&NativeUiTextSpec::new(
                        NativeUiFontName::Body,
                        label,
                        36.0,
                        baseline,
                    ));
                }
            } else if !dye.swatch_rows.is_empty() && dye.target_item_id.is_none() {
                c.button(
                    &format!("item:{}", entry.item.id),
                    &format!("Dye {}", entry.item.name),
                    r,
                );
            }
        }
        let label = if dye.target_item_id.is_some() { "CANCEL" } else { "DONE" };
        let cancel = c.button(
            "dye-cancel",
            if dye.target_item_id.is_some() { "Cancel layer choice" } else { "Done Fabric Dye" },
            HudRect::new(690.0, 390.0, 220.0, 44.0),
        );
        cancel.fill(HudRect::new(0.0, 0.0, 220.0, 44.0), 0x191916, 1.0);
        cancel.stroke(HudRect::new(0.0, 0.0, 220.0, 44.0), 0xd8ba70, 2.0);
        cancel.text(&NativeUiTextSpec::new(NativeUiFontName::Menu, label, 110.0, 31.0));
        Ok(())
    }
    fn draw_compact_dye(
        &self,
        c: &PanelCanvas<'_, '_>,
        s: &PlayerPanelSnapshot<'_>,
    ) -> Result<(), String> {
        let dye = self.dye.as_ref().expect("dye owner");
        let c = self.dye_canvas(c);
        let width = (self.viewport_size.0 as f64 - 24.0).min(640.0);
        let center = width / 2.0;
        let items =
            solomon_sim::economy::inventory_items_at_sack_path(&s.economy.backpack, &dye.path)
                .unwrap_or(&[]);
        let items = project_inventory_root_slots(items)
            .into_iter()
            .filter(|entry| {
                entry.item.is_dyeable_clothing()
                    && !dye.target_item_id.is_some_and(|id| id != entry.item.id)
            })
            .collect::<Vec<_>>();
        let actions_top = 400.0 + items.len().div_ceil(3) as f64 * 100.0;
        let height = actions_top + if dye.target_item_id.is_some() { 132.0 } else { 78.0 };
        let mut child = super::chrome::child_ui(
            c.ui,
            "compact-dye",
            egui::Rect::from_min_size(
                egui::pos2((self.viewport_size.0 as f32 - width as f32) / 2.0, 12.0),
                egui::vec2(width as f32, self.viewport_size.1 as f32 - 24.0),
            ),
        );
        let mut result = Ok(());
        egui::ScrollArea::vertical().id_salt("fabric-dye-scroll").show(&mut child, |ui| {
            let origin = ui.cursor().min;
            let sheet = c.on_ui(ui).at(origin.x as f64, origin.y as f64);
            let paint = (|| {
                sheet.tiles(&self.ui, 49, HudRect::new(0.0, 0.0, width, height), 1.0, 0xffffff)?;
                sheet.native_frame(
                    &self.ui,
                    17,
                    HudRect::new(0.0, 0.0, width, height),
                    true,
                    0xffffff,
                )?;
                sheet.semantic_value(
                    "panel:dye",
                    "Fabric Dye",
                    "dialog",
                    HudRect::new(0.0, 0.0, width, height),
                    &dye_presentation(dye, self.now),
                );
                sheet.text(&NativeUiTextSpec {
                    tint: 0xe4c56d,
                    ..NativeUiTextSpec::new(NativeUiFontName::Menu, "FABRIC DYE", center, 48.0)
                });
                sheet.text(&NativeUiTextSpec {
                    max_width: Some(width - 36.0),
                    ..NativeUiTextSpec::new(
                        NativeUiFontName::Medium,
                        dye_instruction(dye),
                        center,
                        88.0,
                    )
                });
                for row in 0..18 {
                    self.dye_swatch(
                        &sheet,
                        row,
                        HudRect::new(
                            center - 142.0 + (row % 6) as f64 * 48.0,
                            128.0 + (row / 6) as f64 * 50.0,
                            44.0,
                            44.0,
                        ),
                    )?;
                }
                let tub = HudRect::new(center - 36.0, 294.0, 72.0, 72.0);
                sheet.fill(
                    tub,
                    native_dye_mixed_tint(&dye.swatch_rows).unwrap_or(0xffffff),
                    if dye.swatch_rows.is_empty() { 0.2 } else { 1.0 },
                );
                sheet.stroke(tub, 0xeadab3, 3.0);
                sheet.text(&NativeUiTextSpec {
                    tint: 0xe4c56d,
                    ..NativeUiTextSpec::new(NativeUiFontName::Medium, "DYE TUB", center, 390.0)
                });
                for (index, entry) in items.iter().enumerate() {
                    let r = HudRect::new(
                        center - 120.0 + (index % 3) as f64 * 84.0,
                        410.0 + (index / 3) as f64 * 100.0,
                        72.0,
                        72.0,
                    );
                    let target = sheet.admitted(!dye.swatch_rows.is_empty()).button(
                        &format!("item:{}", entry.item.id),
                        &format!("Dye {}", entry.item.name),
                        r,
                    );
                    target.raw(
                        &self.inventory,
                        10,
                        HudRect::new(0.0, 0.0, 72.0, 72.0),
                        0xffffff,
                    )?;
                    self.draw_item(&target, entry.item, 36.0, 36.0, 1)?;
                    if dye.target_item_id == Some(entry.item.id) {
                        target.stroke(HudRect::new(0.0, 0.0, 72.0, 72.0), 0xffffff, 2.0);
                    }
                }
                if dye.target_item_id.is_some() {
                    for (action, label, left) in
                        [("dye-cloth", "CLOTH", 20.0), ("dye-trim", "TRIM", center + 8.0)]
                    {
                        sheet.native_button(
                            &self.ui,
                            action,
                            label,
                            HudRect::new(left, actions_top + 16.0, center - 28.0, 44.0),
                        )?;
                    }
                }
                sheet.native_button(
                    &self.ui,
                    "dye-cancel",
                    if dye.target_item_id.is_some() { "CANCEL" } else { "DONE" },
                    HudRect::new(20.0, height - 64.0, width - 40.0, 44.0),
                )?;
                Ok(())
            })();
            result = paint;
            ui.set_min_size(egui::vec2(width as f32, height as f32));
        });
        result
    }
}
fn dye_presentation(dye: &DyeSelection, now: f64) -> String {
    let phase = if dye.target_item_id.is_some() {
        "layer"
    } else if dye.swatch_rows.is_empty() {
        "mix"
    } else {
        "target"
    };
    serde_json::json!({
        "phase": phase,
        "opacity": native_dye_modal_opacity(dye.opened_at, dye.closing_at, now),
    })
    .to_string()
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
