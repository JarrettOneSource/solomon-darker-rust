//! Source phase composition through the actual Boneyard and player owners.
use serde_json::{Value, json};
use solomon_sim::boneyard::arena::{ArenaPhase, ArenaTransition};
use solomon_sim::boneyard::navigation::NavigationSet;
use solomon_sim::boneyard::*;
use solomon_sim::enemies::config::EnemyArenaScalars;
use solomon_sim::enemies::model::EnemyLifeState;
use solomon_sim::enemies::path::EnemyNavigation;
use solomon_sim::enemies::world::{
    EnemyGeometry, EnemyMovementRequest, EnemyProjectileWorldQuery, EnemySpawnContext,
};
use solomon_sim::enemies::{EnemySpawnIntent, EnemyToken};
use solomon_sim::painter::{PainterRegistration, WorldManagerLane, WorldPainterOrder};
use solomon_sim::player::PlayerState;
use solomon_sim::profile::{NewGameSelection, WizardDiscipline, WizardElement};
use solomon_sim::secondary::NativeSecondarySimulationState;
use solomon_sim::spells::PrimarySpellInput;
use solomon_sim::spells::boneyard::*;
use solomon_sim::spells::model::PrimaryTransientRef;
use solomon_sim::spells::model::{PrimaryProjectileRef, PrimarySpellProjectilePhase, SpellState};
use solomon_sim::spells::targeting::{
    PrimarySpellTarget, PrimarySpellTargetKind, native_primary_view_ray_endpoint,
};
use solomon_sim::{NativeRng, Vec2};

#[path = "support/website_rows.rs"]
mod website_rows;

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/website/boneyard-primary-goldens.json")).unwrap()
}
fn vector(value: &Value) -> Vec2 {
    Vec2::new(value["x"].as_f64().unwrap(), value["y"].as_f64().unwrap())
}
fn point(value: Vec2) -> [f64; 2] {
    [value.x, value.y]
}
fn bounds(value: &Value) -> BoneyardBounds {
    BoneyardBounds::new(
        value["x"].as_f64().unwrap(),
        value["y"].as_f64().unwrap(),
        value["w"].as_f64().unwrap(),
        value["h"].as_f64().unwrap(),
    )
}
fn registration(value: PainterRegistration) -> Value {
    json!([
        if value.manager_lane == WorldManagerLane::Actor { "actor" } else { "transient" },
        value.registration_ordinal
    ])
}
fn loaded(scene: &Value) -> LoadedBoneyard {
    let objects = scene["objects"]
        .as_array()
        .unwrap()
        .iter()
        .map(|object| BoneyardObject {
            eid: object["eid"].as_str().unwrap().into(),
            type_id: object["typeId"].as_u64().unwrap() as u32,
            position: vector(&object["pos"]),
            variant: object["variant"].as_u64().map(|value| value as u32),
            rotation_degrees: None,
            scale: None,
            sort_bias: None,
            atlas_entry: None,
            secondary_atlas_entry: None,
            secondary_variant: None,
            secondary_visible: None,
            overlay_atlas_entry: None,
            overlay_variant: None,
            atlas_entries: None,
        })
        .collect();
    let fences = scene["fences"]
        .as_array()
        .unwrap()
        .iter()
        .map(|fence| BoneyardFence {
            eid: fence["eid"].as_str().unwrap().into(),
            type_id: fence["typeId"].as_u64().unwrap() as u32,
            points: fence["points"].as_array().unwrap().iter().map(vector).collect(),
            style: fence["style"].as_u64().map(|value| value as u32),
            segment_code: None,
            start_post_variant: None,
            end_post_variant: None,
        })
        .collect();
    LoadedBoneyard {
        choice_id: "oracle".into(),
        choice_source: solomon_sim::boneyard::BoneyardSource::Mod,
        mod_id: None,
        mod_name: None,
        name: "Primary adapter oracle".into(),
        run_id: "adapter".into(),
        seed: "primary-adapter".into(),
        source_sha256: "oracle".into(),
        geometry_sha256: "oracle".into(),
        scene: BoneyardScene {
            name: "Primary adapter oracle".into(),
            environment_mode: 0,
            bounds: bounds(&scene["bounds"]),
            spawn: BoneyardSpawn { position: vector(&scene["spawn"]), facing_degrees: 90.0 },
            objects,
            fences,
            roads: Vec::new(),
            sprites: Vec::new(),
            terrain: Vec::new(),
            solomon_dig: None,
        }
        .into(),
    }
}
fn scenery_row(target: &PrimarySpellTarget) -> Value {
    json!([
        target.id,
        point(target.position),
        target.body_radius,
        if target.kind == PrimarySpellTargetKind::Gravestone { "gravestone" } else { "scenery" },
        target.registration_order,
        target.cell_binding_order,
        target.native_priority
    ])
}

