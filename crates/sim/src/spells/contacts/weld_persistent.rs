//! Released Weld Boulder/Hail pools and Meteor contacts from source
//! `spell-combat/transients.ts`. Native gathers retain their separate root/cell
//! ordering; each accepted contact updates the real enemy before the next query.

use super::*;

impl ContactWork<'_, '_> {
    pub(super) fn resolve_weld_persistent_contacts(&mut self) {
        let mut effects = self.source.transients.clone();
        effects.sort_by_key(|effect| effect.common().id);
        for effect in effects {
            let PrimarySpellTransient::Weld(NativeWeldWorldActor::Persistent(actor)) = effect
            else {
                continue;
            };
            if actor.held() || actor.common().owner.world_key != self.context.world_key {
                continue;
            }
            match actor {
                NativeWeldPersistentActorState::EtherealBoulder(actor) => {
                    self.resolve_weld_boulder_contact(actor)
                }
                NativeWeldPersistentActorState::Hailstones(actor) => {
                    self.resolve_weld_hail_contacts(actor)
                }
                NativeWeldPersistentActorState::MeteorField(_) => {}
            }
        }
    }

    fn resolve_weld_boulder_contact(&mut self, mut actor: NativeWeldEtherealBoulderState) {
        let rows = target_rows(self.enemies);
        let targets: Vec<_> = rows.iter().map(|row| row.target.clone()).collect();
        let contacts =
            native_primary_root_targets(actor.common.origin, actor.scale * 75.0, 0x6, &targets);
        let contacts: Vec<_> = contacts
            .into_iter()
            .filter(|target| !actor.hit_target_ids.contains(&target.id))
            .collect();
        if contacts.is_empty() {
            return;
        }
        let original = actor.clone();
        let mut scale = actor.scale;
        let mut continue_traversal = true;
        for target in contacts {
            if !continue_traversal {
                break;
            }
            let row = rows
                .iter()
                .find(|row| row.target.id == target.id)
                .expect("gathered Boulder target");
            let contact = consume_native_earth_boulder_contact(NativeEarthBoulderContactInput {
                release_base_damage: actor.damage,
                release_charge: actor.maximum_scale,
                remaining_pool: actor.remaining_damage,
                target_health: row.current_health.max(0.0),
                toughness: actor.toughness,
            });
            let damaged =
                self.damage(row.actor_id, contact.damage, &actor.common.owner.owner_id, true);
            if !damaged.accepted {
                continue;
            }
            actor.hit_target_ids.push(target.id.clone());
            scale = contact.charge;
            continue_traversal = contact.continue_traversal;
            self.publish_weld_boulder_debris(&original, scale);
            actor.remaining_damage = contact.remaining_pool;
            self.hit(
                row.actor_id,
                contact.damage,
                &actor.common.owner.owner_id,
                actor.common.owner.id,
                BoneyardSpellHitKind::Weld,
                damaged.killed,
            );
        }
        let hit_ids = actor.hit_target_ids.clone();
        let remaining = actor.remaining_damage;
        if retain_native_weld_persistent_actor_contacts(&mut actor, hit_ids, remaining, scale) {
            self.updated_transients.insert(
                actor.common.owner.id,
                PrimarySpellTransient::Weld(NativeWeldWorldActor::Persistent(
                    NativeWeldPersistentActorState::EtherealBoulder(actor),
                )),
            );
        } else {
            self.consumed_transients.insert(original.common.owner.id);
            let children = create_primary_spell_weld_boulder_terminal(
                self.next_id,
                &original,
                self.context.tick,
                self.rng,
                self.order,
            );
            self.next_id += children.len() as u64;
            for actor in children {
                let effect = self.enroll_weld_combat_actor(actor);
                self.impact_transients.push(effect);
            }
        }
    }

