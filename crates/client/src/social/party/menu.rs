//! GPU painter for the canonical 1600×900 stock Party plan.

use egui::{Color32, Id, Rect, Sense, Ui, vec2};
use solomon_sim::hud::HudRect;
use solomon_sim::social::party_ui::{
    PARTY_MENU_FRAME, PARTY_MENU_VIEWPORT, PARTY_MENU_WHEEL_STEP, PARTY_VISIBILITIES,
    PartyMenuMember, PartyMenuPlan, PartyMenuRequest, PartyMenuSpec, PartyMenuTag,
    clamp_party_menu_scroll, plan_party_menu, visibility_id,
};
use solomon_sim::social::{PartyVisibility, party_member_display_name, party_menu_leave_label};
use solomon_sim::ui_text::UiTextSpec;

use super::super::{PartyTab, SocialUi, gpu};
use crate::gpu_ui::{UiFrame, message_frame_scaled};

/// One frame of the Party menu: the selected tab's plan, the party's join code and
/// visibility, and the menu's placement on the 1600×900 stage.
struct PartyMenu {
    tab: PartyTab,
    plan: PartyMenuPlan,
    code: Option<String>,
    visibility: PartyVisibility,
    origin: egui::Pos2,
    scale: f32,
    viewport: Rect,
}

impl SocialUi {
    pub(in crate::social) fn paint_party_menu(
        &mut self,
        frame: &mut UiFrame<'_>,
        ui: &mut Ui,
        tab: PartyTab,
    ) -> Result<(), String> {
        let Some(menu) = self.party_menu(tab) else { return Ok(()) };
        let panel = gpu::rect(PARTY_MENU_FRAME, menu.origin, menu.scale);
        ui.interact(panel, Id::unique("social/party-panel-block"), egui::Sense::CLICK);
        message_frame_scaled(frame, ui.painter(), &self.ui, panel, menu.scale)?;
        frame.record_rect("social/party-menu", "PARTY", "dialog", panel, true);
        let mut activated = None;
        self.paint_party_tabs(frame, ui, &menu, &mut activated)?;
        if tab == PartyTab::Members {
            self.paint_member_rows(frame, ui, &menu)?;
        }
        for bounds in &menu.plan.settings_rows {
            gpu::brackets(
                frame,
                ui.painter(),
                &self.ui,
                gpu::rect(*bounds, menu.origin, menu.scale),
                menu.scale,
            )?;
        }
        if tab == PartyTab::Mods {
            self.paint_party_mods(frame, ui, menu.origin, menu.scale)?;
        }
        paint_party_texts(frame, ui, &menu);
        self.paint_party_actions(frame, ui, &menu, &mut activated)?;
        if let Some(action) = activated {
            if action == "copy" {
                if let Some(code) = menu.code {
                    self.clipboard.copy(code);
                }
            } else {
                self.activate(match action.as_str() {
                    "generate" => "party-generate",
                    _ => &action,
                })?;
            }
        }
        Ok(())
    }

    /// The selected tab's plan from the party snapshot, or `None` outside a party.
    fn party_menu(&self, tab: PartyTab) -> Option<PartyMenu> {
        let snapshot = self.snapshot.as_ref()?;
        let state = snapshot.party_state.as_ref()?;
        let leader = state.party.leader_player_id == snapshot.local_player_id;
        let members: Vec<_> = state
            .party
            .member_player_ids
            .iter()
            .map(|id| {
                let mut tags = Vec::new();
                if *id == snapshot.local_player_id {
                    tags.push(PartyMenuTag::You);
                }
                if *id == state.party.leader_player_id {
                    tags.push(PartyMenuTag::Leader);
                }
                if state.party_roster.iter().any(|row| row.player_id == *id && !row.connected) {
                    tags.push(PartyMenuTag::Offline);
                }
                PartyMenuMember {
                    id: id.clone(),
                    name: party_member_display_name(state, id).to_owned(),
                    removable: leader && *id != snapshot.local_player_id,
                    tags,
                }
            })
            .collect();
        let requests: Vec<_> = if leader {
            state
                .join_requests
                .iter()
                .map(|request| PartyMenuRequest {
                    id: request.id.clone(),
                    name: request.requester.display_name.clone(),
                })
                .collect()
        } else {
            Vec::new()
        };
        let plan = plan_party_menu(&PartyMenuSpec {
            code: if leader { state.party.join_code.as_deref() } else { None },
            copied: self.party_ui.copied,
            error: self.party_status.as_deref(),
            leader,
            leave_label: party_menu_leave_label(
                state.party.member_player_ids.len(),
                snapshot.session_kind,
            ),
            members: &members,
            requests: &requests,
            scroll_y: self.party_ui.scroll_y,
            selected_tab: tab,
            visibility: state.party.visibility,
            visibility_options: if snapshot.directory_available {
                &PARTY_VISIBILITIES
            } else {
                &[]
            },
        });
        let code = state.party.join_code.clone();
        let visibility = state.party.visibility;
        let (origin, scale) = gpu::stage(self.size);
        let viewport = gpu::rect(PARTY_MENU_VIEWPORT, origin, scale);
        Some(PartyMenu { tab, plan, code, visibility, origin, scale, viewport })
    }

