//! Hurricane's early orbit/contact phase from `spell-combat/forces.ts`.
use crate::spells::transient_storage::PrimaryTransientRef;

use super::*;
use crate::hurricane::{
    NATIVE_HURRICANE_CONTACT_COOLDOWN, NATIVE_HURRICANE_DEFAULT_MOVEMENT_STEP,
    draw_native_hurricane_damage, native_hurricane_movement_due, native_hurricane_orbit_force,
};

impl ContactWork<'_, '_> {
    pub(super) fn resolve_hurricane_contacts(&mut self) {
        let mut hurricanes: Vec<_> = self
            .source
            .transients
            .iter()
            .filter_map(|effect| {
                let PrimaryTransientRef::AirHurricane(actor) = effect else {
                    return None;
                };
                (actor.common.world_key == self.context.world_key && actor.contact_charge > 0.0)
                    .then_some(actor.clone())
            })
            .collect();
        hurricanes.sort_by_key(|actor| actor.common.id);
        if hurricanes.is_empty() {
            return;
        }
        for row in target_rows(self.enemies) {
            if !row.target.active
                || !native_hurricane_movement_due(
                    row.actor_id as u64,
                    self.context.tick,
                    NATIVE_HURRICANE_DEFAULT_MOVEMENT_STEP,
                )
            {
                continue;
            }
            let cooldown = self
                .enemies
                .actors
                .iter()
                .find(|actor| actor.id == row.actor_id)
                .map(|actor| actor.hurricane_contact_cooldown)
                .or_else(|| {
                    self.enemies
                        .maggots
                        .iter()
                        .find(|actor| actor.id == row.actor_id)
                        .map(|actor| actor.hurricane_contact_cooldown)
                })
                .expect("Hurricane target belongs to enemy world");
            let mut delta = Vec2::ZERO;
            let mut contact = None;
            for hurricane in &hurricanes {
                let Some(force) = native_hurricane_orbit_force(
                    hurricane.position,
                    row.target.position,
                    hurricane.contact_charge,
                    NATIVE_HURRICANE_DEFAULT_MOVEMENT_STEP,
                ) else {
                    continue;
                };
                delta.x = fround(delta.x + force.x);
                delta.y = fround(delta.y + force.y);
                if contact.is_none() && cooldown < 1.0 {
                    contact = Some(hurricane);
                }
            }
            if delta != Vec2::ZERO {
                let requested = Vec2::new(
                    fround(row.target.position.x + delta.x),
                    fround(row.target.position.y + delta.y),
                );
                let resolved = (self.context.resolve_enemy_movement)(
                    row.actor_id,
                    row.target.position,
                    requested,
                    row.target.body_radius,
                );
                self.enemies.position_enemy(row.actor_id, resolved);
            }
            let Some(contact) = contact else {
                continue;
            };
            let damage = draw_native_hurricane_damage(
                self.rng,
                contact.contact_charge,
                contact.damage_minimum,
                contact.damage_maximum,
            );
            let amount = fround(
                damage.damage
                    * self.multiplier(
                        row.actor_id,
                        BoneyardSpellHitKind::Air,
                        &contact.common.owner_id,
                    ),
            );
            if amount > 0.0 {
                let damaged = self.damage_with_hurt_sound(
                    row.actor_id,
                    amount,
                    &contact.common.owner_id,
                    true,
                    damage.suppress_hit_sound,
                );
                if damaged.accepted {
                    self.hit(
                        row.actor_id,
                        amount,
                        &contact.common.owner_id,
                        contact.common.id,
                        BoneyardSpellHitKind::AirHurricane,
                        damaged.killed,
                    );
                }
            }
            // Even a suppressed/zero or rejected hit consumes the native contact timer.
            self.enemies
                .set_hurricane_contact_cooldown(row.actor_id, NATIVE_HURRICANE_CONTACT_COOLDOWN);
        }
        for effect in self.source.transients.iter() {
            let PrimaryTransientRef::FireGoodImp(actor) = effect else {
                continue;
            };
            if actor.common.world_key != self.context.world_key
                || !native_hurricane_movement_due(
                    actor.common.id,
                    self.context.tick,
                    NATIVE_HURRICANE_DEFAULT_MOVEMENT_STEP,
                )
            {
                continue;
            }
            let mut delta = Vec2::ZERO;
            for hurricane in &hurricanes {
                if let Some(force) = native_hurricane_orbit_force(
                    hurricane.position,
                    actor.position,
                    hurricane.contact_charge,
                    NATIVE_HURRICANE_DEFAULT_MOVEMENT_STEP,
                ) {
                    delta.x = fround(delta.x + force.x);
                    delta.y = fround(delta.y + force.y);
                }
            }
            if delta == Vec2::ZERO {
                continue;
            }
            let requested =
                Vec2::new(fround(actor.position.x + delta.x), fround(actor.position.y + delta.y));
            let mut moved = actor.clone();
            moved.position = (self.context.resolve_enemy_movement)(
                actor.common.id as usize,
                actor.position,
                requested,
                actor.collision_radius,
            );
            self.updated_transients
                .insert(actor.common.id, PrimarySpellTransient::FireGoodImp(moved));
        }
    }
}
