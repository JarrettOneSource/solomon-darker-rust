//! Editor gestures and history from `MobileUiEditor.tsx`. The DOM supplies measured
//! client rectangles; all element constraints/snaps use the canonical pure kernel.

use solomon_sim::Vec2;
use solomon_sim::float::hypot;
use solomon_sim::mobile_ui::*;

use super::{
    DockInteraction, EditorEvent, EditorSnapshot, ElementInteraction, HandleInteraction, Hit,
    MobileUiEditor, PageInteraction, PointerPhase,
};

impl MobileUiEditor {
    pub(super) fn handle_event(&mut self, event: EditorEvent) -> Result<(), String> {
        match event {
            EditorEvent::BeginChange => {
                if self.element_pointers.is_empty() && self.handle_interaction.is_none() {
                    self.recorded_interaction = false;
                }
            }
            EditorEvent::Select(id) => {
                self.selected = id;
                self.guides.clear();
            }
            EditorEvent::Scale(scale) => {
                self.update_element(
                    self.selected,
                    MobileUiElementTransform { scale, ..self.layout.get(self.selected) },
                    Vec::new(),
                );
            }
            EditorEvent::Command(action) => self.command(&action)?,
            EditorEvent::Key { key, shift, command, alt, element } => {
                self.recorded_interaction = false;
                if command && !alt && matches!(key.to_lowercase().as_str(), "z" | "y") {
                    self.restore_history(key.eq_ignore_ascii_case("y") || shift);
                } else if let Some(id) = element {
                    self.key_move(id, &key, shift);
                }
            }
            EditorEvent::Wheel { delta, client } => {
                self.zoom_at(self.zoom * (-delta * 0.002).exp(), client)?
            }
            EditorEvent::Pointer { phase, hit, id, client } => match hit {
                Hit::Element(owner) => self.element_pointer(phase, owner, id, client),
                Hit::Resize(owner, handle) => {
                    self.handle_pointer(phase, owner, Some(handle), id, client)
                }
                Hit::Rotate(owner) => self.handle_pointer(phase, owner, None, id, client),
                Hit::Page => self.page_pointer(phase, id, client)?,
                Hit::Dock => self.dock_pointer(phase, id, client),
            },
        }
        Ok(())
    }

    fn command(&mut self, action: &str) -> Result<(), String> {
        let current = self.layout.get(self.selected);
        match action {
            "undo" => self.restore_history(false),
            "redo" => self.restore_history(true),
            "grid" => {
                self.grid = !self.grid;
                self.guides.clear();
            }
            "tools" => self.tools_open = !self.tools_open,
            "fit" => self.fit_page()?,
            "zoom-out" => self.zoom_at(self.zoom / 1.25, self.viewport_center())?,
            "zoom-in" => self.zoom_at(self.zoom * 1.25, self.viewport_center())?,
            "reset-layout" => {
                if !self.restoring_default {
                    self.remember_change();
                    self.guides.clear();
                    self.layout = self.geometry.layout.clone();
                    self.restoring_default = true;
                }
            }
            "reset-control" => self.update_element(
                self.selected,
                self.geometry.layout.get(self.selected),
                Vec::new(),
            ),
            "left" => self.nudge(Vec2::new(-1.0, 0.0)),
            "up" => self.nudge(Vec2::new(0.0, -1.0)),
            "down" => self.nudge(Vec2::new(0.0, 1.0)),
            "right" => self.nudge(Vec2::new(1.0, 0.0)),
            "smaller" | "larger" => {
                let scale = current.scale + if action == "smaller" { -0.05 } else { 0.05 };
                self.update_element(
                    self.selected,
                    MobileUiElementTransform { scale, ..current },
                    Vec::new(),
                );
            }
            "rotate-left" | "rotate-right" => {
                let angle = if action == "rotate-left" {
                    -std::f64::consts::PI / 12.0
                } else {
                    std::f64::consts::PI / 12.0
                };
                let rotation = mobile_ui_element_rotation(current.rotation, 0.0, angle, false);
                self.update_element(
                    self.selected,
                    MobileUiElementTransform { rotation, ..current },
                    Vec::new(),
                );
            }
            "straighten" => self.update_element(
                self.selected,
                MobileUiElementTransform { rotation: 0.0, ..current },
                Vec::new(),
            ),
            _ => return Err(format!("Unknown mobile editor action: {action}")),
        }
        Ok(())
    }

