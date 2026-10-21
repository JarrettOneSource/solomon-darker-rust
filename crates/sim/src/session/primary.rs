//! One shared cast/flight epoch, followed by the source's later contact epoch.

use super::*;
use crate::Vec2;
use crate::boneyard::BONEYARD_CAMERA_SCALE;
use crate::boneyard::navigation::NavigationSet;
use crate::boneyard::world::boneyard_goodie_spell_targets;
use crate::boneyard::world_light::{
    BoneyardLightEnvironment, BoneyardWorldLightQuery, WorldLightPlayer, WorldLightScene,
    boneyard_world_light_query, native_primary_view_bounds,
};
use crate::enemies::path::EnemyNavigation;
use crate::hub::regions::HubRegion;
use crate::hub::{HUB_CAMERA_SCALE, is_traversable_against_segments};
use crate::line_obstruction::{
    line_capsule_obstruction, line_segment_obstruction, nearer_line_obstruction,
};
use crate::profile::primary::{
    NativePrimarySkillProfile, native_primary_skill_profile_with_stat_book,
};
use crate::score::NativeHallOfFamePlayerScoreState;
use crate::spells::boneyard::{
    BoneyardPrimaryContactEpoch, BoneyardPrimaryContactPlayer, BoneyardPrimaryGeometry,
    boneyard_primary_scenery_targets, resolve_boneyard_primary_contacts_for_players,
};
use crate::spells::casting::NativePlayerWeaponKind;
use crate::spells::contacts::{BoneyardSpellCombatResult, BoneyardSpellDamageNotice};
use crate::spells::model::PrimarySpellTickResult;
use crate::spells::targeting::native_primary_view_ray_endpoint;
use crate::spells::{
    PrimarySpellBatchTickResult, PrimarySpellCastAuthority, PrimarySpellEpochContext,
    PrimarySpellInput, PrimarySpellPlayer, PrimarySpellTraversalActor, PrimarySpellWorldContext,
};
use std::collections::BTreeMap;

impl GameSession {
    pub(super) fn synchronize_air_water_visuals(
        &mut self,
        cast: &PrimarySpellTickResult,
        tick: u64,
    ) {
        use crate::hurricane::native_hurricane_charge_tick;
        use crate::profile::offensive::resolve_native_skill_damage_value;
        use crate::profile::runtime::effective_skill_numeric_value;
        use crate::spells::air_water::{
            AirWaterPlayerVisualOwner, synchronize_air_water_player_visual_actors,
        };

        let mut owners = Vec::with_capacity(self.participants.len());
        for id in &self.participant_order {
            let participant = &self.participants[id];
            let player = &participant.player;
            let runtime = &player.skill_runtime;
            let factors = player.derived_stats().offensive_factors(runtime);
            let wizard = &participant.wizard;
            let hurricane = native_hurricane_charge_tick(
                runtime.hurricane_charge,
                runtime.hurricane_refreshed,
                runtime.hurricane_enabled,
                wizard.primary_cast.channel_active
                    && player.book.primary_skill_id == 24
                    && !wizard.primary_cast.underpowered,
            );
            owners.push(AirWaterPlayerVisualOwner {
                hurricane_charge: hurricane.next_charge,
                hurricane_contact_charge: hurricane.contact_charge,
                hurricane_damage_maximum: resolve_native_skill_damage_value(
                    29,
                    effective_skill_numeric_value(&player.book, 29, "mDamage2"),
                    &factors,
                ),
                hurricane_damage_minimum: resolve_native_skill_damage_value(
                    29,
                    effective_skill_numeric_value(&player.book, 29, "mDamage1"),
                    &factors,
                ),
                owner_id: id.clone(),
                position: wizard.position,
                world_key: self.world_key(id).unwrap(),
            });
        }
        synchronize_air_water_player_visual_actors(
            &mut self.spells,
            &owners,
            tick,
            &mut self.combat_rng,
            &cast.channel_emissions,
            Some(&mut self.painter_order),
            self.enhanced_effects,
        );
    }

