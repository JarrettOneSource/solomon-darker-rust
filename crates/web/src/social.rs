//! Social GPU surfaces from Website `GameChat.tsx`, `NativeUiPartyChip.tsx`,
//! `NativeUiPartyMenu.tsx`, `PlayerCardDialog.tsx` and `AllyHud.tsx`.
//! Accepted messages, membership, profiles and health remain host-owned.

mod allies;
mod card;
mod chat;
pub(crate) mod gpu;
mod hosted;
mod hub_players;
mod party;

use std::cell::RefCell;
use std::rc::Rc;

use egui::{Color32, Id, Order};
use solomon_sim::settings::GameControlBindings;
use solomon_sim::social::party_ui::NativePartyMenuTab as PartyTab;
use solomon_sim::social::{
    AllyHudRow, GameChatChannel, GameChatRejection, GameChatSender, GameChatWorldKind,
    PartyActionRejection, SocialAction, SocialSnapshot, available_game_chat_channels,
    next_game_chat_channel, normalize_game_chat_text, should_increment_game_chat_unread,
};
use web_sys::Document;

use crate::gpu_ui::{StockAtlas, UiFrame};
use crate::ui::portraits::StockPortraits;
pub use hosted::HostedSocialAction;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SocialUiContext {
    pub visible: bool,
    pub chat_enabled: bool,
    pub input_suspended: bool,
    pub gold_clear_left: f64,
}

/// Display metadata for packages actually admitted by the running session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PartyModView {
    pub id: String,
    pub name: String,
    pub version: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Surface {
    Party(PartyTab),
    Card(solomon_sim::social::GamePlayerCardProfile),
    HubPlayer(String),
}

enum Intent {
    Action(String),
    Submit(String),
    NextChannel,
    Close,
    PartyCopied { code: String, copied: bool },
}

pub struct SocialUi {
    document: Document,
    ui: StockAtlas,
    bonedit: StockAtlas,
    control_panel: StockAtlas,
    skills: StockAtlas,
    portraits: StockPortraits,
    snapshot: Option<SocialSnapshot>,
    ally_rows: Vec<AllyHudRow>,
    surface: Option<Surface>,
    hub_players: Vec<hub_players::HubPlayerCard>,
    requested_card: Option<(bool, String)>,
    hosted: bool,
    hosted_card_request: Option<u64>,
    next_hosted_request: u64,
    hosted_actions: Vec<HostedSocialAction>,
    college_invitations: Vec<solomon_sim::social::GameCollegeInvitation>,
    resolving_college: Option<String>,
    hosted_notice: Option<String>,
    focus_college: bool,
    party_expanded: bool,
    party_ui: party::PartyMenuInteraction,
    clipboard: party::PartyClipboard,
    shown_party_invitation: Option<String>,
    chat_open: bool,
    chat_draft: String,
    channel: GameChatChannel,
    whisper: Option<GameChatSender>,
    unread: [u8; 4],
    last_sequence: u64,
    last_activity: f64,
    last_world: Option<GameChatWorldKind>,
    manually_selected_channel: bool,
    chat_status: Option<String>,
    party_status: Option<String>,
    party_mods: Vec<PartyModView>,
    controls: GameControlBindings,
    context: SocialUiContext,
    now: f64,
    size: (i32, i32),
    coarse: bool,
    focus_chat: bool,
    focus_modal: bool,
    actions: Vec<SocialAction>,
    intents: Rc<RefCell<Vec<Intent>>>,
}

impl SocialUi {
    pub async fn attach(document: &Document) -> Result<Self, String> {
        let intents = Rc::new(RefCell::new(Vec::new()));
        let clipboard = party::PartyClipboard::new(document, intents.clone())?;
        Ok(Self {
            document: document.clone(),
            ui: StockAtlas::load("UI").await?,
            bonedit: StockAtlas::load("Bonedit").await?,
            control_panel: StockAtlas::load("ControlPanel").await?,
            skills: StockAtlas::load("Skills").await?,
            portraits: StockPortraits::load().await?,
            snapshot: None,
            ally_rows: Vec::new(),
            surface: None,
            hub_players: Vec::new(),
            requested_card: None,
            hosted: false,
            hosted_card_request: None,
            next_hosted_request: 0,
            hosted_actions: Vec::new(),
            college_invitations: Vec::new(),
            resolving_college: None,
            hosted_notice: None,
            focus_college: false,
            party_expanded: false,
            party_ui: party::PartyMenuInteraction::default(),
            shown_party_invitation: None,
            chat_open: false,
            chat_draft: String::new(),
            channel: GameChatChannel::Party,
            whisper: None,
            unread: [0; 4],
            last_sequence: 0,
            last_activity: 0.0,
            last_world: None,
            manually_selected_channel: false,
            chat_status: None,
            party_status: None,
            party_mods: Vec::new(),
            controls: GameControlBindings::default(),
            context: SocialUiContext {
                visible: false,
                chat_enabled: false,
                input_suspended: true,
                gold_clear_left: 0.0,
            },
            now: 0.0,
            size: (0, 0),
            coarse: false,
            focus_chat: false,
            focus_modal: false,
            actions: Vec::new(),
            intents,
            clipboard,
        })
    }

