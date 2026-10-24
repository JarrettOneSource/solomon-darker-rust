//! GPU mod detail, screenshot gallery, versions and comments. The controller
//! ports `DarkCloudModDetail`, `DarkCloudGallery` and `DarkCloudComments` lifetimes.
use super::rows::{control_action, ink};
use super::{super::widgets, art::CloudArt, format, images::CloudImages};
use crate::gpu_ui::{
    FontRunOptions, NativeFontRun, OverflowWrap, UiFrame, font_run, font_run_with_options,
};
use crate::hosted::{ModComments, ModDetail, ModSubscription, ModSummary};
use crate::menu::{HostedMenuAction, ModSubscriptionAction};
use egui::{Color32, Rect, Ui, Vec2};
use solomon_sim::native_ui_text::{NativeUiFontName, measure_native_ui_text};

struct DetailHeader {
    title: Rect,
    author: Rect,
    height: f32,
}

fn detail_header(panel: Rect, viewport: Vec2, fallback_height: Option<f32>) -> DetailHeader {
    let phone = viewport.x <= 700.0;
    let short = viewport.y <= 620.0;
    let minimum: f32 = if short {
        70.0
    } else if phone {
        120.0
    } else {
        90.0
    };
    let padding_top = if short {
        8.0
    } else if phone {
        56.0
    } else {
        12.0
    };
    let padding_bottom = if short || phone { 8.0 } else { 12.0 };
    let padding_x = if phone { 4.0 } else { 124.0 };
    let title_height = fallback_height.unwrap_or(34.0);
    let gap = if fallback_height.is_some() { 6.0 } else { 0.0 };
    let intrinsic = padding_top + title_height + gap + 23.2 + padding_bottom;
    let height = if fallback_height.is_some() { minimum.max(intrinsic) } else { minimum };
    let center = if fallback_height.is_some() { (height - intrinsic) / 2.0 } else { 0.0 };
    let title = Rect::from_min_size(
        panel.min + Vec2::new(padding_x, padding_top + center),
        Vec2::new(panel.width() - padding_x * 2.0, title_height),
    );
    DetailHeader {
        title,
        author: Rect::from_min_size(
            title.min + Vec2::new(0.0, title_height + gap),
            Vec2::new(title.width(), 23.2),
        ),
        height,
    }
}

pub(super) struct ModDetailView {
    pub summary: ModSummary,
    detail: Option<ModDetail>,
    detail_request: Option<u64>,
    detail_error: Option<String>,
    comments: Option<ModComments>,
    comments_request: Option<u64>,
    comment_mutation: Option<(u64, bool)>,
    comment_error: Option<String>,
    account: Option<String>,
    image_index: usize,
    body: String,
    actions: Vec<HostedMenuAction>,
    swipe_start_x: Option<f32>,
    reset_scroll: bool,
    comment_height: f32,
    comment_resize_origin: Option<f32>,
}
impl ModDetailView {
    pub fn open(summary: ModSummary, account: Option<&str>, next: &mut u64) -> Self {
        let mut view = Self {
            summary,
            detail: None,
            detail_request: None,
            detail_error: None,
            comments: None,
            comments_request: None,
            comment_mutation: None,
            comment_error: None,
            account: account.map(str::to_owned),
            image_index: 0,
            body: String::new(),
            actions: Vec::new(),
            swipe_start_x: None,
            reset_scroll: true,
            comment_height: 100.0,
            comment_resize_origin: None,
        };
        view.request_detail(next);
        view.request_comments(next);
        view
    }
    pub fn request_detail(&mut self, next: &mut u64) {
        *next += 1;
        self.detail_request = Some(*next);
        self.detail = None;
        self.detail_error = None;
        self.image_index = 0;
        self.actions.push(HostedMenuAction::LoadModDetail {
            request_id: *next,
            slug: self.summary.slug.clone(),
        });
    }

    fn request_comments(&mut self, next: &mut u64) {
        *next += 1;
        self.comments_request = Some(*next);
        self.comments = None;
        self.comment_error = None;
        self.actions.push(HostedMenuAction::LoadModComments {
            request_id: *next,
            slug: self.summary.slug.clone(),
        });
    }

    pub fn set_account(&mut self, account: Option<&str>) -> Result<(), String> {
        if self.account.as_deref() != account {
            self.account = account.map(str::to_owned);
            self.body.clear();
        }
        Ok(())
    }

    pub fn complete_detail(
        &mut self,
        request_id: u64,
        result: Result<ModDetail, String>,
    ) -> Result<bool, String> {
        if self.detail_request != Some(request_id) {
            return Ok(false);
        }
        self.detail_request = None;
        match result {
            Ok(detail) => {
                self.detail = Some(detail);
                self.detail_error = None;
            }
            Err(error) => self.detail_error = Some(error),
        }
        Ok(true)
    }

    pub fn complete_comments(
        &mut self,
        request_id: u64,
        result: Result<ModComments, String>,
    ) -> Result<bool, String> {
        if self.comments_request != Some(request_id) {
            return Ok(false);
        }
        self.comments_request = None;
        match result {
            Ok(comments) => {
                self.comments = Some(comments);
                self.comment_error = None;
            }
            Err(error) => self.comment_error = Some(error),
        }
        Ok(true)
    }

    pub fn complete_comment_mutation(
        &mut self,
        request_id: u64,
        result: Result<(), String>,
        next: &mut u64,
    ) -> Result<bool, String> {
        let Some((pending, posted)) = self.comment_mutation else {
            return Ok(false);
        };
        if pending != request_id {
            return Ok(false);
        }
        self.comment_mutation = None;
        match result {
            Ok(()) => {
                if posted {
                    self.body.clear();
                }
                self.request_comments(next);
            }
            Err(error) => self.comment_error = Some(error),
        }
        Ok(true)
    }

