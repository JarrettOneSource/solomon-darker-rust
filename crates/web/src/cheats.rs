//! The source `CheatMenu.tsx` debug surface. The actual host executes every command;
//! this module owns only browser form drafts, bounded console output and request identity.

mod console;
mod controls;

use std::cell::RefCell;
use std::rc::Rc;

use solomon_protocol::hosted::{HostedCommand, HostedServerMessage};
use solomon_sim::boneyard::waves::director::WavePhase;
use solomon_sim::game_run::GameRunPhase;
use solomon_sim::menu::cheats::{
    CHEAT_MENU_CATALOG_QUERY, CheatMenuCatalogs, compile_cheat_menu_action,
    decode_cheat_menu_catalogs, game_cheat_menu_available,
};
use solomon_sim::modding::value::LuaValue;
use solomon_sim::session::GameSession;
use solomon_sim::social::GameSessionKind;
use wasm_bindgen::{JsCast, prelude::*};
use web_sys::{Document, Element, Event, HtmlElement, KeyboardEvent};

use crate::ui::{append, element};
use console::ConsoleView;
use controls::CheatControls;

/// Borrowed accepted state. `can_open` and `retain_open` are the source scene/modal
/// admission predicates, supplied by the same owner that admits gameplay input.
#[derive(Clone, Copy)]
pub struct CheatViewSnapshot<'a> {
    pub session: &'a GameSession,
    pub local_player_id: &'a str,
    pub open_key_code: &'a str,
    pub session_kind: GameSessionKind,
    pub developer_access: bool,
    pub cheats_enabled: bool,
    pub is_host: bool,
    pub can_open: bool,
    pub retain_open: bool,
}

#[derive(Clone, Debug, PartialEq)]
struct PlayerView {
    id: String,
    name: String,
    stats: [String; 6],
    gold: u64,
}

struct CheatFrame {
    players: Vec<PlayerView>,
    local_player_id: String,
    identity: String,
    developer: bool,
    hub_seed_available: bool,
    enemy_spawn_available: bool,
    next_wave_spawn_available: bool,
    bot_summon_available: bool,
}

#[derive(Default)]
struct KeyAdmission {
    open: bool,
    can_open: bool,
    code: String,
}

enum UiEvent {
    Open,
    Close,
    Action(String),
    Target,
    Skill,
    ConsoleEdited,
    History(i32),
}

struct PendingAction {
    id: u64,
    label: String,
}

pub struct CheatUi {
    document: Document,
    root: HtmlElement,
    view: Option<OpenView>,
    frame: Option<CheatFrame>,
    events: Rc<RefCell<Vec<UiEvent>>>,
    key_admission: Rc<RefCell<KeyAdmission>>,
    commands: Vec<HostedCommand>,
    next_request_id: u64,
    catalog_request: Option<u64>,
    action_request: Option<PendingAction>,
    catalogs: Option<CheatMenuCatalogs>,
    key: Closure<dyn FnMut(Event)>,
    click: Closure<dyn FnMut(Event)>,
    submit: Closure<dyn FnMut(Event)>,
    input: Closure<dyn FnMut(Event)>,
}

struct OpenView {
    dialog: HtmlElement,
    identity: HtmlElement,
    cheats_tab: HtmlElement,
    console_tab: HtmlElement,
    controls: CheatControls,
    console: ConsoleView,
    catalog_loading: HtmlElement,
    catalog_error: HtmlElement,
    action_receipt: HtmlElement,
}

