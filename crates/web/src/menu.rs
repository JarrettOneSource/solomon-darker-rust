//! Offline title/Create/pause/settings surfaces. This module owns navigation and art;
//! typed actions leave through `take_actions` so the session owns gameplay and persistence.

mod cloud;
mod create;
mod hall;
mod join;
pub(crate) mod navigation;
mod prompt;
mod routing;
mod settings;
mod title;

use std::cell::RefCell;
use std::rc::Rc;

use solomon_sim::NativeRng;
use solomon_sim::audio::{
    AudioCue, AudioEvent, AudioOptions, MusicScene, create_entry_audio_events,
    create_selection_audio_events,
};
use solomon_sim::menu::native_pause_menu_reveal;
use solomon_sim::profile::create::{
    NATIVE_CREATE_ENTRY_FADE_MS, NATIVE_CREATE_RANDOMIZE_FINALIZE_MS, NativeCreateLoadoutSelection,
    finalize_native_create_loadout,
};
use solomon_sim::profile::{NewGameSelection, WizardDiscipline, WizardElement};
use solomon_sim::settings::GameSettings;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{Document, Element, Event, HtmlElement, KeyboardEvent, PointerEvent};

use crate::assets;
use crate::ui::{StockAtlas, append, element, native_button, style};
use cloud::CloudView;
use create::CreateView;
pub use hall::HallSnapshot;
use hall::HallView;
use join::JoinView;
use navigation::MenuNavigation;
use prompt::{TitlePrompt, TitlePromptKind};
pub use routing::RoutingDownloadProgress;
use routing::RoutingView;
pub use settings::SettingsContext;
use settings::SettingsView;
use title::TitleView;

#[derive(Clone, Debug, PartialEq)]
pub enum MenuAction {
    Start(NewGameSelection),
    CancelCreate,
    ConfirmLoadout(NewGameSelection),
    StartTutorial,
    TutorialDeclined,
    Continue,
    Resume,
    Cheats,
    Leave,
    SettingsChanged(Box<GameSettings>),
    SetEnhancedEffects { enabled: bool },
    SaveMobileLayout(Option<Box<solomon_sim::mobile_ui::MobileUiLayout>>),
}

