//! OS events enter the same ordered editor receipts and UI passes as browser input.

mod accessibility;
mod editing;

use std::sync::Arc;

use winit::event::{ElementState, WindowEvent};
use winit::keyboard::{Key as WindowKey, NamedKey, PhysicalKey};
use winit::window::Window;

use super::super::edit::model::EditGroup;
use super::*;

pub(in crate::gpu_ui) struct NativeInput {
    window: Arc<Window>,
    ctx: egui::Context,
    state: egui_winit::State,
    queue: InputQueue,
    modifiers: Modifiers,
    editing: editing::Editing,
    last_click: Option<(f64, Pos2, u32)>,
    pointer_origin: Option<Pos2>,
    touch_owners: std::collections::HashSet<u64>,
}

impl NativeInput {
    pub fn attach(surface: &crate::platform::Surface, ctx: &egui::Context) -> Result<Self, String> {
        let window = surface.window().clone();
        let mut state = egui_winit::State::new(
            ctx.clone(),
            egui::ViewportId::ROOT,
            window.as_ref(),
            Some(window.scale_factor() as f32),
            window.theme(),
            None,
        );
        state.set_clipboard_shortcuts(false);
        Ok(Self {
            queue: InputQueue { focused: window.has_focus(), ..Default::default() },
            window,
            ctx: ctx.clone(),
            state,
            modifiers: Modifiers::NONE,
            editing: editing::Editing::default(),
            last_click: None,
            pointer_origin: None,
            touch_owners: Default::default(),
        })
    }

    pub fn init_accesskit<T: From<egui_winit::accesskit_winit::Event> + Send>(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        proxy: winit::event_loop::EventLoopProxy<T>,
    ) {
        self.state.init_accesskit(event_loop, &self.window, proxy);
    }

    pub fn accesskit_action(
        &mut self,
        request: egui::accesskit::ActionRequest,
    ) -> Result<(), String> {
        if accessibility::edit_action(&mut self.queue, &self.ctx, &request) {
            self.queue.finish_dispatch();
            return Ok(());
        }
        self.state.on_accesskit_action_request(request);
        self.collect_events()
    }

    /// The window owner commits the previous dispatch before calling this method.
    /// No callbacks reenter a borrowed game or its GPU resource owner.
    pub fn window_event(&mut self, event: &WindowEvent) -> Result<(), String> {
        if let WindowEvent::Touch(touch) = event {
            crate::platform::set_coarse_pointer(true);
            if touch.phase == winit::event::TouchPhase::Started {
                let pos = pos2(touch.location.x as f32, touch.location.y as f32)
                    / self.ctx.pixels_per_point();
                if self.queue.scope.is_some()
                    || self.queue.hit_regions.iter().any(|region| region.contains(pos))
                {
                    self.touch_owners.insert(touch.id);
                }
            }
            if !self.touch_owners.contains(&touch.id) {
                return Ok(());
            }
            if matches!(
                touch.phase,
                winit::event::TouchPhase::Ended | winit::event::TouchPhase::Cancelled
            ) {
                self.touch_owners.remove(&touch.id);
            }
        } else if matches!(event, WindowEvent::CursorMoved { .. }) {
            crate::platform::set_coarse_pointer(false);
        }
        if let WindowEvent::ModifiersChanged(modifiers) = event {
            let mods = modifiers.state();
            self.modifiers = Modifiers {
                alt: mods.alt_key(),
                ctrl: mods.control_key(),
                shift: mods.shift_key(),
                mac_cmd: mods.super_key(),
                command: if cfg!(target_os = "macos") {
                    mods.super_key()
                } else {
                    mods.control_key()
                },
            };
        }
        if let WindowEvent::KeyboardInput { event, .. } = event
            && !self.queue.composing
        {
            let stroke = KeyStroke {
                code: match event.physical_key {
                    PhysicalKey::Code(winit::keyboard::KeyCode::SuperLeft) => "MetaLeft".into(),
                    PhysicalKey::Code(winit::keyboard::KeyCode::SuperRight) => "MetaRight".into(),
                    PhysicalKey::Code(code) => format!("{code:?}"),
                    PhysicalKey::Unidentified(_) => String::new(),
                },
                key: match &event.logical_key {
                    WindowKey::Character(text) => text.to_string(),
                    WindowKey::Named(NamedKey::Space) => " ".into(),
                    WindowKey::Named(key) => format!("{key:?}"),
                    WindowKey::Dead(_) | WindowKey::Unidentified(_) => String::new(),
                },
                pressed: event.state == ElementState::Pressed,
                repeat: event.repeat,
                alt: self.modifiers.alt,
                ctrl: self.modifiers.ctrl,
                shift: self.modifiers.shift,
                meta: self.modifiers.mac_cmd,
            };
            if crate::platform::dispatch_keyboard(
                self.window.id(),
                &stroke,
                self.queue.capture.text,
            ) {
                return Ok(());
            }
            self.queue.codes.push(stroke);
        }
        let _ = self.state.on_window_event(&self.window, event);
        self.collect_events()
    }