#[test]
fn authored_scenery_shared_posts_cleanup_and_view_rays_match_the_source() {
    let fixture = fixture();
    let mut world = BoneyardWorld::new(loaded(&fixture["scene"]));
    let scenery: Vec<_> =
        boneyard_primary_scenery_targets(&world).iter().map(scenery_row).collect();
    website_rows::compare(&json!(scenery), &fixture["scenery"], "scenery");
    let mut arena =
        ArenaTransition::new(world.loaded.scene.bounds, world.loaded.scene.spawn.position);
    arena.phase = ArenaPhase::Sealed;
    arena.combat_bounds = bounds(&fixture["cleanupBounds"]);
    world.arena_transition = Some(arena);
    // Source target arrays survive until the world's final cleanup phase.
    let incoming: Vec<_> =
        boneyard_primary_scenery_targets(&world).iter().map(scenery_row).collect();
    website_rows::compare(&json!(incoming), &fixture["scenery"], "before cleanup");
    world.retire_goodies_after_sealing();
    let scenery: Vec<_> =
        boneyard_primary_scenery_targets(&world).iter().map(scenery_row).collect();
    website_rows::compare(&json!(scenery), &fixture["cleanedScenery"], "cleanedScenery");
    world.arena_transition = None;
    let scenery: Vec<_> =
        boneyard_primary_scenery_targets(&world).iter().map(scenery_row).collect();
    website_rows::compare(&json!(scenery), &fixture["cleanedScenery"], "retained cleanup");
    for ray in fixture["viewRays"].as_array().unwrap() {
        let actual = native_primary_view_ray_endpoint(
            vector(&ray["start"]),
            vector(&ray["direction"]),
            bounds(&ray["view"]),
        );
        website_rows::compare(&json!(point(actual)), &ray["endpoint"], "view endpoint");
    }
}

struct OpenBirthGeometry;
impl EnemyNavigation for OpenBirthGeometry {
    fn direct_path_clear(&self, _: Vec2, _: Vec2, _: f64, _: f64) -> bool {
        true
    }
    fn find_route(&self, _: Vec2, _: Vec2, _: f64, _: f64) -> Option<Vec<Vec2>> {
        None
    }
}
impl EnemyGeometry for OpenBirthGeometry {
    fn resolve_movement(&self, request: EnemyMovementRequest) -> Vec2 {
        request.requested_position
    }
    fn line_obstruction(&self, _: Vec2, _: Vec2, _: u32) -> Option<Vec2> {
        None
    }
    fn projectile_world_blocked(&self, _: EnemyProjectileWorldQuery) -> bool {
        false
    }
}

fn projectile_row(spell: PrimaryProjectileRef<'_>) -> Value {
    let kind = match spell {
        PrimaryProjectileRef::Weld(_) => "weld",
        PrimaryProjectileRef::Earth(_) => "earth",
        PrimaryProjectileRef::Ether(_) => "ether",
        PrimaryProjectileRef::Fire(_) => "fire",
    };
    let common = spell.common();
    json!([
        common.id,
        kind,
        common.age_ticks,
        if common.phase == PrimarySpellProjectilePhase::Flight { "flight" } else { "held" },
        point(common.position),
        common.charge,
        common.damage
    ])
}

