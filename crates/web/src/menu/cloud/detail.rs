//! The actual catalog detail, gallery, version history and comments from
//! Website `DarkCloudModDetail.tsx`, `DarkCloudGallery.tsx` and `DarkCloudComments.tsx`.

use std::cell::RefCell;
use std::rc::Rc;

use solomon_sim::native_ui_text::NativeUiFontName::{Medium, Menu};
use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{Document, Event, HtmlElement, HtmlTextAreaElement, PointerEvent};

use super::dialog::CloudDialog;
use super::format;
use super::images::CloudImages;
use super::widgets::{self, control_action, control_panel, narrative, text};
use crate::hosted::{ModComments, ModDetail, ModSubscription, ModSummary};
use crate::menu::{HostedMenuAction, ModSubscriptionAction};
use crate::ui::{append, element};

pub(super) struct ModDetailView {
    pub dialog: CloudDialog,
    pub summary: ModSummary,
    document: Document,
    detail: Option<ModDetail>,
    detail_request: Option<u64>,
    detail_error: Option<String>,
    comments: Option<ModComments>,
    comments_request: Option<u64>,
    comment_mutation: Option<(u64, bool)>,
    comment_error: Option<String>,
    account: Option<String>,
    image_index: usize,
    header: HtmlElement,
    gallery: HtmlElement,
    about: HtmlElement,
    subscription: HtmlElement,
    copy: HtmlElement,
    versions: HtmlElement,
    comment_form: HtmlElement,
    comment_count: HtmlElement,
    comment_list: HtmlElement,
    comment_notice: HtmlElement,
    load_notice: HtmlElement,
    body: Option<HtmlTextAreaElement>,
    counter: Option<HtmlElement>,
    post: Option<HtmlElement>,
    actions: Vec<HostedMenuAction>,
    _swipe: Vec<Closure<dyn FnMut(Event)>>,
}