    pub fn activate(&mut self, action: &str, next: &mut u64) -> Result<bool, String> {
        match action {
            "cloud-detail-retry" => {
                self.request_detail(next);
            }
            "cloud-gallery-previous" => self.change_image(false)?,
            "cloud-gallery-next" => self.change_image(true)?,
            "cloud-post-comment" => {
                if self.comment_mutation.is_some() {
                    return Ok(true);
                }
                if self.account.is_none() {
                    return Ok(true);
                }
                let body = self.body.clone();
                let body = js_sys::JsString::from(body)
                    .trim()
                    .as_string()
                    .ok_or("comment body is not text")?;
                if body.is_empty() {
                    return Ok(true);
                }
                *next += 1;
                self.comment_mutation = Some((*next, true));
                self.comment_error = None;
                self.actions.push(HostedMenuAction::PostModComment {
                    request_id: *next,
                    slug: self.summary.slug.clone(),
                    body,
                });
            }
            _ => {
                if let Some(index) = action.strip_prefix("cloud-gallery:") {
                    let index: usize = index.parse().map_err(|_| "gallery image index")?;
                    if self.detail.as_ref().is_some_and(|detail| index < detail.screenshots.len()) {
                        self.image_index = index;
                    }
                } else if let Some(id) = action.strip_prefix("cloud-delete-comment:") {
                    let id = id.parse().map_err(|_| "comment identity")?;
                    if self.comment_mutation.is_some() || !self.can_delete(id) {
                        return Ok(true);
                    }
                    *next += 1;
                    self.comment_mutation = Some((*next, false));
                    self.comment_error = None;
                    self.actions.push(HostedMenuAction::DeleteModComment {
                        request_id: *next,
                        slug: self.summary.slug.clone(),
                        comment_id: id,
                    });
                } else {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }

    pub fn change_image(&mut self, next: bool) -> Result<(), String> {
        let count = self.detail.as_ref().map_or(0, |detail| detail.screenshots.len());
        if count >= 2 {
            self.image_index = if next {
                (self.image_index + 1) % count
            } else {
                (self.image_index + count - 1) % count
            };
        }
        Ok(())
    }

    pub fn take_actions(&mut self) -> Vec<HostedMenuAction> {
        std::mem::take(&mut self.actions)
    }
    // This painter borrows the scene art, image lifetime owner and subscription policy.
    #[allow(clippy::too_many_arguments)]
    pub fn paint(
        &mut self,
        frame: &mut UiFrame<'_>,
        ui: &mut Ui,
        art: &CloudArt,
        images: &mut CloudImages,
        view: Rect,
        subscription: Option<&ModSubscription>,
        subscription_busy: bool,
        subscription_error: Option<&str>,
        actions: &mut Vec<String>,
    ) -> Result<(), String> {
        let rect = Rect::from_center_size(
            view.center(),
            Vec2::new(1100.0_f32.min(view.width() - 48.0), 820.0_f32.min(view.height() - 48.0)),
        );
        art.panel(frame, ui, rect)?;
        let phone = view.width() <= 700.0;
        let short = view.height() <= 620.0;
        let summary = self.detail.as_ref().map_or(&self.summary, |d| &d.summary);
        let native = solomon_sim::native_ui_text::layout_native_ui_text(
            &solomon_sim::native_ui_text::NativeUiTextSpec {
                scale: 1.05,
                ..solomon_sim::native_ui_text::NativeUiTextSpec::new(
                    NativeUiFontName::Menu,
                    &summary.name,
                    0.0,
                    0.0,
                )
            },
        );
        let fallback = if native.unsupported_code_points.is_empty() {
            None
        } else {
            // The Source fallback is ordinary inline text; its CSS whitespace
            // collapses before normal phone wrapping. Bitmap text stays authored.
            let text = summary
                .name
                .split([' ', '\t', '\r', '\n', '\u{c}'])
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join(" ");
            let title_width = rect.width() - if phone { 8.0 } else { 248.0 };
            let size = solomon_sim::native_ui_text::native_ui_font(NativeUiFontName::Menu).metrics
                [0] as f32
                * 1.05;
            Some(font_run_with_options(
                ui.ctx(),
                egui::FontId::new(size, egui::FontFamily::Name("Cinzel".into())),
                widgets::color(0xd9ba70),
                &text,
                FontRunOptions {
                    wrap_width: phone.then_some(title_width),
                    overflow_wrap: OverflowWrap::Normal,
                    hang_line_end_spaces: true,
                    line_height: Some(size * 1.5),
                    ..Default::default()
                },
            )?)
        };
        let header = detail_header(rect, view.size(), fallback.as_ref().map(|run| run.size().y));
        frame.record_rect("cloud-detail-title", &summary.name, "heading", header.title, false);
        frame.record_value("cloud-detail-title", &summary.name, None);
        if let Some(run) = fallback {
            run.paint_in_line_boxes(
                frame,
                &ui.painter().with_clip_rect(header.title.intersect(ui.clip_rect())),
                header.title,
                egui::Align::Center,
                1.0,
            )?;
        } else {
            ink(
                frame,
                ui,
                &summary.name,
                NativeUiFontName::Menu,
                1.05,
                0xd9ba70,
                header.title,
                true,
            )?;
        }
        ink(
            frame,
            ui,
            &format!("BY {}", summary.author.username),
            NativeUiFontName::Medium,
            1.0,
            0xd9ba70,
            header.author,
            true,
        )?;
        let back = Rect::from_min_size(
            egui::pos2(
                rect.right() - 108.0,
                rect.top()
                    + if short {
                        14.0
                    } else if phone {
                        4.0
                    } else {
                        22.0
                    },
            ),
            Vec2::new(100.0, 44.0),
        );
        if widgets::cloud_button(frame, ui, "cloud-dialog-close", "BACK", back, 0.55, true)
            .clicked()
        {
            actions.push("cloud-dialog-close".to_owned());
        }
        let footer_height = if short {
            60.0
        } else if phone {
            64.0
        } else {
            74.0
        };
        let content = Rect::from_min_max(
            rect.min + Vec2::new(if phone { 4.0 } else { 24.0 }, header.height + 12.0),
            rect.max - Vec2::new(if phone { 4.0 } else { 24.0 }, footer_height),
        );
        let mut child =
            ui.new_child(egui::UiBuilder::new().id_salt("cloud-detail-content").max_rect(content));
        child.set_clip_rect(content);
        child.spacing_mut().item_spacing = Vec2::ZERO;
        let mut result = Ok(());
        let mut scroll = egui::ScrollArea::vertical()
            .animated(false)
            .id_salt(format!("cloud-detail-scroll:{}", self.summary.slug))
            .auto_shrink([false, false]);
        if self.reset_scroll {
            scroll = scroll.vertical_scroll_offset(0.0);
            if child.is_visible() && child.is_enabled() {
                self.reset_scroll = false;
            }
        }
        scroll.show(&mut child, |ui| {
            result = self.paint_contents(
                frame,
                ui,
                art,
                images,
                content.width(),
                view.width(),
                subscription,
                subscription_busy,
                subscription_error,
                actions,
            );
        });
        std::mem::replace(&mut result, Ok(()))?;
        let done = Rect::from_center_size(
            egui::pos2(
                rect.center().x,
                rect.bottom() - footer_height
                    + if short {
                        30.0
                    } else if phone {
                        32.0
                    } else {
                        34.0
                    },
            ),
            Vec2::new(300.0_f32.min(rect.width() - 20.0), 44.0),
        );
        if widgets::stone_button(frame, ui, &art.ui, "cloud-detail-done", "DONE", done, true)?
            .clicked()
        {
            actions.push("cloud-dialog-close".to_owned());
        }
        if !crate::gpu_ui::text_edit_focused(frame.ctx) {
            if frame.ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft)) {
                actions.push("cloud-gallery-previous".to_owned());
            }
            if frame.ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight))
            {
                actions.push("cloud-gallery-next".to_owned());
            }
        }
        Ok(())
    }
    // The scroll painter retains the same borrowed scene resources and host policy.
    #[allow(clippy::too_many_arguments)]
    fn paint_contents(
        &mut self,
        frame: &mut UiFrame<'_>,
        ui: &mut Ui,
        art: &CloudArt,
        images: &mut CloudImages,
        width: f32,
        viewport_width: f32,
        subscription: Option<&ModSubscription>,
        subscription_busy: bool,
        subscription_error: Option<&str>,
        actions: &mut Vec<String>,
    ) -> Result<(), String> {
        if let Some(error) = &self.detail_error {
            let message = narrative_tinted(ui, error, width - 140.0, widgets::ERROR)?;
            let (_, r) = ui.allocate_space(Vec2::new(width, message.size().y.max(44.0)));
            message.paint(
                frame,
                ui.painter(),
                egui::pos2(r.left(), r.center().y - message.size().y / 2.0),
                1.0,
            )?;
            if widgets::cloud_button(
                frame,
                ui,
                "cloud-detail-retry",
                "RETRY",
                Rect::from_center_size(
                    egui::pos2(r.right() - 60.0, r.center().y),
                    Vec2::new(120.0, 44.0),
                ),
                0.55,
                true,
            )
            .clicked()
            {
                actions.push("cloud-detail-retry".to_owned());
            }
            ui.add_space(18.0);
        }
        let columns = viewport_width > 980.0;
        let gallery_width = if columns { (width - 24.0) * 1.55 / 2.4 } else { width };
        let about_width = if columns { width - gallery_width - 24.0 } else { width };
        let origin = ui.cursor().min;
        let image_height = gallery_stage_size(gallery_width, frame.ctx.content_rect().size()).y;
        let has_thumbnails =
            self.detail.as_ref().is_some_and(|detail| detail.screenshots.len() > 1);
        let gallery = Rect::from_min_size(
            origin,
            Vec2::new(
                gallery_width,
                104.0 + image_height + if has_thumbnails { 64.0 } else { 0.0 },
            ),
        );
        self.paint_gallery(frame, ui, art, images, gallery, actions)?;
        let about_origin = if columns {
            origin + Vec2::new(gallery_width + 24.0, 0.0)
        } else {
            egui::pos2(origin.x, gallery.bottom() + 24.0)
        };
        let mut summary_ui = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("cloud-detail-summary")
                .max_rect(Rect::from_min_size(about_origin, Vec2::new(about_width, 1000.0))),
        );
        self.paint_about(
            frame,
            &mut summary_ui,
            art,
            about_width,
            subscription,
            subscription_busy,
            subscription_error,
            actions,
        )?;
        let end = summary_ui.min_rect().bottom().max(gallery.bottom());
        ui.allocate_space(Vec2::new(width, end - origin.y));
        ui.add_space(24.0);
        heading(frame, ui, "DESCRIPTION", None, width)?;
        let summary = self.detail.as_ref().map_or(&self.summary, |d| &d.summary);
        let description = self
            .detail
            .as_ref()
            .map(|d| d.description.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or(&summary.summary);
        panel_text(frame, ui, art, description, width)?;
        ui.add_space(24.0);
        let releases = self.detail.as_ref().map_or("LOADING".to_owned(), |d| {
            format!("{} RELEASE{}", d.versions.len(), if d.versions.len() == 1 { "" } else { "S" })
        });
        heading(frame, ui, "VERSION HISTORY", Some(&releases), width)?;
        self.paint_versions(frame, ui, art, width)?;
        ui.add_space(24.0);
        let comment_count =
            self.comments.as_ref().map_or("LOADING".to_owned(), |c| format!("{} TOTAL", c.total));
        heading(frame, ui, "COMMENTS", Some(&comment_count), width)?;
        if self.account.is_none() {
            let (_, r) = ui.allocate_space(Vec2::new(width, 50.0));
            if control(
                frame,
                ui,
                art,
                "cloud-detail-signin-comment",
                "SIGN IN TO COMMENT",
                r,
                true,
            )?
            .clicked()
            {
                actions.push("cloud-sign-in".to_owned());
            }
        } else {
            let form = Rect::from_min_size(
                ui.cursor().min,
                Vec2::new(width, 40.0 + self.comment_height + 44.0 + 6.0),
            );
            widgets::control_panel(frame, ui, &art.controls, form, false)?;
            let (_, r) = ui.allocate_space(Vec2::new(width - 6.0, 40.0));
            ink(
                frame,
                ui,
                "LEAVE A COMMENT",
                NativeUiFontName::Medium,
                1.0,
                0xd9ba70,
                Rect::from_min_size(
                    r.min + Vec2::new(14.0, 12.0),
                    Vec2::new(r.width() - 28.0, 16.0),
                ),
                false,
            )?;
            let (_, field) = ui.allocate_space(Vec2::new(width - 6.0, self.comment_height));
            widgets::control_panel(frame, ui, &art.controls, field, true)?;
            let response = widgets::edit(
                frame,
                ui,
                "dark-cloud-comment",
                "Leave a comment",
                &mut self.body,
                field,
                "IM FELL English",
                16.0,
                true,
                true,
                crate::gpu_ui::EditorRules { utf16_limit: Some(1000), ..Default::default() },
                "Write a comment...",
            );
            frame.record_value("dark-cloud-comment", &self.body, None);
            let grip =
                Rect::from_min_size(field.right_bottom() - Vec2::splat(16.0), Vec2::splat(16.0));
            let resize =
                ui.interact(grip, egui::Id::unique("cloud-comment-resize"), egui::Sense::DRAG);
            if resize.hovered() || resize.dragged() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
            }
            if resize.drag_started() {
                self.comment_resize_origin = Some(self.comment_height);
            }
            if let Some(origin) = self.comment_resize_origin
                && let Some(delta) = resize.total_drag_delta()
            {
                self.comment_height = (origin + delta.y).max(100.0);
            }
            if resize.drag_stopped() {
                self.comment_resize_origin = None;
            }
            if resize.is_pointer_button_down_on() {
                response.request_focus();
            }
            for inset in [3.0, 7.0, 11.0] {
                ui.painter().line_segment(
                    [
                        field.right_bottom() - Vec2::new(inset, 2.0),
                        field.right_bottom() - Vec2::new(2.0, inset),
                    ],
                    egui::Stroke::new(1.0, widgets::color(0x8e7c4d)),
                );
            }
            frame.record("cloud-comment-resize", "Resize comment field", "slider", &resize);
            frame.record_value("cloud-comment-resize", &self.comment_height.to_string(), None);
            let (_, row) = ui.allocate_space(Vec2::new(width - 6.0, 44.0));
            ink(
                frame,
                ui,
                &format!("{} / 1000", self.body.encode_utf16().count()),
                NativeUiFontName::Medium,
                0.85,
                0xd9ba70,
                Rect::from_min_size(
                    row.min + Vec2::new(14.0, 0.0),
                    Vec2::new(row.width() - 28.0, 44.0),
                ),
                false,
            )?;
            let busy = self.comment_mutation.is_some();
            let admitted = !busy && !self.body.trim().is_empty();
            let label = if busy { "POSTING..." } else { "POST COMMENT" };
            let button_width = content_width(ui, label, 1.0)? + 20.0;
            let b = Rect::from_min_size(
                egui::pos2(row.right() - button_width, row.top()),
                Vec2::new(button_width, 44.0),
            );
            if control_action(frame, ui, "cloud-post-comment", label, b, admitted)?.clicked()
                && admitted
            {
                actions.push("cloud-post-comment".to_owned());
            }
            ui.add_space(6.0);
        }
        if let Some(error) = &self.comment_error {
            flow(frame, ui, error, width, widgets::ERROR)?;
        }
        self.paint_comment_list(frame, ui, art, width, actions)?;
        Ok(())
    }
    fn paint_gallery(
        &mut self,
        frame: &mut UiFrame<'_>,
        ui: &mut Ui,
        art: &CloudArt,
        images: &mut CloudImages,
        rect: Rect,
        actions: &mut Vec<String>,
    ) -> Result<(), String> {
        let screenshots = self.detail.as_ref().map_or(&[][..], |d| d.screenshots.as_slice());
        let status = if screenshots.is_empty() {
            "NO IMAGES".to_owned()
        } else {
            format!("{} / {}", self.image_index + 1, screenshots.len())
        };
        caption(frame, ui, "SCREENSHOTS", Some(&status), rect)?;
        let stage = Rect::from_min_size(
            rect.min + Vec2::new(6.0, 34.0),
            gallery_stage_size(rect.width(), frame.ctx.content_rect().size()),
        );
        widgets::control_panel(frame, ui, &art.controls, stage, false)?;
        images.paint(
            frame,
            ui,
            screenshots.get(self.image_index).map(|i| i.url.as_str()),
            &self.summary.name,
            stage.shrink(6.0),
            true,
        )?;
        frame.record_clipped_rect(
            "cloud-gallery-image",
            &format!("SCREENSHOTS   {status}"),
            "img",
            stage,
            ui.clip_rect(),
            false,
        );
        frame.record_value("cloud-gallery-image", &status, None);
        let swipe = ui.interact(stage, egui::Id::unique("cloud-gallery-swipe"), egui::Sense::DRAG);
        if let Some(action) = gallery_swipe(&swipe, &mut self.swipe_start_x) {
            actions.push(action.to_owned());
        }
        for (label, id, x) in [
            ("PREV", "cloud-gallery-previous", stage.left() + 4.0),
            ("NEXT", "cloud-gallery-next", stage.right() - 100.0),
        ] {
            let b =
                Rect::from_min_size(egui::pos2(x, stage.bottom() + 10.0), Vec2::new(96.0, 44.0));
            let enabled = screenshots.len() > 1;
            if widgets::cloud_button(frame, ui, id, label, b, 0.55, enabled).clicked() && enabled {
                actions.push(id.to_owned());
            }
        }
        if screenshots.len() > 1 {
            let thumbs = Rect::from_min_size(
                egui::pos2(stage.left(), stage.bottom() + 74.0),
                Vec2::new(stage.width(), 58.0),
            );
            let mut child =
                ui.new_child(egui::UiBuilder::new().id_salt("cloud-thumbnails").max_rect(thumbs));
            child.set_clip_rect(thumbs);
            egui::ScrollArea::horizontal()
                .animated(false)
                .id_salt("cloud-gallery-thumbnails")
                .auto_shrink([false, false])
                .show(&mut child, |ui| {
                    ui.set_width(screenshots.len() as f32 * 88.0);
                    for (index, image) in screenshots.iter().enumerate() {
                        let r = Rect::from_min_size(
                            ui.cursor().min + Vec2::new(index as f32 * 88.0, 0.0),
                            Vec2::new(80.0, 54.0),
                        );
                        images.paint(frame, ui, Some(&image.url), &self.summary.name, r, false)?;
                        let id = format!("cloud-gallery:{index}");
                        if widgets::interact(
                            frame,
                            ui,
                            &id,
                            &format!("Show screenshot {}", index + 1),
                            "button",
                            r,
                            true,
                        )
                        .clicked()
                        {
                            actions.push(id);
                        }
                        if index == self.image_index {
                            ui.painter().rect_stroke(
                                r.shrink(3.0),
                                0.0,
                                egui::Stroke::new(1.0, widgets::GOLD),
                                egui::StrokeKind::Inside,
                            );
                        }
                    }
                    ui.allocate_space(Vec2::new(screenshots.len() as f32 * 88.0, 54.0));
                    Ok::<(), String>(())
                })
                .inner?;
        }
        Ok(())
    }
    fn paint_comment_list(
        &self,
        frame: &UiFrame<'_>,
        ui: &mut Ui,
        art: &CloudArt,
        width: f32,
        actions: &mut Vec<String>,
    ) -> Result<(), String> {
        let Some(comments) = &self.comments else { return Ok(()) };
        ui.add_space(18.0);
        if comments.items.is_empty() {
            return empty_panel(frame, ui, art, "NO COMMENTS YET.", width);
        }
        let available = width - 34.0;
        let mut layouts = Vec::new();
        for comment in &comments.items {
            let author_width = content_width(ui, &comment.author.username, 1.0)?;
            let time = format::time_ago(&comment.created_at_utc);
            let time_width = content_width(ui, &time, 0.85)?;
            let wraps = author_width + 16.0 + time_width > available;
            let time_position =
                if wraps { egui::pos2(0.0, 26.0) } else { egui::pos2(author_width + 16.0, 0.0) };
            let mut header_height = time_position.y + 16.0;
            let delete = self.can_delete(comment.id).then(|| {
                let inline = time_position.x + time_width + 16.0 + 105.0 <= available;
                let y = if inline { time_position.y } else { header_height + 10.0 };
                header_height = header_height.max(y + 44.0);
                Rect::from_min_size(egui::pos2(available - 105.0, y), Vec2::new(105.0, 44.0))
            });
            let body = narrative(ui, &comment.body, available)?;
            let height = 24.0 + header_height + 8.0 + body.size().y;
            layouts.push(CommentLayout {
                id: comment.id,
                author: comment.author.username.clone(),
                author_width,
                time,
                time_width,
                time_position,
                delete,
                body,
                header_height,
                height,
            });
        }
        let total = layouts.iter().map(|layout| layout.height).sum::<f32>()
            + layouts.len().saturating_sub(1) as f32
            + 6.0;
        let (_, panel) = ui.allocate_space(Vec2::new(width, total));
        widgets::control_panel(frame, ui, &art.controls, panel, false)?;
        let mut top = panel.top();
        for (index, layout) in layouts.into_iter().enumerate() {
            if index != 0 {
                panel_rule(ui, panel, top);
                top += 1.0;
            }
            let origin = egui::pos2(panel.left() + 14.0, top + 12.0);
            ink(
                frame,
                ui,
                &layout.author,
                NativeUiFontName::Medium,
                1.0,
                0xd9ba70,
                Rect::from_min_size(
                    origin,
                    Vec2::new(
                        layout.author_width,
                        if layout.delete.is_some_and(|rect| rect.top() == 0.0) {
                            44.0
                        } else {
                            16.0
                        },
                    ),
                ),
                false,
            )?;
            ink(
                frame,
                ui,
                &layout.time,
                NativeUiFontName::Medium,
                0.85,
                0xd9ba70,
                Rect::from_min_size(
                    origin + layout.time_position.to_vec2(),
                    Vec2::new(
                        layout.time_width,
                        if layout.delete.is_some_and(|rect| rect.top() == layout.time_position.y) {
                            44.0
                        } else {
                            16.0
                        },
                    ),
                ),
                false,
            )?;
            if let Some(delete) = layout.delete {
                let id = format!("cloud-delete-comment:{}", layout.id);
                if widgets::cloud_button(
                    frame,
                    ui,
                    &id,
                    "DELETE",
                    delete.translate(origin.to_vec2()),
                    0.55,
                    self.comment_mutation.is_none(),
                )
                .clicked()
                    && self.comment_mutation.is_none()
                {
                    actions.push(id);
                }
            }
            layout.body.paint(
                frame,
                ui.painter(),
                origin + Vec2::new(0.0, layout.header_height + 8.0),
                1.0,
            )?;
            top += layout.height;
        }
        Ok(())
    }

    fn paint_versions(
        &self,
        frame: &UiFrame<'_>,
        ui: &mut Ui,
        art: &CloudArt,
        width: f32,
    ) -> Result<(), String> {
        let Some(detail) = &self.detail else {
            return empty_panel(frame, ui, art, "CONSULTING THE DARK CLOUD...", width);
        };
        if detail.versions.is_empty() {
            return empty_panel(frame, ui, art, "NO RELEASES LISTED.", width);
        }
        let mut layouts = Vec::new();
        let available = width - 34.0;
        for version in &detail.versions {
            let metadata = [
                (format!("v{}", version.version), 1.0),
                (format::date(&version.created_at_utc)?, 0.85),
                (format::bytes(version.file_size)?, 0.85),
                (format!("{} DOWNLOADS", format::count(version.downloads)?), 0.85),
            ];
            let mut fields = Vec::new();
            let mut x = 0.0;
            let mut y = 0.0;
            for (label, scale) in metadata {
                let ink_width = content_width(ui, &label, scale)?;
                let gap = if x == 0.0 { 0.0 } else { 16.0 };
                if x != 0.0 && x + gap + ink_width > available {
                    x = 0.0;
                    y += 24.0;
                } else {
                    x += gap;
                }
                fields.push(MetadataField {
                    label,
                    position: egui::pos2(x, y),
                    width: ink_width,
                    scale,
                });
                x += ink_width;
            }
            let header_height = y + 16.0;
            let changelog = if version.changelog.is_empty() {
                None
            } else {
                Some(narrative(ui, &version.changelog, available)?)
            };
            let height = 24.0
                + header_height
                + 8.0
                + changelog.as_ref().map_or(0.0, |galley| galley.size().y);
            layouts.push(ReleaseLayout { fields, header_height, changelog, height });
        }
        let total = layouts.iter().map(|layout| layout.height).sum::<f32>()
            + layouts.len().saturating_sub(1) as f32
            + 6.0;
        let (_, panel) = ui.allocate_space(Vec2::new(width, total));
        widgets::control_panel(frame, ui, &art.controls, panel, false)?;
        let mut top = panel.top();
        for (index, layout) in layouts.into_iter().enumerate() {
            if index != 0 {
                panel_rule(ui, panel, top);
                top += 1.0;
            }
            for field in layout.fields {
                ink(
                    frame,
                    ui,
                    &field.label,
                    NativeUiFontName::Medium,
                    field.scale,
                    0xd9ba70,
                    Rect::from_min_size(
                        egui::pos2(
                            panel.left() + 14.0 + field.position.x,
                            top + 12.0 + field.position.y,
                        ),
                        Vec2::new(field.width, 16.0),
                    ),
                    false,
                )?;
            }
            if let Some(changelog) = layout.changelog {
                changelog.paint(
                    frame,
                    ui.painter(),
                    egui::pos2(panel.left() + 14.0, top + 12.0 + layout.header_height + 8.0),
                    1.0,
                )?;
            }
            top += layout.height;
        }
        Ok(())
    }

    // The About section paints current subscription state without owning its requests.
    #[allow(clippy::too_many_arguments)]
    fn paint_about(
        &self,
        frame: &mut UiFrame<'_>,
        ui: &mut Ui,
        art: &CloudArt,
        width: f32,
        subscription: Option<&ModSubscription>,
        busy: bool,
        error: Option<&str>,
        actions: &mut Vec<String>,
    ) -> Result<(), String> {
        let summary = self.detail.as_ref().map_or(&self.summary, |d| &d.summary);
        heading(frame, ui, "ABOUT THIS MOD", None, width)?;
        let inner_width = width - 6.0;
        let deck = narrative(ui, &summary.summary, inner_width - 28.0)?;
        let deck_height = if summary.summary.is_empty() { 28.0 } else { deck.size().y + 28.0 };
        let tag_width = inner_width
            - 28.0
            - 14.0
            - measure_native_ui_text("TAGS:", NativeUiFontName::Medium, 1.0) as f32;
        let mut tag_lines: Vec<Vec<(&str, f32)>> = Vec::new();
        let mut line_width = 0.0;
        for tag in &summary.tags {
            let tag_ink = content_width(ui, tag, 1.0)?;
            if tag_lines.is_empty() || line_width + 12.0 + tag_ink > tag_width {
                tag_lines.push(Vec::new());
                line_width = 0.0;
            }
            let line = tag_lines.last_mut().expect("a tag creates its line");
            if !line.is_empty() {
                line_width += 12.0;
            }
            line.push((tag, tag_ink));
            line_width += tag_ink;
        }
        let tag_height =
            if tag_lines.is_empty() { 0.0 } else { 20.0 + tag_lines.len() as f32 * 22.0 - 6.0 };
        let (_, panel) =
            ui.allocate_space(Vec2::new(width, deck_height + 160.0 + tag_height + 6.0));
        widgets::control_panel(frame, ui, &art.controls, panel, false)?;
        deck.paint(frame, ui.painter(), panel.min + Vec2::splat(14.0), 1.0)?;
        let content_right = panel.right() - 20.0;
        for (index, (label, value)) in [
            ("VERSION:", summary.latest_version.clone()),
            ("DOWNLOADS:", format::count(summary.downloads)?),
            ("PUBLISHED:", format::date(&summary.created_at_utc)?),
            ("UPDATED:", format::time_ago(&summary.updated_at_utc)),
        ]
        .into_iter()
        .enumerate()
        {
            let top = panel.top() + deck_height + index as f32 * 40.0;
            ui.painter().line_segment(
                [egui::pos2(panel.left(), top), egui::pos2(panel.right() - 6.0, top)],
                egui::Stroke::new(1.0, widgets::color(0x404040)),
            );
            ink(
                frame,
                ui,
                label,
                NativeUiFontName::Medium,
                1.0,
                0xd9ba70,
                Rect::from_min_size(
                    egui::pos2(panel.left() + 14.0, top),
                    Vec2::new(inner_width - 28.0, 40.0),
                ),
                false,
            )?;
            let value_width = content_width(ui, &value, 1.0)?;
            ink(
                frame,
                ui,
                &value,
                NativeUiFontName::Medium,
                1.0,
                0xd9ba70,
                Rect::from_min_size(
                    egui::pos2(content_right - value_width, top),
                    Vec2::new(value_width, 40.0),
                ),
                false,
            )?;
        }
        if !tag_lines.is_empty() {
            let top = panel.top() + deck_height + 160.0;
            ink(
                frame,
                ui,
                "TAGS:",
                NativeUiFontName::Medium,
                1.0,
                0xd9ba70,
                Rect::from_min_size(
                    egui::pos2(panel.left() + 14.0, top),
                    Vec2::new(inner_width - 28.0, tag_height),
                ),
                false,
            )?;
            for (index, line) in tag_lines.iter().enumerate() {
                let line_width = line.iter().map(|(_, width)| *width).sum::<f32>()
                    + line.len().saturating_sub(1) as f32 * 12.0;
                let mut x = content_right - line_width;
                for (tag, width) in line {
                    ink(
                        frame,
                        ui,
                        tag,
                        NativeUiFontName::Medium,
                        1.0,
                        0xd9ba70,
                        Rect::from_min_size(
                            egui::pos2(x, top + 10.0 + index as f32 * 22.0),
                            Vec2::new(*width, 16.0),
                        ),
                        false,
                    )?;
                    x += width + 12.0;
                }
            }
        }
        let state = if self.account.is_none() {
            "GUEST"
        } else {
            subscription
                .map_or("NOT SUBSCRIBED", |s| if s.enabled { "ENABLED" } else { "DISABLED" })
        };
        ui.add_space(24.0);
        heading(frame, ui, "SUBSCRIPTION", Some(state), width)?;
        let choices = if self.account.is_none() {
            vec![("SIGN IN TO SUBSCRIBE".to_owned(), "cloud-detail-signin-subscribe".to_owned())]
        } else if let Some(s) = subscription {
            vec![
                (
                    if busy {
                        "WORKING..."
                    } else if s.enabled {
                        "DISABLE MOD"
                    } else {
                        "ENABLE MOD"
                    }
                    .to_owned(),
                    format!(
                        "cloud-detail-subscription:{}:{}",
                        if s.enabled { "disable" } else { "enable" },
                        self.summary.slug
                    ),
                ),
                (
                    "UNSUBSCRIBE".to_owned(),
                    format!("cloud-detail-subscription:unsubscribe:{}", self.summary.slug),
                ),
            ]
        } else {
            vec![(
                if busy { "WORKING..." } else { "SUBSCRIBE" }.to_owned(),
                format!("cloud-detail-subscription:subscribe:{}", self.summary.slug),
            )]
        };
        let (_, panel) = ui.allocate_space(Vec2::new(width, choices.len() as f32 * 44.0 + 6.0));
        widgets::control_panel(frame, ui, &art.controls, panel, false)?;
        for (index, (label, id)) in choices.into_iter().enumerate() {
            let top = panel.top() + index as f32 * 44.0;
            if index != 0 {
                panel_rule(ui, panel, top);
            }
            let r =
                Rect::from_min_size(egui::pos2(panel.left(), top), Vec2::new(width - 6.0, 44.0));
            if control_action(frame, ui, &id, &label, r, !busy)?.clicked() && !busy {
                actions.push(if id == "cloud-detail-signin-subscribe" {
                    "cloud-sign-in".to_owned()
                } else {
                    id.replacen("cloud-detail-subscription:", "cloud-subscription:", 1)
                });
            }
        }
        if let Some(error) = error {
            flow(frame, ui, error, width, widgets::ERROR)?;
        }
        Ok(())
    }
    fn can_delete(&self, id: u64) -> bool {
        let Some(account) = &self.account else {
            return false;
        };
        self.comments.as_ref().is_some_and(|comments| {
            comments.items.iter().any(|comment| {
                comment.id == id
                    && (account == &self.summary.author.username
                        || account == &comment.author.username)
            })
        })
    }
}

