//! Browser gameplay input, sampled once per frame. Website `input/gameplay-input.ts`,
//! `standard-gamepad.ts` and `TouchJoystick.tsx` own levels, aim and cancellation.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use crate::hotbar::HotbarSelection;
use crate::mobile_layout::apply_mobile_ui_element;
use crate::ui::{append, element};
use solomon_sim::Vec2;
use solomon_sim::belt::{NATIVE_BELT_SLOT_COUNT, PLAYER_BELT_SLOT_COUNT};
use solomon_sim::hud::belt_pull_off::native_belt_pull_off_started;
use solomon_sim::input::{
    BeltInputSource, GameplayAction, GameplayAim, GameplayInputSample, HeldBeltInputs,
    MovementKeys, StandardGamepadSnapshot, StandardGameplayState, joystick_vector,
    movement_from_gamepad, sample_movement,
};
use solomon_sim::mobile_ui::{
    MobileUiElementId, MobileUiLayoutState, MobileUiSize, mobile_ui_joystick_input_radius,
    mobile_ui_joystick_knob_offset,
};
use solomon_sim::settings::{GameControlAction, GameControlBindings};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{
    Element, Event, EventTarget, Gamepad, GamepadButton, GamepadMappingType, HtmlCanvasElement,
    HtmlElement, KeyboardEvent, MouseEvent, PointerEvent, Window,
};

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
    hotbar: HotbarSelection,
    state: Rc<RefCell<GameplayInputState>>,
    joystick: HtmlElement,
    knob: HtmlElement,
    primary_joystick: HtmlElement,
    primary_knob: HtmlElement,
    menu_requested: Cell<bool>,
    geometry: Cell<(i32, i32, u16)>,
    _listeners: Vec<Closure<dyn FnMut(Event)>>,
}

