//! `DarkCloudLayouts.tsx` retains actual saved-layout admission and completion.
use super::{
    super::widgets,
    art::CloudArt,
    rows::{control_action, ink},
};
use crate::{gpu_ui::UiFrame, hosted::SharedMobileUiLayout, menu::HostedMenuAction};
use egui::{Rect, Ui, Vec2};
use solomon_sim::native_ui_text::{NativeUiFontName, NativeUiTextSpec, layout_native_ui_text};
use std::cell::RefCell;
use std::rc::Rc;
#[derive(Clone, Copy)]
enum LayoutAction {
    Load,
    Publish,
}
struct CopyCompletion {
    generation: u64,
    result: Result<(), String>,
}
pub(super) struct LayoutsView {
    account: Option<String>,
    customized: bool,
    input: String,
    pending: Option<(u64, LayoutAction)>,
    shared: Option<(LayoutAction, SharedMobileUiLayout)>,
    actions: Vec<HostedMenuAction>,
    error: Option<String>,
    copied: bool,
    copy_result: Rc<RefCell<Option<CopyCompletion>>>,
    copy_generation: u64,
}
impl LayoutsView {
    pub fn new() -> Self {
        Self {
            account: None,
            customized: false,
            input: String::new(),
            pending: None,
            shared: None,
            actions: Vec::new(),
            error: None,
            copied: false,
            copy_result: Rc::new(RefCell::new(None)),
            copy_generation: 0,
        }
    }
    pub fn set_account(&mut self, account: Option<&str>) -> Result<(), String> {
        self.account = account.map(str::to_owned);
        Ok(())
    }
    pub fn set_customized(&mut self, value: bool) -> Result<(), String> {
        self.customized = value;
        Ok(())
    }
    pub fn close(&mut self) {
        self.pending = None;
        self.shared = None;
        self.actions.clear();
        self.copy_generation += 1;
        self.copy_result.borrow_mut().take();
        self.input.clear();
        self.error = None;
        self.copied = false;
    }
    pub fn open(&mut self) -> Result<(), String> {
        self.close();
        Ok(())
    }
    pub fn activate(&mut self, action: &str, next: &mut u64) -> Result<bool, String> {
        if action == "cloud-copy-layout" {
            if let Some((_, layout)) = &self.shared {
                let request = crate::platform::copy_text(&layout.code);
                let generation = self.copy_generation;
                let result = self.copy_result.clone();
                crate::platform::spawn_local(async move {
                    *result.borrow_mut() = Some(CopyCompletion {
                        generation,
                        result: match request {
                            Ok(request) => request.await,
                            Err(error) => Err(error),
                        },
                    });
                });
            }
            return Ok(true);
        }
        let action = match action {
            "cloud-layout-load" => LayoutAction::Load,
            "cloud-layout-publish" => LayoutAction::Publish,
            _ => return Ok(false),
        };
        if self.pending.is_some()
            || matches!(action, LayoutAction::Load) && self.input.len() != 9
            || matches!(action, LayoutAction::Publish)
                && (self.account.is_none() || !self.customized)
        {
            return Ok(true);
        }
        *next += 1;
        self.pending = Some((*next, action));
        self.shared = None;
        self.copy_generation += 1;
        self.copied = false;
        self.error = None;
        self.actions.push(match action {
            LayoutAction::Load => {
                HostedMenuAction::LoadMobileLayout { request_id: *next, code: self.input.clone() }
            }
            LayoutAction::Publish => HostedMenuAction::PublishMobileLayout { request_id: *next },
        });
        Ok(true)
    }