    fn collect_events(&mut self) -> Result<(), String> {
        let raw = self.state.take_egui_input(&self.window);
        self.queue.focused = raw.focused;
        for event in raw.events {
            self.event(event)?;
        }
        self.queue.finish_dispatch();
        Ok(())
    }

    fn event(&mut self, event: UiEvent) -> Result<(), String> {
        match event {
            UiEvent::Text(text) => {
                if !self.queue.composing {
                    self.insert(text, Some(EditGroup::Typing));
                }
            }
            UiEvent::Paste(text) => self.insert(text, None),
            UiEvent::Copy => self.copy(false)?,
            UiEvent::Cut => self.copy(true)?,
            UiEvent::Ime(event) => editing::ime(&mut self.queue, event),
            UiEvent::Key { key, pressed, modifiers, .. } => {
                if self.queue.composing {
                    return Ok(());
                }
                let mut consumed = false;
                if pressed && self.queue.session.is_some() {
                    if modifiers.command && matches!(key, Key::C | Key::X | Key::V) {
                        if key == Key::V {
                            let text = crate::platform::clipboard_text()?;
                            self.insert(text, None);
                        } else {
                            self.copy(key == Key::X)?;
                        }
                        consumed = true;
                    } else {
                        consumed = self.editing.key(&mut self.queue, key, modifiers)?;
                    }
                }
                // Physical editing arrows stay out of spatial menu navigation.
                if !consumed
                    && !(self.queue.session.is_some()
                        && matches!(
                            key,
                            Key::ArrowLeft | Key::ArrowRight | Key::ArrowUp | Key::ArrowDown
                        ))
                {
                    self.queue.events.push(event);
                }
            }
            UiEvent::PointerButton { pos, button, pressed, modifiers } => {
                if button == egui::PointerButton::Primary {
                    if pressed {
                        self.pointer_origin = Some(pos);
                        if self.queue.scope.is_some()
                            || self.queue.hit_regions.iter().any(|region| region.contains(pos))
                        {
                            self.queue.owned_pointers.insert(0);
                        }
                        self.pointer_start(pos, modifiers.shift)?;
                    } else {
                        self.queue.owned_pointers.remove(&0);
                        if let Some(session) = &mut self.queue.session {
                            session.pointer = None;
                        }
                    }
                }
                self.queue.events.push(event);
                self.queue.finish_dispatch();
                if button == egui::PointerButton::Primary && !pressed {
                    self.click(pos)?;
                }
            }
            UiEvent::PointerMoved(pos) => {
                if let Some(session) =
                    self.queue.session.as_mut().filter(|session| session.pointer.is_some())
                {
                    let (intent, caret) = session.pointer(pos, true)?;
                    self.queue.edit(intent, Some(caret));
                    self.editing.pointer(&self.queue, caret)?;
                }
                self.queue.events.push(event);
            }
            UiEvent::Touch { id, phase, .. } => {
                crate::platform::set_coarse_pointer(true);
                if phase == TouchPhase::Start && self.queue.primary_touch.is_none() {
                    self.queue.primary_touch = Some(id.0);
                }
                if matches!(phase, TouchPhase::Cancel | TouchPhase::End)
                    && self.queue.primary_touch == Some(id.0)
                {
                    self.queue.primary_touch = None;
                    if phase == TouchPhase::Cancel {
                        self.cancel_pointer();
                    }
                }
                self.queue.events.push(event);
            }
            UiEvent::WindowFocused(false) | UiEvent::PointerCancelled => {
                self.touch_owners.clear();
                self.cancel_pointer();
                if self.queue.composing {
                    self.queue.edit(Intent::CompositionEnd { cancelled: true }, None);
                    self.queue.composing = false;
                }
                self.queue.events.push(event);
            }
            _ => self.queue.events.push(event),
        }
        Ok(())
    }

