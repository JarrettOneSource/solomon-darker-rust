//! Weld point contacts and shared combat births from source
//! `spell-combat/projectiles.ts` and `spell-combat/work.ts`.

use super::*;
use crate::secondary::NativeSecondaryElectricBurnEffectState;

impl ContactWork<'_, '_> {
    pub(super) fn apply_channel_projectile_force(
        &mut self,
        row: &ChannelTargetRow,
        amount: f64,
        direction: Vec2,
    ) -> bool {
        match row.kind {
            ChannelTargetKind::Arrow { id, accumulator } => {
                let accumulator = fround(accumulator + amount);
                if accumulator <= 1.0 {
                    self.enemies.set_arrow_chill_tumble_accumulator(id, accumulator);
                } else {
                    self.enemies.tumble_arrow(
                        id,
                        direction,
                        self.context.tick,
                        self.rng,
                        Some(self.order),
                    );
                }
                true
            }
            ChannelTargetKind::Silk { id } => {
                let light_at =
                    self.context.light_at.expect("Silk force requires the native light sampler");
                self.enemies.apply_silk_force(
                    id,
                    amount,
                    direction,
                    self.context.tick,
                    self.rng,
                    light_at,
                );
                true
            }
            ChannelTargetKind::Enemy { .. } | ChannelTargetKind::Scenery => false,
        }
    }

    pub(super) fn resolve_weld_projectile_contact(
        &mut self,
        spell: &NativeWeldProjectileState,
        rows: &[TargetRow],
    ) {
        let common = &spell.common;
        let magic_missile = matches!(spell.build_id, 1000..=1002);
        let mut targets =
            if magic_missile { self.context.primary_scenery_targets.to_vec() } else { Vec::new() };
        targets.extend(rows.iter().map(|row| row.target.clone()));
        let target = first_native_primary_point_contact(PrimaryPointContactQuery {
            actor_mask: if magic_missile && (common.age_ticks >= 200 || spell.target_id.is_none()) {
                0x6
            } else {
                0x2
            },
            position: common.position,
            query_radius: if spell.build_id == 1009 { 15.0 } else { 6.0 },
            targets: &targets,
        });
        let Some(target) = target else { return };
        let Some(row) = rows.iter().find(|row| row.target.id == target.id) else {
            self.resolve_frost_missile_area_contact(spell);
            self.consumed_projectiles.insert(common.id);
            self.finish_weld_projectile_contact(spell);
            return;
        };
        if spell.build_id == 1001 && spell.vector[6] == 0.0 {
            self.queue_frost_missile_cold(row);
        }
        let electric = matches!(spell.build_id, 1002 | 1009);
        if electric {
            let ticks = if spell.build_id == 1002 { 100 } else { 50 };
            let (arcs, stun) = if spell.build_id == 1002 {
                (spell.vector[5], spell.vector[6])
            } else {
                (spell.vector[2], spell.vector[3])
            };
            self.target_effect(
                row.actor_id,
                NativeSecondaryTargetEffectPatch {
                    electric_burn: Some(Some(NativeSecondaryElectricBurnEffectState {
                        arc_count: arcs.round().max(0.0) as usize,
                        damage_per_tick: common.damage / ticks as f64,
                        owner_id: common.owner_id.clone(),
                        source_actor_id: common.id,
                        stun_factor: stun,
                        ticks,
                    })),
                    ..NativeSecondaryTargetEffectPatch::default()
                },
            );
        }
        let amount = if electric {
            common.damage
                * self.multiplier(row.actor_id, BoneyardSpellHitKind::Air, &common.owner_id)
        } else {
            common.damage
        };
        let damaged = self.damage_with_flags(
            row.actor_id,
            amount,
            &common.owner_id,
            true,
            false,
            spell.build_id == 1002,
        );
        if !damaged.accepted {
            return;
        }
        self.hit(
            row.actor_id,
            amount,
            &common.owner_id,
            common.id,
            BoneyardSpellHitKind::Weld,
            damaged.killed,
        );
        self.resolve_frost_missile_area_contact(spell);
        if spell.build_id == 1009 && spell.contacts_remaining > 1 {
            let mut retained = spell.clone();
            retained.contacts_remaining -= 1;
            self.updated_projectiles.insert(common.id, PrimarySpellProjectile::Weld(retained));
            self.publish_impact(&PrimarySpellProjectile::Weld(spell.clone()), common.position);
        } else {
            self.consumed_projectiles.insert(common.id);
            self.finish_weld_projectile_contact(spell);
        }
    }

