//! `DarkCloudLayouts.tsx` retains actual saved-layout admission and completion.
use super::{super::widgets, art::CloudArt};
use crate::{gpu_ui::UiFrame, hosted::SharedMobileUiLayout, menu::HostedMenuAction};
use egui::{Align2, Rect, Ui, Vec2};
use std::cell::RefCell;
use std::rc::Rc;
#[derive(Clone, Copy)]
enum LayoutAction {
    Load,
    Publish,
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
    copy_result: Rc<RefCell<Option<(u64, Result<(), String>)>>>,
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
        if action == "cloud-layout-edited" {
            self.normalize();
            return Ok(true);
        }
        if action == "cloud-copy-layout" {
            if let Some((_, layout)) = &self.shared {
                let promise = web_sys::window()
                    .ok_or("copy layout window")?
                    .navigator()
                    .clipboard()
                    .write_text(&layout.code);
                let generation = self.copy_generation;
                let result = self.copy_result.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    *result.borrow_mut() = Some((
                        generation,
                        wasm_bindgen_futures::JsFuture::from(promise)
                            .await
                            .map(|_| ())
                            .map_err(|_| "The code could not be copied.".to_owned()),
                    ));
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
    fn normalize(&mut self) {
        let compact: String = self
            .input
            .to_uppercase()
            .chars()
            .filter(|c| "ABCDEFGHJKLMNPQRSTUVWXYZ23456789".contains(*c))
            .take(8)
            .collect();
        self.input = if compact.len() > 4 {
            format!("{}-{}", &compact[..4], &compact[4..])
        } else {
            compact
        };
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
        if let Some((generation, result)) = self.copy_result.borrow_mut().take()
            && generation == self.copy_generation
        {
            match result {
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
        let mut result = Ok(());
        egui::ScrollArea::vertical()
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
        let (_, heading) = ui.allocate_space(Vec2::new(width, 44.0));
        widgets::native(
            frame,
            ui,
            "MOBILE UI LAYOUTS",
            solomon_sim::native_ui_text::NativeUiFontName::Menu,
            heading.min + Vec2::new(0.0, 26.0),
            1.0,
            0xd9ba70,
            solomon_sim::native_ui_text::NativeUiTextAlign::Left,
        );
        let busy = self.pending.is_some();
        let section_width = if width >= 700.0 { (width - 30.0) / 2.0 } else { width };
        let origin = ui.cursor().min;
        let mut bottom = origin.y;
        for (index, title) in ["LOAD A LAYOUT", "SUBMIT YOUR LAYOUT"].into_iter().enumerate() {
            let min = if width >= 700.0 {
                origin + Vec2::new(index as f32 * (section_width + 30.0), 0.0)
            } else {
                egui::pos2(origin.x, bottom)
            };
            let section = Rect::from_min_size(min, Vec2::new(section_width, 230.0));
            widgets::native(
                frame,
                ui,
                title,
                solomon_sim::native_ui_text::NativeUiFontName::Medium,
                min + Vec2::new(0.0, 18.0),
                1.0,
                0xd9ba70,
                solomon_sim::native_ui_text::NativeUiTextAlign::Left,
            );
            let panel = Rect::from_min_size(
                min + Vec2::new(0.0, 34.0),
                Vec2::new(section_width, if index == 0 { 136.0 } else { 62.0 }),
            );
            widgets::control_panel(frame, ui, &art.controls, panel, false)?;
            if index == 0 {
                widgets::label(
                    ui,
                    "SHARE CODE",
                    panel.min + Vec2::new(10.0, 12.0),
                    Align2::LEFT_TOP,
                    "Palatino",
                    16.0,
                    widgets::GOLD,
                );
                let field = Rect::from_min_size(
                    panel.min + Vec2::new(10.0, 38.0),
                    Vec2::new(panel.width() - 26.0, 40.0),
                );
                widgets::control_panel(frame, ui, &art.controls, field, true)?;
                let response = widgets::edit(
                    frame,
                    ui,
                    "dark-cloud-layout-code",
                    "Share code",
                    &mut self.input,
                    field,
                    "IM FELL English",
                    16.0,
                    enabled && !busy,
                    false,
                );
                frame.record_value("dark-cloud-layout-code", &self.input, None);
                if response.changed() {
                    self.normalize();
                }
                let admitted = enabled && !busy && self.input.len() == 9;
                let b = Rect::from_min_size(
                    panel.min + Vec2::new(10.0, 84.0),
                    Vec2::new(panel.width() - 26.0, 44.0),
                );
                if widgets::cloud_button(
                    frame,
                    ui,
                    "cloud-layout-load",
                    if matches!(self.pending, Some((_, LayoutAction::Load))) {
                        "LOADING..."
                    } else {
                        "LOAD LAYOUT"
                    },
                    b,
                    0.55,
                    admitted,
                )
                .clicked()
                    && admitted
                {
                    actions.push("cloud-layout-load".to_owned());
                }
                widgets::paragraph(
                    ui,
                    "Replaces the layout saved on this device.",
                    Rect::from_min_size(
                        egui::pos2(section.left(), panel.bottom() + 12.0),
                        Vec2::new(section_width, 48.0),
                    ),
                    "IM FELL English",
                    16.0,
                    widgets::GOLD,
                );
            } else {
                let admitted = enabled && !busy && self.account.is_some() && self.customized;
                let b = panel.shrink(8.0);
                if widgets::cloud_button(
                    frame,
                    ui,
                    "cloud-layout-publish",
                    if matches!(self.pending, Some((_, LayoutAction::Publish))) {
                        "SUBMITTING..."
                    } else {
                        "SUBMIT CURRENT LAYOUT"
                    },
                    b,
                    0.55,
                    admitted,
                )
                .clicked()
                    && admitted
                {
                    actions.push("cloud-layout-publish".to_owned());
                }
                widgets::paragraph(
                    ui,
                    if self.account.is_none() {
                        "Sign in to submit."
                    } else if !self.customized {
                        "Save a layout in Game Settings first."
                    } else {
                        "Each submission creates a new share code."
                    },
                    Rect::from_min_size(
                        egui::pos2(section.left(), panel.bottom() + 12.0),
                        Vec2::new(section_width, 70.0),
                    ),
                    "IM FELL English",
                    16.0,
                    widgets::GOLD,
                );
            }
            bottom = section.bottom() + 18.0;
        }
        ui.allocate_space(Vec2::new(width, bottom - origin.y));
        if let Some((action, layout)) = &self.shared {
            let (_, r) = ui.allocate_space(Vec2::new(width, 100.0));
            widgets::label(
                ui,
                if matches!(action, LayoutAction::Load) {
                    "LAYOUT LOADED"
                } else {
                    "LAYOUT PUBLISHED"
                },
                r.min,
                Align2::LEFT_TOP,
                "Palatino",
                18.0,
                widgets::PALE,
            );
            widgets::label(
                ui,
                &layout.code,
                r.min + Vec2::new(0.0, 30.0),
                Align2::LEFT_TOP,
                "JetBrains Mono",
                20.0,
                widgets::GOLD,
            );
            widgets::label(
                ui,
                &format!("BY {}", layout.author.username),
                r.min + Vec2::new(200.0, 34.0),
                Align2::LEFT_TOP,
                "Palatino",
                16.0,
                widgets::GOLD,
            );
            let b = Rect::from_min_size(
                egui::pos2(r.right() - 160.0, r.top() + 12.0),
                Vec2::new(150.0, 44.0),
            );
            if widgets::cloud_button(
                frame,
                ui,
                "cloud-copy-layout",
                if self.copied { "COPIED" } else { "COPY CODE" },
                b,
                0.55,
                enabled,
            )
            .clicked()
                && enabled
            {
                actions.push("cloud-copy-layout".to_owned());
            }
        }
        if let Some(error) = &self.error {
            let (_, r) = ui.allocate_space(Vec2::new(width, 60.0));
            widgets::paragraph(ui, error, r, "IM FELL English", 16.0, widgets::ERROR);
        }
        Ok(())
    }
}