    pub fn complete(
        &mut self,
        id: u64,
        result: Result<SharedMobileUiLayout, String>,
    ) -> Result<bool, String> {
        let Some((pending, action)) = self.pending else { return Ok(false) };
        if pending != id {
            return Ok(false);
        }
        self.pending = None;
        match result {
            Ok(layout) => {
                self.input = layout.code.clone();
                self.shared = Some((action, layout));
            }
            Err(error) => self.error = Some(error),
        }
        Ok(true)
    }
    pub fn update(&mut self) -> Result<(), String> {
        if let Some(completion) = self.copy_result.borrow_mut().take()
            && completion.generation == self.copy_generation
        {
            match completion.result {
                Ok(()) => {
                    self.copied = true;
                    self.error = None;
                }
                Err(error) => self.error = Some(error),
            }
        }
        Ok(())
    }
    pub fn take_actions(&mut self) -> Vec<HostedMenuAction> {
        std::mem::take(&mut self.actions)
    }
    pub fn paint(
        &mut self,
        frame: &mut UiFrame<'_>,
        ui: &mut Ui,
        art: &CloudArt,
        rect: Rect,
        enabled: bool,
        actions: &mut Vec<String>,
    ) -> Result<(), String> {
        let mut child =
            ui.new_child(egui::UiBuilder::new().id_salt("cloud-layouts-content").max_rect(rect));
        child.set_clip_rect(rect);
        child.spacing_mut().item_spacing = Vec2::ZERO;
        frame.record_rect("cloud-layouts", "Shared mobile UI layouts", "region", rect, enabled);
        let mut result = Ok(());
        egui::ScrollArea::vertical()
            .animated(false)
            .id_salt("cloud-layouts-scroll")
            .auto_shrink([false, false])
            .show(&mut child, |ui| {
                result = self.paint_contents(frame, ui, art, rect.width(), enabled, actions);
            });
        result
    }