    pub fn set_party_mods(&mut self, mods: &[PartyModView]) {
        if self.party_mods != mods {
            self.party_mods = mods.to_vec();
        }
    }

    pub fn set_controls(&mut self, controls: GameControlBindings) {
        if self.controls != controls {
            self.controls = controls;
        }
    }

    pub fn open_chat(&mut self, now_ms: f64) {
        if !self.context.visible || !self.context.chat_enabled || self.context.input_suspended {
            return;
        }
        self.now = now_ms;
        self.surface = None;
        self.chat_open = true;
        self.chat_status = None;
        self.unread[channel_index(self.channel)] = 0;
        self.mark_activity();
        self.focus_chat = true;
    }

    #[must_use]
    pub fn input_active(&self) -> bool {
        self.context.visible
            && !self.context.input_suspended
            && (self.chat_open
                || self.surface.is_some()
                || self.college_visible()
                || self.party_invitation_visible())
    }

    pub fn take_actions(&mut self) -> Vec<SocialAction> {
        std::mem::take(&mut self.actions)
    }

    pub fn chat_rejected(&mut self, rejection: GameChatRejection) {
        self.chat_status = Some(rejection.text());
        self.chat_open = true;
        self.focus_chat = true;
        self.mark_activity();
    }

    pub fn party_rejected(&mut self, rejection: PartyActionRejection) {
        self.party_error(rejection.text());
    }

    /// The host may already supply authored, user-visible error copy with its command result.
    pub fn party_error(&mut self, message: &str) {
        self.party_status = Some(message.to_owned());
    }

    pub fn update(
        &mut self,
        snapshot: Option<&SocialSnapshot>,
        allies: &[AllyHudRow],
        context: SocialUiContext,
        now_ms: f64,
        width: i32,
        height: i32,
    ) -> Result<(), String> {
        self.now = now_ms;
        self.context = context;
        self.size = (width, height);
        self.party_ui.expire_copy(now_ms);
        self.coarse = self
            .document
            .default_view()
            .ok_or("social window")?
            .match_media("(hover:none) and (pointer:coarse)")
            .map_err(|_| "social pointer query")?
            .is_some_and(|query| query.matches());
        if self.snapshot.as_ref() != snapshot {
            self.accept_snapshot(snapshot);
        }
        self.ally_rows = allies.to_vec();
        if !context.visible || context.input_suspended || snapshot.is_none() {
            self.close_surfaces();
        } else if !context.chat_enabled && self.chat_open {
            self.close_chat();
        }
        let intents = std::mem::take(&mut *self.intents.borrow_mut());
        for intent in intents {
            self.handle_intent(intent)?;
        }
        Ok(())
    }

