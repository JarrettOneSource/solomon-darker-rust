//! Source transactions through actual enemy acceptance, shared RNG and painter owners.
use serde_json::{Map, Value, json};
use solomon_sim::enemies::config::EnemyArenaScalars;
use solomon_sim::enemies::model::{EnemyActorRead, EnemyLifeState};
use solomon_sim::enemies::path::EnemyNavigation;
use solomon_sim::enemies::projectile_emission::{SpawnEnemyProjectileOptions, spawn_projectile};
use solomon_sim::enemies::projectiles::EnemyProjectileKind;
use solomon_sim::enemies::projectiles::EnemyProjectileRead;
use solomon_sim::enemies::silk::{EnemySilkActor, create_native_silk, step_native_silk};
use solomon_sim::enemies::world::{
    EnemyGeometry, EnemyMovementRequest, EnemyProjectileWorldQuery, EnemySpawnContext, EnemyWorld,
};
use solomon_sim::enemies::{EnemySpawnIntent, EnemyToken};
use solomon_sim::painter::{PainterRegistration, WorldManagerLane, WorldPainterOrder};
use solomon_sim::profile::primary::*;
use solomon_sim::profile::weld::{NativeWeldCastKind, NativeWeldPrimaryVector};
use solomon_sim::secondary::NativeSecondaryTargetEffectPatch;
use solomon_sim::spells::contacts::*;
use solomon_sim::spells::model::*;
use solomon_sim::spells::targeting::{
    PrimarySpellQueryLane, PrimarySpellTarget, PrimarySpellTargetKind,
};
use solomon_sim::{NativeRng, Vec2};

#[path = "support/website_rows.rs"]
mod website_rows;
#[path = "primary_contact_goldens/weld.rs"]
mod weld;
#[path = "weld_step_goldens/rows.rs"]
mod weld_rows;