/// Requests whose asynchronous I/O belongs to the Website host coordinator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HostedMenuAction {
    LoadGlobalHall { request_id: u64, board: solomon_sim::hall_of_fame::HallOfFameBoard },
    LoadParties { request_id: u64 },
    ResolvePartyCode { request_id: u64, code: String },
    JoinPublicParty { request_id: u64, listing_id: String },
    RequestPartyInvite { request_id: u64, listing_id: String },
    CancelPartyJoin { request_id: u64 },
    ContinuePartyJoin { request_id: u64 },
    ContinueModdedPlay { request_id: u64 },
    PlayVanilla { request_id: u64 },
    CancelModdedPlay { request_id: u64 },
    ContinueSaveModMismatch { request_id: u64 },
    CancelSaveModMismatch { request_id: u64 },
    ContinueCheatsInPrivateCollege,
    LoadDarkCloudCatalog { request_id: u64 },
    LoadModDetail { request_id: u64, slug: String },
    LoadModComments { request_id: u64, slug: String },
    PostModComment { request_id: u64, slug: String, body: String },
    DeleteModComment { request_id: u64, slug: String, comment_id: u64 },
    ChangeModSubscription { request_id: u64, slug: String, action: ModSubscriptionAction },
    LoadModImage { request_id: u64, url: String },
    LoadDeveloperPresence { request_id: u64 },
    ObserveMatch { request_id: u64, match_id: String },
    LoadMobileLayout { request_id: u64, code: String },
    PublishMobileLayout { request_id: u64 },
    InspectSaveFiles { request_id: u64, files: Vec<web_sys::File> },
    ExportSave { request_id: u64 },
    ReplaceImportedSave { request_id: u64, document: String },
    SignIn,
    SignOut,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModSubscriptionAction {
    Subscribe,
    Enable,
    Disable,
    Unsubscribe,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Screen {
    Hidden,
    Title,
    Play,
    Join,
    Cloud,
    Create,
    Hall,
    Pause,
    Settings,
}

enum UiEvent {
    Action(String),
    Key(String, bool),
    Pointer(i16),
    SettingsChanged,
}

#[derive(Clone, Copy)]
enum PauseChoice {
    Resume,
    Cheats,
    Settings,
    Leave,
    SignOut,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CreateCommitKind {
    Manual,
    Randomize,
}

struct PendingCreateCommit {
    started_at: f64,
    kind: CreateCommitKind,
    selection: NewGameSelection,
    submitted: bool,
}

pub struct MenuUi {
    root: HtmlElement,
    orientation_hint: HtmlElement,
    title: TitleView,
    hall: HallView,
    join: JoinView,
    cloud: CloudView,
    create: CreateView,
    settings_view: SettingsView,
    pause: PauseView,
    prompt: TitlePrompt,
    routing: RoutingView,
    screen: Screen,
    settings_back: Screen,
    pause_back: Screen,
    settings: GameSettings,
    account_username: Option<String>,
    can_resume: bool,
    party_directory_available: bool,
    desktop: bool,
    cheat_menu_available: bool,
    input_suspended: bool,
    loadout_locked: bool,
    selected_element: Option<WizardElement>,
    pending_start: Option<PendingCreateCommit>,
    create_rng: Option<NativeRng>,
    create_previous_phase_elapsed: f64,
    pending_pause: Option<(f64, PauseChoice)>,
    events: Rc<RefCell<Vec<UiEvent>>>,
    actions: Vec<MenuAction>,
    hosted_actions: Vec<HostedMenuAction>,
    audio_events: Vec<AudioEvent>,
    now: f64,
    title_started: f64,
    create_started: f64,
    phase_started: f64,
    pause_started: f64,
    viewport: (i32, i32),
    names: Vec<String>,
    document: Document,
    navigation: MenuNavigation,
    _listeners: Vec<Closure<dyn FnMut(Event)>>,
}

impl MenuUi {
    /// A player panel owns Escape while it is open. Drop queued gameplay shortcuts before
    /// the hidden pause surface samples them; visible menu navigation remains unchanged.
    pub fn discard_gameplay_keys(&self) {
        if self.screen == Screen::Hidden {
            self.events.borrow_mut().retain(|event| !matches!(event, UiEvent::Key(_, _)));
        }
    }
    #[must_use]
    pub fn settings_open(&self) -> bool {
        self.screen == Screen::Settings
    }

    /// Chat/social dialogs keep the visible menu but own its keyboard/controller input.
    /// Native inert also preserves the source's pointer and focus admission.
    pub fn set_input_suspended(&mut self, suspended: bool) -> Result<(), String> {
        if self.input_suspended == suspended {
            return Ok(());
        }
        self.root
            .toggle_attribute_with_force("inert", suspended)
            .map_err(|_| "menu input admission")?;
        self.input_suspended = suspended;
        // Window capture can queue the modal's closing key before its own handler
        // stops propagation. Resuming must not replay that key in the menu below.
        self.events.borrow_mut().retain(|event| !matches!(event, UiEvent::Key(_, _)));
        self.navigation.release();
        Ok(())
    }

    pub async fn attach(document: &Document) -> Result<Self, String> {
        let root = element(document, "div", "game-menus")?;
        root.set_hidden(true);
        let title = TitleView::new(document).await?;
        let create = CreateView::new(document).await?;
        let hall = HallView::new(document).await?;
        let join = JoinView::new(document)?;
        let cloud = CloudView::new(document).await?;
        let settings_view = SettingsView::new(document).await?;
        let pause = PauseView::new(document).await?;
        let prompt = TitlePrompt::new(document).await?;
        let routing = RoutingView::new(document)?;
        for surface in [
            &title.root,
            &create.root,
            &hall.root,
            &join.root,
            &cloud.root,
            &settings_view.root,
            &pause.root,
        ] {
            append(&root, surface)?;
        }
        append(&root, &prompt.root)?;
        append(&root, &routing.root)?;
        append(&document.body().ok_or("page has no body")?, &root)?;
        // MainMenuScene keeps this outside its fixed-aspect stage, including
        // during gameplay. Dark Cloud alone opts out of the portrait gate.
        let orientation_hint = element(document, "div", "game-orientation-hint")?;
        orientation_hint.set_attribute("role", "status").map_err(|_| "orientation hint role")?;
        orientation_hint
            .set_text_content(Some("Rotate your device to landscape to enter the College."));
        append(&document.body().ok_or("page has no body")?, &orientation_hint)?;
        let names_bytes = assets::fetch_bytes("assets/stock/menu/wizard-names.json").await?;
        let names_json =
            String::from_utf8(names_bytes).map_err(|_| "wizard name list is not UTF-8")?;
        let names_value =
            js_sys::JSON::parse(&names_json).map_err(|_| "wizard name list is not JSON")?;
        if !js_sys::Array::is_array(&names_value) {
            return Err("wizard name list must be an array".to_string());
        }
        let mut names = Vec::new();
        for value in js_sys::Array::from(&names_value) {
            names.push(value.as_string().ok_or("wizard name list contains a non-string")?);
        }
        if names.is_empty() {
            return Err("wizard name list is empty".to_string());
        }
        let events = Rc::new(RefCell::new(Vec::new()));
        let mut listeners = Vec::new();
        let clicked = events.clone();
        let click =
            Closure::<dyn FnMut(Event)>::new(move |event: Event| {
                match menu_action_target(&event) {
                    Ok(Some(action)) => {
                        event.prevent_default();
                        clicked.borrow_mut().push(UiEvent::Action(action));
                    }
                    Ok(None) => {}
                    Err(message) => web_sys::console::error_1(&JsValue::from_str(&message)),
                }
            });
        root.add_event_listener_with_callback("click", click.as_ref().unchecked_ref())
            .map_err(|_| "menu click listener")?;
        root.add_event_listener_with_callback("submit", click.as_ref().unchecked_ref())
            .map_err(|_| "menu submit listener")?;
        listeners.push(click);
        let changed = events.clone();
        let change = Closure::<dyn FnMut(Event)>::new(move |_| {
            changed.borrow_mut().push(UiEvent::SettingsChanged)
        });
        for event in ["input", "change"] {
            root.add_event_listener_with_callback(event, change.as_ref().unchecked_ref())
                .map_err(|_| "settings value listener")?;
        }
        listeners.push(change);
        let keyed = events.clone();
        let key_scope = root.clone();
        let key = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
            let Some(key) = event.dyn_ref::<KeyboardEvent>() else {
                return;
            };
            if key.repeat() || key.alt_key() || key.ctrl_key() || key.meta_key() {
                return;
            }
            if key_scope.get_attribute("data-menu-screen").as_deref() == Some("dark-cloud")
                && key_scope.get_attribute("data-cloud-menu-binding").as_deref()
                    == Some(key.code().as_str())
                && matches!(key_scope.query_selector("dialog[open]"), Ok(None))
            {
                key.prevent_default();
                key.stop_propagation();
            }
            match key_scope.query_selector("[data-awaiting-binding]") {
                Ok(Some(_)) => {
                    key.prevent_default();
                    key.stop_immediate_propagation();
                }
                Ok(None) => match key_scope.query_selector(".game-settings-backdrop:not([hidden])")
                {
                    Ok(Some(settings))
                        if key.code() == "Escape"
                            || settings.get_attribute("data-settings-menu-binding").as_deref()
                                == Some(key.code().as_str()) =>
                    {
                        key.prevent_default();
                        key.stop_immediate_propagation();
                    }
                    Ok(_) => {}
                    Err(_) => web_sys::console::error_1(&"settings key scope failed".into()),
                },
                Err(_) => web_sys::console::error_1(&"binding capture selector failed".into()),
            }
            let editing =
                key.target().and_then(|target| target.dyn_into::<Element>().ok()).is_some_and(
                    |target| ["INPUT", "TEXTAREA", "SELECT"].contains(&target.tag_name().as_str()),
                );
            keyed.borrow_mut().push(UiEvent::Key(key.code(), editing));
        });
        let window = web_sys::window().ok_or("no window for menu input")?;
        window
            .add_event_listener_with_callback_and_bool(
                "keydown",
                key.as_ref().unchecked_ref(),
                true,
            )
            .map_err(|_| "menu key listener")?;
        listeners.push(key);
        let pointed = events.clone();
        let pointer_scope = root.clone();
        let pointer = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
            let Some(pointer) = event.dyn_ref::<PointerEvent>() else {
                return;
            };
            if pointer.pointer_type() != "mouse" || pointer.button() < 1 {
                return;
            }
            // GameSettingsDialog captures secondary mouse buttons only for a belt row.
            match pointer_scope
                .query_selector("[data-awaiting-binding][data-menu-action^='bind:belt-']")
            {
                Ok(Some(_)) => {
                    pointer.prevent_default();
                    pointer.stop_propagation();
                    pointed.borrow_mut().push(UiEvent::Pointer(pointer.button()));
                }
                Ok(None) => {}
                Err(_) => {
                    web_sys::console::error_1(&"mouse binding capture selector failed".into())
                }
            }
        });
        root.add_event_listener_with_callback("pointerdown", pointer.as_ref().unchecked_ref())
            .map_err(|_| "binding pointer listener")?;
        listeners.push(pointer);
        let mut ui = Self {
            root,
            orientation_hint,
            title,
            hall,
            join,
            cloud,
            create,
            settings_view,
            pause,
            prompt,
            routing,
            screen: Screen::Hidden,
            settings_back: Screen::Title,
            pause_back: Screen::Hidden,
            settings: GameSettings::default(),
            account_username: None,
            can_resume: false,
            party_directory_available: false,
            desktop: false,
            cheat_menu_available: false,
            input_suspended: false,
            loadout_locked: false,
            selected_element: None,
            pending_start: None,
            create_rng: None,
            create_previous_phase_elapsed: 0.0,
            pending_pause: None,
            events,
            actions: Vec::new(),
            hosted_actions: Vec::new(),
            audio_events: Vec::new(),
            now: 0.0,
            title_started: 0.0,
            create_started: 0.0,
            phase_started: 0.0,
            pause_started: 0.0,
            viewport: (0, 0),
            names,
            document: document.clone(),
            navigation: MenuNavigation::new(),
            _listeners: listeners,
        };
        ui.show_title(false)?;
        Ok(ui)
    }

    pub fn show_title(&mut self, can_resume: bool) -> Result<(), String> {
        self.can_resume = can_resume;
        self.title_started = self.now;
        self.title.restart();
        self.title.show_controls(false, can_resume)?;
        self.set_screen(Screen::Title)
    }

    pub fn set_resume_available(&mut self, can_resume: bool) -> Result<(), String> {
        self.can_resume = can_resume;
        let play = self.screen == Screen::Play
            || (self.screen == Screen::Settings && self.settings_back == Screen::Play);
        self.title.show_controls(play, can_resume)
    }

    pub fn set_hall_snapshot(&mut self, snapshot: HallSnapshot<'_>) {
        self.hall.set_snapshot(snapshot);
    }

    fn desktop_action(&self, explore: bool) {
        wasm_bindgen_futures::spawn_local(async move {
            let Some(window) = web_sys::window() else { return };
            let result = if explore {
                crate::desktop::open_website(&window).await
            } else {
                crate::desktop::return_to_launcher(&window).await
            };
            if let Err(error) = result {
                crate::set_status(&error);
            }
        });
    }

    pub fn set_desktop(&mut self, desktop: bool) {
        self.desktop = desktop;
    }

    pub fn set_global_hall_available(&mut self, available: bool) {
        self.hall.set_global_available(available);
    }

    pub fn take_hosted_actions(&mut self) -> Vec<HostedMenuAction> {
        let mut actions = std::mem::take(&mut self.hosted_actions);
        actions.extend(self.hall.take_hosted_actions());
        actions.extend(self.join.take_actions());
        actions.extend(self.cloud.take_actions());
        actions.extend(self.routing.take_actions());
        actions.extend(self.settings_view.take_save_transfer_actions());
        actions
    }

    /// The Website mount enables this source route by default; packaged play has
    /// no account/content backend to advertise.
    pub fn set_dark_cloud_available(&mut self, available: bool) -> Result<(), String> {
        self.title.set_dark_cloud_available(available);
        if matches!(self.screen, Screen::Title | Screen::Play) {
            self.title.show_controls(self.screen == Screen::Play, self.can_resume)?;
        }
        Ok(())
    }

    pub fn set_hosted_account(
        &mut self,
        account: Option<crate::hosted::Account>,
    ) -> Result<(), String> {
        self.set_settings_account_username(
            account.as_ref().map(|account| account.username.as_str()),
        )?;
        self.cloud.set_account(account)
    }

    pub fn show_dark_cloud(&mut self) -> Result<(), String> {
        self.show_dark_cloud_tab(solomon_sim::menu::dark_cloud::DarkCloudTab::Mods)
    }

    pub fn show_dark_cloud_tab(
        &mut self,
        tab: solomon_sim::menu::dark_cloud::DarkCloudTab,
    ) -> Result<(), String> {
        self.cloud.open_tab(tab, self.now)?;
        self.set_screen(Screen::Cloud)
    }

    pub fn boneyards_mount(&self) -> HtmlElement {
        self.cloud.boneyards_mount()
    }

    pub fn dark_cloud_boneyards_selected(&self) -> bool {
        self.cloud.boneyards_selected()
    }

    pub fn request_boneyard_library_menu(&mut self) -> Result<(), String> {
        if self.screen == Screen::Cloud {
            self.handle_event(UiEvent::Key(self.settings.controls.open_menu.clone(), false))?;
        }
        Ok(())
    }

    pub fn complete_dark_cloud_catalog(
        &mut self,
        id: u64,
        mods: Result<Vec<crate::hosted::ModSummary>, String>,
        subscriptions: Result<Vec<crate::hosted::ModSubscription>, String>,
    ) -> Result<bool, String> {
        self.cloud.complete_catalog(id, mods, subscriptions)
    }
    pub fn complete_mod_detail(
        &mut self,
        id: u64,
        result: Result<crate::hosted::ModDetail, String>,
    ) -> Result<bool, String> {
        self.cloud.complete_detail(id, result)
    }
    pub fn complete_mod_comments(
        &mut self,
        id: u64,
        result: Result<crate::hosted::ModComments, String>,
    ) -> Result<bool, String> {
        self.cloud.complete_comments(id, result)
    }
    pub fn complete_mod_comment_mutation(
        &mut self,
        id: u64,
        result: Result<(), String>,
    ) -> Result<bool, String> {
        self.cloud.complete_comment_mutation(id, result)
    }
    pub fn complete_mod_subscription(
        &mut self,
        id: u64,
        result: Result<(), String>,
    ) -> Result<bool, String> {
        self.cloud.complete_subscription(id, result)
    }
    pub fn complete_mod_image(
        &mut self,
        id: u64,
        result: Result<crate::hosted::OwnedImageUrl, String>,
    ) -> Result<bool, String> {
        self.cloud.complete_image(id, result)
    }
    pub fn complete_developer_presence(
        &mut self,
        id: u64,
        result: Result<
            (Vec<crate::hosted::ConnectedGamePlayer>, Vec<crate::hosted::DeveloperGameMatch>),
            String,
        >,
    ) -> Result<bool, String> {
        self.cloud.complete_presence(id, result)
    }
    pub fn complete_observe_match(
        &mut self,
        id: u64,
        result: Result<(), String>,
    ) -> Result<bool, String> {
        self.cloud.complete_observe(id, result)
    }
    pub fn set_dark_cloud_progress(
        &mut self,
        id: u64,
        progress: Option<RoutingDownloadProgress<'_>>,
    ) -> Result<bool, String> {
        self.cloud.set_progress(id, progress)
    }
    /// Receipts are supplied after the root has validated/applied the real layout.
    pub fn complete_mobile_layout(
        &mut self,
        id: u64,
        result: Result<crate::hosted::SharedMobileUiLayout, String>,
    ) -> Result<bool, String> {
        if self.settings_view.mobile_layout_publication_pending(id) {
            return self.settings_view.complete_mobile_layout_publication(id, result);
        }
        self.cloud.complete_layout(id, result)
    }
    pub fn set_mobile_layout_customized(&mut self, customized: bool) -> Result<(), String> {
        self.cloud.set_mobile_layout_customized(customized)
    }

    /// Call after the active directory request or College invitation resolves.
    /// The coordinator keeps the real intent; no admission occurs until Continue.
    pub fn show_party_join_consent(
        &mut self,
        request_id: u64,
        target: crate::hosted::PartyJoinTarget,
        requires_vanilla: bool,
        signed_in: bool,
    ) -> Result<(), String> {
        self.routing.show_party(request_id, target, requires_vanilla, signed_in)?;
        self.root.set_hidden(false);
        self.navigation.release();
        Ok(())
    }

    /// New Game has emitted its initial admission intent, before College/Create.
    pub fn show_modded_play(
        &mut self,
        request_id: u64,
        active_mods: Vec<crate::hosted::ActiveWebMod>,
        cheats_enabled: bool,
    ) -> Result<(), String> {
        if self.screen == Screen::Hidden {
            self.title.show_controls(true, self.can_resume)?;
            self.set_screen(Screen::Play)?;
        }
        self.routing.show_modded(request_id, active_mods, cheats_enabled)?;
        self.root.set_hidden(false);
        self.navigation.release();
        Ok(())
    }

    pub fn show_mod_mismatch(
        &mut self,
        request_id: u64,
        mismatch: solomon_sim::menu::save_mods::GameSaveModMismatch,
    ) -> Result<(), String> {
        self.routing.show_mismatch(request_id, mismatch)?;
        self.routing.resize(f64::from(self.viewport.0), f64::from(self.viewport.1))?;
        self.root.set_hidden(false);
        self.navigation.release();
        Ok(())
    }

    pub fn set_routing_progress(
        &self,
        request_id: u64,
        progress: Option<RoutingDownloadProgress<'_>>,
    ) -> Result<bool, String> {
        self.routing.progress(request_id, progress)
    }

    pub fn complete_routing(
        &mut self,
        request_id: u64,
        result: Result<(), String>,
    ) -> Result<bool, String> {
        let accepted = self.routing.complete(request_id, result)?;
        self.root.set_hidden(self.screen == Screen::Hidden && !self.routing.active());
        Ok(accepted)
    }

    #[must_use]
    pub fn routing_active(&self) -> bool {
        self.routing.active()
    }

    /// The Website mount enables its real directory; packaged direct-host play
    /// has no broker URL or party-code resolver to advertise.
    pub fn set_party_directory_available(&mut self, available: bool) -> Result<(), String> {
        self.party_directory_available = available;
        self.title.set_party_directory_available(available);
        if matches!(self.screen, Screen::Title | Screen::Play) {
            self.title.show_controls(self.screen == Screen::Play, self.can_resume)?;
        }
        if !available && self.screen == Screen::Join {
            self.back()?;
        }
        Ok(())
    }

    pub fn complete_party_directory(
        &mut self,
        request_id: u64,
        result: Result<Vec<crate::hosted::PublicGameParty>, String>,
    ) -> Result<bool, String> {
        if self.cloud_visible(self.screen) {
            self.cloud.complete_directory(request_id, result)
        } else {
            self.join.complete_directory(request_id, result)
        }
    }

    /// Root calls this before consuming a resolved admission. False means the
    /// source surface unmounted or a newer request superseded this callback.
    pub fn complete_party_join(
        &mut self,
        request_id: u64,
        result: Result<(), String>,
    ) -> Result<bool, String> {
        if self.cloud_visible(self.screen) {
            self.cloud.complete_join(request_id, result)
        } else {
            self.join.complete_join(request_id, result)
        }
    }

    pub fn show_join_party(&mut self) -> Result<(), String> {
        if self.party_directory_available {
            self.join.open(self.now)?;
            self.set_screen(Screen::Join)?;
        }
        Ok(())
    }

    pub fn complete_global_hall(
        &mut self,
        request_id: u64,
        result: Result<Vec<solomon_sim::hall_of_fame::HallOfFameEntry>, String>,
    ) -> bool {
        self.hall.complete_global(request_id, result)
    }

    pub fn show_hall(&mut self) -> Result<(), String> {
        self.hall.open(self.now);
        self.set_screen(Screen::Hall)
    }

    /// The storage owner calls this only after verifying that the save slot is absent.
    /// An unavailable store or a disabled Continue button does not establish eligibility.
    pub fn offer_tutorial(&mut self) -> Result<(), String> {
        if self.screen == Screen::Title && !self.can_resume {
            self.show_prompt(TitlePromptKind::Tutorial)?;
        }
        Ok(())
    }

    pub fn show_pause(&mut self) -> Result<(), String> {
        self.audio_events.push(AudioEvent::Play(AudioCue::OpenPanel, AudioOptions::default()));
        self.pause_started = self.now;
        self.pending_pause = None;
        self.pause_back = Screen::Hidden;
        self.set_gameplay_pause_rows()?;
        self.pause.resize(f64::from(self.viewport.0), f64::from(self.viewport.1))?;
        self.set_screen(Screen::Pause)
    }

    /// Source requestGameSettingsUpdate rejects enabling cheats in the global Hub
    /// until this explicit choice. The coordinator retains the accepted settings.
    pub fn show_cheat_college_prompt(&mut self) -> Result<(), String> {
        self.routing.show_cheats()
    }

    /// The host supplies accepted console authority, never the saved preference.
    pub fn set_cheat_menu_available(&mut self, available: bool) -> Result<(), String> {
        if self.cheat_menu_available == available {
            return Ok(());
        }
        self.cheat_menu_available = available;
        // Settings overlays Pause; Back must reveal rows for the accepted authority.
        if (self.screen == Screen::Pause
            || (self.screen == Screen::Settings && self.settings_back == Screen::Pause))
            && self.pause_back != Screen::Cloud
        {
            self.set_gameplay_pause_rows()?;
            self.pause.resize(f64::from(self.viewport.0), f64::from(self.viewport.1))?;
        }
        Ok(())
    }

    fn set_gameplay_pause_rows(&mut self) -> Result<(), String> {
        let mut rows = vec![("RESUME GAME", "resume")];
        if self.cheat_menu_available {
            rows.push(("CHEAT MENU", "pause-cheats"));
        }
        rows.extend([("GAME SETTINGS", "pause-settings"), ("LEAVE GAME", "leave")]);
        self.pause.set_rows(&rows)
    }

    fn show_cloud_pause(&mut self) -> Result<(), String> {
        if self.cloud.back()? {
            return Ok(());
        }
        self.pause_back = Screen::Cloud;
        self.pending_pause = None;
        self.pause_started = self.now;
        let mut rows = vec![("RESUME", "resume"), ("GAME SETTINGS", "pause-settings")];
        if self.account_username.is_some() {
            rows.push(("SIGN OUT", "cloud-sign-out"));
        }
        rows.push(("MAIN MENU", "leave"));
        self.pause.set_rows(&rows)?;
        self.pause.resize(f64::from(self.viewport.0), f64::from(self.viewport.1))?;
        self.audio_events.push(AudioEvent::Play(AudioCue::OpenPanel, AudioOptions::default()));
        self.set_screen(Screen::Pause)
    }

    pub fn show_settings(&mut self) -> Result<(), String> {
        let context = if matches!(self.screen, Screen::Title | Screen::Play) {
            SettingsContext::Title
        } else if self.cloud_visible(self.screen) {
            SettingsContext::DarkCloud
        } else {
            SettingsContext::Gameplay
        };
        self.show_settings_in_context(context)
    }

    pub fn show_settings_in_context(&mut self, context: SettingsContext) -> Result<(), String> {
        self.settings_back = self.screen;
        self.settings_view.set(&self.settings)?;
        self.settings_view.open(context)?;
        self.set_screen(Screen::Settings)
    }

    pub fn set_settings_account_username(&mut self, username: Option<&str>) -> Result<(), String> {
        self.account_username = username.map(str::to_owned);
        self.title.set_account_username(username)?;
        self.settings_view.set_account_username(username)
    }

    pub fn set_mobile_layout_state(
        &mut self,
        state: &solomon_sim::mobile_ui::MobileUiLayoutState,
    ) -> Result<(), String> {
        self.settings_view.set_mobile_layout_state(state)?;
        self.cloud.set_mobile_layout_customized(state.customized)
    }

    pub fn set_mobile_editor_player(
        &mut self,
        player: Option<&solomon_sim::player::PlayerState>,
        in_hub: bool,
    ) {
        self.settings_view.set_mobile_editor_player(player, in_hub);
    }

    pub fn set_mobile_editor_secondary(
        &mut self,
        secondary: Option<&solomon_sim::secondary::NativeSecondaryPlayerState>,
    ) {
        self.settings_view.set_mobile_editor_secondary(secondary);
    }

    pub fn complete_mobile_layout_save(
        &mut self,
        result: Result<solomon_sim::mobile_ui::MobileUiLayoutState, String>,
    ) -> Result<(), String> {
        if let Ok(state) = &result {
            self.cloud.set_mobile_layout_customized(state.customized)?;
        }
        self.settings_view.complete_mobile_layout_save(result)
    }

    pub fn set_save_transfer_available(&mut self, can_export: bool) -> Result<(), String> {
        self.settings_view.set_save_transfer_available(can_export)
    }

    pub fn complete_save_inspection(
        &mut self,
        request_id: u64,
        result: Result<solomon_assets::native_save::GameSaveImportPreview, String>,
    ) -> Result<bool, String> {
        self.settings_view.complete_save_inspection(request_id, result)
    }

    pub fn complete_save_export(
        &mut self,
        request_id: u64,
        result: Result<solomon_assets::native_save::PortableExportResult, String>,
    ) -> Result<bool, String> {
        self.settings_view.complete_save_export(request_id, result)
    }

    pub fn complete_save_replacement(
        &mut self,
        request_id: u64,
        result: Result<(), String>,
    ) -> Result<bool, String> {
        self.settings_view.complete_save_replacement(request_id, result)
    }

    /// Source `initialCreateWizardNameForSession`: guests roll a stock name;
    /// account names pass through the same glyph/width admission as Create.
    /// The coordinator stores its party requester name once at MainMenu mount.
    #[must_use]
    pub fn initial_wizard_name(&self, account_username: Option<&str>) -> String {
        if let Some(name) = account_username.filter(|name| !name.is_empty()) {
            return solomon_sim::menu::create_name::initial_create_wizard_name(name);
        }
        let index = (js_sys::Math::random() * self.names.len() as f64).floor() as usize;
        self.names[index].clone()
    }

    pub fn hide(&mut self) -> Result<(), String> {
        self.set_screen(Screen::Hidden)
    }
    pub fn is_blocking(&self) -> bool {
        self.screen != Screen::Hidden || self.routing.active()
    }
    pub fn covers_world(&self) -> bool {
        if self.cloud_visible(self.screen) {
            return true;
        }
        matches!(
            self.screen,
            Screen::Title | Screen::Play | Screen::Join | Screen::Create | Screen::Hall
        ) || (self.screen == Screen::Settings
            && matches!(self.settings_back, Screen::Title | Screen::Play))
    }
    /// Source edge controls follow the scene beneath the Settings overlay.
    pub fn fullscreen_placement(&self) -> crate::fullscreen::FullscreenPlacement {
        use crate::fullscreen::FullscreenPlacement;
        if self.cloud_visible(self.screen) {
            return FullscreenPlacement::DarkCloud;
        }
        let screen = if self.screen == Screen::Settings { self.settings_back } else { self.screen };
        match screen {
            Screen::Title | Screen::Play | Screen::Hall => FullscreenPlacement::Title,
            Screen::Create => FullscreenPlacement::Create {
                // `main-menu.css`: min(0.625cqw, 1.111111cqh) is ten scaled stage pixels.
                frame_scale: (f64::from(self.viewport.0) / 1600.0)
                    .min(f64::from(self.viewport.1) / 900.0),
            },
            _ => FullscreenPlacement::Gameplay,
        }
    }
    pub fn take_actions(&mut self) -> Vec<MenuAction> {
        std::mem::take(&mut self.actions)
    }
    pub fn settings(&self) -> &GameSettings {
        &self.settings
    }
    pub fn set_settings(&mut self, settings: GameSettings) -> Result<(), String> {
        self.settings_view.set(&settings)?;
        self.settings = settings;
        Ok(())
    }

    pub fn take_audio_events(&mut self) -> Vec<AudioEvent> {
        std::mem::take(&mut self.audio_events)
    }

    #[must_use]
    pub fn music_scene(&self) -> Option<MusicScene> {
        if self.cloud_visible(self.screen) {
            return Some(MusicScene::Title);
        }
        match self.screen {
            Screen::Title | Screen::Play | Screen::Join | Screen::Hall => Some(MusicScene::Title),
            Screen::Create => Some(MusicScene::Create),
            Screen::Settings if matches!(self.settings_back, Screen::Title | Screen::Play) => {
                Some(MusicScene::Title)
            }
            _ => None,
        }
    }

    pub fn update(&mut self, now: f64, width: i32, height: i32) -> Result<(), String> {
        if self.now == 0.0 {
            self.title_started = now;
        }
        self.now = now;
        if self.viewport != (width, height) {
            self.title.resize(f64::from(width), f64::from(height))?;
            self.create.resize(f64::from(width), f64::from(height))?;
            self.pause.resize(f64::from(width), f64::from(height))?;
            self.prompt.resize(width, height)?;
            self.hall.resize(width, height)?;
            self.routing.resize(f64::from(width), f64::from(height))?;
            self.viewport = (width, height);
        }
        let events = std::mem::take(&mut *self.events.borrow_mut());
        for event in events {
            self.handle_event(event)?;
        }
        if self.screen == Screen::Settings {
            self.settings_view.update()?;
            if let Some(layout) = self.settings_view.take_mobile_layout_save() {
                self.actions.push(MenuAction::SaveMobileLayout(layout.map(Box::new)));
            }
        }
        if self.screen == Screen::Join {
            let visible_height = web_sys::window()
                .and_then(|window| window.visual_viewport())
                .map_or(f64::from(height), |viewport| viewport.height());
            self.join.update(now, visible_height)?;
        }
        if self.cloud_visible(self.screen) {
            self.cloud.update(now)?;
        }
        if self.is_blocking() && self.pending_start.is_none() && !self.input_suspended {
            let surface = self.active_surface().clone();
            let scope = if self.prompt.kind.is_some() {
                "title-prompt".to_string()
            } else {
                self.root.get_attribute("data-menu-screen").ok_or("menu has no navigation scope")?
            };
            if self.navigation.update(&surface, &self.document, &scope, now)? {
                self.back()?;
            }
        } else {
            self.navigation.release();
        }
        if self.screen == Screen::Title
            || self.screen == Screen::Play
            || (self.screen == Screen::Settings
                && matches!(self.settings_back, Screen::Title | Screen::Play))
        {
            self.title.update(self.now - self.title_started)?;
        }
        if self.screen == Screen::Create {
            let presentation_now = self
                .pending_start
                .as_ref()
                .filter(|pending| pending.kind == CreateCommitKind::Randomize)
                .map_or(self.now, |pending| pending.started_at);
            let phase_elapsed = presentation_now - self.phase_started;
            let audio = if let Some(element) = self.selected_element {
                create_selection_audio_events(
                    element,
                    self.create_previous_phase_elapsed,
                    phase_elapsed,
                )
            } else {
                create_entry_audio_events(self.create_previous_phase_elapsed, phase_elapsed)
            };
            self.create_previous_phase_elapsed = phase_elapsed;
            for event in audio {
                self.audio_events.push(event.into());
            }
            self.create.update(
                self.selected_element,
                phase_elapsed,
                self.now - self.create_started,
                self.settings.reduced_screen_flashes,
                self.pending_start.is_some(),
            )?;
            if let Some(pending) = &mut self.pending_start {
                let duration = match pending.kind {
                    CreateCommitKind::Manual => 880.0,
                    CreateCommitKind::Randomize => NATIVE_CREATE_RANDOMIZE_FINALIZE_MS as f64,
                };
                if !pending.submitted && self.now - pending.started_at >= duration {
                    pending.submitted = true;
                    if pending.kind == CreateCommitKind::Manual {
                        self.audio_events
                            .push(AudioEvent::Play(AudioCue::CatchIt, AudioOptions::default()));
                    }
                    let selection = pending.selection.clone();
                    self.actions.push(if self.loadout_locked {
                        MenuAction::ConfirmLoadout(selection)
                    } else {
                        MenuAction::Start(selection)
                    });
                }
            }
        }
        if self.screen == Screen::Hall {
            self.hall.update(now)?;
        }
        if self.screen == Screen::Pause {
            let (opening, elapsed) =
                self.pending_pause.map_or((true, self.now - self.pause_started), |(started, _)| {
                    (false, self.now - started)
                });
            let reveal = native_pause_menu_reveal(opening, elapsed);
            self.pause.update(reveal)?;
            if !opening && reveal == 0.0 {
                match self.pending_pause.take().expect("closing pause has an action").1 {
                    PauseChoice::Resume if self.pause_back == Screen::Cloud => {
                        self.set_screen(Screen::Cloud)?
                    }
                    PauseChoice::Leave if self.pause_back == Screen::Cloud => {
                        self.show_title(self.can_resume)?
                    }
                    PauseChoice::Settings if self.pause_back == Screen::Cloud => {
                        self.set_screen(Screen::Cloud)?;
                        self.show_settings_in_context(SettingsContext::DarkCloud)?;
                    }
                    PauseChoice::SignOut => {
                        self.actions.retain(|action| !matches!(action, MenuAction::Resume));
                        self.hosted_sign_out();
                        self.show_title(self.can_resume)?;
                    }
                    PauseChoice::Resume => {
                        self.actions.push(MenuAction::Resume);
                        self.hide()?;
                    }
                    PauseChoice::Cheats => {
                        self.actions.push(MenuAction::Cheats);
                        self.hide()?;
                    }
                    PauseChoice::Leave => {
                        self.actions.push(MenuAction::Leave);
                        self.hide()?;
                    }
                    PauseChoice::Settings => {
                        self.show_settings()?;
                    }
                }
            }
        }
        Ok(())
    }

    fn handle_event(&mut self, event: UiEvent) -> Result<(), String> {
        if self.pending_start.is_some() || self.input_suspended {
            return Ok(());
        }
        if self.routing.active() {
            if let UiEvent::Action(action) = event {
                self.routing.activate(&action)?;
                self.root.set_hidden(self.screen == Screen::Hidden && !self.routing.active());
            }
            // The pinned routing dialogs provide buttons/controller Back; they
            // do not add an Escape listener to the scene underneath them.
            return Ok(());
        }
        match event {
            UiEvent::SettingsChanged if self.screen == Screen::Settings => {
                self.settings_changed()?
            }
            UiEvent::Key(code, editing) => {
                if self.screen == Screen::Cloud {
                    if !self.cloud.modal_open() && code == self.settings.controls.open_menu {
                        self.show_cloud_pause()?;
                    }
                    return Ok(());
                }
                if self.screen == Screen::Pause && self.pause_back == Screen::Cloud {
                    return Ok(());
                }
                // JoinPartyScene provides footer/controller Back, not a keyboard shortcut.
                if self.screen == Screen::Join {
                    return Ok(());
                }
                if self.screen == Screen::Settings && self.settings_view.awaiting_binding() {
                    if self.settings_view.capture_binding(&code)? {
                        self.settings_changed()?;
                    }
                    return Ok(());
                }
                if (code == "Escape" || code == self.settings.controls.open_menu)
                    && (!editing || code == "Escape")
                {
                    self.back()?;
                }
            }
            UiEvent::Pointer(button)
                if self.screen == Screen::Settings
                    && self.settings_view.awaiting_mouse_binding() =>
            {
                if self.settings_view.capture_binding(&format!("Mouse{button}"))? {
                    self.settings_changed()?;
                }
            }
            UiEvent::Action(action) => self.activate(&action)?,
            _ => {}
        }
        Ok(())
    }

    fn activate(&mut self, action: &str) -> Result<(), String> {
        if self.screen == Screen::Cloud {
            if action == "cloud-menu" {
                self.show_cloud_pause()?;
                return Ok(());
            }
            if self.cloud.activate(action, self.now)? {
                return Ok(());
            }
        }
        if self.screen == Screen::Join {
            if action == "join-back" {
                self.back()?;
                return Ok(());
            }
            if self.join.activate(action, self.now)? {
                return Ok(());
            }
        }
        if self.screen == Screen::Settings {
            if action == "settings-mobile-publish" {
                let request_id = self.cloud.reserve_layout_request();
                if self.settings_view.begin_mobile_layout_publication(request_id)? {
                    self.hosted_actions.push(HostedMenuAction::PublishMobileLayout { request_id });
                }
                return Ok(());
            }
            if action == "settings-toggle:enhanced-effects" {
                if let Some(enabled) = self.settings_view.requested_enhanced_effects() {
                    self.actions.push(MenuAction::SetEnhancedEffects { enabled });
                }
                return Ok(());
            }
            if action == "settings-back" {
                self.settings_changed()?;
                if !self.settings_view.back()? {
                    self.set_screen(self.settings_back)?;
                }
                return Ok(());
            }
            if self.settings_view.activate(action)? {
                self.settings_changed()?;
                return Ok(());
            }
        }
        if self.screen == Screen::Hall {
            if self.hall.activate(action) {
                return Ok(());
            }
            if let Some(index) = action.strip_prefix("hall-toggle:") {
                self.hall.toggle(index.parse().map_err(|_| "invalid Hall row")?);
                return Ok(());
            }
        }
        if self.prompt.kind.is_some() && !matches!(action, "prompt-primary" | "prompt-secondary") {
            return Ok(());
        }
        if !action.starts_with("element:") && !action.starts_with("discipline:") {
            self.audio_events.push(AudioEvent::Play(AudioCue::Click, AudioOptions::default()));
        }
        match action {
            "play" => {
                self.title.show_controls(true, self.can_resume)?;
                self.set_screen(Screen::Play)?;
            }
            "title" => {
                self.title.show_controls(false, self.can_resume)?;
                self.set_screen(Screen::Title)?;
            }
            "continue" if self.can_resume => {
                self.actions.push(MenuAction::Continue);
                self.hide()?;
            }
            "new-game" if self.can_resume => self.show_prompt(TitlePromptKind::KillWizard)?,
            "new-game" => self.begin_new_game()?,
            "prompt-primary" | "prompt-secondary" => {
                let kind = self.prompt.kind;
                self.hide_prompt()?;
                match (kind, action == "prompt-primary") {
                    (Some(TitlePromptKind::KillWizard), true) => self.begin_new_game()?,
                    (Some(TitlePromptKind::Tutorial), true) => {
                        self.actions.push(MenuAction::StartTutorial);
                    }
                    (Some(TitlePromptKind::Tutorial), false) => {
                        self.actions.push(MenuAction::TutorialDeclined);
                    }
                    _ => {}
                }
            }
            "settings" => self.show_settings()?,
            "hall" => self.show_hall()?,
            "join-party" if self.desktop => self.desktop_action(false),
            "explore" if self.desktop => self.desktop_action(true),
            "join-party" => self.show_join_party()?,
            "explore" => self.show_dark_cloud()?,
            "hall-back" => {
                self.title.show_controls(false, self.can_resume)?;
                self.set_screen(Screen::Title)?;
            }
            "resume" => self.begin_pause_close(PauseChoice::Resume),
            "pause-settings" => self.begin_pause_close(PauseChoice::Settings),
            "pause-cheats" if self.cheat_menu_available => {
                self.begin_pause_close(PauseChoice::Cheats)
            }
            "leave" => self.begin_pause_close(PauseChoice::Leave),
            "cloud-sign-out" => self.begin_pause_close(PauseChoice::SignOut),
            "create-back" => self.back()?,
            "clear-name" => {
                self.create.set_name("")?;
                self.create.name.focus().map_err(|_| "focus wizard name")?;
            }
            "randomize-name" => {
                let index = (js_sys::Math::random() * self.names.len() as f64).floor() as usize;
                self.create.set_name(&self.names[index])?;
                self.create.message.set_text_content(None);
            }
            "randomize-loadout" => self.randomize_create_loadout()?,
            _ => {
                if let Some(id) = action.strip_prefix("element:") {
                    if self.screen != Screen::Create || self.selected_element.is_some() {
                        return Ok(());
                    }
                    self.selected_element =
                        WizardElement::ALL.into_iter().find(|element| element.as_str() == id);
                    self.audio_events
                        .push(AudioEvent::Play(AudioCue::PickSkill, AudioOptions::default()));
                    self.phase_started = self.now;
                    self.create_previous_phase_elapsed = 0.0;
                    self.create.restart();
                } else if let Some(id) = action.strip_prefix("discipline:") {
                    let Some(element) = self.selected_element else {
                        return Ok(());
                    };
                    let Some(discipline) = WizardDiscipline::CHOICES
                        .into_iter()
                        .find(|discipline| discipline.as_str() == id)
                    else {
                        return Err("unknown Create discipline".to_string());
                    };
                    match self.create.finalized_name() {
                        Ok(name) => {
                            self.audio_events.push(AudioEvent::Play(
                                AudioCue::PickSkill,
                                AudioOptions::default(),
                            ));
                            self.begin_create_commit(
                                NewGameSelection { name, element, discipline },
                                CreateCommitKind::Manual,
                            )?;
                        }
                        Err(message) => self.create.message.set_text_content(Some(&message)),
                    }
                }
            }
        }
        Ok(())
    }

    fn settings_changed(&mut self) -> Result<(), String> {
        let settings = self.settings_view.read()?;
        if settings != self.settings {
            // The coordinator may require private-College consent. Keep accepted
            // preferences until it commits the requested draft with set_settings.
            self.actions.push(MenuAction::SettingsChanged(Box::new(settings)));
        }
        Ok(())
    }

    fn begin_new_game(&mut self) -> Result<(), String> {
        // MainMenuScene starts College admission before asking for the final loadout.
        // The generated name is presentation randomness, independent of the run RNG.
        let name = self.initial_wizard_name(self.account_username.as_deref());
        self.actions.push(MenuAction::Start(NewGameSelection {
            name,
            element: WizardElement::Ether,
            discipline: WizardDiscipline::Arcane,
        }));
        self.hide()
    }

    pub fn show_create(&mut self) -> Result<(), String> {
        self.loadout_locked = false;
        self.set_create_back_enabled(true)?;
        self.selected_element = None;
        self.pending_start = None;
        self.create_rng = None;
        self.create_previous_phase_elapsed = 0.0;
        self.create.root.remove_attribute("inert").map_err(|_| "Create interactivity")?;
        self.create
            .root
            .remove_attribute("data-finalizing-kind")
            .map_err(|_| "Create finalization kind")?;
        self.create
            .root
            .set_attribute("data-finalizing", "false")
            .map_err(|_| "Create finalization state")?;
        self.create_started = self.now;
        self.phase_started = self.now;
        self.create.restart();
        self.set_screen(Screen::Create)
    }

    #[must_use]
    pub fn is_create_pending(&self) -> bool {
        self.pending_start.is_some()
    }

    pub fn set_enhanced_effects_authority(
        &mut self,
        authority: Option<solomon_protocol::hosted::EnhancedEffectsAuthority>,
    ) -> Result<(), String> {
        self.settings_view.set_enhanced_effects_authority(authority)
    }

    /// Source `startCreateLoadoutFinalization` waits for admission. A failed
    /// submission releases this draft and its native RNG for a real retry.
    pub fn complete_create_loadout(&mut self, started: bool) -> Result<(), String> {
        let Some(pending) = self.pending_start.take() else { return Ok(()) };
        if started {
            return self.hide();
        }
        if pending.kind == CreateCommitKind::Randomize {
            self.phase_started += self.now - pending.started_at;
        }
        self.create.root.remove_attribute("inert").map_err(|_| "Create retry admission")?;
        self.create
            .root
            .set_attribute("data-finalizing", "false")
            .map_err(|_| "Create retry state")?;
        self.create
            .root
            .remove_attribute("data-finalizing-kind")
            .map_err(|_| "Create retry kind")?;
        self.create.restart();
        self.set_screen(Screen::Create)
    }

    fn begin_create_commit(
        &mut self,
        selection: NewGameSelection,
        kind: CreateCommitKind,
    ) -> Result<(), String> {
        self.pending_start =
            Some(PendingCreateCommit { started_at: self.now, kind, selection, submitted: false });
        self.create.root.set_attribute("inert", "").map_err(|_| "Create finalization")?;
        self.create
            .root
            .set_attribute("data-finalizing", "true")
            .map_err(|_| "Create finalization state")?;
        self.create
            .root
            .set_attribute(
                "data-finalizing-kind",
                if kind == CreateCommitKind::Manual { "manual" } else { "randomize" },
            )
            .map_err(|_| "Create finalization kind")?;
        Ok(())
    }

    fn randomize_create_loadout(&mut self) -> Result<(), String> {
        if self.screen != Screen::Create
            || self.now - self.create_started < NATIVE_CREATE_ENTRY_FADE_MS as f64
        {
            return Ok(());
        }
        let name = match self.create.finalized_name() {
            Ok(name) => name,
            Err(message) => {
                self.create.message.set_text_content(Some(&message));
                return Ok(());
            }
        };
        if self.create_rng.is_none() {
            // Create owns this cosmetic-selection stream. It must not consume
            // the accepted player's gameplay, appearance or progression RNG.
            let mut bytes = [0_u8; 4];
            self.document
                .default_view()
                .ok_or("Create window")?
                .crypto()
                .map_err(|_| "Create random source")?
                .get_random_values_with_u8_array(&mut bytes)
                .map_err(|_| "Create random seed")?;
            self.create_rng = Some(NativeRng::new(i64::from(u32::from_ne_bytes(bytes))));
        }
        let selection = finalize_native_create_loadout(
            self.create_rng.as_mut().expect("Create random stream initialized"),
            &NativeCreateLoadoutSelection {
                name,
                element: self.selected_element,
                discipline: None,
            },
            &[],
        );
        self.audio_events.push(AudioEvent::Play(AudioCue::Click, AudioOptions::default()));
        self.begin_create_commit(selection, CreateCommitKind::Randomize)
    }

    /// College loadout shares Create's native sequence but cannot leave its admission hold.
    pub fn show_loadout(&mut self, name: &str) -> Result<(), String> {
        self.show_create()?;
        self.create.set_name(name)?;
        self.loadout_locked = true;
        self.set_create_back_enabled(false)
    }

    fn set_create_back_enabled(&self, enabled: bool) -> Result<(), String> {
        let back = self
            .create
            .root
            .query_selector("[data-menu-action='create-back']")
            .map_err(|_| "Create Back control")?
            .ok_or("Create has no Back control")?;
        back.toggle_attribute_with_force("disabled", !enabled)
            .map_err(|_| "Create Back availability")?;
        Ok(())
    }

    fn show_prompt(&mut self, kind: TitlePromptKind) -> Result<(), String> {
        self.prompt.show(kind)?;
        self.title.set_prompt_visible(true);
        self.title.root.set_attribute("inert", "").map_err(|_| "title prompt ownership")?;
        self.navigation.release();
        Ok(())
    }

    fn hide_prompt(&mut self) -> Result<(), String> {
        self.title.set_prompt_visible(false);
        self.prompt.hide();
        self.title.root.remove_attribute("inert").map_err(|_| "title prompt handoff")?;
        self.navigation.release();
        Ok(())
    }

    fn begin_pause_close(&mut self, choice: PauseChoice) {
        if self.screen == Screen::Pause && self.pending_pause.is_none() {
            self.pending_pause = Some((self.now, choice));
        }
    }

    fn back(&mut self) -> Result<(), String> {
        if self.routing.active() {
            self.routing.back();
            self.root.set_hidden(self.screen == Screen::Hidden && !self.routing.active());
            return Ok(());
        }
        if self.prompt.kind.is_some() {
            return self.activate("prompt-secondary");
        }
        match self.screen {
            Screen::Cloud => self.show_cloud_pause()?,
            Screen::Hidden => self.show_pause()?,
            Screen::Pause => self.begin_pause_close(PauseChoice::Resume),
            Screen::Settings => {
                self.settings_changed()?;
                if !self.settings_view.back()? {
                    self.set_screen(self.settings_back)?;
                }
            }
            Screen::Create if self.loadout_locked => {}
            Screen::Create if self.selected_element.is_some() => {
                self.audio_events.push(AudioEvent::StopStream(AudioCue::StartCast));
                self.audio_events.push(AudioEvent::StopStream(AudioCue::ChooseElement));
                self.selected_element = None;
                self.phase_started = self.now;
                self.create_started = self.now;
                self.create_previous_phase_elapsed = 0.0;
                self.create.restart();
            }
            Screen::Create => {
                self.actions.push(MenuAction::CancelCreate);
                self.title.show_controls(true, self.can_resume)?;
                self.set_screen(Screen::Play)?;
            }
            Screen::Play => {
                self.title.show_controls(false, self.can_resume)?;
                self.set_screen(Screen::Title)?;
            }
            Screen::Join => {
                self.title.show_controls(true, self.can_resume)?;
                self.set_screen(Screen::Play)?;
            }
            Screen::Hall => {
                self.title.show_controls(false, self.can_resume)?;
                self.set_screen(Screen::Title)?;
            }
            Screen::Title => {}
        }
        Ok(())
    }

    fn set_screen(&mut self, screen: Screen) -> Result<(), String> {
        self.hide_prompt()?;
        self.routing.close();
        if self.cloud_visible(self.screen) && !self.cloud_visible(screen) {
            self.cloud.close();
        }
        if self.screen == Screen::Create && screen != Screen::Create {
            self.audio_events.push(AudioEvent::StopStream(AudioCue::StartCast));
            self.audio_events.push(AudioEvent::StopStream(AudioCue::ChooseElement));
        }
        if self.screen == Screen::Hall && screen != Screen::Hall {
            self.hall.close();
        }
        if self.screen == Screen::Settings && screen != Screen::Settings {
            self.settings_view.close()?;
        }
        if self.screen == Screen::Join && screen != Screen::Join {
            self.join.close();
        }
        self.screen = screen;
        self.orientation_hint.set_hidden(self.cloud_visible(screen));
        self.root
            .set_attribute(
                "data-orientation-gate",
                if self.cloud_visible(screen) { "false" } else { "true" },
            )
            .map_err(|_| "orientation scene admission")?;
        self.root.set_hidden(screen == Screen::Hidden);
        self.title.root.set_hidden(
            !(matches!(screen, Screen::Title | Screen::Play)
                || (screen == Screen::Settings
                    && matches!(self.settings_back, Screen::Title | Screen::Play))),
        );
        self.title
            .root
            .toggle_attribute_with_force("inert", screen == Screen::Settings)
            .map_err(|_| "title modal ownership")?;
        self.create.root.set_hidden(screen != Screen::Create);
        self.hall.root.set_hidden(screen != Screen::Hall);
        self.join.root.set_hidden(screen != Screen::Join);
        self.cloud.root.set_hidden(!self.cloud_visible(screen));
        self.cloud
            .root
            .toggle_attribute_with_force("inert", screen != Screen::Cloud)
            .map_err(|_| "Dark Cloud overlay input")?;
        self.settings_view.root.set_hidden(screen != Screen::Settings);
        self.pause.root.set_hidden(screen != Screen::Pause);
        let name = match screen {
            Screen::Hidden => "hidden",
            Screen::Title => "title",
            Screen::Play => "play",
            Screen::Join => "join-party",
            Screen::Cloud => "dark-cloud",
            Screen::Create => "create",
            Screen::Hall => "hall",
            Screen::Pause => "pause",
            Screen::Settings => "settings",
        };
        self.root.set_attribute("data-menu-screen", name).map_err(|_| "menu screen")?;
        self.root
            .set_attribute("data-cloud-menu-binding", &self.settings.controls.open_menu)
            .map_err(|_| "Dark Cloud menu binding")?;
        if matches!(screen, Screen::Create | Screen::Join) {
            if let Some(active) = self.document.active_element()
                && let Some(active) = active.dyn_ref::<HtmlElement>()
            {
                active.blur().map_err(|_| "Create focus handoff")?;
            }
            return Ok(());
        }
        if let Some(button) = self
            .active_surface()
            .query_selector("button:not([disabled]), input:not([disabled]), select:not([disabled])")
            .map_err(|_| "default menu focus")?
        {
            let button: HtmlElement =
                button.dyn_into().map_err(|_| "menu focus target is not HTML")?;
            button.focus().map_err(|_| "menu focus")?;
        }
        Ok(())
    }

    fn active_surface(&self) -> &HtmlElement {
        if self.routing.active() {
            return &self.routing.root;
        }
        if self.prompt.kind.is_some() {
            return &self.prompt.root;
        }
        match self.screen {
            Screen::Title | Screen::Play => &self.title.root,
            Screen::Create => &self.create.root,
            Screen::Hall => &self.hall.root,
            Screen::Join => &self.join.root,
            Screen::Cloud => self.cloud.active_surface(),
            Screen::Pause => &self.pause.root,
            Screen::Settings => &self.settings_view.root,
            Screen::Hidden => &self.root,
        }
    }

    fn cloud_visible(&self, screen: Screen) -> bool {
        screen == Screen::Cloud
            || (screen == Screen::Pause && self.pause_back == Screen::Cloud)
            || (screen == Screen::Settings
                && (self.settings_back == Screen::Cloud
                    || (self.settings_back == Screen::Pause && self.pause_back == Screen::Cloud)))
    }

    fn hosted_sign_out(&mut self) {
        self.hosted_actions.push(HostedMenuAction::SignOut);
    }
}

