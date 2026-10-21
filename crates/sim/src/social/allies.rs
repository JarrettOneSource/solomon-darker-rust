//! Ally identities and durable/offline health presentation from Website `ally-hud.ts`.

use crate::combat::PlayerLifeState;
use crate::profile::WizardElement;

use super::PartyRosterPlayer;

#[derive(Clone, Debug, PartialEq)]
pub enum AllyHudIdentity {
    Player { display_name: String, element: WizardElement },
    Golem,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AllyHudRow {
    pub connected: bool,
    pub dead: bool,
    pub health_ratio: f64,
    pub id: String,
    pub identity: AllyHudIdentity,
}

impl AllyHudRow {
    #[must_use]
    pub fn accessible_name(&self) -> &str {
        match &self.identity {
            AllyHudIdentity::Player { display_name, .. } => display_name,
            AllyHudIdentity::Golem => "Golem",
        }
    }

    #[must_use]
    pub fn accessible_status(&self) -> String {
        let name = self.accessible_name();
        if self.identity == AllyHudIdentity::Golem {
            return name.to_string();
        }
        if self.dead && !self.connected {
            format!("{name}, dead and disconnected")
        } else if self.dead {
            format!("{name}, dead")
        } else if !self.connected {
            format!("{name}, disconnected")
        } else {
            name.to_string()
        }
    }
}

#[must_use]
pub fn derive_player_ally_hud_rows(
    players: &[PartyRosterPlayer],
    local_player_id: &str,
    party_roster: Option<&[PartyRosterPlayer]>,
) -> Vec<AllyHudRow> {
    let mut roster: Vec<_> = party_roster
        .unwrap_or(players)
        .iter()
        .filter(|player| player.player_id != local_player_id)
        .collect();
    roster.sort_by(|left, right| left.player_id.cmp(&right.player_id));
    let mut rows = Vec::with_capacity(roster.len());
    for retained in roster {
        let snapshot = players.iter().find(|player| player.player_id == retained.player_id);
        let connected = party_roster.map_or(snapshot.is_some(), |_| retained.connected);
        let player = if connected { snapshot.unwrap_or(retained) } else { retained };
        rows.push(AllyHudRow {
            connected,
            dead: player.life_state != PlayerLifeState::Alive || player.current_health <= 0.0,
            health_ratio: clamp_ally_hud_health_ratio(
                player.current_health / player.maximum_health,
            ),
            id: retained.player_id.clone(),
            identity: AllyHudIdentity::Player {
                display_name: player.display_name.clone(),
                element: player.element,
            },
        });
    }
    rows
}

/// A borrowed actor projection is constructed only from actual Golem actors with a
/// populated Golem component. It contains no second health or lifecycle owner.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AllyGolemProjection<'a> {
    pub id: u64,
    pub world_key: &'a str,
    pub current_health: f64,
    pub maximum_health: f64,
}

#[must_use]
pub fn derive_golem_ally_hud_rows(
    actors: &[AllyGolemProjection<'_>],
    world_key: &str,
) -> Vec<AllyHudRow> {
    let mut actors: Vec<_> = actors.iter().filter(|actor| actor.world_key == world_key).collect();
    actors.sort_by_key(|actor| actor.id);
    actors
        .into_iter()
        .map(|actor| AllyHudRow {
            connected: true,
            dead: false,
            health_ratio: clamp_ally_hud_health_ratio(actor.current_health / actor.maximum_health),
            id: format!("golem:{}", actor.id),
            identity: AllyHudIdentity::Golem,
        })
        .collect()
}

#[must_use]
pub fn clamp_ally_hud_health_ratio(ratio: f64) -> f64 {
    ratio.clamp(0.0, 1.0)
}