struct MetadataField {
    label: String,
    position: egui::Pos2,
    width: f32,
    scale: f32,
}

struct CommentLayout {
    id: u64,
    author: String,
    author_width: f32,
    time: String,
    time_width: f32,
    time_position: egui::Pos2,
    delete: Option<Rect>,
    body: std::sync::Arc<NativeFontRun>,
    header_height: f32,
    height: f32,
}

struct ReleaseLayout {
    fields: Vec<MetadataField>,
    header_height: f32,
    changelog: Option<std::sync::Arc<NativeFontRun>>,
    height: f32,
}

fn panel_rule(ui: &Ui, panel: Rect, y: f32) {
    ui.painter().line_segment(
        [egui::pos2(panel.left(), y), egui::pos2(panel.right() - 6.0, y)],
        egui::Stroke::new(1.0, widgets::color(0x404040)),
    );
}

fn empty_panel(
    frame: &UiFrame<'_>,
    ui: &mut Ui,
    art: &CloudArt,
    text: &str,
    width: f32,
) -> Result<(), String> {
    let (_, panel) = ui.allocate_space(Vec2::new(width, 50.0));
    widgets::control_panel(frame, ui, &art.controls, panel, false)?;
    ink(
        frame,
        ui,
        text,
        NativeUiFontName::Medium,
        1.0,
        0xd9ba70,
        Rect::from_min_size(panel.min + Vec2::splat(14.0), Vec2::new(width - 34.0, 22.0)),
        false,
    )?;
    Ok(())
}

