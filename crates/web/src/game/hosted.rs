//! Website-backed browser lifecycle. The native connection owns accepted gameplay;
//! this owner resolves account/content admission and persists the host's save stream.

use std::{cell::RefCell, rc::Rc};

use super::hosted_save::{HostedSaveCoordinator, SaveBatch, SaveOutcome, SaveReceipt};
use super::*;
use crate::hosted::{
    Account, ActiveWebMod, BrowserAdmission, PartyJoinRequestReceipt, PartyJoinRequestStatus,
    PartyJoinResolution, ProvisionedGameEndpoint, PublicGameParty, WebsiteApi, WebsiteMount,
};
use crate::menu::HostedMenuAction;
use solomon_assets::website_save::{self, ParsedSaveDocument, SaveIntegrity};
use solomon_protocol::hosted::{
    GameSaveIntent, HostedCommand, HostedHello, HostedServerMessage, ModAction,
};
use solomon_sim::modding::{LuaObject, LuaValue};
use solomon_sim::settings::{game_online_preferences, game_shared_hub_enabled};
use solomon_sim::social::PlayerSocialProfile;

#[cfg(target_arch = "wasm32")]
mod boneyards;
mod cloud;
mod observer;
mod presentation;
mod routing;
mod social;
mod transfer;
use cloud::{CloudCompletion, CloudRequest, SubscriptionUpdate};
use observer::HostedObserver;
use presentation::PreparedHostedPresentation;
use routing::{CompletedRoute, PreparedRoute, RouteRequest};
use transfer::{TransferCompletion, TransferRequest};

pub(super) struct HostedGame {
    api: WebsiteApi,
    desktop_endpoint: Option<crate::desktop::DesktopEndpoint>,
    account: Option<Account>,
    active_mods: Vec<ActiveWebMod>,
    store: SaveStore,
    saves: HostedSaveCoordinator,
    profile: Option<ParsedSaveDocument>,
    pub(super) generation: u64,
    pending: Vec<HostedRequest>,
    pub(super) connecting: bool,
    retire_then_start: Option<(NewGameSelection, SaveReceipt)>,
    tutorial_declined: bool,
    leave_checkpoint: Option<SaveReceipt>,
    enable_cheats_after_leave: bool,
    pending_transfer: Option<(u64, PreparedRoute)>,
    transfer_save_document: Option<String>,
    import_checkpoint: Option<(u64, SaveReceipt)>,
    deployment: Option<(u64, String, SaveReceipt)>,
    pub(super) deployment_update: crate::deployment::DeploymentUpdate,
    join_request: Option<(u64, String, f64)>,
    join_resolution: Option<(u64, PartyJoinResolution)>,
    pub(super) welcome: Option<solomon_protocol::hosted::HostedWelcome>,
    runtime: Option<solomon_sim::modding::LuaObject>,
    routing_sequence: u64,
    new_game_selection: Option<NewGameSelection>,
    admission_override: Option<Admission>,
    allow_mod_mismatch: bool,
    active_join_request: Option<u64>,
    party_requester: (String, String),
    pending_mismatch: Option<(u64, NewGameSelection, bool)>,
    pub(super) content_pending: bool,
    pub(super) pending_content_frame: Option<crate::network::ReceivedNativeSnapshot>,
    mod_request_sequence: u64,
    progress: Rc<RefCell<Vec<HostedDownloadProgress>>>,
    subscription_updates: Rc<RefCell<Vec<SubscriptionUpdate>>>,
    pub(super) observer: Option<HostedObserver>,
    #[cfg(target_arch = "wasm32")]
    boneyard_library: Option<boneyards::BoneyardLibrary>,
}

impl HostedGame {
    #[cfg(target_arch = "wasm32")]
    pub(super) fn paint_boneyard_library(
        &mut self,
        menu: &MenuUi,
        frame: &mut crate::gpu_ui::UiFrame<'_>,
    ) -> Result<(), String> {
        if !menu.dark_cloud_boneyards_selected() {
            return Ok(());
        }
        let Some(bounds) = frame.widget_rect("cloud-boneyards-content") else { return Ok(()) };
        if let Some(library) = &mut self.boneyard_library {
            library.paint(frame, bounds, menu.boneyards_interactive())?;
        }
        Ok(())
    }

    pub(super) fn end_connection(&mut self) {
        self.connecting = false;
        self.leave_checkpoint = None;
        // The page's deployment save/poll owner survives the announced socket close.
        self.pending_transfer = None;
        self.transfer_save_document = None;
        self.enable_cheats_after_leave = false;
        self.generation += 1;
    }
}

enum Admission {
    GlobalHub,
    PrivateCollege,
    Party(String),
    Resume { token: Option<String>, document: String, shared_hub: bool },
}

struct ConnectRequest {
    desktop_endpoint: Option<crate::desktop::DesktopEndpoint>,
    admission: Admission,
    hello: HostedHello,
    tutorial: bool,
    assets: Vec<solomon_sim::modding::assets::GameModAsset>,
    disable_mods: bool,
}

struct ConnectionEndpoint {
    url: String,
    credential: String,
}

enum HostedRequest {
    Menu { action: HostedMenuAction, requester: Option<(String, String)> },
    PollJoin { request_id: u64, token: String },
    Connect(Box<ConnectRequest>),
    Leaderboard(String),
    Route(RouteRequest),
    Presentation(Vec<solomon_sim::modding::assets::GameModAsset>),
    Cloud(CloudRequest),
    Transfer(TransferRequest),
    CollegeInvitation(solomon_sim::social::GameCollegeInvitation),
}

pub(crate) struct PendingHostedWork {
    api: WebsiteApi,
    generation: u64,
    operation: HostedOperation,
    progress: Rc<RefCell<Vec<HostedDownloadProgress>>>,
    subscription_updates: Rc<RefCell<Vec<SubscriptionUpdate>>>,
}

struct HostedDownloadProgress {
    generation: u64,
    request_id: Option<u64>,
    completed_bytes: u64,
    total_bytes: u64,
    active_mod_id: Option<String>,
    cloud: bool,
}

enum HostedOperation {
    Request(HostedRequest),
    Save { store: SaveStore, batch: SaveBatch },
}

