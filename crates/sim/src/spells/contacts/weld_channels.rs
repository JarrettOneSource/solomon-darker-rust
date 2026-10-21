//! Flame Lash, Blizzard and Steam channel contacts from source
//! `spell-combat/channels.ts` and its native pushback helpers.

use super::*;

fn channel_birth(
    emission: &PrimarySpellChannelEmission,
    id: u64,
    tick: u64,
    origin: Vec2,
    direction: Vec2,
) -> NativeWeldActorBirth<'_> {
    let NativePrimarySkillProfile::Weld(profile) = &emission.primary_skill else {
        unreachable!("Weld channel profile")
    };
    NativeWeldActorBirth {
        build_id: profile.build_id,
        direction,
        id,
        origin,
        owner_id: &emission.owner_id,
        tick,
        vector: &profile.vector.values,
        world_key: &emission.world_key,
    }
}

impl ContactWork<'_, '_> {
    pub(super) fn resolve_weld_channel(&mut self, emission: &PrimarySpellChannelEmission) {
        let NativePrimarySkillProfile::Weld(profile) = &emission.primary_skill else {
            panic!("Weld channel requires its own profile")
        };
        match profile.build_id {
            1003 => self.resolve_flame_lash(emission),
            1004 => self.resolve_blizzard(emission),
            1005 => self.resolve_steam_pushback(emission),
            _ => panic!("Weld channel build is not supported"),
        }
    }

