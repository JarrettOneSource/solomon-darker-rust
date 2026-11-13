//! Stock GPU Party chip and menu; actions retain actual host identities.

mod interaction;
mod invitation;
mod menu;
pub(super) use interaction::{PartyClipboard, PartyMenuInteraction};

use egui::{Color32, Id, Rect, Sense, pos2, vec2};
use solomon_sim::social::party_ui::{fit_party_menu_text, party_menu_name_font};
use solomon_sim::social::{GameChatWorldKind, party_member_display_name};
use solomon_sim::ui_text::{UiFontName, UiTextAlign, UiTextSpec, measure_ui_text};

use super::{SocialUi, gpu};
use crate::gpu_ui::UiFrame;

impl SocialUi {
    pub(super) fn paint_party_chip(&mut self, frame: &mut UiFrame<'_>) -> Result<(), String> {
        let Some(snapshot) = &self.snapshot else { return Ok(()) };
        let Some(state) = &snapshot.party_state else { return Ok(()) };
        if snapshot.world_kind != GameChatWorldKind::Hub || self.context.input_suspended {
            return Ok(());
        }
        let expanded = !self.coarse || self.party_expanded;
        let leader = state.party.leader_player_id == snapshot.local_player_id;
        let mut rows = Vec::new();
        if expanded {
            for id in &state.party.member_player_ids {
                let mut tags = Vec::new();
                if *id == snapshot.local_player_id {
                    tags.push(("YOU", 0xaaa2a6));
                }
                if *id == state.party.leader_player_id {
                    tags.push(("LEADER", 0xd9ba70));
                }
                if state.party_roster.iter().any(|row| row.player_id == *id && !row.connected) {
                    tags.push(("OFFLINE", 0xaaa2a6));
                }
                rows.push((
                    format!("member:{id}"),
                    party_member_display_name(state, id).to_owned(),
                    tags,
                ));
            }
            if leader {
                for request in &state.join_requests {
                    rows.push((
                        format!("request:{}", request.id),
                        request.requester.display_name.clone(),
                        vec![("WANTS TO JOIN", 0xd9ba70)],
                    ));
                }
            }
        }
        let gear = leader
            || state.party.member_player_ids.len() > 1
            || snapshot.session_kind == solomon_sim::social::GameSessionKind::PrivateCollege;
        let error = self
            .party_status
            .as_deref()
            .map(|error| fit_party_menu_text(error, UiFontName::Medium, 188.0, 0.85))
            .unwrap_or_default();
        let mut y = 54.0 + if error.is_empty() { 0.0 } else { 22.0 };
        let tall = !rows.is_empty() || !error.is_empty();
        let height = if tall { y + rows.len() as f64 * 36.0 - 4.0 + 12.0 } else { 50.0 };
        let scale = if self.coarse { 0.55 } else { 1.0 };
        let position = solomon_sim::social::party_ui::place_party_chip(
            solomon_sim::hud::HudRect::new(
                if self.coarse { 6.0 } else { 11.0 },
                if self.coarse { 54.0 } else { 174.0 },
                236.0 * scale,
                height * scale,
            ),
            solomon_sim::Vec2::new(f64::from(self.size.0), f64::from(self.size.1)),
            &self.guidance_bounds,
        );
        let ctx = frame.ctx.clone();
        let mut action = None;
        let layer = Self::party_layer();
        let area = egui::Area::new(layer.id).fade_in(false).order(layer.order).constrain(false);
        ctx.set_transform_layer(
            area.layer(),
            egui::emath::TSTransform {
                scaling: scale as f32,
                translation: vec2(position.x as f32, position.y as f32),
            },
        );
        area.fixed_pos(egui::Pos2::ZERO)
            .show(&ctx, |ui| -> Result<(), String> {
                let origin = ui.min_rect().min;
                let panel = Rect::from_min_size(origin, vec2(236.0, height as f32));
                ui.set_min_size(panel.size());
                let body = ui.interact(panel, Id::unique("social/party-chip-body"), Sense::CLICK);
                frame.record("social/party-chip-body", "Party panel", "region", &body);
                self.ui.paint_tiles(frame, ui.painter(), 49, panel, 1.0, Color32::WHITE)?;
                self.ui.paint_stock_frame_scaled(
                    frame,
                    ui.painter(),
                    17,
                    panel,
                    if tall { 0.4 } else { 0.3 },
                    false,
                    Color32::WHITE,
                )?;
                self.ui.paint(
                    frame,
                    ui.painter(),
                    38,
                    origin + vec2(14.0, 12.25),
                    0.5,
                    Color32::WHITE,
                )?;
                let header = ui.interact(
                    Rect::from_min_size(origin, vec2(236.0, 50.0)),
                    Id::unique("social/party"),
                    Sense::click(),
                );
                frame.record("social/party", "PARTY", "button", &header);
                gpu::focus_outline(frame, ui, &header, 1.0);
                if header.clicked() {
                    action = Some("party".to_owned());
                }
                gpu::paint_stock_text(
                    frame,
                    ui.painter(),
                    &UiTextSpec {
                        align: UiTextAlign::Left,
                        tint: 0xd9ba70,
                        ..UiTextSpec::new(UiFontName::Menu, "PARTY", 42.0, 33.0)
                    },
                    origin,
                    1.0,
                );
                if gear {
                    let response = ui.interact(
                        Rect::from_min_size(origin + vec2(168.0, 3.0), vec2(40.0, 44.0)),
                        Id::unique("social/party-settings-shortcut"),
                        Sense::click(),
                    );
                    frame.record(
                        "social/party-settings-shortcut",
                        "Party settings",
                        "button",
                        &response,
                    );
                    let record = self.bonedit.record(54)?;
                    let [x, y] = record.registered_origin();
                    let image = frame
                        .image_size(self.bonedit.path())
                        .ok_or("Party settings icon is not ready")?;
                    let target = Rect::from_min_size(
                        origin
                            + vec2(188.0, 25.0)
                            + (vec2(x, y)
                                - vec2(record.logical_width as f32, record.logical_height as f32)
                                    / 2.0)
                                * 0.6,
                        vec2(record.width as f32, record.height as f32) * 0.6,
                    );
                    let uv = Rect::from_min_max(
                        pos2(record.x as f32 / image.x, record.y as f32 / image.y),
                        pos2(
                            (record.x + record.width) as f32 / image.x,
                            (record.y + record.height) as f32 / image.y,
                        ),
                    );
                    frame.with_sprite_record(record).image_quad_masked(
                        ui.painter(),
                        self.bonedit.path(),
                        [
                            target.left_top(),
                            target.right_top(),
                            target.right_bottom(),
                            target.left_bottom(),
                        ],
                        uv,
                        gpu::GOLD,
                        solomon_graphics::BlendMode::Normal,
                    );
                    gpu::focus_outline(frame, ui, &response, 1.0);
                    if response.clicked() {
                        action = Some("party-settings".to_owned());
                    }
                }
                self.control_panel.paint_centered(
                    frame,
                    ui.painter(),
                    0,
                    origin + vec2(215.0, 25.0),
                    1.0,
                    if self.coarse && expanded { std::f32::consts::FRAC_PI_2 } else { 0.0 },
                    Color32::WHITE,
                )?;
                if !error.is_empty() {
                    gpu::paint_stock_text(
                        frame,
                        ui.painter(),
                        &UiTextSpec {
                            align: UiTextAlign::Left,
                            scale: 0.85,
                            tint: 0xff9a8a,
                            ..UiTextSpec::new(UiFontName::Medium, &error, 24.0, 67.0)
                        },
                        origin,
                        1.0,
                    );
                }
                for (id, name, tags) in rows {
                    let bounds =
                        Rect::from_min_size(origin + vec2(12.0, y as f32), vec2(212.0, 32.0));
                    let response = ui.interact(bounds, Id::unique(("social", &id)), Sense::click());
                    frame.record(&format!("social/{id}"), &name, "button", &response);
                    if response.clicked() {
                        action = Some(id);
                    }
                    gpu::brackets(frame, ui.painter(), &self.ui, bounds, 1.0)?;
                    gpu::focus_outline(frame, ui, &response, 1.0);
                    let tags_width: f64 = tags
                        .iter()
                        .enumerate()
                        .map(|(index, (label, _))| {
                            measure_ui_text(label, UiFontName::Body, 1.0)
                                + if index > 0 { 8.0 } else { 0.0 }
                        })
                        .sum();
                    let tags_left = 212.0 - tags_width;
                    let limit = if tags.is_empty() { 188.0 } else { tags_left - 32.0 };
                    let font = party_menu_name_font(&name);
                    let fitted = fit_party_menu_text(&name, font, limit, 0.65);
                    gpu::paint_stock_text(
                        frame,
                        ui.painter(),
                        &UiTextSpec {
                            align: UiTextAlign::Left,
                            scale: 0.65,
                            ..UiTextSpec::new(font, &fitted, 24.0, y + 21.0)
                        },
                        origin,
                        1.0,
                    );
                    let mut tag_x = tags_left;
                    for (label, tint) in tags {
                        gpu::paint_stock_text(
                            frame,
                            ui.painter(),
                            &UiTextSpec {
                                align: UiTextAlign::Left,
                                tint,
                                ..UiTextSpec::new(UiFontName::Body, label, tag_x, y + 21.0)
                            },
                            origin,
                            1.0,
                        );
                        tag_x += measure_ui_text(label, UiFontName::Body, 1.0) + 8.0;
                    }
                    y += 36.0;
                }
                Ok(())
            })
            .inner?;
        if let Some(action) = action {
            self.activate(&action)?;
        }
        Ok(())
    }
}
