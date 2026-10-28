//! Stateful standard-controller actions shared by native and browser gameplay.
//! One selected pad owns movement, aim and casts until it disconnects. Modal handoffs
//! require neutral controls, so a held confirmation/trigger cannot become a new action.

use super::{GAMEPAD_MOVEMENT_DEAD_ZONE, GAMEPAD_MOVEMENT_OUTER_DEAD_ZONE, radial_dead_zone};
use crate::Vec2;
use crate::float::hypot;

const RELEVANT_BUTTONS: [usize; 12] = [0, 2, 3, 4, 5, 7, 8, 9, 12, 13, 14, 15];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StandardGamepadSnapshot {
    pub index: u32,
    pub connected: bool,
    pub standard: bool,
    pub left: Vec2,
    pub right: Vec2,
    pub buttons: [bool; 16],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameplayAction {
    Interact,
    Inventory,
    Skills,
    Chat,
    Pause,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StandardGameplaySample {
    pub actions: Vec<GameplayAction>,
    pub aim_active: bool,
    pub aim_direction: Option<Vec2>,
    pub gamepad_index: Option<u32>,
    pub primary: bool,
    pub quickbar: Option<usize>,
    pub quickbar_selection_changed: bool,
    pub selected_quickbar_slot: usize,
}

impl StandardGameplaySample {
    fn idle(selected_quickbar_slot: usize) -> Self {
        Self {
            actions: Vec::new(),
            aim_active: false,
            aim_direction: None,
            gamepad_index: None,
            primary: false,
            quickbar: None,
            quickbar_selection_changed: false,
            selected_quickbar_slot,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct StandardGameplayState {
    aim_direction: Option<Vec2>,
    awaiting_neutral: bool,
    held_quickbar_slot: Option<usize>,
    previous_buttons: [bool; 16],
    selected_gamepad_index: Option<u32>,
    selected_quickbar_slot: usize,
}

impl StandardGameplayState {
    #[must_use]
    pub fn selected_belt_slot(&self) -> usize {
        self.selected_quickbar_slot
    }
    pub fn clear(&mut self, require_neutral: bool) {
        self.aim_direction = None;
        self.awaiting_neutral = require_neutral;
        self.held_quickbar_slot = None;
        self.previous_buttons = [false; 16];
    }

    pub fn sample(&mut self, pads: &[StandardGamepadSnapshot]) -> StandardGameplaySample {
        let standard: Vec<_> = pads.iter().filter(|pad| pad.connected && pad.standard).collect();
        let mut selected_this_sample = false;
        let mut pad = self
            .selected_gamepad_index
            .and_then(|index| standard.iter().copied().find(|pad| pad.index == index));
        if self.selected_gamepad_index.is_some() && pad.is_none() {
            self.selected_gamepad_index = None;
            self.clear(!standard.is_empty());
        }
        if self.awaiting_neutral {
            let active = if let Some(pad) = pad {
                has_activity(pad)
            } else {
                standard.iter().any(|pad| has_activity(pad))
            };
            if !active {
                self.awaiting_neutral = false;
                self.previous_buttons = [false; 16];
            }
            return StandardGameplaySample::idle(self.selected_quickbar_slot);
        }
        if pad.is_none() {
            pad = standard.iter().copied().find(|pad| has_activity(pad));
            let Some(selected) = pad else {
                return StandardGameplaySample::idle(self.selected_quickbar_slot);
            };
            self.selected_gamepad_index = Some(selected.index);
            selected_this_sample = true;
        }
        let pad = pad.expect("an active or previously selected pad exists");
        let right = stick(pad.right);
        let magnitude = hypot(right.x, right.y);
        let aim_active = magnitude > 0.0;
        if aim_active {
            self.aim_direction = Some(Vec2::new(right.x / magnitude, right.y / magnitude));
        }
        let pressed = pad.buttons;
        let rising = |button: usize| pressed[button] && !self.previous_buttons[button];
        let quickbar_pressed = pressed[2];
        let mut quickbar_selection_changed = selected_this_sample;
        if !quickbar_pressed && self.held_quickbar_slot.is_none() {
            let previous = rising(4);
            let next = rising(5);
            if previous != next {
                self.selected_quickbar_slot =
                    (self.selected_quickbar_slot + if next { 1 } else { 7 }) % 8;
                quickbar_selection_changed = true;
            }
        }
        if quickbar_pressed && self.held_quickbar_slot.is_none() {
            self.held_quickbar_slot = Some(self.selected_quickbar_slot);
        }
        if !quickbar_pressed {
            self.held_quickbar_slot = None;
        }
        let mut actions = Vec::new();
        for (button, action) in [
            (0, GameplayAction::Interact),
            (3, GameplayAction::Skills),
            (8, GameplayAction::Inventory),
            (9, GameplayAction::Pause),
        ] {
            if rising(button) {
                actions.push(action);
            }
        }
        self.previous_buttons = pressed;
        StandardGameplaySample {
            actions,
            aim_active,
            aim_direction: self.aim_direction,
            gamepad_index: Some(pad.index),
            primary: pressed[7],
            quickbar: self.held_quickbar_slot,
            quickbar_selection_changed,
            selected_quickbar_slot: self.selected_quickbar_slot,
        }
    }
}

fn stick(value: Vec2) -> Vec2 {
    radial_dead_zone(value.x, value.y, GAMEPAD_MOVEMENT_DEAD_ZONE, GAMEPAD_MOVEMENT_OUTER_DEAD_ZONE)
}
fn has_activity(pad: &StandardGamepadSnapshot) -> bool {
    let left = stick(pad.left);
    let right = stick(pad.right);
    hypot(left.x, left.y) > 0.0
        || hypot(right.x, right.y) > 0.0
        || RELEVANT_BUTTONS.iter().any(|index| pad.buttons[*index])
}

/// Raw browser aim remains independent of the camera. The session maps a normalized
/// canvas point through its actual viewport, and directions through the player anchor.
/// `Retain` keeps the previously mapped world point; `None` in a sample clears it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GameplayAim {
    Pointer(Vec2),
    Direction(Vec2),
    Facing,
    Retain,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GameplayInputSample {
    pub movement: Vec2,
    pub aim: Option<GameplayAim>,
    pub primary: bool,
    pub quickbar: Option<usize>,
    pub actions: Vec<GameplayAction>,
    pub selected_belt_slot: usize,
    pub controller_present: bool,
    /// Rising press intents. Items activate once; a GUI skill tap also supplies a one-frame
    /// quickbar pulse because its press/release can both occur between animation frames.
    pub belt_pressed: Vec<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BeltInputSource {
    Gamepad,
    Keyboard(usize),
    Mouse(usize),
    Touch(usize),
}

#[derive(Default)]
pub struct HeldBeltInputs {
    entries: Vec<(BeltInputSource, usize)>,
}

impl HeldBeltInputs {
    pub fn hold(&mut self, source: BeltInputSource, slot: usize) -> bool {
        if self.entries.contains(&(source, slot)) {
            return false;
        }
        self.entries.retain(|entry| entry.0 != source);
        self.entries.push((source, slot));
        true
    }
    pub fn release(&mut self, source: BeltInputSource, slot: usize) {
        self.entries.retain(|entry| *entry != (source, slot));
    }
    /// A binding keeps the bank resolved at press time, even after the visible
    /// hotbar changes. Releases address the physical source, not the new bank.
    pub fn release_source(&mut self, source: BeltInputSource) {
        self.entries.retain(|entry| entry.0 != source);
    }
    #[must_use]
    pub fn held_slot(&self, source: BeltInputSource) -> Option<usize> {
        self.entries.iter().find(|entry| entry.0 == source).map(|entry| entry.1)
    }
    pub fn clear(&mut self) {
        self.entries.clear();
    }
    #[must_use]
    pub fn current(&self) -> Option<usize> {
        self.entries.last().map(|entry| entry.1)
    }

    #[must_use]
    pub fn is_held(&self, source: BeltInputSource, slot: usize) -> bool {
        self.entries.contains(&(source, slot))
    }
    #[must_use]
    pub fn desktop_held(&self) -> bool {
        self.entries.iter().any(|entry| {
            matches!(entry.0, BeltInputSource::Keyboard(_) | BeltInputSource::Mouse(_))
        })
    }
    pub fn sync_gamepad(&mut self, slot: Option<usize>) {
        let previous = self
            .entries
            .iter()
            .find(|entry| entry.0 == BeltInputSource::Gamepad)
            .map(|entry| entry.1);
        if previous == slot {
            return;
        }
        if let Some(previous) = previous {
            self.release(BeltInputSource::Gamepad, previous);
        }
        if let Some(slot) = slot {
            self.hold(BeltInputSource::Gamepad, slot);
        }
    }
}

/// Website `input/gameplay-pointer.ts`. The caller supplies the actual drawing viewport,
/// including any renderer letterbox, rather than assuming that it fills the canvas.
#[must_use]
pub fn project_native_world_pointer(
    pointer: Vec2,
    surface: crate::hud::HudRect,
    viewport: Vec2,
    origin: Vec2,
    scale: f64,
) -> Option<Vec2> {
    if !finite(pointer)
        || !finite(origin)
        || !surface.x.is_finite()
        || !surface.y.is_finite()
        || surface.width <= 0.0
        || surface.height <= 0.0
        || !(viewport.x > 0.0 && viewport.y > 0.0 && scale > 0.0)
        || !scale.is_finite()
    {
        return None;
    }
    let world = Vec2::new(
        origin.x + (pointer.x - surface.x) * viewport.x / surface.width / scale,
        origin.y + (pointer.y - surface.y) * viewport.y / surface.height / scale,
    );
    finite(world).then_some(world)
}

#[must_use]
pub fn project_native_stick_aim(
    direction: Vec2,
    player: Vec2,
    viewport: Vec2,
    scale: f64,
) -> Option<Vec2> {
    if !finite(direction)
        || !finite(player)
        || !(viewport.x > 0.0 && viewport.y > 0.0 && scale > 0.0)
        || !scale.is_finite()
    {
        return None;
    }
    let magnitude = hypot(direction.x, direction.y);
    let reach_pixels = viewport.x.min(viewport.y) / 2.0 - 25.0;
    if magnitude <= 0.001 || reach_pixels <= 0.0 {
        return None;
    }
    let reach = reach_pixels / scale;
    Some(Vec2::new(
        player.x + direction.x / magnitude * reach,
        player.y - 25.0 / scale + direction.y / magnitude * reach,
    ))
}

fn finite(point: Vec2) -> bool {
    point.x.is_finite() && point.y.is_finite()
}
