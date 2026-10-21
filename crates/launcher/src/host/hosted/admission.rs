//! Ordinary source Hello admission. Recovery tickets use the separate retained-run path.

use solomon_assets::website_save::{self, SaveIntegrity, SaveMetadata};
use solomon_protocol::hosted::{GameSaveIntent, HostedHello};
use solomon_protocol::{HostErrorCode, PROTOCOL_VERSION};
use solomon_sim::modding::LuaObject;
use solomon_sim::social::GameSessionKind;

use super::{HostedAdmission, HostedGameHost, HostedPlayer, HostedWorlds};
use crate::host::admission::validate_selection;
use crate::host::authority::close_peer;
use crate::host::connection::HostConnection;
use crate::host::signing::{credentials_equal, random_party_identity, random_token};

type AdmissionError = (HostErrorCode, String);

impl HostedGameHost {
    pub(in crate::host) fn admit_ordinary(
        &mut self,
        connection: &mut HostConnection,
        hello: HostedHello,
        admission: HostedAdmission,
        now_ms: f64,
    ) -> Result<String, AdmissionError> {
        if admission.observer.is_some() {
            return Err(authentication("An observer credential cannot admit a playable wizard."));
        }
        if hello.version != PROTOCOL_VERSION {
            return Err((
                HostErrorCode::UnsupportedVersion,
                "The game client needs to update.".into(),
            ));
        }
        if self.editor_test
            && (hello.save.is_some()
                || hello.resume_token.is_some()
                || hello.begin_college_intro
                || admission.party_id.is_some())
        {
            return Err(authentication("Editor tests always use a fresh disposable wizard."));
        }
        validate_selection(&hello.selection)?;
        if self.kind == GameSessionKind::GlobalHub {
            if !admission.content.manifest.mods.is_empty() {
                return Err(invalid("Mods require a private College."));
            }
            if hello.cheats_enabled && !admission.developer_access {
                return Err(invalid("Cheats require a private College."));
            }
        }
        if hello.decline_tutorial
            && (hello.begin_college_intro || hello.save.is_some() || hello.resume_token.is_some())
        {
            return Err(invalid(
                "Tutorial decline requires a fresh admission without the College introduction.",
            ));
        }
        if admission.party_rejoin_token.is_some() || admission.party_recovery_seed {
            return self.admit_recovery(connection, hello, admission, now_ms);
        }
        if admission.party_id.is_some()
            && !self.valid_party_reservation(
                admission.party_id.as_deref().unwrap(),
                admission.reservation_id.as_deref(),
                now_ms,
            )
        {
            return Err(authentication("That party admission has expired."));
        }
        if self.capacity_participant_count() >= self.maximum_players {
            return Err((HostErrorCode::HostFull, "The session is full.".into()));
        }
        // Acquire OS identities before changing a source player or gameplay RNG lane.
        let identity = random_party_identity().map_err(server_error)?;
        let token = random_token::<32>().map_err(server_error)?;
        let new_reference = format!("player-ref-{}", random_token::<24>().map_err(server_error)?);
        let mut profile = None;
        let mut continuation = None;
        let mut restore_mod_state = false;
        if let Some(document) = &hello.save {
            let intent =
                hello.save_intent.ok_or_else(|| invalid("The game save intent is missing."))?;
            let importing_into_private_party = self.kind == GameSessionKind::PrivateCollege
                && admission.party_id.is_some()
                && intent == GameSaveIntent::NewGame
                && self.worlds.hub().boneyard.is_none();
            if self.kind == GameSessionKind::PrivateCollege
                && !importing_into_private_party
                && (!self.players.is_empty()
                    || self.worlds.participant_count() != 0
                    || self.worlds.hub().boneyard.is_some())
            {
                return Err(invalid("A save may load only on a fresh host owner."));
            }
            let parsed = website_save::parse(document).map_err(invalid_owned)?;
            let retired;
            let profile_document =
                if intent == GameSaveIntent::NewGame && parsed.continuation.is_some() {
                    retired = website_save::retire(document).map_err(invalid_owned)?;
                    &retired
                } else {
                    document
                };
            let saved = website_save::restore_profile(profile_document).map_err(invalid_owned)?;
            if self.kind == GameSessionKind::GlobalHub
                && saved.metadata.integrity == SaveIntegrity::LocalOnly
            {
                return Err(invalid("Local-only saves require a private College."));
            }
            let mismatch = !same_mods(&saved.metadata.mods, &admission.content.manifest.mods);
            restore_mod_state = !importing_into_private_party && !mismatch;
            if mismatch && !hello.allow_mod_mismatch {
                return Err(invalid(
                    "The saved mod list does not match this session. Confirm the mismatch before loading it.",
                ));
            }
            if intent == GameSaveIntent::Resume {
                let mut restored = website_save::restore(document).map_err(invalid_owned)?;
                let character =
                    &restored.session.participants[&restored.player_id].player.selection;
                if character != &hello.selection {
                    return Err(invalid(
                        "The game save character does not match the resume request.",
                    ));
                }
                if mismatch
                    && restored.loaded_boneyard.as_ref().is_some_and(|loaded| {
                        loaded.mod_id.as_ref().is_some_and(|id| {
                            !admission.content.manifest.mods.iter().any(|active| {
                                active.id.eq_ignore_ascii_case(id)
                                    && saved
                                        .metadata
                                        .mods
                                        .iter()
                                        .any(|saved| same_mod(saved, active))
                            })
                        })
                    })
                {
                    restored.session.return_to_hub();
                    restored.loaded_boneyard = None;
                }
                continuation = Some(restored);
            }
            profile = Some(saved);
        }
        let mut previous_peer = None;
        let id = if let Some(restored) = continuation {
            let id = restored.player_id;
            if let Some(live) = self.worlds.state_for_player(&id) {
                let current = self
                    .player(&id)
                    .ok_or_else(|| invalid("This wizard is already active in another browser."))?;
                if hello
                    .resume_token
                    .as_ref()
                    .is_none_or(|candidate| !credentials_equal(candidate, &current.resume_token))
                {
                    return Err(invalid("This wizard is already active in another browser."));
                }
                if live.participants[&id].player.selection != hello.selection {
                    return Err(invalid("The active wizard does not match the resume request."));
                }
                previous_peer = Some(current.peer_id);
            } else {
                let navigation = restored
                    .loaded_boneyard
                    .as_ref()
                    .map(|loaded| self.content_loader.load_navigation(loaded))
                    .transpose()
                    .map_err(server_error)?;
                self.worlds
                    .restore_player(restored.session, restored.loaded_boneyard, &id, identity)
                    .map_err(invalid_owned)?;
                if let Some(navigation) = navigation {
                    let party_id = self
                        .party_id_for_player(&id)
                        .expect("a restored source actor has a party")
                        .to_owned();
                    self.navigation.insert(party_id, navigation);
                }
            }
            if self.kind != GameSessionKind::GlobalHub
                && let Some(number) = id
                    .strip_prefix("player-")
                    .and_then(|number| number.parse::<u64>().ok())
                    .filter(|number| *number <= 9_007_199_254_740_991)
            {
                self.next_player_id = self.next_player_id.max(number + 1);
            }
            id
        } else {
            let id = if self.kind == GameSessionKind::GlobalHub {
                format!("player-{}", random_token::<12>().map_err(server_error)?)
            } else {
                format!("player-{}", self.next_player_id)
            };
            self.worlds
                .add_player(
                    id.clone(),
                    hello.selection.clone(),
                    identity,
                    self.available_party_members(
                        admission.party_id.as_deref(),
                        admission.reservation_id.as_deref(),
                    ),
                )
                .map_err(invalid_owned)?;
            if self.kind != GameSessionKind::GlobalHub {
                self.next_player_id += 1;
            }
            if let Some(profile) = &profile {
                website_save::hydrate_profile(
                    self.worlds.state_for_player_mut(&id).unwrap(),
                    &id,
                    profile,
                )
                .map_err(invalid_owned)?;
            }
            id
        };
        if let Some(party_id) = &admission.party_id {
            let maximum_members = self.available_party_members(
                admission.party_id.as_deref(),
                admission.reservation_id.as_deref(),
            );
            if let HostedWorlds::Global(worlds) = &mut self.worlds
                && let Err(reason) = worlds.join_party(&id, party_id, maximum_members)
            {
                worlds.remove_player(&id).map_err(server_error)?;
                return Err(invalid_owned(reason.id().to_owned()));
            }
            self.cancel_party_reservation(admission.reservation_id.as_deref().unwrap());
        }
        let state = self
            .worlds
            .state_for_player_mut(&id)
            .expect("accepted admission retains its source actor");
        if hello.decline_tutorial {
            state.decline_tutorial(&id);
        }
        if hello.begin_college_intro
            || profile.as_ref().is_some_and(|profile| profile.economy.college_intro_pending)
        {
            state.arm_college_intro(&id);
            self.college_intro_ready.remove(&id);
        }
        let requested_cheats = hello.cheats_enabled && !admission.developer_access;
        if self.kind == GameSessionKind::PrivateCollege && !self.cheat_policy_initialized {
            self.cheats_enabled = requested_cheats;
            self.cheat_policy_initialized = true;
        }
        let cheat_mode = if self.kind == GameSessionKind::PrivateCollege {
            self.cheats_enabled
        } else {
            requested_cheats
        };
        connection.cheats_enabled = cheat_mode;
        let previous_reference = self.player(&id).map(|player| player.player_reference.clone());
        if let Some(peer_id) = previous_peer {
            if let Some(replaced) = self.connections.get_mut(&peer_id) {
                replaced.player_id = None;
                replaced.input.neutralize();
                replaced.planned_disconnect =
                    Some(("connection-replaced", "wizard resumed in another browser".into()));
                close_peer(&replaced.peer, 4002, "wizard resumed in another browser")
                    .map_err(server_error)?;
            }
            self.players.retain(|player| player.id != id);
        }
        let metadata = SaveMetadata {
            integrity: if self.kind != GameSessionKind::GlobalHub
                || cheat_mode
                || profile
                    .as_ref()
                    .is_some_and(|profile| profile.metadata.integrity == SaveIntegrity::LocalOnly)
                || !admission.content.manifest.mods.is_empty()
            {
                SaveIntegrity::LocalOnly
            } else {
                SaveIntegrity::GlobalClean
            },
            mods: admission.content.manifest.mods.clone(),
            mod_state: profile
                .as_ref()
                .filter(|profile| {
                    same_mods(&profile.metadata.mods, &admission.content.manifest.mods)
                })
                .map_or_else(LuaObject::new, |profile| profile.metadata.mod_state.clone()),
            native_source: profile
                .as_ref()
                .and_then(|profile| profile.metadata.native_source.clone()),
        };
        self.players.push(HostedPlayer {
            id: id.clone(),
            resume_token: token,
            content: admission.content.clone(),
            peer_id: connection.peer.id(),
            player_reference: previous_reference.unwrap_or(new_reference),
            social_connection: None,
            profile: hello.profile,
            preferences: hello.online_preferences,
            developer_access: admission.developer_access,
            leaderboard_user_id: admission.leaderboard_user_id,
            save_metadata: metadata,
            global_score_eligible: self.kind != GameSessionKind::PrivateCollege
                && hello.save.is_none()
                && !cheat_mode
                && admission.content.manifest.mods.is_empty(),
            terminal_saved_run_id: None,
            last_save_sequence: 0,
            last_save_document: None,
            tutorial_eligible: hello.save.is_none()
                && !cheat_mode
                && admission.content.manifest.mods.is_empty(),
        });
        self.initialize_enhanced_effects(
            &id,
            hello.enhanced_effects,
            hello.save_intent == Some(GameSaveIntent::Resume),
            previous_peer.is_some(),
        );
        self.initialize_admitted_mod_world(&id, restore_mod_state && previous_peer.is_none())
            .map_err(invalid_owned)?;
        if let Some(state) = self.worlds.state_for_player(&id)
            && let Some(run_id) = state.run.run_id.as_ref().filter(|_| state.boneyard.is_some())
        {
            let run_id = run_id.clone();
            let party_id = self.party_id_for_player(&id).unwrap().to_owned();
            let facts = self.control_facts(&party_id);
            let control = self.controls.entry(party_id.clone()).or_default();
            control.arm_renderers(&run_id, &facts.connected);
            control.begin_loading_grace(
                &id,
                solomon_protocol::ResumeGraceReason::GameRestarted,
                &facts.connected,
                &[],
            );
            self.arm_party_rejoins(&party_id).map_err(server_error)?;
        }
        connection.player_id = Some(id.clone());
        self.publish_dirty = true;
        self.social_dirty = true;
        Ok(id)
    }
}

fn same_mod(
    first: &solomon_sim::modding::identity::GameContentIdentity,
    second: &solomon_sim::modding::identity::GameContentIdentity,
) -> bool {
    first.id.eq_ignore_ascii_case(&second.id)
        && first.version == second.version
        && first.content_sha256.eq_ignore_ascii_case(&second.content_sha256)
}
pub(super) fn same_mods(
    first: &[solomon_sim::modding::identity::GameContentIdentity],
    second: &[solomon_sim::modding::identity::GameContentIdentity],
) -> bool {
    first.len() == second.len()
        && first.iter().all(|value| second.iter().any(|candidate| same_mod(value, candidate)))
}
fn invalid(reason: &str) -> AdmissionError {
    (HostErrorCode::InvalidMessage, reason.into())
}
fn invalid_owned(reason: String) -> AdmissionError {
    (HostErrorCode::InvalidMessage, reason)
}
fn authentication(reason: &str) -> AdmissionError {
    (HostErrorCode::InvalidCredentials, reason.into())
}
fn server_error(reason: String) -> AdmissionError {
    (HostErrorCode::ServerError, reason)
}