impl CheatUi {
    /// All art comes from the already loaded native-settings CSS atlas. No debug
    /// catalog or script is evaluated until the admitted menu actually opens.
    pub fn attach(document: &Document) -> Result<Self, String> {
        let root = element(document, "div", "game-settings-backdrop cheat-menu-backdrop")?;
        root.set_attribute("role", "presentation").map_err(|_| "cheat backdrop semantics")?;
        root.set_hidden(true);
        let events = Rc::new(RefCell::new(Vec::new()));
        let key_admission = Rc::new(RefCell::new(KeyAdmission::default()));
        let pending = events.clone();
        let admission = key_admission.clone();
        let key = Closure::new(move |event: Event| {
            let Some(key) = event.dyn_ref::<KeyboardEvent>() else { return };
            let state = admission.borrow();
            if key.repeat() || key.alt_key() || key.ctrl_key() || key.meta_key() {
                return;
            }
            let closes = state.open && (key.code() == state.code || key.key() == "Escape");
            let opens = !state.open && state.can_open && key.code() == state.code;
            if !closes && !opens {
                return;
            }
            event.prevent_default();
            event.stop_immediate_propagation();
            pending.borrow_mut().push(if closes { UiEvent::Close } else { UiEvent::Open });
        });
        let window = document.default_view().ok_or("cheat window")?;
        window
            .add_event_listener_with_callback_and_bool(
                "keydown",
                key.as_ref().unchecked_ref(),
                true,
            )
            .map_err(|_| "cheat binding listener")?;

        let pending = events.clone();
        let click = Closure::new(move |event: Event| {
            if let Some(action) = event_action(&event, "[data-cheat-action]", "data-cheat-action") {
                pending.borrow_mut().push(UiEvent::Action(action));
            }
        });
        root.add_event_listener_with_callback("click", click.as_ref().unchecked_ref())
            .map_err(|_| "cheat click listener")?;
        let pending = events.clone();
        let submit = Closure::new(move |event: Event| {
            if let Some(action) = event_action(&event, "[data-cheat-submit]", "data-cheat-submit") {
                event.prevent_default();
                pending.borrow_mut().push(UiEvent::Action(action));
            }
        });
        root.add_event_listener_with_callback("submit", submit.as_ref().unchecked_ref())
            .map_err(|_| "cheat form listener")?;
        let pending = events.clone();
        let input = Closure::new(move |event: Event| {
            let Some(target) = event.target().and_then(|target| target.dyn_into::<Element>().ok())
            else {
                return;
            };
            let action = match target.get_attribute("data-cheat-field").as_deref() {
                Some("target") => UiEvent::Target,
                Some("skill") => UiEvent::Skill,
                Some("console") => UiEvent::ConsoleEdited,
                _ => return,
            };
            pending.borrow_mut().push(action);
        });
        root.add_event_listener_with_callback("input", input.as_ref().unchecked_ref())
            .map_err(|_| "cheat field listener")?;
        append(&document.body().ok_or("cheat document body")?, &root)?;
        Ok(Self {
            document: document.clone(),
            root,
            view: None,
            frame: None,
            events,
            key_admission,
            commands: Vec::new(),
            next_request_id: 1,
            catalog_request: None,
            action_request: None,
            catalogs: None,
            key,
            click,
            submit,
            input,
        })
    }

    #[must_use]
    pub fn is_open(&self) -> bool {
        self.view.is_some()
    }

    pub fn update(&mut self, snapshot: Option<CheatViewSnapshot<'_>>) -> Result<(), String> {
        let Some(snapshot) = snapshot else {
            self.close();
            self.frame = None;
            self.key_admission.borrow_mut().can_open = false;
            return Ok(());
        };
        let available = game_cheat_menu_available(
            snapshot.developer_access,
            snapshot.cheats_enabled,
            snapshot.is_host,
        );
        {
            let mut admission = self.key_admission.borrow_mut();
            admission.can_open = available && snapshot.can_open;
            admission.code = snapshot.open_key_code.to_owned();
        }
        if self.is_open() && (!available || !snapshot.retain_open) {
            self.close();
        }
        self.frame = Some(CheatFrame::from_snapshot(snapshot));
        let events = std::mem::take(&mut *self.events.borrow_mut());
        for event in events {
            self.process(event)?;
        }
        if let (Some(view), Some(frame)) = (&mut self.view, &self.frame) {
            view.identity.set_text_content(Some(&frame.identity));
            view.controls.update(frame, self.action_request.is_some(), self.catalogs.as_ref())?;
        }
        Ok(())
    }

