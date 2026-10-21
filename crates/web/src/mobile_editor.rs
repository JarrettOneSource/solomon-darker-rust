//! The device layout editor in Website `MobileUiEditor.tsx`.
//! The draft, gesture history and page navigation live here. Only a saved draft leaves
//! through MenuUi; native controls continue using the actual device store until then.

mod interaction;
mod presentation;
mod preview;

use std::cell::RefCell;
use std::rc::Rc;

use solomon_sim::Vec2;
use solomon_sim::mobile_ui::*;
use wasm_bindgen::{JsCast, prelude::*};
use web_sys::{
    Document, Element, Event, EventTarget, HtmlElement, KeyboardEvent, PointerEvent, WheelEvent,
};

use crate::ui::{StockAtlas, append, element};

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
}

type PointerCaptures = Rc<RefCell<Vec<(i32, Hit, HtmlElement)>>>;

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

struct Listener {
    target: EventTarget,
    name: &'static str,
    capture: bool,
    callback: Closure<dyn FnMut(Event)>,
}

impl Drop for Listener {
    fn drop(&mut self) {
        if let Err(error) = self.target.remove_event_listener_with_callback_and_bool(
            self.name,
            self.callback.as_ref().unchecked_ref(),
            self.capture,
        ) {
            web_sys::console::error_1(&error);
        }
    }
}

pub(crate) struct MobileUiEditor {
    pub root: HtmlElement,
    document: Document,
    art: Rc<MobileEditorArt>,
    viewport: HtmlElement,
    shell: HtmlElement,
    page_node: HtmlElement,
    guide_layer: HtmlElement,
    elements: Vec<HtmlElement>,
    preview_nodes: Vec<HtmlElement>,
    dock: HtmlElement,
    inspector: HtmlElement,
    status: HtmlElement,
    toolbar: HtmlElement,
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
    dirty_preview: bool,
    busy: bool,
    error: Option<String>,
    events: Rc<RefCell<Vec<EditorEvent>>>,
    listeners: Vec<Listener>,
    pointer_captures: PointerCaptures,
}

impl MobileUiEditor {
    pub fn new(
        document: &Document,
        art: Rc<MobileEditorArt>,
        state: &MobileUiLayoutState,
        page: MobileUiSize,
        ui_scale: f64,
        fullscreen: bool,
    ) -> Result<Self, String> {
        let root = element(document, "div", "mobile-ui-editor")?;
        root.set_attribute(
            "data-editor-presentation",
            if fullscreen { "fullscreen" } else { "windowed" },
        )
        .map_err(|_| "mobile editor presentation")?;
        let toolbar = element(document, "div", "mobile-ui-editor-toolbar")?;
        let viewport = element(document, "div", "mobile-ui-editor-viewport")?;
        let shell = element(document, "div", "mobile-ui-editor-page-shell")?;
        let page_node = element(document, "div", "mobile-ui-editor-page")?;
        page_node.set_attribute("role", "application").map_err(|_| "mobile editor page role")?;
        let guide_layer = element(document, "div", "mobile-ui-editor-guides")?;
        append(&page_node, &guide_layer)?;
        append(&shell, &page_node)?;
        append(&viewport, &shell)?;
        let dock = element(
            document,
            "div",
            if fullscreen { "mobile-ui-editor-dock" } else { "mobile-ui-editor-details" },
        )?;
        let inspector = element(
            document,
            "div",
            if fullscreen {
                "mobile-ui-editor-inspector"
            } else {
                "mobile-ui-editor-adjustments-owner"
            },
        )?;
        let status = element(document, "div", "mobile-ui-editor-status")?;
        append(&dock, &inspector)?;
        if !fullscreen {
            append(&dock, &status)?;
            append(&root, &toolbar)?;
        }
        append(&root, &viewport)?;
        append(&root, &dock)?;
        let geometry = default_mobile_ui_geometry(page.width, page.height, ui_scale);
        let layout = if state.customized { state.layout.clone() } else { geometry.layout.clone() };
        let mut editor = Self {
            root,
            document: document.clone(),
            art,
            viewport,
            shell,
            page_node,
            guide_layer,
            elements: Vec::new(),
            preview_nodes: Vec::new(),
            dock,
            inspector,
            status,
            toolbar,
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
            dirty_preview: true,
            busy: false,
            error: None,
            events: Rc::new(RefCell::new(Vec::new())),
            listeners: Vec::new(),
            pointer_captures: Rc::new(RefCell::new(Vec::new())),
        };
        editor.build_controls()?;
        editor.build_elements()?;
        editor.listen()?;
        editor.paint()?;
        Ok(editor)
    }

