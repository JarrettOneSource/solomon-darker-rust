//! GPU chat transcript/composer from Website `GameChat.tsx`.
//! Drafts stay local; sending queues a host command and never inserts a message.

use egui::{Color32, FontFamily, FontId, Id, Rect, RichText, Stroke, pos2, vec2};
use solomon_sim::settings::game_binding_label;
use solomon_sim::social::{GameChatChannel, GameChatMessage};

use super::{Intent, SocialUi, channel_id, channel_index, gpu};
use crate::gpu_ui::{GroupEffect, UiFrame};
use std::collections::{BTreeMap, BTreeSet};

pub(super) struct ChatMotion {
    panel: super::motion::EaseTransition,
    visible: bool,
    reduced: bool,
    births: BTreeMap<u64, f64>,
}
impl Default for ChatMotion {
    fn default() -> Self {
        Self {
            panel: super::motion::EaseTransition::new(1.0, 650.0),
            visible: false,
            reduced: false,
            births: BTreeMap::new(),
        }
    }
}
impl ChatMotion {
    pub(super) fn hide(&mut self) {
        self.visible = false;
        self.births.clear();
    }
    fn sync(
        &mut self,
        visible: bool,
        faded: bool,
        ids: impl IntoIterator<Item = u64>,
        now: f64,
        reduced: bool,
    ) {
        let target = if faded { 0.0 } else { 1.0 };
        if !visible {
            self.hide();
            self.panel.snap(target);
            return;
        }
        if !self.visible || reduced {
            self.panel.snap(target);
        } else {
            self.panel.set(target, now);
        }
        if self.reduced != reduced {
            self.births.clear();
        }
        let ids: BTreeSet<_> = ids.into_iter().collect();
        self.births.retain(|id, _| ids.contains(id));
        for id in ids {
            self.births.entry(id).or_insert(now);
        }
        self.visible = true;
        self.reduced = reduced;
    }
    fn entry(&self, id: u64, now: f64) -> f32 {
        if self.reduced {
            return 1.0;
        }
        let Some(born) = self.births.get(&id) else {
            return 1.0;
        };
        let elapsed = now - born;
        if elapsed <= 0.0 {
            0.0
        } else if elapsed >= 150.0 {
            1.0
        } else {
            crate::gpu_ui::css_bezier((elapsed / 150.0) as f32, 0.0, 0.0, 0.58, 1.0)
        }
    }
}

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
            self.chat_motion.hide();
            return Ok(());
        }
        let Some(snapshot) = self.snapshot.clone() else { return Ok(()) };
        let ctx = frame.ctx.clone();
        let screen = ctx.content_rect();
        let open = self.chat_open;
        let width = if self.coarse {
            if open { screen.width() } else { 430.0_f32.min(screen.width() * 0.62) }
        } else {
            480.0_f32.min(screen.width() - 28.0)
        }
        .max(120.0);
        let left = if self.coarse {
            if open { 0.0 } else { 56.0 }
        } else {
            (screen.width() * 0.022).clamp(14.0, 34.0).max(self.context.gold_clear_left as f32)
        };
        let faded = solomon_sim::social::is_game_chat_faded(open, self.last_activity, self.now);
        let reduced = self
            .document
            .default_view()
            .ok_or("chat window")?
            .match_media("(prefers-reduced-motion: reduce)")
            .map_err(|_| "chat motion preference")?
            .is_some_and(|query| query.matches());
        let messages: Vec<_> = if open {
            snapshot
                .chat_messages
                .iter()
                .filter(|message| message.channel == self.channel)
                .collect()
        } else {
            snapshot
                .chat_messages
                .iter()
                .skip(snapshot.chat_messages.len().saturating_sub(5))
                .collect()
        };
        let panel_visible = open || !messages.is_empty() || self.chat_status.is_some();
        self.chat_motion.sync(
            panel_visible,
            faded,
            messages.iter().map(|message| message.sequence),
            self.now,
            reduced,
        );
        let opacity = self.chat_motion.panel.value(self.now) as f32;
        let translation = vec2(0.0, 8.0 * (1.0 - opacity));

        let mut intent = None;
        let focused = ctx.memory(|memory| memory.has_focus(Id::unique("social/chat-message")));
        let channel_key = open
            && focused
            && ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Tab));
        let anchor = if self.coarse { egui::Align2::LEFT_TOP } else { egui::Align2::LEFT_BOTTOM };
        let offset = vec2(left, if self.coarse { if open { 0.0 } else { 11.0 } } else { -34.0 });
        let layer = Self::chat_layer(open);
        egui::Area::new(layer.id)
            .fade_in(false)
            .background_interactable(open)
            .order(layer.order)
            .anchor(anchor, offset)
            .show(&ctx, |ui| -> Result<(), String> {
                ui.set_min_width(width);
                ui.set_max_width(width);
                if self.coarse && !open && self.paint_chat_opener(frame, ui)?.clicked() {
                    intent = Some(Intent::Action("chat-open".into()));
                }
                if panel_visible {
                    let available = ui.available_rect_before_wrap();
                    let mut panel_ui = ui.new_child(
                        egui::UiBuilder::new()
                            .id_salt("social/chat-panel")
                            .max_rect(available.translate(translation)),
                    );
                    let painter = panel_ui.painter().clone();
                    frame.paint_group(
                        &painter,
                        GroupEffect { opacity, ..Default::default() },
                        |frame, _| {
                            let ui = &mut panel_ui;
                            ui.spacing_mut().item_spacing.y = if open { 3.0 } else { 4.0 };
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
                                                    &format!(
                                                        "social/channel:{}",
                                                        channel_id(channel)
                                                    ),
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
                                            let close = ui.add(
                                                egui::Button::new(text(
                                                    "×",
                                                    if self.coarse { 24.0 } else { 20.0 },
                                                    gpu::GOLD,
                                                ))
                                                .frame(false),
                                            );
                                            frame.record(
                                                "social/chat-close",
                                                "Close chat",
                                                "button",
                                                &close,
                                            );
                                            if close.clicked() {
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
                                            .min_scrolled_height(if self.coarse {
                                                64.0
                                            } else {
                                                104.0
                                            })
                                            .stick_to_bottom(true)
                                            .show(ui, |ui| -> Result<(), String> {
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
                                                for message in &messages {
                                                    if let Some(action) = self.paint_chat_message(
                                                        frame, ui, message, true,
                                                    )? {
                                                        intent = Some(Intent::Action(action));
                                                    }
                                                }
                                                Ok(())
                                            })
                                            .inner?;
                                        if let Some(status) = &self.chat_status {
                                            let response = ui.label(text(
                                                status,
                                                13.0,
                                                Color32::from_rgb(255, 154, 138),
                                            ));
                                            frame.record(
                                                "social/chat-status",
                                                status,
                                                "status",
                                                &response,
                                            );
                                        }
                                        ui.separator();
                                        ui.horizontal(|ui| {
                                            ui.label(text("◆", 12.0, channel_color(self.channel)));
                                            let placeholder = if self.channel
                                                == GameChatChannel::Whisper
                                            {
                                                format!(
                                                    "Whisper {}",
                                                    self.whisper
                                                        .as_ref()
                                                        .map_or("", |target| &target.display_name)
                                                )
                                            } else {
                                                format!("Message {}", self.channel.label())
                                            };
                                            let (_, field) = ui.allocate_space(vec2(
                                                (width - 118.0).max(50.0),
                                                if self.coarse { 40.0 } else { 34.0 },
                                            ));
                                            let response =
                                                crate::gpu_ui::NativeTextEdit::singleline(
                                                    &mut self.chat_draft,
                                                    "social/chat-message",
                                                    "Chat message",
                                                )
                                                .hint_text(placeholder)
                                                .font(FontId::new(
                                                    if self.coarse { 16.0 } else { 14.0 },
                                                    FontFamily::Name("Alegreya".into()),
                                                ))
                                                .color(Color32::from_rgb(255, 245, 220))
                                                .rules(crate::gpu_ui::EditorRules {
                                                    utf16_limit: Some(180),
                                                    ..Default::default()
                                                })
                                                .request_focus(self.focus_chat)
                                                .show(frame, ui, field);
                                            if ui.is_visible() && response.has_focus() {
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
                                            frame.record(
                                                "social/chat-send",
                                                "Send",
                                                "button",
                                                &send,
                                            );
                                            if response.submitted || send.clicked() {
                                                intent =
                                                    Some(Intent::Submit(self.chat_draft.clone()));
                                            }
                                        });
                                        if !self.coarse {
                                            ui.label(
                                                RichText::new(
                                                    "Enter send · Tab channel · Esc close",
                                                )
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
                                for message in &messages {
                                    if let Some(action) =
                                        self.paint_chat_message(frame, ui, message, false)?
                                    {
                                        intent = Some(Intent::Action(action));
                                    }
                                }
                                if let Some(status) = &self.chat_status {
                                    ui.add(
                                        egui::Label::new(text(
                                            status,
                                            13.0,
                                            Color32::from_rgb(255, 154, 138),
                                        ))
                                        .selectable(false),
                                    );
                                }
                            }
                            Ok(())
                        },
                    )?;
                    let bounds = panel_ui.min_rect();
                    ui.advance_cursor_after_rect(bounds.translate(-translation));
                    frame.record_rect(
                        "social/chat-panel",
                        "Chat transcript",
                        "region",
                        bounds,
                        open,
                    );
                    frame.record_value(
                        "social/chat-panel",
                        &format!(
                            "{{\"opacity\":{opacity},\"translateY\":{},\"faded\":{faded}}}",
                            translation.y
                        ),
                        None,
                    );
                }
                if !self.coarse && !open {
                    ui.add_space(7.0);
                    if self.paint_chat_opener(frame, ui)?.clicked() {
                        intent = Some(Intent::Action("chat-open".into()));
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
            self.handle_intent(intent)?;
        }
        Ok(())
    }

    fn paint_chat_opener(
        &self,
        frame: &UiFrame<'_>,
        ui: &mut egui::Ui,
    ) -> Result<egui::Response, String> {
        let key = game_binding_label(&self.controls.open_chat);
        let unread: u16 = self.unread.iter().map(|n| u16::from(*n)).sum();
        let key_run = crate::gpu_ui::font_run_with_options(
            frame.ctx,
            FontId::new(9.0, FontFamily::Name("JetBrains Mono".into())),
            Color32::from_rgb(188, 174, 137),
            &key,
            crate::gpu_ui::FontRunOptions { line_height: Some(12.6), ..Default::default() },
        )?;
        let size = if self.coarse { vec2(30.0, 30.0) } else { vec2(key_run.size().x + 58.0, 32.0) };
        let (_, bounds) = ui.allocate_space(size);
        let response = ui.interact(
            if self.coarse { bounds.expand(7.0) } else { bounds },
            Id::unique("social/chat-open"),
            egui::Sense::click(),
        );
        let radius = if self.coarse { 6.0 } else { 4.0 };
        frame.paint_group(
            ui.painter(),
            GroupEffect {
                opacity: if response.hovered() { 1.0 } else { 0.8 },
                ..Default::default()
            },
            |frame, painter| {
                frame.paint_outer_shadow(
                    painter,
                    crate::gpu_ui::ImageShadow {
                        offset: vec2(0.0, 5.0),
                        blur: 7.5,
                        color: Color32::from_black_alpha(107),
                    },
                    |caster| {
                        caster.rect_filled(bounds, radius, Color32::WHITE);
                        Ok(())
                    },
                )?;
                painter.rect_filled(bounds, radius, Color32::from_rgba_unmultiplied(6, 8, 11, 209));
                frame.paint_inner_shadow(
                    painter,
                    crate::gpu_ui::ImageShadow {
                        offset: vec2(0.0, 1.0),
                        blur: 0.0,
                        color: Color32::from_rgba_unmultiplied(255, 232, 160, 20),
                    },
                    |caster| {
                        caster.rect_filled(bounds.shrink(1.0), radius - 1.0, Color32::WHITE);
                        Ok(())
                    },
                )?;
                painter.rect_stroke(
                    bounds,
                    radius,
                    Stroke::new(
                        1.0,
                        if response.hovered() {
                            Color32::from_rgb(226, 194, 112)
                        } else {
                            Color32::from_rgba_unmultiplied(194, 161, 88, 133)
                        },
                    ),
                    egui::StrokeKind::Inside,
                );
                let icon_size = if self.coarse { 16.0 } else { 17.0 };
                let icon = Rect::from_center_size(
                    if self.coarse {
                        bounds.center()
                    } else {
                        pos2(bounds.left() + 11.0 + icon_size / 2.0, bounds.center().y)
                    },
                    vec2(icon_size, icon_size),
                );
                let point = |x, y| icon.min + vec2(x, y) * (icon_size / 24.0);
                let glyph = Color32::from_rgb(216, 195, 142);
                painter.rect_filled(
                    Rect::from_min_max(point(2.4, 4.2), point(21.6, 17.0)),
                    2.6 * icon_size / 24.0,
                    glyph,
                );
                painter.add(egui::Shape::convex_polygon(
                    vec![point(8.5, 17.0), point(8.5, 20.8), point(12.6, 17.0)],
                    glyph,
                    Stroke::NONE,
                ));
                for x in [8.1, 12.0, 15.9] {
                    painter.circle_filled(
                        point(x, 10.6),
                        1.15 * icon_size / 24.0,
                        Color32::from_rgb(13, 10, 4),
                    );
                }
                if !self.coarse {
                    let key_bounds = Rect::from_center_size(
                        pos2(
                            icon.right() + 7.0 + (key_run.size().x + 12.0) / 2.0,
                            bounds.center().y,
                        ),
                        key_run.size() + vec2(12.0, 4.0),
                    );
                    painter.rect_filled(key_bounds, 2.0, Color32::from_white_alpha(10));
                    painter.rect_stroke(
                        key_bounds,
                        2.0,
                        Stroke::new(1.0, Color32::from_rgba_unmultiplied(224, 197, 130, 97)),
                        egui::StrokeKind::Inside,
                    );
                    key_run.paint(
                        frame,
                        painter,
                        key_bounds.center() - key_run.size() / 2.0,
                        1.0,
                    )?;
                }
                if unread > 0 {
                    let run = crate::gpu_ui::font_run_with_options(
                        frame.ctx,
                        FontId::new(10.0, FontFamily::Name("JetBrains Mono Bold".into())),
                        Color32::from_rgb(22, 15, 7),
                        &unread.to_string(),
                        crate::gpu_ui::FontRunOptions {
                            line_height: Some(16.0),
                            ..Default::default()
                        },
                    )?;
                    let size = vec2((run.size().x + 6.0).max(16.0), 16.0);
                    let badge = Rect::from_min_size(
                        pos2(bounds.right() + 5.0 - size.x, bounds.top() - 5.0),
                        size,
                    );
                    frame.paint_outer_shadow(
                        painter,
                        crate::gpu_ui::ImageShadow {
                            offset: vec2(0.0, 1.0),
                            blur: 2.0,
                            color: Color32::from_black_alpha(179),
                        },
                        |caster| {
                            caster.rect_filled(badge, 8.0, Color32::WHITE);
                            Ok(())
                        },
                    )?;
                    painter.rect_filled(badge, 8.0, Color32::from_rgb(230, 189, 98));
                    run.paint(frame, painter, badge.center() - run.size() / 2.0, 1.0)?;
                }
                if frame.focus_visible(&response) {
                    painter.rect_stroke(
                        bounds.shrink(1.0),
                        radius - 1.0,
                        Stroke::new(1.0, Color32::from_rgb(242, 214, 141)),
                        egui::StrokeKind::Inside,
                    );
                }
                Ok(())
            },
        )?;
        frame.record("social/chat-open", "Open chat", "button", &response);
        frame.record_painted_rect(
            ui.painter(),
            "social/chat-open-art",
            "Chat opener",
            "img",
            bounds,
            true,
        );
        Ok(response)
    }

    fn paint_chat_message(
        &self,
        frame: &UiFrame<'_>,
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
        let font_size = if self.coarse { 12.5 } else { 14.0 };
        let line_height = font_size * 1.35;
        let activity = message.activity.is_some();
        let family = if activity {
            "Alegreya Italic"
        } else if message.channel == GameChatChannel::Whisper {
            "Alegreya Bold Italic"
        } else {
            "Alegreya Bold"
        };
        let name_color = if activity {
            Color32::from_rgb(159, 201, 221)
        } else if message.channel == GameChatChannel::Whisper {
            Color32::from_rgb(201, 163, 232)
        } else if message.channel == GameChatChannel::Global {
            Color32::from_rgb(141, 200, 227)
        } else {
            gpu::GOLD
        };
        let name_font = FontId::new(font_size, FontFamily::Name(family.into()));
        let name_options =
            crate::gpu_ui::FontRunOptions { line_height: Some(line_height), ..Default::default() };
        let name_run = crate::gpu_ui::font_run_with_options(
            frame.ctx,
            name_font.clone(),
            name_color,
            &label,
            name_options,
        )?;
        let body_font = FontId::new(
            font_size,
            FontFamily::Name(if activity { "Alegreya Italic" } else { "Alegreya" }.into()),
        );
        let body_color = if activity { name_color } else { Color32::from_rgb(238, 230, 210) };
        let body_full = crate::gpu_ui::font_run_with_options(
            frame.ctx,
            body_font.clone(),
            body_color,
            suffix,
            name_options,
        )?;
        let padding = if open { egui::vec2(0.0, 3.0) } else { egui::vec2(18.0, 5.0) };
        let available = (ui.available_width() - padding.x).max(1.0);
        let name_width = name_run.size().x.min(if activity { available } else { 130.0 });
        let inner_width =
            if open { available } else { (name_width + 7.0 + body_full.size().x).min(available) };
        let body = crate::gpu_ui::font_run_with_options(
            frame.ctx,
            body_font,
            body_color,
            suffix,
            crate::gpu_ui::FontRunOptions {
                wrap_width: Some((inner_width - name_width - 7.0).max(1.0)),
                ..name_options
            },
        )?;
        let baseline = name_run.baseline().max(body.baseline());
        let content_height = (baseline - name_run.baseline() + name_run.size().y)
            .max(baseline - body.baseline() + body.size().y);
        let (_, logical) =
            ui.allocate_space(vec2(inner_width + padding.x, content_height + padding.y));
        let entry = self.chat_motion.entry(message.sequence, self.now);
        let translation = vec2(0.0, 3.0 * (1.0 - entry));
        let bounds = logical.translate(translation);
        let origin = bounds.min + if open { vec2(0.0, 1.0) } else { vec2(9.0, 2.0) };
        let name_rect = Rect::from_min_size(
            origin + vec2(0.0, baseline - name_run.baseline()),
            vec2(name_width, name_run.size().y),
        );
        let admitted = open || self.chat_motion.panel.target() != 0.0;
        let author_id = format!("social/chat-author:{}", message.sequence);
        let author = target.map(|_| {
            ui.interact(
                name_rect,
                Id::unique(&author_id),
                if admitted { egui::Sense::click() } else { egui::Sense::hover() },
            )
        });
        let highlighted = author
            .as_ref()
            .is_some_and(|response| response.hovered() || frame.focus_visible(response));
        let colored_name = if highlighted {
            crate::gpu_ui::font_run_with_options(
                frame.ctx,
                name_font,
                Color32::from_rgb(255, 242, 194),
                &label,
                name_options,
            )?
        } else {
            name_run
        };
        let painter = ui.painter().clone();
        frame.paint_group(
            &painter,
            GroupEffect { opacity: entry, ..Default::default() },
            |frame, painter| {
                if !open {
                    painter.rect_filled(
                        bounds,
                        egui::CornerRadius { nw: 0, ne: 3, sw: 0, se: 3 },
                        Color32::from_rgba_unmultiplied(4, 6, 10, 153),
                    );
                    let border = match message.channel {
                        GameChatChannel::Global => {
                            Color32::from_rgba_unmultiplied(141, 200, 227, 179)
                        }
                        GameChatChannel::Boneyard => {
                            Color32::from_rgba_unmultiplied(208, 152, 101, 194)
                        }
                        GameChatChannel::Whisper => {
                            Color32::from_rgba_unmultiplied(190, 150, 230, 179)
                        }
                        GameChatChannel::Party => {
                            Color32::from_rgba_unmultiplied(217, 186, 112, 191)
                        }
                    };
                    painter.line_segment(
                        [bounds.left_top() + vec2(1.0, 0.0), bounds.left_bottom() + vec2(1.0, 0.0)],
                        Stroke::new(2.0, border),
                    );
                }
                let mut shadows = vec![crate::gpu_ui::ImageShadow {
                    offset: vec2(0.0, 1.0),
                    blur: 1.0,
                    color: Color32::BLACK,
                }];
                if !open {
                    shadows.push(crate::gpu_ui::ImageShadow {
                        offset: egui::Vec2::ZERO,
                        blur: 2.0,
                        color: Color32::from_black_alpha(217),
                    });
                }
                frame.paint_group(
                    painter,
                    GroupEffect { shadows, ..Default::default() },
                    |frame, painter| {
                        let ink = colored_name.paint_elided(frame, painter, name_rect, 1.0)?;
                        if highlighted && ink.is_positive() {
                            painter.line_segment(
                                [
                                    pos2(ink.left(), name_rect.top() + baseline + 2.0),
                                    pos2(ink.right(), name_rect.top() + baseline + 2.0),
                                ],
                                Stroke::new(1.0, Color32::from_rgb(255, 242, 194)),
                            );
                        }
                        body.paint(
                            frame,
                            painter,
                            origin + vec2(name_width + 7.0, baseline - body.baseline()),
                            1.0,
                        )
                    },
                )
            },
        )?;
        if let Some(author) = &author {
            frame.record(&author_id, &label, "button", author);
        } else {
            frame.record_rect(&author_id, &label, "text", name_rect, true);
        }
        let message_id = format!("social/chat-message:{}", message.sequence);
        frame.record_rect(&message_id, &format!("{label} {suffix}"), "log", bounds, true);
        frame.record_value(
            &message_id,
            &format!("{{\"opacity\":{entry},\"translateY\":{}}}", translation.y),
            None,
        );
        let action = author
            .filter(|response| response.clicked())
            .and(target)
            .map(|target| format!("card:{}", target.player_reference));
        Ok(action)
    }
}

#[cfg(test)]
mod motion_tests {
    use super::ChatMotion;
    #[test]
    fn entry_is_keyed_and_existing_rows_do_not_reanimate_on_arrival() {
        let mut motion = ChatMotion::default();
        motion.sync(true, false, [1], 100.0, false);
        assert_eq!(motion.entry(1, 100.0), 0.0);
        motion.sync(true, false, [1, 2], 200.0, false);
        assert!(motion.entry(1, 200.0) > 0.8);
        assert_eq!(motion.entry(2, 200.0), 0.0);
        assert_eq!(motion.entry(1, 250.0), 1.0);
        motion.sync(true, false, [2], 260.0, false);
        motion.sync(true, false, [1, 2], 300.0, false);
        assert_eq!(motion.entry(1, 300.0), 0.0);
    }
    #[test]
    fn fade_retains_panel_for_650ms_and_reduced_motion_settles() {
        let mut motion = ChatMotion::default();
        motion.sync(true, false, [1], 0.0, false);
        motion.sync(true, true, [1], 5000.0, false);
        assert_eq!(motion.panel.value(5000.0), 1.0);
        assert!(motion.panel.value(5325.0) > 0.19 && motion.panel.value(5325.0) < 0.21);
        assert_eq!(motion.panel.value(5650.0), 0.0);
        motion.sync(true, false, [1], 6000.0, true);
        assert_eq!(motion.panel.value(6000.0), 1.0);
        assert_eq!(motion.entry(1, 6000.0), 1.0);
    }
    #[test]
    fn hidden_panel_recreates_its_view_clocks_when_shown() {
        let mut motion = ChatMotion::default();
        motion.sync(true, false, [1], 0.0, false);
        motion.hide();
        motion.sync(true, true, [1], 100.0, false);
        assert_eq!(motion.panel.value(100.0), 0.0);
        assert_eq!(motion.entry(1, 100.0), 0.0);
    }
}