struct OpenGeometry;
impl EnemyNavigation for OpenGeometry {
    fn direct_path_clear(&self, _: Vec2, _: Vec2, _: f64, _: f64) -> bool {
        true
    }
    fn find_route(&self, _: Vec2, _: Vec2, _: f64, _: f64) -> Option<Vec<Vec2>> {
        None
    }
}
impl EnemyGeometry for OpenGeometry {
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

fn number(value: &Value, key: &str) -> f64 {
    value[key].as_f64().unwrap_or_else(|| panic!("missing number {key}"))
}
fn integer(value: &Value, key: &str) -> u64 {
    value[key].as_u64().unwrap_or_else(|| panic!("missing integer {key}"))
}
fn text(value: &Value, key: &str) -> String {
    value[key].as_str().unwrap_or_else(|| panic!("missing text {key}")).into()
}
fn vector(value: &Value) -> Vec2 {
    Vec2::new(number(value, "x"), number(value, "y"))
}
fn point(value: Vec2) -> [f64; 2] {
    [value.x, value.y]
}
fn values<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value[key].as_array().map_or(&[], Vec::as_slice)
}
fn registration(value: &Value) -> PainterRegistration {
    PainterRegistration {
        manager_lane: if value["managerLane"] == "actor" {
            WorldManagerLane::Actor
        } else {
            WorldManagerLane::Transient
        },
        registration_ordinal: integer(value, "registrationOrdinal") as usize,
    }
}
fn registration_row(value: PainterRegistration) -> Value {
    json!([
        if value.manager_lane == WorldManagerLane::Actor { "actor" } else { "transient" },
        value.registration_ordinal
    ])
}
fn random_row(value: &NativeRng) -> Value {
    json!({"indexA": value.index_a, "indexB": value.index_b, "words": value.words.as_slice()})
}
fn transient_common(value: &Value) -> PrimarySpellTransientBase {
    PrimarySpellTransientBase {
        age_ticks: integer(value, "ageTicks"),
        id: integer(value, "id"),
        light_registration: value["lightRegistration"]
            .as_object()
            .map(|_| registration(&value["lightRegistration"])),
        owner_id: text(value, "ownerId"),
        painter_registrations: values(value, "painterRegistrations")
            .iter()
            .map(registration)
            .collect(),
        world_key: text(value, "worldKey"),
    }
}
fn projectile(value: &Value) -> PrimarySpellProjectile {
    let common = PrimarySpellProjectileBase {
        age_ticks: integer(value, "ageTicks"),
        charge: number(value, "charge"),
        damage: number(value, "damage"),
        direction: vector(&value["direction"]),
        flight_ticks: integer(value, "flightTicks"),
        id: integer(value, "id"),
        registration: registration(&value["lightRegistration"]),
        owner_id: text(value, "ownerId"),
        phase: PrimarySpellProjectilePhase::Flight,
        position: vector(&value["position"]),
        velocity: vector(&value["velocity"]),
        world_key: text(value, "worldKey"),
    };
    match value["kind"].as_str().unwrap() {
        "weld" => PrimarySpellProjectile::Weld(weld::projectile(value, common)),
        "earth" => PrimarySpellProjectile::Earth(PrimarySpellEarthProjectile {
            common,
            program: EarthProjectileProgram {
                assembly_charge: number(value, "assemblyCharge"),
                hit_target_ids: values(value, "hitTargetIds")
                    .iter()
                    .map(|id| id.as_str().unwrap().into())
                    .collect(),
                maximum_charge: number(value, "maximumCharge"),
                orientation: std::array::from_fn(|i| value["orientation"][i].as_f64().unwrap()),
                remaining_damage: number(value, "remainingDamage"),
                shell_charge: number(value, "shellCharge"),
                toughness: number(value, "toughness"),
            },
        }),
        "ether" => PrimarySpellProjectile::Ether(PrimarySpellEtherProjectile {
            common,
            program: EtherProjectileProgram {
                damage_retention: number(value, "damageRetention"),
                heading_degrees: number(value, "headingDegrees"),
                pierces_remaining: integer(value, "piercesRemaining") as u32,
                reacquires_target: value["reacquiresTarget"].as_bool().unwrap(),
                speed: number(value, "speed"),
                target_id: value["targetId"].as_str().map(String::from),
                turn_input: number(value, "turnInput"),
                turn_accumulator: number(value, "turnAccumulator"),
                underpowered: value["underpowered"].as_bool().unwrap(),
                visual_scale: number(value, "visualScale"),
            },
        }),
        "fire" => PrimarySpellProjectile::Fire(PrimarySpellFireProjectile {
            common,
            program: FireProjectileProgram {
                burn_damage: number(value, "burnDamage"),
                ember_damage: number(value, "emberDamage"),
                ember_fragments: integer(value, "emberFragments") as u32,
                explode_damage: number(value, "explodeDamage"),
                explode_radius: number(value, "explodeRadius"),
                private_seed: number(value, "privateSeed") as i32,
                spent_ember: NativeSpentEmber::None,
                underpowered: value["underpowered"].as_bool().unwrap(),
            },
        }),
        other => panic!("unexpected contact fixture projectile {other}"),
    }
}
fn transient(value: &Value) -> PrimarySpellTransient {
    if value["kind"].as_str().unwrap().starts_with("weld-") {
        return PrimarySpellTransient::Weld(weld::actor(value));
    }
    let common = transient_common(value);
    match value["kind"].as_str().unwrap() {
        "air-hurricane" => PrimarySpellTransient::AirHurricane(PrimarySpellAirHurricane {
            common,
            birth_tick: integer(value, "birthTick"),
            charge: number(value, "charge"),
            contact_charge: number(value, "contactCharge"),
            damage_maximum: number(value, "damageMaximum"),
            damage_minimum: number(value, "damageMinimum"),
            enhanced_effects: value["enhancedEffects"].as_bool().unwrap(),
            lanes: std::array::from_fn(|index| {
                let lane = &value["lanes"][index];
                solomon_sim::hurricane::NativeHurricaneLane {
                    angle_degrees: number(lane, "angleDegrees"),
                    angular_velocity_degrees: number(lane, "angularVelocityDegrees"),
                    radius: number(lane, "radius"),
                    vertical_offset: number(lane, "verticalOffset"),
                }
            }),
            phase_degrees: number(value, "phaseDegrees"),
            position: vector(&value["position"]),
        }),
        "air" => PrimarySpellTransient::Air(PrimarySpellAirTransient {
            common,
            chained: value["chained"].as_bool().unwrap_or(false),
            enhanced_effects: value["enhancedEffects"].as_bool().unwrap_or(true),
            birth_tick: integer(value, "birthTick"),
            direction: vector(&value["direction"]),
            endpoint: vector(&value["endpoint"]),
            hurricane_charge: number(value, "hurricaneCharge"),
            midpoint: vector(&value["midpoint"]),
            origin: vector(&value["origin"]),
            target_id: value["targetId"].as_str().map(String::from),
            underpowered: value["underpowered"].as_bool().unwrap(),
            variant: integer(value, "variant") as u32,
        }),
        "ether-blast" => PrimarySpellTransient::EtherBlast(PrimarySpellEtherBlast {
            common,
            birth_tick: integer(value, "birthTick"),
            charges: integer(value, "charges") as u32,
            origin: vector(&value["origin"]),
            presentation_rng: NativeRng::new(9),
        }),
        "fire-ember" => PrimarySpellTransient::FireEmber(PrimarySpellFireEmber {
            common,
            burn_damage: number(value, "burnDamage"),
            contact_cadence: integer(value, "contactCadence"),
            contact_due: value["contactDue"].as_bool().unwrap(),
            damage: number(value, "damage"),
            height: number(value, "height"),
            horizontal_velocity: vector(&value["horizontalVelocity"]),
            life: number(value, "life"),
            phase: number(value, "phase"),
            position: vector(&value["position"]),
            spent_ember: NativeSpentEmber::None,
            vertical_velocity: number(value, "verticalVelocity"),
        }),
        "fire-explosion" => PrimarySpellTransient::FireExplosion(PrimarySpellFireExplosion {
            common,
            burn_damage: number(value, "burnDamage"),
            damage: number(value, "damage"),
            footprint_dimension: number(value, "footprintDimension"),
            origin: vector(&value["origin"]),
            presentation: if value["presentation"] == "steam" {
                FireExplosionPresentation::Steam
            } else {
                FireExplosionPresentation::Fire
            },
            sound_pitch: number(value, "soundPitch"),
            visual_scale: number(value, "visualScale"),
        }),
        other => panic!("unexpected contact fixture transient {other}"),
    }
}
fn channel(value: &Value) -> PrimarySpellChannelEmission {
    let skill = &value["primarySkill"];
    let common = NativePrimarySkillProfileBase {
        damage_maximum: number(skill, "damageMaximum"),
        damage_minimum: number(skill, "damageMinimum"),
        damage_roll_count: number(skill, "damageRollCount"),
        mana_cost: number(skill, "manaCost"),
        rank: integer(skill, "rank") as u32,
        skill_id: integer(skill, "skillId") as u32,
    };
    let (kind, primary_skill) = if value["kind"] == "air" {
        (
            PrimarySpellChannelKind::Air,
            NativePrimarySkillProfile::Air(NativeAirPrimarySkillProfile {
                common,
                arc_count: integer(skill, "arcCount") as u32,
                disintegrate_chance: number(skill, "disintegrateChance"),
                hurricane_damage_maximum: number(skill, "hurricaneDamageMaximum"),
                hurricane_damage_minimum: number(skill, "hurricaneDamageMinimum"),
                stun_movement_factor: number(skill, "stunMovementFactor"),
            }),
        )
    } else if value["kind"] == "weld" {
        let build_id = integer(skill, "buildId") as u32;
        (
            PrimarySpellChannelKind::Weld,
            NativePrimarySkillProfile::Weld(NativeWeldPrimarySkillProfile {
                common,
                build_id,
                cast_kind: NativeWeldCastKind::Channel,
                damage_factor: number(skill, "damageFactor"),
                vector: NativeWeldPrimaryVector {
                    build_id,
                    cast_kind: NativeWeldCastKind::Channel,
                    values: values(&skill["vector"], "values")
                        .iter()
                        .map(|v| v.as_f64().unwrap())
                        .collect(),
                },
            }),
        )
    } else {
        (
            PrimarySpellChannelKind::Water,
            NativePrimarySkillProfile::Water(NativeWaterPrimarySkillProfile {
                common,
                armor_maximum: number(skill, "armorMaximum"),
                armor_per_second: number(skill, "armorPerSecond"),
                aura_radius_scale: number(skill, "auraRadiusScale"),
                aura_movement_factor: number(skill, "auraMovementFactor"),
                aura_slow_factor: number(skill, "auraSlowFactor"),
                cold_duration_ticks: integer(skill, "coldDurationTicks"),
                cold_movement_factor: number(skill, "coldMovementFactor"),
                hail_chance: number(skill, "hailChance"),
                hail_damage_maximum: number(skill, "hailDamageMaximum"),
                hail_damage_minimum: number(skill, "hailDamageMinimum"),
                hail_threshold: integer(skill, "hailThreshold") as u32,
                half_angle_degrees: number(skill, "halfAngleDegrees"),
                minimum_cold_duration_ticks: integer(skill, "minimumColdDurationTicks"),
                pushback_factor: number(skill, "pushbackFactor"),
                reach: number(skill, "reach"),
                slowdown_scale: number(skill, "slowdownScale"),
                widen_half_degrees: number(skill, "widenHalfDegrees"),
            }),
        )
    };
    PrimarySpellChannelEmission {
        flame_lash_presentation: value.get("flameLashPresentation").map(|main| {
            use solomon_sim::spells::weld::NativeFlameLashFactoryInput;
            PrimarySpellFlameLashPresentation {
                camera_displacement: main
                    .get("cameraDisplacement")
                    .map(|point| if point.is_null() { None } else { Some(vector(point)) }),
                midpoint: main["midpoint"].as_object().map(|_| vector(&main["midpoint"])),
                target_id: main["targetId"].as_str().map(String::from),
                native_geometry: main.get("nativeGeometry").map(|geometry| {
                    NativeFlameLashFactoryInput {
                        start: vector(&geometry["start"]),
                        control: vector(&geometry["control"]),
                        end: vector(&geometry["end"]),
                        offset: vector(&geometry["offset"]),
                    }
                }),
                preclip_endpoint: main.get("preclipEndpoint").map(vector),
                creator_registration: main.get("creatorRegistration").map(registration),
            }
        }),
        damage: number(value, "damage"),
        direction: vector(&value["direction"]),
        endpoint: Some(vector(&value["endpoint"])),
        id: integer(value, "id"),
        kind,
        mana_cost: number(value, "manaCost"),
        origin: vector(&value["origin"]),
        owner_id: text(value, "ownerId"),
        primary_skill,
        query_origin: vector(&value["queryOrigin"]),
        terrain_contact: value["terrainContact"].as_bool().unwrap(),
        underpowered: value["underpowered"].as_bool().unwrap(),
        world_key: text(value, "worldKey"),
    }
}
fn fire_contact(value: &Value) -> FireActorContact {
    if value["kind"] == "fire-good-imp" {
        return FireActorContact::GoodImp(FireTargetContact {
            amount: number(value, "amount"),
            owner_id: text(value, "ownerId"),
            spell_id: integer(value, "spellId"),
            target_id: text(value, "targetId"),
            world_key: text(value, "worldKey"),
        });
    }
    let contact = FireRadialContact {
        amount: number(value, "amount"),
        burn_damage: number(value, "burnDamage"),
        owner_id: text(value, "ownerId"),
        position: vector(&value["position"]),
        radius: number(value, "radius"),
        spell_id: integer(value, "spellId"),
        world_key: text(value, "worldKey"),
    };
    if value["kind"] == "fire-ember" {
        FireActorContact::Ember(contact)
    } else {
        FireActorContact::Patch(contact)
    }
}

