//! MainMenuScene's explicit vanilla, private-College and party-content choices.

use super::*;
use solomon_sim::social::GameSessionKind;

pub(super) enum RouteKind {
    Local,
    Vanilla,
    Party(PartyJoinResolution),
}

pub(super) struct RouteRequest {
    pub request_id: u64,
    kind: RouteKind,
    signed_in: bool,
    active_mods: Vec<ActiveWebMod>,
    selection: NewGameSelection,
    vanilla_shared: bool,
}

pub(super) struct CompletedRoute {
    active_mods: Vec<ActiveWebMod>,
    result: Result<PreparedRoute, String>,
}

pub(super) struct PreparedRoute {
    admission: Admission,
    allow_mod_mismatch: bool,
    disable_cheats: bool,
    selection: NewGameSelection,
}

impl RouteRequest {
    pub async fn execute(
        self,
        api: &WebsiteApi,
        progress: impl FnMut(crate::mod_content::GameContentDownloadProgress<'_>),
    ) -> CompletedRoute {
        let vanilla = matches!(self.kind, RouteKind::Vanilla)
            || matches!(&self.kind, RouteKind::Party(resolution) if resolution.target.kind == GameSessionKind::GlobalHub);
        let mut active_mods = self.active_mods;
        let result = async {
        if vanilla && self.signed_in && !active_mods.is_empty() {
            api.disable_all_mods().await.map_err(|error| error.to_string())?;
            active_mods = api.active_mods().await.map_err(|error| error.to_string())?.mods;
        }
        let (admission, allow_mod_mismatch, downloads) = match self.kind {
            RouteKind::Vanilla => (
                if self.vanilla_shared { Admission::GlobalHub } else { Admission::PrivateCollege },
                false,
                Vec::new(),
            ),
            RouteKind::Local => (
                Admission::PrivateCollege,
                false,
                active_mods
                    .iter()
                    .map(|content| {
                        (
                            content.id.clone(),
                            content.name.clone(),
                            content.slug.clone(),
                            content.assets.clone(),
                        )
                    })
                    .collect::<Vec<_>>(),
            ),
            RouteKind::Party(resolution) => {
                let owns_content = resolution.target.kind == GameSessionKind::PrivateCollege;
                if owns_content && self.signed_in && !resolution.target.content.mods.is_empty() {
                    api.sync_mods(&resolution.target.content.mods)
                        .await
                        .map_err(|error| error.to_string())?;
                    active_mods = api.active_mods().await.map_err(|error| error.to_string())?.mods;
                }
                let downloads = if owns_content {
                    resolution
                        .target
                        .content
                        .mods
                        .into_iter()
                        .map(|content| (content.id, content.name, content.slug, content.assets))
                        .collect()
                } else {
                    Vec::new()
                };
                (Admission::Party(resolution.intent_id), owns_content, downloads)
            }
        };
        if !downloads.is_empty() {
            let assets = downloads
                .iter()
                .flat_map(|(_, _, _, assets)| assets)
                .cloned()
                .collect::<Vec<_>>();
            let cache = crate::mod_content::ModContentCache::open(api.clone())
                .await
                .map_err(|error| error.to_string())?;
            if let Err(error) = cache
                .prefetch(&assets, progress)
                .await
            {
                if self.signed_in
                    && let Some((_, name, slug, _)) =
                        downloads.iter().find(|(id, ..)| *id == error.mod_id)
                {
                    api.set_mod_enabled(slug, false).await.map_err(|_| {
                        format!("{error} The mod could not be disabled automatically.")
                    })?;
                    active_mods = api.active_mods().await.map_err(|_| {
                        format!("{error} The mod could not be disabled automatically.")
                    })?.mods;
                    return Err(format!(
                        "{name} was disabled because its content could not be loaded or verified."
                    ));
                }
                return Err(error.to_string());
            }
        }
        Ok(PreparedRoute {
            admission,
            allow_mod_mismatch,
            disable_cheats: vanilla,
            selection: self.selection,
        })
        }.await;
        CompletedRoute { active_mods, result }
    }
}

impl Game {
    pub(super) fn request_hosted_new_game(
        &mut self,
        selection: NewGameSelection,
    ) -> Result<(), String> {
        let hosted = self.hosted.as_mut().unwrap();
        if self.menu.is_create_pending() {
            return self.start_hosted_new_game(selection);
        }
        let cheats = self.menu.settings().enable_cheats
            && !hosted.account.as_ref().is_some_and(|account| account.developer_access);
        if !hosted.active_mods.is_empty() || cheats {
            hosted.routing_sequence += 1;
            hosted.new_game_selection = Some(selection);
            self.menu.show_modded_play(hosted.routing_sequence, hosted.active_mods.clone(), cheats)
        } else if self.confirm_hosted_saved_mods(&selection, false)? {
            Ok(())
        } else {
            self.hosted.as_mut().unwrap().allow_mod_mismatch = false;
            self.start_hosted_new_game(selection)
        }
    }

