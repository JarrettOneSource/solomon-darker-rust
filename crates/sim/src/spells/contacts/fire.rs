//! Current Source `spell-combat/fire.ts`. Newborn terminal dispatch calls these
//! same receiver passes immediately, without replaying older actor contacts.
use super::*;

impl ContactWork<'_, '_> {
    pub(super) fn resolve_fire_explosion(&mut self, effect: &PrimarySpellFireExplosion) {
        if effect.common.world_key != self.context.world_key {
            return;
        }
        let rows = target_rows(self.enemies);
        let targets: Vec<_> = rows.iter().map(|row| row.target.clone()).collect();
        let radius = fround(fround(effect.visual_scale * 110.0) * 0.5);
        let ordered = if effect.presentation == FireExplosionPresentation::Steam {
            native_primary_rectangle_targets(effect.origin, radius, radius, 0x2, &targets)
                .into_iter()
                .filter(|target| {
                    squared_distance(target.position, effect.origin)
                        < radius * radius + target.body_radius * target.body_radius
                })
                .collect()
        } else {
            native_primary_root_targets(effect.origin, radius, 0x2, &targets)
        };
        for target in ordered {
            let row = rows
                .iter()
                .find(|row| row.target.id == target.id)
                .expect("gathered Fire explosion receiver");
            self.burn(row.actor_id, &effect.common.owner_id, effect.burn_damage);
            // Each native damage lane stores one half; the receiver gets both.
            let amount = fround(effect.damage + effect.damage);
            if amount <= 0.0 {
                continue;
            }
            let mut request = EnemyDamageRequest::new(
                row.actor_id,
                amount,
                self.context.tick,
                Some(effect.common.owner_id.clone()),
            );
            request.has_magic_damage = true;
            // Reset native context flags stay zero, independently of the lane.
            request.magic = false;
            let damaged = self.damage_with_request(request);
            if damaged.accepted {
                self.hit(
                    row.actor_id,
                    amount,
                    &effect.common.owner_id,
                    effect.common.id,
                    BoneyardSpellHitKind::FireExplosion,
                    damaged.killed,
                );
            }
        }
    }

    pub(super) fn resolve_fire_contacts(&mut self) {
        let mut explosions: Vec<_> = self
            .source
            .transients
            .iter()
            .chain(self.impact_transients.iter().map(|effect| effect.view()))
            .filter_map(|effect| {
                if let PrimaryTransientRef::FireExplosion(explosion) = effect
                    && explosion.common.age_ticks == 0
                {
                    Some(explosion.clone())
                } else {
                    None
                }
            })
            .collect();
        explosions.sort_by_key(|effect| effect.common.id);
        for explosion in explosions {
            self.resolve_fire_explosion(&explosion);
        }
        let contacts = std::mem::take(&mut self.pending_fire_contacts);
        self.resolve_fire_actor_contacts(contacts);
    }

    pub(super) fn resolve_fire_actor_contacts(&mut self, mut contacts: Vec<FireActorContact>) {
        contacts.sort_by_key(fire_contact_id);
        for contact in contacts {
            let (id, owner_id, world_key, kind) = match &contact {
                FireActorContact::Ember(contact) => (
                    contact.spell_id,
                    contact.owner_id.as_str(),
                    contact.world_key.as_str(),
                    BoneyardSpellHitKind::FireEmber,
                ),
                FireActorContact::GoodImp(contact) => (
                    contact.spell_id,
                    contact.owner_id.as_str(),
                    contact.world_key.as_str(),
                    BoneyardSpellHitKind::FireGoodImp,
                ),
                FireActorContact::Patch(contact) => (
                    contact.spell_id,
                    contact.owner_id.as_str(),
                    contact.world_key.as_str(),
                    BoneyardSpellHitKind::FirePatch,
                ),
            };
            if world_key != self.context.world_key
                || (kind == BoneyardSpellHitKind::FireEmber
                    && self.consumed_transients.contains(&id))
            {
                continue;
            }
            let rows = target_rows(self.enemies);
            let contacted = match &contact {
                FireActorContact::GoodImp(contact) => {
                    rows.iter().filter(|row| row.target.id == contact.target_id).collect::<Vec<_>>()
                }
                FireActorContact::Ember(contact) => {
                    let targets: Vec<_> = rows.iter().map(|row| row.target.clone()).collect();
                    native_primary_root_targets(contact.position, contact.radius, 0x2, &targets)
                        .first()
                        .and_then(|target| rows.iter().find(|row| row.target.id == target.id))
                        .into_iter()
                        .collect()
                }
                FireActorContact::Patch(contact) => rows
                    .iter()
                    .filter(|row| {
                        native_primary_target_eligible(&row.target, 0x2)
                            && squared_distance(row.target.position, contact.position)
                                < contact.radius * contact.radius
                    })
                    .collect(),
            };
            for row in contacted {
                let base_amount = match &contact {
                    FireActorContact::GoodImp(contact) => contact.amount,
                    FireActorContact::Ember(contact) | FireActorContact::Patch(contact) => {
                        self.burn(row.actor_id, owner_id, contact.burn_damage);
                        contact.amount
                    }
                };
                let amount = base_amount * self.multiplier(row.actor_id, kind, owner_id);
                if amount <= 0.0 {
                    continue;
                }
                let mut request = EnemyDamageRequest::new(
                    row.actor_id,
                    amount,
                    self.context.tick,
                    Some(owner_id.into()),
                );
                request.magic = true;
                request.has_magic_damage = true;
                if kind == BoneyardSpellHitKind::FirePatch {
                    request.hit_strength = fround(0.25 + self.rng.draw_scaled_float(0.5, false));
                    request.suppress_hit_reaction = true;
                }
                let damaged = self.damage_with_request(request);
                if !damaged.accepted {
                    continue;
                }
                self.hit(row.actor_id, amount, owner_id, id, kind, damaged.killed);
                if let FireActorContact::Ember(contact) = &contact {
                    self.consume_ember(id, contact.position, owner_id, world_key);
                    break;
                }
            }
        }
    }
}
