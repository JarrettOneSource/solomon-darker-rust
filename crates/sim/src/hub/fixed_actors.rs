//! Immovable Hub bodies shared by authority and local display prediction.
//! Retail `0x00525800 -> 0x00526520`: resistance 90, strength 0, the five Courtyard
//! NPCs and complete private-room collider tables.

use crate::Vec2;
use crate::actor_physics::ActorPhysicsBody;
use crate::hub::PLAYER_CHARACTER_RADIUS;
use crate::hub::regions::HubRegion;

/// An immovable Hub body (an NPC or prop): its ID, position and collision radius.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HubFixedActorCollider {
    pub id: &'static str,
    pub position: Vec2,
    pub radius: f64,
}

impl HubFixedActorCollider {
    #[must_use]
    pub const fn new(id: &'static str, position: Vec2, radius: f64) -> Self {
        Self { id, position, radius }
    }

    #[must_use]
    pub fn physics_body(self) -> ActorPhysicsBody {
        ActorPhysicsBody {
            collision_recipient: true,
            id: self.id.into(),
            position: self.position,
            delta: Vec2::ZERO,
            radius: self.radius,
            push_resistance: 90.0,
            push_strength: 0.0,
            driven: false,
            push_enabled: false,
        }
    }
}

const ANNALIST: HubFixedActorCollider =
    HubFixedActorCollider::new("annalist", Vec2::new(895.5, 455.5), 8.0);

const COURTYARD_FIXED_ACTORS: [HubFixedActorCollider; 5] = [
    HubFixedActorCollider::new("perk-witch", Vec2::new(1340.0, 280.0), 15.0),
    HubFixedActorCollider::new("potion-trader", Vec2::new(1397.0, 664.0), 30.0),
    ANNALIST,
    HubFixedActorCollider::new("items-trader", Vec2::new(1700.5, 449.5), 25.0),
    HubFixedActorCollider::new("teacher", Vec2::new(576.5, 710.5), 25.0),
];

/// Only participants whose College admission is pending collide with this body.
pub const HUB_STORY_OFFICE_POLISHER: HubFixedActorCollider =
    HubFixedActorCollider::new("story-office-polisher", Vec2::new(566.0, 735.0), 15.0);

impl HubRegion {
    /// Permanent bodies in native contact order. Conditional Office/Skorcha bodies
    /// follow these registrations and retain their separate admission/presence gates.
    pub fn fixed_actor_colliders(self) -> impl Iterator<Item = HubFixedActorCollider> {
        COURTYARD_FIXED_ACTORS
            .into_iter()
            .filter(move |_| self == Self::Courtyard)
            .chain(
                self.room_actors().iter().map(|actor| {
                    HubFixedActorCollider::new(actor.id, actor.position, actor.radius)
                }),
            )
            .chain(
                self.room_props()
                    .iter()
                    .map(|prop| HubFixedActorCollider::new(prop.id, prop.position, prop.radius)),
            )
    }
}

/// Reject new bench-pocket entry when scenery prevents the fixed NPC's separation.
#[must_use]
pub fn constrain_hub_bench_entry(region: HubRegion, previous: Vec2, candidate: Vec2) -> Vec2 {
    if region != HubRegion::Courtyard {
        return candidate;
    }
    // The existing bench end (856,465) and fountain corner (920,427) bound the
    // pocket. Older saves behind that edge must keep their ordinary east escape.
    let pocket_side = 64.0 * (previous.y - 465.0) + 38.0 * (previous.x - 856.0);
    if pocket_side < 0.0 {
        return candidate;
    }
    let radius = PLAYER_CHARACTER_RADIUS + ANNALIST.radius;
    let distance_squared = |point: Vec2| {
        let dx = point.x - ANNALIST.position.x;
        let dy = point.y - ANNALIST.position.y;
        dx * dx + dy * dy
    };
    if distance_squared(previous) >= radius * radius
        && distance_squared(candidate) < radius * radius
    {
        previous
    } else {
        candidate
    }
}