enum MenuResult {
    Hall(Vec<solomon_sim::hall_of_fame::HallOfFameEntry>),
    Parties(Vec<PublicGameParty>),
    Join(PartyJoinResolution),
    Requested(PartyJoinRequestReceipt),
    Cancelled,
}

enum Completion {
    Menu { action: HostedMenuAction, result: Result<MenuResult, String> },
    PollJoin { request_id: u64, token: String, result: Result<PartyJoinRequestStatus, String> },
    Connect { request: Box<ConnectRequest>, result: Result<ConnectionEndpoint, String> },
    Save { batch: SaveBatch, result: Result<u64, String> },
    Leaderboard(Result<(), String>),
    Route { request_id: u64, result: CompletedRoute },
    Presentation(Result<PreparedHostedPresentation, String>),
    Cloud { action: HostedMenuAction, result: CloudCompletion },
    Transfer(TransferCompletion),
    CollegeInvitation { invitation_id: String, result: Result<PartyJoinResolution, String> },
}

pub(crate) struct HostedCompletion {
    generation: u64,
    completion: Completion,
}

impl PendingHostedWork {
    pub async fn execute(self) -> HostedCompletion {
        let api = self.api;
        let request_id = match &self.operation {
            HostedOperation::Request(HostedRequest::Route(request)) => Some(request.request_id),
            HostedOperation::Request(HostedRequest::Cloud(CloudRequest {
                action: HostedMenuAction::ChangeModSubscription { request_id, .. },
                ..
            })) => Some(*request_id),
            _ => None,
        };
        let cloud = matches!(&self.operation, HostedOperation::Request(HostedRequest::Cloud(_)));
        let mut progress = |progress: crate::mod_content::GameContentDownloadProgress<'_>| {
            self.progress.borrow_mut().push(HostedDownloadProgress {
                generation: self.generation,
                request_id,
                completed_bytes: progress.completed_bytes,
                total_bytes: progress.total_bytes,
                active_mod_id: progress.active.map(|asset| asset.mod_id.clone()),
                cloud,
            });
        };
        let completion = match self.operation {
            HostedOperation::Save { store, batch } => {
                let result = if batch.unchanged {
                    Ok(batch.revision)
                } else {
                    store.write(batch.document.clone(), batch.revision).await
                };
                Completion::Save { batch, result }
            }
            HostedOperation::Request(HostedRequest::Connect(request)) => {
                let result = async {
                    if let Some(endpoint) = &request.desktop_endpoint {
                        return Ok(ConnectionEndpoint {
                            url: endpoint.url.clone(),
                            credential: endpoint.credential.clone(),
                        });
                    }
                    if request.disable_mods {
                        api.disable_all_mods().await.map_err(|error| error.to_string())?;
                    }
                    if !request.assets.is_empty() {
                        let cache = crate::mod_content::ModContentCache::open(api.clone())
                            .await
                            .map_err(|error| error.to_string())?;
                        cache
                            .prefetch(&request.assets, &mut progress)
                            .await
                            .map_err(|error| error.to_string())?;
                    }
                    let admission = match &request.admission {
                        Admission::GlobalHub => BrowserAdmission::GlobalHub,
                        Admission::PrivateCollege => BrowserAdmission::PrivateCollege,
                        Admission::Party(intent_id) => BrowserAdmission::Party { intent_id },
                        Admission::Resume { token, document, shared_hub } => {
                            BrowserAdmission::Resume {
                                party_rejoin_token: token.as_deref(),
                                document,
                                shared_hub: *shared_hub,
                            }
                        }
                    };
                    api.admit(admission)
                        .await
                        .map(|endpoint| ConnectionEndpoint {
                            url: endpoint.url,
                            credential: endpoint.credential,
                        })
                        .map_err(|error| error.to_string())
                }
                .await;
                Completion::Connect { request, result }
            }
            HostedOperation::Request(HostedRequest::Menu { action, requester }) => {
                let result = match &action {
                    HostedMenuAction::LoadGlobalHall { board, .. } => {
                        api.leaderboard(*board).await.map(|entries| {
                            MenuResult::Hall(entries.into_iter().map(|row| row.entry).collect())
                        })
                    }
                    HostedMenuAction::LoadParties { .. } => {
                        api.parties().await.map(MenuResult::Parties)
                    }
                    HostedMenuAction::ResolvePartyCode { code, .. } => {
                        api.resolve_party_code(code).await.map(MenuResult::Join)
                    }
                    HostedMenuAction::JoinPublicParty { listing_id, .. } => {
                        api.resolve_public_party(listing_id).await.map(MenuResult::Join)
                    }
                    HostedMenuAction::RequestPartyInvite { listing_id, .. } => {
                        let (name, id) = requester.expect("the menu owns its requester identity");
                        api.request_party_join(listing_id, &name, &id)
                            .await
                            .map(MenuResult::Requested)
                    }
                    HostedMenuAction::CancelPartyJoin { .. }
                    | HostedMenuAction::CancelModdedPlay { .. }
                    | HostedMenuAction::CancelSaveModMismatch { .. } => Ok(MenuResult::Cancelled),
                    HostedMenuAction::ContinuePartyJoin { .. }
                    | HostedMenuAction::ContinueModdedPlay { .. }
                    | HostedMenuAction::PlayVanilla { .. }
                    | HostedMenuAction::ContinueSaveModMismatch { .. } => {
                        unreachable!("routing is prepared by its owner")
                    }
                    _ => unreachable!("Dark Cloud requests have their own I/O owner"),
                }
                .map_err(|error| error.to_string());
                Completion::Menu { action, result }
            }
            HostedOperation::Request(HostedRequest::PollJoin { request_id, token }) => {
                let result =
                    api.party_request_status(&token).await.map_err(|error| error.to_string());
                Completion::PollJoin { request_id, token, result }
            }
            HostedOperation::Request(HostedRequest::Route(request)) => Completion::Route {
                request_id: request.request_id,
                result: request.execute(&api, &mut progress).await,
            },
            HostedOperation::Request(HostedRequest::Presentation(assets)) => {
                Completion::Presentation(
                    PreparedHostedPresentation::load(&api, &assets, &mut progress).await,
                )
            }
            HostedOperation::Request(HostedRequest::Cloud(request)) => {
                let (action, result) = request
                    .execute(&api, &mut progress, |update| {
                        self.subscription_updates.borrow_mut().push(update);
                    })
                    .await;
                Completion::Cloud { action, result }
            }
            HostedOperation::Request(HostedRequest::Leaderboard(receipt)) => {
                Completion::Leaderboard(
                    api.submit_score(&receipt).await.map(|_| ()).map_err(|error| error.to_string()),
                )
            }
            HostedOperation::Request(HostedRequest::Transfer(request)) => {
                Completion::Transfer(request.execute().await)
            }
            HostedOperation::Request(HostedRequest::CollegeInvitation(invitation)) => {
                Completion::CollegeInvitation {
                    invitation_id: invitation.id,
                    result: api
                        .resolve_party_code(&invitation.join_code)
                        .await
                        .map_err(|error| error.to_string()),
                }
            }
        };
        HostedCompletion { generation: self.generation, completion }
    }
}

