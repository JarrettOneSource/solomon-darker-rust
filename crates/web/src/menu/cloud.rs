//! Website `DarkCloudScene.tsx`: real catalog/directory data, source selection,
//! sheets and detail lifetimes. The Website coordinator owns all I/O and admission.

mod detail;
mod dialog;
mod format;
mod images;
mod layouts;
mod rows;
mod widgets;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use solomon_sim::menu::dark_cloud::{DarkCloudTab as Tab, native_dark_cloud_tab_strip};
use solomon_sim::menu::party_directory::{DirectoryPartyAction, directory_party_action};
use solomon_sim::native_ui_text::NativeUiFontName::{Heading, Medium, Menu};
use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{Document, Element, Event, HtmlElement, HtmlInputElement, KeyboardEvent};

use super::{HostedMenuAction, ModSubscriptionAction, RoutingDownloadProgress};
use crate::hosted::{
    Account, ConnectedGamePlayer, DeveloperGameMatch, ModComments, ModDetail, ModSubscription,
    ModSummary, OwnedImageUrl, PartyActivity, PublicGameParty, SharedMobileUiLayout,
};
use crate::ui::cloud::DarkCloudArt;
use crate::ui::{append, element, style};
use detail::ModDetailView;
use dialog::CloudDialog;
use images::CloudImages;
use layouts::LayoutsView;
use widgets::{control_action, control_panel, narrative, text};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Sort {
    Newest,
    Updated,
    Downloads,
    Name,
    Members,
}
impl Sort {
    fn id(self) -> &'static str {
        match self {
            Self::Newest => "newest",
            Self::Updated => "updated",
            Self::Downloads => "downloads",
            Self::Name => "name",
            Self::Members => "members",
        }
    }
}

enum Row {
    Mod(Box<ModSummary>, Option<Box<ModSubscription>>),
    Party(PublicGameParty),
}
impl Row {
    fn key(&self) -> String {
        match self {
            Self::Mod(content, _) => format!("mod:{}", content.slug),
            Self::Party(party) => format!("party:{}", party.id),
        }
    }
}

enum Sheet {
    Search { dialog: CloudDialog, input: HtmlInputElement },
    Sort(CloudDialog),
}
impl Sheet {
    fn dialog(&self) -> &CloudDialog {
        match self {
            Self::Search { dialog, .. } | Self::Sort(dialog) => dialog,
        }
    }
}

pub(super) struct CloudView {
    pub root: HtmlElement,
    document: Document,
    art: DarkCloudArt,
    scene_art: HtmlElement,
    list_art: HtmlElement,
    heading: HtmlElement,
    tabs: HtmlElement,
    frame: HtmlElement,
    columns: HtmlElement,
    status: HtmlElement,
    list: HtmlElement,
    footer: HtmlElement,
    notice: HtmlElement,
    skull: HtmlElement,
    layouts: LayoutsView,
    boneyards: HtmlElement,
    account: Option<Account>,
    active: bool,
    tab: Tab,
    sort: Sort,
    query: String,
    selected: Option<String>,
    mods: Vec<ModSummary>,
    subscriptions: Vec<ModSubscription>,
    parties: Vec<PublicGameParty>,
    players: Vec<ConnectedGamePlayer>,
    matches: Vec<DeveloperGameMatch>,
    catalog_request: Option<u64>,
    directory_request: Option<u64>,
    presence_request: Option<u64>,
    subscription_request: Option<(u64, String)>,
    join_request: Option<u64>,
    pending_listing: Option<String>,
    observe_request: Option<(u64, String)>,
    mods_error: Option<String>,
    subscriptions_error: Option<String>,
    directory_error: Option<String>,
    presence_error: Option<String>,
    action_error: Option<String>,
    subscription_error: Option<String>,
    detail: Option<ModDetailView>,
    sheet: Option<Sheet>,
    images: CloudImages,
    next_request: u64,
    refresh_at: f64,
    download_progress: Option<(u64, u64, bool)>,
    events: Rc<RefCell<Vec<String>>>,
    gallery_active: Rc<Cell<bool>>,
    actions: Vec<HostedMenuAction>,
    art_size: String,
    _listeners: Vec<Closure<dyn FnMut(Event)>>,
}

