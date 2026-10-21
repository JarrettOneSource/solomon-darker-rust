//! Social browser surfaces from Website `GameChat.tsx`, `NativeUiPartyChip.tsx`,
//! `NativeUiPartyMenu.tsx`, `PlayerCardDialog.tsx` and `AllyHud.tsx`.
//!
//! Only draft/navigation state lives here. Accepted messages, membership, profiles and
//! ally health always come from the native host's borrowed projection.

mod allies;
mod card;
mod chat;
mod hosted;
mod hub_players;
mod party;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use solomon_sim::settings::GameControlBindings;
use solomon_sim::social::party_ui::NativePartyMenuTab as PartyTab;
use solomon_sim::social::{
    AllyHudRow, GameChatChannel, GameChatRejection, GameChatSender, GameChatWorldKind,
    PartyActionRejection, SocialAction, SocialSnapshot, available_game_chat_channels,
    next_game_chat_channel, normalize_game_chat_text, should_increment_game_chat_unread,
};
use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{Document, Event, EventTarget, HtmlElement, KeyboardEvent};

pub use hosted::HostedSocialAction;

use crate::menu::navigation::MenuNavigation;
use crate::ui::portraits::StockPortraits;
use crate::ui::{StockAtlas, append, element};

/// Admission and placement supplied by the active game view. Coordinates are physical
/// client pixels, like DOM rectangles; chat never controls the shared simulation clock.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SocialUiContext {
    pub visible: bool,
    pub chat_enabled: bool,
    pub input_suspended: bool,
    pub gold_clear_left: f64,
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
    DraftChanged,
    NextChannel,
    Close,
    PartyChanged,
    PartyCopied { code: String, copied: bool },
}

struct Listener {
    target: EventTarget,
    name: &'static str,
    capture: bool,
    callback: Closure<dyn FnMut(Event)>,
}

impl Drop for Listener {
    fn drop(&mut self) {
        if let Err(error) = self.target.remove_event_listener_with_callback_and_bool(
            self.name,
            self.callback.as_ref().unchecked_ref(),
            self.capture,
        ) {
            web_sys::console::error_1(&error);
        }
    }
}

pub struct SocialUi {
    document: Document,
    root: HtmlElement,
    chip: HtmlElement,
    modal: HtmlElement,
    party_invitation: HtmlElement,
    shown_party_invitation: Option<String>,
    college_dialog: HtmlElement,
    hosted_notice: HtmlElement,
    allies: HtmlElement,
    chat: chat::ChatView,
    ui: StockAtlas,
    bonedit: StockAtlas,
    control_panel: StockAtlas,
    skills: StockAtlas,
    portraits: StockPortraits,
    snapshot: Option<SocialSnapshot>,
    ally_rows: Vec<AllyHudRow>,
    ally_nodes: Vec<allies::AllyRowView>,
    surface: Option<Surface>,
    hub_players: Vec<hub_players::HubPlayerCard>,
    requested_card: Option<(bool, String)>,
    hosted: bool,
    hosted_card_request: Option<u64>,
    next_hosted_request: u64,
    hosted_actions: Vec<HostedSocialAction>,
    college_invitations: Vec<solomon_sim::social::GameCollegeInvitation>,
    resolving_college: Option<String>,
    focus_college: bool,
    party_expanded: bool,
    party_ui: Rc<RefCell<party::PartyMenuInteraction>>,
    chat_open: bool,
    channel: GameChatChannel,
    whisper: Option<GameChatSender>,
    unread: [u8; 4],
    last_sequence: u64,
    last_activity: f64,
    last_world: Option<GameChatWorldKind>,
    manually_selected_channel: bool,
    chat_status: Option<String>,
    party_status: Option<String>,
    controls: GameControlBindings,
    context: SocialUiContext,
    now: f64,
    size: (i32, i32),
    coarse: bool,
    dirty: bool,
    party_dirty: bool,
    chat_dirty: bool,
    allies_dirty: bool,
    focus_chat: bool,
    focus_modal: bool,
    actions: Vec<SocialAction>,
    intents: Rc<RefCell<Vec<Intent>>>,
    capture_keys: Rc<Cell<bool>>,
    navigation: MenuNavigation,
    _listeners: Vec<Listener>,
}

