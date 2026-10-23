//! GPU painter for the canonical 1600×900 stock Party plan.

use egui::{Color32, Id, Rect, Sense, Ui, vec2};
use solomon_sim::hud::HudRect;
use solomon_sim::native_ui_text::NativeUiTextSpec;
use solomon_sim::social::party_ui::{
    NATIVE_PARTY_MENU_FRAME, NATIVE_PARTY_MENU_VIEWPORT, NATIVE_PARTY_MENU_WHEEL_STEP,
    NATIVE_PARTY_VISIBILITIES, NativePartyMenuMember, NativePartyMenuRequest, NativePartyMenuSpec,
    NativePartyMenuTag, clamp_native_party_menu_scroll, plan_native_party_menu, visibility_id,
};
use solomon_sim::social::{party_member_display_name, party_menu_leave_label};
use wasm_bindgen::JsCast;

use super::super::{Intent, PartyTab, SocialUi, gpu};
use crate::gpu_ui::{UiFrame, message_frame};

impl SocialUi {
    pub(in crate::social) fn paint_native_party_menu(
        &mut self,
        frame: &mut UiFrame<'_>,
        ui: &mut Ui,
        tab: PartyTab,
    ) -> Result<(), String> {
        let Some(snapshot) = &self.snapshot else { return Ok(()) };
        let Some(state) = &snapshot.party_state else { return Ok(()) };
        let leader = state.party.leader_player_id == snapshot.local_player_id;
        let members: Vec<_> = state
            .party
            .member_player_ids
            .iter()
            .map(|id| {
                let mut tags = Vec::new();
                if *id == snapshot.local_player_id {
                    tags.push(NativePartyMenuTag::You);
                }
                if *id == state.party.leader_player_id {
                    tags.push(NativePartyMenuTag::Leader);
                }
                if state.party_roster.iter().any(|row| row.player_id == *id && !row.connected) {
                    tags.push(NativePartyMenuTag::Offline);
                }
                NativePartyMenuMember {
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
                .map(|request| NativePartyMenuRequest {
                    id: request.id.clone(),
                    name: request.requester.display_name.clone(),
                })
                .collect()
        } else {
            Vec::new()
        };
        let plan = plan_native_party_menu(&NativePartyMenuSpec {
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
                &NATIVE_PARTY_VISIBILITIES
            } else {
                &[]
            },
        });
        let code = state.party.join_code.clone();
        let visibility = state.party.visibility;
        let (origin, scale) = gpu::stage(self.size);
        let panel = gpu::rect(NATIVE_PARTY_MENU_FRAME, origin, scale);
        ui.interact(panel, Id::unique("social/party-panel-block"), Sense::click());
        message_frame(frame, ui.painter(), &self.ui, panel)?;
        frame.record_rect("social/party-menu", "PARTY", "dialog", panel, true);
        let viewport = gpu::rect(NATIVE_PARTY_MENU_VIEWPORT, origin, scale);
        let mut activated = None;
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
            let mut spec = NativeUiTextSpec::new(
                solomon_sim::native_ui_text::NativeUiFontName::Menu,
                candidate.label(),
                554.0 + index as f64 * 246.0,
                if selected { 235.0 } else { 243.0 },
            );
            spec.tint = if selected { 0xffffff } else { 0xaaa2a6 };
            gpu::native_text(frame, ui.painter(), &spec, origin, scale);
            if response.clicked() {
                activated = Some(format!("party-{}", candidate.id()));
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
                    activated = Some(format!("party-{}", PartyTab::ALL[next].id()));
                    ui.memory_mut(|memory| {
                        memory.request_focus(Id::unique(("social/tab", PartyTab::ALL[next].id())))
                    });
                }
            }
        }
        if tab == PartyTab::Members {
            let scroll = ui.interact(viewport, Id::unique("social/party-scroll"), Sense::drag());
            let wheel =
                if scroll.hovered() { ui.input(|input| input.smooth_scroll_delta.y) } else { 0.0 };
            let mut requested = self.party_ui.scroll_y;
            if wheel != 0.0 {
                requested -= f64::from(wheel.signum()) * NATIVE_PARTY_MENU_WHEEL_STEP;
            }
            if scroll.dragged() {
                requested -= f64::from(scroll.drag_delta().y / scale);
            }
            if scroll.has_focus() {
                requested += ui.input_mut(|input| {
                    if input.consume_key(egui::Modifiers::NONE, egui::Key::PageDown) {
                        NATIVE_PARTY_MENU_VIEWPORT.height
                    } else if input.consume_key(egui::Modifiers::NONE, egui::Key::PageUp) {
                        -NATIVE_PARTY_MENU_VIEWPORT.height
                    } else if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) {
                        NATIVE_PARTY_MENU_WHEEL_STEP
                    } else if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) {
                        -NATIVE_PARTY_MENU_WHEEL_STEP
                    } else {
                        0.0
                    }
                });
            }
            self.party_ui.scroll_y = clamp_native_party_menu_scroll(requested, plan.content_height);
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
        }
        for bounds in &plan.settings_rows {
            gpu::brackets(frame, ui.painter(), &self.ui, gpu::rect(*bounds, origin, scale), scale)?;
        }
        for text in &plan.texts {
            if text.label.starts_with("visibility:") && text.label.ends_with(":label") {
                continue;
            }
            let painter = if text.clipped {
                ui.painter().with_clip_rect(viewport)
            } else {
                ui.painter().clone()
            };
            gpu::native_text(
                frame,
                &painter,
                &NativeUiTextSpec {
                    align: text.align,
                    scale: text.scale,
                    tint: text.tint,
                    ..NativeUiTextSpec::new(text.font, &text.text, text.x, text.y)
                },
                origin,
                scale,
            );
        }
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
                    gpu::native_text(
                        frame,
                        ui.painter(),
                        &NativeUiTextSpec {
                            scale: label.scale,
                            tint: label.tint,
                            ..NativeUiTextSpec::new(label.font, &label.text, label.x, label.y)
                        },
                        origin,
                        scale,
                    );
                }
                frame.record(&format!("social/{}", action.id), &action.label, "radio", &response);
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
                        let index = NATIVE_PARTY_VISIBILITIES
                            .iter()
                            .position(|value| *value == visibility)
                            .unwrap_or(0);
                        let next = NATIVE_PARTY_VISIBILITIES
                            [(index as i32 + direction).rem_euclid(3) as usize];
                        activated = Some(format!("visibility:{}", visibility_id(next)));
                    }
                }
                response
            } else {
                gpu::stock_button(
                    frame,
                    ui,
                    &self.ui,
                    &action.id,
                    &action.label,
                    bounds,
                    !action.disabled,
                )?
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
                    clamp_native_party_menu_scroll(self.party_ui.scroll_y, plan.content_height);
            }
            if response.clicked() && !action.disabled {
                activated = Some(action.id.clone());
            }
            ui.set_clip_rect(old_clip);
        }
        if let Some(action) = activated {
            if action == "copy" {
                if let Some(code) = code {
                    self.copy_party_code(code);
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

    fn copy_party_code(&mut self, code: String) {
        let promise = (|| {
            let navigator = web_sys::window()?.navigator();
            let clipboard = js_sys::Reflect::get(&navigator, &"clipboard".into()).ok()?;
            let write: js_sys::Function =
                js_sys::Reflect::get(&clipboard, &"writeText".into()).ok()?.dyn_into().ok()?;
            write.call1(&clipboard, &code.clone().into()).ok()?.dyn_into::<js_sys::Promise>().ok()
        })();
        let Some(promise) = promise else {
            self.party_status = Some("Could not copy the party ID.".into());
            return;
        };
        let pending = self.intents.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let copied = wasm_bindgen_futures::JsFuture::from(promise).await.is_ok();
            pending.borrow_mut().push(Intent::PartyCopied { code, copied });
        });
    }
}