fn menu_action_target(event: &Event) -> Result<Option<String>, String> {
    let Some(target) = event.target() else {
        return Ok(None);
    };
    let Ok(target) = target.dyn_into::<Element>() else {
        return Ok(None);
    };
    let Some(button) = target.closest("[data-menu-action]").map_err(|_| "menu action selector")?
    else {
        return Ok(None);
    };
    if button.has_attribute("disabled") {
        return Ok(None);
    }
    // Forms own submit, not every pointer click inside an editable field.
    if button.tag_name() == "FORM" && event.type_() != "submit" {
        return Ok(None);
    }
    Ok(button.get_attribute("data-menu-action"))
}

struct PauseView {
    root: HtmlElement,
    stage: HtmlElement,
    frame: HtmlElement,
    header: HtmlElement,
    arrows: Vec<(HtmlElement, f64, f64, f64)>,
    atlas: StockAtlas,
    document: Document,
    previous: Option<f64>,
    row_count: usize,
}

impl PauseView {
    async fn new(document: &Document) -> Result<Self, String> {
        let atlas = StockAtlas::load("UI").await?;
        let root = element(document, "section", "menu-screen pause-screen")?;
        root.set_attribute("role", "dialog").map_err(|_| "pause role")?;
        root.set_attribute("aria-modal", "true").map_err(|_| "pause modal")?;
        root.set_attribute("aria-label", "Game paused").map_err(|_| "pause label")?;
        let stage = element(document, "div", "pause-stage")?;
        for (index, (label, action)) in [
            ("RESUME GAME", "resume"),
            ("GAME SETTINGS", "pause-settings"),
            ("LEAVE GAME", "leave"),
        ]
        .into_iter()
        .enumerate()
        {
            let button = native_button(document, label, action)?;
            style(
                &button,
                &format!("position:absolute;left:623.5px;top:{}px", 339.5 + index as f64 * 76.0),
            )?;
            append(&stage, &button)?;
        }
        let frame = element(document, "div", "pause-frame")?;
        append(&stage, &frame)?;
        let header = atlas.sprite(document, 18, "pause-header")?;
        append(&stage, &header)?;
        let mut arrows = Vec::new();
        for (x, offset, scale) in [(800.0, 55.0, 1.0), (725.0, 42.0, 0.75), (875.0, 42.0, 0.75)] {
            let arrow = atlas.sprite(document, 8, "pause-arrow")?;
            append(&stage, &arrow)?;
            arrows.push((arrow, x, offset, scale));
        }
        append(&root, &stage)?;
        Ok(Self {
            root,
            stage,
            frame,
            header,
            arrows,
            atlas,
            document: document.clone(),
            previous: None,
            row_count: 3,
        })
    }