impl SocialUi {
    pub async fn attach(document: &Document) -> Result<Self, String> {
        let ui = StockAtlas::load("UI").await?;
        let bonedit = StockAtlas::load("Bonedit").await?;
        let control_panel = StockAtlas::load("ControlPanel").await?;
        let skills = StockAtlas::load("Skills").await?;
        let portraits = StockPortraits::load().await?;
        let root = element(document, "div", "game-social")?;
        let chip = element(document, "section", "hub-party-panel")?;
        chip.set_attribute("aria-label", "Party").map_err(|_| "party chip label")?;
        let allies = element(document, "div", "hub-hud-allies")?;
        allies.set_attribute("role", "list").map_err(|_| "ally list role")?;
        allies.set_attribute("aria-label", "Allies").map_err(|_| "ally list label")?;
        let modal = element(document, "div", "social-modal")?;
        let party_invitation = element(document, "div", "hub-party-menu-overlay")?;
        let chat = chat::ChatView::new(document)?;
        let college_dialog = element(document, "div", "play-routing-backdrop")?;
        let hosted_notice = element(document, "div", "main-menu-runtime-status")?;
        attribute(&hosted_notice, "role", "status")?;
        for node in
            [&chip, &allies, &modal, &party_invitation, &chat.root, &college_dialog, &hosted_notice]
        {
            append(&root, node)?;
        }
        root.set_hidden(true);
        append(&document.body().ok_or("social document has no body")?, &root)?;
        let intents = Rc::new(RefCell::new(Vec::new()));
        let party_ui = Rc::new(RefCell::new(party::PartyMenuInteraction::default()));
        let capture_keys = Rc::new(Cell::new(false));
        let mut listeners = Vec::new();
        let clicked = intents.clone();
        let party_copy = party_ui.clone();
        listen(&mut listeners, root.as_ref(), "click", false, move |event| {
            let result = event
                .target()
                .and_then(|target| target.dyn_into::<web_sys::Element>().ok())
                .map(|target| target.closest("[data-social-action]"));
            match result {
                Some(Ok(Some(target))) => {
                    if let Some(action) = target.get_attribute("data-social-action") {
                        event.prevent_default();
                        if action == "party-copy" {
                            party::copy_party_code(&target, &party_copy, &clicked);
                            return;
                        }
                        clicked.borrow_mut().push(Intent::Action(action));
                    }
                }
                Some(Err(error)) => web_sys::console::error_1(&error),
                _ => {}
            }
        })?;
        // Bubble cancellation keeps a UI gesture from turning into a world attack.
        for name in ["pointerdown", "mousedown", "mouseup", "contextmenu"] {
            listen(&mut listeners, root.as_ref(), name, false, |event| {
                event.stop_propagation();
            })?;
        }
        let submitted = intents.clone();
        let input = chat.input.clone();
        listen(&mut listeners, chat.form.as_ref(), "submit", false, move |event| {
            event.prevent_default();
            submitted.borrow_mut().push(Intent::Submit(input.value()));
        })?;
        let changed = intents.clone();
        listen(&mut listeners, chat.input.as_ref(), "input", false, move |_| {
            changed.borrow_mut().push(Intent::DraftChanged);
        })?;
        let keyed = intents.clone();
        listen(&mut listeners, chat.input.as_ref(), "keydown", false, move |event| {
            event.stop_propagation();
            if event.dyn_ref::<KeyboardEvent>().is_some_and(|key| key.key() == "Tab") {
                event.prevent_default();
                keyed.borrow_mut().push(Intent::NextChannel);
            }
        })?;
        let outside = intents.clone();
        let backdrop = modal.clone();
        listen(&mut listeners, modal.as_ref(), "pointerdown", false, move |event| {
            if event.target().is_some_and(|target| target == backdrop.clone().into()) {
                outside.borrow_mut().push(Intent::Close);
            }
        })?;
        let window = document.default_view().ok_or("social document has no window")?;
        party::listen_to_party(&mut listeners, &modal, &party_ui, &intents)?;
        let captured = capture_keys.clone();
        let closed = intents.clone();
        listen(&mut listeners, window.as_ref(), "keydown", true, move |event| {
            if captured.get()
                && event.dyn_ref::<KeyboardEvent>().is_some_and(|key| key.key() == "Escape")
            {
                event.prevent_default();
                event.stop_immediate_propagation();
                closed.borrow_mut().push(Intent::Close);
            }
        })?;
        Ok(Self {
            document: document.clone(),
            root,
            chip,
            modal,
            party_invitation,
            shown_party_invitation: None,
            college_dialog,
            hosted_notice,
            allies,
            chat,
            ui,
            bonedit,
            control_panel,
            skills,
            portraits,
            snapshot: None,
            ally_rows: Vec::new(),
            ally_nodes: Vec::new(),
            surface: None,
            hub_players: Vec::new(),
            requested_card: None,
            hosted: false,
            hosted_card_request: None,
            next_hosted_request: 0,
            hosted_actions: Vec::new(),
            college_invitations: Vec::new(),
            resolving_college: None,
            focus_college: false,
            party_expanded: false,
            party_ui,
            chat_open: false,
            channel: GameChatChannel::Party,
            whisper: None,
            unread: [0; 4],
            last_sequence: 0,
            last_activity: 0.0,
            last_world: None,
            manually_selected_channel: false,
            chat_status: None,
            party_status: None,
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
            dirty: true,
            party_dirty: true,
            chat_dirty: true,
            allies_dirty: true,
            focus_chat: false,
            focus_modal: false,
            actions: Vec::new(),
            intents,
            capture_keys,
            navigation: MenuNavigation::new(),
            _listeners: listeners,
        })
    }

