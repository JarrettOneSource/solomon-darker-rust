//! The readonly panel in Website `DeveloperObserverScene.tsx`.
//! The connection owns the observed participant and camera. This view retains only the
//! preceding skill-choice projection needed by `observer-activity.ts`.

use std::cell::RefCell;
use std::rc::Rc;

use solomon_sim::observer_activity::{
    ObserverSkillEvent, ObserverSkillOfferPresentation, ObserverSkillPlayer,
    derive_observer_skill_events, observer_skill_offers,
};
use solomon_sim::profile::PlayerSkillOffer;
use solomon_sim::session::GameSession;
use solomon_sim::social::GameChatMessage;
use wasm_bindgen::{JsCast, prelude::*};
use web_sys::{Document, Element, Event, HtmlElement};

use crate::ui::{append, element};

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
    document: Document,
    root: HtmlElement,
    subtitle: HtmlElement,
    targets: HtmlElement,
    skills: HtmlElement,
    chat: HtmlElement,
    previous: Option<Vec<PreviousPlayerSkills>>,
    events: Vec<ObserverSkillEvent>,
    painted_roster: Vec<(String, String)>,
    painted_offers: Vec<ObserverSkillOfferPresentation>,
    painted_events: Vec<ObserverSkillEvent>,
    painted_chat: Vec<GameChatMessage>,
    painted: bool,
    actions: Rc<RefCell<Vec<ObserverAction>>>,
    click: Closure<dyn FnMut(Event)>,
}

impl ObserverUi {
    pub fn attach(document: &Document) -> Result<Self, String> {
        let root = element(document, "section", "developer-observer-scene")?;
        root.set_attribute("aria-label", "Developer match observer")
            .map_err(|_| "observer description")?;
        root.set_hidden(true);
        let panel = element(document, "aside", "developer-observer-panel")?;
        let header = element(document, "header", "")?;
        let title = element(document, "div", "")?;
        append_text(document, &title, "strong", "DEVELOPER OBSERVER")?;
        let subtitle = element(document, "span", "")?;
        append(&title, &subtitle)?;
        append(&header, &title)?;
        let exit = action_button(document, "EXIT", "exit")?;
        append(&header, &exit)?;
        append(&panel, &header)?;
        let targets = element(document, "div", "developer-observer-targets")?;
        let skills = element(document, "section", "")?;
        let chat = element(document, "section", "")?;
        for child in [&targets, &skills, &chat] {
            append(&panel, child)?;
        }
        append(&root, &panel)?;
        let actions = Rc::new(RefCell::new(Vec::new()));
        let pending = actions.clone();
        let click = Closure::new(move |event: Event| {
            if let Err(error) = queue_observer_action(&event, &pending) {
                web_sys::console::error_1(&JsValue::from_str(&error));
            }
        });
        root.add_event_listener_with_callback("click", click.as_ref().unchecked_ref())
            .map_err(|_| "observer controls listener")?;
        append(&document.body().ok_or("observer document body")?, &root)?;
        Ok(Self {
            document: document.clone(),
            root,
            subtitle,
            targets,
            skills,
            chat,
            previous: None,
            events: Vec::new(),
            painted_roster: Vec::new(),
            painted_offers: Vec::new(),
            painted_events: Vec::new(),
            painted_chat: Vec::new(),
            painted: false,
            actions,
            click,
        })
    }

    pub fn take_actions(&mut self) -> Vec<ObserverAction> {
        std::mem::take(&mut *self.actions.borrow_mut())
    }