// DarkCloudGallery.tsx compares the original touch with its released clientX.
// egui's drag_delta is a per-frame movement and becomes zero on release.
fn gallery_swipe(response: &egui::Response, start_x: &mut Option<f32>) -> Option<&'static str> {
    let cancelled = response.ctx.input(|input| {
        input.events.iter().any(|event| {
            matches!(event, egui::Event::Touch { phase: egui::TouchPhase::Cancel, .. })
        })
    });
    if !response.enabled() || cancelled {
        *start_x = None;
        return None;
    }
    if response.drag_started() {
        *start_x = if response.ctx.input(|input| input.any_touches()) {
            response.interact_pointer_pos().map(|position| position.x)
        } else {
            None
        };
    }
    if !response.drag_stopped() {
        return None;
    }
    let start = start_x.take()?;
    let travel = response.interact_pointer_pos()?.x - start;
    if travel.abs() < 40.0 {
        None
    } else if travel < 0.0 {
        Some("cloud-gallery-next")
    } else {
        Some("cloud-gallery-previous")
    }
}

fn gallery_stage_size(width: f32, view: Vec2) -> Vec2 {
    let width = width - 12.0;
    let short = view.y <= 620.0;
    let minimum = if short || view.x <= 700.0 { 160.0 } else { 200.0 };
    let maximum = if short {
        280.0
    } else if view.x <= 980.0 {
        360.0
    } else {
        f32::INFINITY
    };
    Vec2::new(width, (width * 9.0 / 16.0).clamp(minimum, maximum))
}