    fn cancel_pointer(&mut self) {
        self.queue.owned_pointers.clear();
        self.queue.primary_touch = None;
        self.pointer_origin = None;
        self.last_click = None;
        if let Some(session) = &mut self.queue.session {
            session.pointer = None;
        }
        self.queue.events.push(UiEvent::PointerCancelled);
        self.queue.events.push(UiEvent::PointerGone);
    }

    fn pointer_start(&mut self, pos: Pos2, extend: bool) -> Result<(), String> {
        let target = self.ctx.pointer_widget_at(pos);
        let descriptor = self
            .queue
            .editors
            .iter()
            .rev()
            .find(|editor| {
                editor.enabled
                    && editor.bounds.contains(pos)
                    && target == Some(editor.owner.id)
                    && self.ctx.memory(|memory| memory.allows_interaction(editor.layer_id))
            })
            .cloned();
        if let Some(descriptor) = descriptor {
            if self
                .queue
                .session
                .as_ref()
                .is_none_or(|session| session.descriptor.owner != descriptor.owner)
            {
                self.queue.session = Some(Session::new(descriptor));
            }
            let session = self.queue.session.as_mut().expect("actual editor hit");
            session.pointer = Some(0);
            let (intent, caret) = session.pointer(pos, extend)?;
            self.queue.edit(intent, Some(caret));
            self.editing.pointer(&self.queue, caret)?;
        } else {
            self.queue.session = None;
        }
        Ok(())
    }

    fn click(&mut self, pos: Pos2) -> Result<(), String> {
        let options = self.ctx.options(|options| options.input_options);
        if !self
            .pointer_origin
            .take()
            .is_some_and(|origin| origin.distance(pos) <= options.max_click_dist)
        {
            self.last_click = None;
            return Ok(());
        }
        let now = crate::platform::monotonic_ms() / 1000.0;
        let count = self
            .last_click
            .filter(|(time, point, _)| {
                now - *time <= options.max_double_click_delay
                    && point.distance(pos) <= options.max_click_dist
            })
            .map_or(1, |(_, _, count)| count + 1);
        self.last_click = Some((now, pos, count));
        self.queue.record_click(pos, count as i32);
        if count >= 2
            && let Some(session) = self
                .queue
                .session
                .as_ref()
                .filter(|session| session.descriptor.bounds.contains(pos))
        {
            self.queue.edit(session.click_selection(pos, count as i32)?, None);
        }
        Ok(())
    }

    fn insert(&mut self, text: String, group: Option<EditGroup>) {
        let range = self
            .queue
            .session
            .as_ref()
            .map_or(TextSelection::caret(0), |session| session.model.selection());
        self.queue.edit(Intent::Replace { range, text, group }, None);
    }

