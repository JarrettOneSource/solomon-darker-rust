//! The Hub's five regions and the doorways between them.
//!
//! The courtyard opens onto four private rooms. Touching a doorway's trigger starts a scripted
//! walk toward its target while the view fades out; the wizard then appears in the destination
//! and walks in while the view fades back. Reference:
//! `Website/frontend/src/game/core-kernels/hub-regions.ts`, with room sizes from
//! `hub-private-room-layout.ts`.
//!
//! [`HubRegion::segments`] supplies the courtyard and private-room collision tables.

use crate::collision::Vec2;
use crate::float::hypot;
use crate::hub::{
    HubSegment, HubWizardPlan, MOVEMENT_LANE_CAP, MOVEMENT_TICK_SECONDS, PLAYER_CHARACTER_RADIUS,
    circle_overlaps_segment,
};

/// A room of the Hub. The declaration order is the stock region index.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum HubRegion {
    Courtyard,
    Mortuary,
    Library,
    Storeroom,
    Office,
}

impl HubRegion {
    pub const ALL: [Self; 5] =
        [Self::Courtyard, Self::Mortuary, Self::Library, Self::Storeroom, Self::Office];

    /// The stock region index (`nativeId`).
    #[must_use]
    pub const fn native_id(self) -> u8 {
        self as u8
    }

    /// The Website's region id, as the protocol and fixtures spell it.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Courtyard => "courtyard",
            Self::Mortuary => "mortuary",
            Self::Library => "library",
            Self::Storeroom => "storeroom",
            Self::Office => "office",
        }
    }

    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|region| region.id() == id)
    }

    /// The room's size in world units.
    #[must_use]
    pub const fn size(self) -> Vec2 {
        match self {
            Self::Courtyard => Vec2::new(2000.0, 1024.0),
            Self::Mortuary | Self::Library | Self::Office => Vec2::new(1024.0, 1024.0),
            Self::Storeroom => Vec2::new(1075.0, 800.0),
        }
    }

    #[must_use]
    pub const fn is_private(self) -> bool {
        !matches!(self, Self::Courtyard)
    }
}

/// A one-way doorway. Touching `trigger` while in `source` starts a scripted walk toward
/// `scripted_target` at `scripted_speed` (a fraction of the lane cap) and a fade into
/// `destination`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HubPortal {
    pub source: HubRegion,
    pub destination: HubRegion,
    pub trigger: HubSegment,
    pub scripted_speed: f64,
    pub scripted_target: Vec2,
    /// The mortuary's return walk keeps the x at which the wizard touched the trigger.
    pub preserve_contact_x: bool,
}

const fn courtyard_exit(
    destination: HubRegion,
    scripted_speed: f64,
    scripted_target: Vec2,
    trigger: HubSegment,
) -> HubPortal {
    HubPortal {
        source: HubRegion::Courtyard,
        destination,
        trigger,
        scripted_speed,
        scripted_target,
        preserve_contact_x: false,
    }
}

/// A private room's way back: a horizontal trigger near its south wall and a walk off the
/// bottom edge. The mortuary's is wider and nearer the wall.
const fn private_return(region: HubRegion) -> HubPortal {
    let size = region.size();
    let center_x = size.x / 2.0;
    let mortuary = matches!(region, HubRegion::Mortuary);
    let reach = if mortuary { 1000.0 } else { 100.0 };
    let trigger_y = size.y - if mortuary { 60.0 } else { 100.0 };
    HubPortal {
        source: region,
        destination: HubRegion::Courtyard,
        trigger: HubSegment::new(center_x - reach, trigger_y, center_x + reach, trigger_y),
        scripted_speed: 1.0,
        scripted_target: Vec2::new(center_x, size.y + 1000.0),
        preserve_contact_x: mortuary,
    }
}

