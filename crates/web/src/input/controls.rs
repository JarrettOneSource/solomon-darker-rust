//! Keyboard and mouse semantics shared by browser and OS event transports.
use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum MousePhase {
    Down,
    Move,
    Up,
}

impl GameplayInputState {
    pub fn keyboard(&mut self, event: &KeyboardEvent<'_>, bank_start: usize) -> KeyboardEffect {
        let key = event.stroke;
        let pressed = key.pressed;
        if pressed && event.editing {
            return KeyboardEffect::Pass;
        }
        let state = self;
        let code = &key.code;
        let mut effect = KeyboardEffect::Pass;
        if pressed && state.ui.text {
            return effect;
        }
        if pressed && state.ui.keyboard && matches!(code.as_str(), "Enter" | "Space") {
            return effect;
        }
        if pressed
            && !key.repeat
            && !key.alt
            && !key.ctrl
            && !key.meta
            && code == &state.controls.open_chat
        {
            effect = KeyboardEffect::PreventDefault;
            state.actions.push(GameplayAction::Chat);
        }
        // The source chat opener remains available over Inventory. Its own
        // view admission rejects title/pause/Tutorial; movement stays sealed.
        if state.world_input_blocked() {
            return effect;
        }
        if let Some(slot) = belt_slot(&state.controls, code) {
            effect = KeyboardEffect::PreventDefault;
            if pressed && !key.repeat {
                hold_belt(state, BeltInputSource::Keyboard(slot), bank_start + slot);
            } else if !pressed {
                state.belt.release_source(BeltInputSource::Keyboard(slot));
            }
        }

        if pressed && !key.repeat && code == "KeyE" {
            state.actions.push(GameplayAction::Interact);
        }
        if is_movement_key(code, &state.controls) {
            effect = KeyboardEffect::PreventDefault;
            if pressed {
                state.held.insert(code.clone());
            } else {
                state.held.remove(code);
            }
        }
        effect
    }

    pub fn mouse(
        &mut self,
        phase: MousePhase,
        button: i16,
        logical: egui::Pos2,
        point: Vec2,
        bank_start: usize,
    ) -> bool {
        if phase == MousePhase::Down {
            self.mouse_ui_press = self.ui.hit(logical).is_some();
            if self.mouse_ui_press {
                return false;
            }
        }
        if phase == MousePhase::Move {
            // Hover keeps the next belt cast's point without taking aim ownership.
            self.captured_pointer = Some(point);
            if self.world_input_blocked() {
                return false;
            }
            if self.mouse_primary || self.belt.current().is_some() {
                self.aim_owner = Some(AimOwner::Mouse);
                self.pending_aim = Some(if self.mouse_primary || self.secondary_at_pointer {
                    GameplayAim::Pointer(point)
                } else {
                    GameplayAim::Facing
                });
                return true;
            }
            return false;
        }
        if self.world_input_blocked() {
            return false;
        }
        let primary = button == 0;
        let slot =
            if primary { None } else { belt_slot(&self.controls, &format!("Mouse{button}")) };
        if !primary && slot.is_none() || primary && !self.primary_enabled {
            return false;
        }
        if phase == MousePhase::Down
            && self.spectator_camera_active
            && (primary || slot.is_none_or(|slot| !self.spectator_item_slots[bank_start + slot]))
        {
            self.spectator_camera_advances += 1;
            return true;
        }
        let held = if primary {
            self.mouse_primary
        } else {
            slot.is_some_and(|slot| self.belt.held_slot(BeltInputSource::Mouse(slot)).is_some())
        };
        if phase == MousePhase::Up && !held {
            return false;
        }
        self.captured_pointer = Some(point);
        self.aim_owner = Some(AimOwner::Mouse);
        self.pending_aim = Some(if primary || self.secondary_at_pointer {
            GameplayAim::Pointer(point)
        } else {
            GameplayAim::Facing
        });
        if primary {
            self.mouse_primary = phase == MousePhase::Down;
        } else if let Some(slot) = slot {
            if phase == MousePhase::Down {
                hold_belt(self, BeltInputSource::Mouse(slot), bank_start + slot);
            } else {
                self.belt.release_source(BeltInputSource::Mouse(slot));
            }
        }
        true
    }

    pub fn click(&mut self, button: i16, logical: egui::Pos2, point: Option<Vec2>) {
        if std::mem::take(&mut self.mouse_ui_press) {
            return;
        }
        if !self.world_input_blocked()
            && !self.spectator_camera_active
            && button == 0
            && self.ui.hit(logical).is_none()
        {
            self.world_click = point;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::KeyStroke;

    fn key(code: &str, pressed: bool) -> KeyStroke {
        KeyStroke {
            code: code.into(),
            key: code.into(),
            pressed,
            repeat: false,
            alt: false,
            ctrl: false,
            shift: false,
            meta: false,
        }
    }

    #[test]
    fn held_keyboard_cast_keeps_its_press_time_bank_until_release() {
        let mut state = GameplayInputState::new(0);
        let code = state.controls.belt[0].clone();
        let mut stroke = key(&code, true);
        state.keyboard(
            &KeyboardEvent { stroke: &stroke, editing: false, default_prevented: false },
            NATIVE_BELT_SLOT_COUNT,
        );
        assert_eq!(state.belt.current(), Some(NATIVE_BELT_SLOT_COUNT));
        stroke.repeat = true;
        state.keyboard(
            &KeyboardEvent { stroke: &stroke, editing: false, default_prevented: false },
            0,
        );
        assert_eq!(state.belt.current(), Some(NATIVE_BELT_SLOT_COUNT));
        stroke.pressed = false;
        state.keyboard(
            &KeyboardEvent { stroke: &stroke, editing: false, default_prevented: false },
            0,
        );
        assert_eq!(state.belt.current(), None);
        assert_eq!(state.belt_pressed, vec![NATIVE_BELT_SLOT_COUNT]);
    }

    #[test]
    fn chat_can_open_over_a_blocked_world_but_editing_never_starts_gameplay() {
        let mut state = GameplayInputState::new(0);
        state.blocked = true;
        let stroke = key(&state.controls.open_chat, true);
        let mut event = KeyboardEvent { stroke: &stroke, editing: false, default_prevented: false };
        state.keyboard(&event, 0);
        assert_eq!(state.actions, vec![GameplayAction::Chat]);
        state.actions.clear();
        event.editing = true;
        state.keyboard(&event, 0);
        assert!(state.actions.is_empty());
        assert!(state.held.is_empty());
    }

    #[test]
    fn mouse_hold_aim_and_focus_cancellation_share_one_state() {
        let mut state = GameplayInputState::new(0);
        let point = Vec2::new(0.7, 0.2);
        let logical = egui::pos2(700.0, 200.0);
        assert!(state.mouse(MousePhase::Down, 0, logical, point, 0));
        assert!(state.mouse_primary);
        assert_eq!(state.pending_aim, Some(GameplayAim::Pointer(point)));
        clear_input(&mut state);
        assert!(!state.mouse_primary);
        assert_eq!(state.pending_aim, None);
        assert!(!state.mouse(MousePhase::Up, 0, logical, point, 0));
        assert_eq!(state.captured_pointer, Some(point));
    }
}