    pub fn paint(&mut self, frame: &mut UiFrame<'_>) -> Result<(), String> {
        self.clipboard.clear();
        if !self.context.visible || self.snapshot.is_none() {
            return Ok(());
        }
        if self.chat_open {
            frame.input_scope(&format!("social/chat:{}", channel_id(self.channel)));
        }
        if self.college_visible() {
            frame.input_scope(&format!("social/college:{}", self.college_invitations[0].id));
        } else if self.party_invitation_visible() {
            frame.input_scope("social/party-invitation");
        } else if let Some(surface) = &self.surface {
            frame.input_scope(&match surface {
                Surface::Party(tab) => format!("social/party:{}", tab.id()),
                Surface::Card(card) => format!("social/card:{}", card.player_reference),
                Surface::HubPlayer(id) => format!("social/hub-player:{id}"),
            });
        }
        let ctx = frame.ctx.clone();
        self.paint_allies(frame)?;
        self.paint_party_chip(frame)?;
        self.paint_chat(frame)?;
        if self.context.input_suspended {
            return Ok(());
        }
        let modal =
            self.college_visible() || self.party_invitation_visible() || self.surface.is_some();
        if modal {
            let mut close = false;
            egui::Area::new(Id::unique("social/modal"))
                .order(Order::Foreground)
                .fixed_pos(egui::Pos2::ZERO)
                .show(&ctx, |ui| -> Result<(), String> {
                    ui.memory_mut(|memory| memory.set_modal_layer(ui.layer_id()));
                    let screen = ctx.content_rect();
                    ui.set_min_size(screen.size());
                    let back =
                        ui.interact(screen, Id::unique("social/backdrop"), egui::Sense::CLICK);
                    ui.painter().rect_filled(screen, 0.0, Color32::from_black_alpha(178));
                    if self.college_visible() {
                        self.paint_college_invitation(frame, ui)?;
                    } else if self.party_invitation_visible() {
                        self.paint_party_invitation(frame, ui)?;
                    } else {
                        match self.surface.clone() {
                            Some(Surface::Party(tab)) => {
                                self.paint_native_party_menu(frame, ui, tab)?
                            }
                            Some(Surface::Card(_) | Surface::HubPlayer(_)) => {
                                self.paint_card(frame, ui)?
                            }
                            None => {}
                        }
                    }
                    close = back.clicked()
                        || ui.input_mut(|input| {
                            input.consume_key(egui::Modifiers::NONE, egui::Key::Escape)
                        });
                    Ok(())
                })
                .inner?;
            if close {
                self.handle_intent(Intent::Close)?;
            }
        }
        if let Some(notice) = &self.hosted_notice {
            egui::Area::new(Id::unique("social/notice"))
                .order(Order::Tooltip)
                .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 16.0))
                .show(&ctx, |ui| {
                    let response = ui.label(gpu::rich(notice, 16.0, gpu::GOLD));
                    frame.record("social/notice", notice, "status", &response);
                });
        }
        self.clipboard.finish_frame();
        Ok(())
    }

    fn accept_snapshot(&mut self, snapshot: Option<&SocialSnapshot>) {
        let previously_grouped = self.channels().len() > 1;
        if self.snapshot.as_ref().map(|state| &state.local_player_id)
            != snapshot.map(|state| &state.local_player_id)
        {
            self.close_surfaces();
            self.last_sequence = snapshot
                .and_then(|state| state.chat_messages.last())
                .map_or(0, |message| message.sequence);
            self.last_activity = self.now;
            self.last_world = None;
            self.unread = [0; 4];
            self.whisper = None;
            self.chat_draft.clear();
            self.party_status = None;
        }
        self.snapshot = snapshot.cloned();
        let Some(snapshot) = snapshot else {
            return;
        };
        for message in &snapshot.chat_messages {
            if message.sequence <= self.last_sequence {
                continue;
            }
            if message.channel == GameChatChannel::Whisper {
                let partner = if message.sender.player_id == snapshot.local_player_id {
                    message.recipient.as_ref()
                } else {
                    Some(&message.sender)
                };
                if let Some(partner) = partner {
                    let preserve_draft = self
                        .whisper
                        .as_ref()
                        .is_some_and(|current| current.player_id != partner.player_id)
                        && message.sender.player_id != snapshot.local_player_id
                        && self.chat_open
                        && self.channel == GameChatChannel::Whisper
                        && !self.chat_draft.is_empty();
                    if !preserve_draft {
                        self.whisper = Some(partner.clone());
                    }
                }
            }
            if should_increment_game_chat_unread(
                message,
                &snapshot.local_player_id,
                self.chat_open,
                self.channel,
            ) {
                let unread = &mut self.unread[channel_index(message.channel)];
                *unread = unread.saturating_add(1).min(99);
            }
            self.last_sequence = message.sequence;
            self.chat_status = None;
            self.last_activity = self.now;
        }
        let channels = self.channels();
        let world_changed = self.last_world != Some(snapshot.world_kind);
        if world_changed
            || (channels.len() > 1 && !previously_grouped && !self.manually_selected_channel)
            || !channels.contains(&self.channel)
        {
            self.channel = channels[0];
            self.manually_selected_channel = false;
        }
        self.last_world = Some(snapshot.world_kind);
        if snapshot.world_kind != GameChatWorldKind::Hub
            && matches!(self.surface, Some(Surface::Party(_)))
        {
            self.surface = None;
        }
        if self.chat_open {
            self.unread[channel_index(self.channel)] = 0;
        }
        if !self.hosted
            && let Some(Surface::Card(current)) = &mut self.surface
            && let Some(updated) = snapshot
                .player_cards
                .iter()
                .find(|card| card.player_reference == current.player_reference)
        {
            *current = updated.clone();
        }
        if let Some((by_reference, target)) = &self.requested_card {
            let found = snapshot.player_cards.iter().find(|card| {
                if *by_reference {
                    card.player_reference == *target
                } else {
                    card.player_id == *target
                }
            });
            if let Some(card) = found {
                self.surface = Some(Surface::Card(card.clone()));
                self.requested_card = None;
                self.focus_modal = true;
            }
        }
    }

    fn channels(&self) -> Vec<GameChatChannel> {
        let Some(snapshot) = &self.snapshot else {
            return vec![GameChatChannel::Party];
        };
        available_game_chat_channels(
            snapshot.world_kind,
            snapshot.party_state.as_ref(),
            snapshot.global_chat_available,
            self.whisper.is_some(),
        )
    }

    fn handle_intent(&mut self, intent: Intent) -> Result<(), String> {
        if !self.context.visible || self.context.input_suspended || self.snapshot.is_none() {
            return Ok(());
        }
        match intent {
            Intent::Action(action) => self.activate(&action)?,
            Intent::Close => {
                if self.college_visible() {
                    self.decline_college_invitation();
                } else if self.party_invitation_visible() {
                    self.deny_party_invitation();
                } else {
                    self.close_surfaces();
                }
            }
            Intent::PartyCopied { code, copied } => {
                if self
                    .snapshot
                    .as_ref()
                    .and_then(|snapshot| snapshot.party_state.as_ref())
                    .and_then(|state| state.party.join_code.as_deref())
                    == Some(code.as_str())
                {
                    self.party_ui.copied(copied, self.now);
                }
            }
            Intent::NextChannel => {
                self.channel = next_game_chat_channel(self.channel, &self.channels());
                self.manually_selected_channel = true;
                self.unread[channel_index(self.channel)] = 0;
                self.chat_status = None;
                self.focus_chat = true;
                self.mark_activity();
            }
            Intent::Submit(draft) => match normalize_game_chat_text(&draft) {
                Ok(text) => {
                    self.actions.push(SocialAction::SendChat {
                        channel: self.channel,
                        text: text.to_owned(),
                        recipient_player_reference: if self.channel == GameChatChannel::Whisper {
                            self.whisper.as_ref().map(|target| target.player_reference.clone())
                        } else {
                            None
                        },
                    });
                    self.chat_draft.clear();
                    self.chat_status = None;
                    self.close_chat();
                }
                Err(error) => {
                    self.chat_status = Some(error.text().to_owned());
                    self.mark_activity();
                }
            },
        }
        Ok(())
    }

    fn activate(&mut self, action: &str) -> Result<(), String> {
        match action {
            "chat-open" => self.open_chat(self.now),
            "chat-close" => self.close_chat(),
            "close" => self.close_surfaces(),
            "party" if self.coarse => {
                self.party_expanded = !self.party_expanded;
            }
            "party" | "party-members" => self.show_party(PartyTab::Members),
            "party-settings" => self.show_party(PartyTab::Settings),
            "party-mods" => self.show_party(PartyTab::Mods),
            "party-generate" => self.actions.push(SocialAction::RotatePartyCode),
            "leave" => {
                self.actions.push(SocialAction::LeaveParty);
                self.close_surfaces();
            }
            "card-message" => self.whisper_to_card(),
            "college-accept" => self.accept_college_invitation(),
            "college-decline" => self.decline_college_invitation(),
            "card-invite" => {
                if let Some(Surface::HubPlayer(player_id)) = &self.surface {
                    self.actions.push(SocialAction::Invite { player_id: player_id.clone() });
                } else if let Some(card) = self.current_card().cloned() {
                    if self.hosted {
                        self.invite_to_college(&card);
                    } else {
                        self.actions
                            .push(SocialAction::Invite { player_id: card.player_id.clone() });
                    }
                    self.close_surfaces();
                }
            }
            _ => {
                if let Some(id) = action.strip_prefix("party-invitation-accept:") {
                    self.actions
                        .push(SocialAction::AcceptInvitation { invitation_id: id.to_owned() });
                } else if let Some(id) = action.strip_prefix("party-invitation-deny:") {
                    self.actions
                        .push(SocialAction::DenyInvitation { invitation_id: id.to_owned() });
                } else if let Some(id) = action.strip_prefix("member:") {
                    self.open_hub_player(id);
                } else if action.starts_with("request:") {
                    self.show_party(PartyTab::Members);
                } else if let Some(reference) = action.strip_prefix("card:") {
                    self.request_card(true, reference);
                } else if let Some(id) = action.strip_prefix("kick:") {
                    self.actions.push(SocialAction::Kick { player_id: id.to_owned() });
                } else if let Some(id) = action.strip_prefix("accept:") {
                    self.actions.push(SocialAction::DecideJoinRequest {
                        request_id: id.to_owned(),
                        accept: true,
                    });
                } else if let Some(id) = action.strip_prefix("deny:") {
                    self.actions.push(SocialAction::DecideJoinRequest {
                        request_id: id.to_owned(),
                        accept: false,
                    });
                } else if let Some(visibility) = action.strip_prefix("visibility:").and_then(|id| {
                    solomon_sim::social::party_ui::NATIVE_PARTY_VISIBILITIES.into_iter().find(
                        |visibility| {
                            solomon_sim::social::party_ui::visibility_id(*visibility) == id
                        },
                    )
                }) {
                    self.actions.push(SocialAction::SetPartyVisibility { visibility });
                } else if let Some(channel) =
                    action.strip_prefix("channel:").and_then(channel_from_id)
                    && self.channels().contains(&channel)
                {
                    self.channel = channel;
                    self.manually_selected_channel = true;
                    self.unread[channel_index(channel)] = 0;
                    self.chat_status = None;
                    self.focus_chat = true;
                    self.mark_activity();
                }
            }
        }
        Ok(())
    }

    fn request_card(&mut self, by_reference: bool, target: &str) {
        self.close_chat();
        if self.hosted {
            self.request_hosted_card(by_reference, target);
            return;
        }
        self.actions.push(if by_reference {
            SocialAction::RequestPlayerCardByReference { player_reference: target.to_owned() }
        } else {
            SocialAction::RequestPlayerCard { player_id: target.to_owned() }
        });
        self.requested_card = Some((by_reference, target.to_owned()));
        if let Some(snapshot) = &self.snapshot {
            let card = snapshot.player_cards.iter().find(|card| {
                if by_reference {
                    card.player_reference == target
                } else {
                    card.player_id == target
                }
            });
            if let Some(card) = card {
                self.surface = Some(Surface::Card(card.clone()));
                self.requested_card = None;
                self.focus_modal = true;
            }
        }
    }

    fn show_party(&mut self, tab: PartyTab) {
        if self.snapshot.as_ref().is_some_and(|snapshot| {
            snapshot.world_kind == GameChatWorldKind::Hub && snapshot.party_state.is_some()
        }) {
            self.close_chat();
            let opening = !matches!(self.surface, Some(Surface::Party(_)));
            self.party_ui.select_tab(tab, opening);
            self.surface = Some(Surface::Party(tab));
            if opening {
                self.party_status = None;
                self.focus_modal = true;
            }
        }
    }

    fn close_chat(&mut self) {
        if self.chat_open {
            self.chat_open = false;
            self.mark_activity();
        }
    }

    fn close_surfaces(&mut self) {
        self.close_chat();
        self.surface = None;
        self.requested_card = None;
    }

    fn mark_activity(&mut self) {
        self.last_activity = self.now;
    }
}

fn channel_index(channel: GameChatChannel) -> usize {
    match channel {
        GameChatChannel::Boneyard => 0,
        GameChatChannel::Global => 1,
        GameChatChannel::Party => 2,
        GameChatChannel::Whisper => 3,
    }
}
fn channel_id(channel: GameChatChannel) -> &'static str {
    match channel {
        GameChatChannel::Boneyard => "boneyard",
        GameChatChannel::Global => "global",
        GameChatChannel::Party => "party",
        GameChatChannel::Whisper => "whisper",
    }
}
fn channel_from_id(id: &str) -> Option<GameChatChannel> {
    match id {
        "boneyard" => Some(GameChatChannel::Boneyard),
        "global" => Some(GameChatChannel::Global),
        "party" => Some(GameChatChannel::Party),
        "whisper" => Some(GameChatChannel::Whisper),
        _ => None,
    }
}