fn heading(
    frame: &UiFrame<'_>,
    ui: &mut Ui,
    text: &str,
    status: Option<&str>,
    width: f32,
) -> Result<(), String> {
    let (_, r) = ui.allocate_space(Vec2::new(width, 28.0));
    caption(frame, ui, text, status, r)
}

fn caption(
    frame: &UiFrame<'_>,
    ui: &Ui,
    text: &str,
    status: Option<&str>,
    rect: Rect,
) -> Result<(), String> {
    ink(
        frame,
        ui,
        text,
        NativeUiFontName::Medium,
        1.0,
        0xd9ba70,
        Rect::from_min_size(rect.min, Vec2::new(rect.width(), 16.0)),
        false,
    )?;
    if let Some(status) = status {
        let width = measure_native_ui_text(status, NativeUiFontName::Medium, 0.85) as f32;
        ink(
            frame,
            ui,
            status,
            NativeUiFontName::Medium,
            0.85,
            0xd9ba70,
            Rect::from_min_size(
                egui::pos2(rect.right() - width, rect.top()),
                Vec2::new(width, 16.0),
            ),
            false,
        )?;
    }
    Ok(())
}
fn content_width(ui: &Ui, text: &str, scale: f32) -> Result<f32, String> {
    let spec = solomon_sim::native_ui_text::NativeUiTextSpec {
        scale: f64::from(scale),
        ..solomon_sim::native_ui_text::NativeUiTextSpec::new(
            NativeUiFontName::Medium,
            text,
            0.0,
            0.0,
        )
    };
    let layout = solomon_sim::native_ui_text::layout_native_ui_text(&spec);
    if layout.unsupported_code_points.is_empty() {
        Ok(layout.width as f32)
    } else {
        Ok(font_run(
            ui.ctx(),
            egui::FontId::new(16.0 * scale, egui::FontFamily::Name("Cinzel".into())),
            widgets::GOLD,
            text,
            None,
        )?
        .size()
        .x)
    }
}

