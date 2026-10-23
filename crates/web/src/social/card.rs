//! The actual host profile card and its Message/Invite commands.

use solomon_sim::profile::native_wizard_class_title;
use solomon_sim::social::{
    GameChatSender, GameChatWorldKind, GamePlayerCardProfile, GameSessionKind,
    hub_player_party_invite_available, native_player_card_college_invite_available,
};

use super::{Intent, SocialUi, Surface};

impl SocialUi {
    pub(super) fn current_card(&self) -> Option<&GamePlayerCardProfile> {
        let Some(Surface::Card(card)) = &self.surface else {
            return None;
        };
        Some(card)
    }

    pub(super) fn whisper_to_card(&mut self) {
        let Some(card) = self.current_card().cloned().or_else(|| self.hub_player_profile()) else {
            return;
        };
        if (!self.hosted || matches!(self.surface, Some(Surface::HubPlayer(_))))
            && self
                .snapshot
                .as_ref()
                .is_none_or(|snapshot| snapshot.local_player_id == card.player_id)
        {
            return;
        }
        self.whisper = Some(GameChatSender {
            display_name: card.display_name,
            player_id: card.player_id,
            player_reference: card.player_reference,
        });
        self.open_chat(self.now);
        self.channel = solomon_sim::social::GameChatChannel::Whisper;
        self.manually_selected_channel = true;
        self.unread[super::channel_index(self.channel)] = 0;
    }

