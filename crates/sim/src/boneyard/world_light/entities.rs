//! Actual provider ownership from `core-server/boneyard-world-light.ts`.
//! Each factory receives the retained source fields and the owner's real registration.

use crate::Vec2;
use crate::enemies::model::EnemyDeathEffectKind;
use crate::enemies::projectiles::EnemyProjectileEffectKind;
use crate::enemies::world::EnemyWorld;
use crate::float::fround;
use crate::painter::{PainterRegistration, WorldManagerLane};
use crate::secondary::NativeSecondarySimulationState;
use crate::spells::air::{
    NativeAirLightPlan, build_native_air_contact_light_source, build_native_air_path_light_sources,
};
use crate::spells::model::{PrimarySpellProjectile, PrimarySpellTransient, SpellState};
use crate::spells::weld::{
    NativeWeldMeteorPhase, NativeWeldPersistentActorState, NativeWeldWorldActor,
};

use super::super::lighting::enemy::{
    NativeEnemyLightOwner, NativeEnemyProjectileEffectLightKind, native_enemy_light_sources,
    native_enemy_projectile_effect_light_provider, native_enemy_projectile_light_provider,
};
use super::super::lighting::secondary::{
    native_secondary_misc_light_source_at_age, native_secondary_provider_light_source_at_age,
};
use super::super::lighting::weld::{
    native_weld_meteor_light_source, native_weld_projectile_light_source,
    native_weld_rock_light_source,
};
use super::super::lighting::{
    NativeBoneyardLightSource, ether_primary_impact_light_source, native_boulder_light_source,
    native_fire_ember_light_source, native_fire_explosion_light_source,
    native_fire_good_imp_light_source, native_fire_impact_light_source,
    native_fire_patch_light_source, native_fireball_light_source, native_missile_light_source,
};
use super::{
    BoneyardWorldLightQuery, STOCK_VIEWPORT, WORLD_LIGHT_ZOOM, WorldLightOwner, WorldLightPlayer,
    WorldLightScene, WorldMiscLightOwner, native_primary_view_bounds, native_region_point_gain,
};

#[derive(Clone, Copy, Default)]
pub struct BoneyardLightEnvironment<'a> {
    pub primary_spells: Option<&'a SpellState>,
    pub secondary_abilities: Option<&'a NativeSecondarySimulationState>,
}

/// Logical queries use the accepted tick and current point gain. The renderer supplies its
/// presentation clock, shadow setting and the explosion gains captured by its live views.
pub struct BoneyardEntityLightPresentation<'a> {
    pub frame: u64,
    /// Display-only ages; logical authority queries use the actual actor clocks.
    pub primary_age_ticks: Option<&'a std::collections::BTreeMap<u64, f64>>,
    pub secondary_age_ticks: Option<&'a std::collections::BTreeMap<u64, f64>>,
    pub multiple_shadows: bool,
    pub point_gain_at: &'a dyn Fn(Vec2) -> f64,
    pub fire_explosion_point_gain: &'a dyn Fn(u64, Vec2) -> f64,
    pub ring_fire_explosion_point_gain: &'a dyn Fn(u64, Vec2) -> f64,
}