fn narrative(ui: &Ui, text: &str, width: f32) -> Result<std::sync::Arc<NativeFontRun>, String> {
    narrative_tinted(ui, text, width, widgets::GOLD)
}

fn narrative_tinted(
    ui: &Ui,
    text: &str,
    width: f32,
    color: Color32,
) -> Result<std::sync::Arc<NativeFontRun>, String> {
    font_run_with_options(
        ui.ctx(),
        egui::FontId::new(16.0, egui::FontFamily::Name("IM FELL English".into())),
        color,
        text,
        FontRunOptions { wrap_width: Some(width), line_height: Some(23.2), ..Default::default() },
    )
}

fn flow(
    frame: &UiFrame<'_>,
    ui: &mut Ui,
    text: &str,
    width: f32,
    tint: Color32,
) -> Result<(), String> {
    let run = narrative_tinted(ui, text, width - 28.0, tint)?;
    let (_, r) = ui.allocate_space(Vec2::new(width, run.size().y + 28.0));
    run.paint(frame, ui.painter(), r.min + Vec2::splat(14.0), 1.0)
}

fn panel_text(
    frame: &UiFrame<'_>,
    ui: &mut Ui,
    art: &CloudArt,
    text: &str,
    width: f32,
) -> Result<(), String> {
    let run = narrative(ui, text, width - 34.0)?;
    let height = if text.is_empty() { 34.0 } else { run.size().y + 34.0 };
    let (_, r) = ui.allocate_space(Vec2::new(width, height));
    widgets::control_panel(frame, ui, &art.controls, r, false)?;
    run.paint(frame, ui.painter(), r.min + Vec2::splat(14.0), 1.0)
}

