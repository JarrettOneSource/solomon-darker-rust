//! Current Source `spell-combat/special-death.ts`. Only the admitted newborn's
//! receiver contacts run here, before the caller resumes its native factory.
use super::*;
use crate::spells::fire::{
    NativeFireProjectilePayload, begin_native_fire_detonation, native_fire_ember_contact,
    step_native_fire_ember,
};
use crate::spells::special_death::{NativeSpellSpecialDeathKind, NativeSpellSpecialDeathPayload};

impl ContactWork<'_, '_> {
    pub(super) fn resolve_special_death_payload(
        &mut self,
        payload: NativeSpellSpecialDeathPayload,
        victim_id: usize,
        origin: Vec2,
        owner_id: &str,
    ) {
        if self.enemies.actors.iter().any(|actor| {
            actor.id == victim_id
                && matches!(actor.brain, crate::enemies::model::EnemyBrain::Cocoon(_))
        }) {
            return;
        }
        if payload.kind == NativeSpellSpecialDeathKind::Steam {
            if payload.explode_radius <= 0.0 {
                return;
            }
            let explosion_id = self.next_id;
            let detonation = create_native_weld_steam_detonation(
                NativeWeldSteamDetonationInput {
                    explode_damage: payload.explode_damage,
                    explode_radius: payload.explode_radius,
                    first_fragment_id: explosion_id + 1,
                    fragment_count: payload.ember_fragments,
                    fragment_damage: payload.ember_damage,
                    origin,
                    owner_id,
                    private_seed: payload.private_seed,
                    tick: self.context.tick,
                    world_key: self.context.world_key,
                },
                self.rng,
            );
            if let Some(explosion) = detonation.explosion {
                let actor = fire_explosion(
                    explosion_id,
                    explosion.explosion,
                    explosion.sound_pitch,
                    self.order,
                );
                self.owned_transients.push(PrimarySpellTransient::FireExplosion(actor.clone()));
                self.resolve_fire_explosion(&actor);
            }
            self.next_id = detonation.next_id;
            for fragment in detonation.fragments {
                let actor = self.enroll_weld_combat_actor(NativeWeldWorldActor::Steam(fragment));
                self.owned_transients.push(actor);
            }
            return;
        }
        if payload.explode_damage <= 0.0 {
            return;
        }
        let explosion_id = self.next_id;
        let first_ember_id = explosion_id + u64::from(payload.explode_radius > 0.0);
        // Contact impacts can allocate IDs inside an Ember's ten pre-ticks.
        self.next_id = first_ember_id + u64::from(payload.ember_fragments);
        let mut birth = begin_native_fire_detonation(
            first_ember_id,
            NativeFireProjectilePayload {
                burn_damage: 0.0,
                ember_damage: payload.ember_damage,
                ember_fragments: payload.ember_fragments,
                explode_damage: payload.explode_damage,
                explode_radius: payload.explode_radius,
                private_seed: payload.private_seed as i32,
                spent_ember: crate::profile::primary::NativeSpentEmber::None,
            },
            origin,
            owner_id,
            self.context.world_key,
            self.rng,
        );
        if let Some(explosion) = birth.explosion {
            let actor = fire_explosion(explosion_id, explosion, birth.sound_pitch, self.order);
            self.owned_transients.push(PrimarySpellTransient::FireExplosion(actor.clone()));
            self.resolve_fire_explosion(&actor);
        }
        while let Some(mut ember) = birth.fan.next_ember(self.rng) {
            let registration = self.order.register(WorldManagerLane::Actor);
            ember.common.light_registration = Some(registration);
            ember.common.painter_registrations.push(registration);
            let mut consumed = false;
            for _ in 0..10 {
                ember = step_native_fire_ember(ember)
                    .ember
                    .expect("newborn Ember survives its ten native pre-ticks");
                if ember.contact_due {
                    self.resolve_fire_actor_contacts(vec![FireActorContact::Ember(
                        native_fire_ember_contact(&ember),
                    )]);
                    consumed = self.consumed_transients.contains(&ember.common.id);
                    if consumed {
                        break;
                    }
                }
            }
            if !consumed {
                ember.contact_due = false;
                self.owned_transients.push(PrimarySpellTransient::FireEmber(ember));
            }
        }
    }
}