impl Game {
    #[cfg(target_arch = "wasm32")]
    fn update_boneyard_library(&mut self) -> Result<(), String> {
        let Some(hosted) = &mut self.hosted else { return Ok(()) };
        if hosted.desktop_endpoint.is_some() || !self.menu.dark_cloud_boneyards_selected() {
            hosted.boneyard_library = None;
            return Ok(());
        }
        let account = hosted.account.as_ref().map(|account| account.username.as_str());
        if let Some(library) = &mut hosted.boneyard_library {
            library.set_account(account);
            library.update()?;
        } else {
            hosted.boneyard_library = Some(boneyards::BoneyardLibrary::new(
                web_sys::window().ok_or("Boneyard library window")?,
                hosted.api.clone(),
                account,
            )?);
        }
        Ok(())
    }
    pub(super) async fn load_hosted(
        &mut self,
        mount: WebsiteMount,
        desktop_endpoint: Option<crate::desktop::DesktopEndpoint>,
    ) -> Result<(), String> {
        let desktop = desktop_endpoint.is_some();
        let api = WebsiteApi::new(crate::platform::location()?.as_str(), mount)?;
        let account = if desktop {
            None
        } else {
            match api.account().await {
                Ok(account) => account,
                // AuthProvider.refresh retires an unavailable account token before opening
                // the guest menu; it never silently sends that token with guest admission.
                Err(_) => {
                    api.logout().map_err(|error| error.to_string())?;
                    None
                }
            }
        };
        self.menu.set_desktop(desktop);
        self.menu
            .set_settings_account_username(account.as_ref().map(|user| user.username.as_str()))?;
        self.menu.set_global_hall_available(!desktop);
        self.menu.set_party_directory_available(true)?;
        self.menu.set_dark_cloud_available(true)?;
        self.menu.set_hosted_account(account.clone())?;
        if account.as_ref().is_some_and(|account| account.developer_access)
            && self.menu.settings().enable_cheats
        {
            let mut settings = self.menu.settings().clone();
            settings.enable_cheats = false;
            self.commit_settings(settings)?;
        }
        self.menu.set_mobile_layout_customized(self.mobile_layout_store.read()?.customized)?;
        let active_mods = if account.is_some() {
            match api.active_mods().await {
                Ok(content) => {
                    if !content.disabled_mods.is_empty() {
                        set_status(
                            &content
                                .disabled_mods
                                .iter()
                                .map(|item| format!("{}: {}", item.name, item.error))
                                .collect::<Vec<_>>()
                                .join("\n"),
                        );
                    }
                    content.mods
                }
                Err(error) => {
                    Self::show_storage_error(&format!("Mods could not be loaded: {error}"));
                    Vec::new()
                }
            }
        } else {
            Vec::new()
        };
        let store = if account.is_some() {
            SaveStore::cloud(api.clone())
        } else {
            SaveStore::open_source().await?
        };
        let (record, missing) = match store.read().await {
            Ok(record) => {
                let missing = record.is_none();
                (record, missing)
            }
            Err(error) => {
                Self::show_storage_error(&error);
                (None, false)
            }
        };
        let profile =
            record.as_ref().and_then(|record| match website_save::parse(&record.document) {
                Ok(profile) => Some(profile),
                Err(error) => {
                    Self::show_storage_error(&error);
                    None
                }
            });
        let selection = profile
            .as_ref()
            .and_then(|profile| profile.continuation.as_ref())
            .map(|save| save.summary.character.clone());
        let can_resume = selection.is_some();
        self.menu.set_save_transfer_available(profile.is_some())?;
        let mut native = NativeGame::idle();
        native.set_saved_selection(selection);
        self.native = Some(native);
        let party_requester = (
            self.menu
                .initial_wizard_name(account.as_ref().map(|account| account.username.as_str())),
            format!("guest-{}", crate::platform::random_uuid()?.replace('-', "")),
        );
        let deployment_update =
            crate::deployment::DeploymentUpdate::new(api.mount().clone(), !desktop)?;
        self.hosted = Some(HostedGame {
            api,
            desktop_endpoint,
            account,
            active_mods,
            store,
            saves: HostedSaveCoordinator::new(record),
            profile,
            generation: 0,
            pending: Vec::new(),
            connecting: false,
            retire_then_start: None,
            tutorial_declined: storage::tutorial_declined().unwrap_or(true),
            leave_checkpoint: None,
            enable_cheats_after_leave: false,
            pending_transfer: None,
            transfer_save_document: None,
            import_checkpoint: None,
            deployment: None,
            deployment_update,
            join_request: None,
            join_resolution: None,
            welcome: None,
            runtime: None,
            routing_sequence: 1 << 32,
            new_game_selection: None,
            admission_override: None,
            allow_mod_mismatch: false,
            active_join_request: None,
            party_requester,
            pending_mismatch: None,
            content_pending: false,
            pending_content_frame: None,
            mod_request_sequence: 0,
            progress: Rc::new(RefCell::new(Vec::new())),
            subscription_updates: Rc::new(RefCell::new(Vec::new())),
            observer: None,
            #[cfg(target_arch = "wasm32")]
            boneyard_library: None,
        });
        self.menu.show_title(can_resume)?;
        if !desktop && crate::platform::take_query_parameter("dark-cloud", "boneyards")? {
            self.menu
                .show_dark_cloud_tab(solomon_sim::menu::dark_cloud::DarkCloudTab::Boneyards)?;
        } else if missing && !self.hosted.as_ref().unwrap().tutorial_declined {
            self.menu.offer_tutorial()?;
        }
        Ok(())
    }

