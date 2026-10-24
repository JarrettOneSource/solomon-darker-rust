//! Browser gameplay input, sampled once per frame. Website `input/gameplay-input.ts`,
//! `standard-gamepad.ts` and `TouchJoystick.tsx` own levels, aim and cancellation.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use crate::hotbar::HotbarSelection;
use solomon_sim::Vec2;
use solomon_sim::belt::{NATIVE_BELT_SLOT_COUNT, PLAYER_BELT_SLOT_COUNT};
use solomon_sim::input::{
    BeltInputSource, GameplayAction, GameplayAim, GameplayInputSample, HeldBeltInputs,
    MovementKeys, StandardGamepadSnapshot, StandardGameplayState, joystick_vector,
    movement_from_gamepad, sample_movement,
};
use solomon_sim::mobile_ui::MobileUiLayoutState;
use solomon_sim::settings::{GameControlAction, GameControlBindings};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{
    Event, EventTarget, Gamepad, GamepadButton, GamepadMappingType, HtmlCanvasElement, HtmlElement,
    KeyboardEvent, MouseEvent, PointerEvent, Window,
};

mod joystick;
mod pointer;
use crate::gpu_ui::{UiDiagnostics, UiFrame};
use crate::hud::HudBeltHit;
use joystick::JoystickGeometry;
use pointer::InputUiState;
const ARROWS: [&str; 4] = ["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"];

