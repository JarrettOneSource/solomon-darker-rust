//! Weld channel and held-slot composition from `primary-spells.ts`. Native
//! constructors own their random programs; this phase owns only IDs, queue order
//! and the player's shared cast clock.

use super::*;
use crate::float::fround;
use crate::hub::HubWizard;
use crate::painter::WorldPainterOrder;
use crate::profile::WizardElement;
use crate::profile::primary::NativePrimarySkillProfile;
use crate::spells::model::{PrimarySpellTickResult, PrimarySpellTransient, SpellState};
use crate::spells::projectiles::emitter;
use crate::spells::simulation::{
    PrimarySpellCastAuthority, PrimarySpellWorldContext, air_geometry, channel_emission,
};
use crate::spells::targeting::NATIVE_PRIMARY_CAST_TERRAIN_EXCLUSION_MASK;
use crate::{NativeRng, Vec2};

impl SpellState {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn step_weld_cast(
        &mut self,
        wizard: &mut HubWizard,
        authority: PrimarySpellCastAuthority<'_>,
        underpowered: bool,
        native_selected_age: Option<u64>,
        context: &PrimarySpellWorldContext<'_>,
        rng: &mut NativeRng,
        order: &mut WorldPainterOrder,
        result: &mut PrimarySpellTickResult,
    ) {
        let NativePrimarySkillProfile::Weld(profile) = authority.primary_skill else {
            unreachable!("Weld cast payload")
        };
        let direction = wizard.primary_cast.aim_direction;
        let origin = emitter(wizard, WizardElement::Fire, authority.weapon_kind);
        let birth = NativeWeldActorBirth {
            build_id: profile.build_id,
            direction,
            id: self.next_id,
            origin,
            owner_id: context.owner_id,
            tick: context.tick,
            vector: &profile.vector.values,
            world_key: context.world_key,
        };
        if matches!(profile.build_id, 1003..=1005) {
            let flame_camera = if profile.build_id == 1003 {
                draw_native_flame_camera_displacement(rng, underpowered)
            } else {
                None
            };
            let lightning = (profile.build_id == 1003)
                .then(|| air_geometry(wizard, origin, direction, context));
            let mut endpoint = lightning.as_ref().map(|geometry| geometry.0);
            let mut terrain_contact = false;
            if profile.build_id == 1004 {
                let view_endpoint = (context.range_endpoint)(origin, direction, 100.0);
                let obstruction = (context.obstruction)(
                    wizard.position,
                    view_endpoint,
                    None,
                    NATIVE_PRIMARY_CAST_TERRAIN_EXCLUSION_MASK,
                );
                endpoint = Some(obstruction.unwrap_or(view_endpoint));
                terrain_contact = obstruction.is_some();
            }
            let mut emission = channel_emission(
                self.next_id,
                authority.primary_skill,
                origin,
                endpoint,
                wizard.position,
                direction,
                underpowered,
                profile.common.mana_cost / 100.0,
                context,
            );
            if let Some((endpoint, midpoint, target_id)) = &lightning {
                emission.flame_lash_presentation = Some(
                    native_flame_lash_main_presentation(NativeFlameMainPresentationInput {
                        origin,
                        aim: direction,
                        endpoint: *endpoint,
                        midpoint: *midpoint,
                        target_id: target_id.clone(),
                        view_bounds: context.flame_lash_view_bounds,
                        creator_registration: context.player_light_registration,
                        camera_displacement: flame_camera,
                    })
                    .expect("native Flame view clipping converges"),
                );
            }
            emission.terrain_contact = terrain_contact;
            let channel_damage = emission.damage;
            result.channel_emissions.push(emission);
            if profile.build_id == 1003 {
                // Receiver damage and terminal dispatch precede the factory birth.
                self.next_id += 1;
            } else if profile.build_id == 1005 {
                let obstruction = |start, end| {
                    (context.obstruction)(
                        start,
                        end,
                        None,
                        NATIVE_PRIMARY_CAST_TERRAIN_EXCLUSION_MASK,
                    )
                };
                if let Some(actor) = spawn_native_weld_steam_actor(
                    SpawnNativeWeldSteamInput {
                        birth,
                        damage: channel_damage,
                        query_origin: wizard.position,
                        underpowered,
                        obstruction_point: &obstruction,
                    },
                    rng,
                ) {
                    self.transients
                        .push(PrimarySpellTransient::Weld(NativeWeldWorldActor::Steam(actor)));
                    self.next_id += 1;
                }
            } else {
                let midpoint = lightning.as_ref().map(|geometry| geometry.1).or_else(|| {
                    endpoint.map(|point| {
                        Vec2::new(
                            fround((origin.x + point.x) * 0.5),
                            fround((origin.y + point.y) * 0.5),
                        )
                    })
                });
                let target_id = lightning.as_ref().and_then(|geometry| geometry.2.clone());
                let actor = create_native_weld_channel_actor(NativeWeldChannelInput {
                    birth,
                    endpoint,
                    midpoint,
                    target_id,
                    underpowered,
                    enhanced_effects: context.enhanced_effects,
                    flame_visual: None,
                });
                self.transients
                    .push(PrimarySpellTransient::Weld(NativeWeldWorldActor::Channel(actor)));
                self.next_id += 1;
                let glows = create_native_weld_blizzard_source_glows(
                    NativeWeldActorBirth { id: self.next_id, ..birth },
                    rng,
                );
                for glow in glows {
                    self.transients.push(PrimarySpellTransient::Weld(
                        NativeWeldWorldActor::BlizzardGlow(glow),
                    ));
                    self.next_id += 1;
                }
            }
            if let Some(geometry) = lightning {
                wizard.primary_cast.target_id = geometry.2;
            }
        } else {
            assert!(matches!(profile.build_id, 1006..=1008), "persistent Weld build");
            let mut meteor_program = None;
            if profile.build_id == 1007 {
                let center = native_weld_meteor_target_point(wizard.position, direction);
                let marker = spawn_native_weld_meteor_marker(
                    NativeWeldActorBirth { origin: center, ..birth },
                    rng,
                );
                self.transients
                    .push(PrimarySpellTransient::Weld(NativeWeldWorldActor::MeteorMarker(marker)));
                self.next_id += 1;
                let damage = draw_native_weld_damage(
                    rng,
                    profile.common.damage_minimum,
                    profile.common.damage_maximum,
                );
                meteor_program = Some((center, damage));
            }
            let current = self.transients.iter().position(|effect| {
                let PrimarySpellTransient::Weld(NativeWeldWorldActor::Persistent(actor)) = effect
                else {
                    return false;
                };
                actor.common().owner.owner_id == context.owner_id
                    && actor.common().build_id == profile.build_id
                    && actor.held()
            });
            let mut update = NativeWeldPersistentUpdate::default();
            let actor = if let Some(index) = current {
                let PrimarySpellTransient::Weld(NativeWeldWorldActor::Persistent(mut actor)) =
                    self.transients[index].clone()
                else {
                    unreachable!()
                };
                update = update_native_weld_persistent_actor(
                    &mut actor,
                    origin,
                    direction,
                    NativeWeldPersistentUpdateOptions {
                        cast_progress_factor: authority.cast_progress_factor,
                        enhanced_effects: true,
                        underpowered,
                    },
                    rng,
                );
                actor
            } else {
                let actor = create_native_weld_persistent_actor(
                    NativeWeldActorBirth { id: self.next_id, ..birth },
                    Some(order),
                );
                if matches!(profile.build_id, 1006 | 1008) {
                    wizard.primary_cast.last_weld_playback_rate =
                        Some(fround(1.5 - rng.draw_scaled_float(0.5_f32 as f64, false)));
                }
                self.next_id += 1;
                actor
            };
            if let Some(index) = current {
                self.transients[index] =
                    PrimarySpellTransient::Weld(NativeWeldWorldActor::Persistent(actor.clone()));
            } else {
                self.transients.push(PrimarySpellTransient::Weld(
                    NativeWeldWorldActor::Persistent(actor.clone()),
                ));
            }
            let common = actor.common();
            let child_birth = NativeWeldActorBirth {
                build_id: common.build_id,
                direction: common.direction,
                id: self.next_id,
                origin: common.origin,
                owner_id: &common.owner.owner_id,
                tick: context.tick,
                vector: &common.vector,
                world_key: &common.owner.world_key,
            };
            for debris in update.debris {
                let child = create_native_weld_boulder_debris_actor(
                    NativeWeldActorBirth { id: self.next_id, ..child_birth },
                    debris,
                );
                self.transients
                    .push(PrimarySpellTransient::Weld(NativeWeldWorldActor::BoulderDebris(child)));
                self.next_id += 1;
            }
            for fade in update.hail_rock_fades {
                let child = create_native_weld_hail_rock_fade_actor(
                    NativeWeldActorBirth { id: self.next_id, ..child_birth },
                    fade.position,
                    fade.rotation_degrees,
                );
                self.transients
                    .push(PrimarySpellTransient::Weld(NativeWeldWorldActor::HailRockFade(child)));
                self.next_id += 1;
            }
            if update.release_requested {
                let id = actor.common().owner.id;
                let release = release_native_weld_persistent_actor(
                    actor,
                    self.next_id,
                    context.tick,
                    rng,
                    Some(order),
                );
                self.next_id = release.next_id;
                self.transients.retain(|effect| effect.common().id != id);
                self.append_weld_release(release);
                wizard.primary_cast.action_tick = None;
                wizard.primary_cast.channel_active = false;
                wizard.primary_cast.target_id = None;
            }
            if profile.build_id == 1007
                && native_selected_age.is_some_and(|age| {
                    age.is_multiple_of(native_weld_meteor_cadence_ticks(
                        authority.cast_progress_factor,
                        underpowered,
                    ))
                })
            {
                let (center, damage) =
                    meteor_program.expect("Meteor keeps its damage and target program");
                let resolve = |candidate| {
                    (context.obstruction)(center, candidate, None, 0).unwrap_or(candidate)
                };
                let spawn = create_native_weld_meteor_spawn_program(
                    direction,
                    center,
                    &resolve,
                    underpowered,
                    &profile.vector.values,
                    rng,
                );
                let vector: Vec<_> = profile
                    .vector
                    .values
                    .iter()
                    .enumerate()
                    .map(|(index, value)| if underpowered && index >= 5 { 0.0 } else { *value })
                    .collect();
                let actor = create_native_weld_meteor(
                    NativeWeldActorBirth {
                        id: self.next_id,
                        origin: spawn.position,
                        vector: &vector,
                        ..birth
                    },
                    spawn,
                    fround(damage * if underpowered { 0.5 } else { 1.0 }),
                    underpowered,
                    Some(order),
                );
                self.transients
                    .push(PrimarySpellTransient::Weld(NativeWeldWorldActor::Meteor(actor)));
                self.next_id += 1;
            }
        }
        if underpowered && context.tick.is_multiple_of(50) {
            wizard.primary_cast.fizzle_sequence += 1;
        }
        wizard.primary_cast.underpowered = underpowered;
    }