    pub fn saved_layout(&mut self) -> Result<Option<MobileUiLayout>, String> {
        self.update()?;
        Ok((!self.restoring_default).then(|| self.layout.clone()))
    }

    pub fn set_busy(&mut self, busy: bool, error: Option<String>) -> Result<(), String> {
        self.busy = busy;
        self.error = error;
        self.root
            .toggle_attribute_with_force("inert", busy)
            .map_err(|_| "mobile editor save admission")?;
        self.paint()
    }

    pub fn set_preview(&mut self, preview: Option<MobileEditorPlayerPreview>) {
        if self.preview != preview {
            self.preview = preview;
            self.dirty_preview = true;
        }
    }

    pub fn update(&mut self) -> Result<(), String> {
        let events = std::mem::take(&mut *self.events.borrow_mut());
        if !self.busy {
            for event in events {
                self.handle_event(event)?;
                self.paint()?;
            }
        }
        if self.dirty_preview {
            self.paint_previews()?;
            self.dirty_preview = false;
        }
        if self.fullscreen {
            self.constrain_dock()?;
        }
        Ok(())
    }

    pub fn fit_after_attach(&mut self) -> Result<(), String> {
        self.fit_page()
    }

    pub fn resize(&mut self, page: MobileUiSize, ui_scale: f64) -> Result<(), String> {
        if self.page == page && self.ui_scale == ui_scale {
            return Ok(());
        }
        self.page = page;
        self.ui_scale = ui_scale;
        self.geometry = default_mobile_ui_geometry(page.width, page.height, ui_scale);
        if self.restoring_default {
            self.layout = self.geometry.layout.clone();
        }
        self.dirty_preview = true;
        self.fit_page()?;
        self.paint()
    }

    fn listen(&mut self) -> Result<(), String> {
        let queue = self.events.clone();
        self.add_listener(self.root.clone().into(), "click", true, move |_| {
            queue.borrow_mut().push(EditorEvent::BeginChange);
        })?;
        let queue = self.events.clone();
        self.add_listener(self.root.clone().into(), "click", false, move |event| {
            report(queue_command(&event, &queue));
        })?;
        let queue = self.events.clone();
        self.add_listener(self.root.clone().into(), "change", false, move |event| {
            report(queue_value(&event, &queue));
        })?;
        let queue = self.events.clone();
        self.add_listener(self.root.clone().into(), "input", false, move |event| {
            report(queue_value(&event, &queue));
        })?;
        let queue = self.events.clone();
        self.add_listener(self.root.clone().into(), "focusin", false, move |event| {
            if let Some(id) = event_element(&event).and_then(|target| element_id(&target)) {
                queue.borrow_mut().push(EditorEvent::Select(id));
            }
        })?;
        let queue = self.events.clone();
        self.add_listener(self.root.clone().into(), "keydown", true, move |event| {
            let Some(key) = event.dyn_ref::<KeyboardEvent>() else { return };
            let element = event_element(&event).and_then(|target| element_id(&target));
            let command = key.ctrl_key() || key.meta_key();
            let undo =
                command && !key.alt_key() && matches!(key.key().to_lowercase().as_str(), "z" | "y");
            let moves = element.is_some()
                && matches!(
                    key.key().as_str(),
                    "ArrowLeft" | "ArrowRight" | "ArrowUp" | "ArrowDown" | "[" | "]" | "-" | "+"
                );
            if undo || moves {
                event.prevent_default();
                event.stop_propagation();
            }
            queue.borrow_mut().push(EditorEvent::Key {
                key: key.key(),
                shift: key.shift_key(),
                command,
                alt: key.alt_key(),
                element,
            });
        })?;
        for (name, phase) in [
            ("pointerdown", PointerPhase::Start),
            ("pointermove", PointerPhase::Move),
            ("pointerup", PointerPhase::End),
            ("pointercancel", PointerPhase::End),
            ("lostpointercapture", PointerPhase::End),
        ] {
            let queue = self.events.clone();
            let fullscreen = self.fullscreen;
            let captures = self.pointer_captures.clone();
            self.add_listener(self.root.clone().into(), name, false, move |event| {
                report(queue_pointer(&event, &queue, &captures, phase, fullscreen));
            })?;
        }
        let queue = self.events.clone();
        let fullscreen = self.fullscreen;
        self.add_listener(self.viewport.clone().into(), "wheel", false, move |event| {
            if fullscreen {
                return;
            }
            let Some(wheel) = event.dyn_ref::<WheelEvent>() else { return };
            event.prevent_default();
            queue.borrow_mut().push(EditorEvent::Wheel {
                delta: wheel.delta_y(),
                client: Vec2::new(f64::from(wheel.client_x()), f64::from(wheel.client_y())),
            });
        })?;
        Ok(())
    }