    pub(super) fn world_lights(
        &self,
        inputs: &BTreeMap<String, GameSessionInput>,
        tick: u64,
    ) -> BoneyardWorldLightQuery {
        let world = self.boneyard.as_ref().expect("Boneyard logical light query has a world");
        let players: Vec<_> = self
            .participant_order
            .iter()
            .map(|id| {
                let participant = &self.participants[id];
                WorldLightPlayer {
                    id,
                    position: participant.wizard.position,
                    heading_index: participant.wizard.heading_index,
                    weapon_pulse: participant.wizard.primary_cast.weapon_pulse,
                    overlay_effect_phase: participant.lighting.overlay_effect_phase,
                    light_registration: Some(participant.lighting.light_registration),
                    viewport: Some(inputs[id].viewport),
                }
            })
            .collect();
        boneyard_world_light_query(
            WorldLightScene {
                bounds: world.loaded.scene.bounds,
                camera_bounds: world.camera_bounds(),
                lantern_position: world.lantern_position,
                lantern_light_registration: world.lantern_light_registration,
            },
            &world.loaded.run_id,
            &players,
            &world.enemies,
            tick,
            BoneyardLightEnvironment {
                primary_spells: Some(&self.spells),
                secondary_abilities: Some(&self.secondary),
            },
        )
    }

