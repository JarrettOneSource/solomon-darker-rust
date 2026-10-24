//! GPU editor page/chrome from `MobileUiEditor.tsx` and its authored control layout.

use super::preview::ElementPaint;
use super::{EditorEvent, Hit, HitTarget, MobileUiEditor, egui_rect, hud_rect};
use crate::gpu_ui::UiFrame;
use egui::{Color32, FontFamily, FontId, Id, Rect, RichText, Stroke, Ui, pos2, vec2};
use solomon_sim::mobile_ui::*;
use solomon_sim::player_panels::native_tooltip_fixed;

const GOLD: Color32 = Color32::from_rgb(233, 215, 157);
fn copy(value: &str, size: f32) -> RichText {
    RichText::new(value).font(FontId::new(size, FontFamily::Name("Cinzel Bold".into()))).color(GOLD)
}

impl MobileUiEditor {
    pub(super) fn paint_page(
        &mut self,
        frame: &UiFrame<'_>,
        ui: &mut Ui,
        viewport: Rect,
    ) -> Result<(), String> {
        let page = egui_rect(self.page_bounds);
        let clipped = ui.painter().with_clip_rect(viewport.intersect(page));
        clipped.rect_filled(page, 0.0, Color32::from_rgb(184, 189, 193));
        if self.grid {
            let step = (MOBILE_UI_GRID_SIZE * self.zoom) as f32;
            let mut x = page.left();
            while x < page.right() {
                clipped.line_segment(
                    [pos2(x, page.top()), pos2(x, page.bottom())],
                    Stroke::new(1.0, Color32::from_rgba_unmultiplied(57, 65, 70, 56)),
                );
                x += step;
            }
            let mut y = page.top();
            while y < page.bottom() {
                clipped.line_segment(
                    [pos2(page.left(), y), pos2(page.right(), y)],
                    Stroke::new(1.0, Color32::from_rgba_unmultiplied(57, 65, 70, 56)),
                );
                y += step;
            }
        }
        let mut order: Vec<_> =
            MobileUiElementId::ALL.into_iter().filter(|id| *id != self.selected).collect();
        order.push(self.selected);
        for id in order {
            if id == self.selected {
                for guide in &self.guides {
                    let color = match guide.kind {
                        MobileUiSnapKind::Element => Color32::from_rgb(255, 207, 74),
                        MobileUiSnapKind::Page => Color32::from_rgb(255, 122, 217),
                        _ => Color32::from_rgb(0, 167, 237),
                    };
                    let coordinate = guide.position as f32 * self.zoom as f32;
                    let points = if guide.axis == MobileUiSnapAxis::X {
                        [
                            page.min + vec2(coordinate, 0.0),
                            page.left_bottom() + vec2(coordinate, 0.0),
                        ]
                    } else {
                        [page.min + vec2(0.0, coordinate), page.right_top() + vec2(0.0, coordinate)]
                    };
                    clipped.line_segment(points, Stroke::new(1.0, color));
                }
            }
            let transform = self.layout.get(id);
            let natural = self.geometry.sizes[id as usize];
            let center = page.min
                + vec2(
                    (transform.x / 100.0 * self.page.width * self.zoom) as f32,
                    (transform.y / 100.0 * self.page.height * self.zoom) as f32,
                );
            let scale = (transform.scale * self.zoom) as f32;
            let rotation = transform.rotation.to_radians() as f32;
            let paint = ElementPaint {
                center,
                natural: vec2(natural.width as f32, natural.height as f32),
                scale,
                rotation,
            };
            let corners = paint.quad(Rect::from_min_size(egui::Pos2::ZERO, paint.natural));
            let bounds = Rect::from_points(&corners);
            self.elements[id as usize] = hud_rect(bounds);
            let target =
                HitTarget { hit: Hit::Element(id), center, size: paint.natural * scale, rotation };
            self.hit_targets.push(target);
            let visible = bounds.intersect(viewport);
            let response = ui.interact(
                visible,
                Id::unique(("mobile/control", id.as_str())),
                if self.busy { egui::Sense::hover() } else { egui::Sense::click_and_drag() },
            );
            if self.pending_element_focus == Some(id) && response.enabled() && !ui.is_sizing_pass()
            {
                response.request_focus();
                self.pending_element_focus = None;
            }
            if response.has_focus() {
                ui.memory_mut(|memory| {
                    memory.set_focus_lock_filter(
                        response.id,
                        egui::EventFilter {
                            horizontal_arrows: true,
                            vertical_arrows: true,
                            ..Default::default()
                        },
                    )
                });
            }
            if response.gained_focus() {
                self.focused_element = Some(id);
                self.events.push(EditorEvent::Select(id));
            }
            self.paint_preview(frame, &clipped, id, paint)?;
            let label = format!(
                "{}, x {} percent, y {} percent, scale {}, rotation {} degrees",
                id.label(),
                native_tooltip_fixed(transform.x, 1),
                native_tooltip_fixed(transform.y, 1),
                native_tooltip_fixed(transform.scale, 2),
                native_tooltip_fixed(transform.rotation, 0)
            );
            frame.record(&format!("mobile/control:{}", id.as_str()), &label, "button", &response);
            frame.record_value(
                &format!("mobile/control:{}", id.as_str()),
                &format!(
                    "{},{},{},{}",
                    transform.x, transform.y, transform.scale, transform.rotation
                ),
                Some(self.selected == id),
            );
            if self.selected == id {
                let outline = paint.quad(Rect::from_min_max(
                    pos2(-4.0, -4.0),
                    (paint.natural + vec2(4.0, 4.0)).to_pos2(),
                ));
                clipped.add(egui::Shape::closed_line(
                    outline.to_vec(),
                    Stroke::new(2.0, Color32::from_rgb(10, 126, 219)),
                ));
                for (handle, direction) in MobileUiResizeHandle::ALL.into_iter().zip([
                    vec2(-1.0, -1.0),
                    vec2(0.0, -1.0),
                    vec2(1.0, -1.0),
                    vec2(1.0, 0.0),
                    vec2(1.0, 1.0),
                    vec2(0.0, 1.0),
                    vec2(-1.0, 1.0),
                    vec2(-1.0, 0.0),
                ]) {
                    let point = paint.point(
                        (paint.natural / 2.0 + direction * (paint.natural / 2.0 + vec2(4.0, 4.0)))
                            .to_pos2(),
                    );
                    let radius = 6.0 * scale;
                    clipped.circle_filled(point, radius, Color32::from_rgb(10, 126, 219));
                    clipped.circle_stroke(point, radius, Stroke::new(2.0 * scale, Color32::WHITE));
                    let rect = Rect::from_center_size(point, vec2(radius * 2.0, radius * 2.0));
                    self.hit_targets.push(HitTarget {
                        hit: Hit::Resize(id, handle),
                        center: point,
                        size: rect.size(),
                        rotation: 0.0,
                    });
                    frame.record_painted_rect(
                        &clipped,
                        &format!("mobile/resize:{}:{}", id.as_str(), handle.as_str()),
                        &format!("Resize {} from {}", id.label(), handle.as_str()),
                        "button",
                        rect,
                        !self.busy && ui.is_enabled(),
                    );
                }
                let point = paint.point(pos2(paint.natural.x / 2.0, -34.0));
                let bottom = paint.point(pos2(paint.natural.x / 2.0, -4.0));
                clipped.line_segment(
                    [point, bottom],
                    Stroke::new(2.0 * scale, Color32::from_rgb(10, 126, 219)),
                );
                clipped.circle_filled(point, 8.0 * scale, Color32::from_rgb(245, 179, 62));
                clipped.circle_stroke(point, 8.0 * scale, Stroke::new(2.0 * scale, Color32::WHITE));
                let rect = Rect::from_center_size(point, vec2(16.0, 16.0) * scale);
                self.hit_targets.push(HitTarget {
                    hit: Hit::Rotate(id),
                    center: point,
                    size: rect.size(),
                    rotation: 0.0,
                });
                frame.record_painted_rect(
                    &clipped,
                    &format!("mobile/rotate:{}", id.as_str()),
                    &format!("Rotate {}", id.label()),
                    "button",
                    rect,
                    !self.busy && ui.is_enabled(),
                );
            }
        }
        frame.record_rect(
            "mobile/page",
            &format!("Mobile HUD layout page, {} by {}", self.page.width, self.page.height),
            "application",
            page.intersect(viewport),
            !self.busy,
        );
        Ok(())
    }
    pub(super) fn paint_toolbar(&mut self, frame: &UiFrame<'_>, ui: &mut Ui, bounds: Rect) {
        self.hit_targets.push(HitTarget {
            hit: Hit::Chrome,
            center: bounds.center(),
            size: bounds.size(),
            rotation: 0.0,
        });
        ui.scope_builder(egui::UiBuilder::new().max_rect(bounds.shrink(4.0)), |ui| {
            ui.horizontal_wrapped(|ui| {
                self.picker(frame, ui);
                self.command_button(frame, ui, "UNDO", "undo");
                self.command_button(frame, ui, "REDO", "redo");
                self.command_button(
                    frame,
                    ui,
                    if self.grid { "GRID ON" } else { "GRID OFF" },
                    "grid",
                );
                self.command_button(frame, ui, "−", "zoom-out");
                ui.label(copy(&format!("{}%", (self.zoom * 100.0).round()), 11.0));
                self.command_button(frame, ui, "+", "zoom-in");
                self.command_button(frame, ui, "FIT", "fit");
                self.command_button(frame, ui, "RESET LAYOUT", "reset-layout");
            });
        });
    }
    pub(super) fn paint_dock(
        &mut self,
        frame: &UiFrame<'_>,
        ui: &mut Ui,
        bounds: Rect,
    ) -> Result<(), String> {
        self.constrain_dock()?;
        let position = self.dock_position.expect("constrained dock");
        let width = 344.0_f32.min(bounds.width() - 16.0);
        let height = if self.tools_open { 400.0_f32.min(bounds.height() - 16.0) } else { 126.0 };
        let dock = Rect::from_min_size(
            bounds.min + vec2(position.x as f32, position.y as f32),
            vec2(width, height),
        );
        self.dock_bounds = hud_rect(dock);
        ui.painter().rect_filled(dock, 10.0, Color32::from_rgba_unmultiplied(20, 19, 16, 247));
        ui.painter().rect_stroke(
            dock,
            10.0,
            Stroke::new(1.0, Color32::from_rgb(198, 168, 95)),
            egui::StrokeKind::Inside,
        );
        self.hit_targets.push(HitTarget {
            hit: Hit::Chrome,
            center: dock.center(),
            size: dock.size(),
            rotation: 0.0,
        });
        let handle = Rect::from_min_size(dock.min, vec2(44.0, 44.0));
        self.hit_targets.push(HitTarget {
            hit: Hit::Dock,
            center: handle.center(),
            size: handle.size(),
            rotation: 0.0,
        });
        ui.interact(handle, Id::unique("mobile/dock-handle"), egui::Sense::drag());
        ui.painter().text(
            handle.center(),
            egui::Align2::CENTER_CENTER,
            "⠿",
            FontId::new(24.0, FontFamily::Name("Georgia".into())),
            GOLD,
        );
        frame.record_painted_rect(
            ui.painter(),
            "mobile/dock-handle",
            "Move editor actions",
            "button",
            handle,
            !self.busy && ui.is_enabled(),
        );
        ui.painter().text(
            dock.min + vec2(50.0, 22.0),
            egui::Align2::LEFT_CENTER,
            "Mobile controls",
            FontId::new(15.0, FontFamily::Name("Cinzel Bold".into())),
            GOLD,
        );
        let save_rect = Rect::from_min_size(dock.right_top() - vec2(72.0, 0.0), vec2(72.0, 44.0));
        let save = ui.put(
            save_rect,
            egui::Button::new(copy("SAVE", 12.0)).fill(Color32::from_rgb(46, 40, 23)),
        );
        frame.record("mobile/save", "SAVE", "button", &save);
        if save.clicked() && !self.busy {
            self.save_request = true;
        }
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(Rect::from_min_size(dock.min + vec2(4.0, 46.0), vec2(width - 8.0, 44.0))),
            |ui| {
                ui.horizontal(|ui| {
                    self.command_button(frame, ui, "UNDO", "undo");
                    self.command_button(frame, ui, "REDO", "redo");
                    self.command_button(
                        frame,
                        ui,
                        if self.grid { "GRID ON" } else { "GRID OFF" },
                        "grid",
                    );
                    self.command_button(
                        frame,
                        ui,
                        if self.tools_open { "ADJUST −" } else { "ADJUST +" },
                        "tools",
                    );
                });
            },
        );
        if self.tools_open {
            self.paint_inspector(
                frame,
                ui,
                Rect::from_min_max(dock.min + vec2(12.0, 96.0), dock.max - vec2(12.0, 10.0)),
            );
        } else {
            ui.painter().text(
                dock.min + vec2(12.0, 108.0),
                egui::Align2::LEFT_CENTER,
                "Drag to move. Pinch to resize.",
                FontId::new(13.0, FontFamily::Name("Cinzel".into())),
                Color32::from_rgb(207, 198, 173),
            );
        }
        Ok(())
    }
    pub(super) fn paint_inspector(&mut self, frame: &UiFrame<'_>, ui: &mut Ui, bounds: Rect) {
        self.hit_targets.push(HitTarget {
            hit: Hit::Chrome,
            center: bounds.center(),
            size: bounds.size(),
            rotation: 0.0,
        });
        ui.scope_builder(egui::UiBuilder::new().max_rect(bounds.shrink(4.0)), |ui| {
            egui::ScrollArea::vertical()
                .id_salt("mobile/inspector-scroll")
                .max_height(bounds.height())
                .show(ui, |ui| {
                    if self.fullscreen {
                        self.picker(frame, ui);
                    }
                    let current = self.layout.get(self.selected);
                    ui.horizontal_wrapped(|ui| {
                        ui.label(copy("Move", 11.0));
                        for (label, action) in
                            [("←", "left"), ("↑", "up"), ("↓", "down"), ("→", "right")]
                        {
                            self.command_button(frame, ui, label, action);
                        }
                    });
                    ui.horizontal_wrapped(|ui| {
                        ui.label(copy(&format!("Size {}%", (current.scale * 100.0).round()), 11.0));
                        self.command_button(frame, ui, "−", "smaller");
                        let mut scale = current.scale * 100.0;
                        let response = ui.add_enabled(
                            !self.busy,
                            egui::Slider::new(&mut scale, 40.0..=300.0)
                                .step_by(5.0)
                                .show_value(false),
                        );
                        frame.record("mobile/scale", "Size", "slider", &response);
                        frame.record_value("mobile/scale", &scale.to_string(), None);
                        if response.drag_started() {
                            self.events.push(EditorEvent::BeginChange);
                        }
                        if response.changed() {
                            self.events.push(EditorEvent::Scale(scale / 100.0));
                        }
                        self.command_button(frame, ui, "+", "larger");
                    });
                    ui.horizontal_wrapped(|ui| {
                        ui.label(copy(&format!("Rotation {}°", current.rotation.round()), 11.0));
                        for (label, action) in [
                            ("−15°", "rotate-left"),
                            ("0°", "straighten"),
                            ("+15°", "rotate-right"),
                        ] {
                            self.command_button(frame, ui, label, action);
                        }
                    });
                    ui.horizontal_wrapped(|ui| {
                        self.command_button(frame, ui, "RESET THIS CONTROL", "reset-control");
                        if self.fullscreen {
                            self.command_button(frame, ui, "RESET LAYOUT", "reset-layout");
                        }
                    });
                    let status = format!(
                        "{}   X {}%   Y {}%   SIZE {}%   ROTATE {}°",
                        self.selected.label(),
                        native_tooltip_fixed(current.x, 1),
                        native_tooltip_fixed(current.y, 1),
                        (current.scale * 100.0).round(),
                        current.rotation.round()
                    );
                    let response = ui.label(copy(&status, 11.0));
                    frame.record("mobile/selection", &status, "status", &response);
                    ui.label(copy(
                        if self.fullscreen {
                            "Changes apply when you save. Undo also restores a reset."
                        } else {
                            "GRID snaps nearby edges and centres. Drag to move; pinch or use a node to resize."
                        },
                        10.0,
                    ));
                });
        });
    }
    fn picker(&mut self, frame: &UiFrame<'_>, ui: &mut Ui) {
        let mut selected = self.selected;
        let result = egui::ComboBox::from_id_salt("mobile/control-picker")
            .selected_text(copy(selected.label(), 12.0))
            .show_ui(ui, |ui| {
                for id in MobileUiElementId::ALL {
                    let response = ui.selectable_value(&mut selected, id, copy(id.label(), 12.0));
                    frame.record(
                        &format!("mobile/select:{}", id.as_str()),
                        id.label(),
                        "option",
                        &response,
                    );
                }
            });
        frame.record(
            "mobile/control-picker",
            "Selected mobile UI element",
            "combobox",
            &result.response,
        );
        frame.record_value("mobile/control-picker", self.selected.as_str(), None);
        if selected != self.selected {
            self.events.push(EditorEvent::Select(selected));
            self.focused_element = None;
        }
    }
    fn command_button(&mut self, frame: &UiFrame<'_>, ui: &mut Ui, label: &str, action: &str) {
        let disabled = self.busy
            || match action {
                "undo" => self.past.is_empty(),
                "redo" => self.future.is_empty(),
                "smaller" => self.layout.get(self.selected).scale <= MOBILE_UI_SCALE_MIN,
                "larger" => self.layout.get(self.selected).scale >= MOBILE_UI_SCALE_MAX,
                "straighten" => self.layout.get(self.selected).rotation == 0.0,
                "reset-layout" => self.restoring_default,
                "zoom-out" => self.zoom <= MOBILE_UI_PAGE_ZOOM_MIN,
                "zoom-in" => self.zoom >= MOBILE_UI_PAGE_ZOOM_MAX,
                _ => false,
            };
        let response = ui.add_enabled(
            !disabled,
            egui::Button::new(copy(label, 11.0))
                .fill(Color32::from_rgb(23, 20, 14))
                .stroke(Stroke::new(1.0, Color32::from_rgb(130, 114, 72)))
                .min_size(vec2(30.0, if self.fullscreen { 38.0 } else { 30.0 })),
        );
        frame.record(&format!("mobile/{action}"), label, "button", &response);
        if response.clicked() {
            self.focused_element = None;
            self.events.push(EditorEvent::BeginChange);
            self.events.push(EditorEvent::Command(action.to_owned()));
        }
    }
}
