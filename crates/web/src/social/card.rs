//! The actual host profile card and its Message/Invite commands.

use solomon_sim::profile::native_wizard_class_title;
use solomon_sim::social::{
    GameChatSender, GameChatWorldKind, GamePlayerCardProfile, GameSessionKind,
    hub_player_party_invite_available, native_player_card_college_invite_available,
};

use super::{SocialUi, Surface};

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
        ui: &mut egui::Ui,
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
        let width = (self.size.0 as f32 - 40.0).clamp(200.0, 400.0);
        let height = (self.size.1 as f32 - 32.0).clamp(180.0, 430.0);
        let bounds = egui::Rect::from_center_size(
            frame.ctx.content_rect().center(),
            egui::vec2(width, height),
        );
        ui.interact(bounds, egui::Id::unique("social/card-panel-block"), egui::Sense::click());
        let mut action = None;
        let mut portrait_error = None;
        ui.scope_builder(egui::UiBuilder::new().max_rect(bounds), |ui| {
            egui::Frame::new()
                .fill(egui::Color32::from_rgb(13, 15, 21))
                .stroke(egui::Stroke::new(2.0, super::gpu::GOLD))
                .corner_radius(5.0)
                .inner_margin(24.0)
                .show(ui, |ui| {
                    ui.set_min_width(width - 48.0);
                    egui::ScrollArea::vertical().max_height(height - 48.0).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let (portrait, _) = ui
                                .allocate_exact_size(egui::vec2(76.0, 84.0), egui::Sense::hover());
                            let painter = ui.painter().with_clip_rect(portrait);
                            if let Err(error) = self.portraits.paint_wizard(
                                frame,
                                &painter,
                                card.element,
                                12,
                                portrait.min + egui::vec2(31.5, 66.5),
                                1.15,
                            ) {
                                portrait_error = Some(error);
                            }
                            if let Err(error) = self.skills.paint_raw(
                                frame,
                                ui.painter(),
                                14,
                                egui::Rect::from_min_size(portrait.min, egui::vec2(76.0, 76.0)),
                                egui::Color32::WHITE,
                            ) {
                                portrait_error = Some(error);
                            }
                            ui.vertical(|ui| {
                                ui.label(super::gpu::rich(
                                    &card.display_name,
                                    22.0,
                                    super::gpu::GOLD,
                                ));
                                ui.label(super::gpu::rich(
                                    native_wizard_class_title(card.element, card.discipline),
                                    14.0,
                                    super::gpu::PALE,
                                ));
                                ui.label(super::gpu::rich(
                                    card.account_username.as_ref().map_or_else(
                                        || "Guest wizard".into(),
                                        |name| format!("Registered · {name}"),
                                    ),
                                    12.0,
                                    egui::Color32::from_gray(173),
                                ));
                                if let Some(activity) = &activity {
                                    ui.label(super::gpu::rich(activity, 10.0, super::gpu::GOLD));
                                }
                            });
                        });
                        ui.add_space(14.0);
                        let playtime = card.total_playtime_ms.map(|time| time / 60000).map_or_else(
                            || "—".into(),
                            |minutes| {
                                if minutes >= 60 {
                                    format!("{}h {}m", minutes / 60, minutes % 60)
                                } else {
                                    format!("{minutes}m")
                                }
                            },
                        );
                        for (label, value, icon) in [
                            ("Gold", format_integer(card.gold), "bag"),
                            (
                                "Highest Wave",
                                card.highest_wave.map_or_else(|| "—".to_owned(), format_integer),
                                "wave",
                            ),
                            ("Time in the Dark", playtime, "infinity"),
                        ] {
                            ui.separator();
                            ui.horizontal(|ui| {
                                let (rect, _) = ui.allocate_exact_size(
                                    egui::vec2(21.0, 21.0),
                                    egui::Sense::hover(),
                                );
                                frame.image(
                                    ui.painter(),
                                    &format!("assets/stock/icon-{icon}.png"),
                                    rect,
                                    None,
                                    egui::Color32::WHITE,
                                );
                                ui.label(super::gpu::rich(label, 14.0, super::gpu::PALE));
                                let response =
                                    ui.label(super::gpu::rich(&value, 18.0, super::gpu::GOLD));
                                frame.record(
                                    &format!("social/card-stat:{icon}"),
                                    &value,
                                    "status",
                                    &response,
                                );
                            });
                        }
                        ui.add_space(18.0);
                        ui.horizontal_wrapped(|ui| {
                            if can_message
                                && super::gpu::browser_button(
                                    frame,
                                    ui,
                                    "card-message",
                                    "Message",
                                    egui::vec2(88.0, 34.0),
                                    true,
                                )
                                .clicked()
                            {
                                action = Some("card-message");
                            }
                            if can_invite
                                && super::gpu::browser_button(
                                    frame,
                                    ui,
                                    "card-invite",
                                    "Invite to Party",
                                    egui::vec2(110.0, 34.0),
                                    true,
                                )
                                .clicked()
                            {
                                action = Some("card-invite");
                            }
                            let close = super::gpu::browser_button(
                                frame,
                                ui,
                                "card-close",
                                "Close",
                                egui::vec2(70.0, 34.0),
                                true,
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
                });
        });
        if let Some(error) = portrait_error {
            return Err(error);
        }
        frame.record_rect("social/player-card", &card.display_name, "dialog", bounds, true);
        if let Some(action) = action {
            self.activate(action)?;
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