fn transient_kind(effect: PrimaryTransientRef<'_>) -> &'static str {
    match effect {
        PrimaryTransientRef::Harden(_) => unreachable!("this fixture has no Harden skill"),
        PrimaryTransientRef::Staff(actor) => actor.kind(),
        PrimaryTransientRef::Weld(_) => unreachable!("this fixture casts the five elements"),
        PrimaryTransientRef::AirHurricane(_) => "air-hurricane",
        PrimaryTransientRef::WaterAura(_) => "water-aura",
        PrimaryTransientRef::WaterHail(_) => "water-hail",
        PrimaryTransientRef::Air(_) => "air",
        PrimaryTransientRef::Water(_) => "water",
        PrimaryTransientRef::EarthBoulderBit(_) => "earth-boulder-bit",
        PrimaryTransientRef::EarthImpact(_) => "earth-impact",
        PrimaryTransientRef::EarthCalledRock(_) => "earth-called-rock",
        PrimaryTransientRef::EtherImpact(_) => "ether-impact",
        PrimaryTransientRef::EtherPierceStreak(_) => "ether-pierce-streak",
        PrimaryTransientRef::EtherBlast(_) => "ether-blast",
        PrimaryTransientRef::FireParticle(_) => "fire",
        PrimaryTransientRef::FireImpact(_) => "fire-impact",
        PrimaryTransientRef::FireEmber(_) => "fire-ember",
        PrimaryTransientRef::FireExplosion(_) => "fire-explosion",
        PrimaryTransientRef::FireGoodImp(_) => "fire-good-imp",
        PrimaryTransientRef::FirePatch(_) => "fire-patch",
    }
}

#[test]
fn boneyard_primary_phases_use_live_equipment_stats_mana_and_enemy_ownership() {
    let fixture = fixture();
    for case in fixture["cases"].as_array().unwrap() {
        let element = WizardElement::ALL
            .into_iter()
            .find(|element| element.as_str() == case["element"])
            .unwrap();
        let mut game_rng = NativeRng::new(7);
        let mut player = PlayerState::new(
            NewGameSelection {
                name: "Oracle".into(),
                element,
                discipline: WizardDiscipline::Arcane,
            },
            &mut game_rng,
        );
        let mut world = BoneyardWorld::new(loaded(&fixture["scene"]));
        if case["boss"] == true {
            player.economy.owned_perk_selectors = vec![22];
        }
        let mut spells = SpellState::default();
        let mut secondary = NativeSecondarySimulationState::new(0);
        if case["boss"] == true {
            solomon_sim::secondary::apply_native_secondary_target_effect(
                &mut secondary.target_effects,
                "boneyard:adapter",
                1,
                solomon_sim::secondary::NativeSecondaryTargetEffectPatch {
                    prismatic_ticks: Some(25),
                    ..solomon_sim::secondary::NativeSecondaryTargetEffectPatch::default()
                },
            );
        }
        let mut rng = NativeRng::new(17);
        let mut order = WorldPainterOrder { next_actor_ordinal: 100, next_transient_ordinal: 70 };
        let navigation = NavigationSet::new("oracle".into(), "oracle".into(), Vec::new());
        let intents = [250.0, 320.0]
            .into_iter()
            .enumerate()
            .map(|(index, x)| {
                let token = if case["boss"] == true && index == 0 {
                    EnemyToken::DemonSkull
                } else {
                    EnemyToken::Skeleton
                };
                EnemySpawnIntent::new(token, Vec2::new(x, 250.0), index + 1, 0, 31)
            })
            .collect::<Vec<_>>();
        world.enemies.spawn(
            &intents,
            EnemySpawnContext {
                players: &[],
                tick: 0,
                arena_scalars: EnemyArenaScalars::default(),
                roll_loot_seed: None,
            },
            &OpenBirthGeometry,
            Some(&mut order),
        );
        let mut wizard = world.spawn_wizard();
        let expected_frames = case["frames"].as_array().unwrap();
        for tick in 0..=170 {
            let frame = BoneyardPrimaryFrame {
                enhanced_effects: true,
                player_light_registration: None,
                owner_id: "hero",
                tick,
                input: PrimarySpellInput {
                    aim: Some(Vec2::new(500.0, 250.0)),
                    primary: tick < 150,
                },
                viewport: Vec2::new(1600.0, 900.0),
                eligible: tick < 160,
                primary_overridden: false,
                navigation: &navigation,
                additional_scenery_targets: &[],
                light_at: &|_| 1.0,
            };
            let cast = step_boneyard_primary_cast(
                &world,
                &mut wizard,
                &mut player,
                &secondary,
                &mut spells,
                frame,
                &mut rng,
                &mut order,
            );
            resolve_boneyard_primary_contacts(
                &mut world,
                &player,
                &mut secondary,
                &mut spells,
                BoneyardPrimaryContactInput { frame, cast: &cast, secondary_deaths: Vec::new() },
                &mut rng,
                &mut order,
            );
            let Some(expected) = expected_frames.iter().find(|expected| expected["tick"] == tick)
            else {
                continue;
            };
            let primary = &wizard.primary_cast;
            let actors: Vec<_> = world
                .enemies
                .actors
                .iter()
                .map(|actor| {
                    json!([
                        actor.id,
                        actor.current_health,
                        if actor.life_state == EnemyLifeState::Alive { "alive" } else { "dying" },
                        point(actor.position)
                    ])
                })
                .collect();
            let effects: Vec<_> = spells
                .transients
                .iter()
                .map(|effect| {
                    json!([effect.common().id, transient_kind(effect), effect.common().age_ticks])
                })
                .collect();
            let secondary_actors: Vec<_> = secondary
                .actors
                .iter()
                .map(|actor| {
                    json!([
                        actor.id,
                        actor.kind.as_str(),
                        actor.damage,
                        actor.target_id,
                        actor.light_registration.map(registration),
                        actor
                            .painter_registrations
                            .iter()
                            .copied()
                            .map(registration)
                            .collect::<Vec<_>>()
                    ])
                })
                .collect();
            let target_effects: Vec<_> = secondary
                .target_effects
                .iter()
                .map(|effect| {
                    json!([effect.target_id, effect.cold_slow_factor, effect.cold_slow_ticks])
                })
                .collect();
            let secondary_targets: Vec<_> = world
                .enemies
                .actors
                .iter()
                .map(|actor| {
                    boneyard_native_secondary_target(&world.enemies, actor.id).map(|target| {
                        json!([
                            target.family,
                            target.id,
                            registration(target.light_registration),
                            target.native_flags,
                            point(target.position),
                            target.radius,
                            target.scale,
                            target.shield_health,
                        ])
                    })
                })
                .collect();
            let actual = json!({
                "tick": tick, "mana": player.combat.current_mana, "spent": cast.mana_spent,
                "underflow": cast.mana_underflow, "heading": wizard.heading_index,
                "cast": [primary.action_tick.unwrap_or(-1.0), primary.cast_sequence, primary.channel_active, primary.emission_sequence, primary.target_id],
                "actors": actors,
                "spells": { "nextId": spells.next_id, "projectiles": spells.projectiles.iter().map(projectile_row).collect::<Vec<_>>(), "transients": effects },
                "secondary": secondary_actors, "targetEffects": target_effects,
                "secondaryTargets": secondary_targets,
                "radialTargets": ([0.0, 100.0, 200.0].map(|radius| {
                    boneyard_native_secondary_targets(&world.enemies, wizard.position, radius)
                        .iter().map(|target| target.id).collect::<Vec<_>>()
                })),
                "rng": { "indexA": rng.index_a, "indexB": rng.index_b, "words": rng.words.as_slice() },
                "order": { "actor": order.next_actor_ordinal, "transient": order.next_transient_ordinal },
            });
            website_rows::compare(&actual, expected, &format!("{} tick{tick}", element.as_str()));
        }
    }
}
