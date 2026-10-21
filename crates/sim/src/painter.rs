//! World-manager registration order from `core-kernels/native-world-manager-order.ts`.
//!
//! The native world has separate actor and transient counters. Registration order survives
//! movement and drawing; region rows only decide when a registered root paints.

use crate::hub::painter::{PainterLane, PainterLayer};

pub mod overlays;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum WorldManagerLane {
    Actor,
    Transient,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct PainterRegistration {
    pub manager_lane: WorldManagerLane,
    pub registration_ordinal: usize,
}

impl PainterRegistration {
    #[must_use]
    pub fn layer(self, world_y: f64, sort_bias: f64) -> PainterLayer {
        let manager_lane = match self.manager_lane {
            WorldManagerLane::Actor => PainterLane::Actor,
            WorldManagerLane::Transient => PainterLane::Transient,
        };
        PainterLayer {
            visible: true,
            is_insertion: false,
            manager_lane,
            registration_ordinal: self.registration_ordinal,
            world_y,
            sort_bias,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct WorldPainterOrder {
    pub next_actor_ordinal: usize,
    pub next_transient_ordinal: usize,
}

impl WorldPainterOrder {
    pub fn register(&mut self, lane: WorldManagerLane) -> PainterRegistration {
        let next = match lane {
            WorldManagerLane::Actor => &mut self.next_actor_ordinal,
            WorldManagerLane::Transient => &mut self.next_transient_ordinal,
        };
        let registration = PainterRegistration { manager_lane: lane, registration_ordinal: *next };
        *next += 1;
        registration
    }

    pub fn register_many(
        &mut self,
        lane: WorldManagerLane,
        count: usize,
    ) -> Vec<PainterRegistration> {
        let mut registrations = Vec::with_capacity(count);
        for _ in 0..count {
            registrations.push(self.register(lane));
        }
        registrations
    }
}
