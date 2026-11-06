//! Native per-tick effect retirement. Called rocks wait until the owner's boulder
//! has been updated; Fire retirements can enroll new actors during this pass.

use super::earth_debris::step_weld_boulder_debris_particle;
use super::effects::{fire_explosion, fire_impact_at, transient_lifetime};
use super::fire::*;
use super::model::*;
use super::simulation::PrimarySpellEpochContext;
use crate::Rng;
use crate::float::fround;
use crate::harden_effects::step_harden_effect;
use crate::painter::{WorldManagerLane, WorldPainterOrder};

impl SpellState {
    pub(super) fn advance_transients(
        &mut self,
        context: &PrimarySpellEpochContext<'_>,
        rng: &mut Rng,
        order: &mut WorldPainterOrder,
        result: &mut PrimarySpellTickResult,
    ) {
        let mut population = std::mem::take(&mut self.transients);
        self.transients.collect_births();
        population.advance(|effect| {
            let retained = self.advance_transient(effect, context, rng, order, result);
            (retained, self.transients.take_births())
        });
        self.transients = population;
    }

    fn advance_transient(
        &mut self,
        effect: &mut PrimarySpellTransient,
        context: &PrimarySpellEpochContext<'_>,
        rng: &mut Rng,
        order: &mut WorldPainterOrder,
        result: &mut PrimarySpellTickResult,
    ) -> bool {
        match effect {
            PrimarySpellTransient::Harden(actor) => {
                let owner_id = actor.common().owner_id.clone();
                step_harden_effect(actor, context.tick, rng, |position| {
                    (context.can_place)(&owner_id, position, 0.0)
                })
            }
            // Staff has already advanced in its contact host this tick.
            PrimarySpellTransient::Staff(_) => true,
            PrimarySpellTransient::Weld(actor) => {
                self.advance_weld_world_actor(actor, context, rng, order)
            }
            PrimarySpellTransient::EarthBoulderBit(effect) => {
                if let Some(debris) = step_weld_boulder_debris_particle(
                    effect.debris,
                    effect.birth_tick + effect.common.age_ticks + 1,
                    rng,
                ) {
                    effect.common.age_ticks += 1;
                    effect.debris = debris;
                    true
                } else {
                    false
                }
            }
            PrimarySpellTransient::EarthCalledRock(_)
            | PrimarySpellTransient::AirHurricane(_)
            | PrimarySpellTransient::WaterHail(_) => true,
            PrimarySpellTransient::FireGoodImp(actor) => {
                let owner_id = actor.common.owner_id.clone();
                let collision_radius = actor.collision_radius;
                let can_occupy =
                    |position| (context.can_place)(&owner_id, position, collision_radius);
                let step = step_fire_good_imp(
                    actor,
                    FireGoodImpStepContext {
                        can_occupy: &can_occupy,
                        navigation: context.navigation,
                        targets: (context.targets_for_owner)(&owner_id),
                    },
                    rng,
                );
                if let Some(contact) = step.contact {
                    result.fire_actor_contacts.push(FireActorContact::GoodImp(contact));
                }
                if !step.retained {
                    self.spawn_fire_patch(
                        actor.burn_damage,
                        actor.damage,
                        &actor.common.owner_id,
                        actor.position,
                        &actor.common.world_key,
                        rng,
                        order,
                    );
                }
                step.retained
            }

            PrimarySpellTransient::FirePatch(patch) => {
                let (retained, contact) = step_fire_patch(patch, context.tick);
                if let Some(contact) = contact {
                    result.fire_actor_contacts.push(FireActorContact::Patch(contact));
                }
                retained
            }
            PrimarySpellTransient::FireEmber(ember) => {
                let step = step_fire_ember(ember);
                if step.retained {
                    if (context.can_place)(&ember.common.owner_id, ember.position, 7.0) {
                        return true;
                    }
                    self.transients.push(PrimarySpellTransient::FireImpact(fire_impact_at(
                        self.next_id,
                        ember.position,
                        &ember.common.owner_id,
                        &ember.common.world_key,
                        order,
                    )));
                    self.next_id += 1;
                } else {
                    match step.retirement {
                        FireEmberRetirement::None => {}
                        FireEmberRetirement::Immolate(explosion) => {
                            let sound_pitch = fround(1.0 + rng.draw_scaled_float(0.1, true));
                            self.transients.push(PrimarySpellTransient::FireExplosion(
                                fire_explosion(self.next_id, explosion, sound_pitch, order),
                            ));
                            self.next_id += 1;
                        }
                        FireEmberRetirement::Imp {
                            burn_damage,
                            damage,
                            lifetime_ticks,
                            owner_id,
                            position,
                            world_key,
                        } => {
                            let mut actor = spawn_fire_good_imp(
                                SpawnFireGoodImpOptions {
                                    burn_damage,
                                    damage,
                                    id: self.next_id,
                                    lifetime_ticks,
                                    owner_id: owner_id.clone(),
                                    position,
                                    world_key: world_key.clone(),
                                },
                                rng,
                            );
                            let registration = order.register(WorldManagerLane::Actor);
                            actor.common.light_registration = Some(registration);
                            actor.common.painter_registrations = vec![registration];
                            self.transients.push(PrimarySpellTransient::FireGoodImp(actor));
                            self.next_id += 1;
                            self.spawn_fire_patch(
                                burn_damage,
                                damage,
                                &owner_id,
                                position,
                                &world_key,
                                rng,
                                order,
                            );
                        }
                    }
                }
                false
            }
            _ => {
                if effect.common().age_ticks + 1 < transient_lifetime(effect) {
                    effect.common_mut().age_ticks += 1;
                    true
                } else {
                    false
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn spawn_fire_patch(
        &mut self,
        burn_damage: f64,
        damage: f64,
        owner_id: &str,
        position: crate::Vec2,
        world_key: &str,
        rng: &mut Rng,
        order: &mut WorldPainterOrder,
    ) {
        let patch = spawn_native_fire_patch(
            CreateFirePatchOptions {
                atlas_phase_step: None,
                burn_damage,
                damage,
                draw_alpha: None,
                fade_alpha: None,
                id: self.next_id,
                life: None,
                patch_type: FirePatchType::Fire,
                owner_id: owner_id.into(),
                painter_registration: Some(order.register(WorldManagerLane::Actor)),
                position,
                scale: None,
                supplemental_contact: false,
                velocity: None,
                velocity_multiplier: None,
                world_key: world_key.into(),
            },
            rng,
        );
        self.transients.push(PrimarySpellTransient::FirePatch(patch));
        self.next_id += 1;
    }
}