fn control(
    frame: &UiFrame<'_>,
    ui: &Ui,
    art: &CloudArt,
    id: &str,
    label: &str,
    rect: Rect,
    enabled: bool,
) -> Result<egui::Response, String> {
    widgets::control_panel(frame, ui, &art.controls, rect, false)?;
    control_action(
        frame,
        ui,
        id,
        label,
        Rect::from_min_max(rect.min, rect.max - Vec2::splat(6.0)),
        enabled,
    )
}

pub(super) fn subscription_action(value: &str) -> Option<ModSubscriptionAction> {
    match value {
        "subscribe" => Some(ModSubscriptionAction::Subscribe),
        "enable" => Some(ModSubscriptionAction::Enable),
        "disable" => Some(ModSubscriptionAction::Disable),
        "unsubscribe" => Some(ModSubscriptionAction::Unsubscribe),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::gallery_swipe;
    use egui::{Event, PointerButton, Rect, TouchPhase, Vec2};

    fn pass(
        ctx: &egui::Context,
        start_x: &mut Option<f32>,
        events: Vec<Event>,
    ) -> Option<&'static str> {
        let mut action = None;
        ctx.run_pass(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(400.0, 300.0),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                ctx.root_ui(|ui| {
                    let response = ui.interact(
                        egui::Rect::from_min_max(egui::pos2(20.0, 20.0), egui::pos2(380.0, 280.0)),
                        egui::Id::unique("gallery-touch-test"),
                        egui::Sense::DRAG,
                    );
                    action = gallery_swipe(&response, start_x);
                });
            },
        )
        .drop_without_applying_deltas();
        action
    }

    fn pointer(x: f32, phase: TouchPhase, touch: bool) -> Vec<Event> {
        let position = egui::pos2(x, 100.0);
        let mut events = Vec::new();
        if touch {
            events.push(Event::Touch {
                device_id: egui::TouchDeviceId(0),
                id: egui::TouchId(1),
                phase,
                pos: position,
                force: None,
            });
        }
        events.push(Event::PointerMoved(position));
        if phase != TouchPhase::Move {
            events.push(Event::PointerButton {
                pos: position,
                button: PointerButton::Primary,
                pressed: phase == TouchPhase::Start,
                modifiers: egui::Modifiers::NONE,
            });
            if touch && phase != TouchPhase::Start {
                events.push(Event::PointerGone);
            }
        }
        events
    }

    fn swipe(distance: f32, touch: bool, cancel: bool) -> Option<&'static str> {
        let ctx = egui::Context::default();
        let mut start_x = None;
        pass(&ctx, &mut start_x, Vec::new());
        assert_eq!(pass(&ctx, &mut start_x, pointer(200.0, TouchPhase::Start, touch)), None);
        for step in 1..=12 {
            assert_eq!(
                pass(
                    &ctx,
                    &mut start_x,
                    pointer(200.0 + distance * step as f32 / 12.0, TouchPhase::Move, touch),
                ),
                None,
                "movement does not switch before release",
            );
        }
        let action = pass(
            &ctx,
            &mut start_x,
            pointer(
                200.0 + distance,
                if cancel { TouchPhase::Cancel } else { TouchPhase::End },
                touch,
            ),
        );
        assert_eq!(start_x, None, "release retires the original touch");
        assert_eq!(pass(&ctx, &mut start_x, Vec::new()), None, "a swipe is emitted once");
        action
    }

    #[test]
    fn gallery_uses_total_touch_travel_at_the_actual_release() {
        assert_eq!(swipe(-72.0, true, false), Some("cloud-gallery-next"));
        assert_eq!(swipe(72.0, true, false), Some("cloud-gallery-previous"));
    }

    #[test]
    fn gallery_retains_source_threshold_and_excludes_mouse_drag_and_touch_cancel() {
        assert_eq!(swipe(-39.0, true, false), None);
        assert_eq!(swipe(-40.0, true, false), Some("cloud-gallery-next"));
        assert_eq!(swipe(40.0, true, false), Some("cloud-gallery-previous"));
        assert_eq!(swipe(-72.0, false, false), None);
        assert_eq!(swipe(-72.0, true, true), None);
    }

    #[test]
    fn fallback_header_matches_native_wrapped_and_nowrap_flow() {
        // Pinned fa08 dark-cloud-dialogs.css, native Chrome with the Website's
        // imported faces: 25.2/37.8 title, 6px margin, 16/23.2 author.
        for (width, height, title_height, header_height, title_x, title_y, author_y) in [
            (390.0, 844.0, 75.6, 168.781_25, 28.0, 80.0, 161.593_75),
            (700.0, 844.0, 37.8, 130.984_38, 28.0, 80.0, 123.796_875),
            (701.0, 844.0, 37.8, 90.984_375, 148.0, 36.0, 79.796_875),
            (896.0, 414.0, 37.8, 82.984_375, 148.0, 32.0, 75.796_875),
            (390.0, 414.0, 75.6, 120.781_25, 28.0, 32.0, 113.593_75),
        ] {
            let viewport = Vec2::new(width, height);
            let panel = Rect::from_center_size(
                egui::pos2(width / 2.0, height / 2.0),
                Vec2::new(1100.0_f32.min(width - 48.0), 820.0_f32.min(height - 48.0)),
            );
            let header = super::detail_header(panel, viewport, Some(title_height));
            for (actual, expected) in [
                (header.height, header_height),
                (header.title.left(), title_x),
                (header.title.top(), title_y),
                (header.author.top(), author_y),
            ] {
                assert!(
                    (actual - expected).abs() < 0.03,
                    "{viewport:?}: {actual} vs native {expected}"
                );
            }
            assert!((header.title.height() - title_height).abs() < 0.001);
            assert!(header.author.bottom() <= panel.top() + header.height);
        }
    }
}