    pub fn update(&mut self, snapshot: ObserverViewSnapshot<'_>) -> Result<(), String> {
        // A removed camera target is replaced by the connection's actual fallback ID.
        // As in the source, no participant means there is no observer panel to show.
        if snapshot.session.participant(snapshot.view_player_id).is_none() {
            self.root.set_hidden(true);
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
        self.root.set_hidden(false);
        self.subtitle
            .set_text_content(Some(&format!("{} · NO PARTICIPANT", snapshot.boneyard_name)));
        let roster: Vec<_> =
            players.iter().map(|player| (player.id.to_owned(), player.name.to_owned())).collect();
        if !self.painted || roster != self.painted_roster {
            self.paint_roster(&roster)?;
            self.painted_roster = roster;
        }
        self.paint_camera_selection(snapshot.view_player_id)?;
        let offers = observer_skill_offers(&players);
        let visible_events = &self.events[self.events.len().saturating_sub(8)..];
        if !self.painted || offers != self.painted_offers || visible_events != self.painted_events {
            paint_skills(&self.document, &self.skills, &offers, visible_events)?;
            self.painted_offers = offers;
            self.painted_events = visible_events.to_vec();
        }
        let chats = &snapshot.chat_messages[snapshot.chat_messages.len().saturating_sub(24)..];
        if !self.painted || chats != self.painted_chat {
            paint_chat(&self.document, &self.chat, chats)?;
            self.painted_chat = chats.to_vec();
        }
        self.painted = true;
        Ok(())
    }

    /// Leaving an observed match discards its display history; the next match starts clean.
    pub fn hide(&mut self) {
        self.root.set_hidden(true);
        self.previous = None;
        self.events.clear();
        self.painted = false;
        self.actions.borrow_mut().clear();
    }

    fn paint_roster(&self, roster: &[(String, String)]) -> Result<(), String> {
        let focus = self.document.active_element().and_then(|element| {
            self.targets
                .contains(Some(&element))
                .then(|| element.get_attribute("data-observer-player"))
                .flatten()
        });
        self.targets.set_text_content(None);
        append_text(&self.document, &self.targets, "span", "CAMERA")?;
        for (id, name) in roster {
            let button = action_button(&self.document, name, "select-player")?;
            button
                .set_attribute("data-observer-player", id)
                .map_err(|_| "observer camera target")?;
            append(&self.targets, &button)?;
            if focus.as_deref() == Some(id) {
                button.focus().map_err(|_| "observer camera focus")?;
            }
        }
        Ok(())
    }

    fn paint_camera_selection(&self, selected: &str) -> Result<(), String> {
        let buttons = self
            .targets
            .query_selector_all("[data-observer-player]")
            .map_err(|_| "observer camera buttons")?;
        for index in 0..buttons.length() {
            if let Some(button) =
                buttons.item(index).and_then(|node| node.dyn_into::<Element>().ok())
            {
                let selected =
                    button.get_attribute("data-observer-player").as_deref() == Some(selected);
                button.set_class_name(if selected { "selected" } else { "" });
            }
        }
        Ok(())
    }
}

impl Drop for ObserverUi {
    fn drop(&mut self) {
        if let Err(error) = self
            .root
            .remove_event_listener_with_callback("click", self.click.as_ref().unchecked_ref())
        {
            web_sys::console::error_1(&error);
        }
        self.root.remove();
    }
}

fn queue_observer_action(
    event: &Event,
    actions: &Rc<RefCell<Vec<ObserverAction>>>,
) -> Result<(), String> {
    let Some(target) = event.target().and_then(|target| target.dyn_into::<Element>().ok()) else {
        return Ok(());
    };
    let Some(button) =
        target.closest("[data-observer-action]").map_err(|_| "observer action selector")?
    else {
        return Ok(());
    };
    let action = match button.get_attribute("data-observer-action").as_deref() {
        Some("exit") => ObserverAction::Exit,
        Some("select-player") => ObserverAction::SelectPlayer(
            button.get_attribute("data-observer-player").ok_or("observer camera participant")?,
        ),
        _ => return Ok(()),
    };
    event.stop_propagation();
    actions.borrow_mut().push(action);
    Ok(())
}

fn paint_skills(
    document: &Document,
    root: &HtmlElement,
    offers: &[ObserverSkillOfferPresentation],
    events: &[ObserverSkillEvent],
) -> Result<(), String> {
    root.set_text_content(None);
    append_text(document, root, "h2", "SKILL PICKS")?;
    if offers.is_empty() {
        append_text(document, root, "p", "NO ACTIVE SKILL CHOICE.")?;
    }
    for offer in offers {
        activity_row(
            document,
            root,
            "",
            &format!("{} · {}", offer.player_name, offer.title),
            &offer.options.join(" · "),
        )?;
    }
    for event in events {
        activity_row(
            document,
            root,
            "resolved",
            &format!("{} · {}", event.player_name, event.title),
            &event.detail,
        )?;
    }
    Ok(())
}

fn paint_chat(
    document: &Document,
    root: &HtmlElement,
    messages: &[GameChatMessage],
) -> Result<(), String> {
    root.set_text_content(None);
    append_text(document, root, "h2", "ALL MATCH CHAT")?;
    if messages.is_empty() {
        append_text(document, root, "p", "NO CHAT YET.")?;
    }
    for message in messages {
        let mut label =
            format!("[{}] {}", message.channel.label().to_uppercase(), message.sender.display_name);
        if let Some(recipient) = &message.recipient {
            label.push_str(&format!(" → {}", recipient.display_name));
        }
        activity_row(document, root, "", &label, &message.text)?;
    }
    Ok(())
}

fn activity_row(
    document: &Document,
    parent: &HtmlElement,
    class: &str,
    title: &str,
    detail: &str,
) -> Result<(), String> {
    let row = element(document, "article", class)?;
    append_text(document, &row, "strong", title)?;
    append_text(document, &row, "span", detail)?;
    append(parent, &row)
}

fn append_text(
    document: &Document,
    parent: &HtmlElement,
    tag: &str,
    text: &str,
) -> Result<(), String> {
    let child = element(document, tag, "")?;
    child.set_text_content(Some(text));
    append(parent, &child)
}

fn action_button(document: &Document, label: &str, action: &str) -> Result<HtmlElement, String> {
    let button = element(document, "button", "")?;
    button.set_attribute("type", "button").map_err(|_| "observer button type")?;
    button.set_attribute("data-observer-action", action).map_err(|_| "observer button action")?;
    button.set_text_content(Some(label));
    Ok(button)
}