    pub(super) fn paint_card(
        &mut self,
        frame: &mut crate::gpu_ui::UiFrame<'_>,
        parent: &mut egui::Ui,
    ) -> Result<(), String> {
        let actor_profile = self.hub_player_profile();
        let Some(card) = self.current_card().or(actor_profile.as_ref()).cloned() else {
            return Ok(());
        };
        let snapshot = self.snapshot.as_ref().expect("a card belongs to a host snapshot");
        let is_actor = matches!(self.surface, Some(Surface::HubPlayer(_)));
        let can_invite = if is_actor {
            hub_player_party_invite_available(
                snapshot.party_state.as_ref().map(|state| &state.party),
                &snapshot.local_player_id,
                &card.player_id,
            )
        } else if self.hosted {
            // MainMenuScene2259: invitation opens the recipient's consent flow;
            // this is deliberately distinct from an in-session party invitation.
            native_player_card_college_invite_available(
                snapshot.session_kind,
                snapshot.world_kind,
                snapshot
                    .party_state
                    .as_ref()
                    .is_some_and(|state| state.party.leader_player_id == snapshot.local_player_id),
                card.activity,
            )
        } else {
            snapshot.directory_available
                && card.player_id != snapshot.local_player_id
                && snapshot.party_state.as_ref().is_some_and(|state| {
                    state.party.leader_player_id == snapshot.local_player_id
                        && !state.party.member_player_ids.contains(&card.player_id)
                })
        };

        let can_message = ((!is_actor && self.hosted)
            || card.player_id != snapshot.local_player_id)
            && self.context.chat_enabled;
        let activity = if is_actor {
            self.hub_players
                .iter()
                .find(|actor| actor.player_id == card.player_id)
                .and_then(|actor| actor.activity)
                .map(|activity| match activity {
                    solomon_protocol::HubActivity::Occupied => "Occupied".to_owned(),
                    solomon_protocol::HubActivity::Paused => "Paused".to_owned(),
                })
        } else {
            Some(format!(
                "{} · {}",
                match card.session_kind {
                    GameSessionKind::PrivateCollege => "PRIVATE COLLEGE",
                    GameSessionKind::GlobalHub => "GLOBAL HUB",
                    GameSessionKind::Standalone => "LOCAL SESSION",
                },
                match card.activity {
                    GameChatWorldKind::Hub => "IN COLLEGE",
                    GameChatWorldKind::Boneyard => "IN BONEYARD",
                }
            ))
        };
        let portrait_definition = self.skills.record(14)?.clone();
        let ctx = frame.ctx.clone();
        let screen = ctx.content_rect();
        let coarse = self.coarse;
        let width = (self.size.0 as f32 - 40.0).min(if coarse { 300.0 } else { 400.0 }).max(180.0);
        let padding = if coarse { egui::vec2(14.0, 12.0) } else { egui::vec2(26.0, 24.0) };
        let max_height = if coarse { screen.height() * 0.9 } else { screen.height() - 32.0 };
        let mut action = None;
        let mut portrait_error = None;
        let layer = egui::LayerId::new(
            parent.layer_id().order,
            egui::Id::unique("social/player-card-surface"),
        );
        ctx.set_sublayer(parent.layer_id(), layer);
        let area = egui::Area::new(layer.id)
            .order(layer.order)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(&ctx, |ui| {
                egui::Frame::new()
                    .fill(egui::Color32::from_rgba_unmultiplied(13, 15, 21, 250))
                    .stroke(egui::Stroke::new(if coarse { 2.0 } else { 3.0 }, super::gpu::GOLD))
                    .corner_radius(5.0)
                    .inner_margin(egui::Margin::symmetric(padding.x as i8, padding.y as i8))
                    .show(ui, |ui| {
                        ui.set_width(width - 2.0 * padding.x - if coarse { 4.0 } else { 6.0 });
                        egui::ScrollArea::vertical()
                            .id_salt("social/player-card-scroll")
                            .max_height((max_height - 2.0 * padding.y).max(80.0))
                            .show(ui, |ui| {
                                ui.spacing_mut().item_spacing = egui::vec2(
                                    if coarse { 10.0 } else { 16.0 },
                                    if coarse { 8.0 } else { 14.0 },
                                );
                                ui.horizontal(|ui| {
                                    let size = if coarse { 72.0 } else { 92.0 };
                                    let (portrait, _) = ui.allocate_exact_size(
                                        egui::vec2(size, size),
                                        egui::Sense::hover(),
                                    );
                                    ui.painter().rect_filled(
                                        portrait,
                                        7.0,
                                        egui::Color32::from_rgb(28, 25, 18),
                                    );
                                    let painter = ui.painter().with_clip_rect(portrait);
                                    let position = portrait.min
                                        + if coarse {
                                            egui::vec2(34.63, 62.02)
                                        } else {
                                            egui::vec2(44.25, 79.25)
                                        };
                                    if let Err(error) = self.portraits.paint_wizard(
                                        frame,
                                        &painter,
                                        card.element,
                                        12,
                                        position,
                                        if coarse { 0.9 } else { 1.15 },
                                    ) {
                                        portrait_error = Some(error);
                                    }
                                    let definition = &portrait_definition;
                                    let [x, y] = definition.registered_origin();
                                    let scale = egui::vec2(
                                        portrait.width() / definition.logical_width as f32,
                                        portrait.height() / definition.logical_height as f32,
                                    );
                                    let crop = egui::Rect::from_min_size(
                                        portrait.min + egui::vec2(x, y) * scale,
                                        egui::vec2(
                                            definition.width as f32,
                                            definition.height as f32,
                                        ) * scale,
                                    );
                                    if let Err(error) = self.skills.paint_raw(
                                        frame,
                                        ui.painter(),
                                        14,
                                        crop,
                                        egui::Color32::WHITE,
                                    ) {
                                        portrait_error = Some(error);
                                    }
                                    ui.vertical(|ui| {
                                        ui.spacing_mut().item_spacing.y =
                                            if coarse { 2.0 } else { 4.0 };
                                        ui.label(super::gpu::rich(
                                            &card.display_name,
                                            if coarse { 19.0 } else { 28.0 },
                                            egui::Color32::from_rgb(240, 221, 160),
                                        ));
                                        ui.label(card_text(
                                            &native_wizard_class_title(
                                                card.element,
                                                card.discipline,
                                            )
                                            .to_uppercase(),
                                            if coarse { 9.0 } else { 11.0 },
                                            super::gpu::PALE,
                                        ));
                                        let badge = card.account_username.as_ref().map_or_else(
                                            || "Guest wizard".to_owned(),
                                            |name| format!("Registered · {name}"),
                                        );
                                        egui::Frame::new()
                                            .stroke(egui::Stroke::new(
                                                1.0,
                                                super::gpu::GOLD.gamma_multiply(0.5),
                                            ))
                                            .corner_radius(9.0)
                                            .inner_margin(egui::Margin::symmetric(
                                                if coarse { 7 } else { 9 },
                                                2,
                                            ))
                                            .show(ui, |ui| {
                                                let displayed = badge.to_uppercase();
                                                let response = ui.label(card_text(
                                                    &displayed,
                                                    if coarse { 8.0 } else { 9.5 },
                                                    egui::Color32::from_rgb(243, 217, 148),
                                                ));
                                                frame.record(
                                                    "social/card-badge",
                                                    &displayed,
                                                    "text",
                                                    &response,
                                                );
                                                frame.record_value(
                                                    "social/card-badge",
                                                    &badge,
                                                    None,
                                                );
                                            });
                                        if let Some(activity) = &activity {
                                            let response = ui.label(card_text(
                                                activity,
                                                if coarse { 8.0 } else { 10.0 },
                                                super::gpu::GOLD,
                                            ));
                                            frame.record(
                                                "social/card-activity",
                                                activity,
                                                "text",
                                                &response,
                                            );
                                        }
                                    });
                                });
                                let playtime =
                                    card.total_playtime_ms.map(|time| time / 60000).map_or_else(
                                        || "—".into(),
                                        |minutes| {
                                            if minutes >= 60 {
                                                format!("{}h {}m", minutes / 60, minutes % 60)
                                            } else {
                                                format!("{minutes}m")
                                            }
                                        },
                                    );
                                ui.vertical(|ui| {
                                    ui.spacing_mut().item_spacing.y = 0.0;
                                    for (label, value, icon) in [
                                        ("Gold", format_integer(card.gold), "bag"),
                                        (
                                            "Highest Wave",
                                            card.highest_wave
                                                .map_or_else(|| "—".to_owned(), format_integer),
                                            "wave",
                                        ),
                                        ("Time in the Dark", playtime, "infinity"),
                                    ] {
                                        ui.separator();
                                        let (rect, _) = ui.allocate_exact_size(
                                            egui::vec2(
                                                ui.available_width(),
                                                if coarse { 24.0 } else { 38.0 },
                                            ),
                                            egui::Sense::hover(),
                                        );
                                        let icon_size = if coarse { 15.0 } else { 21.0 };
                                        let image = egui::Rect::from_center_size(
                                            rect.left_center()
                                                + egui::vec2(2.0 + icon_size / 2.0, 0.0),
                                            egui::vec2(icon_size, icon_size),
                                        );
                                        frame.image(
                                            ui.painter(),
                                            &format!("assets/stock/icon-{icon}.png"),
                                            image,
                                            None,
                                            egui::Color32::WHITE,
                                        );
                                        ui.painter().text(
                                            rect.left_center()
                                                + egui::vec2(if coarse { 28.0 } else { 37.0 }, 0.0),
                                            egui::Align2::LEFT_CENTER,
                                            label.to_uppercase(),
                                            egui::FontId::new(
                                                if coarse { 8.5 } else { 10.5 },
                                                egui::FontFamily::Name("Cinzel Bold".into()),
                                            ),
                                            egui::Color32::from_rgb(164, 156, 137),
                                        );
                                        ui.painter().text(
                                            rect.right_center() - egui::vec2(2.0, 0.0),
                                            egui::Align2::RIGHT_CENTER,
                                            &value,
                                            egui::FontId::new(
                                                if coarse { 13.0 } else { 18.0 },
                                                egui::FontFamily::Name("Georgia".into()),
                                            ),
                                            if icon == "bag" {
                                                egui::Color32::from_rgb(255, 215, 106)
                                            } else {
                                                egui::Color32::from_rgb(255, 240, 192)
                                            },
                                        );
                                        frame.record_rect(
                                            &format!("social/card-stat:{icon}"),
                                            &value,
                                            "status",
                                            rect,
                                            true,
                                        );
                                    }
                                    ui.separator();
                                });
                                ui.horizontal(|ui| {
                                    let actions =
                                        1 + usize::from(can_message) + usize::from(can_invite);
                                    let button_width = (ui.available_width()
                                        - ui.spacing().item_spacing.x * (actions - 1) as f32)
                                        / actions as f32;
                                    if can_message
                                        && card_button(
                                            frame,
                                            ui,
                                            "card-message",
                                            "Message",
                                            coarse,
                                            button_width,
                                        )
                                        .clicked()
                                    {
                                        action = Some("card-message");
                                    }
                                    if can_invite
                                        && card_button(
                                            frame,
                                            ui,
                                            "card-invite",
                                            "Invite to Party",
                                            coarse,
                                            button_width,
                                        )
                                        .clicked()
                                    {
                                        action = Some("card-invite");
                                    }
                                    let close = card_button(
                                        frame,
                                        ui,
                                        "card-close",
                                        "Close",
                                        coarse,
                                        button_width,
                                    );
                                    if self.focus_modal {
                                        close.request_focus();
                                        self.focus_modal = false;
                                    }
                                    if close.clicked() {
                                        action = Some("close");
                                    }
                                });
                            });
                    })
                    .response
                    .rect
            });
        let bounds = area.inner;
        let record = self.ui.record(17)?;
        let scale = if coarse { 0.55 } else { 0.9 };
        let source_size = egui::vec2(record.width as f32, record.height as f32);
        let corner_size = source_size * scale;
        let painter = ctx.layer_painter(area.response.layer_id);
        for (left, mirrored) in [
            (bounds.min - egui::vec2(6.0, 5.0), false),
            (bounds.right_top() + egui::vec2(6.0 - corner_size.x, -5.0), true),
        ] {
            let source = egui::Rect::from_min_max(
                egui::pos2(if mirrored { source_size.x } else { 0.0 }, 0.0),
                egui::pos2(if mirrored { 0.0 } else { source_size.x }, source_size.y),
            );
            self.ui.paint_slice(
                frame,
                &painter,
                17,
                source,
                egui::Rect::from_min_size(left, corner_size),
                egui::Color32::WHITE,
            )?;
        }
        if let Some(error) = portrait_error {
            return Err(error);
        }
        frame.record_rect("social/player-card", &card.display_name, "dialog", bounds, true);
        if let Some(action) = action {
            self.intents.borrow_mut().push(Intent::Action(action.to_owned()));
        }
        Ok(())
    }