    fn set_rows(&mut self, rows: &[(&str, &str)]) -> Result<(), String> {
        let previous = self
            .stage
            .query_selector_all(".native-button")
            .map_err(|_| "SimpleMenu existing rows")?;
        for index in 0..previous.length() {
            if let Some(button) = previous.item(index) {
                self.stage.remove_child(&button).map_err(|_| "SimpleMenu old row")?;
            }
        }
        self.row_count = rows.len();
        self.previous = None;
        let top = (900.0 - self.body_height()) / 2.0;
        for (index, &(label, action)) in rows.iter().enumerate() {
            let button = native_button(&self.document, label, action)?;
            style(
                &button,
                &format!("position:absolute;left:623.5px;top:{}px", top + index as f64 * 76.0),
            )?;
            append(&self.stage, &button)?;
        }
        Ok(())
    }

    fn body_height(&self) -> f64 {
        self.row_count as f64 * 76.0 - 7.0
    }

    fn resize(&self, width: f64, height: f64) -> Result<(), String> {
        let native = (width / 1600.0).min(height / 900.0);
        let scale = if native < 44.0 / 69.0 {
            // `nativePauseMenuStagePlacement` includes the header and lowest
            // arrow around the actual three/four-row SimpleMenu body.
            1.0_f64
                .min((width / 2.0 - 12.0) / 216.5)
                .min((height / 2.0 - 12.0) / (self.body_height() / 2.0 + 151.0))
        } else {
            native
        };
        style(
            &self.stage,
            &format!(
                "left:{}px;top:{}px;transform:scale({scale})",
                (width - 1600.0 * scale) / 2.0,
                (height - 900.0 * scale) / 2.0
            ),
        )
    }

