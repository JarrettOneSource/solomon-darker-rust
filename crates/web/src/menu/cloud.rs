//! GPU Dark Cloud catalog/directory, search/sort sheets and detail lifetimes.
//! Source: `DarkCloudScene.tsx`; the Website coordinator retains all I/O/admission.
mod art;
mod detail;
mod format;
mod images;
mod layouts;
mod rows;
use super::{HostedMenuAction, ModSubscriptionAction, RoutingDownloadProgress, widgets};
use crate::gpu_ui::UiFrame;
use crate::hosted::{
    Account, ConnectedGamePlayer, DeveloperGameMatch, ModComments, ModDetail, ModSubscription,
    ModSummary, OwnedImageUrl, PartyActivity, PublicGameParty, SharedMobileUiLayout,
};
use art::CloudArt;
use detail::ModDetailView;
use egui::{Align2, Color32, Rect, Ui, Vec2};
use images::CloudImages;
use layouts::LayoutsView;
use solomon_sim::menu::dark_cloud::{DarkCloudTab as Tab, native_dark_cloud_tab_strip};
use solomon_sim::menu::party_directory::{DirectoryPartyAction, directory_party_action};
use solomon_sim::native_ui_text::{NativeUiFontName, NativeUiTextAlign};
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
    Search { input: String },
    Sort,
}
pub(super) struct CloudView {
    art: CloudArt,
    layouts: LayoutsView,
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
    actions: Vec<HostedMenuAction>,
    tab_scroll_reset: bool,
    return_focus: Option<String>,
}
impl CloudView {
    pub async fn new() -> Result<Self, String> {
        Ok(Self {
            art: CloudArt::load().await?,
            layouts: LayoutsView::new(),
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
            actions: Vec::new(),
            tab_scroll_reset: true,
            return_focus: None,
        })
    }
    pub fn open(&mut self) -> Result<(), String> {
        self.active = true;
        self.tab = Tab::Mods;
        self.sort = Sort::Newest;
        self.query.clear();
        self.selected = None;
        self.return_focus = None;
        self.action_error = None;
        self.subscription_error = None;

        self.request_catalog();
        self.refresh_selection()
    }

    pub fn open_tab(&mut self, tab: Tab, now_ms: f64) -> Result<(), String> {
        self.open()?;
        if tab != Tab::Mods {
            self.activate(&format!("cloud-tab:{}", tab.id()), now_ms)?;
        }
        Ok(())
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

        self.actions.clear();
        if let Some(request_id) = self.join_request.take() {
            self.actions.push(HostedMenuAction::CancelPartyJoin { request_id });
        }
        self.images.clear();

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
            self.request_catalog();
            self.refresh_selection()?;
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
        self.layouts.update()?;
        if self.tab == Tab::Parties && now >= self.refresh_at {
            self.request_directory(now);
            self.refresh_selection()?;
        }
        Ok(())
    }

    pub fn modal_open(&self) -> bool {
        self.detail.is_some() || self.sheet.is_some()
    }

    pub fn back(&mut self) -> Result<bool, String> {
        if self.modal_open() {
            self.detail = None;
            self.sheet = None;

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
        if let Some(detail) = &mut self.detail
            && detail.activate(action, &mut self.next_request)?
        {
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
            if let Some(Sheet::Search { input }) = &self.sheet {
                self.query = input.clone();
            }
            self.sheet = None;
            self.refresh_selection()?;
            return Ok(true);
        }
        if action == "cloud-search-clear" {
            if let Some(Sheet::Search { input }) = &mut self.sheet {
                input.clear();
            }
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
            self.refresh_selection()?;
            return Ok(true);
        }
        if self.modal_open() {
            return Ok(true);
        }
        if self.layouts.activate(action, &mut self.next_request)? {
            return Ok(true);
        }
        if let Some(tab) = action.strip_prefix("cloud-tab:") {
            if self.tab == Tab::Layouts {
                self.layouts.close();
            }
            self.tab = Tab::from_id(tab).ok_or("unknown Cloud section")?;
            self.tab_scroll_reset = true;
            self.return_focus = None;
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
            self.refresh_selection()?;
            return Ok(true);
        }
        if let Some(key) = action.strip_prefix("cloud-select:") {
            self.selected = Some(key.to_owned());
            self.refresh_selection()?;
            return Ok(true);
        }
        if let Some(key) = action.strip_prefix("cloud-open:") {
            self.return_focus = Some(format!("cloud-select:{key}"));
            self.selected = Some(key.to_owned());
            self.primary()?;
            return Ok(true);
        }
        if let Some(slug) = action.strip_prefix("cloud-open-mod:") {
            self.return_focus = Some(action.to_owned());
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
                self.refresh_selection()?;
            }
            return Ok(true);
        }
        match action {
            "cloud-primary" | "cloud-options" => {
                self.return_focus = Some(action.to_owned());
                self.primary()?;
            }
            "cloud-refresh" => {
                if self.tab == Tab::Parties {
                    self.request_directory(now);
                } else {
                    self.request_catalog();
                }
                self.refresh_selection()?;
            }
            "cloud-clear-query" => {
                self.query.clear();
                self.refresh_selection()?;
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
            content,
            self.account.as_ref().map(|a| a.username.as_str()),
            &mut self.next_request,
        ));
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
        self.refresh_selection()?;
        Ok(())
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
        self.refresh_selection()
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
        self.refresh_selection()?;

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
        self.refresh_selection()?;
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
        self.refresh_selection()?;
        Ok(true)
    }

