//! The session's deferred projectile producer from `native-world-manager-order.ts`.
//!
//! The Website returns an unresolved registration object whose ordinal getter throws
//! until commit. Rust keeps its unresolved identities private to the enemy epoch and
//! replaces every copied reference when the session commits the one shared allocator.

use super::world::EnemyWorld;
use crate::painter::{PainterRegistration, WorldManagerLane, WorldPainterOrder};

#[derive(Clone, Debug)]
pub(super) struct DeferredEnemyPainterRegistrations {
    pub lanes: Vec<WorldManagerLane>,
}

impl EnemyWorld {
    pub fn begin_deferred_projectile_painters(&mut self) {
        self.assert_painter_registrations_committed();
        self.deferred_projectile_painters =
            Some(DeferredEnemyPainterRegistrations { lanes: Vec::new() });
    }

    #[must_use]
    pub fn painter_registrations_committed(&self) -> bool {
        self.deferred_projectile_painters.is_none()
    }

    /// Checkpoints and rendering belong after the session's projectile commit phase.
    pub fn assert_painter_registrations_committed(&self) {
        assert!(
            self.painter_registrations_committed(),
            "enemy projectile painter registrations have not been committed"
        );
    }

    /// Matches the source registration object's ordinal getter. Queries may inspect
    /// older committed rows during a deferred epoch, but cannot read a new birth.
    #[must_use]
    pub fn painter_registration_ordinal(&self, registration: PainterRegistration) -> usize {
        if let Some(pending) = &self.deferred_projectile_painters {
            assert!(
                usize::MAX - registration.registration_ordinal >= pending.lanes.len(),
                "enemy projectile painter registration has not been committed"
            );
        }
        registration.registration_ordinal
    }

    pub(super) fn register_projectile_painter(
        &mut self,
        order: &mut WorldPainterOrder,
        lane: WorldManagerLane,
    ) -> PainterRegistration {
        let Some(pending) = &mut self.deferred_projectile_painters else {
            return order.register(lane);
        };
        // These descending tokens are unresolved identities, never active ordinals.
        // No world-manager cursor advances before the session's commit boundary.
        let token = usize::MAX - pending.lanes.len();
        pending.lanes.push(lane);
        PainterRegistration { manager_lane: lane, registration_ordinal: token }
    }

    pub fn commit_deferred_projectile_painters(&mut self, order: &mut WorldPainterOrder) {
        let pending = self
            .deferred_projectile_painters
            .take()
            .expect("enemy projectile painter registrations are already committed");
        let mut committed = Vec::with_capacity(pending.lanes.len());
        for lane in &pending.lanes {
            let registration = order.register(*lane);
            assert!(
                registration.registration_ordinal < usize::MAX - pending.lanes.len(),
                "world painter ordinal reached the unresolved registration range"
            );
            committed.push(registration);
        }
        let resolve = |registration: &mut PainterRegistration| {
            let index = usize::MAX - registration.registration_ordinal;
            if let Some(resolved) = committed.get(index) {
                assert_eq!(registration.manager_lane, pending.lanes[index]);
                *registration = *resolved;
            }
        };
        for projectile in &mut self.projectiles {
            let state = projectile.state_mut();
            resolve(&mut state.painter_registration);
            if let Some(registration) = &mut state.light_registration {
                resolve(registration);
            }
        }
        for effect in &mut self.projectile_effects {
            resolve(&mut effect.state.painter_registration);
            if let Some(registration) = &mut effect.state.light_registration {
                resolve(registration);
            }
            if let super::projectiles::EnemyProjectileEffectKind::DemonFire { fire } =
                &mut effect.kind
            {
                for registration in &mut fire.common.painter_registrations {
                    resolve(registration);
                }
                if let Some(registration) = &mut fire.common.light_registration {
                    resolve(registration);
                }
            }
        }
        for silk in &mut self.silks {
            resolve(&mut silk.painter_registration);
        }
        for event in &mut self.events {
            if let Some(registration) = &mut event.painter_registration {
                resolve(registration);
            }
        }
        // Effect constructors can retain the same registration as their light. Patch
        // every retained registration-bearing row, rather than relying on the current
        // set of projectile producers never copying into another presentation lane.
        for effect in self.death_effects.iter_mut() {
            if let Some(registration) = &mut effect.painter_registration {
                resolve(registration);
            }
        }
        for pulse in &mut self.mage_lightning_pulses {
            resolve(&mut pulse.light_registration);
            for registration in &mut pulse.painter_registrations {
                resolve(registration);
            }
        }
        for spell in &mut self.boss_spells {
            resolve(&mut spell.owner.painter_registration);
            if let super::boss_spell::BossSpellKind::GreenFire { fire, .. } = &mut spell.kind {
                for registration in &mut fire.common.painter_registrations {
                    resolve(registration);
                }
                if let Some(registration) = &mut fire.common.light_registration {
                    resolve(registration);
                }
            }
        }
    }
}