    pub(super) fn hosted_menu_action(&mut self, action: &MenuAction) -> Result<bool, String> {
        let Some(hosted) = &mut self.hosted else { return Ok(false) };
        match action {
            MenuAction::CancelCreate => {
                hosted.admission_override = None;
                hosted.transfer_save_document = None;
                hosted.allow_mod_mismatch = false;
            }
            MenuAction::Start(selection) => {
                if hosted.connecting {
                    return Ok(true);
                }
                if hosted.profile.as_ref().is_some_and(|profile| profile.continuation.is_some())
                    && !self.menu.is_create_pending()
                {
                    let document = &hosted
                        .saves
                        .record
                        .as_ref()
                        .ok_or("The saved wizard is unavailable")?
                        .document;
                    let receipt = hosted.saves.replace(website_save::retire(document)?);
                    hosted.retire_then_start = Some((selection.clone(), receipt));
                    set_status("Saving game…");
                } else {
                    self.request_hosted_new_game(selection.clone())?;
                }
            }
            MenuAction::Continue => {
                let save = hosted
                    .profile
                    .as_ref()
                    .and_then(|profile| profile.continuation.as_ref())
                    .ok_or("Your saved wizard is not available")?;
                let selection = save.summary.character.clone();
                if !self.confirm_hosted_saved_mods(&selection, true)? {
                    self.hosted.as_mut().unwrap().allow_mod_mismatch = false;
                    self.queue_hosted_connection(selection, true, false, false)?;
                }
            }
            MenuAction::StartTutorial => {
                let mut settings = self.menu.settings().clone();
                settings.enable_cheats = false;
                self.menu.set_settings(settings.clone())?;
                storage::save_settings(&settings)?;
                self.queue_hosted_connection(
                    NewGameSelection {
                        name: "Sirmin".into(),
                        element: WizardElement::Ether,
                        discipline: WizardDiscipline::Arcane,
                    },
                    false,
                    true,
                    false,
                )?;
            }
            MenuAction::TutorialDeclined => {
                hosted.tutorial_declined = true;
                return Ok(false);
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    /// Source requestGameSettingsUpdate retains accepted settings while Global
    /// College consent is visible. Developer access never needs the cheat preference.
    pub(super) fn request_hosted_settings(
        &mut self,
        requested: &mut solomon_sim::settings::GameSettings,
    ) -> Result<bool, String> {
        let Some(hosted) = &self.hosted else { return Ok(true) };
        if hosted.account.as_ref().is_some_and(|account| account.developer_access) {
            requested.enable_cheats = false;
        } else if self.has_game
            && hosted.welcome.as_ref().is_some_and(|welcome| {
                welcome.session_kind == solomon_sim::social::GameSessionKind::GlobalHub
            })
            && !self.menu.settings().enable_cheats
            && requested.enable_cheats
        {
            self.menu.set_settings(self.menu.settings().clone())?;
            self.menu.show_cheat_college_prompt()?;
            return Ok(false);
        }
        Ok(true)
    }

    pub(super) fn send_hosted_settings(
        &self,
        settings: &solomon_sim::settings::GameSettings,
    ) -> Result<(), String> {
        #[cfg(target_arch = "wasm32")]
        if self.is_editor_test() {
            let mut preferences = game_online_preferences(settings);
            preferences.submit_runs = false;
            return self.send_native_message(&ClientMessage::Hosted {
                command: HostedCommand::OnlinePreferences { preferences },
            });
        }
        let Some(hosted) = &self.hosted else { return Ok(()) };
        if !self.has_game {
            return Ok(());
        }
        self.send_native_message(&ClientMessage::Hosted {
            command: HostedCommand::OnlinePreferences {
                preferences: game_online_preferences(settings),
            },
        })?;
        if settings.enable_cheats != self.menu.settings().enable_cheats
            && let Some(welcome) = &hosted.welcome
        {
            let is_host = self
                .native
                .as_ref()
                .and_then(NativeGame::host_snapshot)
                .and_then(|host| host.authority_player_id.as_deref())
                == Some(self.local_player_id.as_str());
            if welcome.session_kind != solomon_sim::social::GameSessionKind::PrivateCollege
                || is_host
            {
                self.send_native_message(&ClientMessage::Hosted {
                    command: HostedCommand::SetCheats {
                        enabled: settings.enable_cheats && !welcome.developer_access,
                    },
                })?;
            }
        }
        Ok(())
    }

    fn start_hosted_new_game(&mut self, selection: NewGameSelection) -> Result<(), String> {
        let hosted = self.hosted.as_mut().unwrap();
        let intro = !hosted.tutorial_declined
            && match &hosted.saves.record {
                Some(record) if hosted.profile.is_some() => {
                    website_save::restore_profile(&record.document)?.economy.college_intro_pending
                }
                _ => true,
            };
        if !intro && !self.menu.is_create_pending() {
            return self.menu.show_create();
        }
        self.queue_hosted_connection(selection, false, false, intro)
    }

    fn queue_hosted_connection(
        &mut self,
        selection: NewGameSelection,
        resume: bool,
        tutorial: bool,
        intro: bool,
    ) -> Result<(), String> {
        let hosted = self.hosted.as_mut().unwrap();
        if hosted.connecting {
            return Ok(());
        }
        let settings = self.menu.settings();
        let cheats = !tutorial
            && settings.enable_cheats
            && !hosted.account.as_ref().is_some_and(|account| account.developer_access);
        let shared = hosted.desktop_endpoint.is_none()
            && game_shared_hub_enabled(settings)
            && (tutorial
                || (!cheats
                    && hosted.active_mods.is_empty()
                    && hosted.profile.as_ref().is_none_or(|profile| {
                        profile.metadata.integrity == SaveIntegrity::GlobalClean
                    })));
        let save = if tutorial {
            None
        } else {
            hosted.transfer_save_document.take().or_else(|| {
                hosted
                    .profile
                    .as_ref()
                    .map(|_| hosted.saves.record.as_ref().unwrap().document.clone())
            })
        };
        let summary = hosted
            .profile
            .as_ref()
            .and_then(|profile| profile.continuation.as_ref())
            .map(|save| &save.summary);
        let admission = if !resume && !tutorial && hosted.admission_override.is_some() {
            hosted.admission_override.take().unwrap()
        } else if resume {
            let token = summary.and_then(|summary| summary.party_rejoin_token.clone());
            Admission::Resume {
                token: if hosted.allow_mod_mismatch
                    || hosted.profile.as_ref().is_some_and(|profile| {
                        profile.metadata.integrity == SaveIntegrity::GlobalClean
                    }) && !game_shared_hub_enabled(settings)
                {
                    None
                } else {
                    token
                },
                document: save.clone().ok_or("Your saved wizard is unavailable")?,
                shared_hub: shared,
            }
        } else if shared {
            Admission::GlobalHub
        } else {
            Admission::PrivateCollege
        };
        let resume_token = if resume {
            crate::network::read_resume()?
                .filter(|credentials| {
                    summary.is_some_and(|summary| credentials.player_id == summary.player_id)
                })
                .map(|credentials| credentials.token)
        } else {
            None
        };
        let assets = if tutorial {
            Vec::new()
        } else {
            hosted.active_mods.iter().flat_map(|content| content.assets.iter()).cloned().collect()
        };
        let hello = HostedHello {
            version: solomon_protocol::PROTOCOL_VERSION,
            enhanced_effects: settings.enhanced_effects,
            credential: String::new(),
            selection,
            profile: PlayerSocialProfile {
                account_username: hosted.account.as_ref().map(|account| account.username.clone()),
                highest_wave: self.hall_entries.iter().map(|entry| entry.wave).max(),
                total_playtime_ms: Some(storage::read_total_playtime_ms()?),
            },
            online_preferences: game_online_preferences(settings),
            resume_token,
            save_intent: save
                .as_ref()
                .map(|_| if resume { GameSaveIntent::Resume } else { GameSaveIntent::NewGame }),
            save,
            begin_college_intro: intro,
            decline_tutorial: !tutorial && hosted.tutorial_declined && hosted.profile.is_none(),
            cheats_enabled: cheats,
            allow_mod_mismatch: hosted.allow_mod_mismatch,
        };
        hosted.generation += 1;
        hosted.connecting = true;
        hosted.leave_checkpoint = None;
        hosted.deployment = None;
        hosted.pending.push(HostedRequest::Connect(Box::new(ConnectRequest {
            desktop_endpoint: hosted.desktop_endpoint.clone(),
            admission,
            hello,
            tutorial,
            assets,
            disable_mods: tutorial && hosted.account.is_some() && !hosted.active_mods.is_empty(),
        })));
        set_status("Connecting to game…");
        self.begin_match_loading(
            if tutorial { MatchLoadingFlow::Boneyard } else { MatchLoadingFlow::Hub },
            MatchLoadingStage::AuthenticatingSession,
        );
        Ok(())
    }

    pub(super) fn poll_hosted(&mut self, now: f64) -> Result<(), String> {
        self.apply_hosted_subscription_updates()?;
        #[cfg(target_arch = "wasm32")]
        self.update_boneyard_library()?;
        let Some(hosted) = &mut self.hosted else { return Ok(()) };
        for progress in std::mem::take(&mut *hosted.progress.borrow_mut()) {
            if progress.generation != hosted.generation && !progress.cloud {
                continue;
            }
            if let Some(request_id) = progress.request_id {
                let projection = Some(crate::menu::RoutingDownloadProgress {
                    completed_bytes: progress.completed_bytes,
                    total_bytes: progress.total_bytes,
                    active_mod_id: progress.active_mod_id.as_deref(),
                });
                if progress.cloud {
                    self.menu.set_dark_cloud_progress(request_id, projection)?;
                } else {
                    self.menu.set_routing_progress(request_id, projection)?;
                }
            } else if hosted.connecting || hosted.content_pending {
                set_status(&format!(
                    "Loading content… {} / {} bytes",
                    progress.completed_bytes, progress.total_bytes
                ));
            }
        }
        let native = self.native.as_mut().unwrap();
        if let Some(welcome) = native.hosted_welcome.take() {
            hosted.pending.push(HostedRequest::Presentation(welcome.mod_assets.clone()));
            hosted.welcome = Some(welcome);
        }
        let messages = std::mem::take(&mut native.hosted_messages);
        for (message, _) in messages {
            if self.is_hosted_observer() {
                continue;
            }
            if self.social.receive_hosted_message(&message)? {
                continue;
            }
            if self.cheats.receive_hosted_message(&message)? {
                continue;
            }
            let runtime_updated = matches!(message, HostedServerMessage::ModRuntime { .. });
            let mut presentation_updated = runtime_updated;
            let hosted = self.hosted.as_mut().unwrap();
            match message {
                HostedServerMessage::SaveCheckpoint { save, sequence, .. } => {
                    hosted.saves.accept(hosted.generation, sequence, save)?;
                }
                HostedServerMessage::SaveCheckpointChunk { .. } => {
                    return Err("The game received an unassembled save checkpoint".into());
                }
                HostedServerMessage::SaveBeforeLeave { checkpoint_sequence, request_id } => {
                    if !self.native.as_ref().unwrap().expects_leave_receipt(request_id) {
                        continue;
                    }
                    if checkpoint_sequence == 0 {
                        self.fail_hosted_leave(
                            "The game server could not create a final save.".into(),
                        )?;
                        continue;
                    }
                    let Some(receipt) = hosted.saves.receipt(checkpoint_sequence) else {
                        self.native_disconnected(
                            "The leave response did not include its final save checkpoint.",
                        )?;
                        continue;
                    };
                    hosted.leave_checkpoint = Some(receipt);
                    if hosted.pending_transfer.is_some() {
                        hosted.transfer_save_document = Some(
                            hosted
                                .saves
                                .document(checkpoint_sequence)
                                .expect("the accepted checkpoint owns its document")
                                .to_owned(),
                        );
                    }
                }
                HostedServerMessage::DeploymentRestart {
                    checkpoint_sequence,
                    target_revision,
                    deployment_id,
                    deployment_sequence,
                } => {
                    if hosted.deployment_update.restarting() {
                        continue;
                    }
                    let receipt = if checkpoint_sequence == 0 {
                        hosted.saves.idle()
                    } else if let Some(receipt) = hosted.saves.receipt(checkpoint_sequence) {
                        receipt
                    } else {
                        self.native_disconnected(
                            "The deployment restart did not include its final save checkpoint.",
                        )?;
                        continue;
                    };
                    hosted.deployment_update.begin(
                        &target_revision,
                        &deployment_id,
                        deployment_sequence,
                    )?;
                    hosted.leave_checkpoint = None;
                    hosted.pending_transfer = None;
                    hosted.transfer_save_document = None;
                    hosted.enable_cheats_after_leave = false;
                    self.native.as_mut().unwrap().begin_deployment();
                    hosted.deployment = Some((checkpoint_sequence, target_revision, receipt));
                }
                HostedServerMessage::LeaderboardReceipt { receipt } => {
                    hosted.pending.push(HostedRequest::Leaderboard(receipt));
                }
                HostedServerMessage::ModRuntime { projection, .. } => {
                    hosted.runtime = Some(projection);
                }
                HostedServerMessage::ModContent { projection } => {
                    match self.mod_content.receive(&projection) {
                        Ok(changed) => presentation_updated = changed,
                        Err(error) => self.native_disconnected(&error)?,
                    }
                }
                HostedServerMessage::ModCatalog { items } => {
                    if let Some(welcome) = &mut hosted.welcome {
                        welcome.mod_catalog = items;
                    }
                }
                HostedServerMessage::CheatMode { enabled } => {
                    if let Some(welcome) = &mut hosted.welcome {
                        welcome.cheats_enabled = enabled;
                    }
                }
                // Social and cheat surfaces consume their own matching responses
                // above. An unmounted surface ignores a late reply, as Source does.
                HostedServerMessage::PlayerCard { .. }
                | HostedServerMessage::CollegeInvitations { .. }
                | HostedServerMessage::PartyAction { .. }
                | HostedServerMessage::LuaResult { .. } => {}
            }
            if presentation_updated {
                self.update_hosted_presentation(runtime_updated)?;
            }
        }
        self.poll_hosted_social()?;
        let mut actions = self.mod_overlays.scene.take_actions();
        actions.extend(self.mod_minimap.take_actions().into_iter().map(|action| {
            let center: LuaObject = [
                ("x".into(), LuaValue::Number(action.center.x)),
                ("y".into(), LuaValue::Number(action.center.y)),
            ]
            .into_iter()
            .collect();
            crate::mod_world_overlays::ModViewAction {
                action: solomon_protocol::hosted::ModAction::UiAction,
                target: action.content_id,
                arguments: [
                    ("action".into(), LuaValue::String(action.action)),
                    ("arguments".into(), LuaValue::Object(center)),
                ]
                .into_iter()
                .collect(),
            }
        }));
        actions.extend(self.mod_skills.take_actions().into_iter().map(|action| {
            use crate::mod_skills::ModSkillAction;
            let (action, target, arguments) = match action {
                ModSkillAction::QuickbarBind { content_id, slot, clear } => {
                    let mut arguments: LuaObject =
                        [("slot".into(), LuaValue::Number(slot))].into_iter().collect();
                    if clear {
                        arguments.insert("clear".into(), LuaValue::Boolean(true));
                    }
                    (ModAction::QuickbarBind, content_id, arguments)
                }
                ModSkillAction::SkillChoose {
                    content_id,
                    mod_offer_sequence,
                    native_offer_sequence,
                } => {
                    let arguments = [
                        ("mod_offer_sequence".into(), LuaValue::Number(mod_offer_sequence)),
                        (
                            "native_offer_sequence".into(),
                            LuaValue::Number(native_offer_sequence as f64),
                        ),
                    ]
                    .into_iter()
                    .collect();
                    (ModAction::SkillChoose, content_id, arguments)
                }
            };
            crate::mod_world_overlays::ModViewAction { action, target, arguments }
        }));
        for action in actions {
            if self.is_hosted_observer() {
                continue;
            }
            let hosted = self.hosted.as_mut().unwrap();
            hosted.mod_request_sequence += 1;
            let request_id = hosted.mod_request_sequence;
            self.send_native_message(&ClientMessage::Hosted {
                command: HostedCommand::ModAction {
                    action: action.action,
                    arguments: action.arguments,
                    request_id,
                    target: action.target,
                },
            })?;
        }
        for action in self.mod_panels.take_actions() {
            if self.is_hosted_observer() {
                continue;
            }
            let hosted = self.hosted.as_mut().unwrap();
            hosted.mod_request_sequence += 1;
            let request_id = hosted.mod_request_sequence;
            use crate::mod_panels::ModPanelAction;
            let command = match action {
                ModPanelAction::Cast { content_id, position } => HostedCommand::ModCast {
                    content_id,
                    request_id,
                    target_x: position.x,
                    target_y: position.y,
                },
                ModPanelAction::ShopBuy { content_id, row } => HostedCommand::ModAction {
                    action: ModAction::ShopBuy,
                    arguments: [("row".into(), LuaValue::Number(row as f64))].into_iter().collect(),
                    request_id,
                    target: content_id,
                },
                ModPanelAction::Reforge { content_id, item_id, service } => {
                    HostedCommand::ModAction {
                        action: ModAction::Reforge,
                        arguments: [
                            ("item_id".into(), LuaValue::Number(item_id as f64)),
                            ("service".into(), LuaValue::Number(service as f64)),
                        ]
                        .into_iter()
                        .collect(),
                        request_id,
                        target: content_id,
                    }
                }
                ModPanelAction::PortalEnter { portal_id } => HostedCommand::ModAction {
                    action: ModAction::PortalEnter,
                    arguments: LuaObject::new(),
                    request_id,
                    target: portal_id,
                },
            };
            self.send_native_message(&ClientMessage::Hosted { command })?;
        }
        for action in self.mod_skill_quickbar.take_actions() {
            if self.is_hosted_observer() {
                continue;
            }
            let hosted = self.hosted.as_mut().unwrap();
            hosted.mod_request_sequence += 1;
            let request_id = hosted.mod_request_sequence;
            self.send_native_message(&ClientMessage::Hosted {
                command: HostedCommand::ModCast {
                    content_id: action.content_id,
                    request_id,
                    target_x: action.target.x,
                    target_y: action.target.y,
                },
            })?;
        }
        for action in self.menu.take_hosted_actions() {
            if matches!(action, HostedMenuAction::ContinueCheatsInPrivateCollege) {
                self.apply_menu_action(MenuAction::Leave)?;
                // The Global College must finish its checkpoint before the new
                // private preference can reach any live game connection.
                self.hosted.as_mut().unwrap().enable_cheats_after_leave = true;
                continue;
            }
            if !self.hosted_routing_action(&action)?
                && !self.hosted_cloud_action(&action)?
                && !self.hosted_transfer_action(&action)?
            {
                let hosted = self.hosted.as_mut().unwrap();
                if let HostedMenuAction::ResolvePartyCode { request_id, .. }
                | HostedMenuAction::JoinPublicParty { request_id, .. }
                | HostedMenuAction::RequestPartyInvite { request_id, .. } = &action
                {
                    hosted.active_join_request = Some(*request_id);
                }
                let requester = matches!(action, HostedMenuAction::RequestPartyInvite { .. })
                    .then(|| hosted.party_requester.clone());
                hosted.pending.push(HostedRequest::Menu { action, requester });
            }
        }
        let hosted = self.hosted.as_mut().unwrap();
        if hosted.join_request.as_ref().is_some_and(|(_, _, due)| now >= *due) {
            let (request_id, token, _) = hosted.join_request.take().unwrap();
            hosted.pending.push(HostedRequest::PollJoin { request_id, token });
        }
        self.finish_hosted_persistence()
    }

    fn finish_hosted_persistence(&mut self) -> Result<(), String> {
        let imported = self
            .hosted
            .as_ref()
            .unwrap()
            .import_checkpoint
            .as_ref()
            .map(|(id, receipt)| (*id, receipt.outcome()));
        if let Some((request_id, outcome)) = imported {
            let result = match outcome {
                SaveOutcome::Committed => Some(Ok(())),
                SaveOutcome::Failed(error) => Some(Err(error)),
                SaveOutcome::Pending => None,
            };
            if let Some(result) = result {
                self.hosted.as_mut().unwrap().import_checkpoint = None;
                self.menu.complete_save_replacement(request_id, result)?;
            }
        }
        let hosted = self.hosted.as_mut().unwrap();
        if let Some(receipt) = &hosted.leave_checkpoint {
            match receipt.outcome() {
                SaveOutcome::Committed => {
                    hosted.leave_checkpoint = None;
                    let transfer = hosted.pending_transfer.take();
                    let document = hosted.transfer_save_document.take();
                    let enable_cheats = hosted.enable_cheats_after_leave;
                    if let Some((request_id, _)) = &transfer {
                        self.menu.complete_routing(*request_id, Ok(()))?;
                    }
                    self.send_native_message(&ClientMessage::Disconnect)?;
                    self.finish_desktop_close(Ok(()));
                    self.native_disconnected("")?;
                    if enable_cheats {
                        let mut settings = self.menu.settings().clone();
                        settings.enable_cheats = true;
                        self.commit_settings(settings)?;
                    }
                    if let Some((_, prepared)) = transfer {
                        self.hosted.as_mut().unwrap().transfer_save_document = document;
                        self.begin_prepared_hosted_route(prepared)?;
                    }
                }
                SaveOutcome::Failed(error) => {
                    self.fail_hosted_leave(error)?;
                }
                _ => {}
            }
        }
        let hosted = self.hosted.as_mut().unwrap();
        if let Some((sequence, revision, receipt)) = &hosted.deployment {
            match receipt.outcome() {
                SaveOutcome::Committed => {
                    let command = HostedCommand::DeploymentReady {
                        checkpoint_sequence: *sequence,
                        target_revision: revision.clone(),
                    };
                    hosted.deployment = None;
                    hosted.deployment_update.saved();
                    self.native.as_ref().unwrap().acknowledge_deployment(command)?;
                }
                SaveOutcome::Failed(error) => {
                    hosted.deployment = None;
                    Self::show_storage_error(&error);
                }
                _ => {}
            }
        }
        Ok(())
    }

    pub(super) fn fail_hosted_leave(&mut self, error: String) -> Result<(), String> {
        self.finish_desktop_close(Err(error.clone()));
        let hosted = self.hosted.as_mut().unwrap();
        hosted.leave_checkpoint = None;
        hosted.enable_cheats_after_leave = false;
        hosted.transfer_save_document = None;
        if let Some((request_id, _)) = hosted.pending_transfer.take() {
            self.native.as_mut().unwrap().cancel_leave();
            set_status("");
            self.menu.complete_routing(request_id, Err(error))?;
        } else {
            self.cancel_native_leave()?;
            Self::show_storage_error(&error);
        }
        Ok(())
    }

    pub(crate) fn take_hosted_work(&mut self) -> Vec<PendingHostedWork> {
        let Some(hosted) = &mut self.hosted else { return Vec::new() };
        let mut operations: Vec<_> =
            std::mem::take(&mut hosted.pending).into_iter().map(HostedOperation::Request).collect();
        if let Some(batch) = hosted.saves.take_write() {
            operations.push(HostedOperation::Save { store: hosted.store.clone(), batch });
        }
        operations
            .into_iter()
            .map(|operation| PendingHostedWork {
                api: hosted.api.clone(),
                generation: hosted.generation,
                operation,
                progress: hosted.progress.clone(),
                subscription_updates: hosted.subscription_updates.clone(),
            })
            .collect()
    }

    pub(crate) fn complete_hosted_work(
        &mut self,
        completed: HostedCompletion,
    ) -> Result<(), String> {
        // A zero-asset subscription can finish in one microtask, before the next frame.
        // Apply its catalog while the UI still owns the subscription request.
        self.apply_hosted_subscription_updates()?;
        let Some(hosted) = &mut self.hosted else { return Ok(()) };
        // An admitted save writer owns a revision even if the next transport has started.
        if let Completion::Save { batch, result } = completed.completion {
            let failure = result.as_ref().err().cloned();
            hosted.saves.complete(batch, result);
            if let Some(error) = failure {
                Self::show_storage_error(&error);
            }
            if let Some(record) = &hosted.saves.record {
                hosted.profile = match website_save::parse(&record.document) {
                    Ok(profile) => Some(profile),
                    Err(error) => {
                        Self::show_storage_error(&error);
                        None
                    }
                };
            }
            let selection = hosted
                .profile
                .as_ref()
                .and_then(|profile| profile.continuation.as_ref())
                .map(|save| save.summary.character.clone());
            self.menu.set_resume_available(selection.is_some())?;
            self.menu.set_save_transfer_available(hosted.profile.is_some())?;
            self.native.as_mut().unwrap().set_saved_selection(selection);
            match hosted.retire_then_start.as_ref().map(|(_, receipt)| receipt.outcome()) {
                Some(SaveOutcome::Committed) => {
                    let (selection, _) = hosted.retire_then_start.take().unwrap();
                    set_status("");
                    self.request_hosted_new_game(selection)?;
                }
                Some(SaveOutcome::Failed(error)) => {
                    hosted.retire_then_start = None;
                    if self.menu.is_create_pending() {
                        self.menu.complete_create_loadout(false)?;
                    } else {
                        self.menu.show_title(
                            hosted
                                .profile
                                .as_ref()
                                .is_some_and(|profile| profile.continuation.is_some()),
                        )?;
                    }
                    Self::show_storage_error(&error);
                }
                _ => {}
            }
            return self.finish_hosted_persistence();
        }
        let subscription = matches!(
            &completed.completion,
            Completion::Cloud { action: HostedMenuAction::ChangeModSubscription { .. }, .. }
        );
        if hosted.generation != completed.generation && !subscription {
            return Ok(());
        }
        match completed.completion {
            Completion::Connect { mut request, result } => match result {
                Ok(endpoint) => {
                    request.hello.credential = endpoint.credential;
                    if request.disable_mods {
                        hosted.active_mods.clear();
                    }
                    self.begin_hosted_connection(
                        &endpoint.url,
                        ClientMessage::Hosted {
                            command: HostedCommand::Hello { hello: request.hello },
                        },
                        request.tutorial,
                    )?;
                }
                Err(error) => {
                    hosted.connecting = false;
                    self.native_disconnected(&error)?;
                }
            },
            Completion::Menu { action, result } => self.complete_hosted_menu(action, result)?,
            Completion::PollJoin { request_id, token, result } => {
                self.complete_hosted_join_poll(request_id, token, result)?;
            }
            Completion::Route { request_id, result } => {
                self.complete_hosted_route(request_id, result)?
            }
            Completion::Presentation(result) => self.complete_hosted_presentation(result)?,
            Completion::Cloud { action, result } => self.complete_hosted_cloud(action, result)?,
            Completion::Transfer(completion) => self.complete_hosted_transfer(completion)?,
            Completion::CollegeInvitation { invitation_id, result } => {
                self.complete_hosted_college_invitation(&invitation_id, result)?;
            }
            Completion::Leaderboard(result) => {
                if let Err(error) = result {
                    Self::show_storage_error(&error);
                }
            }
            Completion::Save { .. } => {
                unreachable!("save completion handled before generation admission")
            }
        }
        Ok(())
    }

    fn complete_hosted_menu(
        &mut self,
        action: HostedMenuAction,
        result: Result<MenuResult, String>,
    ) -> Result<(), String> {
        match action {
            HostedMenuAction::LoadGlobalHall { request_id, .. } => {
                self.menu.complete_global_hall(
                    request_id,
                    result.and_then(|result| match result {
                        MenuResult::Hall(entries) => Ok(entries),
                        _ => unreachable!(),
                    }),
                );
            }
            HostedMenuAction::LoadParties { request_id } => {
                self.menu.complete_party_directory(
                    request_id,
                    result.and_then(|result| match result {
                        MenuResult::Parties(parties) => Ok(parties),
                        _ => unreachable!(),
                    }),
                )?;
            }
            HostedMenuAction::ResolvePartyCode { request_id, .. }
            | HostedMenuAction::JoinPublicParty { request_id, .. } => match result {
                Ok(MenuResult::Join(resolution)) => {
                    self.receive_party_resolution(request_id, resolution)?
                }
                Err(error) => {
                    self.menu.complete_party_join(request_id, Err(error))?;
                }
                _ => unreachable!(),
            },
            HostedMenuAction::RequestPartyInvite { request_id, .. } => match result {
                Ok(MenuResult::Requested(receipt)) => self.complete_hosted_join_poll(
                    request_id,
                    receipt.request_token,
                    Ok(PartyJoinRequestStatus::Pending),
                )?,
                Err(error) => {
                    self.menu.complete_party_join(request_id, Err(error))?;
                }
                _ => unreachable!(),
            },
            HostedMenuAction::CancelPartyJoin { .. }
            | HostedMenuAction::CancelModdedPlay { .. }
            | HostedMenuAction::CancelSaveModMismatch { .. } => {}
            HostedMenuAction::ContinuePartyJoin { .. }
            | HostedMenuAction::ContinueModdedPlay { .. }
            | HostedMenuAction::PlayVanilla { .. }
            | HostedMenuAction::ContinueSaveModMismatch { .. } => {
                unreachable!("routing is prepared by its owner")
            }
            _ => unreachable!("Dark Cloud requests have their own completion owner"),
        }
        Ok(())
    }

    fn complete_hosted_join_poll(
        &mut self,
        request_id: u64,
        token: String,
        result: Result<PartyJoinRequestStatus, String>,
    ) -> Result<(), String> {
        if self.hosted.as_ref().unwrap().active_join_request != Some(request_id) {
            return Ok(());
        }
        match result {
            Ok(PartyJoinRequestStatus::Pending) => {
                self.hosted.as_mut().unwrap().join_request =
                    Some((request_id, token, self.last_time.unwrap_or(0.0) + 1000.0));
            }
            Ok(PartyJoinRequestStatus::Accepted { intent_id, target }) => {
                self.receive_party_resolution(
                    request_id,
                    PartyJoinResolution { intent_id, target },
                )?;
            }
            Ok(PartyJoinRequestStatus::Denied) => {
                self.menu.complete_party_join(
                    request_id,
                    Err("The party leader denied your request.".into()),
                )?;
            }
            Err(error) => {
                self.menu.complete_party_join(request_id, Err(error))?;
            }
        }
        Ok(())
    }

    fn receive_party_resolution(
        &mut self,
        request_id: u64,
        resolution: PartyJoinResolution,
    ) -> Result<(), String> {
        self.present_party_resolution(request_id, resolution)
    }
}