    fn resolve_flame_lash(&mut self, emission: &PrimarySpellChannelEmission) {
        use crate::spells::special_death::{
            NativeSpellSpecialDeathKind, NativeSpellSpecialDeathPayload,
        };
        let NativePrimarySkillProfile::Weld(profile) = &emission.primary_skill else {
            unreachable!()
        };
        let main = emission.flame_lash_presentation.as_ref();
        let target_id = if let Some(main) = main {
            main.target_id.clone()
        } else {
            self.source.transients.iter().find_map(|effect| {
                if let PrimarySpellTransient::Weld(NativeWeldWorldActor::Channel(channel)) = effect
                    && channel.common.owner.id == emission.id
                    && channel.common.owner.owner_id == emission.owner_id
                    && channel.common.owner.world_key == emission.world_key
                    && channel.common.build_id == 1003
                {
                    channel.target_id.clone()
                } else {
                    None
                }
            })
        };
        let mut target = target_id.and_then(|id| {
            target_rows(self.enemies)
                .into_iter()
                .find(|row| row.target.id == id && row.target.active && !row.target.pending_remove)
                .map(|row| row.target)
        });
        if target.is_none() {
            self.publish_flame_main(emission);
            return;
        }
        let mut contacted = BTreeSet::new();
        let mut damage = emission.damage;
        let mut previous_point = emission.origin;
        let mut previous_native_point = main
            .and_then(|main| main.native_geometry)
            .map_or(emission.origin, |geometry| geometry.end);
        let fragments = profile.vector.values[7].max(0.0).trunc() as u32;
        for hop in 0..=profile.vector.values[2] as usize {
            let Some(current_target) = target.take() else { break };
            let Some(row) = target_rows(self.enemies)
                .into_iter()
                .find(|row| row.target.id == current_target.id)
            else {
                break;
            };
            if !native_primary_target_eligible(&row.target, 0x2) {
                break;
            }
            contacted.insert(row.target.id.clone());
            let point = primary_spell_target_point(&row.target);
            let direction = normalized_difference(previous_point, point);
            if hop > 0 {
                let width = self.rng.draw_scaled_float(0.3_f32 as f64, false);
                let legacy = air_primary_bolt_geometry(previous_point, direction, point);
                if let Some((geometry, creator_registration)) =
                    main.and_then(|main| main.native_geometry.zip(main.creator_registration))
                {
                    let root = row.target.position;
                    let factory = create_native_flame_lash_factory(
                        NativeFlameLashFactoryBirthInput {
                            channel: NativeWeldChannelInput {
                                birth: channel_birth(
                                    emission,
                                    self.next_id + 1,
                                    self.context.tick,
                                    previous_point,
                                    direction,
                                ),
                                endpoint: Some(legacy.endpoint),
                                midpoint: Some(legacy.midpoint),
                                target_id: Some(row.target.id.clone()),
                                underpowered: false,
                                enhanced_effects: self.context.enhanced_effects,
                                flame_visual: None,
                            },
                            source_fade_id: self.next_id,
                            camera_displacement: None,
                            geometry: NativeFlameLashFactoryInput {
                                start: Vec2::new(
                                    fround(previous_native_point.x + geometry.offset.x),
                                    fround(previous_native_point.y + geometry.offset.y),
                                ),
                                control: Vec2::new(
                                    fround((previous_native_point.x + root.x) * 0.5),
                                    fround((previous_native_point.y + root.y) * 0.5),
                                ),
                                end: root,
                                offset: geometry.offset,
                            },
                            role: NativeFlameChannelRole::Chain,
                            width: fround(0.8_f32 as f64 - width),
                            creator_registration,
                            misc_append_ordinal: hop,
                        },
                        self.rng,
                        self.order,
                    )
                    .expect("valid native chain Flame factory");
                    self.owned_transients.push(PrimarySpellTransient::Weld(
                        NativeWeldWorldActor::FlameLashFade(factory.source),
                    ));
                    self.owned_transients.push(PrimarySpellTransient::Weld(
                        NativeWeldWorldActor::Channel(factory.channel),
                    ));
                    self.next_id += 2;
                } else {
                    self.publish_flame_fade(
                        emission,
                        NativeWeldFlameLashFadeVariant::Source,
                        previous_point,
                        direction,
                    );
                    let actor = create_native_weld_channel_actor(NativeWeldChannelInput {
                        birth: channel_birth(
                            emission,
                            self.next_id,
                            self.context.tick,
                            previous_point,
                            direction,
                        ),
                        endpoint: Some(legacy.endpoint),
                        midpoint: Some(legacy.midpoint),
                        target_id: Some(row.target.id.clone()),
                        underpowered: false,
                        enhanced_effects: self.context.enhanced_effects,
                        flame_visual: None,
                    });
                    let actor = self.enroll_weld_combat_actor(NativeWeldWorldActor::Channel(actor));
                    self.owned_transients.push(actor);
                    self.next_id += 1;
                }
                let offset = main
                    .and_then(|main| main.native_geometry)
                    .map_or(Vec2::new(0.0, -20.0), |geometry| geometry.offset);
                self.publish_flame_fade(
                    emission,
                    NativeWeldFlameLashFadeVariant::Chain,
                    Vec2::new(
                        fround(row.target.position.x + offset.x),
                        fround(row.target.position.y + offset.y),
                    ),
                    direction,
                );
            }
            let stun = profile.vector.values[3];
            if !emission.underpowered && stun < 1.0 {
                self.target_effect(
                    row.actor_id,
                    NativeSecondaryTargetEffectPatch {
                        stun_factor: Some(stun),
                        stun_ticks: Some(25),
                        ..NativeSecondaryTargetEffectPatch::default()
                    },
                );
            }
            // The flash word belongs even to a rejected receiver or no explosion.
            let flash = self.rng.draw_scaled_float(0.5_f32 as f64, false);
            let payload = if profile.vector.values[4] > 0.0 {
                Some(NativeSpellSpecialDeathPayload {
                    kind: NativeSpellSpecialDeathKind::Fire,
                    explode_damage: profile.vector.values[4],
                    explode_radius: profile.vector.values[5],
                    ember_damage: profile.vector.values[6],
                    ember_fragments: if hop == 0 { fragments } else { fragments / 2 },
                    private_seed: self.rng.draw_integer(1_000_000) as u32,
                })
            } else {
                None
            };
            let amount = damage
                * self.multiplier(row.actor_id, BoneyardSpellHitKind::Air, &emission.owner_id);
            let mut request = EnemyDamageRequest::new(
                row.actor_id,
                amount,
                self.context.tick,
                Some(emission.owner_id.clone()),
            );
            request.magic = true;
            request.has_magic_damage = true;
            request.hit_strength = fround(0.25 + flash);
            request.suppress_hit_reaction = true;
            let damaged = self.damage_with_request(request);
            if damaged.accepted {
                self.hit(
                    row.actor_id,
                    amount,
                    &emission.owner_id,
                    emission.id,
                    BoneyardSpellHitKind::Weld,
                    damaged.killed,
                );
                if damaged.special_death_accepted
                    && let Some(payload) = payload
                {
                    self.resolve_special_death_payload(
                        payload,
                        row.actor_id,
                        row.target.position,
                        &emission.owner_id,
                    );
                }
            }
            if hop == 0 {
                self.publish_flame_main(emission);
            }
            previous_point = point;
            if hop > 0 {
                previous_native_point = row.target.position;
            }
            damage = fround(damage * 0.6000000238418579);
            target = if emission.underpowered || hop >= profile.vector.values[2] as usize {
                None
            } else {
                nearest_unused_air_chain_target(
                    self.enemies,
                    if hop == 0 { previous_native_point } else { row.target.position },
                    &contacted,
                    200.0,
                )
                .and_then(|id| {
                    target_rows(self.enemies)
                        .into_iter()
                        .find(|row| row.target.id == id)
                        .map(|row| row.target)
                })
            };
        }
    }