    pub(super) fn hosted_routing_action(
        &mut self,
        action: &HostedMenuAction,
    ) -> Result<bool, String> {
        let hosted = self.hosted.as_mut().unwrap();
        let (request_id, kind) = match action {
            HostedMenuAction::ContinueSaveModMismatch { request_id } => {
                let Some((id, selection, resume)) = hosted.pending_mismatch.take() else {
                    return Ok(true);
                };
                if id != *request_id {
                    hosted.pending_mismatch = Some((id, selection, resume));
                    return Ok(true);
                }
                if !self.menu.complete_routing(*request_id, Ok(()))? {
                    return Ok(true);
                }
                hosted.allow_mod_mismatch = true;
                if resume {
                    self.queue_hosted_connection(selection, true, false, false)?;
                } else {
                    self.start_hosted_new_game(selection)?;
                }
                return Ok(true);
            }
            HostedMenuAction::CancelSaveModMismatch { request_id } => {
                if hosted.pending_mismatch.as_ref().is_some_and(|(id, ..)| id == request_id) {
                    hosted.pending_mismatch = None;
                }
                return Ok(true);
            }
            HostedMenuAction::ContinuePartyJoin { request_id } => {
                let Some((id, resolution)) = &hosted.join_resolution else { return Ok(true) };
                if id != request_id {
                    return Ok(true);
                }
                (*request_id, RouteKind::Party(resolution.clone()))
            }
            HostedMenuAction::ContinueModdedPlay { request_id } => (*request_id, RouteKind::Local),
            HostedMenuAction::PlayVanilla { request_id } => (*request_id, RouteKind::Vanilla),
            HostedMenuAction::CancelPartyJoin { .. } => {
                hosted.join_request = None;
                hosted.active_join_request = None;
                hosted.join_resolution = None;
                return Ok(true);
            }
            HostedMenuAction::CancelModdedPlay { .. } => {
                hosted.new_game_selection = None;
                return Ok(true);
            }
            _ => return Ok(false),
        };
        let selection = match &kind {
            RouteKind::Party(_) => NewGameSelection {
                name: hosted.party_requester.0.clone(),
                element: WizardElement::Ether,
                discipline: WizardDiscipline::Arcane,
            },
            _ => {
                if request_id != hosted.routing_sequence {
                    return Ok(true);
                }
                let Some(selection) = hosted.new_game_selection.clone() else { return Ok(true) };
                selection
            }
        };
        self.menu.set_routing_progress(request_id, None)?;
        hosted.pending.push(HostedRequest::Route(RouteRequest {
            request_id,
            kind,
            signed_in: hosted.account.is_some(),
            active_mods: hosted.active_mods.clone(),
            selection,
            vanilla_shared: game_shared_hub_enabled(self.menu.settings())
                && hosted
                    .profile
                    .as_ref()
                    .is_none_or(|profile| profile.metadata.integrity == SaveIntegrity::GlobalClean),
        }));
        Ok(true)
    }