    pub fn complete_join(&mut self, id: u64, result: Result<(), String>) -> Result<bool, String> {
        if !self.active || self.join_request != Some(id) {
            return Ok(false);
        }
        self.join_request = None;
        self.pending_listing = None;
        self.action_error = result.err();
        self.refresh_selection()?;
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
            self.refresh_selection()?;
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
        self.refresh_selection()?;

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
        let accepted = detail.complete_detail(id, result)?;

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
        self.images.complete(id, result)
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

    fn refresh_selection(&mut self) -> Result<(), String> {
        let rows = self.filtered_rows();
        if !rows.iter().any(|row| Some(row.key()) == self.selected) {
            self.selected = rows.first().map(Row::key);
        }
        Ok(())
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
    fn show_search(&mut self) -> Result<(), String> {
        self.return_focus = Some("cloud-search".to_owned());
        self.sheet = Some(Sheet::Search { input: self.query.clone() });
        Ok(())
    }
    fn show_sort(&mut self) -> Result<(), String> {
        self.return_focus = Some("cloud-sort".to_owned());
        self.sheet = Some(Sheet::Sort);
        Ok(())
    }
    pub fn retire_images(&mut self, frame: &UiFrame<'_>) {
        self.images.drain_retired(frame);
    }
    pub fn default_focus(&self) -> Option<String> {
        if self.detail.is_some() {
            Some("cloud-dialog-close".to_owned())
        } else if matches!(self.sheet, Some(Sheet::Search { .. })) {
            Some("dark-cloud-search-input".to_owned())
        } else if matches!(self.sheet, Some(Sheet::Sort)) {
            Some(format!("cloud-sort:{}", self.sort.id()))
        } else {
            self.return_focus.clone().or_else(|| Some(format!("cloud-tab:{}", self.tab.id())))
        }
    }
    pub fn navigation_scope(&self) -> String {
        format!(
            "cloud-{}-{}",
            self.tab.id(),
            if self.detail.is_some() {
                "detail"
            } else if matches!(self.sheet, Some(Sheet::Search { .. })) {
                "search"
            } else if self.sheet.is_some() {
                "sort"
            } else {
                "scene"
            }
        )
    }
    pub fn paint(
        &mut self,
        frame: &mut UiFrame<'_>,
        ui: &mut Ui,
        view: Rect,
        enabled: bool,
        actions: &mut Vec<String>,
    ) -> Result<(), String> {
        widgets::scrollbar_style(ui, 0x876d37, 0x080808, true);
        ui.painter().rect_filled(view, 0.0, Color32::BLACK);
        self.images.begin_paint();
        let phone = view.width() <= 700.0;
        let compact = view.width() <= 880.0;
        let short = view.height() <= 620.0;
        let outer = if compact || short { if phone { 12.0 } else { 16.0 } } else { 55.0 };
        if !compact && !short {
            self.art.scene(frame, ui, view)?;
        }
        let (heading_height, tab_height, footer_height) = if short {
            (46.0, 44.0, 56.0)
        } else if phone {
            (90.0, 44.0, 68.0)
        } else if compact {
            (104.0, 58.0, 78.0)
        } else {
            (128.0, 69.0, 100.0)
        };
        let heading_width = 760.0_f32.min(view.width() - 32.0);
        let title_scale = 1.0_f32.min(heading_width / 560.0).min(
            if short {
                40.6
            } else if phone {
                38.0
            } else {
                58.0
            } / 58.0,
        );
        widgets::native(
            frame,
            ui,
            "THE DARK CLOUD",
            NativeUiFontName::Heading,
            egui::pos2(view.center().x, view.top() + 50.0 * title_scale),
            title_scale,
            0xd9ba70,
            NativeUiTextAlign::Center,
        );
        if !phone {
            widgets::native(
                frame,
                ui,
                "beta",
                NativeUiFontName::Menu,
                egui::pos2(view.center().x + 235.0 * title_scale, view.top() + 50.0 * title_scale),
                title_scale,
                0xd9ba70,
                NativeUiTextAlign::Center,
            );
        }
        let admitted = enabled && !self.modal_open();
        if !short {
            let label = self.account.as_ref().map_or_else(
                || "You are signed in as a GUEST.".to_owned(),
                |a| format!("You are signed in as {}.", a.username),
            );
            let rect = Rect::from_min_size(
                egui::pos2(
                    view.center().x - 280.0_f32.min((view.width() - 24.0) / 2.0),
                    view.top() + if phone { 38.0 } else { 58.0 },
                ),
                Vec2::new(560.0_f32.min(view.width() - 24.0), 50.0),
            );
            let id = if self.account.is_some() { "cloud-account-menu" } else { "cloud-sign-in" };
            if widgets::interact(frame, ui, id, &label, "button", rect, admitted).clicked()
                && admitted
            {
                actions.push(
                    if self.account.is_some() { "cloud-menu" } else { "cloud-sign-in" }.to_owned(),
                );
            }
            let scale = 1.0_f32.min(rect.width() / 560.0);
            widgets::native(
                frame,
                ui,
                &label,
                NativeUiFontName::Menu,
                egui::pos2(rect.center().x, rect.top() + 25.0),
                scale,
                0xd9ba70,
                NativeUiTextAlign::Center,
            );
            if self.account.is_none() {
                widgets::native(
                    frame,
                    ui,
                    "To change this, click here.",
                    NativeUiFontName::Menu,
                    egui::pos2(rect.center().x, rect.top() + 25.0 + 18.0 * scale),
                    scale,
                    0xd9ba70,
                    NativeUiTextAlign::Center,
                );
                ui.painter().line_segment(
                    [
                        egui::pos2(
                            rect.center().x + 34.0 * scale,
                            rect.top() + 25.0 + 20.0 * scale,
                        ),
                        egui::pos2(
                            rect.center().x + 179.0 * scale,
                            rect.top() + 25.0 + 20.0 * scale,
                        ),
                    ],
                    egui::Stroke::new(2.0, widgets::GOLD),
                );
            }
        }
        let frame_rect = if !compact && !short {
            Rect::from_min_max(view.min + Vec2::new(55.0, 173.0), view.max - Vec2::new(55.0, 100.0))
        } else {
            Rect::from_min_max(
                view.min + Vec2::new(outer, heading_height + tab_height),
                view.max - Vec2::new(outer, footer_height + 6.0),
            )
        };
        self.art.list(frame, ui, frame_rect, if phone || short { 0.58 } else { 1.0 })?;
        let tabs_width =
            if short { view.width() - 124.0 } else { 1122.0_f32.min(view.width() - outer * 2.0) };
        let tabs = Rect::from_min_size(
            egui::pos2(view.center().x - tabs_width / 2.0, view.top() + heading_height),
            Vec2::new(tabs_width, tab_height),
        );
        let strip = native_dark_cloud_tab_strip(f64::from(tabs.width()), f64::from(tabs.height()));
        frame.record_rect("cloud-tabs", "Dark Cloud sections", "region", tabs, admitted);
        let mut child = ui.new_child(egui::UiBuilder::new().id_salt("cloud-tabs").max_rect(tabs));
        child.set_clip_rect(tabs);
        let mut scroll = egui::ScrollArea::horizontal()
            .id_salt("cloud-tabs-scroll")
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
            .auto_shrink([false, false]);
        if self.tab_scroll_reset {
            let b = strip.bounds(self.tab);
            scroll =
                scroll.horizontal_scroll_offset(
                    (b.x + b.width - f64::from(tabs.width())).max(0.0) as f32
                );
            self.tab_scroll_reset = false;
        }
        let mut result = Ok(());
        scroll.show(&mut child, |ui| {
            let origin = ui.cursor().min;
            ui.allocate_space(Vec2::new(strip.width as f32, strip.height as f32));
            for tab in Tab::ALL {
                let b = strip.bounds(tab);
                let rect = Rect::from_min_size(
                    origin + Vec2::new(b.x as f32, 0.0),
                    Vec2::new(b.width as f32, b.height as f32),
                );
                let id = format!("cloud-tab:{}", tab.id());
                let response =
                    widgets::interact(frame, ui, &id, tab.label(), "tab", rect, admitted);
                frame.record_value(&id, tab.id(), Some(self.tab == tab));
                if let Err(error) =
                    self.art.tab(frame, ui, rect, strip.scale as f32, tab.label(), self.tab == tab)
                {
                    result = Err(error);
                    break;
                }
                if response.clicked() && admitted {
                    actions.push(id);
                }
            }
        });
        std::mem::replace(&mut result, Ok(()))?;
        let pad = if phone {
            14.0
        } else if compact || short {
            24.0
        } else {
            50.0
        };
        let content = frame_rect.shrink2(Vec2::new(pad, if phone { 14.0 } else { 24.0 }));
        if self.tab == Tab::Layouts {
            self.layouts.paint(frame, ui, &self.art, content, admitted, actions)?;
        } else if self.tab == Tab::Boneyards {
            frame.record_rect(
                "cloud-boneyards-content",
                "Boneyard library",
                "surface",
                content,
                admitted,
            );
        } else {
            self.paint_rows(frame, ui, content, view, admitted, actions)?;
            self.paint_footer(frame, ui, view, footer_height, admitted, actions)?;
        }
        let coarse = web_sys::window()
            .and_then(|window| window.match_media("(pointer: coarse)").ok().flatten())
            .is_some_and(|query| query.matches());
        let skull = if coarse {
            Rect::from_min_size(view.min + Vec2::splat(4.0), Vec2::splat(44.0))
        } else {
            Rect::from_min_size(view.min + Vec2::new(11.0, 7.0), Vec2::new(31.0, 33.0))
        };
        let response =
            widgets::interact_unstyled(frame, ui, "cloud-menu", "Menu", "button", skull, admitted);
        let paint = frame.with_brightness(
            if admitted && (response.hovered() || frame.focus_visible(&response)) {
                1.28
            } else {
                1.0
            },
        );
        self.art.ui.paint_centered(
            &paint,
            ui.painter(),
            42,
            skull.center(),
            if coarse { 1.161_290_3 } else { 1.0 },
            0.0,
            Color32::WHITE,
        )?;
        widgets::focus_outline(frame, ui, &response, 1.0, 2.0, widgets::color(0xfff0b7));
        if response.clicked() && admitted {
            actions.push("cloud-menu".to_owned());
        }
        if let Some(error) = &self.action_error {
            let r = Rect::from_min_size(view.max - Vec2::new(530.0, 158.0), Vec2::new(500.0, 48.0));
            ui.painter().rect_filled(r, 0.0, widgets::color(0x160504));
            widgets::paragraph(ui, error, r.shrink(8.0), "IM FELL English", 16.0, widgets::ERROR);
        }
        if self.modal_open() {
            let ctx = frame.ctx.clone();
            let dialog_rect = if self.detail.is_some() {
                Rect::from_center_size(
                    view.center(),
                    Vec2::new(
                        1100.0_f32.min(view.width() - 48.0),
                        820.0_f32.min(view.height() - 48.0),
                    ),
                )
            } else if matches!(self.sheet, Some(Sheet::Search { .. })) {
                Rect::from_center_size(
                    view.center(),
                    Vec2::new(
                        520.0_f32.min(view.width() - 48.0),
                        if view.width() < 700.0 { 240.0 } else { 205.0 },
                    ),
                )
            } else {
                Rect::from_center_size(
                    view.center(),
                    Vec2::new(
                        320.0_f32.min(view.width() - 48.0),
                        255.0_f32.min(view.height() - 48.0),
                    ),
                )
            };
            let modal = egui::Modal::new(super::cloud_dialog_layer().id)
                .area(
                    egui::Area::new(super::cloud_dialog_layer().id)
                        .fade_in(false)
                        .fixed_pos(dialog_rect.min)
                        .order(egui::Order::Foreground),
                )
                .frame(egui::Frame::NONE)
                .backdrop_color(Color32::from_black_alpha(191))
                .show(&ctx, |ui| {
                    ui.set_min_size(dialog_rect.size());
                    ui.set_clip_rect(view);
                    if !enabled {
                        ui.disable();
                    }
                    if let Some(detail) = &mut self.detail {
                        let subscription = self
                            .subscriptions
                            .iter()
                            .find(|s| s.content.slug == detail.summary.slug);
                        result = detail.paint(
                            frame,
                            ui,
                            &self.art,
                            &mut self.images,
                            view,
                            subscription,
                            self.subscription_request.is_some(),
                            self.subscription_error.as_deref(),
                            actions,
                        );
                    } else {
                        result = self.paint_sheet(frame, ui, view, actions);
                    }
                });
            if modal.should_close() && enabled {
                actions.push("cloud-dialog-close".to_owned());
            }
            std::mem::replace(&mut result, Ok(()))?;
        }
        self.images.finish_paint(frame);
        Ok(())
    }
    fn status_label(&self, count: usize) -> String {
        if self.loading() && count == 0 {
            "CONSULTING THE DARK CLOUD...".to_owned()
        } else if !self.query.is_empty() {
            format!(
                "\"{}\", {} {}",
                self.query.to_uppercase(),
                count,
                if count == 1 { "MATCH" } else { "MATCHES" }
            )
        } else {
            format!(
                "{} {}",
                count,
                match self.tab {
                    Tab::Parties =>
                        if count == 1 {
                            "PARTY"
                        } else {
                            "PARTIES"
                        },
                    Tab::Subscribed => "SUBSCRIBED",
                    _ =>
                        if count == 1 {
                            "MOD"
                        } else {
                            "MODS"
                        },
                }
            )
        }
    }
    fn paint_rows(
        &mut self,
        frame: &mut UiFrame<'_>,
        ui: &mut Ui,
        content: Rect,
        view: Rect,
        enabled: bool,
        actions: &mut Vec<String>,
    ) -> Result<(), String> {
        let rows = self.filtered_rows();
        let compact = view.width() < 1100.0;
        let status_height = if compact { 44.0 } else { 48.0 };
        let status = Rect::from_min_size(content.min, Vec2::new(content.width(), status_height));
        let label = self.status_label(rows.len());
        if view.width() <= 700.0 {
            let tools = if self.tab == Tab::Parties { 128.0 } else { 0.0 }
                + if !self.query.is_empty() { 128.0 } else { 0.0 };
            let label_rect = Rect::from_min_size(
                status.min,
                Vec2::new((status.width() - tools).max(10.0), status.height()),
            );
            frame.record_rect("cloud-list-status", &label, "status", label_rect, false);
            frame.record_value("cloud-list-status", &label, None);
            rows::ink(
                frame,
                ui,
                &label,
                NativeUiFontName::Medium,
                0.85,
                0xd9ba70,
                label_rect,
                false,
            )?;
            let mut right = status.right();
            if self.tab == Tab::Parties {
                let r = Rect::from_min_size(
                    egui::pos2(right - 120.0, status.top()),
                    Vec2::new(120.0, 44.0),
                );
                if widgets::cloud_button(frame, ui, "cloud-refresh", "REFRESH", r, 0.55, enabled)
                    .clicked()
                    && enabled
                {
                    actions.push("cloud-refresh".to_owned());
                }
                right -= 128.0;
            }
            if !self.query.is_empty() {
                let r = Rect::from_min_size(
                    egui::pos2(right - 120.0, status.top()),
                    Vec2::new(120.0, 44.0),
                );
                if widgets::cloud_button(
                    frame,
                    ui,
                    "cloud-clear-query",
                    "CLEAR SEARCH",
                    r,
                    0.55,
                    enabled,
                )
                .clicked()
                    && enabled
                {
                    actions.push("cloud-clear-query".to_owned());
                }
            }
        } else {
            let party = self.tab == Tab::Parties;
            let gap = if view.width() <= 1300.0 { 8.0 } else { 12.0 };
            let action_width = if party {
                160.0
            } else if view.width() <= 880.0 {
                200.0
            } else if view.width() <= 1300.0 {
                270.0
            } else {
                300.0
            };
            let author_width = if party && view.width() <= 1050.0 {
                75.0
            } else if view.width() <= 880.0 {
                80.0
            } else if view.width() <= 1300.0 {
                95.0
            } else {
                120.0
            };
            let version_width = if !party && view.width() <= 1050.0 {
                0.0
            } else if party && view.width() <= 1050.0 {
                85.0
            } else if view.width() <= 1300.0 {
                75.0
            } else {
                90.0
            };
            let state_width = if party && view.width() <= 1050.0 {
                125.0
            } else if view.width() <= 1300.0 {
                135.0
            } else {
                165.0
            };
            let count = if version_width > 0.0 { 5.0 } else { 4.0 };
            let first = status.width()
                - action_width
                - author_width
                - version_width
                - state_width
                - gap * (count - 1.0);
            let mut x = status.left();
            let captions = if party {
                ["party", "wizards", "status", "location", "action"]
            } else if self.tab == Tab::Subscribed {
                ["subscribed mod", "author", "version", "status", "manage"]
            } else {
                ["mod", "author", "version", "status", "action"]
            };
            for (index, (caption, width)) in captions
                .into_iter()
                .zip([first, author_width, version_width, state_width, action_width])
                .enumerate()
            {
                if width <= 0.0 {
                    continue;
                }
                let r = Rect::from_min_size(
                    egui::pos2(x, status.top()),
                    Vec2::new(width, status.height()),
                );
                if party && index == 4 {
                    let button = Rect::from_center_size(r.center(), Vec2::new(100.0, 44.0));
                    if widgets::cloud_button(
                        frame,
                        ui,
                        "cloud-refresh",
                        "REFRESH",
                        button,
                        0.55,
                        enabled,
                    )
                    .clicked()
                        && enabled
                    {
                        actions.push("cloud-refresh".to_owned());
                    }
                } else {
                    rows::ink(
                        frame,
                        ui,
                        caption,
                        NativeUiFontName::Menu,
                        1.0,
                        0xd9ba70,
                        r,
                        index > 0,
                    )?;
                }
                x += width + gap;
            }
            frame.record_rect("cloud-columns", &captions.join(" · "), "heading", status, false);
        }
        let list = Rect::from_min_max(content.min + Vec2::new(0.0, status_height), content.max);
        frame.record_rect("cloud-list", "Dark Cloud entries", "region", list, enabled);
        let mut child =
            ui.new_child(egui::UiBuilder::new().id_salt("cloud-rows-content").max_rect(list));
        child.set_clip_rect(list);
        let mut result = Ok(());
        egui::ScrollArea::vertical()
            .id_salt(format!("cloud-list-{}", self.tab.id()))
            .auto_shrink([false, false])
            .show(&mut child, |ui| {
                if rows.is_empty() {
                    let text = if self.loading() {
                        "CONSULTING THE DARK CLOUD..."
                    } else if let Some(error) = self.active_error() {
                        error
                    } else if !self.query.is_empty() {
                        "NOTHING MATCHES YOUR SEARCH."
                    } else {
                        match self.tab {
                            Tab::Parties => "NO PUBLIC PARTIES.",
                            Tab::Subscribed if self.account.is_none() => {
                                "SIGN IN TO SEE YOUR MODS."
                            }
                            Tab::Subscribed => "NO SUBSCRIBED MODS.",
                            _ => "NO MODS HAVE REACHED THE DARK CLOUD.",
                        }
                    };
                    let (_, r) = ui.allocate_space(Vec2::new(list.width(), 100.0));
                    widgets::paragraph(
                        ui,
                        text,
                        r.shrink(12.0),
                        "IM FELL English",
                        16.0,
                        widgets::GOLD,
                    );
                    if self.active_error().is_some() {
                        let (_, r) = ui.allocate_space(Vec2::new(list.width(), 50.0));
                        if widgets::cloud_button(
                            frame,
                            ui,
                            "cloud-refresh",
                            "RETRY",
                            Rect::from_min_size(r.min, Vec2::new(170.0, 44.0)),
                            0.55,
                            enabled,
                        )
                        .clicked()
                            && enabled
                        {
                            actions.push("cloud-refresh".to_owned());
                        }
                    }
                }
                for row in rows {
                    let selected = Some(row.key()) == self.selected;
                    let r = match row {
                        Row::Mod(content, subscription) => rows::mod_row(
                            frame,
                            ui,
                            &content,
                            subscription.as_deref(),
                            selected,
                            self.subscription_request.is_some(),
                            self.tab == Tab::Subscribed,
                            &mut self.images,
                            list.width(),
                            view.width(),
                            enabled,
                            actions,
                        ),
                        Row::Party(party) => rows::party_row(
                            frame,
                            ui,
                            &party,
                            selected,
                            self.join_request.is_some(),
                            self.pending_listing.as_deref() == Some(party.id.as_str()),
                            list.width(),
                            view.width(),
                            enabled,
                            actions,
                        ),
                    };
                    if let Err(error) = r {
                        result = Err(error);
                        break;
                    }
                }
                if self.tab == Tab::Parties
                    && self.account.as_ref().is_some_and(|a| a.developer_access)
                    && let Err(error) = rows::developer_presence(
                        frame,
                        ui,
                        &self.players,
                        &self.matches,
                        self.presence_request.is_some(),
                        self.presence_error.as_deref(),
                        self.observe_request.as_ref().map(|(_, id)| id.as_str()),
                        list.width(),
                        enabled,
                        actions,
                    )
                {
                    result = Err(error);
                }
            });
        result
    }
    fn paint_footer(
        &self,
        frame: &mut UiFrame<'_>,
        ui: &Ui,
        view: Rect,
        height: f32,
        enabled: bool,
        actions: &mut Vec<String>,
    ) -> Result<(), String> {
        let compact = view.width() <= 880.0 || view.height() <= 620.0;
        let phone = view.width() <= 700.0;
        let footer = if compact {
            Rect::from_min_max(
                egui::pos2(view.left() + 12.0, view.bottom() - height),
                view.max - Vec2::new(12.0, 6.0),
            )
        } else {
            Rect::from_min_size(
                egui::pos2(view.center().x - 416.0, view.bottom() - 96.5),
                Vec2::new(824.5, 85.0),
            )
        };
        let tool_width = if phone {
            52.0
        } else if compact {
            68.0
        } else {
            90.0
        };
        let tool_height = if phone || view.height() <= 620.0 {
            44.0
        } else if compact {
            48.0
        } else {
            52.0
        };
        for (label, id, icon, index) in
            [("Search", "cloud-search", 58, 0), ("Sort", "cloud-sort", 66, 1)]
        {
            let rect = Rect::from_min_size(
                footer.min
                    + Vec2::new(
                        6.0 + index as f32 * (tool_width + 12.0),
                        (footer.height() - tool_height) / 2.0,
                    ),
                Vec2::new(tool_width, tool_height),
            );
            if self.art.tool(frame, ui, id, label, Some(icon), rect, enabled)? {
                actions.push(id.to_owned());
            }
        }
        let selected = self.filtered_rows().into_iter().find(|r| Some(r.key()) == self.selected);
        let (label, disabled) = if self.tab == Tab::Parties {
            match selected.as_ref() {
                Some(Row::Party(p)) => {
                    let action = directory_party_action(
                        p.visibility,
                        matches!(p.activity, PartyActivity::Playing { .. }),
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
            ("VIEW MOD", !matches!(selected, Some(Row::Mod(..))))
        };
        let primary = if compact {
            let left = footer.left() + tool_width * 2.0 + 36.0;
            Rect::from_min_max(
                egui::pos2(left, footer.center().y - if phone { 24.0 } else { 29.0 }),
                egui::pos2(
                    footer.right() - if phone { 0.0 } else { 148.0 },
                    footer.center().y + if phone { 24.0 } else { 29.0 },
                ),
            )
        } else {
            Rect::from_min_size(footer.min + Vec2::new(239.5, 6.0), Vec2::new(353.0, 69.0))
        };
        if widgets::cloud_button(
            frame,
            ui,
            "cloud-primary",
            label,
            primary,
            primary.height() / 69.0,
            enabled && !disabled,
        )
        .clicked()
            && enabled
            && !disabled
        {
            actions.push("cloud-primary".to_owned());
        }
        if !phone {
            let options = Rect::from_min_size(
                egui::pos2(
                    footer.right() - if compact { 130.0 } else { 191.0 },
                    footer.center().y - tool_height / 2.0,
                ),
                Vec2::new(if compact { 130.0 } else { 185.0 }, tool_height),
            );
            if self.art.tool(
                frame,
                ui,
                "cloud-options",
                "OPTIONS",
                None,
                options,
                enabled && matches!(selected, Some(Row::Mod(..))),
            )? {
                actions.push("cloud-options".to_owned());
            }
        }
        if view.width() > 1360.0 {
            let label = self.status_label(self.filtered_rows().len());
            let status = Rect::from_min_size(
                egui::pos2(footer.left() - 260.0, footer.bottom() - 62.0),
                Vec2::new(240.0, 44.0),
            );
            let count_rect = if self.query.is_empty() {
                status
            } else {
                Rect::from_min_size(status.min, Vec2::new(112.0, 44.0))
            };
            frame.record_rect("cloud-list-status", &label, "status", count_rect, false);
            frame.record_value("cloud-list-status", &label, None);
            rows::ink(
                frame,
                ui,
                &label,
                NativeUiFontName::Medium,
                0.85,
                0xd9ba70,
                count_rect,
                false,
            )?;
            if !self.query.is_empty() {
                let r = Rect::from_min_size(
                    egui::pos2(status.right() - 120.0, status.top()),
                    Vec2::new(120.0, 44.0),
                );
                if widgets::cloud_button(
                    frame,
                    ui,
                    "cloud-clear-query",
                    "CLEAR SEARCH",
                    r,
                    0.55,
                    enabled,
                )
                .clicked()
                    && enabled
                {
                    actions.push("cloud-clear-query".to_owned());
                }
            }
        }
        if let Some((done, total, active)) =
            self.download_progress.filter(|(_, total, _)| *total > 0)
        {
            let percent = (done as f64 / total as f64 * 100.0 + 0.5).floor().min(100.0);
            let rect = Rect::from_min_size(
                footer.min - Vec2::new(0.0, 42.0),
                Vec2::new(footer.width(), 34.0),
            );
            widgets::control_panel(frame, ui, &self.art.controls, rect, true)?;
            ui.painter().rect_filled(
                Rect::from_min_size(
                    rect.min + Vec2::splat(3.0),
                    Vec2::new((rect.width() - 6.0) * percent as f32 / 100.0, rect.height() - 6.0),
                ),
                0.0,
                widgets::color(0x517e49),
            );
            widgets::label(
                ui,
                &format!(
                    "{}, {percent}%",
                    if active { "DOWNLOADING MOD CONTENT" } else { "CONTENT READY" }
                ),
                rect.center(),
                Align2::CENTER_CENTER,
                "Palatino",
                16.0,
                widgets::GOLD,
            );
        }
        Ok(())
    }
    fn paint_sheet(
        &mut self,
        frame: &mut UiFrame<'_>,
        ui: &mut Ui,
        view: Rect,
        actions: &mut Vec<String>,
    ) -> Result<(), String> {
        let search = matches!(self.sheet, Some(Sheet::Search { .. }));
        let size = if search {
            Vec2::new(
                520.0_f32.min(view.width() - 48.0),
                if view.width() < 700.0 { 240.0 } else { 205.0 },
            )
        } else {
            Vec2::new(320.0_f32.min(view.width() - 48.0), 255.0_f32.min(view.height() - 48.0))
        };
        let panel = Rect::from_center_size(view.center(), size);
        self.art.panel(frame, ui, panel)?;
        let title = if search {
            "SEARCH THE DARK CLOUD"
        } else if self.tab == Tab::Parties {
            "SORT PARTIES BY..."
        } else {
            "SORT MODS BY..."
        };
        widgets::native(
            frame,
            ui,
            title,
            NativeUiFontName::Medium,
            panel.min + Vec2::new(40.0, 42.0),
            1.0,
            0xd9ba70,
            NativeUiTextAlign::Left,
        );
        if let Some(Sheet::Search { input }) = &mut self.sheet {
            let body =
                panel.shrink2(Vec2::new(if view.width() < 700.0 { 8.0 } else { 30.0 }, 24.0));
            let phone = view.width() < 700.0;
            let caption = if self.tab == Tab::Parties {
                "LEADER, MEMBER, OR BONEYARD:"
            } else {
                "MOD, AUTHOR, OR TAG:"
            };
            let row = Rect::from_min_size(
                body.min + Vec2::new(0.0, 48.0),
                Vec2::new(body.width(), if phone { 88.0 } else { 44.0 }),
            );
            widgets::control_panel(frame, ui, &self.art.controls, row, false)?;
            let caption_width = solomon_sim::native_ui_text::measure_native_ui_text(
                caption,
                NativeUiFontName::Medium,
                1.0,
            ) as f32;
            widgets::native(
                frame,
                ui,
                caption,
                NativeUiFontName::Medium,
                row.min + Vec2::new(10.0, if phone { 24.0 } else { 28.0 }),
                1.0,
                0xd9ba70,
                NativeUiTextAlign::Left,
            );
            let field = if phone {
                Rect::from_min_size(
                    row.min + Vec2::new(10.0, 42.0),
                    Vec2::new(row.width() - 26.0, 30.0),
                )
            } else {
                Rect::from_min_max(
                    row.min + Vec2::new(caption_width + 20.0, 7.0),
                    row.max - Vec2::new(16.0, 7.0),
                )
            };
            widgets::control_panel(frame, ui, &self.art.controls, field, true)?;
            let response = widgets::edit(
                frame,
                ui,
                "dark-cloud-search-input",
                if self.tab == Tab::Parties {
                    "Leader, member, or boneyard"
                } else {
                    "Mod, author, or tag"
                },
                input,
                field.expand2(Vec2::new(0.0, 7.0)),
                "IM FELL English",
                16.0,
                true,
                false,
            );
            frame.record_value("dark-cloud-search-input", input, None);
            if widgets::submitted_on_enter(&response) {
                actions.push("cloud-search-submit".to_owned());
            }
            let clear =
                Rect::from_min_size(field.right_top() - Vec2::new(44.0, 0.0), Vec2::splat(44.0));
            if widgets::interact(
                frame,
                ui,
                "cloud-search-clear",
                "Clear search text",
                "button",
                clear,
                !input.is_empty(),
            )
            .clicked()
            {
                actions.push("cloud-search-clear".to_owned());
            }
            widgets::label(
                ui,
                "x",
                clear.center(),
                Align2::CENTER_CENTER,
                "Palatino",
                16.0,
                widgets::GOLD,
            );
            let button = Rect::from_min_size(
                egui::pos2(row.left(), row.bottom() + if phone { 14.0 } else { 24.0 }),
                Vec2::new(row.width(), 44.0),
            );
            widgets::control_panel(frame, ui, &self.art.controls, button, false)?;
            let response = widgets::interact(
                frame,
                ui,
                "cloud-search-submit",
                "SEARCH NOW",
                "button",
                button,
                true,
            );
            widgets::native(
                frame,
                ui,
                "SEARCH NOW",
                NativeUiFontName::Medium,
                button.min + Vec2::new(10.0, 28.0),
                1.0,
                0xd9ba70,
                NativeUiTextAlign::Left,
            );
            if response.clicked() {
                actions.push("cloud-search-submit".to_owned());
            }
        } else {
            let choices = if self.tab == Tab::Parties {
                vec![(Sort::Members, "MOST WIZARDS"), (Sort::Name, "LEADER NAME")]
            } else {
                vec![
                    (Sort::Newest, "NEWEST"),
                    (Sort::Updated, "UPDATED RECENTLY"),
                    (Sort::Downloads, "MOST DOWNLOADED"),
                    (Sort::Name, "NAME"),
                ]
            };
            for (index, (sort, label)) in choices.into_iter().enumerate() {
                let rect = Rect::from_min_size(
                    panel.min + Vec2::new(30.0, 62.0 + index as f32 * 44.0),
                    Vec2::new(panel.width() - 60.0, 44.0),
                );
                widgets::control_panel(frame, ui, &self.art.controls, rect, false)?;
                let id = format!("cloud-sort:{}", sort.id());
                if widgets::interact(frame, ui, &id, label, "button", rect, true).clicked() {
                    actions.push(id);
                }
                frame.record_value(
                    &format!("cloud-sort:{}", sort.id()),
                    sort.id(),
                    Some(self.sort == sort),
                );
                widgets::native(
                    frame,
                    ui,
                    label,
                    NativeUiFontName::Medium,
                    rect.min + Vec2::new(10.0, 28.0),
                    1.0,
                    if self.sort == sort { 0xbfffbf } else { 0xd9ba70 },
                    NativeUiTextAlign::Left,
                );
            }
        }
        let close =
            Rect::from_min_size(panel.right_top() + Vec2::new(-36.0, -12.0), Vec2::splat(44.0));
        if widgets::interact(frame, ui, "cloud-dialog-close", "Close", "button", close, true)
            .clicked()
        {
            actions.push("cloud-dialog-close".to_owned());
        }
        widgets::label(
            ui,
            "×",
            close.center(),
            Align2::CENTER_CENTER,
            "Georgia",
            22.0,
            widgets::GOLD,
        );
        Ok(())
    }
}