    fn publish_flame_fade(
        &mut self,
        emission: &PrimarySpellChannelEmission,
        variant: NativeWeldFlameLashFadeVariant,
        origin: Vec2,
        direction: Vec2,
    ) {
        let fade = create_native_weld_flame_lash_fade(
            NativeWeldFlameLashFadeInput {
                birth: channel_birth(emission, self.next_id, self.context.tick, origin, direction),
                alpha: if variant == NativeWeldFlameLashFadeVariant::Endpoint
                    && emission.underpowered
                {
                    0.5
                } else {
                    1.0
                },
                variant,
                enhanced_effects: self.context.enhanced_effects,
            },
            self.rng,
            Some(self.order),
        );
        let actor = self.enroll_weld_combat_actor(NativeWeldWorldActor::FlameLashFade(fade));
        self.owned_transients.push(actor);
        self.next_id += 1;
    }
    fn publish_flame_main(&mut self, emission: &PrimarySpellChannelEmission) {
        let main = emission.flame_lash_presentation.as_ref();
        if let Some(endpoint) = main
            .and_then(|main| main.preclip_endpoint)
            .or(emission.endpoint)
            .filter(|point| point.x > -9999.0)
        {
            self.publish_flame_fade(
                emission,
                NativeWeldFlameLashFadeVariant::Endpoint,
                endpoint,
                emission.direction,
            );
        }
        if let Some(main) = main
            && let Some(geometry) = main.native_geometry
            && let Some(creator_registration) = main.creator_registration
            && let Some(camera_displacement) = main.camera_displacement
        {
            let factory = create_native_flame_lash_factory(
                NativeFlameLashFactoryBirthInput {
                    channel: NativeWeldChannelInput {
                        birth: channel_birth(
                            emission,
                            emission.id,
                            self.context.tick,
                            emission.origin,
                            emission.direction,
                        ),
                        endpoint: emission.endpoint,
                        midpoint: main.midpoint,
                        target_id: main.target_id.clone(),
                        underpowered: emission.underpowered,
                        enhanced_effects: self.context.enhanced_effects,
                        flame_visual: None,
                    },
                    source_fade_id: self.next_id,
                    geometry,
                    role: NativeFlameChannelRole::Main,
                    width: if emission.underpowered { 0.75 } else { 1.0 },
                    camera_displacement,
                    creator_registration,
                    misc_append_ordinal: 0,
                },
                self.rng,
                self.order,
            )
            .expect("valid native main Flame factory");
            self.owned_transients.push(PrimarySpellTransient::Weld(
                NativeWeldWorldActor::FlameLashFade(factory.source),
            ));
            self.owned_transients
                .push(PrimarySpellTransient::Weld(NativeWeldWorldActor::Channel(factory.channel)));
            self.next_id += 1;
        } else {
            // Source compatibility inputs have no retained ribbon recipe.
            self.publish_flame_fade(
                emission,
                NativeWeldFlameLashFadeVariant::Source,
                emission.origin,
                emission.direction,
            );
            if let Some(main) = main {
                let channel = create_native_weld_channel_actor(NativeWeldChannelInput {
                    birth: channel_birth(
                        emission,
                        emission.id,
                        self.context.tick,
                        emission.origin,
                        emission.direction,
                    ),
                    endpoint: emission.endpoint,
                    midpoint: main.midpoint,
                    target_id: main.target_id.clone(),
                    underpowered: emission.underpowered,
                    enhanced_effects: self.context.enhanced_effects,
                    flame_visual: None,
                });
                let actor = self.enroll_weld_combat_actor(NativeWeldWorldActor::Channel(channel));
                self.owned_transients.push(actor);
            }
        }
    }

