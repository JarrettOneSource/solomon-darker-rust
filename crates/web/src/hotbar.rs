//! Current Website `HotbarControls.tsx` owns one presentation bank across the
//! gameplay HUD and both optional books. All assignments remain player-owned.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use solomon_sim::belt::{NATIVE_BELT_SLOT_COUNT, PLAYER_HOTBAR_COUNT};
use solomon_sim::hud::HudRect;
use solomon_sim::settings::GameSettings;
use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{Document, Event, HtmlElement, KeyboardEvent, Window};

#[derive(Clone, Copy)]
pub enum HotbarDirection {
    Previous,
    Next,
}

struct Shortcut {
    code: String,
    enabled: bool,
    reverse: bool,
}

struct SelectionState {
    bank: Cell<usize>,
    shortcut: RefCell<Shortcut>,
}

struct SelectionOwner {
    state: Rc<SelectionState>,
    window: Window,
    listener: Closure<dyn FnMut(Event)>,
}

impl Drop for SelectionOwner {
    fn drop(&mut self) {
        if let Err(error) = self.window.remove_event_listener_with_callback_and_bool(
            "keydown",
            self.listener.as_ref().unchecked_ref(),
            true,
        ) {
            web_sys::console::error_1(&error);
        }
    }
}

/// Clone this one owner into the HUD, input and optional books. Selection starts
/// at bank zero and survives world/session changes for the page's lifetime.
#[derive(Clone)]
pub struct HotbarSelection(Rc<SelectionOwner>);

impl HotbarSelection {
    pub fn attach(window: &Window) -> Result<Self, String> {
        let state = Rc::new(SelectionState {
            bank: Cell::new(0),
            shortcut: RefCell::new(Shortcut {
                code: GameSettings::default().controls.cycle_hotbar,
                enabled: false,
                reverse: false,
            }),
        });
        let keyed = state.clone();
        let listener = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
            let Some(key) = event.dyn_ref::<KeyboardEvent>() else { return };
            let shortcut = keyed.shortcut.borrow();
            if !shortcut.enabled
                || key.code() != shortcut.code
                || key.repeat()
                || event.default_prevented()
                || key.alt_key()
                || key.ctrl_key()
                || key.meta_key()
            {
                return;
            }
            let editing = event
                .target()
                .and_then(|target| target.dyn_into::<HtmlElement>().ok())
                .is_some_and(|target| {
                    target
                        .closest("input,textarea,[contenteditable='true']")
                        .ok()
                        .flatten()
                        .is_some()
                });
            if editing {
                return;
            }
            event.prevent_default();
            event.stop_immediate_propagation();
            let direction = if key.shift_key() != shortcut.reverse {
                HotbarDirection::Previous
            } else {
                HotbarDirection::Next
            };
            cycle_bank(&keyed.bank, direction);
        });
        window
            .add_event_listener_with_callback_and_bool(
                "keydown",
                listener.as_ref().unchecked_ref(),
                true,
            )
            .map_err(|_| "hotbar shortcut listener")?;
        Ok(Self(Rc::new(SelectionOwner { state, window: window.clone(), listener })))
    }

    #[must_use]
    pub fn bank(&self) -> usize {
        self.0.state.bank.get()
    }

    #[must_use]
    pub fn start(&self) -> usize {
        self.bank() * NATIVE_BELT_SLOT_COUNT
    }

    #[must_use]
    pub fn slot(&self, visible_slot: usize) -> usize {
        self.start() + visible_slot
    }

    pub fn cycle(&self, direction: HotbarDirection) {
        cycle_bank(&self.0.state.bank, direction);
    }

    /// Game supplies the Source admission predicate; an owned optional-book pause
    /// allows cycling, while settings/chat/loading/resume barriers do not.
    pub fn set_shortcut(&self, settings: &GameSettings, enabled: bool) {
        let mut shortcut = self.0.state.shortcut.borrow_mut();
        if shortcut.code != settings.controls.cycle_hotbar {
            shortcut.code.clone_from(&settings.controls.cycle_hotbar);
        }
        shortcut.enabled = enabled;
        shortcut.reverse = settings.reverse_hotbar_cycle;
    }
}

fn cycle_bank(bank: &Cell<usize>, direction: HotbarDirection) {
    let step = match direction {
        HotbarDirection::Previous => PLAYER_HOTBAR_COUNT - 1,
        HotbarDirection::Next => 1,
    };
    bank.set((bank.get() + step) % PLAYER_HOTBAR_COUNT);
}