    /// Widen owns Frost Missile's radial cold; Chill Wind occupies a different
    /// vector cell and never selects this lane. Terrain contact uses the same body.
    pub(super) fn resolve_frost_missile_area_contact(&mut self, spell: &NativeWeldProjectileState) {
        if spell.build_id != 1001
            || spell.vector[6] <= 0.0
            || spell.common.damage <= 0.0
            || spell.common.world_key != self.context.world_key
        {
            return;
        }
        let mut radius = fround(spell.vector[6] * 120.0);
        for _ in 0..15 {
            radius = fround(radius * 1.024999976158142);
        }
        let amount = fround(fround(spell.common.damage / 10.0) / 10.0);
        for row in root_target_rows(self.enemies, spell.common.position, radius, 0x2) {
            self.queue_frost_missile_cold(&row);
            let damaged = self.damage(row.actor_id, amount, &spell.common.owner_id, true);
            if damaged.accepted {
                self.hit(
                    row.actor_id,
                    amount,
                    &spell.common.owner_id,
                    spell.common.id,
                    BoneyardSpellHitKind::Weld,
                    damaged.killed,
                );
            }
        }
    }

    fn queue_frost_missile_cold(&mut self, row: &TargetRow) {
        use crate::enemies::model::{EnemyEvent, EnemyEventKind, EnemySound};
        self.weld_cold(row.actor_id, NATIVE_WELD_FROST_SLOW_FACTOR, 150);
        let pitch = self.rng.draw_scaled_float(0.2_f32 as f64, true);
        let mut event =
            EnemyEvent::new(EnemyEventKind::EnemyDamageSound, row.actor_id, self.context.tick);
        event.gain_scale = Some(1.0);
        event.pitch = Some(fround(1.0 + pitch));
        event.sound = Some(EnemySound::Freeze);
        event.source_position = Some(row.target.position);
        self.enemies.emit_event(event);
    }

    fn finish_weld_projectile_contact(&mut self, spell: &NativeWeldProjectileState) {
        if spell.build_id == 1000 {
            let effects = self.detonate_weld_fire(
                NativeWeldActorBirth {
                    build_id: spell.build_id,
                    direction: spell.common.direction,
                    id: self.next_id,
                    origin: spell.common.position,
                    owner_id: &spell.common.owner_id,
                    tick: self.context.tick,
                    vector: &spell.vector,
                    world_key: &spell.common.world_key,
                },
                spell.presentation_seed.unwrap_or(0),
                true,
            );
            self.impact_transients.extend(effects);
        } else {
            self.publish_impact(
                &PrimarySpellProjectile::Weld(spell.clone()),
                spell.common.position,
            );
        }
    }

    pub(super) fn weld_cold(&mut self, actor_id: usize, factor: f64, ticks: u64) {
        self.target_effect(
            actor_id,
            NativeSecondaryTargetEffectPatch {
                cold_slow_factor: Some(factor),
                cold_slow_material: Some(true),
                cold_slow_ticks: Some(ticks),
                ..NativeSecondaryTargetEffectPatch::default()
            },
        );
    }

    pub(super) fn enroll_weld_combat_actor(
        &mut self,
        mut actor: NativeWeldWorldActor,
    ) -> PrimarySpellTransient {
        enroll_native_weld_actor_painter(&mut actor, self.order);
        PrimarySpellTransient::Weld(actor)
    }

    pub(super) fn detonate_weld_fire(
        &mut self,
        birth: NativeWeldActorBirth<'_>,
        seed: u32,
        impact: bool,
    ) -> Vec<PrimarySpellTransient> {
        let detonation =
            create_primary_spell_weld_fire_detonation(birth, self.rng, seed, impact, self.order);
        self.next_id = detonation.next_id;
        self.pending_fire_contacts
            .extend(detonation.contacts.into_iter().map(FireActorContact::Ember));
        detonation
            .transients
            .into_iter()
            .map(|effect| {
                if let PrimarySpellTransient::Weld(actor) = effect {
                    self.enroll_weld_combat_actor(actor)
                } else {
                    effect
                }
            })
            .collect()
    }

    pub(super) fn publish_weld_boulder_debris(
        &mut self,
        boulder: &NativeWeldEtherealBoulderState,
        scale: f64,
    ) {
        let debris = create_native_weld_boulder_contact_debris_program(scale, self.rng);
        let id = self.next_id;
        self.next_id += 1;
        // Preserve the native contact program and ID even for an exhausted 0/0 pool.
        if !scale.is_finite() {
            return;
        }
        let actor = create_native_weld_boulder_debris_actor(
            NativeWeldActorBirth {
                build_id: 1006,
                direction: boulder.common.direction,
                id,
                origin: boulder.common.origin,
                owner_id: &boulder.common.owner.owner_id,
                tick: self.context.tick,
                vector: &boulder.common.vector,
                world_key: &boulder.common.owner.world_key,
            },
            debris,
        );
        let effect = self.enroll_weld_combat_actor(NativeWeldWorldActor::BoulderDebris(actor));
        self.impact_transients.push(effect);
    }
}