    fn copy(&mut self, cut: bool) -> Result<(), String> {
        let Some(session) = self.queue.session.as_ref() else { return Ok(()) };
        let text = super::super::edit::model::slice_utf16(
            session.model.text(),
            session.model.selection().range(),
        );
        if text.is_empty() {
            return Ok(());
        }
        crate::platform::set_clipboard_text(text)?;
        self.queue.edit(if cut { Intent::Cut } else { Intent::Copy }, None);
        self.queue.edits.last_mut().expect("recorded clipboard receipt").clipboard_handled = true;
        Ok(())
    }

    pub fn take(
        &mut self,
        viewport: [f32; 2],
        pixels: f32,
        time: f64,
    ) -> Result<Vec<InputPass>, String> {
        self.take_input(viewport, pixels, time, true)
    }

    pub fn take_native(
        &mut self,
        viewport: [f32; 2],
        pixels: f32,
        time: f64,
    ) -> Result<Option<Vec<InputPass>>, String> {
        if self.queue.has_native_input() {
            self.take_input(viewport, pixels, time, false).map(Some)
        } else {
            Ok(None)
        }
    }

    fn take_input(
        &mut self,
        viewport: [f32; 2],
        pixels: f32,
        time: f64,
        sample_gamepad: bool,
    ) -> Result<Vec<InputPass>, String> {
        if let Some(error) = self.queue.clipboard_error.take() {
            return Err(error);
        }
        self.queue.logical = viewport;
        let mut raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(viewport[0], viewport[1]))),
            time: Some(time),
            focused: self.queue.focused,
            events: std::mem::take(&mut self.queue.events),
            ..Default::default()
        };
        raw.viewports.entry(egui::ViewportId::ROOT).or_default().native_pixels_per_point =
            Some(pixels);
        let mut dispatches = std::mem::take(&mut self.queue.dispatches);
        let codes = std::mem::take(&mut self.queue.codes);
        let edits = std::mem::take(&mut self.queue.edits);
        if sample_gamepad {
            let current = if self.queue.scope.is_some() {
                crate::platform::menu_gamepad()?
            } else {
                Default::default()
            };
            self.queue.append_gamepad(
                current,
                &mut raw,
                &mut dispatches,
                codes.len(),
                edits.len(),
                time,
            );
        }
        Ok(into_passes(raw, codes, edits, dispatches))
    }

    pub fn set_editors(
        &mut self,
        editors: Vec<EditorDescriptor>,
        acknowledged: Option<u64>,
    ) -> Result<(), String> {
        if let Some(sequence) = acknowledged {
            self.queue.acknowledged_edit_sequence =
                self.queue.acknowledged_edit_sequence.max(sequence);
        }
        let focused = editors.iter().find(|editor| editor.focused && editor.enabled).cloned();
        super::super::edit::session::reconcile_session(
            &mut self.queue.session,
            focused,
            self.queue.acknowledged_edit_sequence,
        );
        self.queue.editors = editors;
        Ok(())
    }

    pub fn set_widgets(&mut self, widgets: &std::collections::BTreeMap<String, WidgetGeometry>) {
        self.queue.hit_regions = widgets
            .values()
            .filter(|widget| widget.is_interactive())
            .map(WidgetGeometry::hit_region)
            .collect();
    }
    pub fn owns_pointer(&self) -> bool {
        !self.queue.owned_pointers.is_empty() || !self.touch_owners.is_empty()
    }
    pub fn set_scope(&mut self, scope: Option<&str>) {
        self.queue.set_scope(scope);
    }

    pub fn apply_output(
        &mut self,
        output: &egui::PlatformOutput,
        capture: UiCapture,
    ) -> Result<(), String> {
        self.queue.capture = capture;
        let mut output = output.clone();
        for command in &output.commands {
            if let egui::OutputCommand::CopyText(text) = command {
                crate::platform::set_clipboard_text(text)?;
            }
        }
        output.commands.retain(|command| !matches!(command, egui::OutputCommand::CopyText(_)));
        self.state.handle_platform_output(&self.window, output);
        Ok(())
    }
}