#[derive(Clone, Copy, PartialEq, Eq)]
enum AimOwner {
    Mouse,
    Gamepad,
    Touch,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum JoystickLane {
    Movement,
    Primary,
}

#[derive(Clone, Copy)]
struct BeltPointer {
    slot: usize,
    origin: Vec2,
    scale: f64,
    cast_eligible: bool,
}

struct GameplayInputState {
    ui: InputUiState,
    ui_frame: Option<u64>,
    mouse_ui_press: bool,
    viewport: (i32, i32, u16),
    joystick_geometry: [Option<JoystickGeometry>; 2],
    held: HashSet<String>,
    touch: Vec2,
    pointer: Option<i32>,
    touch_primary: Vec2,
    primary_pointer: Option<i32>,
    mouse_primary: bool,
    captured_pointer: Option<Vec2>,
    pending_aim: Option<GameplayAim>,
    aim_owner: Option<AimOwner>,
    aim_was_set: bool,
    gamepad: StandardGameplayState,
    gamepad_quickbar: Option<usize>,
    hotbar_bank: usize,
    belt: HeldBeltInputs,
    belt_pressed: Vec<usize>,
    touch_belt_pointers: HashMap<i32, BeltPointer>,
    belt_unassignments: Vec<usize>,
    world_click: Option<Vec2>,
    spectator_camera_active: bool,
    spectator_item_slots: [bool; PLAYER_BELT_SLOT_COUNT],
    spectator_camera_advances: usize,
    actions: Vec<GameplayAction>,
    blocked: bool,
    primary_enabled: bool,
    secondary_at_pointer: bool,
    controls: GameControlBindings,
    mobile_layout: MobileUiLayoutState,
}

pub struct MovementInput {
    window: Window,
    canvas: HtmlCanvasElement,
    hotbar: HotbarSelection,
    state: Rc<RefCell<GameplayInputState>>,
    menu_requested: Cell<bool>,
    _listeners: Vec<InputListener>,
}
impl GameplayInputState {
    fn new(bank: usize) -> Self {
        Self {
            ui: InputUiState::default(),
            ui_frame: None,
            mouse_ui_press: false,
            viewport: (0, 0, 100),
            joystick_geometry: [None, None],
            held: HashSet::new(),
            touch: Vec2::ZERO,
            pointer: None,
            touch_primary: Vec2::ZERO,
            primary_pointer: None,
            mouse_primary: false,
            captured_pointer: None,
            pending_aim: None,
            aim_owner: None,
            aim_was_set: false,
            gamepad: StandardGameplayState::default(),
            gamepad_quickbar: None,
            hotbar_bank: bank,
            belt: HeldBeltInputs::default(),
            belt_pressed: Vec::new(),
            touch_belt_pointers: HashMap::new(),
            belt_unassignments: Vec::new(),
            world_click: None,
            spectator_camera_active: false,
            spectator_item_slots: [false; PLAYER_BELT_SLOT_COUNT],
            spectator_camera_advances: 0,
            actions: Vec::new(),
            blocked: false,
            primary_enabled: true,
            secondary_at_pointer: true,
            controls: GameControlBindings::default(),
            mobile_layout: MobileUiLayoutState::default(),
        }
    }
    fn world_input_blocked(&self) -> bool {
        self.blocked || self.ui.text || self.ui.scope.is_some()
    }
}
impl MovementInput {
    pub fn attach(window: &Window, hotbar: HotbarSelection) -> Result<Self, String> {
        let document = window.document().ok_or("no document for gameplay input")?;
        let canvas = document
            .get_element_by_id("game")
            .ok_or("no gameplay canvas")?
            .dyn_into::<HtmlCanvasElement>()
            .map_err(|_| "gameplay surface is not a canvas")?;
        // Source joysticks/scene disable native pan before a gesture starts.
        // preventDefault on pointerdown cannot prevent browser pointercancel.
        canvas
            .style()
            .set_property("touch-action", "none")
            .map_err(|_| "cannot configure game canvas touch gestures".to_owned())?;
        let state = Rc::new(RefCell::new(GameplayInputState::new(hotbar.bank())));
        let mut listeners = Vec::new();
        for (name, pressed) in [("keydown", true), ("keyup", false)] {
            let state = state.clone();
            let hotbar = hotbar.clone();
            listen(window.as_ref(), name, &mut listeners, move |event| {
                let Some(key) = event.dyn_ref::<KeyboardEvent>() else {
                    return;
                };
                if pressed && editing_text(key) {
                    return;
                }
                let mut state = state.borrow_mut();
                let code = key.code();
                if pressed && state.ui.text {
                    return;
                }
                if pressed && state.ui.keyboard && matches!(code.as_str(), "Enter" | "Space") {
                    return;
                }
                if pressed
                    && !key.repeat()
                    && !key.alt_key()
                    && !key.ctrl_key()
                    && !key.meta_key()
                    && code == state.controls.open_chat
                {
                    event.prevent_default();
                    state.actions.push(GameplayAction::Chat);
                }
                // The source chat opener remains available over Inventory. Its own
                // view admission rejects title/pause/Tutorial; movement stays sealed.
                if state.world_input_blocked() {
                    return;
                }
                if let Some(slot) = belt_slot(&state.controls, &code) {
                    event.prevent_default();
                    if pressed && !key.repeat() {
                        hold_belt(&mut state, BeltInputSource::Keyboard(slot), hotbar.slot(slot));
                    } else if !pressed {
                        state.belt.release_source(BeltInputSource::Keyboard(slot));
                    }
                }

                if pressed && !key.repeat() && code == "KeyE" {
                    state.actions.push(GameplayAction::Interact);
                }
                if is_movement_key(&code, &state.controls) {
                    event.prevent_default();
                    if pressed {
                        state.held.insert(code);
                    } else {
                        state.held.remove(&code);
                    }
                }
            })?;
        }
        for name in ["mousedown", "mousemove", "mouseup"] {
            let state = state.clone();
            let hotbar = hotbar.clone();
            let canvas = canvas.clone();
            let target: &EventTarget =
                if name == "mousedown" { canvas.as_ref() } else { window.as_ref() };
            let captured_canvas = canvas.clone();
            listen(target, name, &mut listeners, move |event| {
                let Some(mouse) = event.dyn_ref::<MouseEvent>() else {
                    return;
                };
                let Some(point) =
                    canvas_point(&captured_canvas, mouse.client_x(), mouse.client_y())
                else {
                    return;
                };
                let mut state = state.borrow_mut();
                if name == "mousedown" {
                    let point = egui::pos2(
                        mouse.client_x() as f32
                            - captured_canvas.get_bounding_client_rect().left() as f32,
                        mouse.client_y() as f32
                            - captured_canvas.get_bounding_client_rect().top() as f32,
                    );
                    state.mouse_ui_press = state.ui.hit(point).is_some();
                    if state.mouse_ui_press {
                        return;
                    }
                }
                if name == "mousemove" {
                    // Hover keeps the next keyboard belt cast's screen point without taking aim ownership.
                    state.captured_pointer = Some(point);
                    if state.world_input_blocked() {
                        return;
                    }
                    if state.mouse_primary || state.belt.current().is_some() {
                        state.aim_owner = Some(AimOwner::Mouse);
                        state.pending_aim =
                            Some(if state.mouse_primary || state.secondary_at_pointer {
                                GameplayAim::Pointer(point)
                            } else {
                                GameplayAim::Facing
                            });
                        event.prevent_default();
                    }
                    return;
                }
                if state.world_input_blocked() {
                    return;
                }
                let primary = mouse.button() == 0;
                let slot = if primary {
                    None
                } else {
                    belt_slot(&state.controls, &format!("Mouse{}", mouse.button()))
                };
                if !primary && slot.is_none() {
                    return;
                }
                if primary && !state.primary_enabled {
                    return;
                }
                if name == "mousedown"
                    && state.spectator_camera_active
                    && (primary
                        || slot.is_none_or(|slot| !state.spectator_item_slots[hotbar.slot(slot)]))
                {
                    // Boneyard's claimMouseCastStart follows the item press
                    // claim and consumes either mapped cast lane before aim/hold.
                    state.spectator_camera_advances += 1;
                    event.prevent_default();
                    return;
                }
                let was_held = if primary {
                    state.mouse_primary
                } else {
                    slot.is_some_and(|slot| {
                        state.belt.held_slot(BeltInputSource::Mouse(slot)).is_some()
                    })
                };
                if name == "mouseup" && !was_held {
                    return;
                }
                state.captured_pointer = Some(point);
                state.aim_owner = Some(AimOwner::Mouse);
                state.pending_aim = Some(if primary || state.secondary_at_pointer {
                    GameplayAim::Pointer(point)
                } else {
                    GameplayAim::Facing
                });
                if primary {
                    state.mouse_primary = name == "mousedown";
                } else if let Some(slot) = slot {
                    if name == "mousedown" {
                        hold_belt(&mut state, BeltInputSource::Mouse(slot), hotbar.slot(slot));
                    } else {
                        state.belt.release_source(BeltInputSource::Mouse(slot));
                    }
                }
                event.prevent_default();
            })?;
        }
        listen(canvas.as_ref(), "contextmenu", &mut listeners, |event| event.prevent_default())?;
        let clicked = state.clone();
        let clicked_canvas = canvas.clone();
        listen(canvas.as_ref(), "click", &mut listeners, move |event| {
            let Some(mouse) = event.dyn_ref::<MouseEvent>() else { return };
            let mut state = clicked.borrow_mut();
            if state.mouse_ui_press {
                state.mouse_ui_press = false;
                return;
            }
            let bounds = clicked_canvas.get_bounding_client_rect();
            let point = egui::pos2(
                mouse.client_x() as f32 - bounds.left() as f32,
                mouse.client_y() as f32 - bounds.top() as f32,
            );
            if !state.world_input_blocked()
                && !state.spectator_camera_active
                && mouse.button() == 0
                && state.ui.hit(point).is_none()
            {
                state.world_click =
                    canvas_point(&clicked_canvas, mouse.client_x(), mouse.client_y());
            }
        })?;
        for name in
            ["pointerdown", "pointermove", "pointerup", "pointercancel", "lostpointercapture"]
        {
            let captured = state.clone();
            let surface = canvas.clone();
            let bank = hotbar.clone();
            listen(window.as_ref(), name, &mut listeners, move |event| {
                let Some(pointer) = event.dyn_ref::<PointerEvent>() else { return };
                let client = match pointer_client_point(pointer) {
                    Ok(point) => point,
                    Err(message) => {
                        report_input_error(Err(message));
                        return;
                    }
                };
                let bounds = surface.get_bounding_client_rect();
                let point = Vec2::new(client.x - bounds.left(), client.y - bounds.top());
                let id = pointer.pointer_id();
                let outcome = {
                    let mut state = captured.borrow_mut();
                    match name {
                        "pointerdown" => state.begin_ui_pointer(id, pointer.button(), point),
                        "pointermove" => state.move_ui_pointer(id, point, bank.bank()),
                        _ => state.end_ui_pointer(id, name == "pointerup", bank.bank()),
                    }
                };
                if outcome.owned {
                    event.prevent_default();
                }
                if outcome.capture {
                    report_input_error(
                        surface
                            .set_pointer_capture(id)
                            .map_err(|_| "cannot capture gameplay UI pointer".to_owned()),
                    );
                }
                if outcome.release && surface.has_pointer_capture(id) {
                    report_input_error(
                        surface
                            .release_pointer_capture(id)
                            .map_err(|_| "cannot release gameplay UI pointer".to_owned()),
                    );
                }
            })?;
        }
        for (target, name) in [
            (window.as_ref(), "blur"),
            (window.as_ref(), "pagehide"),
            (document.as_ref(), "visibilitychange"),
        ] {
            let captured = state.clone();
            let visibility = document.clone();
            let surface = canvas.clone();
            listen(target, name, &mut listeners, move |_| {
                if name == "visibilitychange" && !visibility.hidden() {
                    return;
                }
                let pointers = {
                    let mut state = captured.borrow_mut();
                    let pointers = state.captured_ui_pointers();
                    clear_input(&mut state);
                    pointers
                };
                release_pointers(&surface, pointers);
            })?;
        }
        Ok(Self {
            window: window.clone(),
            canvas,
            hotbar,
            state,
            menu_requested: Cell::new(false),
            _listeners: listeners,
        })
    }
    pub fn set_ui_geometry(&self, diagnostics: &UiDiagnostics, belt: &[HudBeltHit]) {
        let pointers = {
            let mut state = self.state.borrow_mut();
            let next = InputUiState::from_diagnostics(diagnostics, belt, &state.joystick_geometry);
            let clears = (next.scope != state.ui.scope && next.scope.is_some())
                || (next.text && !state.ui.text);
            let pointers = if clears {
                let pointers = state.captured_ui_pointers();
                clear_input(&mut state);
                pointers
            } else {
                Vec::new()
            };
            state.ui = next;
            if state.ui_frame != Some(diagnostics.frame) {
                state.ui_frame = Some(diagnostics.frame);
                if !state.world_input_blocked() {
                    for hit in belt {
                        if hit.cast_eligible
                            && diagnostics.widgets.iter().any(|widget| {
                                widget.id == format!("hud:belt:{}", hit.slot)
                                    && widget.enabled
                                    && widget.activated_by_keyboard()
                            })
                        {
                            state.activate_ui_belt(hit.slot);
                        }
                    }
                }
            }
            pointers
        };
        release_pointers(&self.canvas, pointers);
    }
    pub fn paint(&mut self, frame: &mut UiFrame<'_>) -> Result<(), String> {
        joystick::paint(self, frame)
    }
    pub fn layer_id(&self) -> egui::LayerId {
        joystick::layer_id()
    }
    pub fn set_blocked(&self, blocked: bool) -> Result<(), String> {
        let pointers = {
            let mut state = self.state.borrow_mut();
            if state.blocked == blocked {
                return Ok(());
            }
            state.blocked = blocked;
            if blocked {
                let pointers = state.captured_ui_pointers();
                clear_input(&mut state);
                self.menu_requested.set(false);
                pointers
            } else {
                Vec::new()
            }
        };
        release_pointers(&self.canvas, pointers);
        Ok(())
    }
    pub fn set_controls(&self, controls: GameControlBindings) -> Result<(), String> {
        let pointers = {
            let mut state = self.state.borrow_mut();
            state.controls = controls;
            let pointers = state.captured_ui_pointers();
            clear_input(&mut state);
            pointers
        };
        release_pointers(&self.canvas, pointers);
        Ok(())
    }
    pub fn set_primary_casting_enabled(&self, enabled: bool) -> Result<(), String> {
        let pointer = {
            let mut state = self.state.borrow_mut();
            if state.primary_enabled == enabled {
                return Ok(());
            }
            state.primary_enabled = enabled;
            if enabled {
                None
            } else {
                state.mouse_primary = false;
                state.touch_primary = Vec2::ZERO;
                state.primary_pointer.take()
            }
        };
        release_pointers(&self.canvas, pointer.into_iter().collect());
        Ok(())
    }
    pub fn set_secondary_at_pointer(&self, enabled: bool) {
        self.state.borrow_mut().secondary_at_pointer = enabled;
    }
    pub fn secondary_aim(&self, pointer: Option<Vec2>) -> GameplayAim {
        let state = self.state.borrow();
        match pointer.or(state.captured_pointer) {
            Some(point) if state.secondary_at_pointer => GameplayAim::Pointer(point),
            _ => GameplayAim::Facing,
        }
    }
    pub fn set_touch_quickbar(&self, slot: usize, pressed: bool, fallback_direction: Option<Vec2>) {
        let mut state = self.state.borrow_mut();
        if state.world_input_blocked() || slot >= NATIVE_BELT_SLOT_COUNT {
            return;
        }
        if pressed {
            if !state.aim_was_set
                && let Some(direction) =
                    fallback_direction.filter(|direction| direction.x.hypot(direction.y) > 0.001)
            {
                state.pending_aim = Some(GameplayAim::Direction(direction));
                state.aim_owner = Some(AimOwner::Touch);
            }
            hold_belt(&mut state, BeltInputSource::Touch(slot), self.hotbar.slot(slot));
        } else {
            state.belt.release_source(BeltInputSource::Touch(slot));
        }
    }
    pub fn sample(&self) -> Result<GameplayInputSample, String> {
        self.reconcile_hotbar_bank()?;
        let mut state = self.state.borrow_mut();
        if state.world_input_blocked() {
            return Ok(GameplayInputSample {
                movement: Vec2::ZERO,
                aim: None,
                primary: false,
                quickbar: None,
                actions: std::mem::take(&mut state.actions),
                selected_belt_slot: state.gamepad.selected_belt_slot(),
                controller_present: false,
                belt_pressed: Vec::new(),
            });
        }
        let pads = read_gamepads(&self.window)?;
        let controller = state.gamepad.sample(&pads);
        if controller.aim_active {
            state.aim_owner = Some(AimOwner::Gamepad);
        }
        if state.aim_owner == Some(AimOwner::Gamepad) && controller.aim_direction.is_none() {
            state.aim_owner = None;
            state.aim_was_set = false;
        }
        if state.gamepad_quickbar != controller.quickbar {
            if let Some(slot) = controller.quickbar {
                state.belt_pressed.push(self.hotbar.slot(slot));
            }
            state.gamepad_quickbar = controller.quickbar;
            // The same controller binding keeps its press-time bank. Only a
            // release or a newly selected binding resolves the current bank.
            state.belt.sync_gamepad(controller.quickbar.map(|slot| self.hotbar.slot(slot)));
        }
        let gamepad_movement = controller
            .gamepad_index
            .and_then(|index| pads.iter().find(|pad| pad.index == index))
            .map_or(Vec2::ZERO, |pad| {
                movement_from_gamepad(
                    pad.left,
                    MovementKeys {
                        left: pad.buttons[14],
                        right: pad.buttons[15],
                        up: pad.buttons[12],
                        down: pad.buttons[13],
                    },
                    true,
                    true,
                )
            });
        let keyboard = keyboard_movement(&state);
        let movement = sample_movement(state.touch, gamepad_movement, keyboard).movement;
        let pending_aim = state.pending_aim.take();
        let aim = if state.touch_primary != Vec2::ZERO {
            Some(GameplayAim::Direction(state.touch_primary))
        } else if state.mouse_primary || state.belt.desktop_held() {
            if state.mouse_primary || state.secondary_at_pointer {
                state.captured_pointer.map(GameplayAim::Pointer).or(Some(GameplayAim::Facing))
            } else {
                Some(GameplayAim::Facing)
            }
        } else if state.aim_owner == Some(AimOwner::Gamepad) {
            controller.aim_direction.map(GameplayAim::Direction)
        } else {
            pending_aim.or(if state.aim_was_set { Some(GameplayAim::Retain) } else { None })
        };
        if aim.is_some() {
            state.aim_was_set = true;
        }
        let primary = state.primary_enabled
            && (state.mouse_primary || state.touch_primary != Vec2::ZERO || controller.primary);
        let mut actions = std::mem::take(&mut state.actions);
        actions.extend(controller.actions);
        self.menu_requested.set(actions.contains(&GameplayAction::Pause));
        Ok(GameplayInputSample {
            movement,
            aim,
            primary,
            // A GUI tap may begin/end between frames. Keep one sampled pulse alongside
            // its queued press so the session can invoke the real secondary kernel once.
            quickbar: state.belt.current().or_else(|| state.belt_pressed.last().copied()),
            actions,
            selected_belt_slot: controller.selected_quickbar_slot,
            controller_present: controller.gamepad_index.is_some(),
            belt_pressed: std::mem::take(&mut state.belt_pressed),
        })
    }
    pub fn movement(&self) -> Result<Vec2, String> {
        Ok(self.sample()?.movement)
    }
    pub fn take_menu_requested(&self) -> bool {
        self.menu_requested.replace(false)
    }
    pub fn take_world_click(&self) -> Option<Vec2> {
        self.state.borrow_mut().world_click.take()
    }
    pub fn set_spectator_camera(&self, active: bool, item_slots: [bool; PLAYER_BELT_SLOT_COUNT]) {
        let mut state = self.state.borrow_mut();
        state.spectator_camera_active = active;
        state.spectator_item_slots = item_slots;
        if !active {
            state.spectator_camera_advances = 0;
        }
    }
    pub fn take_spectator_camera_advances(&self) -> usize {
        std::mem::take(&mut self.state.borrow_mut().spectator_camera_advances)
    }
    pub fn take_belt_unassignments(&self) -> Vec<usize> {
        std::mem::take(&mut self.state.borrow_mut().belt_unassignments)
    }
    pub fn set_mobile_layout(&self, layout: &MobileUiLayoutState) -> Result<(), String> {
        let pointers = {
            let mut state = self.state.borrow_mut();
            if state.mobile_layout == *layout {
                return Ok(());
            }
            state.mobile_layout = layout.clone();
            let pointers = [state.pointer.take(), state.primary_pointer.take()]
                .into_iter()
                .flatten()
                .collect();
            state.touch = Vec2::ZERO;
            state.touch_primary = Vec2::ZERO;
            pointers
        };
        release_pointers(&self.canvas, pointers);
        Ok(())
    }
    pub fn resize(&self, width: i32, height: i32, ui_scale_percent: u16) -> Result<(), String> {
        self.state.borrow_mut().viewport = (width, height, ui_scale_percent);
        Ok(())
    }
    fn reconcile_hotbar_bank(&self) -> Result<(), String> {
        let pointers = {
            let mut state = self.state.borrow_mut();
            let bank = self.hotbar.bank();
            if state.hotbar_bank == bank {
                return Ok(());
            }
            state.hotbar_bank = bank;
            for slot in 0..NATIVE_BELT_SLOT_COUNT {
                state.belt.release_source(BeltInputSource::Touch(slot));
            }
            state.touch_belt_pointers.drain().map(|(id, _)| id).collect()
        };
        release_pointers(&self.canvas, pointers);
        Ok(())
    }
}
fn keyboard_movement(state: &GameplayInputState) -> Vec2 {
    let pressed = |code: &str, arrow: &str| {
        let assigned =
            GameControlAction::ALL.into_iter().any(|action| state.controls.code(action) == arrow);
        state.held.contains(code) || (!assigned && state.held.contains(arrow))
    };
    MovementKeys {
        left: pressed(&state.controls.move_left, "ArrowLeft"),
        right: pressed(&state.controls.move_right, "ArrowRight"),
        up: pressed(&state.controls.move_up, "ArrowUp"),
        down: pressed(&state.controls.move_down, "ArrowDown"),
    }
    .movement()
}

fn read_gamepads(window: &Window) -> Result<Vec<StandardGamepadSnapshot>, String> {
    let navigator = window.navigator();
    let function = js_sys::Reflect::get(navigator.as_ref(), &JsValue::from_str("getGamepads"))
        .map_err(|_| "cannot inspect gamepad support")?;
    if !function.is_function() {
        return Ok(Vec::new());
    }
    let values = navigator.get_gamepads().map_err(|_| "browser denied gamepad access")?;
    let mut pads = Vec::new();
    for value in values {
        if value.is_null() || value.is_undefined() {
            continue;
        }
        let pad: Gamepad = value.dyn_into().map_err(|_| "browser returned an invalid gamepad")?;
        let axes = pad.axes();
        let buttons = pad.buttons();
        let mut pressed = [false; 16];
        for (index, pressed) in pressed.iter_mut().enumerate() {
            *pressed = buttons
                .get(index as u32)
                .dyn_into::<GamepadButton>()
                .is_ok_and(|button| button.pressed() || button.value() > 0.5);
        }
        pads.push(StandardGamepadSnapshot {
            index: pad.index(),
            connected: pad.connected(),
            standard: pad.mapping() == GamepadMappingType::Standard,
            left: Vec2::new(
                axes.get(0).as_f64().unwrap_or(0.0),
                axes.get(1).as_f64().unwrap_or(0.0),
            ),
            right: Vec2::new(
                axes.get(2).as_f64().unwrap_or(0.0),
                axes.get(3).as_f64().unwrap_or(0.0),
            ),
            buttons: pressed,
        });
    }
    Ok(pads)
}

fn canvas_point(canvas: &HtmlCanvasElement, x: i32, y: i32) -> Option<Vec2> {
    let bounds = canvas.get_bounding_client_rect();
    if bounds.width() <= 0.0 || bounds.height() <= 0.0 {
        return None;
    }
    Some(Vec2::new(
        (f64::from(x) - bounds.left()) / bounds.width(),
        (f64::from(y) - bounds.top()) / bounds.height(),
    ))
}

pub(crate) fn pointer_client_point(pointer: &PointerEvent) -> Result<Vec2, String> {
    // PointerEvent refines MouseEvent's integer coordinates to doubles. Chrome
    // preserves 50.5 here, while web-sys's inherited client_x() truncates it.
    let x = js_sys::Reflect::get(pointer.as_ref(), &JsValue::from_str("clientX"))
        .map_err(|_| "cannot read pointer clientX")?
        .as_f64()
        .ok_or("pointer clientX is not numeric")?;
    let y = js_sys::Reflect::get(pointer.as_ref(), &JsValue::from_str("clientY"))
        .map_err(|_| "cannot read pointer clientY")?
        .as_f64()
        .ok_or("pointer clientY is not numeric")?;
    Ok(Vec2::new(x, y))
}
fn hold_belt(state: &mut GameplayInputState, source: BeltInputSource, slot: usize) {
    if state.belt.held_slot(source).is_none() && state.belt.hold(source, slot) {
        state.belt_pressed.push(slot);
    }
}
fn clear_input(state: &mut GameplayInputState) {
    state.held.clear();
    state.touch = Vec2::ZERO;
    state.pointer = None;
    state.touch_primary = Vec2::ZERO;
    state.primary_pointer = None;
    state.mouse_primary = false;
    state.pending_aim = None;
    state.aim_owner = None;
    state.aim_was_set = false;
    state.gamepad.clear(true);
    state.gamepad_quickbar = None;
    state.belt.clear();
    state.belt_pressed.clear();
    state.touch_belt_pointers.clear();
    state.belt_unassignments.clear();
    state.world_click = None;
    state.spectator_camera_advances = 0;
    state.actions.clear();
    // A screen position is not held input: retain it for a fresh keyboard belt press.
}
fn report_input_error(result: Result<(), String>) {
    if let Err(message) = result {
        web_sys::console::error_1(&JsValue::from_str(&message));
    }
}
fn belt_slot(controls: &GameControlBindings, code: &str) -> Option<usize> {
    controls.belt.iter().position(|binding| binding == code)
}

fn is_movement_key(code: &str, controls: &GameControlBindings) -> bool {
    ARROWS.contains(&code)
        || [&controls.move_left, &controls.move_right, &controls.move_up, &controls.move_down]
            .into_iter()
            .any(|key| key == code)
}
fn editing_text(event: &KeyboardEvent) -> bool {
    let Some(target) = event.target().and_then(|target| target.dyn_into::<HtmlElement>().ok())
    else {
        return false;
    };
    target.is_content_editable()
        || ["INPUT", "TEXTAREA", "SELECT"].contains(&target.tag_name().as_str())
}
struct InputListener {
    target: EventTarget,
    name: &'static str,
    callback: Closure<dyn FnMut(Event)>,
}
impl Drop for InputListener {
    fn drop(&mut self) {
        if let Err(error) = self.target.remove_event_listener_with_callback_and_bool(
            self.name,
            self.callback.as_ref().unchecked_ref(),
            true,
        ) {
            web_sys::console::error_1(&error);
        }
    }
}
fn listen(
    target: &EventTarget,
    name: &'static str,
    listeners: &mut Vec<InputListener>,
    callback: impl FnMut(Event) + 'static,
) -> Result<(), String> {
    let callback = Closure::<dyn FnMut(Event)>::new(callback);
    target
        .add_event_listener_with_callback_and_bool(name, callback.as_ref().unchecked_ref(), true)
        .map_err(|_| format!("cannot listen for {name}"))?;
    listeners.push(InputListener { target: target.clone(), name, callback });
    Ok(())
}
fn release_pointers(canvas: &HtmlCanvasElement, pointers: Vec<i32>) {
    for id in pointers {
        if canvas.has_pointer_capture(id) {
            report_input_error(
                canvas
                    .release_pointer_capture(id)
                    .map_err(|_| "cannot release gameplay pointer".to_owned()),
            );
        }
    }
}
impl Drop for MovementInput {
    fn drop(&mut self) {
        let pointers = self.state.borrow().captured_ui_pointers();
        release_pointers(&self.canvas, pointers);
    }
}