    fn hub_player_profile(&self) -> Option<GamePlayerCardProfile> {
        let Some(Surface::HubPlayer(player_id)) = &self.surface else {
            return None;
        };
        let actor = self.hub_players.iter().find(|actor| actor.player_id == *player_id)?;
        let snapshot = self.snapshot.as_ref()?;
        let profile = snapshot.party_state.as_ref().and_then(|state| {
            state.hub_players.iter().find(|profile| profile.player_id == *player_id)
        });
        Some(GamePlayerCardProfile {
            player_id: player_id.clone(),
            account_username: profile.and_then(|profile| profile.account_username.clone()),
            activity: GameChatWorldKind::Hub,
            discipline: actor.selection.discipline,
            display_name: profile.map_or_else(
                || actor.selection.name.clone(),
                |profile| profile.display_name.clone(),
            ),
            element: actor.selection.element,
            gold: actor.gold,
            highest_wave: profile.and_then(|profile| profile.highest_wave),
            player_reference: player_id.clone(),
            session_kind: snapshot.session_kind,
            total_playtime_ms: profile.and_then(|profile| profile.total_playtime_ms),
        })
    }
}

fn format_integer(value: u64) -> String {
    let digits = value.to_string();
    let mut result = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            result.push(',');
        }
        result.push(digit);
    }
    result
}

fn card_text(value: &str, size: f32, color: egui::Color32) -> egui::RichText {
    egui::RichText::new(value)
        .font(egui::FontId::new(size, egui::FontFamily::Name("Cinzel Bold".into())))
        .color(color)
}
fn card_button(
    frame: &crate::gpu_ui::UiFrame<'_>,
    ui: &mut egui::Ui,
    id: &str,
    label: &str,
    coarse: bool,
    width: f32,
) -> egui::Response {
    let color = if id == "card-message" {
        egui::Color32::from_rgb(207, 176, 232)
    } else {
        super::gpu::GOLD
    };
    let response = ui.add(
        egui::Button::new(card_text(label, if coarse { 9.0 } else { 10.5 }, color))
            .fill(egui::Color32::from_rgb(23, 20, 13))
            .stroke(egui::Stroke::new(1.0, color.gamma_multiply(0.55)))
            .corner_radius(3.0)
            .min_size(egui::vec2(width, if coarse { 44.0 } else { 34.0 })),
    );
    frame.record(&format!("social/{id}"), label, "button", &response);
    response
}