    fn resolve_weld_hail_contacts(&mut self, mut actor: NativeWeldHailstonesState) {
        if actor.release_age_ticks == Some(0) {
            return;
        }
        let mut damage_by_rock: BTreeMap<_, _> =
            actor.rocks.iter().map(|rock| (rock.rock_id, rock.damage_remaining)).collect();
        for (substep_index, substep) in
            native_weld_hailstone_flight_contact_substeps(&actor).into_iter().enumerate()
        {
            let mut query_radius = actor.collision_radius;
            for _ in 0..NATIVE_WELD_HAIL_FLIGHT_SUBSTEPS - substep_index - 1 {
                query_radius = fround(query_radius - actor.widen)
            }
            let rows = target_rows(self.enemies);
            let targets: Vec<_> = rows.iter().map(|row| row.target.clone()).collect();
            for target in native_primary_root_targets(substep.origin, query_radius, 0x2, &targets) {
                let Some(current_row) =
                    target_rows(self.enemies).into_iter().find(|row| row.target.id == target.id)
                else {
                    continue;
                };
                for rock in &actor.rocks {
                    let Some(current) = target_rows(self.enemies)
                        .into_iter()
                        .find(|row| row.target.id == target.id)
                    else {
                        break;
                    };
                    let Some((_, offset)) =
                        substep.release_offsets.iter().find(|(id, _)| *id == rock.rock_id)
                    else {
                        continue;
                    };
                    let remaining = damage_by_rock.get(&rock.rock_id).copied().unwrap_or(0.0);
                    if remaining < 0.001 {
                        continue;
                    }
                    let point = Vec2::new(
                        fround(substep.origin.x + offset.x),
                        fround(substep.origin.y + offset.y),
                    );
                    let radius = fround(
                        current_row.target.body_radius * NATIVE_WELD_HAIL_TARGET_RADIUS_FACTOR,
                    );
                    if squared_distance(point, current_row.target.position) >= radius * radius {
                        continue;
                    }
                    self.weld_cold(
                        current.actor_id,
                        NATIVE_WELD_HAIL_COLD_SLOW_FACTOR,
                        NATIVE_WELD_HAIL_COLD_SLOW_TICKS,
                    );
                    let target_key = format!("enemy:{}", current.actor_id);
                    if !self.active_knockback_target_ids.contains(&target_key)
                        && let Some(knockback) = create_native_weld_hail_knockback(
                            &actor,
                            self.next_id,
                            &target_key,
                            self.context.tick,
                        )
                    {
                        let effect =
                            self.enroll_weld_combat_actor(NativeWeldWorldActor::HailChild(
                                NativeWeldHailChildActorState::Knockback(knockback),
                            ));
                        self.impact_transients.push(effect);
                        self.active_knockback_target_ids.insert(target_key);
                        self.next_id += 1;
                    }
                    let health = current.current_health.max(0.0);
                    let amount = health.min(remaining);
                    let consumed =
                        if remaining < health { amount } else { amount / actor.toughness };
                    let damaged =
                        self.damage(current.actor_id, amount, &actor.common.owner.owner_id, true);
                    if !damaged.accepted {
                        continue;
                    }
                    let next_damage = (remaining - consumed).max(0.0);
                    damage_by_rock.insert(rock.rock_id, next_damage);
                    self.hit(
                        current.actor_id,
                        amount,
                        &actor.common.owner.owner_id,
                        actor.common.owner.id,
                        BoneyardSpellHitKind::Weld,
                        damaged.killed,
                    );
                    if next_damage < 0.001 {
                        let draw = native_weld_hailstone_draw_offset(rock);
                        let start = Vec2::new(
                            fround(substep.origin.x + draw.x),
                            fround(substep.origin.y + draw.y),
                        );
                        let children = create_native_weld_hail_contact_presentation(
                            &actor,
                            start,
                            point,
                            self.next_id,
                            self.context.tick,
                            self.rng,
                        );
                        self.next_id += children.len() as u64;
                        for child in children {
                            let effect = self
                                .enroll_weld_combat_actor(NativeWeldWorldActor::HailChild(child));
                            self.impact_transients.push(effect);
                        }
                    }
                    if damaged.killed {
                        break;
                    }
                }
            }
        }
        let damage: Vec<_> = actor
            .rocks
            .iter()
            .map(|rock| damage_by_rock.get(&rock.rock_id).copied().unwrap_or(0.0))
            .collect();
        if retain_native_weld_hailstone_damage(&mut actor, &damage) {
            self.updated_transients.insert(
                actor.common.owner.id,
                PrimarySpellTransient::Weld(NativeWeldWorldActor::Persistent(
                    NativeWeldPersistentActorState::Hailstones(actor),
                )),
            );
        } else {
            self.consumed_transients.insert(actor.common.owner.id);
        }
    }

    pub(super) fn resolve_weld_meteor_contacts(&mut self) {
        let mut effects = self.source.transients.clone();
        effects.sort_by_key(|effect| effect.common().id);
        for effect in effects {
            let PrimarySpellTransient::Weld(NativeWeldWorldActor::Meteor(meteor)) = effect else {
                continue;
            };
            if meteor.common.owner.world_key != self.context.world_key {
                continue;
            }
            let pulse = if meteor.impact_due {
                Some((meteor.damage * 0.5, native_weld_meteor_direct_radius()))
            } else if meteor.pulse_due {
                Some((
                    meteor.damage / 20.0,
                    native_weld_meteor_pulse_radius(meteor.impact_radius_scalar),
                ))
            } else {
                None
            };
            if let Some((amount, radius)) = pulse {
                for row in root_target_rows(self.enemies, meteor.position, radius, 0x2) {
                    let damaged =
                        self.damage(row.actor_id, amount, &meteor.common.owner.owner_id, true);
                    if damaged.accepted {
                        self.hit(
                            row.actor_id,
                            amount,
                            &meteor.common.owner.owner_id,
                            meteor.common.owner.id,
                            BoneyardSpellHitKind::Weld,
                            damaged.killed,
                        );
                    }
                }
            }
            if meteor.impact_due {
                let effects = self.detonate_weld_fire(
                    NativeWeldActorBirth {
                        build_id: 1007,
                        direction: meteor.common.direction,
                        id: self.next_id,
                        origin: meteor.position,
                        owner_id: &meteor.common.owner.owner_id,
                        tick: self.context.tick,
                        vector: &meteor.common.vector,
                        world_key: &meteor.common.owner.world_key,
                    },
                    meteor.private_seed,
                    false,
                );
                self.impact_transients.extend(effects);
            }
        }
    }
}
