//! GPU `CheatMenu.tsx`; host commands, catalog admission and VM request IDs remain canonical.

mod console;
mod controls;
use crate::gpu_ui::UiFrame;
use console::ConsoleView;
use controls::CheatControls;
use egui::{Color32, FontFamily, FontId, Id, Order, Rect, RichText, Stroke, pos2, vec2};
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
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::{JsCast, prelude::*};
use web_sys::{Document, Event, KeyboardEvent};

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
}

struct PendingAction {
    id: u64,
    label: String,
}

pub struct CheatUi {
    document: Document,
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
}
struct OpenView {
    controls: CheatControls,
    console: ConsoleView,
    console_tab: bool,
    catalog_loading: bool,
    catalog_error: Option<String>,
    action_receipt: Option<(String, &'static str)>,
}
impl CheatUi {
    pub fn attach(document: &Document) -> Result<Self, String> {
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

        Ok(Self {
            document: document.clone(),
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
        let view = OpenView::new(&self.document, frame)?;
        self.view = Some(view);
        self.key_admission.borrow_mut().open = true;
        let id = self.request(CHEAT_MENU_CATALOG_QUERY.to_owned());
        self.catalog_request = Some(id);
        Ok(true)
    }

    /// Unmounting discards form drafts and console history, matching source React
    /// lifetime. The request counter survives so late VM replies cannot reopen it.
    pub fn close(&mut self) {
        self.view = None;
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
            view.catalog_loading = false;
            let result = if *ok {
                decode_cheat_menu_catalogs(values)
            } else {
                Err(error.clone().unwrap_or_else(|| "Catalog query failed.".to_owned()))
            };
            let catalogs = match result {
                Ok(catalogs) => catalogs,
                Err(error) => {
                    view.catalog_error = Some(error);
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
            view.action_receipt = Some((message, if *ok { "ok" } else { "error" }));
            return Ok(true);
        }
        view.console.complete(*request_id, *ok, output, values, error.as_deref())
    }

    fn process(&mut self, event: UiEvent) -> Result<(), String> {
        match event {
            UiEvent::Open => {
                self.open()?;
            }
            UiEvent::Close => self.close(),
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
            "cheats" | "console" => view.tab(action),
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
                            view.action_receipt = Some((format!("{label}…"), "working"));
                            let id = self.request(code);
                            self.action_request =
                                Some(PendingAction { id, label: label.to_owned() });
                        }
                        Err(error) => view.action_receipt = Some((error, "error")),
                    },
                    Ok(None) => {}
                    Err(error) => view.action_receipt = Some((error, "error")),
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

    pub fn paint(&mut self, gpu: &mut UiFrame<'_>) -> Result<(), String> {
        let (Some(view), Some(frame)) = (&mut self.view, &self.frame) else { return Ok(()) };
        gpu.input_scope(if view.console_tab { "cheats/console" } else { "cheats/controls" });
        let ctx = gpu.ctx.clone();
        let screen = ctx.content_rect();
        let bounds = Rect::from_center_size(
            screen.center(),
            vec2(1040.0_f32.min(screen.width() - 24.0), 820.0_f32.min(screen.height() - 24.0)),
        );
        let mut action = None;
        let mut console_error = None;
        egui::Area::new(Id::unique("cheats/dialog"))
            .order(Order::Foreground)
            .fixed_pos(egui::Pos2::ZERO)
            .show(&ctx, |ui| {
                ui.memory_mut(|memory| memory.set_modal_layer(ui.layer_id()));
                ui.set_min_size(screen.size());
                ui.interact(screen, Id::unique("cheats/backdrop"), egui::Sense::click());
                ui.painter().rect_filled(screen, 0.0, Color32::from_black_alpha(209));
                ui.painter().rect_filled(bounds, 0.0, Color32::from_rgb(14, 13, 12));
                if let Some(size) = gpu.image_size("assets/stock/settings-leather.png") {
                    let painter = ui.painter().with_clip_rect(bounds);
                    let mut y = bounds.top();
                    while y < bounds.bottom() {
                        let mut x = bounds.left();
                        while x < bounds.right() {
                            gpu.image(
                                &painter,
                                "assets/stock/settings-leather.png",
                                Rect::from_min_size(pos2(x, y), size),
                                None,
                                Color32::from_white_alpha(150),
                            );
                            x += size.x;
                        }
                        y += size.y;
                    }
                }
                ui.painter().rect_stroke(
                    bounds,
                    0.0,
                    Stroke::new(3.0, Color32::from_rgb(159, 133, 71)),
                    egui::StrokeKind::Inside,
                );
                paint_native_art(gpu, ui.painter(), bounds);
                let inner = bounds.shrink(20.0);
                ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(copy("DEBUG MENU", 22.0));
                            ui.label(
                                RichText::new(&frame.identity)
                                    .font(FontId::new(
                                        10.0,
                                        FontFamily::Name("JetBrains Mono".into()),
                                    ))
                                    .color(Color32::from_rgb(169, 156, 118)),
                            );
                        });
                        for (id, label, selected) in [
                            ("cheats", "CHEATS", !view.console_tab),
                            ("console", "CONSOLE", view.console_tab),
                        ] {
                            let response = ui.add(
                                egui::Button::new(copy(label, 14.0))
                                    .selected(selected)
                                    .min_size(vec2(100.0, 48.0)),
                            );
                            gpu.record(&format!("cheats/tab:{id}"), label, "tab", &response);
                            gpu.record_value(&format!("cheats/tab:{id}"), id, Some(selected));
                            if response.clicked() {
                                action = Some(id.to_owned());
                            }
                        }
                        if button(gpu, ui, "close", "×", true).clicked() {
                            action = Some("close".into());
                        }
                    });
                    ui.separator();
                    if view.console_tab {
                        match view.console.paint(gpu, ui) {
                            Ok(Some(next)) => action = Some(next),
                            Ok(None) => {}
                            Err(error) => console_error = Some(error),
                        }
                    } else {
                        egui::ScrollArea::vertical()
                            .id_salt("cheats/content")
                            .max_height((inner.height() - 72.0).max(50.0))
                            .show(ui, |ui| {
                                if let Some(next) = view.controls.paint(
                                    gpu,
                                    ui,
                                    frame,
                                    self.action_request.is_some(),
                                    self.catalogs.as_ref(),
                                ) {
                                    action = Some(next);
                                }
                                if view.catalog_loading {
                                    ui.label(copy("Loading semantic catalogs…", 12.0));
                                }
                                if let Some(error) = &view.catalog_error {
                                    let response =
                                        ui.colored_label(Color32::from_rgb(255, 142, 120), error);
                                    gpu.record("cheats/catalog-error", error, "alert", &response);
                                }
                                if let Some((message, tone)) = &view.action_receipt {
                                    let response = ui.colored_label(
                                        if *tone == "error" {
                                            Color32::from_rgb(255, 142, 120)
                                        } else {
                                            Color32::from_rgb(217, 202, 155)
                                        },
                                        message,
                                    );
                                    gpu.record(
                                        "cheats/action-receipt",
                                        message,
                                        if *tone == "error" { "alert" } else { "status" },
                                        &response,
                                    );
                                }
                            });
                    }
                });
                gpu.record_rect("cheats/dialog", "Cheat menu", "dialog", bounds, true);
            });
        if let Some(error) = console_error {
            return Err(error);
        }
        if let Some(action) = action {
            self.activate(&action)?;
        }
        Ok(())
    }
}
impl Drop for CheatUi {
    fn drop(&mut self) {
        if let Some(window) = self.document.default_view()
            && let Err(error) = window.remove_event_listener_with_callback_and_bool(
                "keydown",
                self.key.as_ref().unchecked_ref(),
                true,
            )
        {
            web_sys::console::error_1(&error);
        }
    }
}
impl OpenView {
    fn new(document: &Document, frame: &CheatFrame) -> Result<Self, String> {
        Ok(Self {
            controls: CheatControls::new(document, frame)?,
            console: ConsoleView::new(),
            console_tab: false,
            catalog_loading: true,
            catalog_error: None,
            action_receipt: None,
        })
    }
    fn tab(&mut self, tab: &str) {
        self.console_tab = tab == "console";
        if self.console_tab {
            self.console.focus();
        }
    }
}
fn copy(text: &str, size: f32) -> RichText {
    RichText::new(text)
        .font(FontId::new(size, FontFamily::Name("Cinzel Bold".into())))
        .color(Color32::from_rgb(240, 220, 167))
}
fn button(
    gpu: &UiFrame<'_>,
    ui: &mut egui::Ui,
    id: &str,
    label: &str,
    enabled: bool,
) -> egui::Response {
    let response = ui.add_enabled(
        enabled,
        egui::Button::new(copy(label, 12.0))
            .fill(Color32::from_rgb(30, 25, 14))
            .stroke(Stroke::new(1.0, Color32::from_rgb(128, 107, 59)))
            .corner_radius(0.0)
            .min_size(vec2(80.0, 36.0)),
    );
    gpu.record(&format!("cheats/{id}"), label, "button", &response);
    response
}
fn paint_native_art(gpu: &UiFrame<'_>, painter: &egui::Painter, bounds: Rect) {
    let source = Rect::from_min_size(pos2(743.0, 588.0), vec2(80.0, 83.0));
    for (point, mirror_x, mirror_y) in [
        (bounds.min - vec2(8.0, 8.0), false, false),
        (bounds.right_top() + vec2(-72.0, -8.0), true, false),
        (bounds.left_bottom() + vec2(-8.0, -75.0), false, true),
        (bounds.max - vec2(72.0, 75.0), true, true),
    ] {
        let mut uv = source;
        if mirror_x {
            std::mem::swap(&mut uv.min.x, &mut uv.max.x);
        }
        if mirror_y {
            std::mem::swap(&mut uv.min.y, &mut uv.max.y);
        }
        gpu.atlas_image(
            painter,
            "assets/stock/UI.png",
            uv,
            Rect::from_min_size(point, vec2(80.0, 83.0)),
            Color32::WHITE,
        );
    }
    if bounds.width() > 700.0 {
        for (x, mirror) in [(bounds.left() - 52.0, false), (bounds.right() - 15.0, true)] {
            let mut source = Rect::from_min_size(pos2(543.0, 205.0), vec2(67.0, 262.0));
            if mirror {
                std::mem::swap(&mut source.min.x, &mut source.max.x);
            }
            gpu.atlas_image(
                painter,
                "assets/stock/UI.png",
                source,
                Rect::from_center_size(pos2(x + 33.5, bounds.center().y), vec2(67.0, 262.0)),
                Color32::WHITE,
            );
        }
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
