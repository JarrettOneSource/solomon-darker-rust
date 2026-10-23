//! GPU mod detail, screenshot gallery, versions and comments. The controller
//! ports `DarkCloudModDetail`, `DarkCloudGallery` and `DarkCloudComments` lifetimes.
use super::{super::widgets, art::CloudArt, format, images::CloudImages};
use crate::gpu_ui::UiFrame;
use crate::hosted::{ModComments, ModDetail, ModSubscription, ModSummary};
use crate::menu::{HostedMenuAction, ModSubscriptionAction};
use egui::{Align2, Color32, Rect, Ui, Vec2};
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
    swiping: bool,
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
            swiping: false,
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
        let phone = view.width() < 700.0;
        let short = view.height() < 620.0;
        let header_height = if short {
            70.0
        } else if phone {
            120.0
        } else {
            90.0
        };
        let title_top = if phone && !short { 56.0 } else { 12.0 };
        let summary = self.detail.as_ref().map_or(&self.summary, |d| &d.summary);
        frame.record_rect(
            "cloud-detail-title",
            &summary.name,
            "heading",
            Rect::from_min_size(
                rect.min + Vec2::new(0.0, title_top),
                Vec2::new(rect.width(), 34.0),
            ),
            false,
        );
        frame.record_value("cloud-detail-title", &summary.name, None);
        widgets::label(
            ui,
            &summary.name,
            egui::pos2(rect.center().x, rect.top() + title_top),
            Align2::CENTER_TOP,
            "Palatino",
            25.0,
            widgets::GOLD,
        );
        widgets::label(
            ui,
            &format!("BY {}", summary.author.username),
            egui::pos2(rect.center().x, rect.top() + title_top + 34.0),
            Align2::CENTER_TOP,
            "Palatino",
            16.0,
            widgets::GOLD,
        );
        let back = Rect::from_min_size(
            egui::pos2(rect.right() - 108.0, rect.top() + if phone { 4.0 } else { 22.0 }),
            Vec2::new(100.0, 44.0),
        );
        if widgets::cloud_button(frame, ui, "cloud-dialog-close", "BACK", back, 0.55, true)
            .clicked()
        {
            actions.push("cloud-dialog-close".to_owned());
        }
        let footer_height = if short { 56.0 } else { 70.0 };
        let content = Rect::from_min_max(
            rect.min + Vec2::new(if phone { 4.0 } else { 24.0 }, header_height + 12.0),
            rect.max - Vec2::new(if phone { 4.0 } else { 24.0 }, footer_height),
        );
        let mut child =
            ui.new_child(egui::UiBuilder::new().id_salt("cloud-detail-content").max_rect(content));
        child.set_clip_rect(content);
        let mut result = Ok(());
        egui::ScrollArea::vertical()
            .id_salt(format!("cloud-detail-scroll:{}", self.summary.slug))
            .auto_shrink([false, false])
            .show(&mut child, |ui| {
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
            egui::pos2(rect.center().x, rect.bottom() - footer_height / 2.0),
            Vec2::new(300.0_f32.min(rect.width() - 20.0), 41.0),
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
            flow(ui, error, width, widgets::ERROR);
            let (_, r) = ui.allocate_space(Vec2::new(width, 48.0));
            if widgets::cloud_button(
                frame,
                ui,
                "cloud-detail-retry",
                "RETRY",
                Rect::from_min_size(r.min, Vec2::new(120.0, 44.0)),
                0.55,
                true,
            )
            .clicked()
            {
                actions.push("cloud-detail-retry".to_owned());
            }
        }
        let columns = viewport_width >= 980.0;
        let gallery_width = if columns { (width - 24.0) * 1.55 / 2.4 } else { width };
        let about_width = if columns { width - gallery_width - 24.0 } else { width };
        let origin = ui.cursor().min;
        let image_height = (gallery_width * 9.0 / 16.0).clamp(160.0, 360.0);
        let has_thumbnails =
            self.detail.as_ref().is_some_and(|detail| detail.screenshots.len() > 1);
        let gallery = Rect::from_min_size(
            origin,
            Vec2::new(
                gallery_width,
                30.0 + image_height + 64.0 + if has_thumbnails { 64.0 } else { 0.0 },
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
        ui.allocate_space(Vec2::new(width, end - origin.y + 24.0));
        heading(frame, ui, "DESCRIPTION", width);
        let summary = self.detail.as_ref().map_or(&self.summary, |d| &d.summary);
        let description = self
            .detail
            .as_ref()
            .map(|d| d.description.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or(&summary.summary);
        panel_text(frame, ui, art, description, width)?;
        heading(
            frame,
            ui,
            &format!(
                "VERSION HISTORY   {}",
                self.detail.as_ref().map_or("LOADING".to_owned(), |d| format!(
                    "{} RELEASE{}",
                    d.versions.len(),
                    if d.versions.len() == 1 { "" } else { "S" }
                ))
            ),
            width,
        );
        if let Some(detail) = &self.detail {
            if detail.versions.is_empty() {
                panel_text(frame, ui, art, "NO RELEASES LISTED.", width)?;
            }
            for version in &detail.versions {
                panel_text(
                    frame,
                    ui,
                    art,
                    &format!(
                        "v{}   {}   {}   {} DOWNLOADS
{}",
                        version.version,
                        format::date(&version.created_at_utc)?,
                        format::bytes(version.file_size)?,
                        format::count(version.downloads)?,
                        version.changelog
                    ),
                    width,
                )?;
            }
        } else {
            panel_text(frame, ui, art, "CONSULTING THE DARK CLOUD...", width)?;
        }
        heading(
            frame,
            ui,
            &format!(
                "COMMENTS   {}",
                self.comments
                    .as_ref()
                    .map_or("LOADING".to_owned(), |c| format!("{} TOTAL", c.total))
            ),
            width,
        );
        if self.account.is_none() {
            let (_, r) = ui.allocate_space(Vec2::new(width, 52.0));
            if control(
                frame,
                ui,
                art,
                "cloud-detail-signin-comment",
                "SIGN IN TO COMMENT",
                r,
                false,
                true,
            )?
            .clicked()
            {
                actions.push("cloud-sign-in".to_owned());
            }
        } else {
            let (_, r) = ui.allocate_space(Vec2::new(width, 28.0));
            widgets::label(
                ui,
                "LEAVE A COMMENT",
                r.min + Vec2::new(14.0, 0.0),
                Align2::LEFT_TOP,
                "Palatino",
                16.0,
                widgets::GOLD,
            );
            let (_, field) = ui.allocate_space(Vec2::new(width, 110.0));
            widgets::control_panel(frame, ui, &art.controls, field, true)?;
            widgets::edit(
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
            let (_, row) = ui.allocate_space(Vec2::new(width, 50.0));
            widgets::label(
                ui,
                &format!("{} / 1000", self.body.encode_utf16().count()),
                row.min + Vec2::new(14.0, 20.0),
                Align2::LEFT_CENTER,
                "Palatino",
                14.0,
                widgets::GOLD,
            );
            let busy = self.comment_mutation.is_some();
            let admitted = !busy && !self.body.trim().is_empty();
            let b = Rect::from_min_size(
                egui::pos2(row.right() - 200.0, row.top()),
                Vec2::new(200.0, 44.0),
            );
            if control(
                frame,
                ui,
                art,
                "cloud-post-comment",
                if busy { "POSTING..." } else { "POST COMMENT" },
                b,
                false,
                admitted,
            )?
            .clicked()
                && admitted
            {
                actions.push("cloud-post-comment".to_owned());
            }
        }
        if let Some(error) = &self.comment_error {
            flow(ui, error, width, widgets::ERROR);
        }
        if let Some(comments) = &self.comments {
            if comments.items.is_empty() {
                panel_text(frame, ui, art, "NO COMMENTS YET.", width)?;
            }
            for comment in &comments.items {
                let (_, header) = ui.allocate_space(Vec2::new(width, 48.0));
                widgets::label(
                    ui,
                    &comment.author.username,
                    header.min + Vec2::new(14.0, 18.0),
                    Align2::LEFT_CENTER,
                    "Palatino",
                    17.0,
                    widgets::GOLD,
                );
                widgets::label(
                    ui,
                    &format::time_ago(&comment.created_at_utc),
                    header.min + Vec2::new(200.0, 18.0),
                    Align2::LEFT_CENTER,
                    "Palatino",
                    14.0,
                    widgets::GOLD,
                );
                if self.can_delete(comment.id) {
                    let id = format!("cloud-delete-comment:{}", comment.id);
                    let b = Rect::from_min_size(
                        egui::pos2(header.right() - 110.0, header.top()),
                        Vec2::new(105.0, 44.0),
                    );
                    if widgets::cloud_button(
                        frame,
                        ui,
                        &id,
                        "DELETE",
                        b,
                        0.55,
                        self.comment_mutation.is_none(),
                    )
                    .clicked()
                        && self.comment_mutation.is_none()
                    {
                        actions.push(id);
                    }
                }
                panel_text(frame, ui, art, &comment.body, width)?;
            }
        }
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
        let label = if screenshots.is_empty() {
            "SCREENSHOTS   NO IMAGES".to_owned()
        } else {
            format!("SCREENSHOTS   {} / {}", self.image_index + 1, screenshots.len())
        };
        widgets::label(ui, &label, rect.min, Align2::LEFT_TOP, "Palatino", 16.0, widgets::GOLD);
        let height = (rect.width() * 9.0 / 16.0).clamp(160.0, 360.0);
        let stage =
            Rect::from_min_size(rect.min + Vec2::new(0.0, 30.0), Vec2::new(rect.width(), height));
        widgets::control_panel(frame, ui, &art.controls, stage, false)?;
        images.paint(
            frame,
            ui,
            screenshots.get(self.image_index).map(|i| i.url.as_str()),
            &self.summary.name,
            stage.shrink(6.0),
            true,
        );
        frame.record_rect("cloud-gallery-image", &label, "img", stage, false);
        frame.record_value(
            "cloud-gallery-image",
            &format!("{} / {}", self.image_index + 1, screenshots.len()),
            None,
        );
        let swipe =
            ui.interact(stage, egui::Id::unique("cloud-gallery-swipe"), egui::Sense::drag());
        if swipe.drag_started() {
            self.swiping = ui.input(|i| i.any_touches());
        }
        if swipe.drag_stopped() && self.swiping {
            self.swiping = false;
            let travel = swipe.drag_delta().x;
            if travel.abs() >= 40.0 {
                actions.push(
                    if travel < 0.0 { "cloud-gallery-next" } else { "cloud-gallery-previous" }
                        .to_owned(),
                );
            }
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
                egui::pos2(stage.left(), stage.bottom() + 66.0),
                Vec2::new(stage.width(), 58.0),
            );
            let mut child =
                ui.new_child(egui::UiBuilder::new().id_salt("cloud-thumbnails").max_rect(thumbs));
            child.set_clip_rect(thumbs);
            egui::ScrollArea::horizontal()
                .id_salt("cloud-gallery-thumbnails")
                .auto_shrink([false, false])
                .show(&mut child, |ui| {
                    ui.set_width(screenshots.len() as f32 * 88.0);
                    for (index, image) in screenshots.iter().enumerate() {
                        let r = Rect::from_min_size(
                            ui.cursor().min + Vec2::new(index as f32 * 88.0, 0.0),
                            Vec2::new(80.0, 54.0),
                        );
                        images.paint(frame, ui, Some(&image.url), &self.summary.name, r, false);
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
                });
        }
        Ok(())
    }
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
        heading(frame, ui, "ABOUT THIS MOD", width);
        panel_text(frame, ui, art, &summary.summary, width)?;
        for (label, value) in [
            ("VERSION:", summary.latest_version.clone()),
            ("DOWNLOADS:", format::count(summary.downloads)?),
            ("PUBLISHED:", format::date(&summary.created_at_utc)?),
            ("UPDATED:", format::time_ago(&summary.updated_at_utc)),
        ] {
            let (_, r) = ui.allocate_space(Vec2::new(width, 40.0));
            widgets::control_panel(frame, ui, &art.controls, r, false)?;
            widgets::label(
                ui,
                label,
                r.min + Vec2::new(14.0, 20.0),
                Align2::LEFT_CENTER,
                "Palatino",
                16.0,
                widgets::GOLD,
            );
            widgets::label(
                ui,
                &value,
                r.max - Vec2::new(14.0, 20.0),
                Align2::RIGHT_CENTER,
                "Palatino",
                16.0,
                widgets::GOLD,
            );
        }
        if !summary.tags.is_empty() {
            panel_text(frame, ui, art, &format!("TAGS: {}", summary.tags.join("  ")), width)?;
        }
        let state = if self.account.is_none() {
            "GUEST"
        } else {
            subscription
                .map_or("NOT SUBSCRIBED", |s| if s.enabled { "ENABLED" } else { "DISABLED" })
        };
        heading(frame, ui, &format!("SUBSCRIPTION   {state}"), width);
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
        for (label, id) in choices {
            let (_, r) = ui.allocate_space(Vec2::new(width, 48.0));
            if control(frame, ui, art, &id, &label, r, false, !busy)?.clicked() && !busy {
                actions.push(if id == "cloud-detail-signin-subscribe" {
                    "cloud-sign-in".to_owned()
                } else {
                    id.replacen("cloud-detail-subscription:", "cloud-subscription:", 1)
                });
            }
        }
        if let Some(error) = error {
            flow(ui, error, width, widgets::ERROR);
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

fn heading(frame: &UiFrame<'_>, ui: &mut Ui, text: &str, width: f32) {
    ui.add_space(12.0);
    let (_, r) = ui.allocate_space(Vec2::new(width, 30.0));
    widgets::native(
        frame,
        ui,
        text,
        solomon_sim::native_ui_text::NativeUiFontName::Medium,
        r.min + Vec2::new(0.0, 18.0),
        1.0,
        0xd9ba70,
        solomon_sim::native_ui_text::NativeUiTextAlign::Left,
    );
}
fn flow(ui: &mut Ui, text: &str, width: f32, tint: Color32) {
    let galley = ui.painter().layout(
        text.to_owned(),
        egui::FontId::new(16.0, egui::FontFamily::Name("IM FELL English".into())),
        tint,
        width - 28.0,
    );
    let (_, r) = ui.allocate_space(Vec2::new(width, galley.size().y + 28.0));
    ui.painter().galley(r.min + Vec2::splat(14.0), galley, tint);
}
fn panel_text(
    frame: &UiFrame<'_>,
    ui: &mut Ui,
    art: &CloudArt,
    text: &str,
    width: f32,
) -> Result<(), String> {
    let galley = ui.painter().layout(
        text.to_owned(),
        egui::FontId::new(16.0, egui::FontFamily::Name("IM FELL English".into())),
        widgets::GOLD,
        width - 28.0,
    );
    let (_, r) = ui.allocate_space(Vec2::new(width, galley.size().y + 28.0));
    widgets::control_panel(frame, ui, &art.controls, r, false)?;
    ui.painter().galley(r.min + Vec2::splat(14.0), galley, widgets::GOLD);
    Ok(())
}
fn control(
    frame: &mut UiFrame<'_>,
    ui: &Ui,
    art: &CloudArt,
    id: &str,
    label: &str,
    rect: Rect,
    selected: bool,
    enabled: bool,
) -> Result<egui::Response, String> {
    widgets::control_panel(frame, ui, &art.controls, rect, false)?;
    let response = widgets::interact(frame, ui, id, label, "button", rect, enabled);
    widgets::native(
        frame,
        ui,
        label,
        solomon_sim::native_ui_text::NativeUiFontName::Medium,
        rect.min + Vec2::new(10.0, rect.height() / 2.0 + 5.0),
        1.0,
        if enabled { 0xd9ba70 } else { 0x8e7c4d },
        solomon_sim::native_ui_text::NativeUiTextAlign::Left,
    );
    if selected {
        ui.painter().line_segment(
            [rect.left_bottom(), rect.right_bottom()],
            egui::Stroke::new(2.0, widgets::GOLD),
        );
    }
    Ok(response)
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
