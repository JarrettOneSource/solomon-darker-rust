//! The local `HubScene` actor popup reads the presented wizard and current Party
//! profile. It never performs the separate hosted PlayerCard lookup.

use std::collections::BTreeMap;

use solomon_protocol::HubActivity;
use solomon_sim::profile::NewGameSelection;
use solomon_sim::session::GameSession;

use super::{SocialUi, Surface};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct HubPlayerCard {
    pub player_id: String,
    pub selection: NewGameSelection,
    pub gold: u64,
    pub activity: Option<HubActivity>,
}

impl SocialUi {
    /// Supply the actual displayed College actors. Only card display fields are
    /// retained, and departure/region changes close the selected actor popup.
    pub fn set_hub_players(
        &mut self,
        session: Option<&GameSession>,
        local_player_id: &str,
        activities: Option<&BTreeMap<String, HubActivity>>,
    ) {
        let mut players = Vec::new();
        if let Some(session) = session
            && session.boneyard.is_none()
            && let Some(local) = session.participants.get(local_player_id)
        {
            for id in &session.participant_order {
                let participant = &session.participants.get(id).expect("participant exists");
                if participant.wizard.region != local.wizard.region {
                    continue;
                }
                players.push(HubPlayerCard {
                    player_id: id.clone(),
                    selection: participant.player.selection.clone(),
                    gold: participant.player.economy.gold,
                    activity: activities.and_then(|activities| activities.get(id).copied()),
                });
            }
        }
        if self.hub_players == players {
            return;
        }
        self.hub_players = players;
        if let Some(Surface::HubPlayer(id)) = &self.surface
            && !self.hub_players.iter().any(|player| player.player_id == *id)
        {
            self.surface = None;
        }
    }

    /// Call after the source pointer/controller selection. The same local route
    /// handles Party member rows; no account or card-service availability gate is added.
    pub fn open_hub_player(&mut self, player_id: &str) -> bool {
        if !self.context.visible
            || self.context.input_suspended
            || self.snapshot.is_none()
            || !self.hub_players.iter().any(|player| player.player_id == player_id)
        {
            return false;
        }
        self.close_chat();
        self.requested_card = None;
        self.hosted_card_request = None;
        self.surface = Some(Surface::HubPlayer(player_id.to_owned()));
        self.focus_modal = true;
        true
    }
}