    fn add_listener(
        &mut self,
        target: EventTarget,
        name: &'static str,
        capture: bool,
        callback: impl FnMut(Event) + 'static,
    ) -> Result<(), String> {
        let callback = Closure::<dyn FnMut(Event)>::new(callback);
        target
            .add_event_listener_with_callback_and_bool(
                name,
                callback.as_ref().unchecked_ref(),
                capture,
            )
            .map_err(|_| format!("mobile editor {name} listener"))?;
        self.listeners.push(Listener { target, name, capture, callback });
        Ok(())
    }
}

fn event_element(event: &Event) -> Option<Element> {
    event.target()?.dyn_into().ok()
}
fn element_id(element: &Element) -> Option<MobileUiElementId> {
    element
        .closest("[data-mobile-ui-editor-element]")
        .ok()??
        .get_attribute("data-mobile-ui-editor-element")
        .and_then(|id| MobileUiElementId::from_id(&id))
}

fn queue_command(event: &Event, queue: &Rc<RefCell<Vec<EditorEvent>>>) -> Result<(), String> {
    let Some(target) = event_element(event) else { return Ok(()) };
    if let Some(button) =
        target.closest("[data-mobile-editor-action]").map_err(|_| "mobile editor action target")?
    {
        event.stop_propagation();
        if let Some(action) = button.get_attribute("data-mobile-editor-action") {
            queue.borrow_mut().push(EditorEvent::Command(action));
        }
    }
    Ok(())
}

fn queue_value(event: &Event, queue: &Rc<RefCell<Vec<EditorEvent>>>) -> Result<(), String> {
    let Some(target) = event_element(event) else { return Ok(()) };
    if target.has_attribute("data-mobile-editor-select") {
        let select =
            target.dyn_into::<web_sys::HtmlSelectElement>().map_err(|_| "mobile editor select")?;
        if let Some(id) = MobileUiElementId::from_id(&select.value()) {
            queue.borrow_mut().push(EditorEvent::Select(id));
        }
    } else if target.has_attribute("data-mobile-editor-scale") {
        let input = target
            .dyn_into::<web_sys::HtmlInputElement>()
            .map_err(|_| "mobile editor size input")?;
        queue.borrow_mut().push(EditorEvent::Scale(input.value_as_number() / 100.0));
    }
    Ok(())
}

