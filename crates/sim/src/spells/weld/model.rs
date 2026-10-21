//! Ownership shared by the source Weld world actors in `native-weld-primary-runtime.ts`.

use crate::Vec2;
use crate::painter::{PainterRegistration, WorldManagerLane, WorldPainterOrder};
use crate::spells::model::PrimarySpellTransientBase;

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NativeWeldActorBase {
    /// Weld actors live in the same ordered transient queue as ordinary effects.
    /// Reuse its owner so ticking, painter enrollment and contact IDs stay shared.
    pub owner: PrimarySpellTransientBase,
    pub birth_tick: u64,
    pub build_id: u32,
    pub direction: Vec2,
    pub origin: Vec2,
    pub vector: Vec<f64>,
}

/// The source's common actor constructor input. Family constructors decide which
/// manager registrations exist; a plain effect has no logical light provider.
#[derive(Clone, Copy)]
pub struct NativeWeldActorBirth<'a> {
    pub build_id: u32,
    pub direction: Vec2,
    pub id: u64,
    pub origin: Vec2,
    pub owner_id: &'a str,
    pub tick: u64,
    pub vector: &'a [f64],
    pub world_key: &'a str,
}

impl NativeWeldActorBirth<'_> {
    #[must_use]
    pub fn actor(self) -> NativeWeldActorBase {
        NativeWeldActorBase {
            owner: PrimarySpellTransientBase {
                age_ticks: 0,
                id: self.id,
                light_registration: None,
                owner_id: self.owner_id.into(),
                painter_registrations: Vec::new(),
                world_key: self.world_key.into(),
            },
            birth_tick: self.tick,
            build_id: self.build_id,
            direction: self.direction,
            origin: self.origin,
            vector: self.vector.to_vec(),
        }
    }

    #[must_use]
    pub fn actor_with_light(self, painters: Option<&mut WorldPainterOrder>) -> NativeWeldActorBase {
        let registration = if let Some(order) = painters {
            order.register(WorldManagerLane::Actor)
        } else {
            PainterRegistration {
                manager_lane: WorldManagerLane::Actor,
                registration_ordinal: self.id as usize,
            }
        };
        let mut actor = self.actor();
        actor.owner.light_registration = Some(registration);
        actor
    }
}