/// Every doorway, courtyard exits first, in the reference's order; [`hub_portal_at`] takes
/// the first whose trigger the wizard touches.
pub const HUB_PORTALS: [HubPortal; 8] = [
    courtyard_exit(
        HubRegion::Mortuary,
        0.65,
        Vec2::new(32.0, 363.0),
        HubSegment::new(179.0, 394.0, 33.0, 529.0),
    ),
    courtyard_exit(
        HubRegion::Library,
        0.45,
        Vec2::new(2057.5, 460.5),
        HubSegment::new(1995.5, 606.5, 1915.5, 443.5),
    ),
    courtyard_exit(
        HubRegion::Storeroom,
        0.45,
        Vec2::new(627.5, -1000.0),
        HubSegment::new(679.5, 146.5, 576.5, 146.5),
    ),
    courtyard_exit(
        HubRegion::Office,
        0.45,
        Vec2::new(881.5, -1000.0),
        HubSegment::new(1024.5, 115.5, 881.5, 115.5),
    ),
    private_return(HubRegion::Mortuary),
    private_return(HubRegion::Library),
    private_return(HubRegion::Storeroom),
    private_return(HubRegion::Office),
];

/// Alpha change per tick while the College intro fades in.
pub const HUB_COLLEGE_INTRO_FADE_RATE: f64 = 0.01;
/// Alpha change per tick while leaving a region.
pub const HUB_OUTGOING_FADE_RATE: f64 = 0.01;

/// Alpha change per tick while arriving in `region`; the storeroom comes up faster.
#[must_use]
pub const fn hub_incoming_fade_rate(region: HubRegion) -> f64 {
    match region {
        HubRegion::Storeroom => 0.025,
        _ => 0.01,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum HubTransitionPhase {
    CollegeIntro,
    CollegeLoadout,
    Outgoing,
    Incoming,
}

/// A wizard part-way between regions (`HubParticipantTransition`).
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct HubTransition {
    pub alpha: f64,
    pub destination: HubRegion,
    pub phase: HubTransitionPhase,
    pub scripted_speed: f64,
    pub scripted_target: Vec2,
    pub source_region: HubRegion,
}

/// The doorway whose trigger a wizard of `radius` at `position` touches in `region`.
#[must_use]
pub fn hub_portal_at(region: HubRegion, position: Vec2, radius: f64) -> Option<&'static HubPortal> {
    HUB_PORTALS.iter().find(|portal| {
        portal.source == region && circle_overlaps_segment(position, radius, &portal.trigger)
    })
}

/// The outgoing transition `portal` starts for a wizard that touched its trigger at
/// `contact` (`beginHubTransition`).
#[must_use]
pub fn begin_hub_transition(portal: &HubPortal, contact: Vec2) -> HubTransition {
    let scripted_target = if portal.preserve_contact_x {
        Vec2::new(contact.x, portal.scripted_target.y)
    } else {
        portal.scripted_target
    };
    HubTransition {
        alpha: 0.0,
        destination: portal.destination,
        phase: HubTransitionPhase::Outgoing,
        scripted_speed: portal.scripted_speed,
        scripted_target,
        source_region: portal.source,
    }
}

/// Where a wizard appears after a doorway, and the walk it takes in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HubIncomingPlacement {
    pub position: Vec2,
    pub scripted_speed: f64,
    pub scripted_target: Vec2,
}

/// Courtyard re-entry points, just inside each doorway, with the walk away from it.
const fn courtyard_reentry(source: HubRegion) -> HubIncomingPlacement {
    let (position, scripted_target) = match source {
        HubRegion::Mortuary => (Vec2::new(63.0, 413.0), Vec2::new(123.0, 488.0)),
        HubRegion::Library => (Vec2::new(1990.5, 504.5), Vec2::new(1917.5, 563.5)),
        HubRegion::Storeroom => (Vec2::new(627.5, 98.5), Vec2::new(627.5, 198.5)),
        HubRegion::Office => (Vec2::new(952.5, 67.5), Vec2::new(952.5, 157.5)),
        HubRegion::Courtyard => panic!("the courtyard has no same-region entrance"),
    };
    HubIncomingPlacement { position, scripted_speed: 1.0, scripted_target }
}