    fn remember_change(&mut self) {
        if self.recorded_interaction {
            return;
        }
        self.recorded_interaction = true;
        if self.past.len() >= 100 {
            self.past.drain(..self.past.len() - 99);
        }
        self.past.push(EditorSnapshot {
            layout: self.layout.clone(),
            restoring_default: self.restoring_default,
        });
        self.future.clear();
    }

    fn restore_history(&mut self, redo: bool) {
        let snapshot = if redo { self.future.pop() } else { self.past.pop() };
        let Some(snapshot) = snapshot else { return };
        let current = EditorSnapshot {
            layout: self.layout.clone(),
            restoring_default: self.restoring_default,
        };
        if redo {
            self.past.push(current);
        } else {
            self.future.push(current);
        }
        self.element_pointers.clear();
        self.pointer_captures
            .borrow_mut()
            .retain(|(_, hit, _)| matches!(hit, Hit::Page | Hit::Dock));
        self.element_owner = None;
        self.element_interaction = None;
        self.handle_interaction = None;
        self.guides.clear();
        self.layout =
            if snapshot.restoring_default { self.geometry.layout.clone() } else { snapshot.layout };
        self.restoring_default = snapshot.restoring_default;
    }

    fn update_element(
        &mut self,
        id: MobileUiElementId,
        transform: MobileUiElementTransform,
        guides: Vec<MobileUiSnapGuide>,
    ) {
        self.guides = guides;
        let next =
            constrain_mobile_ui_transform(transform, self.geometry.sizes[id as usize], self.page);
        if self.layout.get(id) == next {
            return;
        }
        self.remember_change();
        self.layout.set(id, next);
        self.restoring_default = false;
    }

    fn siblings(&self, id: MobileUiElementId) -> Vec<MobileUiSnapRect> {
        let mut siblings = Vec::new();
        for other in MobileUiElementId::ALL {
            if other != id {
                siblings.push(mobile_ui_element_snap_rect(
                    self.layout.get(other),
                    self.geometry.sizes[other as usize],
                    self.page,
                ));
            }
        }
        siblings
    }

    fn nudge(&mut self, direction: Vec2) {
        let step = if self.grid { MOBILE_UI_GRID_SIZE } else { 1.0 };
        let current = self.layout.get(self.selected);
        let candidate = MobileUiElementTransform {
            x: current.x + direction.x * step / self.page.width * 100.0,
            y: current.y + direction.y * step / self.page.height * 100.0,
            ..current
        };
        self.move_to(self.selected, candidate, MOBILE_UI_SNAP_THRESHOLD);
    }

    fn move_to(
        &mut self,
        id: MobileUiElementId,
        candidate: MobileUiElementTransform,
        threshold: f64,
    ) {
        if !self.grid {
            self.update_element(id, candidate, Vec::new());
            return;
        }
        let result = snap_mobile_ui_move(
            candidate,
            self.geometry.sizes[id as usize],
            self.page,
            &self.siblings(id),
            threshold,
        );
        self.update_element(id, result.transform, result.guides);
    }

    fn key_move(&mut self, id: MobileUiElementId, key: &str, shift: bool) {
        let current = self.layout.get(id);
        let step = if shift { MOBILE_UI_GRID_SIZE } else { 1.0 };
        let mut candidate = current;
        match key {
            "ArrowLeft" => candidate.x -= step / self.page.width * 100.0,
            "ArrowRight" => candidate.x += step / self.page.width * 100.0,
            "ArrowUp" => candidate.y -= step / self.page.height * 100.0,
            "ArrowDown" => candidate.y += step / self.page.height * 100.0,
            "[" | "]" => {
                let angle = if key == "[" {
                    -std::f64::consts::PI / 36.0
                } else {
                    std::f64::consts::PI / 36.0
                };
                candidate.rotation = mobile_ui_element_rotation(current.rotation, 0.0, angle, true);
            }
            "-" | "+" => candidate.scale += if key == "-" { -0.05 } else { 0.05 },
            _ => return,
        }
        self.update_element(id, candidate, Vec::new());
    }