    pub(super) fn complete_hosted_route(
        &mut self,
        request_id: u64,
        completed: CompletedRoute,
    ) -> Result<(), String> {
        self.hosted.as_mut().unwrap().active_mods = completed.active_mods;
        let prepared = match completed.result {
            Ok(prepared) => prepared,
            Err(error) => {
                self.menu.complete_routing(request_id, Err(error))?;
                return Ok(());
            }
        };
        if !self.menu.set_routing_progress(request_id, None)? {
            return Ok(());
        }
        if prepared.disable_cheats && self.menu.settings().enable_cheats {
            let mut settings = self.menu.settings().clone();
            settings.enable_cheats = false;
            self.commit_settings(settings)?;
        }
        if self.has_game {
            // Source leaveGameplayForPartyTransfer preserves its exact checkpoint
            // through Create and cannot destroy the old session before storage commits.
            self.native_menu_action(&MenuAction::Leave)?;
            self.hosted.as_mut().unwrap().pending_transfer = Some((request_id, prepared));
            return Ok(());
        }
        self.menu.complete_routing(request_id, Ok(()))?;
        self.begin_prepared_hosted_route(prepared)
    }

    pub(super) fn begin_prepared_hosted_route(
        &mut self,
        prepared: PreparedRoute,
    ) -> Result<(), String> {
        let hosted = self.hosted.as_mut().unwrap();
        hosted.admission_override = Some(prepared.admission);
        hosted.allow_mod_mismatch = prepared.allow_mod_mismatch;
        hosted.new_game_selection = None;
        hosted.join_resolution = None;
        if !prepared.allow_mod_mismatch
            && self.confirm_hosted_saved_mods(&prepared.selection, false)?
        {
            return Ok(());
        }
        self.start_hosted_new_game(prepared.selection)
    }

    pub(super) fn confirm_hosted_saved_mods(
        &mut self,
        selection: &NewGameSelection,
        resume: bool,
    ) -> Result<bool, String> {
        let hosted = self.hosted.as_mut().unwrap();
        let Some(profile) = &hosted.profile else { return Ok(false) };
        let active: Vec<_> = hosted
            .active_mods
            .iter()
            .map(|item| solomon_sim::modding::identity::GameContentIdentity {
                id: item.id.clone(),
                version: item.version.clone(),
                content_sha256: item.content_sha256.clone(),
            })
            .collect();
        let Some(mismatch) =
            solomon_sim::menu::save_mods::game_save_mod_mismatch(&profile.metadata.mods, &active)
        else {
            return Ok(false);
        };
        hosted.routing_sequence += 1;
        hosted.pending_mismatch = Some((hosted.routing_sequence, selection.clone(), resume));
        self.menu.show_mod_mismatch(hosted.routing_sequence, mismatch)?;
        Ok(true)
    }

    pub(super) fn present_party_resolution(
        &mut self,
        request_id: u64,
        resolution: PartyJoinResolution,
    ) -> Result<(), String> {
        let hosted = self.hosted.as_mut().unwrap();
        if hosted.active_join_request != Some(request_id) {
            return Ok(());
        }
        if resolution.target.kind == GameSessionKind::GlobalHub
            && !game_shared_hub_enabled(self.menu.settings())
        {
            self.menu.complete_party_join(
                request_id,
                Err("The shared Hub is disabled in Game Settings.".into()),
            )?;
            return Ok(());
        }
        if !self.menu.complete_party_join(request_id, Ok(()))? {
            return Ok(());
        }
        hosted.active_join_request = None;
        self.offer_resolved_party(request_id, resolution)
    }

    pub(super) fn offer_resolved_party(
        &mut self,
        request_id: u64,
        resolution: PartyJoinResolution,
    ) -> Result<(), String> {
        let hosted = self.hosted.as_mut().unwrap();
        let cheats = self.menu.settings().enable_cheats
            && !hosted.account.as_ref().is_some_and(|account| account.developer_access);
        if resolution.target.kind == GameSessionKind::GlobalHub
            && hosted.active_mods.is_empty()
            && !cheats
        {
            hosted.admission_override = Some(Admission::Party(resolution.intent_id));
            let selection = NewGameSelection {
                name: hosted.party_requester.0.clone(),
                element: WizardElement::Ether,
                discipline: WizardDiscipline::Arcane,
            };
            return self.start_hosted_new_game(selection);
        }
        self.menu.show_party_join_consent(
            request_id,
            resolution.target.clone(),
            !hosted.active_mods.is_empty() || cheats,
            hosted.account.is_some(),
        )?;
        hosted.join_resolution = Some((request_id, resolution));
        Ok(())
    }
}
