//! College actor selection from Website `hub-player-selection.ts` and the local
//! `HubScene.tsx` player card. Hosted profile lookup is a separate social route.

use crate::Vec2;
use crate::hub::regions::HubRegion;

use super::PartyMembership;

/// Borrow actors in the source player insertion order, including the local actor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HubPlayerSelection<'a> {
    pub player_id: &'a str,
    pub position: Vec2,
    pub region: Option<HubRegion>,
}

/// `selectHubPlayerAtPoint`: overlapping actors follow feet depth, with the later
/// registered player winning a tie. The visual target includes the whole wizard.
#[must_use]
pub fn select_hub_player_at_point<'a>(
    players: &[HubPlayerSelection<'a>],
    local_player_id: &str,
    point: Vec2,
) -> Option<&'a str> {
    let region = players.iter().find(|player| player.player_id == local_player_id)?.region?;
    if !point.x.is_finite() || !point.y.is_finite() {
        return None;
    }
    let mut selected: Option<&HubPlayerSelection<'a>> = None;
    for player in players {
        if player.player_id == local_player_id
            || player.region != Some(region)
            || point.x < player.position.x - 45.0
            || point.x > player.position.x + 45.0
            || point.y < player.position.y - 110.0
            || point.y > player.position.y + 30.0
        {
            continue;
        }
        let is_in_front = match selected {
            None => true,
            Some(previous) => player.position.y >= previous.position.y,
        };
        if is_in_front {
            selected = Some(player);
        }
    }
    selected.map(|player| player.player_id)
}

/// `nearestHubPlayer`: controller interaction defaults to 120 world units. Equal
/// distances retain the first actor, unlike the pointer's equal-depth ordering.
#[must_use]
pub fn nearest_hub_player<'a>(
    players: &[HubPlayerSelection<'a>],
    local_player_id: &str,
    maximum_distance: f64,
) -> Option<&'a str> {
    let local = players.iter().find(|player| player.player_id == local_player_id)?;
    let region = local.region?;
    if maximum_distance <= 0.0 || !maximum_distance.is_finite() {
        return None;
    }
    let mut nearest = None;
    for player in players {
        if player.player_id == local_player_id || player.region != Some(region) {
            continue;
        }
        let dx = player.position.x - local.position.x;
        let dy = player.position.y - local.position.y;
        let distance_squared = dx * dx + dy * dy;
        if distance_squared > maximum_distance * maximum_distance {
            continue;
        }
        if nearest.is_none_or(|(_, previous)| distance_squared < previous) {
            nearest = Some((player.player_id, distance_squared));
        }
    }
    nearest.map(|(player_id, _)| player_id)
}

/// The local actor card sends an in-session invitation. It neither requests a
/// hosted profile nor requires an account, and therefore also works for bots.
#[must_use]
pub fn hub_player_party_invite_available(
    party: Option<&PartyMembership>,
    local_player_id: &str,
    selected_player_id: &str,
) -> bool {
    party.is_some_and(|party| {
        party.leader_player_id == local_player_id
            && !party.member_player_ids.iter().any(|id| id == selected_player_id)
    })
}