    pub fn set_controls(&mut self, controls: GameControlBindings) {
        if self.controls != controls {
            self.controls = controls;
            self.dirty = true;
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
        self.capture_keys.set(true);
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
        self.dirty = true;
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
        if self.party_ui.borrow_mut().expire_copy(now_ms) {
            self.party_dirty = true;
        }
        if self.context != context || self.size != (width, height) {
            self.context = context;
            self.size = (width, height);
            self.dirty = true;
        }
        let window = self.document.default_view().ok_or("social window")?;
        let coarse = window
            .match_media("(hover:none) and (pointer:coarse)")
            .map_err(|_| "social pointer media query")?
            .is_some_and(|query| query.matches());
        if self.coarse != coarse {
            self.coarse = coarse;
            self.dirty = true;
        }
        if self.snapshot.as_ref() != snapshot {
            self.accept_snapshot(snapshot);
        }
        if self.ally_rows != allies {
            self.ally_rows = allies.to_vec();
            self.allies_dirty = true;
        }
        if !context.visible || context.input_suspended || snapshot.is_none() {
            self.close_surfaces();
        } else if !context.chat_enabled && self.chat_open {
            self.close_chat();
        }
        let intents = std::mem::take(&mut *self.intents.borrow_mut());
        for intent in intents {
            self.handle_intent(intent)?;
        }
        self.root.set_hidden(!context.visible || snapshot.is_none());
        self.capture_keys.set(self.input_active());
        self.hosted_notice
            .set_hidden(self.hosted_notice.text_content().is_none_or(|text| text.is_empty()));
        if self.dirty || self.party_dirty {
            self.draw_party()?;
            self.draw_modal()?;
            self.draw_college_invitation()?;
            self.draw_party_invitation()?;
        }
        if self.dirty || self.allies_dirty {
            self.draw_allies()?;
        }
        if self.dirty || self.chat_dirty {
            self.draw_chat()?;
        }
        self.dirty = false;
        self.party_dirty = false;
        self.chat_dirty = false;
        self.allies_dirty = false;
        self.chat
            .root
            .set_attribute(
                "data-chat-faded",
                &solomon_sim::social::is_game_chat_faded(
                    self.chat_open,
                    self.last_activity,
                    now_ms,
                )
                .to_string(),
            )
            .map_err(|_| "chat fade state")?;
        if self.focus_chat {
            self.chat.input.focus().map_err(|_| "chat input focus")?;
            self.focus_chat = false;
            self.chat.messages.set_scroll_top(self.chat.messages.scroll_height());
        }
        if self.focus_modal {
            if let Some(node) = self
                .modal
                .query_selector("[data-social-focus]")
                .map_err(|_| "social focus target")?
            {
                node.dyn_into::<HtmlElement>()
                    .map_err(|_| "social focus element")?
                    .focus()
                    .map_err(|_| "social dialog focus")?;
            }
            self.focus_modal = false;
        }
        if self.college_visible() {
            if self.navigation.update(
                &self.college_dialog,
                &self.document,
                "college-invitation",
                now_ms,
            )? {
                self.decline_college_invitation();
            }
        } else if self.party_invitation_visible() {
            if self.navigation.update(
                &self.party_invitation,
                &self.document,
                "party-invitation",
                now_ms,
            )? {
                self.deny_party_invitation();
            }
        } else if self.surface.is_some() {
            if self.navigation.update(&self.modal, &self.document, "social", now_ms)? {
                self.close_surfaces();
            }
        } else {
            self.navigation.release();
        }
        Ok(())
    }

    fn accept_snapshot(&mut self, snapshot: Option<&SocialSnapshot>) {
        let previous = self.snapshot.as_ref();
        self.party_dirty |= !party_markup_equal(previous, snapshot)
            || previous.map(|value| &value.player_cards)
                != snapshot.map(|value| &value.player_cards);
        self.chat_dirty |= previous.map(|value| {
            (
                &value.chat_messages,
                value.world_kind,
                value.global_chat_available,
                value.party_state.as_ref().map(|state| state.party.member_player_ids.len()),
            )
        }) != snapshot.map(|value| {
            (
                &value.chat_messages,
                value.world_kind,
                value.global_chat_available,
                value.party_state.as_ref().map(|state| state.party.member_player_ids.len()),
            )
        });
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
            self.chat.input.set_value("");
            self.party_status = None;
        }
        self.snapshot = snapshot.cloned();
        let Some(snapshot) = snapshot else {
            self.dirty = true;
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
                        && !self.chat.input.value().is_empty();
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
            self.party_dirty = true;
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
                self.party_dirty = true;
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
            Intent::PartyChanged => self.party_dirty = true,
            Intent::PartyCopied { code, copied } => {
                if self
                    .snapshot
                    .as_ref()
                    .and_then(|snapshot| snapshot.party_state.as_ref())
                    .and_then(|state| state.party.join_code.as_deref())
                    == Some(code.as_str())
                {
                    self.party_ui.borrow_mut().copied(copied, self.now);
                    self.party_dirty = true;
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
            Intent::DraftChanged => self.last_activity = self.now,
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
                    self.chat.input.set_value("");
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
                self.dirty = true;
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
        self.dirty = true;
    }

    fn show_party(&mut self, tab: PartyTab) {
        if self.snapshot.as_ref().is_some_and(|snapshot| {
            snapshot.world_kind == GameChatWorldKind::Hub && snapshot.party_state.is_some()
        }) {
            self.close_chat();
            let opening = !matches!(self.surface, Some(Surface::Party(_)));
            self.party_ui.borrow_mut().select_tab(tab, opening);
            self.surface = Some(Surface::Party(tab));
            if opening {
                self.party_status = None;
                self.focus_modal = true;
            }
            self.dirty = true;
        }
    }

    fn close_chat(&mut self) {
        if self.chat_open {
            self.chat_open = false;
            self.mark_activity();
            if let Err(error) = self.chat.opener.focus() {
                web_sys::console::error_1(&error);
            }
        }
    }

    fn close_surfaces(&mut self) {
        self.close_chat();
        let had_surface = self.surface.take().is_some();
        self.party_ui.borrow_mut().close();
        let had_request = self.requested_card.take().is_some();
        if had_surface || had_request {
            self.dirty = true;
        }
        self.capture_keys.set(false);
    }

    fn mark_activity(&mut self) {
        self.last_activity = self.now;
        self.dirty = true;
    }
}

fn listen(
    listeners: &mut Vec<Listener>,
    target: &EventTarget,
    name: &'static str,
    capture: bool,
    callback: impl FnMut(Event) + 'static,
) -> Result<(), String> {
    let callback = Closure::<dyn FnMut(Event)>::new(callback);
    target
        .add_event_listener_with_callback_and_bool(name, callback.as_ref().unchecked_ref(), capture)
        .map_err(|_| format!("social {name} listener"))?;
    listeners.push(Listener { target: target.clone(), name, capture, callback });
    Ok(())
}

fn attribute(node: &HtmlElement, name: &str, value: &str) -> Result<(), String> {
    node.set_attribute(name, value).map_err(|_| format!("social {name}"))
}

fn action_button(
    document: &Document,
    label: &str,
    action: &str,
    class: &str,
) -> Result<HtmlElement, String> {
    let node = element(document, "button", class)?;
    attribute(&node, "type", "button")?;
    attribute(&node, "data-social-action", action)?;
    attribute(&node, "aria-label", label)?;
    node.set_text_content(Some(label));
    Ok(node)
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

/// Health changes update ally bars independently; Party labels only depend on identity,
/// membership and connection. An accepted combat snapshot must not rebuild a focused dialog.
fn party_markup_equal(left: Option<&SocialSnapshot>, right: Option<&SocialSnapshot>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => match (&left.party_state, &right.party_state) {
            (Some(left), Some(right)) => {
                left.party == right.party
                    && left.hub_players == right.hub_players
                    && left.join_requests == right.join_requests
                    && left.invitations == right.invitations
                    && left.party_roster.len() == right.party_roster.len()
                    && left.party_roster.iter().zip(&right.party_roster).all(|(left, right)| {
                        left.player_id == right.player_id
                            && left.display_name == right.display_name
                            && left.connected == right.connected
                    })
            }
            (None, None) => true,
            _ => false,
        },
        (None, None) => true,
        _ => false,
    }
}