    pub(super) fn step_primary_cast(
        &mut self,
        inputs: &BTreeMap<String, GameSessionInput>,
        tick: u64,
        navigation: Option<&NavigationSet>,
        extensions: Option<&dyn GameSimulationExtensions>,
    ) -> Result<PrimarySpellBatchTickResult, String> {
        // Sample immutable authority before borrowing the canonical characters.
        // These small rows are input values; existing actors remain in one queue.
        let mut samples = Vec::with_capacity(self.participants.len());
        for id in &self.participant_order {
            let participant = &self.participants[id];
            let player = &participant.player;
            let derived = player.derived_stats();
            samples.push(PrimaryPlayerSample {
                id: id.clone(),
                world_key: self.world_key(id).unwrap(),
                focus: participant.wizard.position,
                input: inputs[id],
                primary_unset: self.hub_primary_unset(id),
                profile: native_primary_skill_profile_with_stat_book(
                    &player.book,
                    &player.stat_book,
                    &derived.offensive_factors(&player.skill_runtime),
                ),
                weapon_kind: super::secondary::player_weapon(player),
                alive: player.combat.life_state == crate::combat::PlayerLifeState::Alive,
                available_mana: player.combat.current_mana,
                cast_progress_factor: derived.cast_progress_factor,
                eligible: player.combat.can_cast() && player.progression.pending_offer.is_none(),
                plane_active: self
                    .secondary
                    .players
                    .get(id)
                    .is_some_and(|state| state.planewalker_ticks_remaining > 0),
                region: participant.wizard.region,
            });
        }
        let regions: BTreeMap<_, _> =
            samples.iter().map(|row| (row.id.clone(), row.region)).collect();
        let targets = self.boneyard.as_ref().map_or_else(Vec::new, |world| {
            let mut targets = boneyard_primary_scenery_targets(world);
            targets.extend(boneyard_goodie_spell_targets(world));
            targets.extend(world.enemies.spell_targets());
            targets
        });
        // Source target order is authored scenery, live goodies, then native actor cells.
        let gravestone_ids = self
            .boneyard
            .as_ref()
            .map_or_else(Vec::new, BoneyardWorld::scenery_gravestone_target_ids);
        let gravestones: Vec<_> = gravestone_ids.iter().map(String::as_str).collect();
        let geometry = PrimaryWorldGeometry {
            boneyard: self.boneyard.as_ref().map(|world| BoneyardPrimaryGeometry {
                bounds: world.active_bounds(),
                collision: &world.dynamic_collision,
                static_collision: &world.collision,
                prepared: navigation.expect("Boneyard primary epoch has prepared navigation"),
                gravestones: &gravestones,
            }),
            regions: &regions,
        };
        let can_place = |owner: &str, position, radius| geometry.can_place(owner, position, radius);
        let can_traverse = |actor: PrimarySpellTraversalActor<'_>, start, end, radius, mask| {
            geometry.can_traverse(actor, start, end, radius, mask)
        };
        let obstruction = |owner: &str, start, end, excluded: Option<&str>, mask| {
            geometry.obstruction(owner, start, end, excluded, mask)
        };
        let targets_for_owner = |_: &str| targets.as_slice();
        let navigation =
            geometry.boneyard.as_ref().map(|geometry| geometry as &dyn EnemyNavigation);
        let epoch = PrimarySpellEpochContext {
            tick,
            enhanced_effects: self.enhanced_effects,
            targets_for_owner: &targets_for_owner,
            can_place: &can_place,
            can_traverse: &can_traverse,
            obstruction: &obstruction,
            navigation,
        };
        let scale = if self.boneyard.is_some() { BONEYARD_CAMERA_SCALE } else { HUB_CAMERA_SCALE };
        let bounds = self.boneyard.as_ref().map(BoneyardWorld::camera_bounds);
        let range_queries: Vec<_> = samples
            .iter()
            .map(|sample| {
                move |start, direction, padding| {
                    let bounds = bounds.unwrap_or(crate::boneyard::BoneyardBounds::new(
                        0.0,
                        0.0,
                        sample.region.size().x,
                        sample.region.size().y,
                    ));
                    native_primary_view_ray_endpoint(
                        start,
                        direction,
                        native_primary_view_bounds(
                            bounds,
                            sample.focus,
                            sample.input.viewport,
                            scale,
                            padding,
                        ),
                    )
                }
            })
            .collect();
        let obstruction_queries: Vec<_> = samples
            .iter()
            .map(|sample| {
                let geometry = &geometry;
                move |start, end, excluded: Option<&str>, mask| {
                    geometry.obstruction(&sample.id, start, end, excluded, mask)
                }
            })
            .collect();
        let mut players = Vec::with_capacity(samples.len());
        for ((participant, sample), (range, obstruction)) in
            ordered_participants_mut(&mut self.participants, &self.participant_order)
                .into_iter()
                .zip(&samples)
                .zip(range_queries.iter().zip(&obstruction_queries))
        {
            let authority = (!sample.primary_unset).then_some(PrimarySpellCastAuthority {
                primary_skill: &sample.profile,
                weapon_kind: sample.weapon_kind,
                alive: sample.alive,
                available_mana: sample.available_mana,
                cast_progress_factor: sample.cast_progress_factor,
                eligible: sample.eligible,
                plane_active: sample.plane_active,
            });
            players.push(PrimarySpellPlayer {
                wizard: &mut participant.wizard,
                authority,
                input: PrimarySpellInput { aim: sample.input.aim, primary: sample.input.primary },
                context: PrimarySpellWorldContext {
                    enhanced_effects: self.enhanced_effects,
                    flame_lash_view_bounds: Some(native_primary_view_bounds(
                        bounds.unwrap_or(crate::boneyard::BoneyardBounds::new(
                            0.0,
                            0.0,
                            sample.region.size().x,
                            sample.region.size().y,
                        )),
                        sample.focus,
                        sample.input.viewport,
                        scale,
                        25.0,
                    )),
                    player_light_registration: Some(participant.lighting.light_registration),
                    tick,
                    view_scale: scale,
                    owner_id: &sample.id,
                    world_key: &sample.world_key,
                    targets: &targets,
                    can_place: &can_place,
                    can_traverse: &can_traverse,
                    obstruction,
                    range_endpoint: range,
                    navigation,
                },
            });
        }
        let result = self.spells.step_primary_players(
            &mut players,
            &epoch,
            &mut self.combat_rng,
            Some(&mut self.painter_order),
        );
        // Underflow boasts precede the mana transactions in the source host.
        for player in &result.players {
            if player.mana_underflow {
                self.fail_boast_with_extensions(
                    &player.owner_id,
                    crate::hub::interaction::BoastFailureProducer::ManaUnderflow,
                    extensions,
                );
            }
        }
        for player in &result.players {
            if player.mana_spent > 0.0 {
                super::extensions::apply_filtered_mana_delta(
                    &mut self.participants.get_mut(&player.owner_id).unwrap().player,
                    &player.owner_id,
                    -player.mana_spent,
                    crate::modding::extensions::GameSimulationManaSource::PrimaryCast,
                    tick,
                    extensions,
                    Some("primary spell mana authority diverged"),
                )?;
            }
        }
        Ok(result)
    }

    pub(super) fn resolve_primary_contacts(
        &mut self,
        inputs: &BTreeMap<String, GameSessionInput>,
        tick: u64,
        cast: &PrimarySpellTickResult,
        secondary_deaths: Vec<crate::spells::contacts::BoneyardSpecialDeathResult>,
    ) -> BoneyardSpellCombatResult {
        let light = self.world_lights(inputs, tick);
        let light_at = |position| light.scalar_at(position);
        let scenery = boneyard_goodie_spell_targets(self.boneyard.as_ref().unwrap());
        let players: Vec<_> = self
            .participant_order
            .iter()
            .map(|id| BoneyardPrimaryContactPlayer {
                owner_id: id,
                player: &self.participants[id].player,
                viewport: inputs[id].viewport,
            })
            .collect();
        let result = resolve_boneyard_primary_contacts_for_players(
            self.boneyard.as_mut().unwrap(),
            &players,
            &mut self.secondary,
            &mut self.spells,
            BoneyardPrimaryContactEpoch {
                enhanced_effects: self.enhanced_effects,
                tick,
                cast,
                secondary_deaths,
                additional_scenery_targets: &scenery,
                light_at: &light_at,
            },
            &mut self.combat_rng,
            &mut self.painter_order,
        );
        self.record_spell_damage_notices(&result.damage_notices);
        result
    }

    pub(super) fn record_spell_damage_notices(&mut self, notices: &[BoneyardSpellDamageNotice]) {
        let pulse = self.boneyard.as_ref().unwrap().enemy_world_feedback.accumulator;
        for notice in notices {
            if let BoneyardSpellDamageNotice::LethalReward { enemy, owner_id, .. } = notice
                && let Some(participant) = self.participants.get_mut(owner_id)
            {
                let player = &participant.player;
                participant.score.record_enemy_kill(
                    enemy,
                    NativeHallOfFamePlayerScoreState {
                        current_health: player.combat.current_health,
                        maximum_health: player.combat.maximum_health,
                        level: player.progression.level,
                        score_health_multiplier_enabled: player.combat.life_state
                            == crate::combat::PlayerLifeState::Alive,
                    },
                    pulse,
                    &mut self.gameplay_rng,
                );
            }
        }
    }
}