    fn element_pointer(
        &mut self,
        phase: PointerPhase,
        id: MobileUiElementId,
        pointer: i32,
        client: Vec2,
    ) {
        match phase {
            PointerPhase::Start => {
                if !self.element_pointers.is_empty() && self.element_owner != Some(id) {
                    return;
                }
                self.guides.clear();
                self.selected = id;
                self.element_owner = Some(id);
                set_pointer(&mut self.element_pointers, pointer, client);
                if self.element_pointers.len() == 1 {
                    self.element_interaction = Some(ElementInteraction::Drag {
                        id,
                        initial_pointer: self.page_percent_point(client),
                        initial: self.layout.get(id),
                    });
                } else if self.element_pointers.len() == 2 {
                    self.element_interaction = Some(ElementInteraction::Pinch {
                        id,
                        initial_distance: distance(
                            self.element_pointers[0].1,
                            self.element_pointers[1].1,
                        ),
                        initial: self.layout.get(id),
                    });
                }
            }
            PointerPhase::Move => {
                if !has_pointer(&self.element_pointers, pointer) {
                    return;
                }
                set_pointer(&mut self.element_pointers, pointer, client);
                match self.element_interaction {
                    Some(ElementInteraction::Drag { id: owner, initial_pointer, initial })
                        if owner == id =>
                    {
                        let current = self.page_percent_point(client);
                        let candidate = MobileUiElementTransform {
                            x: initial.x + current.x - initial_pointer.x,
                            y: initial.y + current.y - initial_pointer.y,
                            ..initial
                        };
                        self.move_to(id, candidate, MOBILE_UI_SNAP_THRESHOLD / self.zoom);
                    }
                    Some(ElementInteraction::Pinch { id: owner, initial_distance, initial })
                        if owner == id && self.element_pointers.len() >= 2 =>
                    {
                        let scale = mobile_ui_element_pinch_scale(
                            initial.scale,
                            initial_distance,
                            distance(self.element_pointers[0].1, self.element_pointers[1].1),
                        );
                        let candidate = MobileUiElementTransform { scale, ..initial };
                        if !self.grid {
                            self.update_element(id, candidate, Vec::new());
                            return;
                        }
                        let result = snap_mobile_ui_scale(
                            candidate,
                            self.geometry.sizes[id as usize],
                            self.page,
                            &self.siblings(id),
                            MOBILE_UI_SNAP_THRESHOLD / self.zoom,
                        );
                        self.update_element(id, result.transform, result.guides);
                    }
                    _ => {}
                }
            }
            PointerPhase::End => {
                if !has_pointer(&self.element_pointers, pointer) {
                    return;
                }
                self.element_pointers.retain(|(id, _)| *id != pointer);
                if self.element_pointers.is_empty() {
                    self.element_owner = None;
                }
                self.element_interaction = None;
                if let Some((_, remaining)) = self.element_pointers.first()
                    && let Some(owner) = self.element_owner
                {
                    self.element_interaction = Some(ElementInteraction::Drag {
                        id: owner,
                        initial_pointer: self.page_percent_point(*remaining),
                        initial: self.layout.get(owner),
                    });
                }
                self.guides.clear();
            }
        }
    }