    fn paint_contents(
        &mut self,
        frame: &mut UiFrame<'_>,
        ui: &mut Ui,
        art: &CloudArt,
        width: f32,
        enabled: bool,
        actions: &mut Vec<String>,
    ) -> Result<(), String> {
        // Pinned dark-cloud-content.css uses viewport breakpoints, centered
        // bounded columns and a one-column receipt on phones.
        let viewport = frame.ctx.content_rect().size();
        let phone = viewport.x <= 700.0;
        let short = !phone && viewport.y <= 620.0;
        let padding = if phone { Vec2::new(4.0, 8.0) } else { Vec2::splat(10.0) };
        let content_width = (width - 2.0 * padding.x).max(1.0);
        let top = ui.cursor().min + padding;
        let heading = "MOBILE UI LAYOUTS";
        let heading_size = text_size(ui, heading, NativeUiFontName::Menu)?;
        let heading_rect = Rect::from_min_size(top, Vec2::new(content_width, heading_size.y));
        ink(frame, ui, heading, NativeUiFontName::Menu, 1.0, 0xd9ba70, heading_rect, true)?;
        frame.record_rect("cloud-layout-heading", heading, "heading", heading_rect, false);
        let header_gap = if short {
            18.0
        } else if phone {
            24.0
        } else {
            30.0
        };
        let sections_width = content_width.min(980.0);
        let gap = if phone {
            28.0
        } else if viewport.x <= 880.0 {
            24.0
        } else {
            32.0
        };
        let section_width = if phone { sections_width } else { (sections_width - gap) / 2.0 };
        let origin =
            top + Vec2::new((content_width - sections_width) / 2.0, heading_size.y + header_gap);
        let busy = self.pending.is_some();
        let mut bottom = origin.y;
        for (index, title) in ["LOAD A LAYOUT", "SUBMIT YOUR LAYOUT"].into_iter().enumerate() {
            let min = if phone {
                egui::pos2(origin.x, bottom)
            } else {
                origin + Vec2::new(index as f32 * (section_width + gap), 0.0)
            };
            let title_size = text_size(ui, title, NativeUiFontName::Medium)?;
            let title_rect = Rect::from_min_size(
                min + Vec2::new(10.0, 0.0),
                Vec2::new(section_width - 10.0, title_size.y),
            );
            ink(frame, ui, title, NativeUiFontName::Medium, 1.0, 0xd9ba70, title_rect, false)?;
            let panel_top = min + Vec2::new(0.0, title_size.y + 12.0);
            let label_size = text_size(ui, "SHARE CODE", NativeUiFontName::Medium)?;
            let field_row_height =
                if phone { (label_size.y + 7.0 + 30.0 + 20.0).max(44.0) } else { 44.0 };
            let panel_height = if index == 0 { field_row_height + 44.0 + 6.0 } else { 50.0 };
            let panel = Rect::from_min_size(panel_top, Vec2::new(section_width, panel_height));
            widgets::control_panel(frame, ui, &art.controls, panel, false)?;
            let action_width = section_width - 6.0;
            if index == 0 {
                let label_rect = Rect::from_min_size(
                    panel.min
                        + Vec2::new(10.0, if phone { 10.0 } else { (44.0 - label_size.y) / 2.0 }),
                    label_size,
                );
                ink(
                    frame,
                    ui,
                    "SHARE CODE",
                    NativeUiFontName::Medium,
                    1.0,
                    0xd9ba70,
                    label_rect,
                    false,
                )?;
                let field = if phone {
                    Rect::from_min_size(
                        panel.min + Vec2::new(10.0, 10.0 + label_size.y + 7.0),
                        Vec2::new(action_width - 20.0, 30.0),
                    )
                } else {
                    Rect::from_min_size(
                        panel.min + Vec2::new(20.0 + label_size.x, 7.0),
                        Vec2::new(action_width - 30.0 - label_size.x, 30.0),
                    )
                };
                widgets::control_panel(frame, ui, &art.controls, field, true)?;
                let input_rect = field.expand2(Vec2::new(0.0, 7.0));
                let response = crate::gpu_ui::NativeTextEdit::singleline(
                    &mut self.input,
                    "dark-cloud-layout-code",
                    "Share code",
                )
                .font(egui::FontId::new(16.0, egui::FontFamily::Name("JetBrains Mono".into())))
                .color(widgets::GOLD)
                .line_height(21.6)
                .letter_spacing(1.28)
                .horizontal_align(egui::Align::Center)
                .margin(Vec2::new(10.0, 7.0))
                .hint_text("ABCD-EFGH")
                .enabled(enabled)
                .rules(crate::gpu_ui::EditorRules {
                    normalize: Some(normalize_layout_code),
                    ..Default::default()
                })
                .show(frame, ui, input_rect);
                if response.has_focus() {
                    ui.painter().rect_stroke(
                        input_rect.shrink(7.0),
                        0.0,
                        egui::Stroke::new(1.0, widgets::GOLD),
                        egui::StrokeKind::Inside,
                    );
                }
                let admitted = enabled && !busy && self.input.len() == 9;
                if admitted && widgets::submitted_on_enter(&response) {
                    actions.push("cloud-layout-load".to_owned());
                }
                let button = Rect::from_min_size(
                    panel.min + Vec2::new(0.0, field_row_height),
                    Vec2::new(action_width, 44.0),
                );
                let label = if matches!(self.pending, Some((_, LayoutAction::Load))) {
                    "LOADING..."
                } else {
                    "LOAD LAYOUT"
                };
                if control_action(frame, ui, "cloud-layout-load", label, button, admitted)?
                    .clicked()
                    && admitted
                {
                    actions.push("cloud-layout-load".to_owned());
                }
            } else {
                let admitted = enabled && !busy && self.account.is_some() && self.customized;
                let button = Rect::from_min_size(panel.min, Vec2::new(action_width, 44.0));
                let label = if matches!(self.pending, Some((_, LayoutAction::Publish))) {
                    "SUBMITTING..."
                } else {
                    "SUBMIT CURRENT LAYOUT"
                };
                if control_action(frame, ui, "cloud-layout-publish", label, button, admitted)?
                    .clicked()
                    && admitted
                {
                    actions.push("cloud-layout-publish".to_owned());
                }
            }
            let note = if index == 0 {
                "Replaces the layout saved on this device."
            } else if self.account.is_none() {
                "Sign in to submit."
            } else if !self.customized {
                "Save a layout in Game Settings first."
            } else {
                "Each submission creates a new share code."
            };
            let note_size = if short { 13.0 } else { 15.0 };
            let run =
                narrative(ui, note, section_width - 20.0, note_size, widgets::color(0xb2a78d))?;
            let note_top = panel.left_bottom() + Vec2::new(10.0, 12.0);
            run.paint(frame, ui.painter(), note_top, 1.0)?;
            bottom = bottom.max(note_top.y + run.size().y);
            if phone && index == 0 {
                bottom += gap;
            }
        }
        if let Some((action, layout)) = &self.shared {
            let status = if matches!(action, LayoutAction::Load) {
                "LAYOUT LOADED"
            } else {
                "LAYOUT PUBLISHED"
            };
            let author = format!("BY {}", layout.author.username);
            let texts = [
                ("cloud-layout-status", status, NativeUiFontName::Medium, 0xbfffbf),
                (
                    "cloud-layout-receipt-code",
                    layout.code.as_str(),
                    NativeUiFontName::Menu,
                    0xd9ba70,
                ),
                ("cloud-layout-author", author.as_str(), NativeUiFontName::Medium, 0xd9ba70),
            ];
            let sizes = texts
                .iter()
                .map(|(_, text, font, _)| text_size(ui, text, *font))
                .collect::<Result<Vec<_>, _>>()?;
            let receipt_width = content_width.min(800.0);
            let receipt_top =
                egui::pos2(top.x + (content_width - receipt_width) / 2.0, bottom + 30.0);
            let row_height = sizes.iter().fold(44.0_f32, |height, size| height.max(size.y));
            let first_width = (receipt_width - sizes[1].x - sizes[2].x - 130.0 - 54.0).max(0.0);
            let mut cursor = receipt_top;
            for (index, (id, text, font, tint)) in texts.iter().enumerate() {
                let width = if phone {
                    receipt_width
                } else if index == 0 {
                    first_width
                } else {
                    sizes[index].x
                };
                let height = if phone { sizes[index].y } else { row_height };
                let rect = Rect::from_min_size(cursor, Vec2::new(width, height));
                ink(frame, ui, text, *font, 1.0, *tint, rect, false)?;
                frame.record_rect(
                    id,
                    text,
                    if index == 0 { "status" } else { "text" },
                    rect,
                    false,
                );
                cursor += if phone {
                    Vec2::new(0.0, height + 14.0)
                } else {
                    Vec2::new(width + 18.0, 0.0)
                };
            }
            let button = Rect::from_min_size(
                cursor,
                Vec2::new(if phone { receipt_width } else { 130.0 }, 44.0),
            );
            if widgets::cloud_button(
                frame,
                ui,
                "cloud-copy-layout",
                if self.copied { "COPIED" } else { "COPY CODE" },
                button,
                0.55,
                enabled,
            )
            .clicked()
                && enabled
            {
                actions.push("cloud-copy-layout".to_owned());
            }
            frame.announce("cloud-layout-receipt", &format!("{status}: {}. {author}", layout.code));
            bottom = button.bottom();
        }
        if let Some(error) = &self.error {
            let run = narrative(ui, error, content_width, 16.0, widgets::ERROR)?;
            let rect = Rect::from_min_size(egui::pos2(top.x, bottom + 24.0), run.size());
            run.paint(frame, ui.painter(), rect.min, 1.0)?;
            frame.record_rect("cloud-layout-error", error, "alert", rect, false);
            frame.announce("cloud-layout-error", error);
            bottom = rect.bottom() + 24.0;
        }
        ui.allocate_space(Vec2::new(width, bottom - ui.cursor().min.y + padding.y));
        Ok(())
    }
}

