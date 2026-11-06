//! Weld casting: channel emissions, held persistent builds and their release. Native
//! constructors own their random programs; this phase owns only IDs, queue order
//! and the player's shared cast clock.

use crate::spells::weld::{WeldPersistentActorState, WeldWorldActor};

use super::*;
use crate::float::fround;
use crate::hub::HubWizard;
use crate::painter::WorldPainterOrder;
use crate::profile::primary::PrimarySkillProfile;
use crate::profile::{WeldBuildId, WizardElement};
use crate::spells::model::{PrimarySpellTickResult, PrimarySpellTransient, SpellState};
use crate::spells::projectiles::emitter;
use crate::spells::simulation::{
    PrimarySpellCastAuthority, PrimarySpellWorldContext, air_geometry, channel_emission,
};
use crate::spells::targeting::PRIMARY_CAST_TERRAIN_EXCLUSION_MASK;
use crate::{Rng, Vec2};

impl SpellState {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn step_weld_cast(
        &mut self,
        wizard: &mut HubWizard,
        authority: PrimarySpellCastAuthority<'_>,
        underpowered: bool,
        selected_age: Option<u64>,
        context: &PrimarySpellWorldContext<'_>,
        rng: &mut Rng,
        order: &mut WorldPainterOrder,
        result: &mut PrimarySpellTickResult,
    ) {
        let PrimarySkillProfile::Weld(profile) = authority.primary_skill else {
            unreachable!("Weld cast payload")
        };
        let direction = wizard.primary_cast.aim_direction;
        let origin = emitter(wizard, WizardElement::Fire, authority.weapon_kind);
        let birth = WeldActorBirth {
            build_id: profile.build_id,
            direction,
            id: self.next_id,
            origin,
            owner_id: context.owner_id,
            tick: context.tick,
            vector: &profile.vector.values,
            world_key: context.world_key,
        };
        if matches!(
            profile.build_id,
            WeldBuildId::FlameLash | WeldBuildId::BlizzardBeam | WeldBuildId::SteamJet
        ) {
            let flame_camera = if profile.build_id == WeldBuildId::FlameLash {
                draw_flame_camera_displacement(rng, underpowered)
            } else {
                None
            };
            let lightning = (profile.build_id == WeldBuildId::FlameLash)
                .then(|| air_geometry(wizard, origin, direction, context));
            let mut endpoint = lightning.as_ref().map(|geometry| geometry.0);
            let mut terrain_contact = false;
            if profile.build_id == WeldBuildId::BlizzardBeam {
                let view_endpoint = (context.range_endpoint)(origin, direction, 100.0);
                let obstruction = (context.obstruction)(
                    wizard.position,
                    view_endpoint,
                    None,
                    PRIMARY_CAST_TERRAIN_EXCLUSION_MASK,
                );
                endpoint = Some(obstruction.unwrap_or(view_endpoint));
                terrain_contact = obstruction.is_some();
            }
            let steam_cast_factor = if profile.build_id == WeldBuildId::SteamJet {
                authority.cast_progress_factor
            } else {
                1.0
            };
            let mut emission = channel_emission(
                self.next_id,
                authority.primary_skill,
                origin,
                endpoint,
                wizard.position,
                direction,
                underpowered,
                profile.common.mana_cost / 100.0 * steam_cast_factor,
                context,
            );
            // Steam's weak branch replaces this rate multiplier with fixed .5.
            // channel_emission already applied weak half strength to resolved damage.
            if !underpowered {
                emission.damage *= steam_cast_factor;
            }
            if let Some((endpoint, midpoint, target_id)) = &lightning {
                emission.flame_lash_presentation = Some(
                    flame_lash_main_presentation(FlameMainPresentationInput {
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
            if profile.build_id == WeldBuildId::FlameLash {
                // Receiver damage and terminal dispatch precede the factory birth.
                self.next_id += 1;
            } else if profile.build_id == WeldBuildId::SteamJet {
                let obstruction = |start, end| {
                    (context.obstruction)(start, end, None, PRIMARY_CAST_TERRAIN_EXCLUSION_MASK)
                };
                if let Some(actor) = spawn_weld_steam_actor(
                    SpawnWeldSteamInput {
                        birth,
                        damage: channel_damage,
                        query_origin: wizard.position,
                        underpowered,
                        obstruction_point: &obstruction,
                    },
                    rng,
                ) {
                    self.transients.push(PrimarySpellTransient::Weld(WeldWorldActor::Steam(actor)));
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
                let actor = create_weld_channel_actor(WeldChannelInput {
                    birth,
                    endpoint,
                    midpoint,
                    target_id,
                    underpowered,
                    enhanced_effects: context.enhanced_effects,
                    flame_visual: None,
                });
                self.transients.push(PrimarySpellTransient::Weld(WeldWorldActor::Channel(actor)));
                self.next_id += 1;
                let glows = create_weld_blizzard_source_glows(
                    WeldActorBirth { id: self.next_id, ..birth },
                    rng,
                );
                for glow in glows {
                    self.transients
                        .push(PrimarySpellTransient::Weld(WeldWorldActor::BlizzardGlow(glow)));
                    self.next_id += 1;
                }
            }
            if let Some(geometry) = lightning {
                wizard.primary_cast.target_id = geometry.2;
            }
        } else {
            assert!(
                matches!(
                    profile.build_id,
                    WeldBuildId::EtherealBoulder
                        | WeldBuildId::MeteorSwarm
                        | WeldBuildId::Hailstones
                ),
                "persistent Weld build"
            );
            let mut meteor_program = None;
            if profile.build_id == WeldBuildId::MeteorSwarm {
                let center = weld_meteor_target_point(wizard.position, direction);
                let marker =
                    spawn_weld_meteor_marker(WeldActorBirth { origin: center, ..birth }, rng);
                self.transients
                    .push(PrimarySpellTransient::Weld(WeldWorldActor::MeteorMarker(marker)));
                self.next_id += 1;
                let damage = draw_weld_damage(
                    rng,
                    profile.common.damage_minimum,
                    profile.common.damage_maximum,
                );
                meteor_program = Some((center, damage));
            }
            let current = self.transients.iter().position(|effect| {
                let PrimarySpellTransient::Weld(WeldWorldActor::Persistent(actor)) = effect else {
                    return false;
                };
                actor.common().owner.owner_id == context.owner_id
                    && actor.common().build_id == profile.build_id
                    && actor.held()
            });
            let mut update = WeldPersistentUpdate::default();
            let actor = if let Some(index) = current {
                let PrimarySpellTransient::Weld(WeldWorldActor::Persistent(actor)) =
                    self.transients.get_mut(index).expect("selected persistent spell remains live")
                else {
                    unreachable!()
                };
                update = update_weld_persistent_actor(
                    actor,
                    origin,
                    direction,
                    WeldPersistentUpdateOptions {
                        cast_progress_factor: authority.cast_progress_factor,
                        enhanced_effects: true,
                        underpowered,
                    },
                    rng,
                );
                actor.clone()
            } else {
                let actor = create_weld_persistent_actor(
                    WeldActorBirth { id: self.next_id, ..birth },
                    Some(order),
                );
                if matches!(
                    profile.build_id,
                    WeldBuildId::EtherealBoulder | WeldBuildId::Hailstones
                ) {
                    wizard.primary_cast.last_weld_playback_rate =
                        Some(fround(1.5 - rng.draw_scaled_float(0.5_f32 as f64, false)));
                }
                self.next_id += 1;
                self.transients
                    .push(PrimarySpellTransient::Weld(WeldWorldActor::Persistent(actor.clone())));
                actor
            };
            let common = actor.common();
            let child_birth = WeldActorBirth {
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
                let child = create_weld_boulder_debris_actor(
                    WeldActorBirth { id: self.next_id, ..child_birth },
                    debris,
                );
                self.transients
                    .push(PrimarySpellTransient::Weld(WeldWorldActor::BoulderDebris(child)));
                self.next_id += 1;
            }
            for fade in update.hail_rock_fades {
                let child = create_weld_hail_rock_fade_actor(
                    WeldActorBirth { id: self.next_id, ..child_birth },
                    fade.position,
                    fade.rotation_degrees,
                );
                self.transients
                    .push(PrimarySpellTransient::Weld(WeldWorldActor::HailRockFade(child)));
                self.next_id += 1;
            }
            if update.release_requested {
                let id = actor.common().owner.id;
                let release = release_weld_persistent_actor(
                    actor,
                    self.next_id,
                    context.tick,
                    rng,
                    Some(order),
                    context.targets,
                );
                self.next_id = release.next_id;
                self.transients.retain(|effect| effect.common().id != id);
                self.append_weld_release(release);
                wizard.primary_cast.action_tick = None;
                wizard.primary_cast.channel_active = false;
                wizard.primary_cast.target_id = None;
            }
            if profile.build_id == WeldBuildId::MeteorSwarm
                && selected_age.is_some_and(|age| {
                    age.is_multiple_of(weld_meteor_cadence_ticks(
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
                let spawn = create_weld_meteor_spawn_program(
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
                let actor = create_weld_meteor(
                    WeldActorBirth {
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
                self.transients.push(PrimarySpellTransient::Weld(WeldWorldActor::Meteor(actor)));
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
        rng: &mut Rng,
        order: &mut WorldPainterOrder,
        targets: &[crate::spells::targeting::PrimarySpellTarget],
    ) {
        let releasing_hail: Vec<_> = self
            .transients
            .iter()
            .filter_map(|effect| {
                if let PrimarySpellTransient::Weld(WeldWorldActor::Persistent(
                    WeldPersistentActorState::Hailstones(hail),
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
        let mut population = std::mem::take(&mut self.transients);
        population.advance(|effect| {
            if let PrimarySpellTransient::Weld(WeldWorldActor::HailRockFade(fade)) = effect
                && fade.common.owner.owner_id == owner_id
                && releasing_hail.iter().any(|(world, birth)| {
                    fade.common.owner.world_key == *world && fade.common.birth_tick >= *birth
                })
            {
                return (false, Vec::new());
            }
            let PrimarySpellTransient::Weld(WeldWorldActor::Persistent(actor)) = effect else {
                return (true, Vec::new());
            };
            if actor.common().owner.owner_id != owner_id || !actor.held() {
                return (true, Vec::new());
            }
            let parent_id = actor.common().owner.id;
            let release = release_weld_persistent_actor(
                actor.clone(),
                self.next_id,
                tick,
                rng,
                Some(order),
                targets,
            );
            self.next_id = release.next_id;
            let mut actors = release.actors.into_iter();
            let retained = if let Some(parent) = actors.next() {
                // Boulder and Hail retain their first released body and its painter identity.
                assert_eq!(parent.common().owner.id, parent_id);
                *effect = PrimarySpellTransient::Weld(WeldWorldActor::Persistent(parent));
                true
            } else {
                false
            };
            let mut births: Vec<_> = actors
                .map(|actor| PrimarySpellTransient::Weld(WeldWorldActor::Persistent(actor)))
                .collect();
            if let Some(fade) = release.frost_fade {
                births.push(PrimarySpellTransient::Weld(WeldWorldActor::FrostFade(fade)));
            }
            (retained, births)
        });
        self.transients = population;
    }

    fn append_weld_release(&mut self, release: WeldPersistentRelease) {
        self.transients.extend(
            release
                .actors
                .into_iter()
                .map(|actor| PrimarySpellTransient::Weld(WeldWorldActor::Persistent(actor))),
        );
        if let Some(fade) = release.frost_fade {
            self.transients.push(PrimarySpellTransient::Weld(WeldWorldActor::FrostFade(fade)));
        }
    }
}