impl MovementInput {
    pub fn attach(window: &Window, hotbar: HotbarSelection) -> Result<Self, String> {
        let document = window.document().ok_or("no document for gameplay input")?;
        let canvas = document
            .get_element_by_id("game")
            .ok_or("no gameplay canvas")?
            .dyn_into::<HtmlCanvasElement>()
            .map_err(|_| "gameplay surface is not a canvas")?;
        let state = Rc::new(RefCell::new(GameplayInputState {
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
            hotbar_bank: hotbar.bank(),
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
        }));
        let mut listeners = Vec::new();
        let (joystick, knob) =
            attach_joystick(window, &document, &state, JoystickLane::Movement, &mut listeners)?;
        let (primary_joystick, primary_knob) =
            attach_joystick(window, &document, &state, JoystickLane::Primary, &mut listeners)?;
        for (element, id) in [
            (&joystick, MobileUiElementId::LeftJoystick),
            (&primary_joystick, MobileUiElementId::RightJoystick),
        ] {
            apply_mobile_ui_element(element, &state.borrow().mobile_layout, id)?;
        }

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
                if state.blocked {
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
                if name == "mousemove" {
                    // Hover keeps the next keyboard belt cast's screen point without taking aim ownership.
                    state.captured_pointer = Some(point);
                    if state.blocked {
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
                if state.blocked {
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
            let Some(mouse) = event.dyn_ref::<MouseEvent>() else {
                return;
            };
            let mut state = clicked.borrow_mut();
            if !state.blocked && !state.spectator_camera_active && mouse.button() == 0 {
                state.world_click =
                    canvas_point(&clicked_canvas, mouse.client_x(), mouse.client_y());
            }
        })?;

        let touch_state = state.clone();
        listen(window.as_ref(), "pointerdown", &mut listeners, move |event| {
            let Some(pointer) = event.dyn_ref::<PointerEvent>() else {
                return;
            };
            if pointer.button() != 0 {
                return;
            }
            let Some(button) = hud_belt_button(&event) else {
                return;
            };
            let Some(slot) = hud_belt_slot(&event) else { return };
            let Some(root) = button
                .closest(".game-hud")
                .ok()
                .flatten()
                .and_then(|root| root.dyn_into::<HtmlElement>().ok())
            else {
                return;
            };
            let point = match pointer_client_point(pointer) {
                Ok(point) => point,
                Err(error) => {
                    report_input_error(Err(error));
                    return;
                }
            };
            let mut state = touch_state.borrow_mut();
            if state.blocked {
                return;
            }
            let custom_scale = if state.mobile_layout.customized {
                button
                    .get_attribute("data-mobile-ui-element")
                    .and_then(|id| MobileUiElementId::from_id(&id))
                    .map_or(1.0, |id| state.mobile_layout.layout.get(id).scale)
            } else {
                1.0
            };
            let input_scale = root
                .get_attribute("data-hud-input-scale")
                .and_then(|scale| scale.parse::<f64>().ok())
                .unwrap_or(1.0)
                * custom_scale;
            let scale =
                if input_scale.is_finite() && input_scale > 0.0 { input_scale } else { 1.0 };
            state.touch_belt_pointers.retain(|_, press| press.slot != slot);
            state.touch_belt_pointers.insert(
                pointer.pointer_id(),
                BeltPointer {
                    slot,
                    origin: point,
                    scale,
                    cast_eligible: button.get_attribute("data-belt-cast-eligible").as_deref()
                        != Some("false"),
                },
            );
            drop(state);
            // Meter/cooldown updates may replace slot art. Capture on the stable
            // HUD root so the same gesture survives those presentation updates.
            report_input_error(
                root.set_pointer_capture(pointer.pointer_id())
                    .map_err(|_| "cannot capture belt pointer".to_owned()),
            );
            event.prevent_default();
        })?;
        let pulled = state.clone();
        let pull_hotbar = hotbar.clone();
        listen(window.as_ref(), "pointermove", &mut listeners, move |event| {
            let Some(pointer) = event.dyn_ref::<PointerEvent>() else { return };
            let point = match pointer_client_point(pointer) {
                Ok(point) => point,
                Err(error) => {
                    report_input_error(Err(error));
                    return;
                }
            };
            let mut state = pulled.borrow_mut();
            let Some(press) = state.touch_belt_pointers.get(&pointer.pointer_id()).copied() else {
                return;
            };
            if press.slot / NATIVE_BELT_SLOT_COUNT != pull_hotbar.bank() {
                state.touch_belt_pointers.remove(&pointer.pointer_id());
                return;
            }
            let movement = Vec2::new(
                (point.x - press.origin.x) / press.scale,
                (point.y - press.origin.y) / press.scale,
            );
            if !native_belt_pull_off_started(Vec2::ZERO, movement) {
                return;
            }
            state.touch_belt_pointers.remove(&pointer.pointer_id());
            state.belt_unassignments.push(press.slot);
            drop(state);
            // Retirement precedes capture release: lostpointercapture must never
            // re-enter a borrowed input state or turn a pulled entry into a tap.
            if let Some(root) =
                event.target().and_then(|target| target.dyn_into::<HtmlElement>().ok())
                && root.has_pointer_capture(pointer.pointer_id())
            {
                report_input_error(
                    root.release_pointer_capture(pointer.pointer_id())
                        .map_err(|_| "cannot release belt pointer".to_owned()),
                );
            }
        })?;
        let activated = state.clone();
        listen(window.as_ref(), "click", &mut listeners, move |event| {
            // Native keyboard activation has no pointer level. Preserve its single press.
            if !event.dyn_ref::<MouseEvent>().is_some_and(|event| event.detail() == 0) {
                return;
            }
            if hud_belt_button(&event).is_some_and(|button| {
                button.get_attribute("data-belt-cast-eligible").as_deref() == Some("false")
            }) {
                return;
            }
            if let Some(slot) = hud_belt_slot(&event) {
                let mut state = activated.borrow_mut();
                if !state.blocked {
                    state.belt_pressed.push(slot);
                    if !state.aim_was_set {
                        state.pending_aim = Some(GameplayAim::Facing);
                        state.aim_owner = Some(AimOwner::Touch);
                    }
                }
            }
        })?;
        for name in ["pointerup", "pointercancel", "lostpointercapture"] {
            let state = state.clone();
            let hotbar = hotbar.clone();
            listen(window.as_ref(), name, &mut listeners, move |event| {
                let Some(pointer) = event.dyn_ref::<PointerEvent>() else {
                    return;
                };
                let mut state = state.borrow_mut();
                if let Some(press) = state.touch_belt_pointers.remove(&pointer.pointer_id()) {
                    // SkillQuickbar activates a UI tap on release; cancellation never casts.
                    if name == "pointerup"
                        && !state.blocked
                        && press.cast_eligible
                        && press.slot / NATIVE_BELT_SLOT_COUNT == hotbar.bank()
                    {
                        state.belt_pressed.push(press.slot);
                        if !state.aim_was_set {
                            state.pending_aim = Some(GameplayAim::Facing);
                            state.aim_owner = Some(AimOwner::Touch);
                        }
                    }
                }
            })?;
        }
        for (target, name) in [
            (window.as_ref(), "blur"),
            (window.as_ref(), "pagehide"),
            (document.as_ref(), "visibilitychange"),
        ] {
            let state = state.clone();
            let visibility = document.clone();
            let bases = [
                (joystick.clone(), knob.clone()),
                (primary_joystick.clone(), primary_knob.clone()),
            ];
            listen(target, name, &mut listeners, move |_| {
                if name == "visibilitychange" && !visibility.hidden() {
                    return;
                }
                clear_input(&mut state.borrow_mut());
                for (base, knob) in &bases {
                    report_input_error(paint_joystick(base, knob, Vec2::ZERO, 0.0));
                }
            })?;
        }
        Ok(Self {
            window: window.clone(),
            hotbar,
            state,
            joystick,
            knob,
            primary_joystick,
            primary_knob,
            menu_requested: Cell::new(false),
            geometry: Cell::new((0, 0, 0)),
            _listeners: listeners,
        })
    }

    pub fn set_blocked(&self, blocked: bool) -> Result<(), String> {
        let mut state = self.state.borrow_mut();
        if state.blocked == blocked {
            return Ok(());
        }
        state.blocked = blocked;
        for base in [&self.joystick, &self.primary_joystick] {
            base.toggle_attribute_with_force("data-blocked", blocked)
                .map_err(|_| "joystick availability")?;
        }
        if blocked {
            clear_input(&mut state);
            self.menu_requested.set(false);
            self.reset_joysticks()?;
        }
        Ok(())
    }

    pub fn set_controls(&self, controls: GameControlBindings) -> Result<(), String> {
        let mut state = self.state.borrow_mut();
        state.controls = controls;
        clear_input(&mut state);
        self.reset_joysticks()
    }

    pub fn set_primary_casting_enabled(&self, enabled: bool) -> Result<(), String> {
        let mut state = self.state.borrow_mut();
        if state.primary_enabled == enabled {
            return Ok(());
        }
        state.primary_enabled = enabled;
        self.primary_joystick
            .toggle_attribute_with_force("data-cast-disabled", !enabled)
            .map_err(|_| "primary joystick availability")?;
        if !enabled {
            state.mouse_primary = false;
            state.touch_primary = Vec2::ZERO;
            state.primary_pointer = None;
            paint_joystick(&self.primary_joystick, &self.primary_knob, Vec2::ZERO, 0.0)?;
        }
        Ok(())
    }

    pub fn set_secondary_at_pointer(&self, enabled: bool) {
        self.state.borrow_mut().secondary_at_pointer = enabled;
    }

    /// Modal belt commands use the same retained pointer and preference as gameplay casts.
    pub fn secondary_aim(&self, pointer: Option<Vec2>) -> GameplayAim {
        let state = self.state.borrow();
        match pointer.or(state.captured_pointer) {
            Some(point) if state.secondary_at_pointer => GameplayAim::Pointer(point),
            _ => GameplayAim::Facing,
        }
    }

    pub fn set_touch_quickbar(&self, slot: usize, pressed: bool, fallback_direction: Option<Vec2>) {
        let mut state = self.state.borrow_mut();
        if state.blocked || slot >= NATIVE_BELT_SLOT_COUNT {
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

    pub fn set_mobile_layout(&self, layout: &MobileUiLayoutState) -> Result<(), String> {
        let mut state = self.state.borrow_mut();
        if state.mobile_layout == *layout {
            return Ok(());
        }
        state.mobile_layout = layout.clone();
        // The source replaces TouchJoystick's layout-dependent pointer callback and
        // releases its old pointer in the effect cleanup. Keyboard/controller hold stays.
        state.pointer = None;
        state.primary_pointer = None;
        state.touch = Vec2::ZERO;
        state.touch_primary = Vec2::ZERO;
        for (element, id) in [
            (&self.joystick, MobileUiElementId::LeftJoystick),
            (&self.primary_joystick, MobileUiElementId::RightJoystick),
        ] {
            apply_mobile_ui_element(element, layout, id)?;
        }
        self.reset_joysticks()
    }

    pub fn resize(&self, width: i32, height: i32, ui_scale_percent: u16) -> Result<(), String> {
        if self.geometry.get() == (width, height, ui_scale_percent) {
            return Ok(());
        }
        self.geometry.set((width, height, ui_scale_percent));
        let scale = (f64::from(width) / 1600.0).min(f64::from(height) / 900.0).min(1.0)
            * f64::from(ui_scale_percent)
            / 100.0;
        for base in [&self.joystick, &self.primary_joystick] {
            base.style()
                .set_property("--input-scale", &scale.to_string())
                .map_err(|_| "joystick scale")?;
        }
        Ok(())
    }

    /// `sample` owns the single native gamepad read for movement and casting this frame.
    pub fn sample(&self) -> Result<GameplayInputSample, String> {
        self.reconcile_hotbar_bank()?;
        let mut state = self.state.borrow_mut();
        if state.blocked {
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

    /// Compatibility for movement-only callers. Do not also call `sample` in that frame.
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

    /// Source quickbar pull-off emits an actual belt index once, and never casts.
    /// The session owns the unbind command; the HUD owns its cosmetic burst.
    pub fn take_belt_unassignments(&self) -> Vec<usize> {
        std::mem::take(&mut self.state.borrow_mut().belt_unassignments)
    }

    fn reconcile_hotbar_bank(&self) -> Result<(), String> {
        let mut state = self.state.borrow_mut();
        let bank = self.hotbar.bank();
        if state.hotbar_bank == bank {
            return Ok(());
        }
        state.hotbar_bank = bank;
        // Source keys each touch button by bank. Unmount cancels its old press;
        // keyboard, mouse and controller bindings retain their physical holds.
        for slot in 0..NATIVE_BELT_SLOT_COUNT {
            state.belt.release_source(BeltInputSource::Touch(slot));
        }
        let pointers: Vec<_> = state.touch_belt_pointers.drain().map(|(id, _)| id).collect();
        drop(state);
        let root = self
            .window
            .document()
            .ok_or("hotbar pointer document")?
            .query_selector(".game-hud")
            .map_err(|_| "hotbar pointer root")?
            .and_then(|root| root.dyn_into::<HtmlElement>().ok());
        if let Some(root) = root {
            for id in pointers {
                if root.has_pointer_capture(id) {
                    root.release_pointer_capture(id).map_err(|_| "hotbar pointer cancellation")?;
                }
            }
        }
        Ok(())
    }
    fn reset_joysticks(&self) -> Result<(), String> {
        paint_joystick(&self.joystick, &self.knob, Vec2::ZERO, 0.0)?;
        paint_joystick(&self.primary_joystick, &self.primary_knob, Vec2::ZERO, 0.0)
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

fn attach_joystick(
    window: &Window,
    document: &web_sys::Document,
    state: &Rc<RefCell<GameplayInputState>>,
    lane: JoystickLane,
    listeners: &mut Vec<Closure<dyn FnMut(Event)>>,
) -> Result<(HtmlElement, HtmlElement), String> {
    let base = element(
        document,
        "div",
        if lane == JoystickLane::Movement {
            "movement-joystick"
        } else {
            "movement-joystick primary-joystick"
        },
    )?;
    base.set_attribute("role", "region").map_err(|_| "joystick role")?;
    base.set_attribute(
        "aria-label",
        if lane == JoystickLane::Movement {
            "Movement joystick"
        } else {
            "Primary attack joystick"
        },
    )
    .map_err(|_| "joystick label")?;
    base.set_attribute(
        "data-joystick",
        if lane == JoystickLane::Movement { "movement" } else { "primary" },
    )
    .map_err(|_| "joystick identity")?;
    let knob = element(document, "span", "movement-joystick-knob")?;
    let label = element(document, "span", "movement-joystick-label")?;
    label.set_text_content(Some(if lane == JoystickLane::Movement {
        "MOVE"
    } else {
        "AIM / CAST"
    }));
    append(&base, &knob)?;
    append(&base, &label)?;
    append(&document.body().ok_or("page has no body")?, &base)?;
    let touch = state.clone();
    let captured_base = base.clone();
    let captured_knob = knob.clone();
    listen(base.as_ref(), "pointerdown", listeners, move |event| {
        let Some(pointer) = event.dyn_ref::<PointerEvent>() else {
            return;
        };
        let mut state = touch.borrow_mut();
        let held =
            if lane == JoystickLane::Movement { state.pointer } else { state.primary_pointer };
        if state.blocked
            || held.is_some()
            || pointer.button() != 0
            || (lane == JoystickLane::Primary && !state.primary_enabled)
        {
            return;
        }
        event.prevent_default();
        if lane == JoystickLane::Movement {
            state.pointer = Some(pointer.pointer_id());
        } else {
            state.primary_pointer = Some(pointer.pointer_id());
        }
        report_input_error(
            captured_base
                .set_pointer_capture(pointer.pointer_id())
                .map_err(|_| "cannot capture joystick pointer".into()),
        );
        report_input_error(update_touch(&mut state, &captured_base, &captured_knob, pointer, lane));
    })?;
    let touch = state.clone();
    let moving_base = base.clone();
    let moving_knob = knob.clone();
    listen(window.as_ref(), "pointermove", listeners, move |event| {
        let Some(pointer) = event.dyn_ref::<PointerEvent>() else {
            return;
        };
        let mut state = touch.borrow_mut();
        let held =
            if lane == JoystickLane::Movement { state.pointer } else { state.primary_pointer };
        if held == Some(pointer.pointer_id()) {
            report_input_error(update_touch(&mut state, &moving_base, &moving_knob, pointer, lane));
        }
    })?;
    for name in ["pointerup", "pointercancel", "lostpointercapture"] {
        let touch = state.clone();
        let ending_base = base.clone();
        let ending_knob = knob.clone();
        listen(window.as_ref(), name, listeners, move |event| {
            let Some(pointer) = event.dyn_ref::<PointerEvent>() else {
                return;
            };
            let mut state = touch.borrow_mut();
            let held =
                if lane == JoystickLane::Movement { state.pointer } else { state.primary_pointer };
            if held == Some(pointer.pointer_id()) {
                if lane == JoystickLane::Movement {
                    state.pointer = None;
                    state.touch = Vec2::ZERO;
                } else {
                    state.primary_pointer = None;
                    state.touch_primary = Vec2::ZERO;
                }
                report_input_error(paint_joystick(&ending_base, &ending_knob, Vec2::ZERO, 0.0));
            }
        })?;
    }
    Ok((base, knob))
}

fn update_touch(
    state: &mut GameplayInputState,
    base: &HtmlElement,
    knob: &HtmlElement,
    event: &PointerEvent,
    lane: JoystickLane,
) -> Result<(), String> {
    let bounds = base.get_bounding_client_rect();
    let mobile_id = if lane == JoystickLane::Movement {
        MobileUiElementId::LeftJoystick
    } else {
        MobileUiElementId::RightJoystick
    };
    let rotation = if state.mobile_layout.customized {
        state.mobile_layout.layout.get(mobile_id).rotation
    } else {
        0.0
    };
    let movement = joystick_vector(
        pointer_client_point(event)?,
        Vec2::new(bounds.left() + bounds.width() / 2.0, bounds.top() + bounds.height() / 2.0),
        mobile_ui_joystick_input_radius(
            MobileUiSize { width: bounds.width(), height: bounds.height() },
            rotation,
        ),
    );
    if lane == JoystickLane::Movement {
        state.touch = movement;
    } else {
        state.touch_primary = movement;
        if movement != Vec2::ZERO {
            state.aim_owner = Some(AimOwner::Touch);
        }
    }
    paint_joystick(base, knob, movement, rotation)
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
fn listen(
    target: &EventTarget,
    name: &'static str,
    listeners: &mut Vec<Closure<dyn FnMut(Event)>>,
    callback: impl FnMut(Event) + 'static,
) -> Result<(), String> {
    let closure = Closure::<dyn FnMut(Event)>::new(callback);
    target
        .add_event_listener_with_callback(name, closure.as_ref().unchecked_ref())
        .map_err(|_| format!("cannot listen for {name}"))?;
    listeners.push(closure);
    Ok(())
}
fn paint_joystick(
    base: &HtmlElement,
    knob: &HtmlElement,
    movement: Vec2,
    rotation: f64,
) -> Result<(), String> {
    let offset = mobile_ui_joystick_knob_offset(
        movement,
        MobileUiSize {
            width: f64::from(base.offset_width()),
            height: f64::from(base.offset_height()),
        },
        rotation,
    );
    knob.style()
        .set_property(
            "transform",
            &format!("translate(-50%, -50%) translate({}px, {}px)", offset.x, offset.y),
        )
        .map_err(|_| "cannot move joystick knob")?;
    base.toggle_attribute_with_force("data-active", movement != Vec2::ZERO)
        .map_err(|_| "cannot show joystick activity")?;
    Ok(())
}
fn report_input_error(result: Result<(), String>) {
    if let Err(message) = result {
        web_sys::console::error_1(&JsValue::from_str(&message));
    }
}
fn belt_slot(controls: &GameControlBindings, code: &str) -> Option<usize> {
    controls.belt.iter().position(|binding| binding == code)
}

fn hud_belt_slot(event: &Event) -> Option<usize> {
    hud_belt_button(event)?.get_attribute("data-belt-index")?.parse::<usize>().ok()
}

fn hud_belt_button(event: &Event) -> Option<HtmlElement> {
    let button =
        event.target()?.dyn_into::<Element>().ok()?.closest("[data-hud-action]").ok()??;
    if button.has_attribute("disabled")
        || !button.get_attribute("data-hud-action")?.starts_with("belt:")
    {
        return None;
    }
    button.dyn_into::<HtmlElement>().ok()
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
