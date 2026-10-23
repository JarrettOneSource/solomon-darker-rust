//! GPU layout editor from Website `MobileUiEditor.tsx`.
//! Layout drafts, gestures and undo history stay local until Settings saves the draft.

mod interaction;
mod presentation;
mod preview;

use crate::gpu_ui::{StockAtlas, UiFrame};
use egui::{Color32, Id, Order, Rect, pos2, vec2};
use solomon_sim::Vec2;
use solomon_sim::hud::HudRect;
use solomon_sim::mobile_ui::*;
use std::rc::Rc;
use web_sys::Document;

pub(crate) use preview::MobileEditorPlayerPreview;
pub(crate) struct MobileEditorArt {
    ui: StockAtlas,
    skills: StockAtlas,
    inventory: StockAtlas,
}
impl MobileEditorArt {
    pub async fn load() -> Result<Self, String> {
        Ok(Self {
            ui: StockAtlas::load("UI").await?,
            skills: StockAtlas::load("Skills").await?,
            inventory: StockAtlas::load("Inventory").await?,
        })
    }
}
#[derive(Clone)]
struct EditorSnapshot {
    layout: MobileUiLayout,
    restoring_default: bool,
}

#[derive(Clone, Copy)]
enum PointerPhase {
    Start,
    Move,
    End,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Hit {
    Element(MobileUiElementId),
    Resize(MobileUiElementId, MobileUiResizeHandle),
    Rotate(MobileUiElementId),
    Page,
    Dock,
    Chrome,
}

enum EditorEvent {
    BeginChange,
    Command(String),
    Select(MobileUiElementId),
    Scale(f64),
    Key { key: String, shift: bool, command: bool, alt: bool, element: Option<MobileUiElementId> },
    Pointer { phase: PointerPhase, hit: Hit, id: i32, client: Vec2 },
    Wheel { delta: f64, client: Vec2 },
}

#[derive(Clone, Copy)]
enum ElementInteraction {
    Drag { id: MobileUiElementId, initial_pointer: Vec2, initial: MobileUiElementTransform },
    Pinch { id: MobileUiElementId, initial_distance: f64, initial: MobileUiElementTransform },
}

#[derive(Clone, Copy)]
enum HandleInteraction {
    Rotate {
        id: MobileUiElementId,
        pointer_id: i32,
        center: Vec2,
        initial_angle: f64,
        initial_rotation: f64,
    },
    Resize {
        id: MobileUiElementId,
        pointer_id: i32,
        gesture: MobileUiResizeGesture,
    },
}

#[derive(Clone, Copy)]
enum PageInteraction {
    Pan { initial_client: Vec2, initial_scroll: Vec2 },
    Pinch { anchor: Vec2, initial_distance: f64, initial_zoom: f64 },
}

#[derive(Clone, Copy)]
struct DockInteraction {
    offset: Vec2,
    pointer_id: i32,
}

#[derive(Clone, Copy)]
struct HitTarget {
    hit: Hit,
    center: egui::Pos2,
    size: egui::Vec2,
    rotation: f32,
}
impl HitTarget {
    fn contains(self, point: egui::Pos2) -> bool {
        let delta = egui::emath::Rot2::from_angle(-self.rotation) * (point - self.center);
        delta.x.abs() <= self.size.x / 2.0 && delta.y.abs() <= self.size.y / 2.0
    }
}
pub(crate) struct MobileUiEditor {
    art: Rc<MobileEditorArt>,
    root_bounds: HudRect,
    viewport: HudRect,
    page_bounds: HudRect,
    elements: [HudRect; 18],
    dock_bounds: HudRect,
    layout: MobileUiLayout,
    restoring_default: bool,
    geometry: MobileUiGeometry,
    page: MobileUiSize,
    ui_scale: f64,
    fullscreen: bool,
    selected: MobileUiElementId,
    grid: bool,
    guides: Vec<MobileUiSnapGuide>,
    zoom: f64,
    tools_open: bool,
    past: Vec<EditorSnapshot>,
    future: Vec<EditorSnapshot>,
    recorded_interaction: bool,
    // Source Map insertion order determines which two touches form a pinch.
    element_pointers: Vec<(i32, Vec2)>,
    element_owner: Option<MobileUiElementId>,
    element_interaction: Option<ElementInteraction>,
    handle_interaction: Option<HandleInteraction>,
    page_pointers: Vec<(i32, Vec2)>,
    page_interaction: Option<PageInteraction>,
    dock_interaction: Option<DockInteraction>,
    dock_position: Option<Vec2>,
    pending_scroll: Option<Vec2>,
    preview: Option<MobileEditorPlayerPreview>,
    busy: bool,
    error: Option<String>,
    events: Vec<EditorEvent>,
    pointer_captures: Vec<(i32, Hit)>,
    hit_targets: Vec<HitTarget>,
    scroll: Vec2,
    fit_pending: bool,
    save_request: bool,
    focused_element: Option<MobileUiElementId>,
}
impl MobileUiEditor {
    pub fn new(
        _document: &Document,
        art: Rc<MobileEditorArt>,
        state: &MobileUiLayoutState,
        page: MobileUiSize,
        ui_scale: f64,
        fullscreen: bool,
    ) -> Result<Self, String> {
        let geometry = default_mobile_ui_geometry(page.width, page.height, ui_scale);
        let layout = if state.customized { state.layout.clone() } else { geometry.layout.clone() };
        let bounds = HudRect::new(0.0, 0.0, page.width, page.height);
        Ok(Self {
            art,
            root_bounds: bounds,
            viewport: bounds,
            page_bounds: bounds,
            elements: [bounds; 18],
            dock_bounds: HudRect::new(0.0, 54.0, 360.0, 120.0),
            layout,
            restoring_default: !state.customized,
            geometry,
            page,
            ui_scale,
            fullscreen,
            selected: MobileUiElementId::Pause,
            grid: true,
            guides: Vec::new(),
            zoom: 1.0,
            tools_open: false,
            past: Vec::new(),
            future: Vec::new(),
            recorded_interaction: false,
            element_pointers: Vec::new(),
            element_owner: None,
            element_interaction: None,
            handle_interaction: None,
            page_pointers: Vec::new(),
            page_interaction: None,
            dock_interaction: None,
            dock_position: None,
            pending_scroll: None,
            preview: None,
            busy: false,
            error: None,
            events: Vec::new(),
            pointer_captures: Vec::new(),
            hit_targets: Vec::new(),
            scroll: Vec2::ZERO,
            fit_pending: true,
            save_request: false,
            focused_element: None,
        })
    }
    pub fn saved_layout(&mut self) -> Result<Option<MobileUiLayout>, String> {
        self.update()?;
        Ok((!self.restoring_default).then(|| self.layout.clone()))
    }
    pub fn set_busy(&mut self, busy: bool, error: Option<String>) -> Result<(), String> {
        self.busy = busy;
        self.error = error;
        if busy {
            self.cancel_gestures()?;
        }
        Ok(())
    }
    pub fn set_preview(&mut self, preview: Option<MobileEditorPlayerPreview>) {
        self.preview = preview;
    }
    pub fn update(&mut self) -> Result<(), String> {
        let events = std::mem::take(&mut self.events);
        if !self.busy {
            for event in events {
                self.handle_event(event)?;
            }
        }
        if self.fullscreen {
            self.constrain_dock()?;
        }
        Ok(())
    }
    pub fn fit_after_attach(&mut self) -> Result<(), String> {
        self.fit_pending = true;
        Ok(())
    }
    pub fn resize(&mut self, page: MobileUiSize, ui_scale: f64) -> Result<(), String> {
        if self.page != page || self.ui_scale != ui_scale {
            self.page = page;
            self.ui_scale = ui_scale;
            self.geometry = default_mobile_ui_geometry(page.width, page.height, ui_scale);
            if self.restoring_default {
                self.layout = self.geometry.layout.clone();
            }
            self.fit_pending = true;
        }
        Ok(())
    }
    pub fn take_save_request(&mut self) -> bool {
        std::mem::take(&mut self.save_request)
    }
    pub fn paint(&mut self, frame: &mut UiFrame<'_>) -> Result<(), String> {
        let bounds = frame.ctx.content_rect();
        self.paint_in(frame, bounds)
    }
    pub fn paint_in(&mut self, frame: &mut UiFrame<'_>, bounds: Rect) -> Result<(), String> {
        let ctx = frame.ctx.clone();
        self.root_bounds = hud_rect(bounds);
        let toolbar_height = if self.fullscreen { 0.0 } else { 80.0 };
        let details_height = if self.fullscreen { 0.0 } else { 146.0 };
        let viewport = Rect::from_min_max(
            bounds.min + vec2(0.0, toolbar_height),
            bounds.max - vec2(0.0, details_height),
        );
        self.viewport = hud_rect(viewport);
        if self.fit_pending {
            self.fit_page()?;
            self.fit_pending = false;
        }
        if let Some(scroll) = self.pending_scroll.take() {
            self.set_scroll_position(scroll)?;
        }
        self.refresh_page_bounds();
        self.read_input(frame)?;
        self.update()?;
        if let Some(scroll) = self.pending_scroll.take() {
            self.set_scroll_position(scroll)?;
        }
        self.refresh_page_bounds();
        self.hit_targets.clear();
        let mut error = None;
        egui::Area::new(Id::unique("mobile/editor"))
            .order(Order::Foreground)
            .fixed_pos(bounds.min)
            .show(&ctx, |ui| {
                ui.set_min_size(bounds.size());
                ui.set_clip_rect(bounds);
                ui.painter().rect_filled(viewport, 0.0, Color32::from_rgb(14, 18, 24));
                if !self.fullscreen {
                    let response =
                        ui.interact(viewport, Id::unique("mobile/page-pan"), egui::Sense::drag());
                    frame.record(
                        "mobile/viewport",
                        "Mobile layout viewport",
                        "application",
                        &response,
                    );
                    self.hit_targets.push(HitTarget {
                        hit: Hit::Page,
                        center: viewport.center(),
                        size: viewport.size(),
                        rotation: 0.0,
                    });
                }
                if let Err(message) = self.paint_page(frame, ui, viewport) {
                    error = Some(message);
                }
                if self.fullscreen {
                    if let Err(message) = self.paint_dock(frame, ui, bounds) {
                        error = Some(message);
                    }
                } else {
                    self.paint_toolbar(
                        frame,
                        ui,
                        Rect::from_min_size(bounds.min, vec2(bounds.width(), toolbar_height)),
                    );
                    self.paint_inspector(
                        frame,
                        ui,
                        Rect::from_min_max(pos2(bounds.left(), viewport.bottom()), bounds.max),
                    );
                }
                if let Some(message) = &self.error {
                    let rect = Rect::from_min_size(
                        bounds.left_bottom() - vec2(0.0, 32.0),
                        vec2(bounds.width(), 32.0),
                    );
                    ui.painter().rect_filled(rect, 0.0, Color32::from_rgb(60, 16, 14));
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        message,
                        egui::FontId::proportional(13.0),
                        Color32::from_rgb(255, 160, 140),
                    );
                    frame.record_rect("mobile/error", message, "alert", rect, true);
                }
                frame.record_rect(
                    "mobile/editor",
                    "Mobile controls",
                    "application",
                    bounds,
                    !self.busy,
                );
                frame.record_value(
                    "mobile/editor",
                    self.selected.as_str(),
                    Some(!self.restoring_default),
                );
            });
        if let Some(error) = error {
            return Err(error);
        }
        self.update()?;
        Ok(())
    }
    fn refresh_page_bounds(&mut self) {
        let inset = if self.fullscreen { 0.0 } else { 24.0 };
        self.page_bounds = HudRect::new(
            self.viewport.x
                + if self.fullscreen {
                    0.0
                } else {
                    ((self.viewport.width - self.page.width * self.zoom - 48.0) / 2.0).max(0.0)
                }
                + inset
                - self.scroll.x,
            self.viewport.y + inset - self.scroll.y,
            self.page.width * self.zoom,
            self.page.height * self.zoom,
        );
    }
    fn read_input(&mut self, frame: &UiFrame<'_>) -> Result<(), String> {
        if self.busy {
            return Ok(());
        }
        let events = frame.ctx.input(|input| input.raw.events.clone());
        let touches = events.iter().any(|event| matches!(event, egui::Event::Touch { .. }));
        for event in events {
            match event {
                egui::Event::Touch { id, phase, pos, .. } => {
                    let phase = match phase {
                        egui::TouchPhase::Start => PointerPhase::Start,
                        egui::TouchPhase::Move => PointerPhase::Move,
                        egui::TouchPhase::End | egui::TouchPhase::Cancel => PointerPhase::End,
                    };
                    self.pointer(phase, id.0 as i32, pos)?;
                }
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    ..
                } if !touches => self.pointer(
                    if pressed { PointerPhase::Start } else { PointerPhase::End },
                    -1,
                    pos,
                )?,
                egui::Event::PointerMoved(pos)
                    if !touches && self.pointer_captures.iter().any(|(id, _)| *id == -1) =>
                {
                    self.pointer(PointerPhase::Move, -1, pos)?
                }
                egui::Event::WindowFocused(false) => self.cancel_gestures()?,
                egui::Event::MouseWheel { delta, .. } if !self.fullscreen => {
                    if let Some(pos) = frame.ctx.input(|input| input.pointer.hover_pos())
                        && egui_rect(self.viewport).contains(pos)
                    {
                        self.handle_event(EditorEvent::Wheel {
                            delta: -f64::from(delta.y),
                            client: Vec2::new(f64::from(pos.x), f64::from(pos.y)),
                        })?;
                    }
                }
                egui::Event::Key { key, pressed: true, modifiers, .. } => {
                    let name = match key {
                        egui::Key::ArrowLeft => "ArrowLeft",
                        egui::Key::ArrowRight => "ArrowRight",
                        egui::Key::ArrowUp => "ArrowUp",
                        egui::Key::ArrowDown => "ArrowDown",
                        egui::Key::Z => "z",
                        egui::Key::Y => "y",
                        egui::Key::Minus => "-",
                        egui::Key::Plus => "+",
                        egui::Key::OpenBracket => "[",
                        egui::Key::CloseBracket => "]",
                        _ => continue,
                    };
                    self.handle_event(EditorEvent::Key {
                        key: name.to_owned(),
                        shift: modifiers.shift,
                        command: modifiers.command || modifiers.ctrl,
                        alt: modifiers.alt,
                        element: self.focused_element,
                    })?;
                }
                _ => {}
            }
        }
        Ok(())
    }
    fn pointer(&mut self, phase: PointerPhase, id: i32, point: egui::Pos2) -> Result<(), String> {
        let hit = if matches!(phase, PointerPhase::Start) {
            let Some(target) = self.hit_targets.iter().rev().find(|target| {
                target.contains(point)
                    && (matches!(target.hit, Hit::Chrome | Hit::Dock)
                        || egui_rect(self.viewport).contains(point))
            }) else {
                return Ok(());
            };
            let hit = target.hit;
            if hit == Hit::Chrome {
                return Ok(());
            }
            if let Hit::Element(owner) = hit
                && self
                    .pointer_captures
                    .iter()
                    .any(|(_, hit)| matches!(hit,Hit::Element(id)if *id!=owner))
            {
                return Ok(());
            }
            self.handle_event(EditorEvent::BeginChange)?;
            self.pointer_captures.push((id, hit));
            if let Hit::Element(owner) = hit {
                self.focused_element = Some(owner);
            }
            hit
        } else {
            let Some((_, hit)) = self.pointer_captures.iter().find(|(pointer, _)| *pointer == id)
            else {
                return Ok(());
            };
            *hit
        };
        self.handle_event(EditorEvent::Pointer {
            phase,
            hit,
            id,
            client: Vec2::new(f64::from(point.x), f64::from(point.y)),
        })?;
        if matches!(phase, PointerPhase::End) {
            self.pointer_captures.retain(|(pointer, _)| *pointer != id);
        }
        Ok(())
    }
    fn cancel_gestures(&mut self) -> Result<(), String> {
        let captured = std::mem::take(&mut self.pointer_captures);
        for (id, hit) in captured {
            self.handle_event(EditorEvent::Pointer {
                phase: PointerPhase::End,
                hit,
                id,
                client: Vec2::ZERO,
            })?;
        }
        Ok(())
    }
}
fn hud_rect(rect: Rect) -> HudRect {
    HudRect::new(
        f64::from(rect.left()),
        f64::from(rect.top()),
        f64::from(rect.width()),
        f64::from(rect.height()),
    )
}
fn egui_rect(rect: HudRect) -> Rect {
    Rect::from_min_size(
        pos2(rect.x as f32, rect.y as f32),
        vec2(rect.width as f32, rect.height as f32),
    )
}