    /// A pause-menu request calls this after the actual resume grace has ended.
    pub fn open(&mut self) -> Result<bool, String> {
        if self.is_open() || !self.key_admission.borrow().can_open {
            return Ok(false);
        }
        let Some(frame) = &self.frame else { return Ok(false) };
        let view = OpenView::new(&self.document, frame, &self.events)?;
        append(&self.root, &view.dialog)?;
        self.view = Some(view);
        self.root.set_hidden(false);
        self.key_admission.borrow_mut().open = true;
        let id = self.request(CHEAT_MENU_CATALOG_QUERY.to_owned());
        self.catalog_request = Some(id);
        Ok(true)
    }

    /// Unmounting discards form drafts and console history, matching source React
    /// lifetime. The request counter survives so late VM replies cannot reopen it.
    pub fn close(&mut self) {
        self.view = None;
        self.root.set_text_content(None);
        self.root.set_hidden(true);
        self.key_admission.borrow_mut().open = false;
        self.catalog_request = None;
        self.action_request = None;
        self.catalogs = None;
        self.commands.clear();
        self.events.borrow_mut().clear();
    }

    pub fn take_commands(&mut self) -> Vec<HostedCommand> {
        std::mem::take(&mut self.commands)
    }

    /// Other hosted messages retain their existing owners, including CheatMode.
    pub fn receive_hosted_message(
        &mut self,
        message: &HostedServerMessage,
    ) -> Result<bool, String> {
        let HostedServerMessage::LuaResult { error, ok, output, request_id, values } = message
        else {
            return Ok(false);
        };
        let Some(view) = &mut self.view else { return Ok(false) };
        if self.catalog_request == Some(*request_id) {
            self.catalog_request = None;
            view.catalog_loading.set_hidden(true);
            let result = if *ok {
                decode_cheat_menu_catalogs(values)
            } else {
                Err(error.clone().unwrap_or_else(|| "Catalog query failed.".to_owned()))
            };
            let catalogs = match result {
                Ok(catalogs) => catalogs,
                Err(error) => {
                    view.catalog_error.set_text_content(Some(&error));
                    view.catalog_error.set_hidden(false);
                    CheatMenuCatalogs::default()
                }
            };
            view.controls.set_catalogs(&self.document, &catalogs)?;
            self.catalogs = Some(catalogs);
            return Ok(true);
        }
        if self.action_request.as_ref().is_some_and(|request| request.id == *request_id) {
            let request = self.action_request.take().expect("matched active debug action");
            let message = if *ok {
                let returned = format_values(values)?;
                if returned.is_empty() {
                    format!("{} accepted.", request.label)
                } else {
                    format!("{}: {returned}", request.label)
                }
            } else {
                error.clone().unwrap_or_else(|| format!("{} failed.", request.label))
            };
            receipt(&view.action_receipt, &message, if *ok { "ok" } else { "error" })?;
            return Ok(true);
        }
        view.console.complete(&self.document, *request_id, *ok, output, values, error.as_deref())
    }

    fn process(&mut self, event: UiEvent) -> Result<(), String> {
        match event {
            UiEvent::Open => {
                self.open()?;
            }
            UiEvent::Close => self.close(),
            UiEvent::History(direction) => {
                if let Some(view) = &mut self.view {
                    view.console.navigate_history(direction)?;
                }
            }
            UiEvent::ConsoleEdited => {
                if let Some(view) = &mut self.view {
                    view.console.edited()?;
                }
            }
            UiEvent::Target => {
                if let (Some(view), Some(frame)) = (&self.view, &self.frame) {
                    view.controls.target_changed(frame);
                }
            }
            UiEvent::Skill => {
                if let (Some(view), Some(catalogs)) = (&self.view, &self.catalogs) {
                    view.controls.skill_changed(catalogs)?;
                }
            }
            UiEvent::Action(action) => self.activate(&action)?,
        }
        Ok(())
    }