/// `hubIncomingPlacement`: a private room is entered at the bottom centre, walking up; the
/// courtyard is re-entered beside the doorway that leads back to `source`.
///
/// # Panics
///
/// When both regions are the courtyard, which no doorway produces.
#[must_use]
pub fn hub_incoming_placement(source: HubRegion, destination: HubRegion) -> HubIncomingPlacement {
    if destination == HubRegion::Courtyard {
        return courtyard_reentry(source);
    }
    let size = destination.size();
    let center_x = size.x / 2.0;
    let mortuary = destination == HubRegion::Mortuary;
    let entry_offset = if mortuary { 70.0 } else { 100.0 };
    let target_offset = if mortuary { 120.0 } else { 150.0 };
    HubIncomingPlacement {
        position: Vec2::new(center_x, size.y - entry_offset),
        scripted_speed: 1.0,
        scripted_target: Vec2::new(center_x, size.y - target_offset),
    }
}

/// Displacement of one scripted tick toward `target`: the lane cap scaled by `speed`, never
/// overshooting.
#[must_use]
pub fn hub_scripted_delta(position: Vec2, target: Vec2, speed: f64) -> Vec2 {
    let dx = target.x - position.x;
    let dy = target.y - position.y;
    let distance = hypot(dx, dy);
    if distance == 0.0 {
        return Vec2::ZERO;
    }
    let step = distance.min(speed * MOVEMENT_LANE_CAP * MOVEMENT_TICK_SECONDS);
    Vec2::new(dx / distance * step, dy / distance * step)
}

/// A movement plan that walks the wizard toward `target` without input
/// (`planHubScriptedMovement`). Nothing is retained between ticks, so the walk stops dead.
#[must_use]
pub fn plan_hub_scripted_movement(position: Vec2, target: Vec2, speed: f64) -> HubWizardPlan {
    let delta = hub_scripted_delta(position, target, speed);
    HubWizardPlan {
        face: true,
        delta,
        movement_active: delta.x != 0.0 || delta.y != 0.0,
        requested_velocity: Vec2::new(
            delta.x / MOVEMENT_TICK_SECONDS,
            delta.y / MOVEMENT_TICK_SECONDS,
        ),
        retained_velocity: Vec2::ZERO,
    }
}

/// Whether a move from `source` to `destination` crosses a doorway: every private room
/// connects only to the courtyard.
#[must_use]
pub fn is_hub_transition_edge(source: HubRegion, destination: HubRegion) -> bool {
    if source == HubRegion::Courtyard {
        destination != HubRegion::Courtyard
    } else {
        destination == HubRegion::Courtyard
    }
}

