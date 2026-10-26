//! Native per-tick effect retirement. Called rocks wait until the owner's boulder
//! has been updated; Fire retirements can enroll new actors during this pass.

use super::earth_debris::step_native_weld_boulder_debris_particle;
use super::effects::{fire_explosion, fire_impact_at, transient_lifetime};
use super::fire::*;
use super::model::*;
use super::simulation::PrimarySpellEpochContext;
use crate::NativeRng;
use crate::float::fround;
use crate::harden_effects::step_native_harden_effect;
use crate::painter::{WorldManagerLane, WorldPainterOrder};

impl SpellState {
    pub(super) fn advance_transients(
        &mut self,
        context: &PrimarySpellEpochContext<'_>,
        rng: &mut NativeRng,
        order: &mut WorldPainterOrder,
        result: &mut PrimarySpellTickResult,
    ) {
        for mut effect in std::mem::take(&mut self.transients) {
            match effect {
                PrimarySpellTransient::Harden(mut actor) => {
                    let owner_id = actor.common().owner_id.clone();
                    if step_native_harden_effect(&mut actor, context.tick, rng, |position| {
                        (context.can_place)(&owner_id, position, 0.0)
                    }) {
                        self.transients.push(PrimarySpellTransient::Harden(actor));
                    }
                }
                // Staff is stepped by its contact host before the primary epoch. It still
                // shares this ordered queue and must not advance again here.
                PrimarySpellTransient::Staff(_) => self.transients.push(effect),
                PrimarySpellTransient::Weld(actor) => {
                    self.advance_weld_world_actor(actor, context, rng, order)
                }
                PrimarySpellTransient::EarthBoulderBit(mut effect) => {
                    if let Some(debris) = step_native_weld_boulder_debris_particle(
                        effect.debris,
                        effect.birth_tick + effect.common.age_ticks + 1,
                        rng,
                    ) {
                        effect.common.age_ticks += 1;
                        effect.debris = debris;
                        self.transients.push(PrimarySpellTransient::EarthBoulderBit(effect));
                    }
                }
                PrimarySpellTransient::EarthCalledRock(_)
                | PrimarySpellTransient::AirHurricane(_)
                | PrimarySpellTransient::WaterHail(_) => self.transients.push(effect),
                PrimarySpellTransient::FireGoodImp(actor) => {
                    let can_occupy = |position| {
                        (context.can_place)(
                            &actor.common.owner_id,
                            position,
                            actor.collision_radius,
                        )
                    };
                    let step = step_native_fire_good_imp(
                        actor.clone(),
                        NativeFireGoodImpStepContext {
                            can_occupy: &can_occupy,
                            navigation: context.navigation,
                            targets: (context.targets_for_owner)(&actor.common.owner_id),
                        },
                        rng,
                    );
                    if let Some(contact) = step.contact {
                        result.fire_actor_contacts.push(FireActorContact::GoodImp(contact));
                    }
                    if let Some(actor) = step.good_imp {
                        self.transients.push(PrimarySpellTransient::FireGoodImp(actor));
                    } else if step.release_fire {
                        self.spawn_fire_patch(
                            actor.burn_damage,
                            actor.damage,
                            &actor.common.owner_id,
                            step.release_position,
                            &actor.common.world_key,
                            rng,
                            order,
                        );
                    }
                }
                PrimarySpellTransient::FirePatch(mut patch) => {
                    let (retained, contact) = step_native_fire_patch(&mut patch, context.tick);
                    if let Some(contact) = contact {
                        result.fire_actor_contacts.push(FireActorContact::Patch(contact));
                    }
                    if retained {
                        self.transients.push(PrimarySpellTransient::FirePatch(patch));
                    }
                }
                PrimarySpellTransient::FireEmber(ember) => {
                    let step = step_native_fire_ember(ember);
                    if let Some(ember) = step.ember {
                        if (context.can_place)(&ember.common.owner_id, ember.position, 7.0) {
                            self.transients.push(PrimarySpellTransient::FireEmber(ember));
                        } else {
                            self.transients.push(PrimarySpellTransient::FireImpact(
                                fire_impact_at(
                                    self.next_id,
                                    ember.position,
                                    &ember.common.owner_id,
                                    &ember.common.world_key,
                                    order,
                                ),
                            ));
                            self.next_id += 1;
                        }
                    } else {
                        match step.retirement {
                            NativeFireEmberRetirement::None => {}
                            NativeFireEmberRetirement::Immolate(explosion) => {
                                let sound_pitch = fround(1.0 + rng.draw_scaled_float(0.1, true));
                                self.transients.push(PrimarySpellTransient::FireExplosion(
                                    fire_explosion(self.next_id, explosion, sound_pitch, order),
                                ));
                                self.next_id += 1;
                            }
                            NativeFireEmberRetirement::Imp {
                                burn_damage,
                                damage,
                                lifetime_ticks,
                                owner_id,
                                position,
                                world_key,
                            } => {
                                let mut actor = spawn_native_fire_good_imp(
                                    SpawnNativeFireGoodImpOptions {
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
                }
                _ => {
                    if effect.common().age_ticks + 1 < transient_lifetime(&effect) {
                        effect.common_mut().age_ticks += 1;
                        self.transients.push(effect);
                    }
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
        rng: &mut NativeRng,
        order: &mut WorldPainterOrder,
    ) {
        let patch = spawn_native_fire_patch(
            CreateNativeFirePatchOptions {
                atlas_phase_step: None,
                burn_damage,
                damage,
                draw_alpha: None,
                fade_alpha: None,
                id: self.next_id,
                life: None,
                native_type: FirePatchType::Fire,
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