/// Captures the caller's actual simulation phase. In the source movement step, `scene`
/// retains the incoming camera bounds while lantern/players already hold their new positions.
#[must_use]
pub fn boneyard_world_light_query(
    scene: WorldLightScene,
    run_id: &str,
    players: &[WorldLightPlayer<'_>],
    enemies: &EnemyWorld,
    tick: u64,
    environment: BoneyardLightEnvironment<'_>,
) -> BoneyardWorldLightQuery {
    let cameras: Vec<_> = players
        .iter()
        .map(|player| {
            native_primary_view_bounds(
                scene.camera_bounds,
                player.position,
                player.viewport.unwrap_or(STOCK_VIEWPORT),
                WORLD_LIGHT_ZOOM,
                0.0,
            )
        })
        .collect();
    let point_gain_at = |position| {
        let mut gain: f64 = 0.0;
        for camera in &cameras {
            let center = Vec2::new(camera.x + camera.width / 2.0, camera.y + camera.height / 2.0);
            gain = gain.max(native_region_point_gain(position, center, camera.width, false));
        }
        gain
    };
    let explosion_gain = |_, position| point_gain_at(position);
    let (providers, misc) = collect_boneyard_entity_lights(
        run_id,
        players,
        enemies,
        tick,
        environment,
        BoneyardEntityLightPresentation {
            frame: tick,
            primary_age_ticks: None,
            secondary_age_ticks: None,
            multiple_shadows: true,
            point_gain_at: &point_gain_at,
            fire_explosion_point_gain: &explosion_gain,
            ring_fire_explosion_point_gain: &explosion_gain,
        },
    );
    BoneyardWorldLightQuery::new(scene, players, &providers, &misc, tick)
}

/// Collects actual entity providers before camera admission. Sharing this pass keeps both
/// logical and rendered light fields in native manager order without reusing culled lights.
#[must_use]
pub fn collect_boneyard_entity_lights(
    run_id: &str,
    players: &[WorldLightPlayer<'_>],
    enemies: &EnemyWorld,
    tick: u64,
    environment: BoneyardLightEnvironment<'_>,
    presentation: BoneyardEntityLightPresentation<'_>,
) -> (Vec<WorldLightOwner>, Vec<WorldMiscLightOwner>) {
    let BoneyardEntityLightPresentation {
        frame,
        primary_age_ticks,
        secondary_age_ticks,
        multiple_shadows,
        point_gain_at,
        fire_explosion_point_gain,
        ring_fire_explosion_point_gain,
    } = presentation;
    let mut providers = Vec::new();
    let mut misc = Vec::new();
    for actor in &enemies.actors {
        let sources =
            native_enemy_light_sources(NativeEnemyLightOwner::from(actor), frame, multiple_shadows);
        if !sources.is_empty() {
            providers.push(WorldLightOwner {
                registration: PainterRegistration {
                    manager_lane: WorldManagerLane::Actor,
                    registration_ordinal: actor.registration_ordinal,
                },
                sources,
            });
        }
    }
    for effect in &enemies.death_effects {
        if effect.kind != EnemyDeathEffectKind::BanishBlack || effect.painter_registration.is_none()
        {
            continue;
        }
        let intensity =
            fround(2.0 - effect.age_ticks as f64 * 0.004_999_999_888_241_291).clamp(0.0, 1.0);
        append(
            &mut providers,
            Some(NativeBoneyardLightSource::new(effect.position, 3.0, intensity, multiple_shadows)),
            effect.painter_registration,
        );
    }
    for spell in &enemies.boss_spells {
        append(
            &mut providers,
            super::super::lighting::native_boss_spell_light(spell, frame, multiple_shadows),
            Some(spell.owner.painter_registration),
        );
    }
    let world_key = format!("boneyard:{run_id}");
    if let Some(primary) = environment.primary_spells {
        for spell in &primary.projectiles {
            let common = spell.common();
            if common.world_key != world_key {
                continue;
            }
            let source = match spell {
                PrimarySpellProjectile::Weld(spell) => native_weld_projectile_light_source(
                    spell.build_id,
                    common.id as usize,
                    common.position,
                    frame,
                    multiple_shadows,
                ),
                PrimarySpellProjectile::Earth(_) => {
                    native_boulder_light_source(common.position, common.charge, multiple_shadows)
                }
                PrimarySpellProjectile::Ether(_) => native_missile_light_source(
                    common.id as u32,
                    common.position,
                    frame,
                    multiple_shadows,
                ),
                PrimarySpellProjectile::Fire(_) => native_fireball_light_source(
                    common.id,
                    common.position,
                    frame,
                    multiple_shadows,
                ),
            };
            append(&mut providers, Some(source), Some(common.registration));
        }
    }
    for projectile in &enemies.projectiles {
        let state = projectile.state();
        let source = native_enemy_projectile_light_provider(
            projectile.kind(),
            state.payload,
            state.id,
            state.position,
            frame,
            multiple_shadows,
        );
        append(&mut providers, source.map(|candidate| candidate.source), state.light_registration);
    }
    for effect in &enemies.projectile_effects {
        let state = &effect.state;
        let kind = match effect.kind {
            EnemyProjectileEffectKind::DemonFire { .. } => {
                NativeEnemyProjectileEffectLightKind::DemonFire
            }
            EnemyProjectileEffectKind::DemonExplosionLitArray => {
                NativeEnemyProjectileEffectLightKind::DemonExplosionLitArray
            }
            EnemyProjectileEffectKind::FireBurst => NativeEnemyProjectileEffectLightKind::FireBurst,
            EnemyProjectileEffectKind::GuidedImpact => {
                NativeEnemyProjectileEffectLightKind::GuidedImpact
            }
            _ => NativeEnemyProjectileEffectLightKind::Unlit,
        };
        let source = native_enemy_projectile_effect_light_provider(
            kind,
            state.age_ticks,
            state.alpha,
            state.position,
            state.light_registration.is_some(),
            multiple_shadows,
            point_gain_at(state.position),
        );
        append(&mut providers, source.map(|candidate| candidate.source), state.light_registration);
    }
    if let Some(primary) = environment.primary_spells {
        for effect in &primary.transients {
            let common = effect.common();
            if common.world_key != world_key {
                continue;
            }
            let source = match effect {
                PrimarySpellTransient::Weld(NativeWeldWorldActor::Meteor(actor)) => {
                    native_weld_meteor_light_source(
                        actor.position,
                        actor.fall_height,
                        actor.phase == NativeWeldMeteorPhase::Impact,
                        actor.impact_ticks_remaining,
                        actor.impact_radius_scalar,
                    )
                }
                PrimarySpellTransient::Weld(NativeWeldWorldActor::Persistent(actor)) => {
                    let scale = match actor {
                        NativeWeldPersistentActorState::EtherealBoulder(actor) => actor.scale,
                        NativeWeldPersistentActorState::Hailstones(actor) => actor.scale,
                        NativeWeldPersistentActorState::MeteorField(_) => continue,
                    };
                    Some(native_weld_rock_light_source(
                        actor.common().origin,
                        scale,
                        multiple_shadows,
                    ))
                }
                PrimarySpellTransient::FirePatch(effect) => {
                    if common.age_ticks > 0 {
                        let registration = common
                            .painter_registrations
                            .first()
                            .copied()
                            .expect("Fire patch light has its native painter root");
                        append(
                            &mut providers,
                            Some(native_fire_patch_light_source(
                                effect.position,
                                effect.life,
                                multiple_shadows,
                            )),
                            Some(registration),
                        );
                    }
                    continue;
                }
                PrimarySpellTransient::FireEmber(effect) => Some(native_fire_ember_light_source(
                    common.id,
                    effect.position,
                    effect.life,
                    frame,
                )),
                PrimarySpellTransient::FireExplosion(effect) => native_fire_explosion_light_source(
                    effect.origin,
                    common.age_ticks,
                    fire_explosion_point_gain(common.id, effect.origin),
                    multiple_shadows,
                ),
                PrimarySpellTransient::FireGoodImp(effect) => {
                    Some(native_fire_good_imp_light_source(
                        common.id,
                        effect.position,
                        effect.light_glow,
                        frame,
                    ))
                }
                PrimarySpellTransient::EtherImpact(effect) => {
                    Some(ether_primary_impact_light_source(effect.origin, common.age_ticks))
                }
                PrimarySpellTransient::FireImpact(effect) => {
                    let age = primary_age_ticks
                        .and_then(|ages| ages.get(&common.id))
                        .copied()
                        .unwrap_or(common.age_ticks as f64);
                    Some(native_fire_impact_light_source(effect.origin, age))
                }
                PrimarySpellTransient::Air(effect) => {
                    let source = build_native_air_contact_light_source(
                        common.id,
                        common.age_ticks,
                        Vec2::new(
                            effect.endpoint.x - effect.origin.x,
                            effect.endpoint.y - effect.origin.y,
                        ),
                        effect.origin,
                        effect.underpowered,
                        effect.chained,
                        effect.enhanced_effects,
                    );
                    if common.age_ticks == 0 {
                        let owner = players
                            .iter()
                            .find(|player| player.id == common.owner_id)
                            .and_then(|player| player.light_registration)
                            .expect("Air MiscLight has its player owner registration");
                        misc.push(WorldMiscLightOwner {
                            birth_tick: effect.birth_tick as i64,
                            id: common.id as usize,
                            append_ordinal: 0,
                            owner: WorldLightOwner {
                                registration: owner,
                                sources: build_native_air_path_light_sources(
                                    common.id,
                                    effect.birth_tick,
                                    effect.origin,
                                    effect.midpoint,
                                    effect.endpoint,
                                    effect.underpowered,
                                    effect.enhanced_effects,
                                )
                                .into_iter()
                                .map(air_source)
                                .collect(),
                            },
                        });
                    }
                    source.map(air_source)
                }
                _ => None,
            };
            append(&mut providers, source, common.light_registration);
        }
    }
    let secondary_actors: Vec<_> = environment
        .secondary_abilities
        .into_iter()
        .flat_map(|secondary| secondary.actors.iter())
        .filter(|actor| actor.world_key == world_key)
        .collect();
    for actor in &secondary_actors {
        let age = secondary_age_ticks
            .and_then(|ages| ages.get(&actor.id))
            .copied()
            .unwrap_or(actor.age_ticks as f64);
        append(
            &mut providers,
            native_secondary_provider_light_source_at_age(
                actor,
                age,
                frame,
                multiple_shadows,
                ring_fire_explosion_point_gain(actor.id, actor.position),
            ),
            actor.light_registration,
        );
        if let Some(source) = native_secondary_misc_light_source_at_age(actor, age) {
            let registration = actor
                .light_registration
                .expect("secondary MiscLight has its native owner registration");
            let append_ordinal = actor
                .misc_light_append_ordinal
                .expect("secondary MiscLight retains its native append ordinal");
            misc.push(WorldMiscLightOwner {
                birth_tick: tick as i64 - actor.age_ticks as i64,
                id: actor.id as usize,
                append_ordinal,
                owner: WorldLightOwner { registration, sources: vec![source] },
            });
        }
    }
    for pulse in &enemies.mage_lightning_pulses {
        if pulse.tick != tick {
            continue;
        }
        let mut append_ordinal = 0;
        for actor in &secondary_actors {
            if actor.target_id == Some(pulse.owner_actor_id)
                && native_secondary_misc_light_source_at_age(
                    actor,
                    secondary_age_ticks
                        .and_then(|ages| ages.get(&actor.id))
                        .copied()
                        .unwrap_or(actor.age_ticks as f64),
                )
                .is_some()
            {
                append_ordinal = append_ordinal
                    .max(actor.misc_light_append_ordinal.map_or(0, |ordinal| ordinal + 1));
            }
        }
        misc.push(WorldMiscLightOwner {
            birth_tick: pulse.tick as i64,
            id: pulse.id,
            append_ordinal,
            owner: WorldLightOwner {
                registration: pulse.light_registration,
                sources: build_native_air_path_light_sources(
                    u64::from(pulse.seed),
                    pulse.tick,
                    pulse.source,
                    pulse.midpoint,
                    pulse.endpoint,
                    false,
                    true,
                )
                .into_iter()
                .map(air_source)
                .collect(),
            },
        });
    }
    (providers, misc)
}

fn append(
    providers: &mut Vec<WorldLightOwner>,
    source: Option<NativeBoneyardLightSource>,
    registration: Option<PainterRegistration>,
) {
    if let Some(source) = source {
        providers.push(WorldLightOwner {
            registration: registration.expect("native provider has its owner registration"),
            sources: vec![source],
        });
    }
}

fn air_source(source: NativeAirLightPlan) -> NativeBoneyardLightSource {
    NativeBoneyardLightSource::new(
        source.position,
        source.radius,
        source.intensity,
        source.casts_directional_shadow,
    )
}