fn projectile_row(spell: PrimaryProjectileRef<'_>) -> Value {
    if matches!(spell, PrimaryProjectileRef::Weld(_)) {
        return weld_rows::projectile(spell);
    }
    let (kind, detail) = match spell {
        PrimaryProjectileRef::Weld(spell) => (
            "weld",
            json!([spell.build_id, spell.contacts_remaining, spell.target_id, spell.vector]),
        ),
        PrimaryProjectileRef::Earth(spell) => {
            ("earth", json!([spell.remaining_damage, spell.hit_target_ids, spell.shell_charge]))
        }
        PrimaryProjectileRef::Ether(spell) => (
            "ether",
            json!([
                spell.pierces_remaining,
                spell.target_id,
                spell.heading_degrees,
                spell.common.direction.x,
                spell.common.direction.y
            ]),
        ),
        PrimaryProjectileRef::Fire(_) => ("fire", json!([])),
    };
    let common = spell.common();
    json!({"id": common.id,
        "kind": kind,
        "charge": common.charge,
        "damage": common.damage,
        "position": point(common.position),
        "velocity": point(common.velocity),
        "detail": detail})
}
fn transient_row(effect: &PrimarySpellTransient) -> Value {
    if matches!(effect, PrimarySpellTransient::Weld(_)) {
        return weld_rows::transient(effect);
    }
    let (kind, detail) = match effect {
        PrimarySpellTransient::AirHurricane(effect) => (
            "air-hurricane",
            json!({"birthTick":effect.birth_tick,"charge":effect.charge,"contactCharge":effect.contact_charge,
            "damageMinimum":effect.damage_minimum,"damageMaximum":effect.damage_maximum,"enhancedEffects":effect.enhanced_effects,"phaseDegrees":effect.phase_degrees,"position":point(effect.position)}),
        ),
        PrimarySpellTransient::Air(effect) => (
            "air",
            json!({"birthTick": effect.birth_tick, "variant": effect.variant,
            "enhancedEffects": effect.enhanced_effects,
            "targetId": effect.target_id, "origin": point(effect.origin), "direction": point(effect.direction),
            "endpoint": point(effect.endpoint), "midpoint": point(effect.midpoint)}),
        ),
        PrimarySpellTransient::EarthBoulderBit(effect) => (
            "earth-boulder-bit",
            json!({"birthTick": effect.birth_tick, "origin": point(effect.origin), "position": point(effect.position)}),
        ),
        PrimarySpellTransient::EarthImpact(effect) => (
            "earth-impact",
            json!({"birthTick": effect.birth_tick, "charge": effect.charge, "enhancedEffects": effect.enhanced_effects,
                "lifetimeTicks": effect.lifetime_ticks, "origin": point(effect.origin)}),
        ),
        PrimarySpellTransient::EtherBlast(effect) => (
            "ether-blast",
            json!({"birthTick": effect.birth_tick, "charges": effect.charges, "origin": point(effect.origin)}),
        ),
        PrimarySpellTransient::EtherImpact(effect) => (
            "ether-impact",
            json!({"birthTick": effect.birth_tick, "visualScale": effect.visual_scale, "origin": point(effect.origin)}),
        ),
        PrimarySpellTransient::EtherPierceStreak(effect) => (
            "ether-pierce-streak",
            json!({"headingDegrees": effect.heading_degrees, "visualScale": effect.visual_scale, "origin": point(effect.origin)}),
        ),
        PrimarySpellTransient::FireImpact(effect) => {
            ("fire-impact", json!({"origin": point(effect.origin)}))
        }
        PrimarySpellTransient::FireExplosion(effect) => (
            "fire-explosion",
            json!({"damage": effect.damage, "burnDamage": effect.burn_damage,
            "footprintDimension": effect.footprint_dimension, "visualScale": effect.visual_scale, "origin": point(effect.origin)}),
        ),
        PrimarySpellTransient::FireEmber(effect) => (
            "fire-ember",
            json!({"damage": effect.damage, "burnDamage": effect.burn_damage, "position": point(effect.position)}),
        ),
        other => panic!("unexpected output contact actor {other:?}"),
    };
    let common = effect.common();
    json!({"id": common.id, "kind": kind, "age": common.age_ticks, "light": common.light_registration.map(registration_row),
        "registrations": common.painter_registrations.iter().copied().map(registration_row).collect::<Vec<_>>(), "detail": detail})
}
fn patch_row(patch: &NativeSecondaryTargetEffectPatch) -> Value {
    let mut row = Map::new();
    for (key, value) in
        [("stunFactor", patch.stun_factor), ("coldSlowFactor", patch.cold_slow_factor)]
    {
        if let Some(value) = value {
            row.insert(key.into(), json!(value));
        }
    }
    for (key, value) in [("stunTicks", patch.stun_ticks), ("coldSlowTicks", patch.cold_slow_ticks)]
    {
        if let Some(value) = value {
            row.insert(key.into(), json!(value));
        }
    }
    if let Some(value) = patch.cold_slow_material {
        row.insert("coldSlowMaterial".into(), json!(value));
    }
    if let Some(effect) = &patch.electric_burn {
        row.insert("electricBurn".into(), effect.as_ref().map_or(Value::Null,|effect|json!({"arcCount":effect.arc_count,"damagePerTick":effect.damage_per_tick,"ownerId":effect.owner_id,"sourceActorId":effect.source_actor_id,"stunFactor":effect.stun_factor,"ticks":effect.ticks})));
    }
    if let Some(effect) = &patch.steamed {
        row.insert("steamed".into(), effect.as_ref().map_or(Value::Null,|effect|json!({"damagePerTick":effect.damage_per_tick,"emberDamage":effect.ember_damage,"emberFragments":effect.ember_fragments,"explodeDamage":effect.explode_damage,"explodeRadius":effect.explode_radius,"ownerId":effect.owner_id,"sourceActorId":effect.source_actor_id,"ticks":effect.ticks})));
    }
    Value::Object(row)
}