    fn update(&mut self, reveal: f64) -> Result<(), String> {
        if self.previous == Some(reveal) {
            return Ok(());
        }
        self.previous = Some(reveal);
        let spread = solomon_sim::float::fround((1.0 - reveal) * 25.0 + 40.0);
        let body_height = self.body_height();
        let top = (900.0 - body_height) / 2.0 - spread;
        let bottom = top + body_height + spread * 2.0;
        self.root
            .style()
            .set_property(
                "background-color",
                &format!("rgba(0, 0, 0, {})", solomon_sim::float::fround(reveal * 0.85)),
            )
            .map_err(|_| "pause curtain")?;
        self.stage
            .style()
            .set_property("opacity", &reveal.to_string())
            .map_err(|_| "pause reveal")?;
        self.frame.set_text_content(None);
        let art = self.atlas.nine_slice(
            &self.document,
            17,
            353.0 + spread * 2.0,
            body_height + spread * 2.0,
        )?;
        art.style()
            .set_property("left", &format!("{}px", 623.5 - spread))
            .map_err(|_| "pause frame x")?;
        art.style().set_property("top", &format!("{top}px")).map_err(|_| "pause frame y")?;
        append(&self.frame, &art)?;
        self.header
            .style()
            .set_property("top", &format!("{}px", top - 42.0))
            .map_err(|_| "pause header position")?;
        for (arrow, x, offset, scale) in &self.arrows {
            arrow.style().set_property("left", &format!("{x}px")).map_err(|_| "pause arrow x")?;
            arrow
                .style()
                .set_property("top", &format!("{}px", bottom + offset))
                .map_err(|_| "pause arrow y")?;
            arrow
                .style()
                .set_property("transform", &format!("translate(-50%, -50%) scale({scale})"))
                .map_err(|_| "pause arrow scale")?;
        }
        Ok(())
    }
}
