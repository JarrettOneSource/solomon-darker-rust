//! Movement intent from Website `input/movement-input.ts`: radial dead zones,
//! normalization, joystick projection and touch → gamepad → keyboard priority.

mod gameplay;
pub use gameplay::{
    BeltInputSource, GameplayAction, GameplayAim, GameplayInputSample, HeldBeltInputs,
    StandardGamepadSnapshot, StandardGameplaySample, StandardGameplayState,
    project_native_stick_aim, project_native_world_pointer,
};

use crate::Vec2;
use crate::float::hypot;

pub const GAMEPAD_MOVEMENT_DEAD_ZONE: f64 = 0.2;
pub const GAMEPAD_MOVEMENT_OUTER_DEAD_ZONE: f64 = 0.95;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MovementInputDevice {
    None,
    Keyboard,
    Gamepad,
    Touch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuDirection {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MenuGamepadButtons {
    pub back: bool,
    pub confirm: bool,
    pub next: bool,
    pub previous: bool,
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MenuGamepadState {
    pub back: bool,
    pub confirm: bool,
    pub direction: Option<MenuDirection>,
    pub next: bool,
    pub previous: bool,
}

impl MenuGamepadState {
    #[must_use]
    pub fn active(self) -> bool {
        self.back || self.confirm || self.direction.is_some() || self.next || self.previous
    }
}

/// Website `input/gamepad-menu-navigation.ts`: vertical directions win at corners.
#[must_use]
pub fn read_menu_gamepad(axes: Vec2, buttons: MenuGamepadButtons) -> MenuGamepadState {
    let direction = if buttons.up || axes.y <= -0.62 {
        Some(MenuDirection::Up)
    } else if buttons.down || axes.y >= 0.62 {
        Some(MenuDirection::Down)
    } else if buttons.left || axes.x <= -0.62 {
        Some(MenuDirection::Left)
    } else if buttons.right || axes.x >= 0.62 {
        Some(MenuDirection::Right)
    } else {
        None
    };
    MenuGamepadState {
        back: buttons.back,
        confirm: buttons.confirm,
        direction,
        next: buttons.next,
        previous: buttons.previous,
    }
}

/// `(move, next_repeat_at)` with the source's 320 ms initial and 110 ms held delays.
#[must_use]
pub fn advance_menu_direction_repeat(
    previous: Option<MenuDirection>,
    current: Option<MenuDirection>,
    now: f64,
    next_repeat_at: f64,
) -> (bool, f64) {
    if current.is_none() {
        return (false, 0.0);
    }
    let changed = current != previous;
    if !changed && now < next_repeat_at {
        return (false, next_repeat_at);
    }
    (true, now + if changed { 320.0 } else { 110.0 })
}

/// `chooseSpatialTarget`: prefer a candidate ahead, weighted against perpendicular travel.
#[must_use]
pub fn choose_spatial_menu_target(
    current: usize,
    centers: &[Vec2],
    direction: MenuDirection,
) -> Option<usize> {
    let origin = centers[current];
    let mut selected = None;
    let mut best_score = f64::INFINITY;
    for (index, center) in centers.iter().enumerate() {
        if index == current {
            continue;
        }
        let dx = center.x - origin.x;
        let dy = center.y - origin.y;
        let primary = match direction {
            MenuDirection::Left => -dx,
            MenuDirection::Right => dx,
            MenuDirection::Up => -dy,
            MenuDirection::Down => dy,
        };
        if primary <= 0.0 {
            continue;
        }
        let perpendicular = match direction {
            MenuDirection::Left | MenuDirection::Right => dy.abs(),
            MenuDirection::Up | MenuDirection::Down => dx.abs(),
        };
        let score = primary + perpendicular * 2.5;
        if score < best_score {
            selected = Some(index);
            best_score = score;
        }
    }
    selected
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MovementInputSample {
    pub device: MovementInputDevice,
    pub movement: Vec2,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MovementKeys {
    pub left: bool,
    pub right: bool,
    pub up: bool,
    pub down: bool,
}

impl MovementKeys {
    #[must_use]
    pub fn movement(self) -> Vec2 {
        normalize_movement(Vec2::new(
            f64::from(self.right) - f64::from(self.left),
            f64::from(self.down) - f64::from(self.up),
        ))
    }
}

/// `normalizeMovement`: only vectors longer than one are shortened.
#[must_use]
pub fn normalize_movement(movement: Vec2) -> Vec2 {
    if !movement.x.is_finite() || !movement.y.is_finite() {
        return Vec2::ZERO;
    }
    let magnitude = hypot(movement.x, movement.y);
    if magnitude <= 1.0 {
        return movement;
    }
    Vec2::new(movement.x / magnitude, movement.y / magnitude)
}

/// `radialDeadZone`: remove inner stick drift and reach full speed at the outer radius.
#[must_use]
pub fn radial_dead_zone(x: f64, y: f64, dead_zone: f64, outer_dead_zone: f64) -> Vec2 {
    if !x.is_finite() || !y.is_finite() {
        return Vec2::ZERO;
    }
    let dead_zone = dead_zone.clamp(0.0, 0.95);
    let outer_dead_zone = outer_dead_zone.clamp(dead_zone, 1.0);
    let magnitude = hypot(x, y);
    if magnitude <= dead_zone {
        return Vec2::ZERO;
    }
    let range = outer_dead_zone - dead_zone;
    let normalized = if range == 0.0 { 1.0 } else { ((magnitude - dead_zone) / range).min(1.0) };
    Vec2::new(x / magnitude * normalized, y / magnitude * normalized)
}

/// One `movementFromGamepads` candidate. The browser selects the first active standard pad.
#[must_use]
pub fn movement_from_gamepad(
    axes: Vec2,
    dpad: MovementKeys,
    connected: bool,
    standard: bool,
) -> Vec2 {
    if !connected || !standard {
        return Vec2::ZERO;
    }
    let dpad = dpad.movement();
    if hypot(dpad.x, dpad.y) > 0.0 {
        return dpad;
    }
    radial_dead_zone(axes.x, axes.y, GAMEPAD_MOVEMENT_DEAD_ZONE, GAMEPAD_MOVEMENT_OUTER_DEAD_ZONE)
}

/// `createMovementInputState.sample`: touch and gamepad must exceed the 0.001 threshold.
#[must_use]
pub fn sample_movement(touch: Vec2, gamepad: Vec2, keyboard: Vec2) -> MovementInputSample {
    for (device, movement) in
        [(MovementInputDevice::Touch, touch), (MovementInputDevice::Gamepad, gamepad)]
    {
        if hypot(movement.x, movement.y) > 0.001 {
            return MovementInputSample { device, movement };
        }
    }
    let device = if hypot(keyboard.x, keyboard.y) > 0.0 {
        MovementInputDevice::Keyboard
    } else {
        MovementInputDevice::None
    };
    MovementInputSample { device, movement: keyboard }
}

#[must_use]
pub fn joystick_vector(pointer: Vec2, center: Vec2, radius: f64) -> Vec2 {
    if !radius.is_finite() || radius <= 0.0 {
        return Vec2::ZERO;
    }
    normalize_movement(Vec2::new((pointer.x - center.x) / radius, (pointer.y - center.y) / radius))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stick_drift_is_removed_and_outer_radius_reaches_full_speed() {
        assert_eq!(radial_dead_zone(0.2, 0.0, 0.2, 0.95), Vec2::ZERO);
        assert_eq!(radial_dead_zone(0.95, 0.0, 0.2, 0.95), Vec2::new(1.0, 0.0));
        assert_eq!(radial_dead_zone(f64::NAN, 0.0, 0.2, 0.95), Vec2::ZERO);
        assert_eq!(
            joystick_vector(Vec2::new(60.0, 20.0), Vec2::new(10.0, 20.0), 100.0),
            Vec2::new(0.5, 0.0)
        );
    }
}