    fn handle_pointer(
        &mut self,
        phase: PointerPhase,
        id: MobileUiElementId,
        handle: Option<MobileUiResizeHandle>,
        pointer: i32,
        client: Vec2,
    ) {
        match phase {
            PointerPhase::Start => {
                self.guides.clear();
                self.selected = id;
                self.handle_interaction = Some(if let Some(handle) = handle {
                    HandleInteraction::Resize {
                        id,
                        pointer_id: pointer,
                        gesture: MobileUiResizeGesture {
                            initial: self.layout.get(id),
                            natural_size: self.geometry.sizes[id as usize],
                            page: self.page,
                            handle,
                            initial_pointer: self.page_pixel_point(client),
                            current_pointer: self.page_pixel_point(client),
                        },
                    }
                } else {
                    let bounds = self.elements[id as usize].get_bounding_client_rect();
                    let center = Vec2::new(
                        bounds.left() + bounds.width() / 2.0,
                        bounds.top() + bounds.height() / 2.0,
                    );
                    HandleInteraction::Rotate {
                        id,
                        pointer_id: pointer,
                        center,
                        initial_angle: angle(center, client),
                        initial_rotation: self.layout.get(id).rotation,
                    }
                });
            }
            PointerPhase::Move => match self.handle_interaction {
                Some(HandleInteraction::Rotate {
                    id,
                    pointer_id,
                    center,
                    initial_angle,
                    initial_rotation,
                }) if pointer_id == pointer => {
                    let rotation = mobile_ui_element_rotation(
                        initial_rotation,
                        initial_angle,
                        angle(center, client),
                        self.grid,
                    );
                    self.update_element(
                        id,
                        MobileUiElementTransform { rotation, ..self.layout.get(id) },
                        Vec::new(),
                    );
                }
                Some(HandleInteraction::Resize { id, pointer_id, gesture })
                    if pointer_id == pointer =>
                {
                    let gesture = MobileUiResizeGesture {
                        current_pointer: self.page_pixel_point(client),
                        ..gesture
                    };
                    if self.grid {
                        let result = snap_mobile_ui_resize(
                            gesture,
                            &self.siblings(id),
                            MOBILE_UI_SNAP_THRESHOLD / self.zoom,
                        );
                        self.update_element(id, result.transform, result.guides);
                    } else {
                        let transform = mobile_ui_resize_transform(
                            gesture.initial,
                            gesture.natural_size,
                            gesture.page,
                            gesture.handle,
                            gesture.initial_pointer,
                            gesture.current_pointer,
                        );
                        self.update_element(id, transform, Vec::new());
                    }
                }
                _ => {}
            },
            PointerPhase::End => {
                let active = match self.handle_interaction {
                    Some(
                        HandleInteraction::Rotate { pointer_id, .. }
                        | HandleInteraction::Resize { pointer_id, .. },
                    ) => pointer_id == pointer,
                    None => false,
                };
                if active {
                    self.handle_interaction = None;
                    self.guides.clear();
                }
            }
        }
    }

    fn page_pointer(
        &mut self,
        phase: PointerPhase,
        pointer: i32,
        client: Vec2,
    ) -> Result<(), String> {
        match phase {
            PointerPhase::Start => {
                self.guides.clear();
                set_pointer(&mut self.page_pointers, pointer, client);
                if self.page_pointers.len() == 1 {
                    self.page_interaction = Some(PageInteraction::Pan {
                        initial_client: client,
                        initial_scroll: self.scroll_position()?,
                    });
                } else if self.page_pointers.len() == 2 {
                    let midpoint = middle(self.page_pointers[0].1, self.page_pointers[1].1);
                    let bounds = self.viewport.get_bounding_client_rect();
                    let scroll = self.scroll_position()?;
                    let anchor = Vec2::new(
                        (scroll.x + midpoint.x - bounds.left() - 24.0) / self.zoom,
                        (scroll.y + midpoint.y - bounds.top() - 24.0) / self.zoom,
                    );
                    self.page_interaction = Some(PageInteraction::Pinch {
                        anchor,
                        initial_distance: distance(
                            self.page_pointers[0].1,
                            self.page_pointers[1].1,
                        ),
                        initial_zoom: self.zoom,
                    });
                }
            }
            PointerPhase::Move => {
                if !has_pointer(&self.page_pointers, pointer) {
                    return Ok(());
                }
                set_pointer(&mut self.page_pointers, pointer, client);
                match self.page_interaction {
                    Some(PageInteraction::Pan { initial_client, initial_scroll }) => {
                        self.set_scroll_position(Vec2::new(
                            initial_scroll.x - (client.x - initial_client.x),
                            initial_scroll.y - (client.y - initial_client.y),
                        ))?;
                    }
                    Some(PageInteraction::Pinch { anchor, initial_distance, initial_zoom })
                        if self.page_pointers.len() >= 2 =>
                    {
                        let first = self.page_pointers[0].1;
                        let second = self.page_pointers[1].1;
                        let midpoint = middle(first, second);
                        let next = mobile_ui_page_pinch_zoom(
                            initial_zoom,
                            initial_distance,
                            distance(first, second),
                        );
                        let bounds = self.viewport.get_bounding_client_rect();
                        self.zoom = next;
                        self.pending_scroll = Some(Vec2::new(
                            anchor.x * next + 24.0 - (midpoint.x - bounds.left()),
                            anchor.y * next + 24.0 - (midpoint.y - bounds.top()),
                        ));
                    }
                    _ => {}
                }
            }
            PointerPhase::End => {
                if has_pointer(&self.page_pointers, pointer) {
                    self.page_pointers.retain(|(id, _)| *id != pointer);
                    self.page_interaction = None;
                }
            }
        }
        Ok(())
    }

