//! Readonly match activity from Website `observer-activity.ts`.
//! Roster order and accepted skill changes come from the session; this projection never
//! chooses a skill or changes progression.

use crate::profile::{NATIVE_SKILL_CATALOG, PlayerSkillOffer, PlayerSkillOfferOption};

/// The fields the source observer reads from one participant, in session insertion order.
#[derive(Clone, Copy, Debug)]
pub struct ObserverSkillPlayer<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub pending_offer: Option<&'a PlayerSkillOffer>,
    pub learned_ranks: &'a [u32; 83],
    pub deferred_choices: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObserverSkillEvent {
    pub detail: String,
    pub id: String,
    pub player_name: String,
    pub title: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObserverSkillOfferPresentation {
    pub id: String,
    pub options: Vec<String>,
    pub player_name: String,
    pub title: String,
}

/// `observerSkillOffers`: actual pending native choices in participant insertion order.
#[must_use]
pub fn observer_skill_offers(
    players: &[ObserverSkillPlayer<'_>],
) -> Vec<ObserverSkillOfferPresentation> {
    let mut offers = Vec::new();
    for player in players {
        if let Some(offer) = player.pending_offer {
            offers.push(ObserverSkillOfferPresentation {
                id: format!("{}:{}", player.id, offer.sequence),
                options: offer.options.iter().map(option_name).collect(),
                player_name: player.name.to_owned(),
                title: format!("LEVEL {} CHOICE", offer.level),
            });
        }
    }
    offers
}

/// `deriveObserverSkillEvents`: report resolved choices before their replacement offers.
/// Learned ranks are permanent progression ranks, including choices which equipment might
/// currently boost. An unchanged offer sequence produces no event.
#[must_use]
pub fn derive_observer_skill_events(
    previous: &[ObserverSkillPlayer<'_>],
    current: &[ObserverSkillPlayer<'_>],
    tick: u64,
) -> Vec<ObserverSkillEvent> {
    let mut events = Vec::new();
    for player in current {
        let Some(before) = previous.iter().find(|before| before.id == player.id) else {
            continue;
        };
        let previous_sequence = before.pending_offer.map(|offer| offer.sequence);
        let current_sequence = player.pending_offer.map(|offer| offer.sequence);
        if previous_sequence == current_sequence {
            continue;
        }
        if let Some(offer) = before.pending_offer {
            let increased = increased_offered_skill(offer, before, player);
            let saved = player.deferred_choices > before.deferred_choices;
            let (title, detail) = if let Some((skill_id, rank)) = increased {
                ("SKILL SELECTED", format!("{} reached rank {rank}.", skill_name(skill_id)))
            } else if saved {
                ("SKILL SAVED", "The choice was saved for later.".to_owned())
            } else if player.pending_offer.is_some() {
                ("SKILL CHOICE UPDATED", "The offered choices were rerolled.".to_owned())
            } else {
                (
                    "SKILL CHOICE UPDATED",
                    "The choice closed without a learned-rank change.".to_owned(),
                )
            };
            events.push(ObserverSkillEvent {
                detail,
                id: format!("{tick}:{}:{}:resolved", player.id, offer.sequence),
                player_name: player.name.to_owned(),
                title: title.to_owned(),
            });
        }
        if let Some(offer) = player.pending_offer {
            events.push(ObserverSkillEvent {
                detail: offer.options.iter().map(option_name).collect::<Vec<_>>().join(" · "),
                id: format!("{tick}:{}:{}:offered", player.id, offer.sequence),
                player_name: player.name.to_owned(),
                title: format!("LEVEL {} CHOICES", offer.level),
            });
        }
    }
    events
}

fn increased_offered_skill(
    offer: &PlayerSkillOffer,
    previous: &ObserverSkillPlayer<'_>,
    current: &ObserverSkillPlayer<'_>,
) -> Option<(usize, u32)> {
    // The source picks the first increased skill in offer order, rather than rank order.
    for option in &offer.options {
        let before = previous.learned_ranks.get(option.skill_id).copied().unwrap_or(0);
        let after = current.learned_ranks.get(option.skill_id).copied().unwrap_or(0);
        if after > before {
            return Some((option.skill_id, after));
        }
    }
    None
}

fn option_name(option: &PlayerSkillOfferOption) -> String {
    let name = skill_name(option.skill_id);
    if option.target_rank > 1 { format!("{name} {}", option.target_rank) } else { name }
}

fn skill_name(skill_id: usize) -> String {
    // Row 82 exists only as the source's extra stat-book row, outside CATALOG.skills.
    // The Rust table retains it for fixed-size runtime indexing; observer names must
    // still use the catalog's absent-row fallback rather than its empty stat-book name.
    NATIVE_SKILL_CATALOG
        .get(skill_id)
        .filter(|_| skill_id != 82)
        .map_or_else(|| format!("Skill {skill_id}"), |skill| skill.name.to_owned())
}