    pub(crate) fn release_owned_weld_persistent_actors(
        &mut self,
        owner_id: &str,
        tick: u64,
        rng: &mut NativeRng,
        order: &mut WorldPainterOrder,
    ) {
        let releasing_hail: Vec<_> = self
            .transients
            .iter()
            .filter_map(|effect| {
                if let PrimarySpellTransient::Weld(NativeWeldWorldActor::Persistent(
                    NativeWeldPersistentActorState::Hailstones(hail),
                )) = effect
                    && hail.common.owner.owner_id == owner_id
                    && hail.phase == crate::spells::model::PrimarySpellProjectilePhase::Held
                {
                    Some((hail.common.owner.world_key.clone(), hail.common.birth_tick))
                } else {
                    None
                }
            })
            .collect();
        for effect in std::mem::take(&mut self.transients) {
            if let PrimarySpellTransient::Weld(NativeWeldWorldActor::HailRockFade(fade)) = &effect
                && fade.common.owner.owner_id == owner_id
                && releasing_hail.iter().any(|(world, birth)| {
                    fade.common.owner.world_key == *world && fade.common.birth_tick >= *birth
                })
            {
                continue;
            }
            if let PrimarySpellTransient::Weld(NativeWeldWorldActor::Persistent(actor)) = effect {
                if actor.common().owner.owner_id == owner_id && actor.held() {
                    let release = release_native_weld_persistent_actor(
                        actor,
                        self.next_id,
                        tick,
                        rng,
                        Some(order),
                    );
                    self.next_id = release.next_id;
                    self.append_weld_release(release);
                } else {
                    self.transients
                        .push(PrimarySpellTransient::Weld(NativeWeldWorldActor::Persistent(actor)));
                }
            } else {
                self.transients.push(effect);
            }
        }
    }

    fn append_weld_release(&mut self, release: NativeWeldPersistentRelease) {
        self.transients.extend(
            release
                .actors
                .into_iter()
                .map(|actor| PrimarySpellTransient::Weld(NativeWeldWorldActor::Persistent(actor))),
        );
        if let Some(fade) = release.frost_fade {
            self.transients
                .push(PrimarySpellTransient::Weld(NativeWeldWorldActor::FrostFade(fade)));
        }
    }
}