    fn dock_pointer(&mut self, phase: PointerPhase, pointer: i32, client: Vec2) {
        match phase {
            PointerPhase::Start => {
                let bounds = self.dock.get_bounding_client_rect();
                self.dock_interaction = Some(DockInteraction {
                    offset: Vec2::new(client.x - bounds.left(), client.y - bounds.top()),
                    pointer_id: pointer,
                });
            }
            PointerPhase::Move => {
                if let Some(DockInteraction { offset, pointer_id }) = self.dock_interaction
                    && pointer_id == pointer
                {
                    let bounds = self.root.get_bounding_client_rect();
                    self.dock_position = Some(Vec2::new(
                        client.x - bounds.left() - offset.x,
                        client.y - bounds.top() - offset.y,
                    ));
                }
            }
            PointerPhase::End => {
                if self
                    .dock_interaction
                    .as_ref()
                    .is_some_and(|interaction| interaction.pointer_id == pointer)
                {
                    self.dock_interaction = None;
                }
            }
        }
    }

    fn page_percent_point(&self, client: Vec2) -> Vec2 {
        let bounds = self.page_node.get_bounding_client_rect();
        if bounds.width() <= 0.0 || bounds.height() <= 0.0 {
            return Vec2::new(50.0, 50.0);
        }
        Vec2::new(
            (client.x - bounds.left()) / bounds.width() * 100.0,
            (client.y - bounds.top()) / bounds.height() * 100.0,
        )
    }

    fn page_pixel_point(&self, client: Vec2) -> Vec2 {
        let bounds = self.page_node.get_bounding_client_rect();
        if bounds.width() <= 0.0 || bounds.height() <= 0.0 {
            return Vec2::new(self.page.width / 2.0, self.page.height / 2.0);
        }
        Vec2::new(
            (client.x - bounds.left()) / bounds.width() * self.page.width,
            (client.y - bounds.top()) / bounds.height() * self.page.height,
        )
    }

    fn viewport_center(&self) -> Vec2 {
        let bounds = self.viewport.get_bounding_client_rect();
        Vec2::new(bounds.left() + bounds.width() / 2.0, bounds.top() + bounds.height() / 2.0)
    }

    fn zoom_at(&mut self, zoom: f64, client: Vec2) -> Result<(), String> {
        let bounds = self.viewport.get_bounding_client_rect();
        let local = Vec2::new(client.x - bounds.left(), client.y - bounds.top());
        let scroll = self.scroll_position()?;
        let anchor = Vec2::new(
            (scroll.x + local.x - 24.0) / self.zoom,
            (scroll.y + local.y - 24.0) / self.zoom,
        );
        self.zoom = zoom.clamp(MOBILE_UI_PAGE_ZOOM_MIN, MOBILE_UI_PAGE_ZOOM_MAX);
        self.pending_scroll = Some(Vec2::new(
            anchor.x * self.zoom + 24.0 - local.x,
            anchor.y * self.zoom + 24.0 - local.y,
        ));
        Ok(())
    }

