//! Ordinary source Hello admission. Recovery tickets use the separate retained-run path.

use solomon_assets::website_save::{
    self, RestoredGameProfile, RestoredGameSave, SaveIntegrity, SaveMetadata,
};
use solomon_protocol::hosted::{GameSaveIntent, HostedHello};
use solomon_protocol::{HostErrorCode, PROTOCOL_VERSION};
use solomon_sim::modding::LuaObject;
use solomon_sim::social::{GameSessionKind, PartyIdentity};

use super::{HostedAdmission, HostedGameHost, HostedPlayer, HostedWorlds};
use crate::host::admission::validate_selection;
use crate::host::authority::close_peer;
use crate::host::connection::HostConnection;
use crate::host::signing::{credentials_equal, random_party_identity, random_token};
use crate::transport::PeerId;

type AdmissionError = (HostErrorCode, String);

/// The save a Hello brings, once accepted: its durable profile, the continuation a Resume
/// restores, and whether the saved mod state may be restored.
#[derive(Default)]
struct AdmittedSave {
    profile: Option<RestoredGameProfile>,
    continuation: Option<RestoredGameSave>,
    restore_mod_state: bool,
}

impl HostedGameHost {
    pub(in crate::host) fn admit_ordinary(
        &mut self,
        connection: &mut HostConnection,
        hello: HostedHello,
        admission: HostedAdmission,
        now_ms: f64,
    ) -> Result<String, AdmissionError> {
        self.validate_ordinary_hello(&hello, &admission)?;
        if admission.party_rejoin_token.is_some() || admission.party_recovery_seed {
            return self.admit_recovery(connection, hello, admission, now_ms);
        }
        self.check_room_for_wizard(&admission, now_ms)?;
        // Acquire OS identities before changing a source player or gameplay RNG lane.
        let identity = random_party_identity().map_err(server_error)?;
        let token = random_token::<32>().map_err(server_error)?;
        let new_reference = format!("player-ref-{}", random_token::<24>().map_err(server_error)?);
        let AdmittedSave { profile, continuation, restore_mod_state } =
            self.load_admitted_save(&hello, &admission)?;
        let mut previous_peer = None;
        let id = if let Some(restored) = continuation {
            self.resume_saved_wizard(restored, &hello, identity, &mut previous_peer)?
        } else {
            self.add_new_wizard(&hello, &admission, identity, profile.as_ref())?
        };
        self.join_reserved_party(&id, &admission)?;
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
        let cheat_mode = self.admission_cheat_mode(&hello, &admission);
        connection.cheats_enabled = cheat_mode;
        let previous_reference = self.player(&id).map(|player| player.player_reference.clone());
        if let Some(peer_id) = previous_peer {
            self.replace_resumed_connection(&id, peer_id)?;
        }
        let metadata = self.admitted_save_metadata(cheat_mode, profile.as_ref(), &admission);
        self.players.push(HostedPlayer {
            reconnect_request_ids: std::collections::BTreeSet::new(),
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
        self.arm_restored_run_controls(&id)?;
        connection.player_id = Some(id.clone());
        self.publish_dirty = true;
        self.social_dirty = true;
        Ok(id)
    }

    /// Rejects a Hello whose credential, client version, selection or requests this session
    /// cannot accept.
    fn validate_ordinary_hello(
        &self,
        hello: &HostedHello,
        admission: &HostedAdmission,
    ) -> Result<(), AdmissionError> {
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
        Ok(())
    }

    /// A party ticket needs its unexpired reservation, and every admission needs a free place.
    fn check_room_for_wizard(
        &mut self,
        admission: &HostedAdmission,
        now_ms: f64,
    ) -> Result<(), AdmissionError> {
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
        Ok(())
    }

    /// Accepts the save a Hello carries, if any: where it may load, its profile and mod list,
    /// and for a Resume, its continuation.
    fn load_admitted_save(
        &self,
        hello: &HostedHello,
        admission: &HostedAdmission,
    ) -> Result<AdmittedSave, AdmissionError> {
        let Some(document) = &hello.save else {
            return Ok(AdmittedSave::default());
        };
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
        let profile_document = if intent == GameSaveIntent::NewGame && parsed.continuation.is_some()
        {
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
        let restore_mod_state = !importing_into_private_party && !mismatch;
        if mismatch && !hello.allow_mod_mismatch {
            return Err(invalid(
                "The saved mod list does not match this session. Confirm the mismatch before loading it.",
            ));
        }
        let mut continuation = None;
        if intent == GameSaveIntent::Resume {
            let mut restored = website_save::restore(document).map_err(invalid_owned)?;
            let character = &restored.session.participants[&restored.player_id].player.selection;
            if character != &hello.selection {
                return Err(invalid("The game save character does not match the resume request."));
            }
            if mismatch
                && restored.loaded_boneyard.as_ref().is_some_and(|loaded| {
                    loaded.mod_id.as_ref().is_some_and(|id| {
                        !admission.content.manifest.mods.iter().any(|active| {
                            active.id.eq_ignore_ascii_case(id)
                                && saved.metadata.mods.iter().any(|saved| same_mod(saved, active))
                        })
                    })
                })
            {
                restored.session.return_to_hub();
                restored.loaded_boneyard = None;
            }
            continuation = Some(restored);
        }
        Ok(AdmittedSave { profile: Some(saved), continuation, restore_mod_state })
    }

    /// Resumes a saved wizard: reclaims it from another browser when it is already live, or
    /// restores its saved world. Returns its player ID.
    fn resume_saved_wizard(
        &mut self,
        restored: RestoredGameSave,
        hello: &HostedHello,
        identity: PartyIdentity,
        previous_peer: &mut Option<PeerId>,
    ) -> Result<String, AdmissionError> {
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
            *previous_peer = Some(current.peer_id);
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
        Ok(id)
    }

    /// Adds a new wizard with the Hello's character, hydrating the saved profile when the Hello
    /// brings one. Returns its player ID.
    fn add_new_wizard(
        &mut self,
        hello: &HostedHello,
        admission: &HostedAdmission,
        identity: PartyIdentity,
        profile: Option<&RestoredGameProfile>,
    ) -> Result<String, AdmissionError> {
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
        if let Some(profile) = profile {
            website_save::hydrate_profile(
                self.worlds.state_for_player_mut(&id).unwrap(),
                &id,
                profile,
            )
            .map_err(invalid_owned)?;
        }
        Ok(id)
    }

    /// Joins the wizard to the party its ticket reserved, then releases the reservation. A
    /// rejected Global College join removes the wizard again.
    fn join_reserved_party(
        &mut self,
        id: &str,
        admission: &HostedAdmission,
    ) -> Result<(), AdmissionError> {
        if let Some(party_id) = &admission.party_id {
            let maximum_members = self.available_party_members(
                admission.party_id.as_deref(),
                admission.reservation_id.as_deref(),
            );
            if let HostedWorlds::Global(worlds) = &mut self.worlds
                && let Err(reason) = worlds.join_party(id, party_id, maximum_members)
            {
                worlds.remove_player(id).map_err(server_error)?;
                return Err(invalid_owned(reason.id().to_owned()));
            }
            self.cancel_party_reservation(admission.reservation_id.as_deref().unwrap());
        }
        Ok(())
    }

    /// Whether cheats apply to the admitted wizard. A private College settles its cheat policy
    /// at its first admission; other sessions follow each Hello.
    fn admission_cheat_mode(&mut self, hello: &HostedHello, admission: &HostedAdmission) -> bool {
        let requested_cheats = hello.cheats_enabled && !admission.developer_access;
        if self.kind == GameSessionKind::PrivateCollege && !self.cheat_policy_initialized {
            self.cheats_enabled = requested_cheats;
            self.cheat_policy_initialized = true;
        }
        if self.kind == GameSessionKind::PrivateCollege {
            self.cheats_enabled
        } else {
            requested_cheats
        }
    }

    /// Closes the browser connection that held a resumed wizard and drops its old player record.
    fn replace_resumed_connection(
        &mut self,
        id: &str,
        peer_id: PeerId,
    ) -> Result<(), AdmissionError> {
        if let Some(replaced) = self.connections.get_mut(&peer_id) {
            replaced.player_id = None;
            replaced.input.neutralize();
            replaced.planned_disconnect =
                Some(("connection-replaced", "wizard resumed in another browser".into()));
            close_peer(&replaced.peer, 4002, "wizard resumed in another browser")
                .map_err(server_error)?;
        }
        self.players.retain(|player| player.id != id);
        Ok(())
    }

    /// The admitted wizard's save metadata. Saves are local-only unless they come from a clean
    /// Global College.
    fn admitted_save_metadata(
        &self,
        cheat_mode: bool,
        profile: Option<&RestoredGameProfile>,
        admission: &HostedAdmission,
    ) -> SaveMetadata {
        SaveMetadata {
            integrity: if self.kind != GameSessionKind::GlobalHub
                || cheat_mode
                || profile
                    .is_some_and(|profile| profile.metadata.integrity == SaveIntegrity::LocalOnly)
                || !admission.content.manifest.mods.is_empty()
            {
                SaveIntegrity::LocalOnly
            } else {
                SaveIntegrity::GlobalClean
            },
            mods: admission.content.manifest.mods.clone(),
            mod_state: profile
                .filter(|profile| {
                    same_mods(&profile.metadata.mods, &admission.content.manifest.mods)
                })
                .map_or_else(LuaObject::new, |profile| profile.metadata.mod_state.clone()),
            native_source: profile.and_then(|profile| profile.metadata.native_source.clone()),
        }
    }

    /// A wizard admitted into a Boneyard run waits, with its party, for every connected renderer
    /// to load the restored world, and the party's rejoin slots are armed.
    fn arm_restored_run_controls(&mut self, id: &str) -> Result<(), AdmissionError> {
        if let Some(state) = self.worlds.state_for_player(id)
            && let Some(run_id) = state.run.run_id.as_ref().filter(|_| state.boneyard.is_some())
        {
            let run_id = run_id.clone();
            let party_id = self.party_id_for_player(id).unwrap().to_owned();
            let facts = self.control_facts(&party_id);
            let control = self.controls.entry(party_id.clone()).or_default();
            control.arm_renderers(&run_id, &facts.connected);
            control.begin_loading_grace(
                id,
                solomon_protocol::ResumeGraceReason::GameRestarted,
                &facts.connected,
                &[],
            );
            self.arm_party_rejoins(&party_id).map_err(server_error)?;
        }
        Ok(())
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