    fn activate(&mut self, action: &str) -> Result<(), String> {
        if action == "close" {
            self.close();
            return Ok(());
        }
        let Some(view) = &mut self.view else { return Ok(()) };
        match action {
            "cheats" | "console" => view.tab(action)?,
            "clear" => view.console.clear()?,
            "run" => {
                if let Some(code) = view.console.prepare()? {
                    let id = self.request(code);
                    self.view.as_mut().expect("console is open").console.set_request(id);
                }
            }
            _ => {
                if self.action_request.is_some() {
                    return Ok(());
                }
                let Some(frame) = &self.frame else { return Ok(()) };
                let result = view.controls.action(action, frame, self.catalogs.as_ref());
                match result {
                    Ok(Some((label, action))) => match compile_cheat_menu_action(&action) {
                        Ok(code) => {
                            receipt(&view.action_receipt, &format!("{label}…"), "working")?;
                            let id = self.request(code);
                            self.action_request =
                                Some(PendingAction { id, label: label.to_owned() });
                        }
                        Err(error) => receipt(&view.action_receipt, &error, "error")?,
                    },
                    Ok(None) => {}
                    Err(error) => receipt(&view.action_receipt, &error, "error")?,
                }
            }
        }
        Ok(())
    }

    fn request(&mut self, code: String) -> u64 {
        let id = self.next_request_id;
        self.next_request_id += 1;
        self.commands.push(HostedCommand::LuaExecute { request_id: id, code });
        id
    }
}

impl Drop for CheatUi {
    fn drop(&mut self) {
        if let Some(window) = self.document.default_view() {
            window
                .remove_event_listener_with_callback_and_bool(
                    "keydown",
                    self.key.as_ref().unchecked_ref(),
                    true,
                )
                .expect("remove the installed cheat key listener");
        }
        for (name, listener) in
            [("click", &self.click), ("submit", &self.submit), ("input", &self.input)]
        {
            self.root
                .remove_event_listener_with_callback(name, listener.as_ref().unchecked_ref())
                .expect("remove the installed cheat form listener");
        }
        self.root.remove();
    }
}

