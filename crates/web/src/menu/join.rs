//! Website `JoinPartyScene.tsx`, `party-directory.ts` and `party-join.ts`.
//! The Website coordinator resolves IDs, polls requests and admits sessions.

use std::cell::Cell;
use std::rc::Rc;

use solomon_sim::menu::party_directory::{
    DirectoryPartyAction, complete_party_code, directory_party_action,
    directory_party_presentation, normalize_party_code,
};
use solomon_sim::social::GameSessionKind;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{Document, Event, HtmlElement, HtmlInputElement};

use super::HostedMenuAction;
use crate::hosted::{PartyActivity, PublicGameParty};
use crate::ui::{append, element};

pub(super) struct JoinView {
    pub root: HtmlElement,
    document: Document,
    code: HtmlInputElement,
    join_button: HtmlElement,
    error: HtmlElement,
    list: HtmlElement,
    parties: Vec<PublicGameParty>,
    active: bool,
    loading: bool,
    busy: Rc<Cell<bool>>,
    next_request: u64,
    directory_request: Option<u64>,
    join_request: Option<u64>,
    pending_listing: Option<String>,
    directory_error: Option<String>,
    join_error: Option<String>,
    refresh_at: f64,
    viewport_height: f64,
    actions: Vec<HostedMenuAction>,
    _input: Closure<dyn FnMut(Event)>,
}

impl JoinView {
    pub fn new(document: &Document) -> Result<Self, String> {
        let root = element(document, "section", "menu-screen join-party-scene")?;
        root.set_attribute("aria-label", "Join Party").map_err(|_| "Join Party label")?;
        root.set_hidden(true);
        let header = element(document, "header", "")?;
        text(document, &header, "h1", "", "JOIN PARTY")?;
        text(document, &header, "p", "", "Enter a Party ID or join a listed party.")?;
        append(&root, &header)?;
        let form = element(document, "form", "join-party-code")?;
        form.set_attribute("data-menu-action", "join-code").map_err(|_| "Party ID form action")?;
        let label = text(document, &form, "label", "", "PARTY ID")?;
        label.set_attribute("for", "join-party-code").map_err(|_| "Party ID form label")?;
        let field = element(document, "div", "")?;
        let code: HtmlInputElement =
            element(document, "input", "")?.dyn_into().map_err(|_| "Party ID input")?;
        code.set_id("join-party-code");
        code.set_type("text");
        code.set_max_length(128);
        code.set_placeholder("XXXX-XXXX");
        code.set_spellcheck(false);
        for (name, value) in [
            ("aria-label", "Party ID"),
            ("autocapitalize", "characters"),
            ("autocomplete", "off"),
            ("autocorrect", "off"),
            ("enterkeyhint", "go"),
            ("inputmode", "text"),
        ] {
            code.set_attribute(name, value).map_err(|_| "Party ID input attributes")?;
        }
        field.append_child(&code).map_err(|_| "Party ID input parent")?;
        let join_button = action_button(document, "JOIN", "join-code")?;
        join_button.set_attribute("disabled", "").map_err(|_| "incomplete Party ID")?;
        append(&field, &join_button)?;
        append(&form, &field)?;
        append(&root, &form)?;
        let error = element(document, "p", "join-party-error")?;
        error.set_attribute("role", "alert").map_err(|_| "Party error role")?;
        error.set_hidden(true);
        append(&root, &error)?;
        let list = element(document, "div", "join-party-list")?;
        list.set_attribute("role", "list").map_err(|_| "Party directory role")?;
        append(&root, &list)?;
        let footer = element(document, "footer", "")?;
        let back = action_button(document, "BACK", "join-back")?;
        back.set_attribute("data-game-back", "true").map_err(|_| "Party directory back")?;
        append(&footer, &back)?;
        append(&footer, &action_button(document, "REFRESH", "join-refresh")?)?;
        append(&root, &footer)?;
        let busy = Rc::new(Cell::new(false));
        let input_code = code.clone();
        let input_button = join_button.clone();
        let input_busy = busy.clone();
        let input = Closure::<dyn FnMut(Event)>::new(move |_| {
            let normalized = normalize_party_code(&input_code.value());
            input_code.set_value(&normalized);
            if let Err(error) = input_button.toggle_attribute_with_force(
                "disabled",
                input_busy.get() || !complete_party_code(&normalized),
            ) {
                web_sys::console::error_1(&error);
            }
        });
        code.add_event_listener_with_callback("input", input.as_ref().unchecked_ref())
            .map_err(|_| "Party ID edit listener")?;
        Ok(Self {
            root,
            document: document.clone(),
            code,
            join_button,
            error,
            list,
            parties: Vec::new(),
            active: false,
            loading: false,
            busy,
            next_request: 0,
            directory_request: None,
            join_request: None,
            pending_listing: None,
            directory_error: None,
            join_error: None,
            refresh_at: 0.0,
            viewport_height: 0.0,
            actions: Vec::new(),
            _input: input,
        })
    }