    fn resolve_blizzard(&mut self, emission: &PrimarySpellChannelEmission) {
        let NativePrimarySkillProfile::Weld(profile) = &emission.primary_skill else {
            unreachable!()
        };
        let Some(endpoint) = emission.endpoint else { return };
        let mut rows: Vec<_> = self
            .context
            .primary_scenery_targets
            .iter()
            .map(|target| ChannelTargetRow {
                target: target.clone(),
                kind: ChannelTargetKind::Scenery,
            })
            .collect();
        rows.extend(water_target_rows(self.enemies));
        let targets: Vec<_> = rows.iter().map(|row| row.target.clone()).collect();
        let polygon = native_weld_blizzard_contact_polygon(
            endpoint,
            emission.origin,
            emission.underpowered,
            profile.vector.values[6],
        );
        let roots = native_primary_polygon_targets(0x1086, &polygon.points, &targets);
        let mut contacted = BTreeSet::new();
        let mut chain_seeds = Vec::new();
        if emission.terrain_contact {
            self.publish_blizzard_glow(emission, endpoint)
        }
        for root in roots {
            let row =
                rows.iter().find(|row| row.target.id == root.id).expect("Blizzard polygon row");
            if self.apply_channel_projectile_force(row, 100.0, emission.direction) {
                continue;
            }
            contacted.insert(root.id.clone());
            self.publish_blizzard_glow(emission, root.position);
            let ChannelTargetKind::Enemy { actor_id } = row.kind else { continue };
            if !native_primary_target_eligible(root, 0x2) {
                continue;
            }
            self.weld_cold(
                actor_id,
                if emission.underpowered { 0.75 } else { NATIVE_WELD_FROST_SLOW_FACTOR },
                25,
            );
            let stun = profile.vector.values[3];
            if !emission.underpowered && stun < 1.0 {
                self.target_effect(
                    actor_id,
                    NativeSecondaryTargetEffectPatch {
                        stun_factor: Some(stun),
                        stun_ticks: Some(25),
                        ..NativeSecondaryTargetEffectPatch::default()
                    },
                );
            }
            let amount = emission.damage
                * self.multiplier(actor_id, BoneyardSpellHitKind::Air, &emission.owner_id);
            let damaged = self.damage(actor_id, amount, &emission.owner_id, true);
            if damaged.accepted {
                self.hit(
                    actor_id,
                    amount,
                    &emission.owner_id,
                    emission.id,
                    BoneyardSpellHitKind::Weld,
                    damaged.killed,
                )
            }
            if !emission.underpowered && profile.vector.values[5] > 0.0 {
                self.apply_blizzard_pushback(
                    actor_id,
                    root.actor_flags,
                    emission.query_origin,
                    profile.vector.values[5],
                );
            }
            if !emission.underpowered && profile.vector.values[2] > 0.0 {
                chain_seeds.push(root.id.clone())
            }
        }
        for seed in chain_seeds {
            let Some(mut source) = target_rows(self.enemies)
                .into_iter()
                .find(|row| row.target.id == seed)
                .map(|row| row.target)
            else {
                continue;
            };
            let mut damage = fround(emission.damage * 0.6000000238418579);
            for _ in 0..profile.vector.values[2].ceil() as usize {
                let Some(id) = nearest_unused_air_chain_target(
                    self.enemies,
                    source.position,
                    &contacted,
                    100.0,
                ) else {
                    break;
                };
                let Some(row) =
                    target_rows(self.enemies).into_iter().find(|row| row.target.id == id)
                else {
                    break;
                };
                if !native_primary_target_eligible(&row.target, 0x2) {
                    break;
                }
                contacted.insert(row.target.id.clone());
                let direction = normalized_difference(source.position, row.target.position);
                let effects = create_native_weld_blizzard_chain_effects(
                    channel_birth(
                        emission,
                        self.next_id,
                        self.context.tick,
                        source.position,
                        direction,
                    ),
                    emission.direction,
                    self.rng,
                    self.order,
                );
                self.next_id = effects.next_id;
                let fade =
                    self.enroll_weld_combat_actor(NativeWeldWorldActor::FrostFade(effects.fade));
                self.owned_transients.push(fade);
                if let Some(chain) = effects.chain_frost {
                    let effect = self
                        .enroll_weld_combat_actor(NativeWeldWorldActor::BlizzardChainFrost(chain));
                    self.owned_transients.push(effect);
                }
                self.weld_cold(row.actor_id, NATIVE_WELD_FROST_SLOW_FACTOR, 25);
                let stun = profile.vector.values[3];
                if stun < 1.0 {
                    self.target_effect(
                        row.actor_id,
                        NativeSecondaryTargetEffectPatch {
                            stun_factor: Some(stun),
                            stun_ticks: Some(25),
                            ..NativeSecondaryTargetEffectPatch::default()
                        },
                    )
                }
                let amount = damage
                    * self.multiplier(row.actor_id, BoneyardSpellHitKind::Air, &emission.owner_id);
                let damaged = self.damage(row.actor_id, amount, &emission.owner_id, true);
                if damaged.accepted {
                    self.hit(
                        row.actor_id,
                        amount,
                        &emission.owner_id,
                        emission.id,
                        BoneyardSpellHitKind::Weld,
                        damaged.killed,
                    )
                }
                source = row.target;
                damage = fround(damage * 0.6000000238418579);
            }
        }
    }

