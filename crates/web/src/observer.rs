//! Read-only match observer panel.
//! Camera selection leaves through the connection owner; this panel creates no participant.

use crate::gpu_ui::UiFrame;
use egui::{Color32, FontFamily, FontId, Id, Stroke};
use solomon_sim::observer_activity::{
    ObserverSkillEvent, ObserverSkillOfferPresentation, ObserverSkillPlayer,
    derive_observer_skill_events, observer_skill_offers,
};
use solomon_sim::profile::PlayerSkillOffer;
use solomon_sim::session::GameSession;
use solomon_sim::social::GameChatMessage;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ObserverAction {
    Exit,
    SelectPlayer(String),
}

/// Actual accepted match state. `view_player_id` is initially the Welcome credential ID;
/// later camera selections are committed by the readonly connection owner.
#[derive(Clone, Copy)]
pub struct ObserverViewSnapshot<'a> {
    pub session: &'a GameSession,
    pub view_player_id: &'a str,
    pub boneyard_name: &'a str,
    pub chat_messages: &'a [GameChatMessage],
    /// The underlying readonly game diagnostics consume the same actual ping.
    pub ping_ms: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PreviousPlayerSkills {
    id: String,
    name: String,
    pending_offer: Option<PlayerSkillOffer>,
    learned_ranks: [u32; 83],
    deferred_choices: usize,
}

impl PreviousPlayerSkills {
    fn borrowed(&self) -> ObserverSkillPlayer<'_> {
        ObserverSkillPlayer {
            id: &self.id,
            name: &self.name,
            pending_offer: self.pending_offer.as_ref(),
            learned_ranks: &self.learned_ranks,
            deferred_choices: self.deferred_choices,
        }
    }
}

pub struct ObserverUi {
    previous: Option<Vec<PreviousPlayerSkills>>,
    events: Vec<ObserverSkillEvent>,
    roster: Vec<(String, String)>,
    offers: Vec<ObserverSkillOfferPresentation>,
    chat: Vec<GameChatMessage>,
    selected: String,
    subtitle: String,
    visible: bool,
    actions: Vec<ObserverAction>,
}
impl ObserverUi {
    pub fn layer_id(&self) -> egui::LayerId {
        egui::LayerId::new(egui::Order::Foreground, egui::Id::unique("observer/panel"))
    }