struct PrimaryWorldGeometry<'a> {
    boneyard: Option<BoneyardPrimaryGeometry<'a>>,
    regions: &'a BTreeMap<String, HubRegion>,
}
impl PrimaryWorldGeometry<'_> {
    fn can_place(&self, owner: &str, position: Vec2, radius: f64) -> bool {
        if let Some(geometry) = &self.boneyard {
            return geometry.can_place(position, radius);
        }
        self.regions.get(owner).is_some_and(|region| {
            is_traversable_against_segments(position, radius, region.segments())
        })
    }
    fn obstruction(
        &self,
        owner: &str,
        start: Vec2,
        end: Vec2,
        excluded: Option<&str>,
        mask: u32,
    ) -> Option<Vec2> {
        if let Some(geometry) = &self.boneyard {
            return geometry.obstruction(start, end, excluded, mask);
        }
        let region = self.regions.get(owner)?;
        let mut nearest = None;
        for segment in region.segments() {
            nearest = nearer_line_obstruction(
                nearest,
                line_segment_obstruction(
                    start,
                    end,
                    Vec2::new(segment.x1, segment.y1),
                    Vec2::new(segment.x2, segment.y2),
                ),
            );
        }
        nearest.map(|hit| hit.point)
    }
    fn can_traverse(
        &self,
        actor: PrimarySpellTraversalActor<'_>,
        start: Vec2,
        end: Vec2,
        radius: f64,
        mask: u32,
    ) -> bool {
        if let Some(geometry) = &self.boneyard {
            return geometry.can_traverse(actor, start, end, radius, mask);
        }
        let owner = match actor {
            PrimarySpellTraversalActor::Projectile(projectile) => &projectile.common().owner_id,
            PrimarySpellTraversalActor::EtherealBoulder(actor) => &actor.common.owner.owner_id,
        };
        let Some(region) = self.regions.get(owner) else {
            return false;
        };
        if radius == 0.0 {
            return self.obstruction(owner, start, end, None, mask).is_none();
        }
        for segment in region.segments() {
            if line_capsule_obstruction(
                start,
                end,
                Vec2::new(segment.x1, segment.y1),
                Vec2::new(segment.x2, segment.y2),
                radius,
            )
            .is_some()
            {
                return false;
            }
        }
        true
    }
}

struct PrimaryPlayerSample {
    id: String,
    world_key: String,
    focus: Vec2,
    input: GameSessionInput,
    primary_unset: bool,
    profile: NativePrimarySkillProfile,
    weapon_kind: Option<NativePlayerWeaponKind>,
    alive: bool,
    available_mana: f64,
    cast_progress_factor: f64,
    eligible: bool,
    plane_active: bool,
    region: HubRegion,
}
