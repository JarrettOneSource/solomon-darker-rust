//! winit pointer/focus events feed the common gameplay controls and UI capture.

use super::*;
use controls::MousePhase;
use winit::event::{ElementState, MouseButton, TouchPhase, WindowEvent};

#[derive(Default)]
struct Pointers {
    position: Option<egui::Pos2>,
    mouse_down: bool,
    touches: HashMap<u64, i32>,
}

pub(super) struct InputTransport {
    surface: crate::platform::Surface,
    state: Rc<RefCell<GameplayInputState>>,
    hotbar: HotbarSelection,
    pointers: RefCell<Pointers>,
}

impl InputTransport {
    pub fn attach(
        surface: &crate::platform::Surface,
        state: Rc<RefCell<GameplayInputState>>,
        hotbar: HotbarSelection,
    ) -> Result<Self, String> {
        Ok(Self {
            surface: surface.clone(),
            state,
            hotbar,
            pointers: RefCell::new(Pointers::default()),
        })
    }

    pub fn release_pointers(&self, released: Vec<i32>) {
        self.pointers.borrow_mut().touches.retain(|_, id| !released.contains(id));
    }

    pub fn window_event(&self, event: &WindowEvent) -> Result<(), String> {
        let mut state = self.state.borrow_mut();
        let mut pointers = self.pointers.borrow_mut();
        let logical = |point: winit::dpi::PhysicalPosition<f64>| {
            let point = point.to_logical::<f64>(self.surface.scale_factor());
            egui::pos2(point.x as f32, point.y as f32)
        };
        let vector = |point: egui::Pos2| Vec2::new(f64::from(point.x), f64::from(point.y));
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                let point = logical(*position);
                pointers.position = Some(point);
                state.move_ui_pointer(0, vector(point), self.hotbar.bank());
                if let Some(fraction) = self.surface.point_fraction(vector(point)) {
                    state.mouse(MousePhase::Move, 0, point, fraction, self.hotbar.start());
                }
            }
            WindowEvent::MouseInput { state: pressed, button, .. } => {
                let Some(point) = pointers.position else { return Ok(()) };
                let Some(fraction) = self.surface.point_fraction(vector(point)) else {
                    return Ok(());
                };
                let Some(button) = mouse_button(*button) else { return Ok(()) };
                let pressed = *pressed == ElementState::Pressed;
                if pressed {
                    state.begin_ui_pointer(0, button, vector(point));
                } else {
                    state.end_ui_pointer(0, true, self.hotbar.bank());
                }
                state.mouse(
                    if pressed { MousePhase::Down } else { MousePhase::Up },
                    button,
                    point,
                    fraction,
                    self.hotbar.start(),
                );
                if button == 0 {
                    if !pressed
                        && pointers.mouse_down
                        && (0.0..=1.0).contains(&fraction.x)
                        && (0.0..=1.0).contains(&fraction.y)
                    {
                        state.click(button, point, Some(fraction));
                    }
                    pointers.mouse_down = pressed;
                }
            }
            WindowEvent::Touch(touch) => {
                let point = vector(logical(touch.location));
                if touch.phase == TouchPhase::Started {
                    let id = (1..=i32::MAX)
                        .find(|id| !pointers.touches.values().any(|used| used == id))
                        .ok_or("Too many active touch inputs")?;
                    if state.begin_ui_pointer(id, 0, point).owned {
                        pointers.touches.insert(touch.id, id);
                    }
                } else if let Some(id) = pointers.touches.get(&touch.id).copied() {
                    if touch.phase == TouchPhase::Moved {
                        state.move_ui_pointer(id, point, self.hotbar.bank());
                    } else {
                        state.end_ui_pointer(
                            id,
                            touch.phase == TouchPhase::Ended,
                            self.hotbar.bank(),
                        );
                        pointers.touches.remove(&touch.id);
                    }
                }
            }
            WindowEvent::Focused(false) | WindowEvent::Occluded(true) => {
                clear_input(&mut state);
                *pointers = Pointers::default();
            }
            _ => {}
        }
        Ok(())
    }
}

fn mouse_button(button: MouseButton) -> Option<i16> {
    Some(match button {
        MouseButton::Left => 0,
        MouseButton::Middle => 1,
        MouseButton::Right => 2,
        MouseButton::Back => 3,
        MouseButton::Forward => 4,
        MouseButton::Other(button) => i16::try_from(button).ok()?,
    })
}