impl ModDetailView {
    pub fn open(
        document: &Document,
        parent: &HtmlElement,
        summary: ModSummary,
        account: Option<&str>,
        events: Rc<RefCell<Vec<String>>>,
        next_request: &mut u64,
        images: &mut CloudImages,
    ) -> Result<Self, String> {
        let dialog =
            CloudDialog::new(document, parent, "dark-cloud-detail", &summary.name, events.clone())?;
        let header = element(document, "header", "dark-cloud-detail-header")?;
        append(dialog.element(), &header)?;
        let scroll = element(document, "div", "dark-cloud-detail-scroll")?;
        let load_notice = element(document, "div", "dark-cloud-detail-load-error")?;
        load_notice.set_attribute("role", "alert").map_err(|_| "mod detail error role")?;
        append(&scroll, &load_notice)?;
        let layout = element(document, "div", "dark-cloud-detail-layout")?;
        let gallery = element(document, "section", "dark-cloud-gallery")?;
        gallery.set_attribute("aria-label", "Screenshots").map_err(|_| "gallery label")?;
        append(&layout, &gallery)?;
        let summary_column = element(document, "aside", "dark-cloud-detail-summary")?;
        let about = element(document, "div", "")?;
        append(&summary_column, &about)?;
        let subscription = element(document, "div", "dark-cloud-detail-subscription")?;
        append(&summary_column, &subscription)?;
        append(&layout, &summary_column)?;
        append(&scroll, &layout)?;
        let copy = element(document, "section", "dark-cloud-detail-copy")?;
        append(&scroll, &copy)?;
        let versions = element(document, "section", "dark-cloud-versions")?;
        append(&scroll, &versions)?;
        let comments = element(document, "section", "dark-cloud-comments")?;
        comments.set_attribute("aria-label", "Comments").map_err(|_| "comments label")?;
        let comment_heading = section_heading(document, &comments, "COMMENTS")?;
        let comment_count = element(document, "span", "")?;
        append(&comment_heading, &comment_count)?;
        let comment_form = element(document, "div", "")?;
        append(&comments, &comment_form)?;
        let comment_notice = element(document, "p", "dark-cloud-inline-error")?;
        comment_notice.set_attribute("role", "alert").map_err(|_| "comment error role")?;
        append(&comments, &comment_notice)?;
        let comment_list = element(document, "div", "")?;
        append(&comments, &comment_list)?;
        append(&scroll, &comments)?;
        append(dialog.element(), &scroll)?;
        let footer = element(document, "footer", "dark-cloud-panel-footer")?;
        let done = widgets::button(
            document,
            "DONE",
            "cloud-dialog-close",
            "native-ui-stone-button",
            None,
            44.0,
            1.0,
            false,
        )?;
        done.set_attribute("data-cloud-stone", "").map_err(|_| "mod detail Done art")?;
        append(&footer, &done)?;
        append(dialog.element(), &footer)?;

        let mut view = Self {
            dialog,
            summary,
            document: document.clone(),
            detail: None,
            detail_request: None,
            detail_error: None,
            comments: None,
            comments_request: None,
            comment_mutation: None,
            comment_error: None,
            account: account.map(str::to_owned),
            image_index: 0,
            header,
            gallery,
            about,
            subscription,
            copy,
            versions,
            comment_form,
            comment_count,
            comment_list,
            comment_notice,
            load_notice,
            body: None,
            counter: None,
            post: None,
            actions: Vec::new(),
            _swipe: Vec::new(),
        };
        // Pointer events preserve the source's forty-pixel touch swipe while
        // leaving mouse drags and the browser's vertical pan untouched.
        let start = Rc::new(RefCell::new(None));
        let swipe_start = start.clone();
        let down = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
            let Some(event) = event.dyn_ref::<PointerEvent>() else {
                return;
            };
            if event.pointer_type() == "touch" {
                *swipe_start.borrow_mut() = Some(event.client_x());
            }
        });
        let up = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
            let Some(event) = event.dyn_ref::<PointerEvent>() else {
                return;
            };
            if event.pointer_type() != "touch" {
                return;
            }
            let Some(start) = start.borrow_mut().take() else {
                return;
            };
            let travel = event.client_x() - start;
            if travel.abs() >= 40 {
                events.borrow_mut().push(
                    if travel < 0 { "cloud-gallery-next" } else { "cloud-gallery-previous" }
                        .to_owned(),
                );
            }
        });
        view.gallery
            .add_event_listener_with_callback("pointerdown", down.as_ref().unchecked_ref())
            .map_err(|_| "gallery swipe start")?;
        view.gallery
            .add_event_listener_with_callback("pointerup", up.as_ref().unchecked_ref())
            .map_err(|_| "gallery swipe end")?;
        view._swipe = vec![down, up];
        view.request_detail(next_request);
        view.request_comments(next_request);
        view.paint_detail(images)?;
        view.paint_comment_form()?;
        view.paint_comments()?;
        view.dialog.show()?;
        Ok(view)
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
            self.paint_comment_form()?;
            self.paint_comments()?;
        }
        Ok(())
    }

    pub fn complete_detail(
        &mut self,
        request_id: u64,
        result: Result<ModDetail, String>,
        images: &mut CloudImages,
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
        self.paint_detail(images)?;
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
        self.paint_comments()?;
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
                if posted && let Some(body) = &self.body {
                    body.set_value("");
                }
                self.request_comments(next);
            }
            Err(error) => self.comment_error = Some(error),
        }
        self.update_comment_input()?;
        self.paint_comments()?;
        Ok(true)
    }

    pub fn activate(
        &mut self,
        action: &str,
        next: &mut u64,
        images: &mut CloudImages,
    ) -> Result<bool, String> {
        match action {
            "cloud-detail-retry" => {
                self.request_detail(next);
                self.paint_detail(images)?;
            }
            "cloud-gallery-previous" => self.change_image(false, images)?,
            "cloud-gallery-next" => self.change_image(true, images)?,
            "cloud-comment-edited" => self.update_comment_input()?,
            "cloud-post-comment" => {
                if self.comment_mutation.is_some() {
                    return Ok(true);
                }
                let Some(body) = self.body.as_ref().map(HtmlTextAreaElement::value) else {
                    return Ok(true);
                };
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
                self.update_comment_input()?;
                self.paint_comments()?;
            }
            _ => {
                if let Some(index) = action.strip_prefix("cloud-gallery:") {
                    let index: usize = index.parse().map_err(|_| "gallery image index")?;
                    if self.detail.as_ref().is_some_and(|detail| index < detail.screenshots.len()) {
                        self.image_index = index;
                        self.paint_gallery(images)?;
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
                    self.update_comment_input()?;
                    self.paint_comments()?;
                } else {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }

    pub fn change_image(&mut self, next: bool, images: &mut CloudImages) -> Result<(), String> {
        let count = self.detail.as_ref().map_or(0, |detail| detail.screenshots.len());
        if count >= 2 {
            self.image_index = if next {
                (self.image_index + 1) % count
            } else {
                (self.image_index + count - 1) % count
            };
            self.paint_gallery(images)?;
        }
        Ok(())
    }

    pub fn paint_subscription(
        &self,
        subscription: Option<&ModSubscription>,
        busy: bool,
        error: Option<&str>,
    ) -> Result<(), String> {
        self.subscription.set_text_content(None);
        let heading = section_heading(&self.document, &self.subscription, "SUBSCRIPTION")?;
        let state = if self.account.is_none() {
            "GUEST"
        } else {
            subscription.map_or("NOT SUBSCRIBED", |subscription| {
                if subscription.enabled { "ENABLED" } else { "DISABLED" }
            })
        };
        text(&self.document, &heading, "span", "", state, Medium, 0.85, 0xd9ba70, false)?;
        let panel = control_panel(&self.document, &self.subscription, "", false)?;
        if self.account.is_none() {
            control_action(
                &self.document,
                &panel,
                "SIGN IN TO SUBSCRIBE",
                "cloud-sign-in",
                false,
                false,
            )?;
        } else if let Some(subscription) = subscription {
            let (label, action) = if subscription.enabled {
                ("DISABLE MOD", "disable")
            } else {
                ("ENABLE MOD", "enable")
            };
            control_action(
                &self.document,
                &panel,
                if busy { "WORKING..." } else { label },
                &format!("cloud-subscription:{action}:{}", self.summary.slug),
                false,
                busy,
            )?;
            control_action(
                &self.document,
                &panel,
                "UNSUBSCRIBE",
                &format!("cloud-subscription:unsubscribe:{}", self.summary.slug),
                false,
                busy,
            )?;
        } else {
            control_action(
                &self.document,
                &panel,
                if busy { "WORKING..." } else { "SUBSCRIBE" },
                &format!("cloud-subscription:subscribe:{}", self.summary.slug),
                false,
                busy,
            )?;
        }
        if let Some(error) = error {
            narrative(&self.document, &self.subscription, "p", "dark-cloud-inline-error", error)?
                .set_attribute("role", "alert")
                .map_err(|_| "subscription error role")?;
        }
        Ok(())
    }

    pub fn take_actions(&mut self) -> Vec<HostedMenuAction> {
        std::mem::take(&mut self.actions)
    }

    fn paint_detail(&mut self, images: &mut CloudImages) -> Result<(), String> {
        let document = &self.document;
        let summary = self.detail.as_ref().map_or(&self.summary, |detail| &detail.summary);
        self.header.set_text_content(None);
        let titles = element(document, "div", "")?;
        let title = text(document, &titles, "h2", "", &summary.name, Menu, 1.05, 0xd9ba70, true)?;
        title.set_id("dark-cloud-detail-title");
        text(
            document,
            &titles,
            "p",
            "",
            &format!("BY {}", summary.author.username),
            Medium,
            1.0,
            0xd9ba70,
            true,
        )?;
        append(&self.header, &titles)?;
        let close = widgets::button(
            document,
            "BACK",
            "cloud-dialog-close",
            "dark-cloud-detail-close",
            Some(100.0),
            44.0,
            0.55,
            false,
        )?;
        close.set_attribute("data-game-back", "true").map_err(|_| "detail Back action")?;
        close.set_attribute("data-game-default-focus", "true").map_err(|_| "detail Back focus")?;
        close.set_attribute("aria-label", "Close mod details").map_err(|_| "detail Back label")?;
        append(&self.header, &close)?;
        self.load_notice.set_text_content(None);
        self.load_notice.set_hidden(self.detail_error.is_none());
        if let Some(error) = &self.detail_error {
            narrative(document, &self.load_notice, "p", "", error)?;
            append(
                &self.load_notice,
                &widgets::button(
                    document,
                    "RETRY",
                    "cloud-detail-retry",
                    "",
                    None,
                    44.0,
                    0.55,
                    false,
                )?,
            )?;
        }
        self.about.set_text_content(None);
        section_heading(document, &self.about, "ABOUT THIS MOD")?;
        let panel = control_panel(document, &self.about, "", false)?;
        narrative(document, &panel, "p", "dark-cloud-detail-deck", &summary.summary)?;
        let descriptions = element(document, "dl", "")?;
        for (label, value) in [
            ("VERSION:", summary.latest_version.clone()),
            ("DOWNLOADS:", format::count(summary.downloads)?),
            ("PUBLISHED:", format::date(&summary.created_at_utc)?),
            ("UPDATED:", format::time_ago(&summary.updated_at_utc)),
        ] {
            let row = element(document, "div", "")?;
            text(document, &row, "dt", "", label, Medium, 1.0, 0xd9ba70, false)?;
            text(document, &row, "dd", "", &value, Medium, 1.0, 0xd9ba70, true)?;
            append(&descriptions, &row)?;
        }
        append(&panel, &descriptions)?;
        if !summary.tags.is_empty() {
            let tags = element(document, "div", "dark-cloud-detail-tags")?;
            tags.set_attribute("aria-label", "Tags").map_err(|_| "detail tags label")?;
            text(document, &tags, "span", "", "TAGS:", Medium, 1.0, 0xd9ba70, false)?;
            let list = element(document, "span", "dark-cloud-detail-tag-list")?;
            for tag in &summary.tags {
                text(document, &list, "span", "", tag, Medium, 1.0, 0xd9ba70, true)?;
            }
            append(&tags, &list)?;
            append(&panel, &tags)?;
        }
        self.copy.set_text_content(None);
        section_heading(document, &self.copy, "DESCRIPTION")?;
        let panel = control_panel(document, &self.copy, "", false)?;
        narrative(
            document,
            &panel,
            "p",
            "",
            self.detail
                .as_ref()
                .map(|detail| detail.description.as_str())
                .filter(|description| !description.is_empty())
                .unwrap_or(&summary.summary),
        )?;
        self.versions.set_text_content(None);
        let heading = section_heading(document, &self.versions, "VERSION HISTORY")?;
        if let Some(detail) = &self.detail {
            text(
                document,
                &heading,
                "span",
                "",
                &format!(
                    "{} RELEASE{}",
                    detail.versions.len(),
                    if detail.versions.len() == 1 { "" } else { "S" }
                ),
                Medium,
                0.85,
                0xd9ba70,
                false,
            )?;
            let panel = control_panel(document, &self.versions, "", false)?;
            if detail.versions.is_empty() {
                text(
                    document,
                    &panel,
                    "p",
                    "dark-cloud-detail-empty",
                    "NO RELEASES LISTED.",
                    Medium,
                    1.0,
                    0xd9ba70,
                    false,
                )?;
            }
            for version in &detail.versions {
                let article = element(document, "article", "")?;
                let row = element(document, "div", "")?;
                text(
                    document,
                    &row,
                    "strong",
                    "",
                    &format!("v{}", version.version),
                    Medium,
                    1.0,
                    0xd9ba70,
                    true,
                )?;
                for value in [
                    format::date(&version.created_at_utc)?,
                    format::bytes(version.file_size)?,
                    format!("{} DOWNLOADS", format::count(version.downloads)?),
                ] {
                    text(document, &row, "span", "", &value, Medium, 0.85, 0xd9ba70, false)?;
                }
                append(&article, &row)?;
                if !version.changelog.is_empty() {
                    narrative(document, &article, "p", "", &version.changelog)?;
                }
                append(&panel, &article)?;
            }
        } else {
            text(document, &heading, "span", "", "LOADING", Medium, 0.85, 0xd9ba70, false)?;
            let panel = control_panel(document, &self.versions, "", false)?;
            text(
                document,
                &panel,
                "p",
                "dark-cloud-detail-empty",
                "CONSULTING THE DARK CLOUD...",
                Medium,
                1.0,
                0xd9ba70,
                false,
            )?;
        }
        self.paint_gallery(images)
    }

    fn paint_gallery(&self, images: &mut CloudImages) -> Result<(), String> {
        self.gallery.set_text_content(None);
        let document = &self.document;
        let heading = section_heading(document, &self.gallery, "SCREENSHOTS")?;
        let screenshots =
            self.detail.as_ref().map_or(&[][..], |detail| detail.screenshots.as_slice());
        let label = if screenshots.is_empty() {
            "NO IMAGES".to_owned()
        } else {
            format!("{} / {}", self.image_index + 1, screenshots.len())
        };
        text(document, &heading, "span", "", &label, Medium, 0.85, 0xd9ba70, false)?;
        let gallery = element(document, "div", "dark-cloud-gallery-box")?;
        let stage = element(document, "div", "dark-cloud-gallery-stage")?;
        let image = screenshots.get(self.image_index);
        let alt = image.map_or_else(
            || self.summary.name.clone(),
            |_| format!("{} screenshot {}", self.summary.name, self.image_index + 1),
        );
        append(
            &stage,
            &images.media(
                document,
                image.map(|image| image.url.as_str()),
                &alt,
                "dark-cloud-gallery-image",
                true,
                true,
            )?,
        )?;
        append(&gallery, &stage)?;
        let navigation = element(document, "div", "dark-cloud-gallery-navigation")?;
        for (label, action, class, aria) in [
            ("PREV", "cloud-gallery-previous", "dark-cloud-gallery-previous", "PREVIOUS IMAGE"),
            ("NEXT", "cloud-gallery-next", "dark-cloud-gallery-next", "NEXT IMAGE"),
        ] {
            let button = widgets::button(
                document,
                label,
                action,
                class,
                Some(96.0),
                44.0,
                0.55,
                screenshots.len() < 2,
            )?;
            button.set_attribute("aria-label", aria).map_err(|_| "gallery navigation label")?;
            append(&navigation, &button)?;
        }
        append(&gallery, &navigation)?;
        if screenshots.len() > 1 {
            let thumbnails = element(document, "div", "dark-cloud-gallery-thumbnails")?;
            thumbnails
                .set_attribute("aria-label", "Choose screenshot")
                .map_err(|_| "gallery thumbnail label")?;
            for (index, image) in screenshots.iter().enumerate() {
                let button = element(
                    document,
                    "button",
                    if index == self.image_index { "selected" } else { "" },
                )?;
                button.set_attribute("type", "button").map_err(|_| "gallery thumbnail type")?;
                button
                    .set_attribute("data-menu-action", &format!("cloud-gallery:{index}"))
                    .map_err(|_| "gallery thumbnail action")?;
                button
                    .set_attribute("aria-label", &format!("Show screenshot {}", index + 1))
                    .map_err(|_| "gallery thumbnail semantics")?;
                button
                    .set_attribute(
                        "aria-pressed",
                        if index == self.image_index { "true" } else { "false" },
                    )
                    .map_err(|_| "gallery thumbnail selection")?;
                append(
                    &button,
                    &images.media(
                        document,
                        Some(&image.url),
                        &format!("{} screenshot {}", self.summary.name, index + 1),
                        "",
                        false,
                        false,
                    )?,
                )?;
                append(&thumbnails, &button)?;
            }
            append(&gallery, &thumbnails)?;
        }
        append(&self.gallery, &gallery)
    }

    fn paint_comment_form(&mut self) -> Result<(), String> {
        self.comment_form.set_text_content(None);
        self.body = None;
        self.counter = None;
        self.post = None;
        let document = &self.document;
        let panel = control_panel(document, &self.comment_form, "", false)?;
        if self.account.is_none() {
            control_action(document, &panel, "SIGN IN TO COMMENT", "cloud-sign-in", false, false)?;
            return Ok(());
        }
        let form = element(document, "form", "")?;
        form.set_attribute("data-menu-action", "cloud-post-comment")
            .map_err(|_| "comment form action")?;
        let label =
            text(document, &form, "label", "", "LEAVE A COMMENT", Medium, 1.0, 0xd9ba70, false)?;
        label.set_attribute("for", "dark-cloud-comment").map_err(|_| "comment input label")?;
        let field = control_panel(document, &form, "", true)?;
        let body: HtmlTextAreaElement =
            element(document, "textarea", "")?.dyn_into().map_err(|_| "comment input")?;
        body.set_id("dark-cloud-comment");
        body.set_max_length(1000);
        body.set_placeholder("Write a comment...");
        field.append_child(&body).map_err(|_| "comment field parent")?;
        let controls = element(document, "div", "")?;
        let counter = element(document, "span", "")?;
        append(&controls, &counter)?;
        let post =
            control_action(document, &controls, "POST COMMENT", "cloud-post-comment", false, true)?;
        post.set_attribute("type", "submit").map_err(|_| "comment submit type")?;
        append(&form, &controls)?;
        append(&panel, &form)?;
        self.body = Some(body);
        self.counter = Some(counter);
        self.post = Some(post);
        self.update_comment_input()
    }

    pub fn update_comment_input(&self) -> Result<(), String> {
        let Some(body) = &self.body else {
            return Ok(());
        };
        let value = body.value();
        if let Some(counter) = &self.counter {
            counter.set_text_content(None);
            append(
                counter,
                &crate::ui::cloud::cloud_text(
                    &self.document,
                    &format!("{} / 1000", value.encode_utf16().count()),
                    Medium,
                    0.85,
                    0xd9ba70,
                    false,
                )?,
            )?;
        }
        if let Some(post) = &self.post {
            post.toggle_attribute_with_force(
                "disabled",
                self.comment_mutation.is_some()
                    || js_sys::JsString::from(value).trim().length() == 0,
            )
            .map_err(|_| "comment submit admission")?;
            post.set_text_content(None);
            append(
                post,
                &crate::ui::cloud::cloud_text(
                    &self.document,
                    if self.comment_mutation.is_some() { "POSTING..." } else { "POST COMMENT" },
                    Medium,
                    1.0,
                    if post.has_attribute("disabled") { 0x8e7c4d } else { 0xd9ba70 },
                    false,
                )?,
            )?;
        }
        Ok(())
    }

    fn paint_comments(&self) -> Result<(), String> {
        let document = &self.document;
        self.comment_count.set_text_content(None);
        append(
            &self.comment_count,
            &crate::ui::cloud::cloud_text(
                document,
                &self.comments.as_ref().map_or_else(
                    || "LOADING".to_owned(),
                    |comments| format!("{} TOTAL", comments.total),
                ),
                Medium,
                0.85,
                0xd9ba70,
                false,
            )?,
        )?;
        self.comment_notice.set_text_content(self.comment_error.as_deref());
        self.comment_notice.set_hidden(self.comment_error.is_none());
        self.comment_list.set_text_content(None);
        let Some(comments) = &self.comments else {
            return Ok(());
        };
        let list = control_panel(document, &self.comment_list, "dark-cloud-comment-list", false)?;
        if comments.items.is_empty() {
            text(
                document,
                &list,
                "p",
                "dark-cloud-detail-empty",
                "NO COMMENTS YET.",
                Medium,
                1.0,
                0xd9ba70,
                false,
            )?;
        }
        for comment in &comments.items {
            let article = element(document, "article", "")?;
            let header = element(document, "header", "")?;
            text(
                document,
                &header,
                "strong",
                "",
                &comment.author.username,
                Medium,
                1.0,
                0xd9ba70,
                true,
            )?;
            text(
                document,
                &header,
                "span",
                "",
                &format::time_ago(&comment.created_at_utc),
                Medium,
                0.85,
                0xd9ba70,
                false,
            )?;
            if self.can_delete(comment.id) {
                let delete = widgets::button(
                    document,
                    "DELETE",
                    &format!("cloud-delete-comment:{}", comment.id),
                    "",
                    None,
                    44.0,
                    0.55,
                    self.comment_mutation.is_some(),
                )?;
                delete
                    .set_attribute(
                        "aria-label",
                        &format!("Delete comment by {}", comment.author.username),
                    )
                    .map_err(|_| "comment deletion label")?;
                append(&header, &delete)?;
            }
            append(&article, &header)?;
            narrative(document, &article, "p", "", &comment.body)?;
            append(&list, &article)?;
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

fn section_heading(
    document: &Document,
    parent: &HtmlElement,
    title: &str,
) -> Result<HtmlElement, String> {
    let header = element(document, "div", "dark-cloud-section-heading")?;
    text(document, &header, "h3", "", title, Medium, 1.0, 0xd9ba70, false)?;
    append(parent, &header)?;
    Ok(header)
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