impl OpenView {
    fn new(
        document: &Document,
        frame: &CheatFrame,
        events: &Rc<RefCell<Vec<UiEvent>>>,
    ) -> Result<Self, String> {
        let dialog = element(document, "section", "game-settings-dialog cheat-menu-dialog")?;
        for (name, value) in
            [("aria-label", "Cheat menu"), ("aria-modal", "true"), ("role", "dialog")]
        {
            dialog.set_attribute(name, value).map_err(|_| "cheat dialog semantics")?;
        }
        // NativePanelArt uses the existing CSS atlas crops without registered-sprite
        // offsets; its flourishes stay centered at every debug-dialog height.
        let art = element(document, "div", "game-settings-native-art")?;
        art.set_attribute("aria-hidden", "true").map_err(|_| "cheat frame semantics")?;
        for corner in ["top-left", "top-right", "bottom-left", "bottom-right"] {
            append(
                &art,
                &element(document, "i", &format!("game-settings-frame-corner {corner}"))?,
            )?;
        }
        for side in ["left", "right"] {
            append(
                &art,
                &element(document, "i", &format!("game-settings-frame-flourish {side}"))?,
            )?;
        }
        append(&dialog, &art)?;
        let header = element(document, "header", "cheat-menu-topbar")?;
        let identity_box = element(document, "div", "cheat-menu-identity")?;
        text(document, &identity_box, "strong", "", "DEBUG MENU")?;
        let identity = text(document, &identity_box, "span", "", &frame.identity)?;
        append(&header, &identity_box)?;
        let tabs = element(document, "nav", "cheat-menu-tabs")?;
        tabs.set_attribute("role", "tablist").map_err(|_| "cheat tabs role")?;
        tabs.set_attribute("aria-label", "Debug menu tabs").map_err(|_| "cheat tabs label")?;
        let cheats_tab = tab_button(document, "cheats")?;
        let console_tab = tab_button(document, "console")?;
        append(&tabs, &cheats_tab)?;
        append(&tabs, &console_tab)?;
        append(&header, &tabs)?;
        let close = button(document, "×", "close", "cheat-menu-close")?;
        close.set_attribute("aria-label", "Close cheat menu").map_err(|_| "cheat close label")?;
        close.set_attribute("data-game-back", "true").map_err(|_| "cheat back control")?;
        append(&header, &close)?;
        append(&dialog, &header)?;
        let controls = CheatControls::new(document, frame)?;
        let catalog_loading = text(
            document,
            &controls.root,
            "p",
            "cheat-menu-receipt",
            "Loading semantic catalogs…",
        )?;
        catalog_loading.set_attribute("data-tone", "working").map_err(|_| "cheat loading tone")?;
        catalog_loading.set_attribute("role", "status").map_err(|_| "cheat loading status")?;
        let catalog_error = text(document, &controls.root, "p", "cheat-menu-receipt", "")?;
        catalog_error.set_hidden(true);
        receipt(&catalog_error, "", "error")?;
        catalog_error.set_hidden(true);
        let action_receipt = element(document, "p", "cheat-menu-receipt")?;
        action_receipt.set_hidden(true);
        append(&controls.root, &action_receipt)?;
        append(&dialog, &controls.root)?;
        let console = ConsoleView::new(document, events)?;
        append(&dialog, &console.root)?;
        let mut view = Self {
            dialog,
            identity,
            cheats_tab,
            console_tab,
            controls,
            console,
            catalog_loading,
            catalog_error,
            action_receipt,
        };
        view.tab("cheats")?;
        Ok(view)
    }

    fn tab(&mut self, tab: &str) -> Result<(), String> {
        let console = tab == "console";
        self.dialog.set_attribute("data-cheat-menu-tab", tab).map_err(|_| "cheat active tab")?;
        self.cheats_tab
            .set_attribute("aria-selected", if console { "false" } else { "true" })
            .map_err(|_| "cheat tab selection")?;
        self.console_tab
            .set_attribute("aria-selected", if console { "true" } else { "false" })
            .map_err(|_| "console tab selection")?;
        self.controls.root.set_hidden(console);
        self.console.root.set_hidden(!console);
        if console {
            self.console.focus()?;
        }
        Ok(())
    }
}