impl CloudView {
    pub async fn new(document: &Document) -> Result<Self, String> {
        let root = element(document, "section", "menu-screen dark-cloud-scene")?;
        root.set_attribute("aria-label", "The Dark Cloud").map_err(|_| "Dark Cloud scene label")?;
        root.set_hidden(true);
        let art = DarkCloudArt::load().await?;
        let scene_art = element(document, "div", "dark-cloud-scene-art-host")?;
        append(&root, &scene_art)?;
        let heading = element(document, "header", "dark-cloud-heading")?;
        append(&root, &heading)?;
        let tabs = element(document, "nav", "dark-cloud-tabs")?;
        tabs.set_attribute("aria-label", "Dark Cloud sections")
            .map_err(|_| "Dark Cloud tab label")?;
        append(&root, &tabs)?;
        let frame = element(document, "main", "dark-cloud-list-frame")?;
        let list_art = element(document, "div", "dark-cloud-list-art-host")?;
        append(&frame, &list_art)?;
        let columns = element(document, "div", "dark-cloud-columns")?;
        append(&frame, &columns)?;
        let status = element(document, "div", "dark-cloud-list-status")?;
        append(&frame, &status)?;
        let list = element(document, "div", "dark-cloud-rows")?;
        list.set_attribute("role", "list").map_err(|_| "Dark Cloud list role")?;
        append(&frame, &list)?;
        let layouts = LayoutsView::new(document)?;
        append(&frame, &layouts.root)?;
        let boneyards = element(document, "div", "dark-cloud-boneyards-mount")?;
        boneyards.set_hidden(true);
        append(&frame, &boneyards)?;
        append(&root, &frame)?;
        let footer = element(document, "footer", "dark-cloud-footer")?;
        append(&root, &footer)?;
        let notice = element(document, "p", "dark-cloud-error")?;
        notice.set_attribute("role", "alert").map_err(|_| "Dark Cloud notice role")?;
        append(&root, &notice)?;
        let skull = element(document, "button", "cloud-menu-skull")?;
        for (name, value) in
            [("type", "button"), ("aria-label", "Menu"), ("data-menu-action", "cloud-menu")]
        {
            skull.set_attribute(name, value).map_err(|_| "Dark Cloud menu skull")?;
        }
        append(&root, &skull)?;
        let events = Rc::new(RefCell::new(Vec::new()));
        let mut listeners = Vec::new();
        let double_events = events.clone();
        let double = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
            let Some(target) = event.target().and_then(|target| target.dyn_into::<Element>().ok())
            else {
                return;
            };
            match target.closest("[data-cloud-row-key]") {
                Ok(Some(row)) => {
                    if let Some(key) = row.get_attribute("data-cloud-row-key") {
                        double_events.borrow_mut().push(format!("cloud-open:{key}"));
                    }
                }
                Ok(None) => {}
                Err(error) => web_sys::console::error_1(&error),
            }
        });
        root.add_event_listener_with_callback("dblclick", double.as_ref().unchecked_ref())
            .map_err(|_| "Dark Cloud double click")?;
        listeners.push(double);
        let edited = events.clone();
        let input = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
            let Some(target) = event.target().and_then(|target| target.dyn_into::<Element>().ok())
            else {
                return;
            };
            if target.id() == "dark-cloud-comment" {
                edited.borrow_mut().push("cloud-comment-edited".to_owned());
            }
            if target.id() == "dark-cloud-layout-code" {
                edited.borrow_mut().push("cloud-layout-edited".to_owned());
            }
            if target.id() == "dark-cloud-search-input" {
                edited.borrow_mut().push("cloud-search-edited".to_owned());
            }
        });
        root.add_event_listener_with_callback("input", input.as_ref().unchecked_ref())
            .map_err(|_| "Dark Cloud input listener")?;
        listeners.push(input);
        let failed = events.clone();
        let error = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
            let Some(target) = event.target().and_then(|target| target.dyn_into::<Element>().ok())
            else {
                return;
            };
            match target.closest("[data-cloud-image-url]") {
                Ok(Some(media)) => {
                    if let Some(url) = media.get_attribute("data-cloud-image-url") {
                        failed.borrow_mut().push(format!("cloud-image-failed:{url}"));
                    }
                }
                Ok(None) => {}
                Err(error) => web_sys::console::error_1(&error),
            }
        });
        root.add_event_listener_with_callback_and_bool(
            "error",
            error.as_ref().unchecked_ref(),
            true,
        )
        .map_err(|_| "Dark Cloud media error listener")?;
        listeners.push(error);
        let gallery_active = Rc::new(Cell::new(false));
        let key_active = gallery_active.clone();
        let keyed = events.clone();
        let key =
            Closure::<dyn FnMut(Event)>::new(move |event: Event| {
                if !key_active.get() {
                    return;
                }
                let Some(event) = event.dyn_ref::<KeyboardEvent>() else {
                    return;
                };
                if event.target().and_then(|target| target.dyn_into::<Element>().ok()).is_some_and(
                    |target| matches!(target.tag_name().as_str(), "INPUT" | "TEXTAREA"),
                ) {
                    return;
                }
                if event.key() == "ArrowLeft" {
                    keyed.borrow_mut().push("cloud-gallery-previous".to_owned());
                }
                if event.key() == "ArrowRight" {
                    keyed.borrow_mut().push("cloud-gallery-next".to_owned());
                }
            });
        web_sys::window()
            .ok_or("Dark Cloud has no window")?
            .add_event_listener_with_callback("keydown", key.as_ref().unchecked_ref())
            .map_err(|_| "gallery key listener")?;
        listeners.push(key);
        Ok(Self {
            root,
            document: document.clone(),
            art,
            scene_art,
            list_art,
            heading,
            tabs,
            frame,
            columns,
            status,
            list,
            footer,
            notice,
            skull,
            layouts,
            boneyards,
            account: None,
            active: false,
            tab: Tab::Mods,
            sort: Sort::Newest,
            query: String::new(),
            selected: None,
            mods: Vec::new(),
            subscriptions: Vec::new(),
            parties: Vec::new(),
            players: Vec::new(),
            matches: Vec::new(),
            catalog_request: None,
            directory_request: None,
            presence_request: None,
            subscription_request: None,
            join_request: None,
            pending_listing: None,
            observe_request: None,
            mods_error: None,
            subscriptions_error: None,
            directory_error: None,
            presence_error: None,
            action_error: None,
            subscription_error: None,
            detail: None,
            sheet: None,
            images: CloudImages::default(),
            next_request: 0,
            refresh_at: 0.0,
            download_progress: None,
            events,
            gallery_active,
            actions: Vec::new(),
            art_size: String::new(),
            _listeners: listeners,
        })
    }

    pub fn open(&mut self) -> Result<(), String> {
        self.active = true;
        self.tab = Tab::Mods;
        self.sort = Sort::Newest;
        self.query.clear();
        self.selected = None;
        self.action_error = None;
        self.subscription_error = None;
        self.art_size.clear();
        self.request_catalog();
        self.paint()
    }

    pub fn open_tab(&mut self, tab: Tab, now_ms: f64) -> Result<(), String> {
        self.open()?;
        if tab != Tab::Mods {
            self.activate(&format!("cloud-tab:{}", tab.id()), now_ms)?;
        }
        Ok(())
    }

    /// The retained Website workshop shelf owns its own data and controls.
    /// The coordinator mounts that same-origin surface inside this actual tab.
    pub fn boneyards_mount(&self) -> HtmlElement {
        self.boneyards.clone()
    }

    pub fn boneyards_selected(&self) -> bool {
        self.active && self.tab == Tab::Boneyards
    }

    pub fn close(&mut self) {
        self.active = false;
        self.catalog_request = None;
        self.directory_request = None;
        self.presence_request = None;
        self.subscription_request = None;
        self.observe_request = None;
        self.detail = None;
        self.sheet = None;
        self.gallery_active.set(false);
        self.actions.clear();
        if let Some(request_id) = self.join_request.take() {
            self.actions.push(HostedMenuAction::CancelPartyJoin { request_id });
        }
        self.images.clear();
        self.events.borrow_mut().clear();
        self.layouts.close();
        self.mods.clear();
        self.subscriptions.clear();
        self.parties.clear();
        self.players.clear();
        self.matches.clear();
        self.mods_error = None;
        self.subscriptions_error = None;
        self.directory_error = None;
        self.presence_error = None;
        self.download_progress = None;
    }

    pub fn set_account(&mut self, account: Option<Account>) -> Result<(), String> {
        let changed =
            self.account.as_ref().map(|account| (&account.username, account.developer_access))
                != account.as_ref().map(|account| (&account.username, account.developer_access));
        self.account = account;
        if let Some(detail) = &mut self.detail {
            detail.set_account(self.account.as_ref().map(|account| account.username.as_str()))?;
        }
        self.layouts.set_account(self.account.as_ref().map(|account| account.username.as_str()))?;
        if self.active && changed {
            self.art_size.clear();
            self.request_catalog();
            self.paint()?;
        }
        Ok(())
    }

    fn request_catalog(&mut self) {
        self.next_request += 1;
        self.catalog_request = Some(self.next_request);
        self.actions.push(HostedMenuAction::LoadDarkCloudCatalog { request_id: self.next_request });
    }

    fn request_directory(&mut self, now: f64) {
        self.next_request += 1;
        self.directory_request = Some(self.next_request);
        self.actions.push(HostedMenuAction::LoadParties { request_id: self.next_request });
        self.refresh_at = now + 15_000.0;
        if self.account.as_ref().is_some_and(|account| account.developer_access) {
            self.next_request += 1;
            self.presence_request = Some(self.next_request);
            self.actions
                .push(HostedMenuAction::LoadDeveloperPresence { request_id: self.next_request });
        }
    }

    pub fn update(&mut self, now: f64) -> Result<(), String> {
        if !self.active {
            return Ok(());
        }
        let events = std::mem::take(&mut *self.events.borrow_mut());
        for action in events {
            self.activate(&action, now)?;
        }
        self.layouts.update()?;
        if self.tab == Tab::Parties && now >= self.refresh_at {
            self.request_directory(now);
            self.paint()?;
        }
        self.paint_art()
    }

    pub fn modal_open(&self) -> bool {
        self.detail.is_some() || self.sheet.is_some()
    }

    pub fn active_surface(&self) -> &HtmlElement {
        if let Some(detail) = &self.detail {
            return detail.dialog.element();
        }
        if let Some(sheet) = &self.sheet {
            return sheet.dialog().element();
        }
        &self.root
    }

    pub fn back(&mut self) -> Result<bool, String> {
        if self.modal_open() {
            self.detail = None;
            self.sheet = None;
            self.gallery_active.set(false);
            self.images.finish_paint(&self.root)?;
            return Ok(true);
        }
        Ok(false)
    }

    pub fn activate(&mut self, action: &str, now: f64) -> Result<bool, String> {
        if action == "cloud-dialog-close" {
            self.back()?;
            return Ok(true);
        }
        if action == "cloud-sign-in" {
            self.actions.push(HostedMenuAction::SignIn);
            return Ok(true);
        }
        if let Some(url) = action.strip_prefix("cloud-image-failed:") {
            self.images.failed(&self.document, &self.root, url)?;
            return Ok(true);
        }
        if let Some(detail) = &mut self.detail
            && detail.activate(action, &mut self.next_request, &mut self.images)?
        {
            self.images.finish_paint(&self.root)?;
            return Ok(true);
        }
        if let Some(value) = action.strip_prefix("cloud-subscription:") {
            if let Some((action, slug)) = value.split_once(':')
                && let Some(action) = detail::subscription_action(action)
            {
                self.change_subscription(slug, action)?;
            }
            return Ok(true);
        }
        if action == "cloud-search-submit" {
            if let Some(Sheet::Search { input, .. }) = &self.sheet {
                self.query = input.value();
            }
            self.sheet = None;
            self.paint()?;
            return Ok(true);
        }
        if action == "cloud-search-clear" {
            if let Some(Sheet::Search { input, .. }) = &self.sheet {
                input.set_value("");
                input.focus().map_err(|_| "clear search focus")?;
            }
            self.update_search_clear()?;
            return Ok(true);
        }
        if action == "cloud-search-edited" {
            self.update_search_clear()?;
            return Ok(true);
        }
        if let Some(sort) = action.strip_prefix("cloud-sort:") {
            self.sort = match sort {
                "newest" => Sort::Newest,
                "updated" => Sort::Updated,
                "downloads" => Sort::Downloads,
                "name" => Sort::Name,
                "members" => Sort::Members,
                _ => return Err("unknown Cloud sort".to_owned()),
            };
            self.sheet = None;
            self.paint()?;
            return Ok(true);
        }
        if self.modal_open() {
            return Ok(true);
        }
        if self.layouts.activate(action, &mut self.next_request)? {
            return Ok(true);
        }
        if let Some(tab) = action.strip_prefix("cloud-tab:") {
            self.art_size.clear();
            if self.tab == Tab::Layouts {
                self.layouts.close();
            }
            self.tab = Tab::from_id(tab).ok_or("unknown Cloud section")?;
            self.query.clear();
            self.sheet = None;
            self.selected = match self.tab {
                Tab::Mods => self.mods.first().map(|content| format!("mod:{}", content.slug)),
                Tab::Subscribed => self
                    .subscriptions
                    .first()
                    .map(|subscription| format!("mod:{}", subscription.content.slug)),
                Tab::Parties => self.parties.first().map(|party| format!("party:{}", party.id)),
                Tab::Layouts | Tab::Boneyards => None,
            };
            if self.tab == Tab::Layouts {
                self.layouts.open()?;
            }
            self.sort = if self.tab == Tab::Parties { Sort::Members } else { Sort::Newest };
            if self.tab == Tab::Parties {
                self.request_directory(now);
            } else {
                self.directory_request = None;
                self.presence_request = None;
            }
            self.paint()?;
            return Ok(true);
        }
        if let Some(key) = action.strip_prefix("cloud-select:") {
            self.selected = Some(key.to_owned());
            self.paint_selection()?;
            return Ok(true);
        }
        if let Some(key) = action.strip_prefix("cloud-open:") {
            self.selected = Some(key.to_owned());
            self.primary()?;
            return Ok(true);
        }
        if let Some(slug) = action.strip_prefix("cloud-open-mod:") {
            self.open_mod(slug)?;
            return Ok(true);
        }
        if let Some(id) = action.strip_prefix("cloud-join-party:") {
            self.join_party(id)?;
            return Ok(true);
        }
        if let Some(id) = action.strip_prefix("cloud-observe:") {
            if self.observe_request.is_none()
                && self.account.as_ref().is_some_and(|account| account.developer_access)
                && self.matches.iter().any(|game| game.id == id)
            {
                self.next_request += 1;
                self.observe_request = Some((self.next_request, id.to_owned()));
                self.action_error = None;
                self.actions.push(HostedMenuAction::ObserveMatch {
                    request_id: self.next_request,
                    match_id: id.to_owned(),
                });
                self.paint()?;
            }
            return Ok(true);
        }
        match action {
            "cloud-primary" => self.primary()?,
            "cloud-refresh" => {
                if self.tab == Tab::Parties {
                    self.request_directory(now);
                } else {
                    self.request_catalog();
                }
                self.paint()?;
            }
            "cloud-clear-query" => {
                self.query.clear();
                self.paint()?;
            }
            "cloud-search" => self.show_search()?,
            "cloud-sort" => self.show_sort()?,
            _ => return Ok(false),
        }
        Ok(true)
    }

    fn primary(&mut self) -> Result<(), String> {
        let selected =
            self.filtered_rows().into_iter().find(|row| Some(row.key()) == self.selected);
        match selected {
            Some(Row::Mod(content, _)) => self.open_mod(&content.slug)?,
            Some(Row::Party(party)) => self.join_party(&party.id)?,
            None => {}
        }
        Ok(())
    }

    fn open_mod(&mut self, slug: &str) -> Result<(), String> {
        let Some(content) = self
            .mods
            .iter()
            .find(|content| content.slug == slug)
            .or_else(|| {
                self.subscriptions
                    .iter()
                    .find(|subscription| subscription.content.slug == slug)
                    .map(|subscription| &subscription.content)
            })
            .cloned()
        else {
            return Ok(());
        };
        self.selected = Some(format!("mod:{slug}"));
        self.sheet = None;
        self.detail = Some(ModDetailView::open(
            &self.document,
            &self.root,
            content,
            self.account.as_ref().map(|account| account.username.as_str()),
            self.events.clone(),
            &mut self.next_request,
            &mut self.images,
        )?);
        self.gallery_active.set(true);
        self.paint_subscription()?;
        self.images.finish_paint(&self.root)?;
        Ok(())
    }

    fn change_subscription(
        &mut self,
        slug: &str,
        action: ModSubscriptionAction,
    ) -> Result<(), String> {
        if self.subscription_request.is_some() {
            return Ok(());
        }
        if self.account.is_none() {
            self.actions.push(HostedMenuAction::SignIn);
            return Ok(());
        }
        if !self.mods.iter().any(|content| content.slug == slug)
            && !self.subscriptions.iter().any(|subscription| subscription.content.slug == slug)
        {
            return Ok(());
        }
        self.next_request += 1;
        self.subscription_request = Some((self.next_request, slug.to_owned()));
        self.action_error = None;
        self.subscription_error = None;
        self.download_progress = None;
        self.actions.push(HostedMenuAction::ChangeModSubscription {
            request_id: self.next_request,
            slug: slug.to_owned(),
            action,
        });
        self.paint()?;
        self.paint_subscription()
    }

    fn join_party(&mut self, id: &str) -> Result<(), String> {
        if self.join_request.is_some() {
            return Ok(());
        }
        let Some(party) = self.parties.iter().find(|party| party.id == id) else {
            return Ok(());
        };
        let action = directory_party_action(
            party.visibility,
            matches!(party.activity, PartyActivity::Playing { .. }),
        );
        if action == DirectoryPartyAction::Wait {
            return Ok(());
        }
        self.next_request += 1;
        self.join_request = Some(self.next_request);
        self.action_error = None;
        self.pending_listing = (action == DirectoryPartyAction::Request).then(|| id.to_owned());
        self.actions.push(if action == DirectoryPartyAction::Request {
            HostedMenuAction::RequestPartyInvite {
                request_id: self.next_request,
                listing_id: id.to_owned(),
            }
        } else {
            HostedMenuAction::JoinPublicParty {
                request_id: self.next_request,
                listing_id: id.to_owned(),
            }
        });
        self.paint()
    }

    pub fn complete_catalog(
        &mut self,
        id: u64,
        mods: Result<Vec<ModSummary>, String>,
        subscriptions: Result<Vec<ModSubscription>, String>,
    ) -> Result<bool, String> {
        let mutation =
            self.subscription_request.as_ref().is_some_and(|(pending, _)| *pending == id);
        if !self.active || (self.catalog_request != Some(id) && !mutation) {
            return Ok(false);
        }
        self.catalog_request = None;
        match mods {
            Ok(mods) => {
                self.mods = mods;
                self.mods_error = None;
            }
            Err(error) => self.mods_error = Some(error),
        }
        match subscriptions {
            Ok(subscriptions) => {
                self.subscriptions = subscriptions;
                self.subscriptions_error = None;
            }
            Err(error) => self.subscriptions_error = Some(error),
        }
        self.paint()?;
        self.paint_subscription()?;
        Ok(true)
    }

    pub fn complete_directory(
        &mut self,
        id: u64,
        result: Result<Vec<PublicGameParty>, String>,
    ) -> Result<bool, String> {
        if !self.active || self.tab != Tab::Parties || self.directory_request != Some(id) {
            return Ok(false);
        }
        self.directory_request = None;
        match result {
            Ok(parties) => {
                self.parties = parties;
                self.directory_error = None;
            }
            Err(error) => self.directory_error = Some(error),
        }
        self.paint()?;
        Ok(true)
    }

    pub fn complete_presence(
        &mut self,
        id: u64,
        result: Result<(Vec<ConnectedGamePlayer>, Vec<DeveloperGameMatch>), String>,
    ) -> Result<bool, String> {
        if !self.active || self.tab != Tab::Parties || self.presence_request != Some(id) {
            return Ok(false);
        }
        self.presence_request = None;
        match result {
            Ok((players, matches)) => {
                self.players = players;
                self.matches = matches;
                self.presence_error = None;
            }
            Err(error) => self.presence_error = Some(error),
        }
        self.paint()?;
        Ok(true)
    }

    pub fn complete_join(&mut self, id: u64, result: Result<(), String>) -> Result<bool, String> {
        if !self.active || self.join_request != Some(id) {
            return Ok(false);
        }
        self.join_request = None;
        self.pending_listing = None;
        self.action_error = result.err();
        self.paint()?;
        Ok(true)
    }

    pub fn complete_observe(
        &mut self,
        id: u64,
        result: Result<(), String>,
    ) -> Result<bool, String> {
        if !self.active || self.observe_request.as_ref().is_none_or(|(pending, _)| *pending != id) {
            return Ok(false);
        }
        if let Err(error) = result {
            self.observe_request = None;
            self.action_error = Some(error);
            self.paint()?;
        }
        Ok(true)
    }

    pub fn complete_subscription(
        &mut self,
        id: u64,
        result: Result<(), String>,
    ) -> Result<bool, String> {
        if !self.active
            || self.subscription_request.as_ref().is_none_or(|(pending, _)| *pending != id)
        {
            return Ok(false);
        }
        self.subscription_request = None;
        self.download_progress = None;
        self.subscription_error = result.err();
        self.action_error = self.subscription_error.clone();
        self.paint()?;
        self.paint_subscription()?;
        Ok(true)
    }

    pub fn complete_detail(
        &mut self,
        id: u64,
        result: Result<ModDetail, String>,
    ) -> Result<bool, String> {
        let Some(detail) = &mut self.detail else {
            return Ok(false);
        };
        let accepted = detail.complete_detail(id, result, &mut self.images)?;
        self.images.finish_paint(&self.root)?;
        Ok(accepted)
    }

    pub fn complete_comments(
        &mut self,
        id: u64,
        result: Result<ModComments, String>,
    ) -> Result<bool, String> {
        self.detail.as_mut().map_or(Ok(false), |detail| detail.complete_comments(id, result))
    }

    pub fn complete_comment_mutation(
        &mut self,
        id: u64,
        result: Result<(), String>,
    ) -> Result<bool, String> {
        self.detail.as_mut().map_or(Ok(false), |detail| {
            detail.complete_comment_mutation(id, result, &mut self.next_request)
        })
    }

    pub fn complete_image(
        &mut self,
        id: u64,
        result: Result<OwnedImageUrl, String>,
    ) -> Result<bool, String> {
        self.images.complete(&self.document, &self.root, id, result)
    }

    pub fn set_progress(
        &mut self,
        id: u64,
        progress: Option<RoutingDownloadProgress<'_>>,
    ) -> Result<bool, String> {
        if self.subscription_request.as_ref().is_none_or(|(pending, _)| *pending != id) {
            return Ok(false);
        }
        self.download_progress = progress.map(|progress| {
            (progress.completed_bytes, progress.total_bytes, progress.active_mod_id.is_some())
        });
        self.paint_footer(self.filtered_rows().len())?;
        Ok(true)
    }

    pub fn complete_layout(
        &mut self,
        id: u64,
        result: Result<SharedMobileUiLayout, String>,
    ) -> Result<bool, String> {
        self.layouts.complete(id, result)
    }
    pub fn set_mobile_layout_customized(&mut self, customized: bool) -> Result<(), String> {
        self.layouts.set_customized(customized)
    }

    pub fn take_actions(&mut self) -> Vec<HostedMenuAction> {
        let mut actions = std::mem::take(&mut self.actions);
        actions.extend(self.images.take_actions());
        actions.extend(self.layouts.take_actions());
        if let Some(detail) = &mut self.detail {
            actions.extend(detail.take_actions());
        }
        actions
    }

    /// Settings and the catalog share one completion route for mobile publication.
    /// Reserve from this same sequence so overlapping live views cannot alias receipts.
    pub(super) fn reserve_layout_request(&mut self) -> u64 {
        self.next_request += 1;
        self.next_request
    }

    fn paint_subscription(&self) -> Result<(), String> {
        if let Some(detail) = &self.detail {
            let subscription = self
                .subscriptions
                .iter()
                .find(|subscription| subscription.content.slug == detail.summary.slug);
            detail.paint_subscription(
                subscription,
                self.subscription_request.is_some(),
                self.subscription_error.as_deref(),
            )?;
        }
        Ok(())
    }

    fn filtered_rows(&self) -> Vec<Row> {
        let query: String = js_sys::JsString::from(self.query.as_str()).trim().into();
        let query = format::lowercase(&query);
        let mut rows = Vec::new();
        if self.tab == Tab::Parties {
            for party in &self.parties {
                let mut values = vec![party.leader.as_str()];
                for member in &party.members {
                    values.push(member);
                }
                if let PartyActivity::Playing { boneyard_name } = &party.activity {
                    values.push(boneyard_name);
                }
                if query.is_empty()
                    || values.iter().any(|value| format::lowercase(value).contains(&query))
                {
                    rows.push(Row::Party(party.clone()));
                }
            }
        } else if !matches!(self.tab, Tab::Layouts | Tab::Boneyards) {
            let source: Vec<&ModSummary> = if self.tab == Tab::Subscribed {
                self.subscriptions.iter().map(|subscription| &subscription.content).collect()
            } else {
                self.mods.iter().collect()
            };
            for content in source {
                let mut values = vec![
                    content.name.as_str(),
                    content.author.username.as_str(),
                    content.summary.as_str(),
                ];
                for tag in &content.tags {
                    values.push(tag);
                }
                if query.is_empty()
                    || values.iter().any(|value| format::lowercase(value).contains(&query))
                {
                    let subscription = self
                        .subscriptions
                        .iter()
                        .find(|subscription| subscription.content.slug == content.slug)
                        .cloned();
                    rows.push(Row::Mod(Box::new(content.clone()), subscription.map(Box::new)));
                }
            }
        }
        rows.sort_by(|first, second| match (first, second) {
            (Row::Party(first), Row::Party(second)) => {
                if self.sort == Sort::Members {
                    second.member_count.cmp(&first.member_count)
                } else {
                    format::compare(&first.leader, &second.leader)
                }
            }
            (Row::Mod(first, _), Row::Mod(second, _)) => match self.sort {
                Sort::Name => format::compare(&first.name, &second.name),
                Sort::Downloads => second.downloads.cmp(&first.downloads),
                Sort::Updated => format::compare(&second.updated_at_utc, &first.updated_at_utc),
                _ => format::compare(&second.created_at_utc, &first.created_at_utc),
            },
            _ => std::cmp::Ordering::Equal,
        });
        rows
    }

    fn paint(&mut self) -> Result<(), String> {
        let focus = widgets::focused_action(&self.document, &self.list);
        let rows = self.filtered_rows();
        if !rows.iter().any(|row| Some(row.key()) == self.selected) {
            self.selected = rows.first().map(Row::key);
        }
        let standalone = matches!(self.tab, Tab::Layouts | Tab::Boneyards);
        self.columns.set_hidden(standalone);
        self.status.set_hidden(standalone);
        self.list.set_hidden(standalone);
        self.layouts.root.set_hidden(self.tab != Tab::Layouts);
        self.boneyards.set_hidden(self.tab != Tab::Boneyards);
        self.columns
            .set_class_name(&format!("dark-cloud-columns dark-cloud-columns-{}", self.tab.id()));
        self.columns.set_text_content(None);
        let labels = match self.tab {
            Tab::Parties => ["party", "wizards", "status", "location", "action"],
            Tab::Subscribed => ["subscribed mod", "author", "version", "status", "manage"],
            _ => ["mod", "author", "version", "status", "action"],
        };
        for (index, label) in labels.into_iter().enumerate() {
            let column = element(&self.document, "span", "")?;
            if self.tab == Tab::Parties && index == 4 {
                append(
                    &column,
                    &widgets::button(
                        &self.document,
                        "REFRESH",
                        "cloud-refresh",
                        "",
                        Some(100.0),
                        44.0,
                        0.55,
                        false,
                    )?,
                )?;
            } else {
                append(
                    &column,
                    &crate::ui::cloud::cloud_text(
                        &self.document,
                        label,
                        Menu,
                        1.0,
                        0xd9ba70,
                        false,
                    )?,
                )?;
            }
            append(&self.columns, &column)?;
        }
        self.paint_status(&self.status, rows.len())?;
        self.list.set_text_content(None);
        self.list
            .set_attribute("aria-label", &format!("{} entries", self.tab.id()))
            .map_err(|_| "Cloud list label")?;
        self.list
            .set_attribute("aria-busy", if self.loading() { "true" } else { "false" })
            .map_err(|_| "Cloud list loading")?;
        if self.loading() && rows.is_empty() {
            text(
                &self.document,
                &self.list,
                "p",
                "dark-cloud-empty",
                "CONSULTING THE DARK CLOUD...",
                Menu,
                0.68,
                0xd9ba70,
                false,
            )?;
        } else if rows.is_empty() && !standalone {
            if let Some(error) = self.active_error() {
                let notice =
                    element(&self.document, "div", "dark-cloud-empty dark-cloud-empty-error")?;
                notice.set_attribute("role", "alert").map_err(|_| "catalog error role")?;
                narrative(&self.document, &notice, "p", "", error)?;
                append(
                    &notice,
                    &widgets::button(
                        &self.document,
                        "RETRY",
                        "cloud-refresh",
                        "",
                        None,
                        44.0,
                        0.55,
                        false,
                    )?,
                )?;
                append(&self.list, &notice)?;
            } else {
                let empty = if !self.query.is_empty() {
                    "NOTHING MATCHES YOUR SEARCH."
                } else {
                    match self.tab {
                        Tab::Parties => "NO PUBLIC PARTIES.",
                        Tab::Subscribed if self.account.is_none() => "SIGN IN TO SEE YOUR MODS.",
                        Tab::Subscribed => "NO SUBSCRIBED MODS.",
                        _ => "NO MODS HAVE REACHED THE DARK CLOUD.",
                    }
                };
                text(
                    &self.document,
                    &self.list,
                    "p",
                    "dark-cloud-empty",
                    empty,
                    Menu,
                    0.68,
                    0xd9ba70,
                    false,
                )?;
            }
        }
        for row in &rows {
            let selected = Some(row.key()) == self.selected;
            let node = match row {
                Row::Mod(content, subscription) => rows::mod_row(
                    &self.document,
                    content,
                    subscription.as_deref(),
                    selected,
                    self.subscription_request.is_some(),
                    self.tab == Tab::Subscribed,
                    &mut self.images,
                )?,
                Row::Party(party) => rows::party_row(
                    &self.document,
                    party,
                    selected,
                    self.join_request.is_some(),
                    self.pending_listing.as_deref() == Some(party.id.as_str()),
                )?,
            };
            append(&self.list, &node)?;
        }
        if self.tab == Tab::Parties
            && self.account.as_ref().is_some_and(|account| account.developer_access)
        {
            append(
                &self.list,
                &rows::developer_presence(
                    &self.document,
                    rows::PresenceSnapshot {
                        players: &self.players,
                        matches: &self.matches,
                        loading: self.presence_request.is_some(),
                        error: self.presence_error.as_deref(),
                        observing: self.observe_request.as_ref().map(|(_, id)| id.as_str()),
                    },
                )?,
            )?;
        }
        self.paint_footer(rows.len())?;
        self.notice.set_text_content(self.action_error.as_deref());
        self.notice.set_hidden(self.action_error.is_none());
        self.images.finish_paint(&self.root)?;
        widgets::restore_focus(&self.document, &self.list, focus.as_deref())?;
        Ok(())
    }

    fn paint_selection(&self) -> Result<(), String> {
        let nodes = self
            .list
            .query_selector_all("[data-cloud-row-key]")
            .map_err(|_| "Cloud selected rows")?;
        for index in 0..nodes.length() {
            let button: HtmlElement = nodes
                .item(index)
                .ok_or("Cloud selected row disappeared")?
                .dyn_into()
                .map_err(|_| "Cloud selected row target")?;
            let selected = button.get_attribute("data-cloud-row-key") == self.selected;
            button
                .set_attribute("aria-pressed", if selected { "true" } else { "false" })
                .map_err(|_| "Cloud row selection")?;
            if let Some(row) = button.parent_element() {
                let base = if row.has_attribute("data-mod-slug") {
                    "dark-cloud-row dark-cloud-mod-row"
                } else {
                    "dark-cloud-row dark-cloud-party-row"
                };
                row.set_attribute(
                    "class",
                    &format!("{base}{}", if selected { " selected" } else { "" }),
                )
                .map_err(|_| "Cloud selected row appearance")?;
            }
            if let Some(title) = button
                .query_selector(".dark-cloud-row-copy > strong")
                .map_err(|_| "Cloud selected row title")?
            {
                let nodes = title
                    .query_selector_all(".native-ui-glyph, .native-ui-content-text")
                    .map_err(|_| "Cloud selected title glyphs")?;
                for index in 0..nodes.length() {
                    let node: HtmlElement = nodes
                        .item(index)
                        .ok_or("Cloud title glyph disappeared")?
                        .dyn_into()
                        .map_err(|_| "Cloud title glyph")?;
                    let color = if selected { "#bfffbf" } else { "#d9ba70" };
                    let property = if node.class_list().contains("native-ui-glyph") {
                        "background-color"
                    } else {
                        "color"
                    };
                    node.style()
                        .set_property(property, color)
                        .map_err(|_| "Cloud selected title tint")?;
                }
            }
        }
        self.paint_footer(self.filtered_rows().len())
    }

    fn loading(&self) -> bool {
        if self.tab == Tab::Parties {
            self.directory_request.is_some()
        } else {
            self.catalog_request.is_some()
        }
    }
    fn active_error(&self) -> Option<&str> {
        match self.tab {
            Tab::Mods => self.mods_error.as_deref(),
            Tab::Subscribed => self.subscriptions_error.as_deref(),
            Tab::Parties => self.directory_error.as_deref(),
            Tab::Layouts | Tab::Boneyards => None,
        }
    }

    fn paint_status(&self, parent: &HtmlElement, count: usize) -> Result<(), String> {
        parent.set_text_content(None);
        let label = if self.tab == Tab::Layouts {
            "MOBILE UI LAYOUTS".to_owned()
        } else if self.loading() && count == 0 {
            "CONSULTING THE DARK CLOUD...".to_owned()
        } else if !self.query.is_empty() {
            format!(
                "\"{}\", {count} {}",
                self.query.to_uppercase(),
                if count == 1 { "MATCH" } else { "MATCHES" }
            )
        } else {
            match self.tab {
                Tab::Parties => format!("{count} {}", if count == 1 { "PARTY" } else { "PARTIES" }),
                Tab::Subscribed => format!("{count} SUBSCRIBED"),
                _ => format!("{count} {}", if count == 1 { "MOD" } else { "MODS" }),
            }
        };
        text(
            &self.document,
            parent,
            "span",
            "dark-cloud-status-label",
            &label,
            Medium,
            0.85,
            0xd9ba70,
            true,
        )?;
        if self.loading() && count > 0 {
            text(
                &self.document,
                parent,
                "span",
                "dark-cloud-status-note",
                "REFRESHING...",
                Medium,
                0.85,
                0xd9ba70,
                false,
            )?;
        }
        if !self.query.is_empty() {
            append(
                parent,
                &widgets::button(
                    &self.document,
                    "CLEAR SEARCH",
                    "cloud-clear-query",
                    "",
                    None,
                    44.0,
                    0.55,
                    false,
                )?,
            )?;
        }
        if self.tab == Tab::Parties {
            let refresh = widgets::button(
                &self.document,
                "REFRESH",
                "cloud-refresh",
                "",
                None,
                44.0,
                0.55,
                false,
            )?;
            refresh
                .set_attribute("data-dark-cloud-refresh", "")
                .map_err(|_| "Cloud inline refresh marker")?;
            append(parent, &refresh)?;
        }
        Ok(())
    }

    fn paint_footer(&self, count: usize) -> Result<(), String> {
        self.footer.set_text_content(None);
        if let Some((completed, total, active)) =
            self.download_progress.filter(|(_, total, _)| *total > 0)
        {
            let percent = (completed as f64 / total as f64 * 100.0 + 0.5).floor().min(100.0);
            let progress =
                control_panel(&self.document, &self.footer, "dark-cloud-download", true)?;
            for (name, value) in [
                ("role", "progressbar".to_owned()),
                ("aria-label", "Downloading mod content".to_owned()),
                ("aria-valuemin", "0".to_owned()),
                ("aria-valuemax", "100".to_owned()),
                ("aria-valuenow", percent.to_string()),
            ] {
                progress.set_attribute(name, &value).map_err(|_| "Cloud progress values")?;
            }
            let track = element(&self.document, "div", "dark-cloud-download-track")?;
            let fill = element(&self.document, "span", "dark-cloud-download-fill")?;
            style(&fill, &format!("width:{percent}%"))?;
            append(&track, &fill)?;
            append(&progress, &track)?;
            text(
                &self.document,
                &progress,
                "small",
                "",
                &format!(
                    "{}, {percent}%",
                    if active { "DOWNLOADING MOD CONTENT" } else { "CONTENT READY" }
                ),
                Medium,
                1.0,
                0xd9ba70,
                false,
            )?;
        }
        if matches!(self.tab, Tab::Layouts | Tab::Boneyards) {
            return Ok(());
        }
        let tools = element(&self.document, "div", "dark-cloud-footer-tools")?;
        for (label, action, icon) in [("Search", "cloud-search", 58), ("Sort", "cloud-sort", 66)] {
            let button = self.tool_button(label, action, "", false)?;
            button
                .set_attribute("data-cloud-tool-icon", &icon.to_string())
                .map_err(|_| "Cloud tool icon record")?;
            append(&tools, &button)?;
        }
        append(&self.footer, &tools)?;
        let selected =
            self.filtered_rows().into_iter().find(|row| Some(row.key()) == self.selected);
        let (label, disabled) = if self.tab == Tab::Parties {
            match &selected {
                Some(Row::Party(party)) => {
                    let action = directory_party_action(
                        party.visibility,
                        matches!(party.activity, PartyActivity::Playing { .. }),
                    );
                    (
                        match action {
                            DirectoryPartyAction::Join => "JOIN PARTY",
                            DirectoryPartyAction::Request => "REQUEST TO JOIN",
                            DirectoryPartyAction::Wait => "IN GAME",
                        },
                        action == DirectoryPartyAction::Wait || self.join_request.is_some(),
                    )
                }
                _ => ("SELECT PARTY", true),
            }
        } else {
            ("VIEW MOD", !matches!(selected, Some(Row::Mod(_, _))))
        };
        let primary = element(&self.document, "div", "dark-cloud-primary-control")?;
        let button = widgets::button(
            &self.document,
            label,
            "cloud-primary",
            "dark-cloud-primary-button",
            None,
            69.0,
            1.0,
            disabled,
        )?;
        // The responsive primary button inherits its wrapper's authored physical
        // height; its glyph/chrome scale follows that measured height.
        button.set_attribute("data-cloud-primary", "").map_err(|_| "Cloud primary geometry")?;
        style(&button, "width:100%;height:100%;left:0;top:0")?;
        append(&primary, &button)?;
        append(&self.footer, &primary)?;
        append(
            &self.footer,
            &self.tool_button(
                "OPTIONS",
                "cloud-primary",
                "dark-cloud-options-button",
                !matches!(selected, Some(Row::Mod(_, _))),
            )?,
        )?;
        let status = element(&self.document, "div", "dark-cloud-footer-status")?;
        self.paint_status(&status, count)?;
        append(&self.footer, &status)?;
        Ok(())
    }

    fn tool_button(
        &self,
        label: &str,
        action: &str,
        class: &str,
        disabled: bool,
    ) -> Result<HtmlElement, String> {
        let button = element(
            &self.document,
            "button",
            &format!("dark-cloud-tool-button cloud-tool-button {class}"),
        )?;
        for (name, value) in [
            ("type", "button"),
            ("aria-label", label),
            ("data-menu-action", action),
            ("data-cloud-label", label),
        ] {
            button.set_attribute(name, value).map_err(|_| "Cloud tool button")?;
        }
        button
            .toggle_attribute_with_force("disabled", disabled)
            .map_err(|_| "Cloud tool admission")?;
        Ok(button)
    }

    fn paint_art(&mut self) -> Result<(), String> {
        let bounds = self.root.get_bounding_client_rect();
        if bounds.width() <= 0.0 || bounds.height() <= 0.0 {
            return Ok(());
        }
        let tabs = self.tabs.get_bounding_client_rect();
        let frame = self.frame.get_bounding_client_rect();
        let key = format!(
            "{},{},{},{},{},{}",
            bounds.width(),
            bounds.height(),
            tabs.width(),
            tabs.height(),
            frame.width(),
            frame.height()
        );
        if self.art_size != key {
            self.scene_art.set_text_content(None);
            append(
                &self.scene_art,
                &self.art.scene(&self.document, bounds.width(), bounds.height())?,
            )?;
            self.list_art.set_text_content(None);
            self.art.list_frame(&self.document, &self.list_art)?;
            self.paint_heading()?;
            self.tabs.set_text_content(None);
            let strip = native_dark_cloud_tab_strip(tabs.width(), tabs.height());
            let plan = element(&self.document, "div", "dark-cloud-tabs-plan")?;
            plan.set_attribute("role", "tablist").map_err(|_| "Cloud tab list role")?;
            plan.set_attribute("aria-label", "Dark Cloud sections")
                .map_err(|_| "Cloud tab list label")?;
            style(
                &plan,
                &format!("position:relative;width:{}px;height:{}px", strip.width, strip.height),
            )?;
            append(&self.tabs, &plan)?;
            let mut selected = None;
            for tab in Tab::ALL {
                let bounds = strip.bounds(tab);
                let button = element(&self.document, "button", "cloud-tab")?;
                for (name, value) in [
                    ("type", "button"),
                    ("role", "tab"),
                    ("aria-label", tab.label()),
                    ("data-menu-action", &format!("cloud-tab:{}", tab.id())),
                    ("aria-selected", if self.tab == tab { "true" } else { "false" }),
                ] {
                    button.set_attribute(name, value).map_err(|_| "Cloud tab semantics")?;
                }
                style(
                    &button,
                    &format!(
                        "position:absolute;left:{}px;top:0;width:{}px;height:{}px",
                        bounds.x, bounds.width, bounds.height,
                    ),
                )?;
                append(
                    &button,
                    &self.art.tab(
                        &self.document,
                        tab.label(),
                        bounds.width,
                        bounds.height,
                        strip.scale,
                        self.tab == tab,
                    )?,
                )?;
                append(&plan, &button)?;
                if self.tab == tab {
                    selected = Some(button);
                }
            }
            // NativeDarkCloudTabs reveals the selected band without disturbing
            // manual scrolling while the band is already inside the viewport.
            if let Some(selected) = selected {
                let left = selected.offset_left();
                let right = left + selected.offset_width();
                if left < self.tabs.scroll_left() {
                    self.tabs.set_scroll_left(left);
                } else if right > self.tabs.scroll_left() + self.tabs.client_width() {
                    self.tabs.set_scroll_left(right - self.tabs.client_width());
                }
            }
            self.skull.set_text_content(None);
            append(&self.skull, &self.art.menu_skull(&self.document)?)?;
            self.art_size = key;
        }
        if let Some(sheet) = &self.sheet {
            self.paint_dialog_art(sheet.dialog())?;
        }
        if let Some(detail) = &self.detail {
            self.paint_dialog_art(&detail.dialog)?;
        }
        if let Some(primary) =
            self.footer.query_selector("[data-cloud-primary]").map_err(|_| "Cloud primary art")?
        {
            let height = primary.get_bounding_client_rect().height();
            primary
                .set_attribute("data-cloud-scale", &(height / 69.0).to_string())
                .map_err(|_| "Cloud primary scale")?;
        }
        widgets::paint_art(&self.document, &self.root, &self.art)
    }

    fn paint_heading(&self) -> Result<(), String> {
        self.heading.set_text_content(None);
        let title = element(&self.document, "h1", "")?;
        append(&self.heading, &title)?;
        let size = title.get_bounding_client_rect();
        let scale = 1.0_f64.min(size.width() / 560.0).min(size.height() / 58.0);
        title
            .set_attribute("aria-label", "THE DARK CLOUD")
            .map_err(|_| "Cloud heading semantics")?;
        widgets::baseline(
            &self.document,
            &title,
            "THE DARK CLOUD",
            Heading,
            scale,
            size.width() / 2.0,
            50.0 * scale,
        )?;
        let beta = element(&self.document, "span", "dark-cloud-beta")?;
        widgets::baseline(
            &self.document,
            &beta,
            "beta",
            Menu,
            scale,
            size.width() / 2.0 + 235.0 * scale,
            50.0 * scale,
        )?;
        append(&title, &beta)?;
        let account = element(&self.document, "button", "")?;
        account.set_attribute("type", "button").map_err(|_| "Cloud account button type")?;
        account
            .set_attribute(
                "data-menu-action",
                if self.account.is_some() { "cloud-menu" } else { "cloud-sign-in" },
            )
            .map_err(|_| "Cloud account action")?;
        let label = self.account.as_ref().map_or_else(
            || "You are signed in as a GUEST.".to_owned(),
            |account| format!("You are signed in as {}.", account.username),
        );
        account.set_attribute("aria-label", &label).map_err(|_| "Cloud account label")?;
        append(&self.heading, &account)?;
        let bounds = account.get_bounding_client_rect();
        // Compact landscape hides this control. Source's element-size hook ignores
        // zero boxes; omit hidden ink here and measure it afresh when shown again.
        if bounds.width() <= 0.0 || bounds.height() <= 0.0 {
            return Ok(());
        }
        let scale = 1.0_f64.min(bounds.width() / 560.0);
        let x = bounds.width() / 2.0;
        widgets::baseline(&self.document, &account, &label, Menu, scale, x, 25.0)?;
        if self.account.is_none() {
            widgets::baseline(
                &self.document,
                &account,
                "To change this, click here.",
                Menu,
                scale,
                x,
                25.0 + 18.0 * scale,
            )?;
            let underline = element(&self.document, "i", "native-dark-cloud-account-underline")?;
            style(
                &underline,
                &format!(
                    "left:{}px;top:{}px;width:{}px",
                    x + 34.0 * scale,
                    25.0 + 20.0 * scale,
                    145.0 * scale
                ),
            )?;
            append(&account, &underline)?;
        }
        Ok(())
    }

    fn paint_dialog_art(&self, dialog: &CloudDialog) -> Result<(), String> {
        let bounds = dialog.root.get_bounding_client_rect();
        let key = format!("{},{}", bounds.width(), bounds.height());
        if dialog.art.get_attribute("data-cloud-panel-size").as_deref() != Some(key.as_str()) {
            dialog.art.set_text_content(None);
            append(&dialog.art, &self.art.panel(&self.document, bounds.width(), bounds.height())?)?;
            dialog
                .art
                .set_attribute("data-cloud-panel-size", &key)
                .map_err(|_| "Cloud dialog measured size")?;
        }
        Ok(())
    }

    fn show_search(&mut self) -> Result<(), String> {
        let dialog = CloudDialog::new(
            &self.document,
            &self.root,
            "dark-cloud-modal dark-cloud-modal-search",
            "SEARCH THE DARK CLOUD",
            self.events.clone(),
        )?;
        let body = element(&self.document, "div", "dark-cloud-panel-body")?;
        text(
            &self.document,
            &body,
            "h2",
            "dark-cloud-panel-caption",
            "SEARCH THE DARK CLOUD",
            Medium,
            1.0,
            0xd9ba70,
            false,
        )?;
        let form = element(&self.document, "form", "")?;
        form.set_attribute("data-menu-action", "cloud-search-submit")
            .map_err(|_| "Cloud search submit")?;
        let panel = control_panel(&self.document, &form, "", false)?;
        let row = element(&self.document, "div", "native-ui-control-field-row")?;
        let label = text(
            &self.document,
            &row,
            "label",
            "",
            if self.tab == Tab::Parties {
                "LEADER, MEMBER, OR BONEYARD:"
            } else {
                "MOD, AUTHOR, OR TAG:"
            },
            Medium,
            1.0,
            0xd9ba70,
            false,
        )?;
        label
            .set_attribute("for", "dark-cloud-search-input")
            .map_err(|_| "Cloud search input label")?;
        let field = control_panel(&self.document, &row, "native-ui-control-field", true)?;
        let input: HtmlInputElement =
            element(&self.document, "input", "")?.dyn_into().map_err(|_| "Cloud search input")?;
        input.set_id("dark-cloud-search-input");
        input.set_value(&self.query);
        input.set_attribute("data-game-default-focus", "true").map_err(|_| "Cloud search focus")?;
        field.append_child(&input).map_err(|_| "Cloud search field")?;
        let clear = element(&self.document, "button", "native-ui-control-clear")?;
        for (name, value) in [
            ("type", "button"),
            ("aria-label", "Clear search text"),
            ("data-menu-action", "cloud-search-clear"),
        ] {
            clear.set_attribute(name, value).map_err(|_| "Cloud clear search")?;
        }
        append(
            &clear,
            &crate::ui::cloud::cloud_text(&self.document, "x", Medium, 1.0, 0xd9ba70, false)?,
        )?;
        append(&field, &clear)?;
        append(&panel, &row)?;
        let panel = control_panel(&self.document, &form, "", false)?;
        control_action(&self.document, &panel, "SEARCH NOW", "cloud-search-submit", false, false)?
            .set_attribute("type", "submit")
            .map_err(|_| "Cloud search form submission")?;
        append(&body, &form)?;
        append(dialog.element(), &body)?;
        self.sheet = Some(Sheet::Search { dialog, input });
        self.update_search_clear()?;
        self.sheet.as_ref().expect("created search sheet").dialog().show()
    }

    fn update_search_clear(&self) -> Result<(), String> {
        if let Some(Sheet::Search { dialog, input }) = &self.sheet
            && let Some(clear) = dialog
                .root
                .query_selector(".native-ui-control-clear")
                .map_err(|_| "Cloud clear search target")?
        {
            clear
                .toggle_attribute_with_force("disabled", input.value().is_empty())
                .map_err(|_| "Cloud clear search admission")?;
        }
        Ok(())
    }

    fn show_sort(&mut self) -> Result<(), String> {
        let title = if self.tab == Tab::Parties { "SORT PARTIES BY..." } else { "SORT MODS BY..." };
        let dialog = CloudDialog::new(
            &self.document,
            &self.root,
            "dark-cloud-modal dark-cloud-modal-sort",
            title,
            self.events.clone(),
        )?;
        let body = element(&self.document, "div", "dark-cloud-panel-body")?;
        text(
            &self.document,
            &body,
            "h2",
            "dark-cloud-panel-caption",
            title,
            Medium,
            1.0,
            0xd9ba70,
            false,
        )?;
        let panel = control_panel(&self.document, &body, "", false)?;
        panel.set_attribute("aria-label", "Sort order").map_err(|_| "Cloud sort group label")?;
        panel.set_attribute("role", "group").map_err(|_| "Cloud sort group role")?;
        let choices: &[(Sort, &str)] = if self.tab == Tab::Parties {
            &[(Sort::Members, "MOST WIZARDS"), (Sort::Name, "LEADER NAME")]
        } else {
            &[
                (Sort::Newest, "NEWEST"),
                (Sort::Updated, "UPDATED RECENTLY"),
                (Sort::Downloads, "MOST DOWNLOADED"),
                (Sort::Name, "NAME"),
            ]
        };
        for &(sort, label) in choices {
            let action = control_action(
                &self.document,
                &panel,
                label,
                &format!("cloud-sort:{}", sort.id()),
                self.sort == sort,
                false,
            )?;
            action
                .set_attribute("aria-pressed", if self.sort == sort { "true" } else { "false" })
                .map_err(|_| "Cloud sort selected state")?;
            if self.sort == sort {
                action
                    .set_attribute("data-game-default-focus", "true")
                    .map_err(|_| "Cloud sort selected focus")?;
            }
        }
        append(dialog.element(), &body)?;
        dialog.show()?;
        self.sheet = Some(Sheet::Sort(dialog));
        Ok(())
    }
}

impl Drop for CloudView {
    fn drop(&mut self) {
        if let Some(window) = web_sys::window()
            && let Some(key) = self._listeners.last()
        {
            let _ =
                window.remove_event_listener_with_callback("keydown", key.as_ref().unchecked_ref());
        }
    }
}
