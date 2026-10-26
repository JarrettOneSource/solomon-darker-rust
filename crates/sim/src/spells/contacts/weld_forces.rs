//! Existing Hail knockback and Steam contacts from `spell-combat/forces.ts`.
//! Steam status damage belongs to the secondary epoch; these actors admit its
//! modifier; the admitted secondary terminal receiver owns its detonation.

use super::*;
use crate::secondary::NativeSecondarySteamedEffectState;

impl ContactWork<'_, '_> {
    pub(super) fn resolve_weld_transient_forces(&mut self) {
        for effect in
            self.source.transients.iter().map(|effect| effect.snapshot()).collect::<Vec<_>>()
        {
            let PrimarySpellTransient::Weld(NativeWeldWorldActor::HailChild(
                NativeWeldHailChildActorState::Knockback(mut actor),
            )) = effect
            else {
                continue;
            };
            if actor.common.owner.world_key != self.context.world_key {
                continue;
            }
            let actor_id =
                actor.target_id.strip_prefix("enemy:").and_then(|id| id.parse::<usize>().ok());
            let row = actor_id.and_then(|id| {
                target_rows(self.enemies).into_iter().find(|row| row.actor_id == id)
            });
            let Some(row) = row else {
                self.consumed_transients.insert(actor.common.owner.id);
                self.active_knockback_target_ids.remove(&actor.target_id);
                continue;
            };
            self.enemies.release_skeleton_pike(row.actor_id);
            let requested = Vec2::new(
                fround(row.target.position.x + actor.delta.x),
                fround(row.target.position.y + actor.delta.y),
            );
            let resolved = (self.context.resolve_enemy_movement)(
                row.actor_id,
                row.target.position,
                requested,
                row.target.body_radius,
            );
            self.enemies.position_enemy(row.actor_id, resolved);
            actor.remaining_ticks -= 1;
            if actor.remaining_ticks == 0 {
                self.consumed_transients.insert(actor.common.owner.id);
                self.active_knockback_target_ids.remove(&actor.target_id);
            } else {
                actor.common.owner.age_ticks += 1;
                self.updated_transients.insert(
                    actor.common.owner.id,
                    PrimarySpellTransient::Weld(NativeWeldWorldActor::HailChild(
                        NativeWeldHailChildActorState::Knockback(actor),
                    )),
                );
            }
        }
        for effect in
            self.source.transients.iter().map(|effect| effect.snapshot()).collect::<Vec<_>>()
        {
            let PrimarySpellTransient::Weld(NativeWeldWorldActor::Steam(actor)) = effect else {
                continue;
            };
            if actor.common.owner.world_key != self.context.world_key
                || !actor.contact_enabled
                || !actor.contact_due
            {
                continue;
            }
            let center = Vec2::new(actor.position.x, fround(actor.position.y + 15.0));
            let radius = fround(actor.scale * 50.0);
            for row in root_target_rows(self.enemies, center, radius, 0x2) {
                self.target_effect(
                    row.actor_id,
                    NativeSecondaryTargetEffectPatch {
                        steamed: Some(Some(NativeSecondarySteamedEffectState {
                            damage_per_tick: fround(actor.life.min(1.0) * actor.contact_damage),
                            ember_damage: actor.common.vector[6],
                            ember_fragments: actor.common.vector[7].round().max(0.0) as usize,
                            explode_damage: actor.common.vector[4],
                            explode_radius: actor.common.vector[5],
                            owner_id: actor.common.owner.owner_id.clone(),
                            source_actor_id: actor.common.owner.id,
                            ticks: 10,
                        })),
                        ..NativeSecondaryTargetEffectPatch::default()
                    },
                );
            }
        }
    }
}
