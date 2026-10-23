//! GPU chat transcript/composer from Website `GameChat.tsx`.
//! Drafts stay local; sending queues a host command and never inserts a message.

use egui::{Color32, FontFamily, FontId, Id, Order, Rect, RichText, Stroke, pos2, vec2};
use solomon_sim::settings::game_binding_label;
use solomon_sim::social::{GameChatChannel, GameChatMessage};

use super::{Intent, SocialUi, channel_id, channel_index, gpu};
use crate::gpu_ui::UiFrame;

fn channel_color(channel: GameChatChannel) -> Color32 {
    match channel {
        GameChatChannel::Global => Color32::from_rgb(141, 200, 227),
        GameChatChannel::Boneyard => Color32::from_rgb(210, 154, 105),
        GameChatChannel::Whisper => Color32::from_rgb(201, 163, 232),
        GameChatChannel::Party => gpu::GOLD,
    }
}
fn text(value: impl Into<String>, size: f32, color: Color32) -> RichText {
    RichText::new(value).font(FontId::new(size, FontFamily::Name("Alegreya".into()))).color(color)
}

impl SocialUi {
    pub(super) fn paint_chat(&mut self, frame: &mut UiFrame<'_>) -> Result<(), String> {
        if !self.context.chat_enabled || self.context.input_suspended {
            return Ok(());
        }
        let Some(snapshot) = self.snapshot.clone() else { return Ok(()) };
        let ctx = frame.ctx.clone();
        let screen = ctx.content_rect();
        let open = self.chat_open;
        let width =
            if self.coarse && open { screen.width() } else { 480.0_f32.min(screen.width() - 28.0) }
                .max(120.0);
        let left = if self.coarse {
            if open { 0.0 } else { 56.0 }
        } else {
            (screen.width() * 0.022).clamp(14.0, 34.0).max(self.context.gold_clear_left as f32)
        };
        let faded = solomon_sim::social::is_game_chat_faded(open, self.last_activity, self.now);
        let mut intent = None;
        let focused = ctx.memory(|memory| memory.has_focus(Id::unique("social/chat-message")));
        let send_key = open
            && focused
            && ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
        let channel_key = open
            && focused
            && ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Tab));
        let anchor = if self.coarse { egui::Align2::LEFT_TOP } else { egui::Align2::LEFT_BOTTOM };
        let offset = vec2(left, if self.coarse { if open { 0.0 } else { 11.0 } } else { -34.0 });
        egui::Area::new(Id::unique("social/chat"))
            .order(if open { Order::Foreground } else { Order::Middle })
            .anchor(anchor, offset)
            .show(&ctx, |ui| -> Result<(), String> {
                ui.set_min_width(width);
                ui.set_max_width(width);
                if self.coarse && !open && self.paint_chat_opener(frame, ui).clicked() {
                    intent = Some(Intent::Action("chat-open".into()));
                }
                if open {
                    egui::Frame::new()
                        .fill(Color32::from_rgba_unmultiplied(10, 12, 17, 244))
                        .stroke(Stroke::new(
                            2.0,
                            Color32::from_rgba_unmultiplied(216, 188, 114, 209),
                        ))
                        .corner_radius(4.0)
                        .inner_margin(9.0)
                        .show(ui, |ui| -> Result<(), String> {
                            ui.set_min_width(width - 22.0);
                            ui.horizontal(|ui| {
                                for channel in self.channels() {
                                    let unread = self.unread[channel_index(channel)];
                                    let label = if unread > 0 {
                                        format!("{} {unread}", channel.label())
                                    } else {
                                        channel.label().to_owned()
                                    };
                                    let response = ui.add(
                                        egui::Button::new(text(
                                            &label,
                                            13.0,
                                            if channel == self.channel {
                                                channel_color(channel)
                                            } else {
                                                Color32::from_gray(142)
                                            },
                                        ))
                                        .selected(channel == self.channel)
                                        .frame(false),
                                    );
                                    frame.record(
                                        &format!("social/channel:{}", channel_id(channel)),
                                        channel.label(),
                                        "tab",
                                        &response,
                                    );
                                    if response.clicked() {
                                        intent = Some(Intent::Action(format!(
                                            "channel:{}",
                                            channel_id(channel)
                                        )));
                                    }
                                }
                                if ui
                                    .add(egui::Button::new(text("×", 22.0, gpu::GOLD)).frame(false))
                                    .clicked()
                                {
                                    intent = Some(Intent::Action("chat-close".into()));
                                }
                            });
                            ui.separator();
                            let height = if self.coarse {
                                (screen.height() - 150.0).clamp(64.0, 220.0)
                            } else {
                                (screen.height() * 0.34).clamp(104.0, 260.0)
                            };
                            egui::ScrollArea::vertical()
                                .id_salt("social/chat/transcript")
                                .max_height(height)
                                .min_scrolled_height(if self.coarse { 64.0 } else { 104.0 })
                                .stick_to_bottom(true)
                                .show(ui, |ui| -> Result<(), String> {
                                    let messages: Vec<_> = snapshot
                                        .chat_messages
                                        .iter()
                                        .filter(|message| message.channel == self.channel)
                                        .collect();
                                    if messages.is_empty() {
                                        ui.label(text(
                                            format!(
                                                "No {} messages yet.",
                                                self.channel.label().to_lowercase()
                                            ),
                                            14.0,
                                            Color32::from_gray(130),
                                        ));
                                    }
                                    for message in messages {
                                        if let Some(action) =
                                            self.paint_chat_message(frame, ui, message, true)?
                                        {
                                            intent = Some(Intent::Action(action));
                                        }
                                    }
                                    Ok(())
                                })
                                .inner?;
                            if let Some(status) = &self.chat_status {
                                let response =
                                    ui.label(text(status, 13.0, Color32::from_rgb(255, 154, 138)));
                                frame.record("social/chat-status", status, "status", &response);
                            }
                            ui.separator();
                            ui.horizontal(|ui| {
                                ui.label(text("◆", 12.0, channel_color(self.channel)));
                                let placeholder = if self.channel == GameChatChannel::Whisper {
                                    format!(
                                        "Whisper {}",
                                        self.whisper
                                            .as_ref()
                                            .map_or("", |target| &target.display_name)
                                    )
                                } else {
                                    format!("Message {}", self.channel.label())
                                };
                                let edit = egui::TextEdit::singleline(&mut self.chat_draft)
                                    .id(Id::unique("social/chat-message"))
                                    .char_limit(180)
                                    .hint_text(placeholder)
                                    .font(FontId::new(
                                        if self.coarse { 16.0 } else { 14.0 },
                                        FontFamily::Name("Alegreya".into()),
                                    ))
                                    .text_color(Color32::from_rgb(255, 245, 220))
                                    .desired_width((width - 118.0).max(50.0));
                                let response = ui.add_sized(
                                    vec2(
                                        (width - 118.0).max(50.0),
                                        if self.coarse { 40.0 } else { 34.0 },
                                    ),
                                    edit,
                                );
                                frame.record(
                                    "social/chat-message",
                                    "Chat message",
                                    "textbox",
                                    &response,
                                );
                                frame.record_value("social/chat-message", &self.chat_draft, None);
                                if self.focus_chat {
                                    response.request_focus();
                                    self.focus_chat = false;
                                }
                                if response.changed() {
                                    self.last_activity = self.now;
                                }
                                if channel_key {
                                    intent = Some(Intent::NextChannel);
                                }
                                let send = ui.add(
                                    egui::Button::new(text("Send", 11.0, gpu::GOLD))
                                        .min_size(vec2(58.0, 34.0))
                                        .fill(Color32::from_rgb(36, 28, 14))
                                        .stroke(Stroke::new(1.0, gpu::GOLD)),
                                );
                                frame.record("social/chat-send", "Send", "button", &send);
                                if send_key || send.clicked() {
                                    intent = Some(Intent::Submit(self.chat_draft.clone()));
                                }
                            });
                            if !self.coarse {
                                ui.label(
                                    RichText::new("Enter send · Tab channel · Esc close")
                                        .font(FontId::new(
                                            10.0,
                                            FontFamily::Name("JetBrains Mono".into()),
                                        ))
                                        .color(Color32::from_rgb(129, 123, 107)),
                                );
                            }
                            Ok(())
                        })
                        .inner?;
                } else {
                    if !faded {
                        for message in snapshot
                            .chat_messages
                            .iter()
                            .skip(snapshot.chat_messages.len().saturating_sub(5))
                        {
                            if let Some(action) =
                                self.paint_chat_message(frame, ui, message, false)?
                            {
                                intent = Some(Intent::Action(action));
                            }
                        }
                        if let Some(status) = &self.chat_status {
                            ui.label(text(status, 13.0, Color32::from_rgb(255, 154, 138)));
                        }
                    }
                    if !self.coarse {
                        ui.add_space(7.0);
                        if self.paint_chat_opener(frame, ui).clicked() {
                            intent = Some(Intent::Action("chat-open".into()));
                        }
                    }
                }
                if open
                    && ui.input_mut(|input| {
                        input.consume_key(egui::Modifiers::NONE, egui::Key::Escape)
                    })
                {
                    intent = Some(Intent::Close);
                }
                Ok(())
            })
            .inner?;
        if let Some(intent) = intent {
            self.intents.borrow_mut().push(intent);
        }
        Ok(())
    }

    fn paint_chat_opener(&self, frame: &mut UiFrame<'_>, ui: &mut egui::Ui) -> egui::Response {
        let key = game_binding_label(&self.controls.open_chat);
        let unread: u16 = self.unread.iter().map(|n| u16::from(*n)).sum();
        let (bounds, response) = ui.allocate_exact_size(
            vec2(67.0 + if unread > 0 { 20.0 } else { 0.0 }, 32.0),
            egui::Sense::click(),
        );
        let painter = ui.painter();
        painter.rect_filled(bounds, 4.0, Color32::from_rgba_unmultiplied(6, 8, 11, 209));
        painter.rect_stroke(
            bounds,
            4.0,
            Stroke::new(1.0, Color32::from_rgba_unmultiplied(194, 161, 88, 133)),
            egui::StrokeKind::Inside,
        );
        let icon = Rect::from_min_size(bounds.min + vec2(9.0, 7.0), vec2(17.0, 14.0));
        painter.rect_filled(icon, 3.0, Color32::from_rgb(216, 195, 142));
        painter.add(egui::Shape::convex_polygon(
            vec![
                pos2(icon.left() + 4.0, icon.bottom() - 1.0),
                pos2(icon.left() + 4.0, icon.bottom() + 4.0),
                pos2(icon.left() + 9.0, icon.bottom() - 1.0),
            ],
            Color32::from_rgb(216, 195, 142),
            Stroke::NONE,
        ));
        for dx in [4.0, 8.0, 12.0] {
            painter.circle_filled(
                pos2(icon.left() + dx, icon.center().y),
                1.0,
                Color32::from_rgb(13, 10, 4),
            );
        }
        painter.text(
            bounds.min + vec2(35.0, 16.0),
            egui::Align2::LEFT_CENTER,
            &key,
            FontId::new(10.0, FontFamily::Name("JetBrains Mono".into())),
            gpu::GOLD,
        );
        if unread > 0 {
            painter.circle_filled(
                bounds.right_top() + vec2(-2.0, 2.0),
                9.0,
                Color32::from_rgb(74, 39, 22),
            );
            painter.text(
                bounds.right_top() + vec2(-2.0, 2.0),
                egui::Align2::CENTER_CENTER,
                unread.to_string(),
                FontId::proportional(10.0),
                gpu::GOLD,
            );
        }
        frame.record("social/chat-open", "Open chat", "button", &response);
        response
    }

    fn paint_chat_message(
        &self,
        frame: &mut UiFrame<'_>,
        ui: &mut egui::Ui,
        message: &GameChatMessage,
        open: bool,
    ) -> Result<Option<String>, String> {
        let own = self
            .snapshot
            .as_ref()
            .is_some_and(|snapshot| message.sender.player_id == snapshot.local_player_id);
        let label = if message.activity.is_some() {
            message.sender.display_name.clone()
        } else if message.channel == GameChatChannel::Whisper {
            if own {
                format!("To {}", message.recipient.as_ref().map_or("", |r| &r.display_name))
            } else {
                format!("From {}", message.sender.display_name)
            }
        } else if own {
            "You".to_owned()
        } else {
            message.sender.display_name.clone()
        };
        let target = if message.activity.is_some() {
            Some(&message.sender)
        } else if message.channel == GameChatChannel::Whisper && own {
            message.recipient.as_ref()
        } else if own {
            None
        } else {
            Some(&message.sender)
        };
        let suffix = if message.activity.is_some() {
            message.text.strip_prefix(&message.sender.display_name).unwrap_or(&message.text)
        } else {
            &message.text
        };
        let mut action = None;
        let result = egui::Frame::new()
            .fill(if open { Color32::TRANSPARENT } else { Color32::from_black_alpha(153) })
            .inner_margin(if open { 2.0 } else { 5.0 })
            .show(ui, |ui| -> Result<(), String> {
                ui.horizontal_wrapped(|ui| -> Result<(), String> {
                    let mut sender_style = crate::gpu_ui::NativeLabelStyle::new(
                        FontId::new(14.0, FontFamily::Name("Alegreya Bold".into())),
                        channel_color(message.channel),
                    );
                    if let Some(target) = target {
                        sender_style.sense = egui::Sense::click();
                        let response = crate::gpu_ui::shaped_label(
                            frame,
                            ui,
                            &format!("social/chat-author:{}", message.sequence),
                            &label,
                            sender_style,
                        )?;
                        if response.clicked() {
                            action = Some(format!("card:{}", target.player_reference));
                        }
                    } else {
                        crate::gpu_ui::shaped_label(
                            frame,
                            ui,
                            &format!("social/chat-author:{}", message.sequence),
                            &label,
                            sender_style,
                        )?;
                    }
                    let mut message_style = crate::gpu_ui::NativeLabelStyle::new(
                        FontId::new(14.0, FontFamily::Name("Alegreya".into())),
                        Color32::from_rgb(238, 228, 202),
                    );
                    message_style.wrap_width = Some(ui.available_width());
                    crate::gpu_ui::shaped_label(
                        frame,
                        ui,
                        &format!("social/chat-text:{}", message.sequence),
                        suffix,
                        message_style,
                    )?;
                    Ok(())
                })
                .inner
            });
        result.inner?;
        frame.record(
            &format!("social/chat-message:{}", message.sequence),
            &format!("{label} {suffix}"),
            "log",
            &result.response,
        );
        Ok(action)
    }
}