    fn scroll_position(&self) -> Result<Vec2, String> {
        let read = |name: &str| {
            js_sys::Reflect::get(&self.viewport, &wasm_bindgen::JsValue::from_str(name))
                .map_err(|_| format!("mobile editor {name}"))?
                .as_f64()
                .ok_or_else(|| format!("mobile editor numeric {name}"))
        };
        Ok(Vec2::new(read("scrollLeft")?, read("scrollTop")?))
    }

    pub(super) fn set_scroll_position(&self, position: Vec2) -> Result<(), String> {
        // CSSOM scroll positions are fractional. The legacy web-sys i32 accessors
        // would truncate a source zoom anchor after every drag or pinch.
        for (name, value) in [("scrollLeft", position.x), ("scrollTop", position.y)] {
            js_sys::Reflect::set(
                &self.viewport,
                &wasm_bindgen::JsValue::from_str(name),
                &wasm_bindgen::JsValue::from_f64(value),
            )
            .map_err(|_| format!("mobile editor {name}"))?;
        }
        Ok(())
    }

    pub(super) fn fit_page(&mut self) -> Result<(), String> {
        if self.fullscreen {
            self.zoom = 1.0;
            self.pending_scroll = Some(Vec2::ZERO);
        } else {
            self.zoom = ((f64::from(self.viewport.client_width()) - 48.0) / self.page.width)
                .min((f64::from(self.viewport.client_height()) - 48.0) / self.page.height)
                .min(1.0)
                .clamp(MOBILE_UI_PAGE_ZOOM_MIN, MOBILE_UI_PAGE_ZOOM_MAX);
            self.pending_scroll = Some(Vec2::new(
                ((self.page.width * self.zoom + 48.0 - f64::from(self.viewport.client_width()))
                    / 2.0)
                    .max(0.0),
                ((self.page.height * self.zoom + 48.0 - f64::from(self.viewport.client_height()))
                    / 2.0)
                    .max(0.0),
            ));
        }
        self.paint()
    }

    pub(super) fn constrain_dock(&mut self) -> Result<(), String> {
        let position = self.dock_position.unwrap_or(Vec2::new(
            (f64::from(self.root.client_width()) - f64::from(self.dock.offset_width())) / 2.0,
            54.0,
        ));
        let position = Vec2::new(
            position.x.clamp(
                8.0,
                (f64::from(self.root.client_width() - self.dock.offset_width()) - 8.0).max(8.0),
            ),
            position.y.clamp(
                8.0,
                (f64::from(self.root.client_height() - self.dock.offset_height()) - 8.0).max(8.0),
            ),
        );
        self.dock_position = Some(position);
        let style = self.dock.style();
        style
            .set_property("left", &format!("{}px", position.x))
            .map_err(|_| "mobile editor action position")?;
        style.set_property("right", "auto").map_err(|_| "mobile editor action right edge")?;
        style
            .set_property("top", &format!("{}px", position.y))
            .map_err(|_| "mobile editor action height")?;
        Ok(())
    }
}

fn has_pointer(pointers: &[(i32, Vec2)], id: i32) -> bool {
    pointers.iter().any(|(pointer, _)| *pointer == id)
}
fn set_pointer(pointers: &mut Vec<(i32, Vec2)>, id: i32, point: Vec2) {
    if let Some((_, current)) = pointers.iter_mut().find(|(pointer, _)| *pointer == id) {
        *current = point;
    } else {
        pointers.push((id, point));
    }
}
fn distance(first: Vec2, second: Vec2) -> f64 {
    hypot(second.x - first.x, second.y - first.y)
}
fn middle(first: Vec2, second: Vec2) -> Vec2 {
    Vec2::new((first.x + second.x) / 2.0, (first.y + second.y) / 2.0)
}
fn angle(center: Vec2, point: Vec2) -> f64 {
    (point.y - center.y).atan2(point.x - center.x)
}