/// GPU controls share the same bank owner as keyboard and controller input.
pub(crate) struct HotbarControls {
    selection: HotbarSelection,
    previous_bank: Cell<Option<usize>>,
    disabled: Cell<bool>,
    modal: bool,
}

impl HotbarControls {
    pub fn new(
        _document: &Document,
        selection: HotbarSelection,
        modal_rects: Option<&[HudRect; 8]>,
    ) -> Result<Self, String> {
        Ok(Self {
            selection,
            previous_bank: Cell::new(None),
            disabled: Cell::new(true),
            modal: modal_rects.is_some(),
        })
    }

    pub fn update(&self, disabled: bool) -> Result<bool, String> {
        self.disabled.set(disabled);
        Ok(self.previous_bank.replace(Some(self.selection.bank())) != Some(self.selection.bank()))
    }

    pub fn paint(
        &self,
        frame: &crate::gpu_ui::UiFrame<'_>,
        ui: &egui::Ui,
        belt: &[HudRect; 8],
        coarse: bool,
        viewport: egui::Vec2,
    ) {
        let top = if coarse && !self.modal { viewport.y - 155.0 } else { belt[0].y as f32 + 10.0 };
        let center = viewport.x / 2.0;
        let width = if coarse && !self.modal { 40.0 } else { 34.0 };
        for (direction, x, label) in [
            (
                HotbarDirection::Previous,
                if coarse && !self.modal { center - 76.0 } else { belt[0].x as f32 - 46.0 },
                "Previous hotbar",
            ),
            (
                HotbarDirection::Next,
                if coarse && !self.modal {
                    center + 36.0
                } else {
                    (belt[7].x + belt[7].width + 12.0) as f32
                },
                "Next hotbar",
            ),
        ] {
            let id = format!(
                "{}:hotbar:{}",
                if self.modal { "panel" } else { "hud" },
                if matches!(direction, HotbarDirection::Previous) { "previous" } else { "next" }
            );
            let rect = egui::Rect::from_min_size(
                egui::pos2(x, top),
                egui::vec2(width, if coarse && !self.modal { 40.0 } else { 32.0 }),
            );
            let response = ui.interact(
                rect,
                egui::Id::unique(&id),
                if self.disabled.get() { egui::Sense::hover() } else { egui::Sense::click() },
            );
            let alpha = if self.disabled.get() { 0.4 } else { 1.0 };
            let painter = ui.painter();
            painter.rect_filled(
                rect,
                3.0,
                egui::Color32::from_rgb(24, 22, 15).gamma_multiply(alpha),
            );
            painter.rect_stroke(
                rect,
                3.0,
                egui::Stroke::new(1.0, egui::Color32::from_rgb(184, 155, 87).gamma_multiply(alpha)),
                egui::StrokeKind::Inside,
            );
            if response.has_focus() {
                painter.rect_stroke(
                    rect.expand(4.0),
                    6.0,
                    egui::Stroke::new(2.0, egui::Color32::from_rgb(240, 215, 143)),
                    egui::StrokeKind::Outside,
                );
            }
            let sign = if matches!(direction, HotbarDirection::Previous) { -1.0 } else { 1.0 };
            let c = rect.center();
            painter.add(egui::Shape::line(
                vec![
                    egui::pos2(c.x - sign * 3.0, c.y - 6.0),
                    egui::pos2(c.x + sign * 3.0, c.y),
                    egui::pos2(c.x - sign * 3.0, c.y + 6.0),
                ],
                egui::Stroke::new(
                    3.0,
                    egui::Color32::from_rgb(233, 210, 147).gamma_multiply(alpha),
                ),
            ));
            frame.record(&id, label, "button", &response);
            if response.clicked() && !self.disabled.get() {
                self.selection.cycle(direction);
            }
        }
        let dot_y = if coarse && !self.modal { top + 15.0 } else { belt[0].y as f32 - 13.5 };
        for bank in 0..PLAYER_HOTBAR_COUNT {
            let active = bank == self.selection.bank();
            let p = egui::pos2(center - 20.0 + bank as f32 * 20.0, dot_y);
            if active {
                for (r, a) in [(9.0, 0.08), (7.0, 0.12)] {
                    ui.painter().circle_filled(
                        p,
                        r,
                        egui::Color32::from_rgb(208, 174, 100).gamma_multiply(a),
                    );
                }
            }
            ui.painter().circle(
                p,
                5.0,
                if active {
                    egui::Color32::from_rgb(208, 174, 100)
                } else {
                    egui::Color32::from_rgb(12, 13, 9)
                },
                egui::Stroke::new(1.0, egui::Color32::from_rgb(184, 155, 87)),
            );
        }
    }
}