    /// The tab headers; arrow keys on a focused tab select and focus its neighbor.
    fn paint_party_tabs(
        &self,
        frame: &mut UiFrame<'_>,
        ui: &mut Ui,
        menu: &PartyMenu,
        activated: &mut Option<String>,
    ) -> Result<(), String> {
        let PartyMenu { tab, origin, scale, .. } = *menu;
        for (index, candidate) in PartyTab::ALL.iter().enumerate() {
            let bounds = gpu::rect(
                HudRect::new(431.0 + index as f64 * 246.0, 190.0, 246.0, 69.0),
                origin,
                scale,
            );
            let response =
                ui.interact(bounds, Id::unique(("social/tab", candidate.id())), Sense::click());
            frame.record(
                &format!("social/party-{}", candidate.id()),
                candidate.label(),
                "tab",
                &response,
            );
            let selected = *candidate == tab;
            let top = if selected { 0.0 } else { 8.0 };
            let height = if selected { 65.0 } else { 51.0 };
            for (source, target) in [
                (
                    Rect::from_min_size(egui::Pos2::ZERO, vec2(34.0, height)),
                    Rect::from_min_size(
                        bounds.min + vec2(0.0, top) * scale,
                        vec2(34.0, height) * scale,
                    ),
                ),
                (
                    Rect::from_min_max(egui::pos2(34.0, 0.0), egui::pos2(0.0, height)),
                    Rect::from_min_size(
                        egui::pos2(bounds.right() - 34.0 * scale, bounds.top() + top * scale),
                        vec2(34.0, height) * scale,
                    ),
                ),
                (
                    Rect::from_min_size(egui::pos2(32.3, 0.0), vec2(1.7, height)),
                    Rect::from_min_size(
                        bounds.min + vec2(34.0, top) * scale,
                        vec2(178.0, height) * scale,
                    ),
                ),
            ] {
                self.ui.paint_slice(frame, ui.painter(), 13, source, target, Color32::WHITE)?;
            }
            let mut spec = UiTextSpec::new(
                solomon_sim::ui_text::UiFontName::Menu,
                candidate.label(),
                554.0 + index as f64 * 246.0,
                if selected { 235.0 } else { 243.0 },
            );
            spec.tint = if selected { 0xffffff } else { 0xaaa2a6 };
            gpu::paint_stock_text(frame, ui.painter(), &spec, origin, scale);
            gpu::focus_outline(frame, ui, &response, scale);
            if response.clicked() {
                *activated = Some(format!("party-{}", candidate.id()));
            }
            if response.has_focus() {
                let direction = ui.input_mut(|input| {
                    if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft) {
                        -1
                    } else if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight) {
                        1
                    } else {
                        0
                    }
                });
                if direction != 0 {
                    let next =
                        (index as i32 + direction).rem_euclid(PartyTab::ALL.len() as i32) as usize;
                    *activated = Some(format!("party-{}", PartyTab::ALL[next].id()));
                    ui.memory_mut(|memory| {
                        memory.request_focus(Id::unique(("social/tab", PartyTab::ALL[next].id())))
                    });
                }
            }
        }
        Ok(())
    }

    /// Scrolls the Members list by wheel, drag and keys, then brackets its visible rows.
    fn paint_member_rows(
        &mut self,
        frame: &mut UiFrame<'_>,
        ui: &mut Ui,
        menu: &PartyMenu,
    ) -> Result<(), String> {
        let PartyMenu { origin, scale, viewport, .. } = *menu;
        let plan = &menu.plan;
        let scroll = ui.interact(viewport, Id::unique("social/party-scroll"), Sense::drag());
        let wheel =
            if scroll.hovered() { ui.input(|input| input.smooth_scroll_delta.y) } else { 0.0 };
        let mut requested = self.party_ui.scroll_y;
        if wheel != 0.0 {
            requested -= f64::from(wheel.signum()) * PARTY_MENU_WHEEL_STEP;
        }
        if scroll.dragged() {
            requested -= f64::from(scroll.drag_delta().y / scale);
        }
        if scroll.has_focus() {
            requested += ui.input_mut(|input| {
                if input.consume_key(egui::Modifiers::NONE, egui::Key::PageDown) {
                    PARTY_MENU_VIEWPORT.height
                } else if input.consume_key(egui::Modifiers::NONE, egui::Key::PageUp) {
                    -PARTY_MENU_VIEWPORT.height
                } else if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) {
                    PARTY_MENU_WHEEL_STEP
                } else if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) {
                    -PARTY_MENU_WHEEL_STEP
                } else {
                    0.0
                }
            });
        }
        self.party_ui.scroll_y = clamp_party_menu_scroll(requested, plan.content_height);
        let painter = ui.painter().with_clip_rect(viewport);
        for row in &plan.rows {
            if row.visible_bounds.is_some() {
                gpu::brackets(
                    frame,
                    &painter,
                    &self.ui,
                    gpu::rect(row.bounds, origin, scale),
                    scale,
                )?;
            }
        }
        Ok(())
    }

    /// The plan's buttons and visibility options; the tab headers are painted separately.
    fn paint_party_actions(
        &mut self,
        frame: &mut UiFrame<'_>,
        ui: &mut Ui,
        menu: &PartyMenu,
        activated: &mut Option<String>,
    ) -> Result<(), String> {
        let PartyMenu { origin, scale, viewport, visibility, .. } = *menu;
        let plan = &menu.plan;
        for action in &plan.actions {
            if action.id.starts_with("tab:") {
                continue;
            }
            let bounds = gpu::rect(action.bounds, origin, scale);
            let clipped = plan.row_actions.iter().any(|row| row.id == action.id);
            if clipped && !bounds.intersects(viewport) {
                continue;
            }
            let old_clip = ui.clip_rect();
            if clipped {
                ui.set_clip_rect(viewport);
            }
            let response = if let Some(id) = action.id.strip_prefix("visibility:") {
                let selected = id == visibility_id(visibility);
                let response = ui.interact(
                    bounds,
                    Id::unique(("social", &action.id)),
                    if action.disabled { Sense::hover() } else { Sense::click() },
                );
                self.ui.paint_raw(
                    frame,
                    ui.painter(),
                    if selected { 105 } else { 67 },
                    bounds,
                    Color32::WHITE,
                )?;
                if let Some(label) =
                    plan.texts.iter().find(|text| text.label == format!("{}:label", action.id))
                {
                    gpu::paint_stock_text(
                        frame,
                        ui.painter(),
                        &UiTextSpec {
                            scale: label.scale,
                            tint: label.tint,
                            ..UiTextSpec::new(label.font, &label.text, label.x, label.y)
                        },
                        origin,
                        scale,
                    );
                }
                frame.record(&format!("social/{}", action.id), &action.label, "radio", &response);
                gpu::focus_outline(frame, ui, &response, scale);
                if response.has_focus() {
                    let direction = ui.input_mut(|input| {
                        if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft)
                            || input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp)
                        {
                            -1
                        } else if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight)
                            || input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown)
                        {
                            1
                        } else {
                            0
                        }
                    });
                    if direction != 0 {
                        let index = PARTY_VISIBILITIES
                            .iter()
                            .position(|value| *value == visibility)
                            .unwrap_or(0);
                        let next =
                            PARTY_VISIBILITIES[(index as i32 + direction).rem_euclid(3) as usize];
                        *activated = Some(format!("visibility:{}", visibility_id(next)));
                    }
                }
                response
            } else {
                gpu::stock_button(frame, ui, &action.id, &action.label, bounds, !action.disabled)?
            };
            if self.focus_modal && action.id == "close" {
                response.request_focus();
                self.focus_modal = false;
            }
            if response.gained_focus() && clipped {
                if bounds.top() < viewport.top() {
                    self.party_ui.scroll_y -= f64::from((viewport.top() - bounds.top()) / scale);
                } else if bounds.bottom() > viewport.bottom() {
                    self.party_ui.scroll_y +=
                        f64::from((bounds.bottom() - viewport.bottom()) / scale);
                }
                self.party_ui.scroll_y =
                    clamp_party_menu_scroll(self.party_ui.scroll_y, plan.content_height);
            }
            if action.id == "copy"
                && !action.disabled
                && let Some(code) = &menu.code
            {
                self.clipboard.target(&response, code.clone());
                frame.record_value("social/copy", code, None);
            }
            if response.clicked() && !action.disabled {
                *activated = Some(action.id.clone());
            }
            ui.set_clip_rect(old_clip);
        }
        Ok(())
    }
    /// The user-directed GPU completion replaces the inherited empty Mods tab with
    /// the actual admitted package metadata, while retaining stock Party chrome.
    fn paint_party_mods(
        &mut self,
        frame: &mut UiFrame<'_>,
        ui: &mut Ui,
        origin: egui::Pos2,
        scale: f32,
    ) -> Result<(), String> {
        use solomon_sim::social::party_ui::{fit_party_menu_text, party_menu_name_font};
        use solomon_sim::ui_text::{UiFontName, UiTextAlign, measure_ui_text};
        let viewport = gpu::rect(PARTY_MENU_VIEWPORT, origin, scale);
        let painter = ui.painter().with_clip_rect(viewport);
        if self.party_mods.is_empty() {
            gpu::paint_stock_text(
                frame,
                &painter,
                &UiTextSpec {
                    tint: 0xaaa2a6,
                    ..UiTextSpec::new(UiFontName::Medium, "No active mods", 800.0, 318.0)
                },
                origin,
                scale,
            );
            frame.record_rect(
                "social/party-mods-empty",
                "No active mods",
                "status",
                viewport,
                true,
            );
            return Ok(());
        }
        let scroll = ui.interact(viewport, Id::unique("social/party-mod-scroll"), Sense::drag());
        let content_height = self.party_mods.len() as f64 * 66.0;
        let wheel =
            if scroll.hovered() { ui.input(|input| input.smooth_scroll_delta.y) } else { 0.0 };
        if wheel != 0.0 {
            self.party_ui.scroll_y -= f64::from(wheel.signum()) * PARTY_MENU_WHEEL_STEP;
        }
        if scroll.dragged() {
            self.party_ui.scroll_y -= f64::from(scroll.drag_delta().y / scale);
        }
        if scroll.has_focus() {
            self.party_ui.scroll_y += ui.input_mut(|input| {
                if input.consume_key(egui::Modifiers::NONE, egui::Key::PageDown) {
                    367.0
                } else if input.consume_key(egui::Modifiers::NONE, egui::Key::PageUp) {
                    -367.0
                } else if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) {
                    PARTY_MENU_WHEEL_STEP
                } else if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) {
                    -PARTY_MENU_WHEEL_STEP
                } else {
                    0.0
                }
            });
        }
        self.party_ui.scroll_y = clamp_party_menu_scroll(self.party_ui.scroll_y, content_height);
        for (index, package) in self.party_mods.iter().enumerate() {
            let y = 278.0 + index as f64 * 66.0 - self.party_ui.scroll_y;
            let bounds = gpu::rect(HudRect::new(460.0, y, 680.0, 56.0), origin, scale);
            if !bounds.intersects(viewport) {
                continue;
            }
            gpu::brackets(frame, &painter, &self.ui, bounds, scale)?;
            let version = format!("v{}", package.version);
            let version_width = measure_ui_text(&version, UiFontName::Medium, 0.9);
            let font = party_menu_name_font(&package.name);
            let name = fit_party_menu_text(
                &package.name,
                font,
                (628.0 - version_width - 20.0).max(1.0),
                0.85,
            );
            gpu::paint_stock_text(
                frame,
                &painter,
                &UiTextSpec {
                    align: UiTextAlign::Left,
                    scale: 0.85,
                    ..UiTextSpec::new(font, &name, 486.0, y + 36.0)
                },
                origin,
                scale,
            );
            gpu::paint_stock_text(
                frame,
                &painter,
                &UiTextSpec {
                    align: UiTextAlign::Right,
                    scale: 0.9,
                    tint: 0xd9ba70,
                    ..UiTextSpec::new(UiFontName::Medium, &version, 1114.0, y + 36.0)
                },
                origin,
                scale,
            );
            frame.record_rect(
                &format!("social/party-mod:{}", package.id),
                &format!("{} {version}", package.name),
                "listitem",
                bounds.intersect(viewport),
                true,
            );
            frame.record_value(&format!("social/party-mod:{}", package.id), &package.version, None);
        }
        Ok(())
    }
}

/// The plan's text, except the Mods tab's empty notice and the visibility labels that
/// their options paint.
fn paint_party_texts(frame: &mut UiFrame<'_>, ui: &Ui, menu: &PartyMenu) {
    let PartyMenu { tab, origin, scale, viewport, .. } = *menu;
    for text in &menu.plan.texts {
        if tab == PartyTab::Mods && text.label == "party:empty" {
            continue;
        }
        if text.label.starts_with("visibility:") && text.label.ends_with(":label") {
            continue;
        }
        let painter =
            if text.clipped { ui.painter().with_clip_rect(viewport) } else { ui.painter().clone() };
        gpu::paint_stock_text(
            frame,
            &painter,
            &UiTextSpec {
                align: text.align,
                scale: text.scale,
                tint: text.tint,
                ..UiTextSpec::new(text.font, &text.text, text.x, text.y)
            },
            origin,
            scale,
        );
    }
}
