//! Weld's phase glue in the shared primary transient queue. Native world epochs
//! precede new casts; impact flashes and debris are appended after their owner.

use super::*;
use crate::float::fround;
use crate::painter::{WorldManagerLane, WorldPainterOrder};
use crate::spells::model::{
    PrimaryProjectilePartsMut, PrimarySpellProjectilePhase, PrimarySpellTransient, SpellState,
};
use crate::spells::simulation::{PrimarySpellEpochContext, PrimarySpellTraversalActor};
use crate::spells::targeting::NATIVE_PRIMARY_FLIGHT_TERRAIN_EXCLUSION_MASK;
use crate::spells::transient_storage::{WeldPersistentRef, WeldWorldMut, WeldWorldRef};
use crate::{NativeRng, Vec2};

pub fn enroll_native_weld_actor_painter(
    actor: &mut NativeWeldActorBase,
    order: &mut WorldPainterOrder,
) {
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
        spell: &mut PrimaryProjectilePartsMut<'_>,
        targets: &[crate::spells::targeting::PrimarySpellTarget],
        tick: u64,
        rng: &mut NativeRng,
    ) {
        crate::spells::projectiles::advance_projectile(spell, targets);
        let PrimaryProjectilePartsMut::Weld(weld) = spell else { return };
        step_native_weld_projectile_presentation(weld, rng);
        if weld.build_id == 1009 {
            let fades = create_native_weld_ground_spark_fade_program(weld.view(), rng);
            for seed in fades {
                let actor = create_native_weld_ground_spark_fade_actor(
                    NativeWeldActorBirth {
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
                self.transients.push(PrimarySpellTransient::Weld(
                    NativeWeldWorldActor::GroundSparkFade(actor),
                ));
                self.next_id += 1;
            }
        }
    }

    pub(crate) fn advance_weld_world_actor(
        &mut self,
        actor: &mut WeldWorldMut<'_>,
        context: &PrimarySpellEpochContext<'_>,
        rng: &mut NativeRng,
        order: &mut WorldPainterOrder,
    ) -> bool {
        if let WeldWorldRef::Persistent(WeldPersistentRef::Hailstones(hail)) = actor.view()
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
                NATIVE_PRIMARY_FLIGHT_TERRAIN_EXCLUSION_MASK,
            )
            .is_some()
            {
                let children = create_native_weld_hail_terrain_impact(
                    hail,
                    true,
                    self.next_id,
                    context.tick,
                    rng,
                );
                self.next_id += children.len() as u64;
                self.transients.extend(children.into_iter().map(|child| {
                    PrimarySpellTransient::Weld(NativeWeldWorldActor::HailChild(child))
                }));
                return false;
            }
        }
        let already_impacting =
            matches!(actor.view(), WeldWorldRef::Meteor(meteor) if meteor.impact_due);
        let can_advance = |persistent: WeldPersistentRef<'_>, from, to| {
            let WeldPersistentRef::EtherealBoulder(boulder) = persistent else {
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
        let step = step_native_weld_world_actor(actor, rng, &can_advance);
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
        if let WeldWorldRef::Meteor(meteor) = actor.view()
            && meteor.impact_due
            && !already_impacting
        {
            children.push(NativeWeldWorldActor::MeteorFlash(create_native_weld_meteor_flash(
                meteor,
                self.next_id,
                context.tick,
            )));
            self.next_id += 1;
            for debris in step.debris {
                children.push(NativeWeldWorldActor::BoulderDebris(
                    create_native_weld_boulder_debris_actor(
                        NativeWeldActorBirth {
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
                    ),
                ));
                self.next_id += 1;
            }
        }
        self.transients.extend(children.into_iter().map(PrimarySpellTransient::Weld));
        true
    }

    /// Source enrollment runs after every actor epoch and new cast. A logical
    /// actor-lane light may share its painter; transient-lane lights do not.
    pub(crate) fn enroll_weld_painters(&mut self, order: &mut WorldPainterOrder) {
        for mut effect in self.transients.iter_mut() {
            let crate::spells::model::PrimaryTransientMut::Weld(actor) = &mut effect else {
                continue;
            };
            enroll_native_weld_actor_painter(actor.common_mut(), order);
        }
    }
}