fn queue_pointer(
    event: &Event,
    queue: &Rc<RefCell<Vec<EditorEvent>>>,
    captures: &PointerCaptures,
    phase: PointerPhase,
    fullscreen: bool,
) -> Result<(), String> {
    let Some(pointer) = event.dyn_ref::<PointerEvent>() else { return Ok(()) };
    if matches!(phase, PointerPhase::Start) {
        queue.borrow_mut().push(EditorEvent::BeginChange);
    }
    let Some(target) = event_element(event) else { return Ok(()) };
    let Some((node, hit)) = pointer_hit(&target, fullscreen)? else { return Ok(()) };
    let pointer_id = pointer.pointer_id();
    if matches!(phase, PointerPhase::Start) {
        if pointer.pointer_type() == "mouse"
            && pointer.button() != 0
            && !matches!(hit, Hit::Resize(..) | Hit::Rotate(_))
        {
            return Ok(());
        }
        // Source checks ownership before focusing or capturing. Keep this browser
        // edge synchronous even when several native pointer events precede a frame.
        if let Hit::Element(owner) = hit
            && captures
                .borrow()
                .iter()
                .any(|(_, previous, _)| matches!(previous, Hit::Element(id) if *id != owner))
        {
            return Ok(());
        }
        captures.borrow_mut().push((pointer_id, hit, node.clone()));
    } else if !captures.borrow().iter().any(|(id, owner, _)| *id == pointer_id && *owner == hit) {
        return Ok(());
    }
    event.prevent_default();
    event.stop_propagation();
    queue.borrow_mut().push(EditorEvent::Pointer {
        phase,
        hit,
        id: pointer_id,
        client: Vec2::new(f64::from(pointer.client_x()), f64::from(pointer.client_y())),
    });
    if matches!(phase, PointerPhase::Start) {
        if matches!(hit, Hit::Element(_)) {
            let options = web_sys::FocusOptions::new();
            options.set_prevent_scroll(true);
            node.focus_with_options(&options).map_err(|_| "mobile editor control focus")?;
        }
        node.set_pointer_capture(pointer_id).map_err(|_| "mobile editor pointer capture")?;
    } else if matches!(phase, PointerPhase::End) {
        // Removing the accepted pointer first makes lostpointercapture idempotent.
        captures.borrow_mut().retain(|(id, _, _)| *id != pointer_id);
        if node.has_pointer_capture(pointer_id) {
            node.release_pointer_capture(pointer_id)
                .map_err(|_| "mobile editor pointer release")?;
        }
    }
    Ok(())
}

fn pointer_hit(target: &Element, fullscreen: bool) -> Result<Option<(HtmlElement, Hit)>, String> {
    for selector in [
        "[data-resize-handle]",
        ".mobile-ui-editor-rotate-handle",
        ".mobile-ui-editor-dock-handle",
        "[data-mobile-ui-editor-element]",
        ".mobile-ui-editor-viewport",
    ] {
        let Some(node) = target.closest(selector).map_err(|_| "mobile editor gesture target")?
        else {
            continue;
        };
        let hit = match selector {
            "[data-resize-handle]" => Hit::Resize(
                element_id(&node).ok_or("mobile resize owner")?,
                MobileUiResizeHandle::from_id(
                    &node.get_attribute("data-resize-handle").ok_or("mobile resize handle")?,
                )
                .ok_or("mobile resize identity")?,
            ),
            ".mobile-ui-editor-rotate-handle" => {
                Hit::Rotate(element_id(&node).ok_or("mobile rotation owner")?)
            }
            ".mobile-ui-editor-dock-handle" => Hit::Dock,
            "[data-mobile-ui-editor-element]" => {
                Hit::Element(element_id(&node).ok_or("mobile gesture owner")?)
            }
            _ if !fullscreen => Hit::Page,
            _ => return Ok(None),
        };
        return Ok(Some((
            node.dyn_into::<HtmlElement>().map_err(|_| "mobile gesture element")?,
            hit,
        )));
    }
    Ok(None)
}

fn report(result: Result<(), String>) {
    if let Err(error) = result {
        web_sys::console::error_1(&JsValue::from_str(&error));
    }
}

impl Drop for MobileUiEditor {
    fn drop(&mut self) {
        let captures = std::mem::take(&mut *self.pointer_captures.borrow_mut());
        for (id, _, node) in captures {
            if node.has_pointer_capture(id) {
                report(
                    node.release_pointer_capture(id)
                        .map_err(|_| "mobile editor pointer cleanup".to_owned()),
                );
            }
        }
        self.root.remove();
    }
}