impl CheatFrame {
    fn from_snapshot(snapshot: CheatViewSnapshot<'_>) -> Self {
        let session = snapshot.session;
        let mut players = Vec::new();
        for id in &session.participant_order {
            let participant = &session.participants[id];
            let player = &participant.player;
            let combat = &player.combat;
            let state = match combat.life_state {
                solomon_sim::combat::PlayerLifeState::Alive => "ALIVE",
                solomon_sim::combat::PlayerLifeState::LethalPending => "LETHAL-PENDING",
                solomon_sim::combat::PlayerLifeState::Dying => "DYING",
                solomon_sim::combat::PlayerLifeState::Spectating => "SPECTATING",
            };
            players.push(PlayerView {
                id: id.clone(),
                name: player.selection.name.clone(),
                gold: player.economy.gold,
                stats: [
                    format!("{} / {}", round(combat.current_health), round(combat.maximum_health)),
                    format!("{} / {}", round(combat.current_mana), round(combat.maximum_mana)),
                    format!(
                        "{} / {}",
                        player.progression.level,
                        round(player.progression.experience)
                    ),
                    player.economy.gold.to_string(),
                    state.to_owned(),
                    format!(
                        "{}, {}",
                        round(participant.wizard.position.x),
                        round(participant.wizard.position.y)
                    ),
                ],
            });
        }
        let scene = match &session.boneyard {
            None => "HUB".to_owned(),
            Some(world) => format!(
                "{} · {}",
                if world.tutorial.is_some() { "TUTORIAL" } else { "BONEYARD" },
                session.run.phase.id().to_ascii_uppercase()
            ),
        };
        Self {
            players,
            local_player_id: snapshot.local_player_id.to_owned(),
            identity: format!(
                "{} · {scene} · TICK {}",
                if snapshot.developer_access { "DEVELOPER" } else { "CHEATS ON" },
                session.ticks
            ),
            developer: snapshot.developer_access,
            hub_seed_available: session.boneyard.is_none()
                && session.run.phase == GameRunPhase::Hub,
            enemy_spawn_available: session.boneyard.is_some()
                && session.run.phase == GameRunPhase::Active,
            next_wave_spawn_available: session.run.phase == GameRunPhase::Active
                && session.level_up_barrier.is_none()
                && session.boneyard.as_ref().is_some_and(|world| {
                    world.tutorial.is_none()
                        && world.waves.as_ref().is_some_and(|waves| {
                            waves.state.pending_spawn_budget == 0
                                && !matches!(
                                    waves.state.phase,
                                    WavePhase::Dormant | WavePhase::Opening | WavePhase::Spawning
                                )
                        })
                }),
            bot_summon_available: snapshot.developer_access
                && snapshot.session_kind == GameSessionKind::GlobalHub
                && session.boneyard.is_none(),
        }
    }
}

fn round(value: f64) -> f64 {
    let rounded = js_sys::Math::round(value * 100.0) / 100.0;
    if rounded == 0.0 { 0.0 } else { rounded }
}

fn tab_button(document: &Document, tab: &str) -> Result<HtmlElement, String> {
    let button = button(document, &tab.to_ascii_uppercase(), tab, "")?;
    for (name, value) in [
        ("role", "tab".to_owned()),
        ("id", format!("cheat-menu-{tab}-tab")),
        ("aria-controls", format!("cheat-menu-{tab}")),
    ] {
        button.set_attribute(name, &value).map_err(|_| "cheat tab semantics")?;
    }
    Ok(button)
}

fn button(
    document: &Document,
    label: &str,
    action: &str,
    class: &str,
) -> Result<HtmlElement, String> {
    let button = element(document, "button", class)?;
    button.set_attribute("type", "button").map_err(|_| "cheat button type")?;
    button.set_attribute("data-cheat-action", action).map_err(|_| "cheat button action")?;
    button.set_text_content(Some(label));
    Ok(button)
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

fn receipt(node: &HtmlElement, message: &str, tone: &str) -> Result<(), String> {
    node.set_text_content(Some(message));
    node.set_attribute("data-tone", tone).map_err(|_| "cheat receipt tone")?;
    node.set_attribute("role", if tone == "error" { "alert" } else { "status" })
        .map_err(|_| "cheat receipt status")?;
    node.set_hidden(false);
    Ok(())
}

fn event_action(event: &Event, selector: &str, attribute: &str) -> Option<String> {
    let target = event.target()?.dyn_into::<Element>().ok()?;
    target.closest(selector).ok()??.get_attribute(attribute)
}

/// Keep the browser's JSON number formatting and insertion order, as in source
/// `formatCheatConsoleValues`; LuaValue serialization is already the wire contract.
fn format_values(values: &[LuaValue]) -> Result<String, String> {
    let mut lines = Vec::new();
    for value in values {
        let json = serde_json::to_string(value).map_err(|error| format!("Lua result: {error}"))?;
        let value = js_sys::JSON::parse(&json).map_err(|_| "parse Lua return value")?;
        let formatted = js_sys::JSON::stringify_with_replacer_and_space(
            &value,
            &JsValue::UNDEFINED,
            &JsValue::from_f64(2.0),
        )
        .map_err(|_| "format Lua return value")?;
        lines.push(String::from(formatted));
    }
    Ok(lines.join("\n"))
}