#[test]
fn primary_contacts_preserve_damage_status_rng_and_painter_order() {
    replay_contacts(include_str!("fixtures/website/primary-contact-goldens.json"));
}
#[test]
fn current_flame_terminal_contacts_precede_native_factory_births() {
    replay_contacts(include_str!("fixtures/website/flame-contact-goldens.json"));
}
#[test]
fn current_frost_missile_direct_area_and_terrain_contacts_replay_source() {
    let fixture = include_str!("fixtures/website/current-frost-contact-goldens.json");
    let source: Value = serde_json::from_str(fixture).unwrap();
    assert_eq!(source["sourceRevision"], "fa08ae249ff5563570bdb2a653e2ef9358ce7b83");
    replay_contacts(fixture);
}
fn replay_contacts(fixture: &str) {
    let fixture: Value = serde_json::from_str(fixture).unwrap();
    for case in values(&fixture, "cases") {
        let name = case["name"].as_str().unwrap();
        let mut enemies = EnemyWorld::new(&format!("primary-contact:{name}"), 50);
        let mut order = WorldPainterOrder { next_actor_ordinal: 100, next_transient_ordinal: 70 };
        let intents: Vec<_> = values(case, "enemies")
            .iter()
            .enumerate()
            .map(|(index, spec)| {
                EnemySpawnIntent::new(
                    EnemyToken::from_id(spec["token"].as_str().unwrap()).unwrap(),
                    vector(&spec["position"]),
                    index + 1,
                    10,
                    31,
                )
            })
            .collect();
        enemies.spawn(
            &intents,
            EnemySpawnContext {
                players: &[],
                tick: 10,
                arena_scalars: EnemyArenaScalars::default(),
                roll_loot_seed: None,
            },
            &OpenGeometry,
            Some(&mut order),
        );
        for (mut actor, spec) in enemies.actors.iter_mut().zip(values(case, "enemies")) {
            if let Some(cooldown) = spec["hurricaneCooldown"].as_f64() {
                actor.hurricane_contact_cooldown = cooldown;
            }
            if let Some(accumulator) = spec["blizzardPushAccumulator"].as_f64() {
                actor.blizzard_push_accumulator = accumulator;
                actor.blizzard_push_last_tick = spec["blizzardPushLastTick"].as_u64();
            }
            if let Some(health) = spec["health"].as_f64() {
                actor.current_health = health;
            }
            if let Some(shield) = spec["shield"].as_f64() {
                actor.shield_health = shield;
                actor.shield_maximum_health = shield;
            }
        }
        if case["forces"] == true {
            let owner = enemies.actors.get(0).unwrap().snapshot();
            spawn_projectile(
                &mut enemies,
                &owner,
                10,
                EnemyProjectileKind::Arrow,
                8.0,
                SpawnEnemyProjectileOptions {
                    position: Some(Vec2::new(200.0, 250.0)),
                    heading_deg: Some(90.0),
                    speed: Some(6.0),
                    minimum_speed: Some(6.0),
                    lifetime_ticks: Some(500),
                    ..SpawnEnemyProjectileOptions::default()
                },
                true,
                &mut order,
            );
            enemies.set_arrow_chill_tumble_accumulator(1, 0.9);
            let mut silk_rng = NativeRng::new(31);
            let mut state = create_native_silk(
                Vec2::new(210.0, 250.0),
                Vec2::new(500.0, 250.0),
                Vec2::ZERO,
                45.0,
                &mut silk_rng,
            )
            .state;
            for _ in 0..20 {
                assert!(step_native_silk(&mut state));
            }
            state.force_accumulator = 0.9;
            enemies.silks.push(EnemySilkActor {
                id: 44,
                owner_actor_id: 1,
                spawn_tick: 10,
                state,
                native_registration_order: 60,
                native_cell_binding_order: 60,
                painter_registration: order.register(WorldManagerLane::Actor),
            });
        }
        let mut spells = SpellState {
            next_id: 100,
            projectiles: values(case, "projectiles").iter().map(projectile).collect(),
            transients: values(case, "transients").iter().map(transient).collect(),
        };
        let emissions: Vec<_> = values(case, "emissions").iter().map(channel).collect();
        let contacts: Vec<_> = values(case, "contacts").iter().map(fire_contact).collect();
        let scenery: Vec<_> = values(case, "scenery")
            .iter()
            .map(|value| PrimarySpellTarget {
                active: true,
                actor_flags: 4,
                attachment: Vec2::ZERO,
                body_radius: 15.0,
                cell_binding_order: 0,
                heading_degrees: None,
                grid_position: None,
                id: text(value, "id"),
                kind: PrimarySpellTargetKind::Scenery,
                native_priority: 0,
                pending_remove: false,
                position: vector(&value["position"]),
                query_lane: PrimarySpellQueryLane::Grid,
                registration_order: 0,
            })
            .collect();
        let obstruction =
            |_: Vec2, end: Vec2, _: f64| (case["blockedX"].as_f64() == Some(end.x)).then_some(0.5);
        let mut movements = Vec::new();
        let mut movement = |id, start: Vec2, requested: Vec2, radius| {
            movements.push(json!([id, point(start), point(requested), radius]));
            Vec2::new(
                requested.x.min(case["movementClampX"].as_f64().unwrap_or(f64::INFINITY)),
                requested.y,
            )
        };
        let multiplier = |_, _, _: &str| case["multiplier"].as_f64().unwrap_or(1.0);
        let mut rng = NativeRng::new(17);
        let frost_world_contacts: Vec<_> = values(case, "frostWorldContacts")
            .iter()
            .map(|value| {
                let PrimarySpellProjectile::Weld(projectile) = projectile(value) else {
                    unreachable!()
                };
                projectile
            })
            .collect();
        let first_event_id = enemies.next_event_id;
        let result = resolve_boneyard_spell_combat(
            &mut enemies,
            &mut spells,
            BoneyardSpellCombatContext {
                enhanced_effects: case["enhancedEffects"].as_bool().unwrap_or(true),
                channel_emissions: &emissions,
                fire_actor_contacts: &contacts,
                frost_missile_world_contacts: &frost_world_contacts,
                tick: case["tick"].as_u64().unwrap_or(10),
                world_key: "boneyard:contact-oracle",
                first_world_contact: Some(&obstruction),
                damage_multiplier: &multiplier,
                primary_scenery_targets: &scenery,
                resolve_enemy_movement: &mut movement,
                fireball_hostile_corridor_length: &|_| 1600.0,
                light_at: Some(&|_| 0.8),
            },
            &mut rng,
            Some(&mut order),
        );
        let actors: Vec<_> = enemies
            .actors
            .iter()
            .map(|actor| {
                json!([
                    actor.id,
                    actor.current_health,
                    if actor.life_state == EnemyLifeState::Alive { "alive" } else { "dying" },
                    point(actor.position),
                    actor.shield_health,
                    actor.lethal_magic_damage,
                    actor.native_cell_binding_order,
                    actor.blizzard_push_accumulator,
                    actor.blizzard_push_last_tick,
                    actor.hurricane_contact_cooldown
                ])
            })
            .collect();
        let hits: Vec<_> = result
            .hits
            .iter()
            .map(|hit| {
                json!({
                    "actorId": hit.actor_id,
                    "amount": hit.amount,
                    "killed": hit.killed,
                    "ownerId": hit.owner_id,
                    "spellId": hit.spell_id,
                    "spellKind": hit.spell_kind.id(),
                    "tick": hit.tick,
                })
            })
            .collect();
        let burns:Vec<_> = result.burns.iter().map(|burn| json!({"damage": burn.damage, "ownerId": burn.owner_id, "targetId": burn.target_id})).collect();
        let ether_burns: Vec<_> = result
            .ether_burns
            .iter()
            .map(|burn| json!({"ownerId": burn.owner_id, "targetId": burn.target_id}))
            .collect();
        let effects:Vec<_> = result.target_effects.iter().map(|effect| json!({"patch": patch_row(&effect.patch), "targetId": effect.target_id, "worldKey": effect.world_key})).collect();
        let notices: Vec<_> = result
            .damage_notices
            .iter()
            .map(|notice| match notice {
                BoneyardSpellDamageNotice::LethalReward { enemy, owner_id, .. } => {
                    json!(["reward", enemy.enemy_token.id(), enemy.maximum_health, owner_id])
                }
                BoneyardSpellDamageNotice::HealthDamage {
                    actor_id,
                    amount,
                    maximum_health,
                    owner_id,
                } => json!(["health", actor_id, amount, maximum_health, owner_id]),
            })
            .collect();
        let mut actual = json!({"actors": actors, "spells": {"nextId": spells.next_id,
        "projectiles": spells.projectiles.iter().map(projectile_row).collect::<Vec<_>>(), "transients": spells.transients.iter().map(transient_row).collect::<Vec<_>>()},
        "hits": hits, "burns": burns, "etherBurns": ether_burns, "targetEffects": effects, "rng": random_row(&rng), "enemyRng": enemies.rng_state,
        "notices": notices, "movements": movements, "painterOrder": {"actor": order.next_actor_ordinal, "transient": order.next_transient_ordinal},
        "forces": {
            "arrows": enemies.projectiles.iter().map(|p| json!([p.state().id,p.state().chill_tumble_accumulator])).collect::<Vec<_>>(),
            "silks": enemies.silks.iter().map(|p|json!([p.id,p.state.force_accumulator])).collect::<Vec<_>>(),
            "fragments": enemies.silk_fragments.iter().map(|p|json!([p.id,point(p.state.start),point(p.state.middle),point(p.state.end),point(p.state.velocity),p.state.opacity,p.state.opacity_loss_per_tick])).collect::<Vec<_>>(),
            "projectileEffects": enemies.projectile_effects.iter().map(|p|json!([p.state.id,p.kind.id(),point(p.state.position)])).collect::<Vec<_>>(),
        }});
        if case["expected"].get("currentDetails").is_some() {
            let spiders: Vec<_> = enemies
                .actors
                .iter()
                .filter_map(|actor| {
                    if let solomon_sim::enemies::model::EnemyBrain::Spider(brain) = &actor.brain {
                        Some(json!([
                            actor.id,
                            brain.action_state,
                            brain.strafe_sign,
                            brain.strafe_ticks_remaining
                        ]))
                    } else {
                        None
                    }
                })
                .collect();
            actual["currentDetails"] =
                json!({"steeringRng":random_row(&enemies.steering_rng),"spiders":spiders});
        }
        if case["expected"].get("events").is_some() {
            actual["events"] = json!(enemies.events.iter().filter(|event|event.event_id>=first_event_id).map(|event|json!({
                "type":event.kind.id(),"actorId":event.actor_id,"eventId":event.event_id,"tick":event.tick,
                "sound":event.sound.map(|sound|sound.id()),"pitch":event.pitch,"gainScale":event.gain_scale,"position":event.source_position.map(|position|json!({"x":position.x,"y":position.y})),
            })).collect::<Vec<_>>());
            actual["nextEventId"] = json!(enemies.next_event_id);
            actual["reactions"] = json!(
                enemies
                    .actors
                    .iter()
                    .map(|actor| [actor.hit_feedback.timer, actor.hit_reaction_timer])
                    .collect::<Vec<_>>()
            );
        }
        let mut expected = case["expected"].clone();
        // Steam stores its unrounded atan2 heading; all float32 state, accepted
        // damage, native draw words and painter ownership still compare exactly.
        for (index, actor) in actual["spells"]["transients"].as_array().unwrap().iter().enumerate()
        {
            if actor["kind"] != "weld-steam" {
                continue;
            }
            let heading = actor["physics"]["rotationDegrees"].as_f64().unwrap();
            let reference = expected["spells"]["transients"][index]["physics"]["rotationDegrees"]
                .as_f64()
                .unwrap();
            assert!((heading - reference).abs() < 1e-9, "{name}: Steam heading");
            expected["spells"]["transients"][index]["physics"]["rotationDegrees"] = json!(heading);
        }
        website_rows::compare(&actual, &expected, name);
    }
}
