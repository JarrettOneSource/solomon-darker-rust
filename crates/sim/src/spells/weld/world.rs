//! Weld's phase glue in the shared primary transient queue. Native world epochs
//! precede new casts; impact flashes and debris are appended after their owner.

use super::*;
use crate::float::fround;
use crate::painter::{WorldManagerLane, WorldPainterOrder};
use crate::spells::model::{
    PrimarySpellProjectile, PrimarySpellProjectilePhase, PrimarySpellTransient, SpellState,
};
use crate::spells::simulation::{PrimarySpellEpochContext, PrimarySpellTraversalActor};
use crate::spells::targeting::PRIMARY_FLIGHT_TERRAIN_EXCLUSION_MASK;
use crate::spells::weld::{WeldPersistentActorState, WeldWorldActor};
use crate::{Rng, Vec2};

pub fn enroll_weld_actor_painter(actor: &mut WeldActorBase, order: &mut WorldPainterOrder) {
    let owner = &mut actor.owner;
    if owner.painter_registrations.is_empty() {
        let registration = owner
            .light_registration
            .filter(|registration| registration.manager_lane == WorldManagerLane::Actor)
            .unwrap_or_else(|| order.register(WorldManagerLane::Actor));
        owner.painter_registrations.push(registration);
    }
}

impl SpellState {
    pub(crate) fn advance_projectile_with_presentation(
        &mut self,
        spell: &mut PrimarySpellProjectile,
        targets: &[crate::spells::targeting::PrimarySpellTarget],
        tick: u64,
        rng: &mut Rng,
    ) {
        crate::spells::projectiles::advance_projectile(spell, targets);
        let PrimarySpellProjectile::Weld(weld) = spell else { return };
        step_weld_projectile_presentation(weld, rng);
        if weld.build_id == 1009 {
            let fades = create_weld_ground_spark_fade_program(weld, rng);
            for seed in fades {
                let actor = create_weld_ground_spark_fade_actor(
                    WeldActorBirth {
                        build_id: 1009,
                        direction: weld.common.direction,
                        id: self.next_id,
                        origin: seed.position,
                        owner_id: &weld.common.owner_id,
                        tick,
                        vector: &weld.vector,
                        world_key: &weld.common.world_key,
                    },
                    seed,
                );
                self.transients
                    .push(PrimarySpellTransient::Weld(WeldWorldActor::GroundSparkFade(actor)));
                self.next_id += 1;
            }
        }
    }

    pub(crate) fn advance_weld_world_actor(
        &mut self,
        actor: &mut WeldWorldActor,
        context: &PrimarySpellEpochContext<'_>,
        rng: &mut Rng,
        order: &mut WorldPainterOrder,
    ) -> bool {
        if let WeldWorldActor::Persistent(WeldPersistentActorState::Hailstones(hail)) = actor
            && hail.phase == PrimarySpellProjectilePhase::Flight
        {
            let origin = hail.common.origin;
            let direction = hail.common.direction;
            let lookahead = Vec2::new(
                fround(origin.x + direction.x * 30.0),
                fround(origin.y + direction.y * 30.0),
            );
            if (context.obstruction)(
                &hail.common.owner.owner_id,
                origin,
                lookahead,
                None,
                PRIMARY_FLIGHT_TERRAIN_EXCLUSION_MASK,
            )
            .is_some()
            {
                let children =
                    create_weld_hail_terrain_impact(hail, true, self.next_id, context.tick, rng);
                self.next_id += children.len() as u64;
                self.transients.extend(
                    children
                        .into_iter()
                        .map(|child| PrimarySpellTransient::Weld(WeldWorldActor::HailChild(child))),
                );
                return false;
            }
        }
        let already_impacting =
            matches!(actor, WeldWorldActor::Meteor(meteor) if meteor.impact_due);
        let can_advance = |persistent: &WeldPersistentActorState, from, to| {
            let WeldPersistentActorState::EtherealBoulder(boulder) = persistent else {
                return true;
            };
            (context.can_traverse)(
                PrimarySpellTraversalActor::EtherealBoulder(boulder),
                from,
                to,
                boulder.scale * 75.0,
                0,
            )
        };
        let step = step_weld_world_actor(actor, rng, &can_advance);
        if let Some(boulder) = step.terrain_contact {
            let children = create_primary_spell_weld_boulder_terminal(
                self.next_id,
                &boulder,
                context.tick,
                rng,
                order,
            );
            self.next_id += children.len() as u64;
            self.transients.extend(children.into_iter().map(PrimarySpellTransient::Weld));
            return false;
        }
        if !step.alive {
            return false;
        }
        let mut children = Vec::new();
        if let WeldWorldActor::Meteor(meteor) = actor
            && meteor.impact_due
            && !already_impacting
        {
            children.push(WeldWorldActor::MeteorFlash(create_weld_meteor_flash(
                meteor,
                self.next_id,
                context.tick,
            )));
            self.next_id += 1;
            for debris in step.debris {
                children.push(WeldWorldActor::BoulderDebris(create_weld_boulder_debris_actor(
                    WeldActorBirth {
                        build_id: 1007,
                        direction: meteor.common.direction,
                        id: self.next_id,
                        origin: meteor.position,
                        owner_id: &meteor.common.owner.owner_id,
                        tick: context.tick,
                        vector: &meteor.common.vector,
                        world_key: &meteor.common.owner.world_key,
                    },
                    debris,
                )));
                self.next_id += 1;
            }
        }
        self.transients.extend(children.into_iter().map(PrimarySpellTransient::Weld));
        true
    }

    /// Source enrollment runs after every actor epoch and new cast. A logical
    /// actor-lane light may share its painter; transient-lane lights do not.
    pub(crate) fn enroll_weld_painters(&mut self, order: &mut WorldPainterOrder) {
        for effect in self.transients.iter_mut() {
            let PrimarySpellTransient::Weld(actor) = effect else {
                continue;
            };
            enroll_weld_actor_painter(actor.common_mut(), order);
        }
    }
}
