//! Actual GPU hit ownership and the source pointer lifecycle. UI capture is per
//! pointer, so movement and primary touch lanes can run together independently.
use super::*;
use crate::gpu_ui::HitRegion;

#[derive(Clone, Copy)]
pub(super) enum UiHitKind {
    Belt(HudBeltHit),
    Joystick(JoystickLane, JoystickGeometry),
    Text,
    Control,
}
#[derive(Clone, Copy)]
struct UiHit {
    region: HitRegion,
    kind: UiHitKind,
    z: usize,
}
#[derive(Default)]
pub(super) struct InputUiState {
    hits: Vec<UiHit>,
    pub scope: Option<String>,
    pub text: bool,
    pub keyboard: bool,
}
impl InputUiState {
    pub fn from_diagnostics(
        ui: &UiDiagnostics,
        belt: &[HudBeltHit],
        joysticks: &[Option<JoystickGeometry>; 2],
    ) -> Self {
        let mut hits = Vec::new();
        for widget in ui.widgets.iter().filter(|widget| widget.is_interactive()) {
            let kind = if widget.enabled {
                if let Some(hit) =
                    belt.iter().find(|hit| widget.id == format!("hud:belt:{}", hit.slot))
                {
                    UiHitKind::Belt(*hit)
                } else if widget.id == "hud:joystick:movement" {
                    joysticks[0].map_or(UiHitKind::Control, |geometry| {
                        UiHitKind::Joystick(JoystickLane::Movement, geometry)
                    })
                } else if widget.id == "hud:joystick:primary" {
                    joysticks[1].map_or(UiHitKind::Control, |geometry| {
                        UiHitKind::Joystick(JoystickLane::Primary, geometry)
                    })
                } else if matches!(widget.role.as_str(), "textbox" | "text-entry" | "input") {
                    UiHitKind::Text
                } else {
                    UiHitKind::Control
                }
            } else {
                UiHitKind::Control
            };
            hits.push(UiHit { region: widget.hit_region(), kind, z: widget.z });
        }
        hits.sort_by_key(|hit| hit.z);
        Self {
            hits,
            scope: ui.input_scope.clone(),
            text: ui.capture.text,
            keyboard: ui.capture.keyboard,
        }
    }
    pub fn hit(&self, point: egui::Pos2) -> Option<UiHitKind> {
        self.hits
            .iter()
            .rev()
            .find(|hit| {
                hit.region.contains(point)
                    && match hit.kind {
                        UiHitKind::Belt(belt) => belt.contains(point),
                        _ => true,
                    }
            })
            .map(|hit| hit.kind)
    }
}
#[derive(Default)]
pub(super) struct PointerOutcome {
    pub owned: bool,
    pub capture: bool,
    pub release: bool,
}
impl GameplayInputState {
    pub fn captured_ui_pointers(&self) -> Vec<i32> {
        let mut pointers = self.touch_belt_pointers.keys().copied().collect::<Vec<_>>();
        pointers.extend(self.pointer);
        pointers.extend(self.primary_pointer);
        pointers.sort_unstable();
        pointers.dedup();
        pointers
    }
    pub fn activate_ui_belt(&mut self, slot: usize) {
        self.belt_pressed.push(slot);
        if !self.aim_was_set {
            self.pending_aim = Some(GameplayAim::Facing);
            self.aim_owner = Some(AimOwner::Touch);
        }
    }
    pub fn begin_ui_pointer(&mut self, id: i32, button: i16, point: Vec2) -> PointerOutcome {
        if button != 0 {
            return PointerOutcome::default();
        }
        let Some(hit) = self.ui.hit(egui::pos2(point.x as f32, point.y as f32)) else {
            return PointerOutcome::default();
        };
        self.mouse_ui_press = true;
        if self.world_input_blocked() {
            return PointerOutcome { owned: true, ..Default::default() };
        }
        match hit {
            UiHitKind::Text => {
                self.ui.text = true;
                PointerOutcome { owned: true, ..Default::default() }
            }
            UiHitKind::Control => PointerOutcome { owned: true, ..Default::default() },
            UiHitKind::Belt(hit) => {
                self.touch_belt_pointers.retain(|_, press| press.slot != hit.slot);
                self.touch_belt_pointers.insert(
                    id,
                    BeltPointer {
                        slot: hit.slot,
                        origin: point,
                        scale: hit.input_scale,
                        cast_eligible: hit.cast_eligible,
                    },
                );
                PointerOutcome { owned: true, capture: true, release: false }
            }
            UiHitKind::Joystick(lane, geometry) => {
                let held = if lane == JoystickLane::Movement {
                    self.pointer
                } else {
                    self.primary_pointer
                };
                if held.is_some() || (lane == JoystickLane::Primary && !self.primary_enabled) {
                    return PointerOutcome { owned: true, ..Default::default() };
                }
                if lane == JoystickLane::Movement {
                    self.pointer = Some(id);
                } else {
                    self.primary_pointer = Some(id);
                }
                self.apply_joystick(lane, geometry, point);
                PointerOutcome { owned: true, capture: true, release: false }
            }
        }
    }
    pub fn move_ui_pointer(&mut self, id: i32, point: Vec2, bank: usize) -> PointerOutcome {
        if let Some(press) = self.touch_belt_pointers.get(&id).copied() {
            let movement = Vec2::new(
                (point.x - press.origin.x) / press.scale,
                (point.y - press.origin.y) / press.scale,
            );
            let changed_bank = press.slot / NATIVE_BELT_SLOT_COUNT != bank;
            let pulled =
                solomon_sim::hud::belt_pull_off::native_belt_pull_off_started(Vec2::ZERO, movement);
            if changed_bank || pulled {
                self.touch_belt_pointers.remove(&id);
                if pulled && !changed_bank && !self.world_input_blocked() {
                    self.belt_unassignments.push(press.slot);
                }
                return PointerOutcome { owned: true, release: true, capture: false };
            }
            return PointerOutcome { owned: true, ..Default::default() };
        }
        for (index, lane, owner) in [
            (0, JoystickLane::Movement, self.pointer),
            (1, JoystickLane::Primary, self.primary_pointer),
        ] {
            if owner == Some(id) {
                if let Some(geometry) = self.joystick_geometry[index] {
                    self.apply_joystick(lane, geometry, point);
                }
                return PointerOutcome { owned: true, ..Default::default() };
            }
        }
        PointerOutcome::default()
    }
    pub fn end_ui_pointer(&mut self, id: i32, released: bool, bank: usize) -> PointerOutcome {
        if let Some(press) = self.touch_belt_pointers.remove(&id) {
            if released
                && !self.world_input_blocked()
                && press.cast_eligible
                && press.slot / NATIVE_BELT_SLOT_COUNT == bank
            {
                self.activate_ui_belt(press.slot);
            }
            return PointerOutcome { owned: true, release: true, capture: false };
        }
        if self.pointer == Some(id) {
            self.pointer = None;
            self.touch = Vec2::ZERO;
            return PointerOutcome { owned: true, release: true, capture: false };
        }
        if self.primary_pointer == Some(id) {
            self.primary_pointer = None;
            self.touch_primary = Vec2::ZERO;
            return PointerOutcome { owned: true, release: true, capture: false };
        }
        PointerOutcome::default()
    }
    fn apply_joystick(&mut self, lane: JoystickLane, geometry: JoystickGeometry, point: Vec2) {
        let vector = joystick_vector(
            point,
            Vec2::new(geometry.center.x as f64, geometry.center.y as f64),
            geometry.input_radius(),
        );
        if lane == JoystickLane::Movement {
            self.touch = vector;
        } else {
            self.touch_primary = vector;
            if vector != Vec2::ZERO {
                self.aim_owner = Some(AimOwner::Touch);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gpu_ui::{HitShape, WidgetBounds};
    fn hit(kind: UiHitKind, rect: egui::Rect) -> UiHit {
        UiHit {
            kind,
            region: HitRegion {
                bounds: WidgetBounds::from(rect),
                paint_bounds: WidgetBounds::from(rect),
                shape: HitShape::Rect,
            },
            z: 0,
        }
    }
    fn belt_state(cast_eligible: bool) -> GameplayInputState {
        let mut state = GameplayInputState::new(0);
        let r = egui::Rect::from_min_size(egui::pos2(100.0, 100.0), egui::vec2(53.0, 53.0));
        state.ui.hits.push(hit(
            UiHitKind::Belt(HudBeltHit {
                slot: 0,
                face: r,
                rotation: 0.0,
                input_scale: 1.0,
                cast_eligible,
            }),
            r,
        ));
        state
    }
    #[test]
    fn belt_threshold_is_strict_and_a_pulled_pointer_never_casts() {
        let mut state = belt_state(true);
        assert!(state.begin_ui_pointer(7, 0, Vec2::new(126.5, 126.5)).capture);
        assert!(!state.move_ui_pointer(7, Vec2::new(126.5, 76.5), 0).release);
        assert!(state.belt_unassignments.is_empty());
        assert!(state.move_ui_pointer(7, Vec2::new(126.5, 76.25), 0).release);
        assert_eq!(state.belt_unassignments, vec![0]);
        state.end_ui_pointer(7, true, 0);
        assert!(state.belt_pressed.is_empty());
    }
    #[test]
    fn release_is_one_pulse_while_cancel_bank_change_and_college_disallow_casts() {
        let mut state = belt_state(true);
        state.begin_ui_pointer(1, 0, Vec2::new(120.0, 120.0));
        state.end_ui_pointer(1, false, 0);
        assert!(state.belt_pressed.is_empty());
        state.begin_ui_pointer(2, 0, Vec2::new(120.0, 120.0));
        state.end_ui_pointer(2, true, 1);
        assert!(state.belt_pressed.is_empty());
        state.begin_ui_pointer(3, 0, Vec2::new(120.0, 120.0));
        state.end_ui_pointer(3, true, 0);
        state.end_ui_pointer(3, true, 0);
        assert_eq!(state.belt_pressed, vec![0]);
        let mut college = belt_state(false);
        college.begin_ui_pointer(4, 0, Vec2::new(120.0, 120.0));
        college.end_ui_pointer(4, true, 0);
        assert!(college.belt_pressed.is_empty());
    }
    #[test]
    fn two_joysticks_retain_independent_pointers_and_cancellation() {
        let mut state = GameplayInputState::new(0);
        let left = JoystickGeometry {
            center: egui::pos2(100.0, 100.0),
            size: 100.0,
            rotation: 0.0,
            scale: 100.0 / 237.5,
        };
        let right = JoystickGeometry { center: egui::pos2(300.0, 100.0), ..left };
        state.joystick_geometry = [Some(left), Some(right)];
        for (lane, geometry) in [(JoystickLane::Movement, left), (JoystickLane::Primary, right)] {
            state.ui.hits.push(hit(UiHitKind::Joystick(lane, geometry), geometry.bounds()));
        }
        state.begin_ui_pointer(1, 0, Vec2::new(130.0, 100.0));
        state.begin_ui_pointer(2, 0, Vec2::new(300.0, 70.0));
        assert!(state.touch.x > 0.5 && state.touch_primary.y < -0.5);
        state.end_ui_pointer(1, false, 0);
        assert_eq!(state.touch, Vec2::ZERO);
        assert_eq!(state.primary_pointer, Some(2));
        assert!(state.touch_primary.y < -0.5);
        state.end_ui_pointer(2, false, 0);
        assert_eq!(state.touch_primary, Vec2::ZERO);
        assert!(!state.mouse_primary);
    }
    #[test]
    fn pointer_hover_capture_never_clears_a_separate_held_lane() {
        let mut state = GameplayInputState::new(0);
        state.pointer = Some(1);
        state.touch = Vec2::new(0.8, 0.0);
        state.ui.keyboard = true;
        assert!(!state.world_input_blocked());
        state.end_ui_pointer(999, false, 0);
        assert_eq!(state.pointer, Some(1));
        assert_eq!(state.touch, Vec2::new(0.8, 0.0));
    }
}
