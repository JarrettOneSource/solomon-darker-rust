//! Social views borrow actual live/detached participants. Party projection does not
//! manufacture health, progression, membership or a second player store.

use std::collections::{BTreeMap, BTreeSet};

use solomon_protocol::ServerMessage;
use solomon_sim::social::{
    GameChatSender, GameChatWorldKind, GamePlayerCardProfile, PartyPlayerProfile,
    PartyRosterPlayer, SocialSnapshot,
};

use super::HostedGameHost;
use crate::host::authority::send_control;

impl HostedGameHost {
    pub(in crate::host) fn chat_world_kind(&self, id: &str) -> GameChatWorldKind {
        if self.worlds.state_for_player(id).is_some_and(|state| state.boneyard.is_some()) {
            GameChatWorldKind::Boneyard
        } else {
            GameChatWorldKind::Hub
        }
    }

    pub(in crate::host) fn player_card(&self, id: &str) -> Option<GamePlayerCardProfile> {
        let admitted = self.player(id)?;
        let participant = self.retained_participant(id)?;
        let selection = &participant.player.selection;
        Some(GamePlayerCardProfile {
            player_id: id.into(),
            account_username: admitted.profile.account_username.clone(),
            activity: self.chat_world_kind(id),
            discipline: selection.discipline,
            display_name: selection.name.clone(),
            element: selection.element,
            gold: participant.player.economy.gold,
            highest_wave: admitted.profile.highest_wave,
            player_reference: admitted.player_reference.clone(),
            session_kind: self.kind,
            total_playtime_ms: admitted.profile.total_playtime_ms,
        })
    }

    pub(in crate::host) fn chat_sender(&self, id: &str) -> Result<GameChatSender, String> {
        let player = self.player(id).ok_or("The chat sender is not connected")?;
        let participant =
            self.retained_participant(id).ok_or("The chat sender has no actual wizard")?;
        Ok(GameChatSender {
            display_name: participant.player.selection.name.clone(),
            player_id: id.into(),
            player_reference: player.player_reference.clone(),
        })
    }

    pub(in crate::host) fn social_snapshot(&self, id: &str) -> Result<SocialSnapshot, String> {
        let mut profiles = BTreeMap::new();
        let mut roster = BTreeMap::new();
        let mut hub_players = BTreeSet::new();
        let mut player_cards = Vec::new();
        for party in &self.worlds.parties().parties {
            for member in &party.member_player_ids {
                let Some(participant) = self.retained_participant(member) else {
                    // Portable recovery may retain a not-yet-rejoined member only in
                    // its signed roster. That is real accepted state, not a new actor.
                    let Some(row) = self
                        .recovery_lineages
                        .iter()
                        .flat_map(|lineage| &lineage.party_roster)
                        .find(|row| &row.player_id == member)
                    else {
                        return Err(format!("The hosted party has no durable roster for {member}"));
                    };
                    roster.insert(
                        member.clone(),
                        PartyRosterPlayer {
                            connected: false,
                            current_health: row.current_health,
                            display_name: row.display_name.clone(),
                            element: row.element,
                            life_state: row.life_state,
                            maximum_health: row.maximum_health,
                            player_id: member.clone(),
                        },
                    );
                    continue;
                };
                let metadata = self
                    .player(member)
                    .map(|player| &player.profile)
                    .or_else(|| {
                        self.rejoin_slots
                            .iter()
                            .find(|slot| slot.player_id == *member)
                            .map(|slot| &slot.profile)
                    })
                    .or_else(|| {
                        self.bots.rows.iter().find(|bot| &bot.id == member).map(|bot| &bot.profile)
                    });
                let player = &participant.player;
                profiles.insert(
                    member.clone(),
                    PartyPlayerProfile {
                        account_username: metadata
                            .and_then(|profile| profile.account_username.clone()),
                        highest_wave: metadata.and_then(|profile| profile.highest_wave),
                        total_playtime_ms: metadata.and_then(|profile| profile.total_playtime_ms),
                        display_name: player.selection.name.clone(),
                        player_id: member.clone(),
                    },
                );
                let connected = self.player(member).is_some() || self.is_bot(member);
                roster.insert(
                    member.clone(),
                    PartyRosterPlayer {
                        connected,
                        current_health: player.combat.current_health,
                        display_name: player.selection.name.clone(),
                        element: player.selection.element,
                        life_state: player.combat.life_state,
                        maximum_health: player.combat.maximum_health,
                        player_id: member.clone(),
                    },
                );
                if connected && self.chat_world_kind(member) == GameChatWorldKind::Hub {
                    hub_players.insert(member.clone());
                }
                if let Some(card) = self.player_card(member) {
                    player_cards.push(card)
                }
            }
        }
        let party_state =
            self.worlds.parties().project(id, &profiles, &hub_players, &roster).map_err(
                |missing| format!("The hosted party has no durable roster for {}", missing.0),
            )?;
        Ok(SocialSnapshot {
            local_player_id: id.into(),
            session_kind: self.kind,
            world_kind: self.chat_world_kind(id),
            directory_available: !self.editor_test
                && self.kind != solomon_sim::social::GameSessionKind::Standalone,
            global_chat_available: self.player(id).is_some_and(|player| {
                !self.editor_test
                    && self.kind != solomon_sim::social::GameSessionKind::Standalone
                    && player.preferences.global_chat
            }),
            party_state: (self.kind != solomon_sim::social::GameSessionKind::Standalone)
                .then_some(party_state),
            chat_messages: Vec::new(),
            player_cards,
        })
    }

    pub(in crate::host) fn publish_social(&self) -> Result<(), String> {
        for player in &self.players {
            if let Some(connection) = self.connections.get(&player.peer_id)
                && !connection.peer.is_closed()
            {
                send_control(
                    &connection.peer,
                    ServerMessage::Social { snapshot: self.social_snapshot(&player.id)? },
                )?;
            }
        }
        Ok(())
    }
}