fn text_size(ui: &Ui, text: &str, font: NativeUiFontName) -> Result<Vec2, String> {
    let layout = layout_native_ui_text(&NativeUiTextSpec {
        placement: solomon_sim::native_ui_text::NativeUiTextPlacement::Box,
        ..NativeUiTextSpec::new(font, text, 0.0, 0.0)
    });
    if layout.unsupported_code_points.is_empty() {
        Ok(Vec2::new(layout.width as f32, layout.height as f32))
    } else {
        let size = solomon_sim::native_ui_text::native_ui_font(font).metrics[0] as f32;
        Ok(crate::gpu_ui::font_run(
            ui.ctx(),
            egui::FontId::new(size, egui::FontFamily::Name("Cinzel".into())),
            widgets::GOLD,
            text,
            None,
        )?
        .size())
    }
}

fn narrative(
    ui: &Ui,
    text: &str,
    width: f32,
    size: f32,
    color: egui::Color32,
) -> Result<std::sync::Arc<crate::gpu_ui::NativeFontRun>, String> {
    crate::gpu_ui::font_run_with_options(
        ui.ctx(),
        egui::FontId::new(size, egui::FontFamily::Name("IM FELL English".into())),
        color,
        text,
        crate::gpu_ui::FontRunOptions {
            wrap_width: Some(width),
            line_height: Some(size * 1.4),
            overflow_wrap: crate::gpu_ui::OverflowWrap::Normal,
            ..Default::default()
        },
    )
}

fn normalize_layout_code(value: &str) -> String {
    let compact: String = value
        .to_uppercase()
        .chars()
        .filter(|c| "ABCDEFGHJKLMNPQRSTUVWXYZ23456789".contains(*c))
        .take(8)
        .collect();
    if compact.len() > 4 { format!("{}-{}", &compact[..4], &compact[4..]) } else { compact }
}