/// [`hub_portal_at`] for a player-sized wizard.
#[must_use]
pub fn hub_portal_touched_by_player(
    region: HubRegion,
    position: Vec2,
) -> Option<&'static HubPortal> {
    hub_portal_at(region, position, PLAYER_CHARACTER_RADIUS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hub::HUB_SPAWN;

    #[test]
    fn regions_keep_the_stock_index_order_and_round_trip_their_ids() {
        for (index, region) in HubRegion::ALL.into_iter().enumerate() {
            assert_eq!(usize::from(region.native_id()), index);
            assert_eq!(HubRegion::from_id(region.id()), Some(region));
        }
        assert_eq!(HubRegion::from_id("boneyard"), None);
        assert!(!HubRegion::Courtyard.is_private() && HubRegion::Office.is_private());
    }

    #[test]
    fn the_spawn_touches_no_doorway_and_each_doorway_is_found_from_its_own_side() {
        assert_eq!(hub_portal_touched_by_player(HubRegion::Courtyard, HUB_SPAWN), None);
        let library = hub_portal_touched_by_player(HubRegion::Courtyard, Vec2::new(1955.5, 525.0))
            .expect("the library doorway");
        assert_eq!(library.destination, HubRegion::Library);
        assert_eq!(
            hub_portal_touched_by_player(HubRegion::Library, Vec2::new(1955.5, 525.0)),
            None
        );
        let back = hub_portal_touched_by_player(HubRegion::Library, Vec2::new(512.0, 930.0))
            .expect("the library's way back");
        assert_eq!(back.destination, HubRegion::Courtyard);
        assert!(!back.preserve_contact_x);
    }

    #[test]
    fn the_mortuary_return_keeps_the_contact_x_and_the_others_do_not() {
        let mortuary = &HUB_PORTALS[4];
        assert_eq!(mortuary.source, HubRegion::Mortuary);
        assert_eq!(mortuary.trigger, HubSegment::new(-488.0, 964.0, 1512.0, 964.0));
        let walk = begin_hub_transition(mortuary, Vec2::new(300.0, 950.0));
        assert_eq!(walk.scripted_target, Vec2::new(300.0, 2024.0));
        assert_eq!(walk.phase, HubTransitionPhase::Outgoing);
        assert_eq!(walk.alpha, 0.0);

        let office = &HUB_PORTALS[7];
        assert_eq!(office.trigger, HubSegment::new(412.0, 924.0, 612.0, 924.0));
        let walk = begin_hub_transition(office, Vec2::new(450.0, 910.0));
        assert_eq!(walk.scripted_target, Vec2::new(512.0, 2024.0));
    }

    #[test]
    fn arrivals_stand_at_the_bottom_centre_of_a_room_or_beside_the_courtyard_doorway() {
        let storeroom = hub_incoming_placement(HubRegion::Courtyard, HubRegion::Storeroom);
        assert_eq!(storeroom.position, Vec2::new(537.5, 700.0));
        assert_eq!(storeroom.scripted_target, Vec2::new(537.5, 650.0));
        let mortuary = hub_incoming_placement(HubRegion::Courtyard, HubRegion::Mortuary);
        assert_eq!(mortuary.position, Vec2::new(512.0, 954.0));
        assert_eq!(mortuary.scripted_target, Vec2::new(512.0, 904.0));
        let back = hub_incoming_placement(HubRegion::Office, HubRegion::Courtyard);
        assert_eq!(back.position, Vec2::new(952.5, 67.5));
        assert_eq!(back.scripted_target, Vec2::new(952.5, 157.5));
        assert_eq!(hub_incoming_fade_rate(HubRegion::Storeroom), 0.025);
        assert_eq!(hub_incoming_fade_rate(HubRegion::Courtyard), 0.01);
    }

    #[test]
    fn scripted_walks_are_capped_by_speed_and_stop_on_the_target() {
        let full = hub_scripted_delta(Vec2::ZERO, Vec2::new(0.0, 100.0), 1.0);
        assert_eq!(full, Vec2::new(0.0, 1.1875));
        let slow = hub_scripted_delta(Vec2::ZERO, Vec2::new(100.0, 0.0), 0.45);
        assert_eq!(slow.x, 0.45 * 1.1875);
        let last = hub_scripted_delta(Vec2::new(99.5, 0.0), Vec2::new(100.0, 0.0), 1.0);
        assert_eq!(last, Vec2::new(0.5, 0.0));
        let arrived = plan_hub_scripted_movement(Vec2::new(5.0, 5.0), Vec2::new(5.0, 5.0), 1.0);
        assert!(!arrived.movement_active);
        assert_eq!(arrived.requested_velocity, Vec2::ZERO);
        let walking = plan_hub_scripted_movement(Vec2::ZERO, Vec2::new(0.0, 100.0), 1.0);
        assert!(walking.movement_active);
        assert_eq!(walking.requested_velocity, Vec2::new(0.0, 118.75));
        assert_eq!(walking.retained_velocity, Vec2::ZERO);
    }

    #[test]
    fn only_moves_through_the_courtyard_are_doorway_edges() {
        assert!(is_hub_transition_edge(HubRegion::Courtyard, HubRegion::Library));
        assert!(is_hub_transition_edge(HubRegion::Library, HubRegion::Courtyard));
        assert!(!is_hub_transition_edge(HubRegion::Courtyard, HubRegion::Courtyard));
        assert!(!is_hub_transition_edge(HubRegion::Library, HubRegion::Office));
    }
}