    pub fn open(&mut self, now: f64) -> Result<(), String> {
        self.active = true;
        self.code.set_value("");
        self.parties.clear();
        self.busy.set(false);
        self.join_request = None;
        self.pending_listing = None;
        self.join_error = None;
        self.directory_error = None;
        self.refresh(now)
    }

    pub fn close(&mut self) {
        self.active = false;
        self.directory_request = None;
        self.actions.clear();
        if let Some(request_id) = self.join_request.take() {
            self.actions.push(HostedMenuAction::CancelPartyJoin { request_id });
        }
        self.busy.set(false);
    }

    pub fn update(&mut self, now: f64, viewport_height: f64) -> Result<(), String> {
        if !self.active {
            return Ok(());
        }
        if now >= self.refresh_at {
            self.refresh(now)?;
        }
        if self.viewport_height != viewport_height {
            self.viewport_height = viewport_height;
            self.root
                .style()
                .set_property("--join-party-viewport-height", &format!("{viewport_height}px"))
                .map_err(|_| "Party directory visible viewport")?;
        }
        Ok(())
    }

    pub fn activate(&mut self, action: &str, now: f64) -> Result<bool, String> {
        if action == "join-refresh" {
            self.refresh(now)?;
            return Ok(true);
        }
        if action == "join-code" {
            let code = normalize_party_code(&self.code.value());
            if !self.busy.get() && complete_party_code(&code) {
                let request_id = self.begin_join(None);
                self.actions.push(HostedMenuAction::ResolvePartyCode { request_id, code });
                self.paint()?;
            }
            return Ok(true);
        }
        let Some(listing_id) = action.strip_prefix("join-listing:") else {
            return Ok(false);
        };
        if self.busy.get() {
            return Ok(true);
        }
        let Some(party) = self.parties.iter().find(|party| party.id == listing_id) else {
            return Ok(true);
        };
        let action = directory_party_action(
            party.visibility,
            matches!(party.activity, PartyActivity::Playing { .. }),
        );
        if action == DirectoryPartyAction::Wait {
            return Ok(true);
        }
        let request_id = self.begin_join(if action == DirectoryPartyAction::Request {
            Some(listing_id.to_owned())
        } else {
            None
        });
        let listing_id = listing_id.to_owned();
        self.actions.push(match action {
            DirectoryPartyAction::Join => {
                HostedMenuAction::JoinPublicParty { request_id, listing_id }
            }
            DirectoryPartyAction::Request => {
                HostedMenuAction::RequestPartyInvite { request_id, listing_id }
            }
            DirectoryPartyAction::Wait => {
                unreachable!("playing listings returned before starting admission")
            }
        });
        self.paint()?;
        Ok(true)
    }

    pub fn complete_directory(
        &mut self,
        request_id: u64,
        result: Result<Vec<PublicGameParty>, String>,
    ) -> Result<bool, String> {
        if !self.active || self.directory_request != Some(request_id) {
            return Ok(false);
        }
        self.directory_request = None;
        self.loading = false;
        match result {
            Ok(parties) => {
                self.parties = parties;
                self.directory_error = None;
            }
            Err(error) => self.directory_error = Some(error),
        }
        self.paint()?;
        Ok(true)
    }

    /// Success only ends the request's busy state. Root retains the real resolution
    /// and chooses the source consent/loading flow; a view never admits membership.
    pub fn complete_join(
        &mut self,
        request_id: u64,
        result: Result<(), String>,
    ) -> Result<bool, String> {
        if !self.active || self.join_request != Some(request_id) {
            return Ok(false);
        }
        self.join_request = None;
        self.pending_listing = None;
        self.busy.set(false);
        self.join_error = result.err();
        self.paint()?;
        Ok(true)
    }

    pub fn take_actions(&mut self) -> Vec<HostedMenuAction> {
        std::mem::take(&mut self.actions)
    }

    fn refresh(&mut self, now: f64) -> Result<(), String> {
        self.next_request += 1;
        self.directory_request = Some(self.next_request);
        self.refresh_at = now + 15_000.0;
        self.loading = true;
        self.actions.push(HostedMenuAction::LoadParties { request_id: self.next_request });
        self.paint()
    }