    pub fn attach() -> Result<Self, String> {
        Ok(Self {
            previous: None,
            events: Vec::new(),
            roster: Vec::new(),
            offers: Vec::new(),
            chat: Vec::new(),
            selected: String::new(),
            subtitle: String::new(),
            visible: false,
            actions: Vec::new(),
        })
    }
    pub fn take_actions(&mut self) -> Vec<ObserverAction> {
        std::mem::take(&mut self.actions)
    }
    pub fn update(&mut self, snapshot: ObserverViewSnapshot<'_>) -> Result<(), String> {
        if snapshot.session.participant(snapshot.view_player_id).is_none() {
            self.visible = false;
            return Ok(());
        }
        let mut players = Vec::new();
        let mut current = Vec::new();
        for id in &snapshot.session.participant_order {
            let participant = &snapshot.session.participants.get(id).expect("participant exists");
            let player = &participant.player;
            players.push(ObserverSkillPlayer {
                id,
                name: &player.selection.name,
                pending_offer: player.progression.pending_offer.as_ref(),
                learned_ranks: &player.book.permanent_ranks,
                deferred_choices: player.progression.deferred_skill_choices,
            });
            current.push(PreviousPlayerSkills {
                id: id.clone(),
                name: player.selection.name.clone(),
                pending_offer: player.progression.pending_offer.clone(),
                learned_ranks: player.book.permanent_ranks,
                deferred_choices: player.progression.deferred_skill_choices,
            });
        }
        if let Some(previous) = &self.previous {
            let before: Vec<_> = previous.iter().map(PreviousPlayerSkills::borrowed).collect();
            self.events.extend(derive_observer_skill_events(
                &before,
                &players,
                snapshot.session.ticks,
            ));
            if self.events.len() > 32 {
                self.events.drain(..self.events.len() - 32);
            }
        }
        self.previous = Some(current);

        self.visible = true;
        self.selected = snapshot.view_player_id.to_owned();
        self.subtitle = format!("{} · NO PARTICIPANT", snapshot.boneyard_name);
        self.roster =
            players.iter().map(|player| (player.id.to_owned(), player.name.to_owned())).collect();
        self.offers = observer_skill_offers(&players);
        self.chat =
            snapshot.chat_messages[snapshot.chat_messages.len().saturating_sub(24)..].to_vec();
        Ok(())
    }
    pub fn hide(&mut self) {
        self.visible = false;
        self.previous = None;
        self.events.clear();
        self.actions.clear();
    }
    pub fn paint(&mut self, frame: &mut UiFrame<'_>) -> Result<(), String> {
        if !self.visible {
            return Ok(());
        }
        frame.input_scope("observer", self.layer_id());
        let screen = frame.ctx.content_rect();
        let small = screen.width() <= 700.0;
        let inset = if small { 8.0 } else { 18.0 };
        let width = (screen.width() - 2.0 * inset).min(if small { 360.0 } else { 410.0 });
        let maximum = if small { screen.height() * 0.55 } else { screen.height() - 36.0 };
        let body_width = (width - 22.0).max(1.0);
        let title = crate::gpu_ui::font_run_with_options(
            frame.ctx,
            FontId::new(11.0, FontFamily::Name("Cinzel Bold".into())),
            Color32::from_rgb(240, 213, 138),
            "DEVELOPER OBSERVER",
            crate::gpu_ui::FontRunOptions { letter_spacing: Some(1.32), ..Default::default() },
        )?;
        let exit = observer_run(
            frame,
            "EXIT",
            9.0,
            "Cinzel Bold",
            Color32::from_rgb(211, 184, 111),
            None,
        )?;
        let exit_width = exit.size().x + 2.0;
        let subtitle = observer_run(
            frame,
            &self.subtitle,
            9.0,
            "Cinzel",
            Color32::from_rgb(143, 133, 107),
            Some((width - 26.0 - 12.0 - exit_width).max(1.0)),
        )?;
        let header_height = (title.size().y + 3.0 + subtitle.size().y).max(32.0) + 20.0 + 1.0;
        let camera =
            observer_run(frame, "CAMERA", 11.0, "Cinzel", Color32::from_rgb(143, 133, 107), None)?;
        let mut target_runs = Vec::new();
        let mut target_positions = Vec::new();
        let mut cursor = egui::vec2(camera.size().x + 9.0, 0.0);
        for (id, name) in &self.roster {
            let selected = *id == self.selected;
            let label = observer_run(
                frame,
                name,
                9.0,
                "Cinzel Bold",
                if selected {
                    Color32::from_rgb(196, 230, 184)
                } else {
                    Color32::from_rgb(211, 184, 111)
                },
                None,
            )?;
            let button_width = label.size().x + 18.0;
            if cursor.x + button_width > body_width && cursor.x > 0.0 {
                cursor.x = 0.0;
                cursor.y += 37.0;
            }
            target_positions.push((cursor, button_width));
            cursor.x += button_width + 5.0;
            target_runs.push(label);
        }
        let target_height = cursor.y + 32.0 + 16.0 + 1.0;
        let skills = self
            .offers
            .iter()
            .enumerate()
            .map(|(i, row)| {
                (
                    format!("offer:{i}"),
                    format!("{} · {}", row.player_name, row.title),
                    row.options.join(" · "),
                    false,
                )
            })
            .chain(self.events.iter().enumerate().skip(self.events.len().saturating_sub(8)).map(
                |(i, row)| {
                    (
                        format!("event:{i}"),
                        format!("{} · {}", row.player_name, row.title),
                        row.detail.clone(),
                        true,
                    )
                },
            ))
            .map(|(id, title, detail, resolved)| {
                observer_article(frame, id, title, detail, resolved, body_width)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let chat = self
            .chat
            .iter()
            .map(|message| {
                let mut title = format!(
                    "[{}] {}",
                    message.channel.label().to_uppercase(),
                    message.sender.display_name
                );
                if let Some(recipient) = &message.recipient {
                    title.push_str(&format!(" → {}", recipient.display_name));
                }
                observer_article(
                    frame,
                    format!("chat:{}", message.sequence),
                    title,
                    message.text.clone(),
                    false,
                    body_width,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let skill_heading = observer_run(
            frame,
            "SKILL PICKS",
            10.0,
            "Cinzel",
            Color32::from_rgb(185, 156, 91),
            None,
        )?;
        let chat_heading = observer_run(
            frame,
            "ALL MATCH CHAT",
            10.0,
            "Cinzel",
            Color32::from_rgb(185, 156, 91),
            None,
        )?;
        let no_skills = observer_run(
            frame,
            "NO ACTIVE SKILL CHOICE.",
            11.0,
            "Cinzel",
            Color32::from_rgb(117, 108, 85),
            Some(body_width),
        )?;
        let no_chat = observer_run(
            frame,
            "NO CHAT YET.",
            11.0,
            "Cinzel",
            Color32::from_rgb(117, 108, 85),
            Some(body_width),
        )?;
        let natural = |heading: &crate::gpu_ui::NativeFontRun,
                       rows: &[ObserverArticle],
                       empty: &crate::gpu_ui::NativeFontRun| {
            heading.size().y
                + 6.0
                + if rows.is_empty() {
                    empty.size().y + 12.0
                } else {
                    rows.iter()
                        .map(|row| row.title.size().y + 2.0 + row.detail.size().y + 7.0)
                        .sum()
                }
        };
        let skill_height = natural(&skill_heading, &skills, &no_skills);
        let chat_height = natural(&chat_heading, &chat, &no_chat);
        let content_budget = (maximum - header_height - target_height - 36.0).max(0.0);
        let ratio = (content_budget / (skill_height + chat_height)).min(1.0);
        let section_heights = [skill_height * ratio, chat_height * ratio];
        let height =
            (header_height + target_height + 34.0 + section_heights.iter().sum::<f32>() + 2.0)
                .min(maximum);
        let bounds = egui::Rect::from_min_size(
            egui::pos2(screen.right() - inset - width, inset),
            egui::vec2(width, height),
        );
        let ctx = frame.ctx.clone();
        egui::Area::new(self.layer_id().id)
            .fade_in(false)
            .order(self.layer_id().order)
            .fixed_pos(bounds.min)
            .constrain(false)
            .show(&ctx, |ui| -> Result<(), String> {
                ui.set_min_size(bounds.size());
                let painter = ui.painter();
                frame.paint_outer_shadow(
                    painter,
                    crate::gpu_ui::ImageShadow {
                        offset: egui::vec2(0.0, 8.0),
                        blur: 15.0,
                        color: Color32::BLACK,
                    },
                    |caster| {
                        caster.rect_filled(bounds, 0.0, Color32::WHITE);
                        Ok(())
                    },
                )?;
                painter.rect_filled(bounds, 0.0, Color32::from_rgba_unmultiplied(8, 7, 6, 230));
                frame.paint_inner_shadow(
                    painter,
                    crate::gpu_ui::ImageShadow {
                        offset: egui::Vec2::ZERO,
                        blur: 12.0,
                        color: Color32::BLACK,
                    },
                    |caster| {
                        caster.rect_filled(bounds.shrink(1.0), 0.0, Color32::WHITE);
                        Ok(())
                    },
                )?;
                painter.rect_stroke(
                    bounds,
                    0.0,
                    Stroke::new(1.0, Color32::from_rgba_unmultiplied(207, 174, 91, 166)),
                    egui::StrokeKind::Inside,
                );
                let header_origin = bounds.min + egui::vec2(13.0, 11.0);
                title.paint(frame, painter, header_origin, 1.0)?;
                subtitle.paint(
                    frame,
                    painter,
                    header_origin + egui::vec2(0.0, title.size().y + 3.0),
                    1.0,
                )?;
                let exit_rect = egui::Rect::from_min_size(
                    egui::pos2(bounds.right() - 13.0 - exit_width, bounds.top() + 11.0),
                    egui::vec2(exit_width, 32.0),
                );
                if observer_button(frame, ui, "observer/exit", "EXIT", &exit, exit_rect, false)?
                    .clicked()
                {
                    self.actions.push(ObserverAction::Exit);
                }
                let divider = |painter: &egui::Painter, y: f32, alpha: u8| {
                    painter.hline(
                        bounds.left() + 1.0..=bounds.right() - 1.0,
                        y,
                        Stroke::new(1.0, Color32::from_rgba_unmultiplied(207, 174, 91, alpha)),
                    )
                };
                divider(ui.painter(), bounds.top() + header_height, 89);
                let targets_origin =
                    egui::pos2(bounds.left() + 11.0, bounds.top() + header_height + 8.0);
                camera.paint(
                    frame,
                    ui.painter(),
                    targets_origin + egui::vec2(0.0, (32.0 - camera.size().y) / 2.0),
                    1.0,
                )?;
                for (((id, name), run), (offset, width)) in
                    self.roster.iter().zip(&target_runs).zip(target_positions)
                {
                    let rect =
                        egui::Rect::from_min_size(targets_origin + offset, egui::vec2(width, 32.0));
                    if observer_button(
                        frame,
                        ui,
                        &format!("observer/player:{id}"),
                        name,
                        run,
                        rect,
                        *id == self.selected,
                    )?
                    .clicked()
                    {
                        self.actions.push(ObserverAction::SelectPlayer(id.clone()));
                    }
                }
                let mut section_top = bounds.top() + header_height + target_height;
                divider(ui.painter(), section_top, 51);
                for (id, heading, heading_text, rows, empty, empty_text, height) in [
                    (
                        "skills",
                        &skill_heading,
                        "SKILL PICKS",
                        &skills,
                        &no_skills,
                        "NO ACTIVE SKILL CHOICE.",
                        section_heights[0],
                    ),
                    (
                        "chat",
                        &chat_heading,
                        "ALL MATCH CHAT",
                        &chat,
                        &no_chat,
                        "NO CHAT YET.",
                        section_heights[1],
                    ),
                ] {
                    let section = egui::Rect::from_min_size(
                        egui::pos2(bounds.left() + 11.0, section_top + 8.0),
                        egui::vec2(body_width, height),
                    );
                    ui.scope_builder(
                        egui::UiBuilder::new().max_rect(section),
                        |ui| -> Result<(), String> {
                            ui.spacing_mut().item_spacing.y = 0.0;
                            egui::ScrollArea::vertical()
                                .id_salt(("observer/section", id))
                                .max_height(height)
                                .min_scrolled_height(0.0)
                                .auto_shrink([false, true])
                                .show(ui, |ui| -> Result<(), String> {
                                    observer_line(
                                        frame,
                                        ui,
                                        &format!("observer/{id}:heading"),
                                        heading_text,
                                        heading,
                                    )?;
                                    ui.add_space(6.0);
                                    if rows.is_empty() {
                                        ui.add_space(6.0);
                                        observer_line(
                                            frame,
                                            ui,
                                            &format!("observer/{id}:empty"),
                                            empty_text,
                                            empty,
                                        )?;
                                        ui.add_space(6.0);
                                    }
                                    for row in rows {
                                        observer_line(
                                            frame,
                                            ui,
                                            &format!("observer/{}:title", row.id),
                                            &row.title_text,
                                            &row.title,
                                        )?;
                                        ui.add_space(2.0);
                                        observer_line(
                                            frame,
                                            ui,
                                            &format!("observer/{}:detail", row.id),
                                            &row.detail_text,
                                            &row.detail,
                                        )?;
                                        ui.add_space(7.0);
                                    }
                                    Ok(())
                                })
                                .inner
                        },
                    )
                    .inner?;
                    section_top += height + 17.0;
                    divider(ui.painter(), section_top, 51);
                }
                frame.record_rect("observer/panel", "DEVELOPER OBSERVER", "region", bounds, true);
                Ok(())
            })
            .inner?;
        Ok(())
    }
}

struct ObserverArticle {
    id: String,
    title_text: String,
    detail_text: String,
    title: std::sync::Arc<crate::gpu_ui::NativeFontRun>,
    detail: std::sync::Arc<crate::gpu_ui::NativeFontRun>,
}
fn observer_run(
    frame: &UiFrame<'_>,
    text: &str,
    size: f32,
    family: &str,
    color: Color32,
    wrap: Option<f32>,
) -> Result<std::sync::Arc<crate::gpu_ui::NativeFontRun>, String> {
    crate::gpu_ui::font_run(
        frame.ctx,
        FontId::new(size, FontFamily::Name(family.to_owned().into())),
        color,
        text,
        wrap,
    )
}
fn observer_article(
    frame: &UiFrame<'_>,
    id: String,
    title_text: String,
    detail_text: String,
    resolved: bool,
    width: f32,
) -> Result<ObserverArticle, String> {
    let title = observer_run(
        frame,
        &title_text,
        9.0,
        "Cinzel Bold",
        Color32::from_rgb(215, 200, 157),
        Some(width),
    )?;
    let detail = observer_run(
        frame,
        &detail_text,
        13.0,
        "IM FELL English",
        if resolved { Color32::from_rgb(159, 200, 147) } else { Color32::from_rgb(170, 160, 128) },
        Some(width),
    )?;
    Ok(ObserverArticle { id, title_text, detail_text, title, detail })
}
fn observer_line(
    frame: &UiFrame<'_>,
    ui: &mut egui::Ui,
    id: &str,
    text: &str,
    run: &crate::gpu_ui::NativeFontRun,
) -> Result<(), String> {
    let (_, bounds) = ui.allocate_space(run.size());
    run.paint(frame, ui.painter(), bounds.min, 1.0)?;
    let transform = frame.ctx.layer_transform_to_global(ui.layer_id()).unwrap_or_default();
    frame.record_rect(id, text, "text", transform * bounds.intersect(ui.clip_rect()), false);
    frame.record_value(id, text, None);
    Ok(())
}
fn observer_button(
    frame: &UiFrame<'_>,
    ui: &mut egui::Ui,
    id: &str,
    text: &str,
    run: &crate::gpu_ui::NativeFontRun,
    bounds: egui::Rect,
    selected: bool,
) -> Result<egui::Response, String> {
    let response = ui.interact(bounds, Id::unique(id), egui::Sense::click());
    ui.painter().rect_filled(bounds, 0.0, Color32::from_rgb(9, 8, 6));
    ui.painter().rect_stroke(
        bounds,
        0.0,
        Stroke::new(
            1.0,
            if selected {
                Color32::from_rgb(159, 200, 147)
            } else {
                Color32::from_rgb(117, 98, 56)
            },
        ),
        egui::StrokeKind::Inside,
    );
    run.paint(frame, ui.painter(), bounds.center() - run.size() / 2.0, 1.0)?;
    frame.record(id, text, "button", &response);
    frame.record_value(id, text, Some(selected));
    Ok(response)
}
