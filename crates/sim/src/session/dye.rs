//! Live inventory painting lifetime from `core-server/game-simulation.ts`.
//! The initiating kit is consumed by the canonical economy transaction only once.

use super::{GameSession, GameSessionInput};
use crate::Vec2;
use crate::combat::PlayerLifeState;
use crate::economy::NativeInventoryDyeSession;
use std::collections::BTreeMap;

impl GameSession {
    #[must_use]
    pub fn inventory_dye_session(&self, id: &str) -> Option<&NativeInventoryDyeSession> {
        self.inventory_dye_sessions.get(id)
    }

    /// A stale Done/hidden-panel action cannot close a newer accepted painting.
    pub fn close_inventory_dye_session(&mut self, id: &str, session_id: Option<&str>) -> bool {
        let Some(current) = self.inventory_dye_sessions.get(id) else { return false };
        if session_id.is_some_and(|session_id| current.id != session_id) {
            return false;
        }
        self.inventory_dye_sessions.remove(id);
        true
    }

    pub(super) fn close_dye_sessions_for_inputs(
        &mut self,
        inputs: &BTreeMap<String, GameSessionInput>,
    ) {
        self.inventory_dye_sessions.retain(|id, _| {
            inputs.get(id).is_none_or(|input| {
                input.movement == Vec2::ZERO && !input.primary && input.quickbar.is_none()
            })
        });
    }

    pub(super) fn close_inactive_dye_sessions(&mut self) {
        self.inventory_dye_sessions.retain(|id, _| {
            self.participants.get(id).is_some_and(|participant| {
                participant.player.combat.life_state == PlayerLifeState::Alive
            })
        });
    }
}
