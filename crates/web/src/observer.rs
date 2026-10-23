//! GPU readonly match panel from `DeveloperObserverScene.tsx`.
//! Camera selection leaves through the connection owner; this panel creates no participant.

use crate::gpu_ui::UiFrame;
use egui::{Color32, FontFamily, FontId, Id, Order, RichText, Stroke, vec2};
use solomon_sim::observer_activity::{
    ObserverSkillEvent, ObserverSkillOfferPresentation, ObserverSkillPlayer,
    derive_observer_skill_events, observer_skill_offers,
};
use solomon_sim::profile::PlayerSkillOffer;
use solomon_sim::session::GameSession;
use solomon_sim::social::GameChatMessage;
use web_sys::Document;

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
    pub fn attach(_document: &Document) -> Result<Self, String> {
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
            let participant = &snapshot.session.participants[id];
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
        let ctx = frame.ctx.clone();
        egui::Area::new(Id::unique("observer/panel"))
            .order(Order::Foreground)
            .anchor(egui::Align2::RIGHT_TOP, vec2(-16.0, 16.0))
            .show(&ctx, |ui| {
                egui::Frame::new()
                    .fill(Color32::from_rgba_unmultiplied(7, 10, 18, 240))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(98, 172, 211)))
                    .inner_margin(12.0)
                    .show(ui, |ui| {
                        ui.set_max_width((ctx.content_rect().width() - 32.0).min(420.0));
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(copy("DEVELOPER OBSERVER", 13.0));
                                ui.label(copy(&self.subtitle, 10.0));
                            });
                            let exit = ui.button(copy("EXIT", 11.0));
                            frame.record("observer/exit", "EXIT", "button", &exit);
                            if exit.clicked() {
                                self.actions.push(ObserverAction::Exit);
                            }
                        });
                        ui.horizontal_wrapped(|ui| {
                            ui.label(copy("CAMERA", 10.0));
                            for (id, name) in &self.roster {
                                let response =
                                    ui.selectable_label(*id == self.selected, copy(name, 11.0));
                                frame.record(
                                    &format!("observer/player:{id}"),
                                    name,
                                    "button",
                                    &response,
                                );
                                if response.clicked() {
                                    self.actions.push(ObserverAction::SelectPlayer(id.clone()));
                                }
                            }
                        });
                        egui::ScrollArea::vertical()
                            .id_salt("observer/activity")
                            .max_height((ctx.content_rect().height() * 0.72).max(100.0))
                            .show(ui, |ui| {
                                ui.separator();
                                ui.label(copy("SKILL PICKS", 11.0));
                                if self.offers.is_empty() {
                                    ui.label(copy("NO ACTIVE SKILL CHOICE.", 10.0));
                                }
                                for offer in &self.offers {
                                    activity(
                                        ui,
                                        &format!("{} · {}", offer.player_name, offer.title),
                                        &offer.options.join(" · "),
                                    );
                                }
                                for event in &self.events[self.events.len().saturating_sub(8)..] {
                                    activity(
                                        ui,
                                        &format!("{} · {}", event.player_name, event.title),
                                        &event.detail,
                                    );
                                }
                                ui.separator();
                                ui.label(copy("ALL MATCH CHAT", 11.0));
                                if self.chat.is_empty() {
                                    ui.label(copy("NO CHAT YET.", 10.0));
                                }
                                for message in &self.chat {
                                    let mut label = format!(
                                        "[{}] {}",
                                        message.channel.label().to_uppercase(),
                                        message.sender.display_name
                                    );
                                    if let Some(recipient) = &message.recipient {
                                        label.push_str(&format!(" → {}", recipient.display_name));
                                    }
                                    activity(ui, &label, &message.text);
                                }
                            });
                    });
            });
        Ok(())
    }
}
fn copy(text: &str, size: f32) -> RichText {
    RichText::new(text)
        .font(FontId::new(size, FontFamily::Name("Cinzel".into())))
        .color(Color32::from_rgb(198, 222, 237))
}
fn activity(ui: &mut egui::Ui, title: &str, detail: &str) {
    egui::Frame::new()
        .fill(Color32::from_rgba_unmultiplied(20, 27, 40, 180))
        .inner_margin(6.0)
        .show(ui, |ui| {
            ui.label(copy(title, 11.0).strong());
            ui.label(
                RichText::new(detail)
                    .font(FontId::new(12.0, FontFamily::Name("Alegreya".into())))
                    .color(Color32::from_rgb(180, 199, 216)),
            );
        });
}
