//! Browser gameplay input, sampled once per frame. Website `input/gameplay-input.ts`,
//! `standard-gamepad.ts` and `TouchJoystick.tsx` own levels, aim and cancellation.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use crate::hotbar::HotbarSelection;
use crate::platform::{KeyboardEffect, KeyboardEvent, KeyboardListener};
use solomon_sim::Vec2;
use solomon_sim::belt::{NATIVE_BELT_SLOT_COUNT, PLAYER_BELT_SLOT_COUNT};
use solomon_sim::input::{
    BeltInputSource, GameplayAction, GameplayAim, GameplayInputSample, HeldBeltInputs,
    MovementKeys, StandardGameplayState, joystick_vector, movement_from_gamepad, sample_movement,
};
use solomon_sim::mobile_ui::MobileUiLayoutState;
use solomon_sim::settings::{GameControlAction, GameControlBindings};

#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg(target_arch = "wasm32")]
use browser as platform;
#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(not(target_arch = "wasm32"))]
use native as platform;

mod controls;
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
    transport: platform::InputTransport,
    hotbar: HotbarSelection,
    state: Rc<RefCell<GameplayInputState>>,
    menu_requested: Cell<bool>,
    _keyboard: KeyboardListener,
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
    pub fn attach(
        surface: &crate::platform::Surface,
        hotbar: HotbarSelection,
    ) -> Result<Self, String> {
        let state = Rc::new(RefCell::new(GameplayInputState::new(hotbar.bank())));
        let keyed = state.clone();
        let bank = hotbar.clone();
        let keyboard = KeyboardListener::attach(surface, move |event| {
            keyed.borrow_mut().keyboard(event, bank.start())
        })?;
        let transport = platform::InputTransport::attach(surface, state.clone(), hotbar.clone())?;
        Ok(Self { hotbar, state, menu_requested: Cell::new(false), transport, _keyboard: keyboard })
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn window_event(&self, event: &winit::event::WindowEvent) -> Result<(), String> {
        self.transport.window_event(event)
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
        self.transport.release_pointers(pointers);
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
        self.transport.release_pointers(pointers);
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
        self.transport.release_pointers(pointers);
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
        self.transport.release_pointers(pointer.into_iter().collect());
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
        let pads = crate::platform::gamepads()?;
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
        self.transport.release_pointers(pointers);
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
        self.transport.release_pointers(pointers);
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
fn belt_slot(controls: &GameControlBindings, code: &str) -> Option<usize> {
    controls.belt.iter().position(|binding| binding == code)
}

fn is_movement_key(code: &str, controls: &GameControlBindings) -> bool {
    ARROWS.contains(&code)
        || [&controls.move_left, &controls.move_right, &controls.move_up, &controls.move_down]
            .into_iter()
            .any(|key| key == code)
}
impl Drop for MovementInput {
    fn drop(&mut self) {
        let pointers = self.state.borrow().captured_ui_pointers();
        self.transport.release_pointers(pointers);
    }
}