    fn begin_join(&mut self, listing: Option<String>) -> u64 {
        self.next_request += 1;
        self.join_request = Some(self.next_request);
        self.pending_listing = listing;
        self.join_error = None;
        self.busy.set(true);
        self.next_request
    }

    fn paint(&self) -> Result<(), String> {
        self.join_button
            .toggle_attribute_with_force(
                "disabled",
                self.busy.get() || !complete_party_code(&self.code.value()),
            )
            .map_err(|_| "Party ID submit admission")?;
        let error = self.join_error.as_deref().or(self.directory_error.as_deref());
        self.error.set_hidden(error.is_none());
        self.error.set_text_content(error);
        self.list
            .set_attribute("aria-busy", if self.loading { "true" } else { "false" })
            .map_err(|_| "Party directory loading")?;
        self.list.set_text_content(None);
        if self.parties.is_empty() {
            text(
                &self.document,
                &self.list,
                "p",
                "",
                if self.loading {
                    "CONSULTING THE DARK CLOUD…"
                } else {
                    "NO PARTIES ARE FORMING RIGHT NOW."
                },
            )?;
            return Ok(());
        }
        for party in &self.parties {
            self.paint_party(party)?;
        }
        Ok(())
    }

    fn paint_party(&self, party: &PublicGameParty) -> Result<(), String> {
        let row = element(&self.document, "article", "")?;
        row.set_attribute("role", "listitem").map_err(|_| "Party listing role")?;
        row.set_attribute("data-party-listing", &party.id).map_err(|_| "Party listing identity")?;
        let copy = element(&self.document, "span", "join-party-copy")?;
        text(&self.document, &copy, "strong", "", &party.leader.to_uppercase())?;
        let flags = element(&self.document, "span", "join-party-flags")?;
        if party.session_kind == GameSessionKind::PrivateCollege {
            text(&self.document, &flags, "em", "", "PRIVATE COLLEGE")?;
        }
        if party.mod_count > 0 {
            text(&self.document, &flags, "em", "", &format!("MODDED · {}", party.mod_count))?;
        }
        if party.cheats_enabled {
            text(&self.document, &flags, "em", "cheats", "CHEATS")?;
        }
        append(&copy, &flags)?;
        text(&self.document, &copy, "small", "", &party.members.join(" · "))?;
        let boneyard = match &party.activity {
            PartyActivity::Hub => None,
            PartyActivity::Playing { boneyard_name } => Some(boneyard_name.as_str()),
        };
        let presentation =
            directory_party_presentation(party.member_count, party.max_members, boneyard);
        let status_line = element(&self.document, "small", "")?;
        text(
            &self.document,
            &status_line,
            "span",
            if boneyard.is_some() { "join-party-status playing" } else { "join-party-status hub" },
            presentation.status,
        )?;
        let separator = text(&self.document, &status_line, "span", "", " · ")?;
        separator.set_attribute("aria-hidden", "true").map_err(|_| "Party listing separator")?;
        text(&self.document, &status_line, "span", "join-party-location", presentation.location)?;
        append(&copy, &status_line)?;
        append(&row, &copy)?;
        let squad = text(&self.document, &row, "span", "join-party-squad", &presentation.squad)?;
        squad
            .set_attribute("aria-label", &format!("{} wizards", presentation.squad))
            .map_err(|_| "Party size label")?;
        let action = directory_party_action(party.visibility, boneyard.is_some());
        let label = match action {
            DirectoryPartyAction::Wait => "IN GAME",
            _ if self.pending_listing.as_deref() == Some(party.id.as_str()) => "REQUESTED",
            DirectoryPartyAction::Request => "REQUEST",
            DirectoryPartyAction::Join => "JOIN",
        };
        let button = action_button(&self.document, label, &format!("join-listing:{}", party.id))?;
        button
            .toggle_attribute_with_force(
                "disabled",
                self.busy.get() || action == DirectoryPartyAction::Wait,
            )
            .map_err(|_| "Party listing admission")?;
        append(&row, &button)?;
        append(&self.list, &row)
    }
}

fn text(
    document: &Document,
    parent: &HtmlElement,
    tag: &str,
    class: &str,
    value: &str,
) -> Result<HtmlElement, String> {
    let node = element(document, tag, class)?;
    node.set_text_content(Some(value));
    append(parent, &node)?;
    Ok(node)
}

fn action_button(document: &Document, label: &str, action: &str) -> Result<HtmlElement, String> {
    let button = element(document, "button", "")?;
    button.set_attribute("type", "button").map_err(|_| "Join Party action type")?;
    button.set_attribute("data-menu-action", action).map_err(|_| "Join Party action")?;
    button.set_text_content(Some(label));
    Ok(button)
}