    fn publish_blizzard_glow(&mut self, emission: &PrimarySpellChannelEmission, position: Vec2) {
        let actor = create_native_weld_blizzard_contact_glow(
            channel_birth(emission, self.next_id, self.context.tick, position, emission.direction),
            self.rng,
            self.order,
        );
        let effect = self.enroll_weld_combat_actor(NativeWeldWorldActor::BlizzardGlow(actor));
        self.owned_transients.push(effect);
        self.next_id += 1;
    }

    fn resolve_steam_pushback(&mut self, emission: &PrimarySpellChannelEmission) {
        let NativePrimarySkillProfile::Weld(profile) = &emission.primary_skill else {
            unreachable!()
        };
        let widen = if emission.underpowered { 0.0 } else { profile.vector.values[2] };
        let pushback = if emission.underpowered { 0.0 } else { profile.vector.values[3] };
        if pushback <= 0.0 {
            return;
        }
        let rows = water_target_rows(self.enemies);
        let targets: Vec<_> = rows.iter().map(|row| row.target.clone()).collect();
        let first_contact = self.context.first_world_contact;
        let line_of_sight = |target: &PrimarySpellTarget| {
            first_contact.is_none_or(|contact| {
                contact(emission.query_origin, target.position, 0.0).is_none()
            })
        };
        let reach = 205.0 + 4.0 * widen;
        for target in native_primary_cone_targets(PrimaryConeQuery {
            actor_mask: 0x1082,
            aim_direction: emission.direction,
            half_angle_degrees: 15.0 + widen * 0.5,
            has_line_of_sight: &line_of_sight,
            origin: emission.query_origin,
            reach,
            targets: &targets,
        }) {
            let row = rows.iter().find(|row| row.target.id == target.id).expect("Steam cone row");
            if self.apply_channel_projectile_force(
                row,
                fround(pushback * 0.32_f32 as f64),
                emission.direction,
            ) {
                continue;
            }
            let ChannelTargetKind::Enemy { actor_id } = row.kind else { continue };
            self.apply_water_pushback(
                actor_id,
                &row.target,
                emission.query_origin,
                pushback,
                reach,
                1.0,
            );
        }
    }

    fn apply_blizzard_pushback(
        &mut self,
        actor_id: usize,
        actor_flags: u32,
        origin: Vec2,
        pushback: f64,
    ) {
        let Some(row) = target_rows(self.enemies).into_iter().find(|row| row.actor_id == actor_id)
        else {
            return;
        };
        let direction = normalized_difference(origin, row.target.position);
        let probe = Vec2::new(
            fround(row.target.position.x + direction.x * 30.0),
            fround(row.target.position.y + direction.y * 30.0),
        );
        let targets: Vec<_> = self
            .enemies
            .spell_targets()
            .into_iter()
            .filter(|target| target.id != row.target.id)
            .collect();
        if first_native_primary_point_contact(PrimaryPointContactQuery {
            actor_mask: 0x2,
            position: probe,
            query_radius: 25.0 / 3.0,
            targets: &targets,
        })
        .is_some()
        {
            return;
        }
        let (previous, last) =
            if let Some(actor) = self.enemies.actors.iter().find(|actor| actor.id == actor_id) {
                (actor.blizzard_push_accumulator, actor.blizzard_push_last_tick)
            } else {
                let actor = self
                    .enemies
                    .maggots
                    .iter()
                    .find(|actor| actor.id == actor_id)
                    .expect("queried Blizzard Maggot");
                (actor.blizzard_push_accumulator, actor.blizzard_push_last_tick)
            };
        let recent = last.is_some_and(|last| self.context.tick - last <= 3);
        let gain = fround(pushback * 0.1_f32 as f64);
        let cap = fround(pushback * 5.0).min(2.0);
        let accumulator = if recent { fround(fround(previous + gain).min(cap)) } else { 0.0 };
        self.enemies.set_blizzard_push_state(actor_id, accumulator, self.context.tick);
        if accumulator == 0.0 {
            return;
        }
        let effective =
            if actor_flags & 0x40 != 0 { fround(pushback * 0.05000000074505806) } else { pushback };
        let magnitude = fround(fround(effective * 2.5) * accumulator);
        let requested = Vec2::new(
            fround(row.target.position.x + direction.x * magnitude),
            fround(row.target.position.y + direction.y * magnitude),
        );
        let resolved = (self.context.resolve_enemy_movement)(
            actor_id,
            row.target.position,
            requested,
            row.target.body_radius,
        );
        self.enemies.position_enemy(actor_id, resolved);
    }
}
