//! Complete field replay against the actual pinned client snapshot interpolator.
use super::*;
use serde_json::Value;
#[path = "../../../../sim/tests/enemy_goldens/sample_json.rs"]
mod sample_json;

fn fixture() -> Value {
    serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../sim/tests/fixtures/website/enemy-samples-goldens.json"
    )))
    .unwrap()
}

#[test]
fn every_family_blends_source_animation_fields_without_crossing_program_boundaries() {
    let source = fixture();
    let records = source["cases"].as_array().unwrap();
    assert_eq!(records.len(), 420);
    for record in records {
        let first = sample_json::animation_from_json(&record["first"]);
        let second = sample_json::animation_from_json(&record["second"]);
        let token = EnemyToken::ALL
            .into_iter()
            .find(|token| Some(token.id()) == record["token"].as_str())
            .unwrap();
        let actual =
            interpolate_animation(&first, &second, record["blend"].as_f64().unwrap(), token);
        assert_source(
            &sample_json::animation_json(&actual),
            &record["expected"],
            record["name"].as_str().unwrap(),
        );
    }
}

fn assert_source(actual: &Value, expected: &Value, path: &str) {
    match (actual, expected) {
        (Value::Number(first), Value::Number(second)) => {
            assert_eq!(first.as_f64(), second.as_f64(), "{path}")
        }
        (Value::Object(first), Value::Object(second)) => {
            assert_eq!(first.len(), second.len(), "{path} fields");
            for (key, value) in first {
                assert_source(value, &second[key], &format!("{path}.{key}"));
            }
        }
        (Value::Array(first), Value::Array(second)) => {
            assert_eq!(first.len(), second.len(), "{path} members");
            for (index, (first, second)) in first.iter().zip(second).enumerate() {
                assert_source(first, second, &format!("{path}[{index}]"));
            }
        }
        _ => assert_eq!(actual, expected, "{path}"),
    }
}

use serde_json::json;
use solomon_sim::NativeRng;
use solomon_sim::enemies::actor_presentation::NativeMaggotState;
use solomon_sim::enemies::effects::{
    DeathEffectOverrides, DeathEffectOwner, SimpleDeathEffectSpec,
};
use solomon_sim::enemies::model::{
    DeathPresentationOwner, EnemyDeathEffectKind, EnemyEffectAtlas, EnemyEffectBlend,
    EnemyEventKind, WebbedState,
};
use solomon_sim::enemies::projectiles::{
    EnemyProjectileEffect, EnemyProjectileEffectKind, EnemyProjectileEffectState,
    EnemyProjectilePayload, EnemyProjectileState, MageLightningContact,
};
use solomon_sim::enemies::silk::{EnemySilkActor, create_native_silk};
use solomon_sim::enemies::silk_force::{NativeFadeLineActor, NativeFadeLineState};
use solomon_sim::enemies::spider_remains::{SpiderDecal, SpiderRemains, create_native_dead_spider};
use solomon_sim::painter::{PainterRegistration, WorldManagerLane, WorldPainterOrder};

fn number(value: &Value, field: &str) -> f64 {
    value[field].as_f64().unwrap()
}
fn id(value: &Value) -> usize {
    value["id"].as_u64().unwrap() as usize
}
fn registration(id: usize) -> PainterRegistration {
    PainterRegistration { manager_lane: WorldManagerLane::Transient, registration_ordinal: id }
}

fn registration_json(registration: Option<PainterRegistration>) -> Value {
    match registration {
        None => Value::Null,
        Some(registration) => json!({"managerLane":match registration.manager_lane {
            WorldManagerLane::Actor=>"actor",WorldManagerLane::Transient=>"transient",
        },"registrationOrdinal":registration.registration_ordinal}),
    }
}

fn projectile(value: &Value) -> EnemyProjectile {
    let kind = value["kind"].as_str().unwrap();
    let heading = number(value, "headingDeg");
    let speed = number(value, "speed");
    let state = EnemyProjectileState {
        age_ticks: number(value, "ageTicks") as u64,
        bounce_velocity: 0.0,
        chill_tumble_accumulator: 0.0,
        cold_slow_ticks: 0,
        contact_radius: 5.0,
        damage: 0.0,
        secondary_damage: 0.0,
        heading_deg: heading,
        hit_player_ids: Vec::new(),
        homing: false,
        id: id(value),
        last_step_tick: 0,
        light_registration: None,
        lifetime_ticks: 100,
        minimum_speed: 0.0,
        native_type_id: match kind {
            "arrow" => EnemyProjectileKind::Arrow,
            "guided-missile" => EnemyProjectileKind::GuidedMissile,
            "demon-bomb" => EnemyProjectileKind::DemonBomb,
            "firebolt" => EnemyProjectileKind::Firebolt,
            "poison-pool" => EnemyProjectileKind::PoisonPool,
            _ => panic!("unknown source projectile"),
        }
        .native_type_id(),
        native_cell_binding_order: 0,
        native_registration_order: 0,
        owner_actor_id: 1,
        painter_registration: registration(id(value)),
        payload: EnemyProjectilePayload::Normal,
        poison_damage: 0.0,
        poison_duration: 0.0,
        position: sample_json::point(&value["position"]),
        speed,
        settled_ticks_remaining: 0,
        spawn_tick: 0,
        target_player_id: None,
        turn_speed: 0.0,
        vertical_offset: number(value, "verticalOffset"),
        vertical_velocity: 0.0,
        visual_phase_deg: number(value, "visualPhaseDeg"),
        visual_scale: number(value, "visualScale"),
    };
    match kind {
        "arrow" => {
            let angle = heading * PI / 180.0;
            EnemyProjectile::Arrow {
                state,
                velocity: Vec2::new(angle.sin() * speed, -angle.cos() * speed),
            }
        }
        "guided-missile" => EnemyProjectile::GuidedMissile(state),
        "demon-bomb" => EnemyProjectile::DemonBomb(state),
        "firebolt" => EnemyProjectile::Firebolt(state),
        "poison-pool" => EnemyProjectile::PoisonPool(state),
        _ => unreachable!(),
    }
}

fn maggot(value: &Value) -> NativeMaggotPresentationInput {
    NativeMaggotPresentationInput {
        alpha: number(value, "alpha"),
        current_health: number(value, "currentHealth"),
        death_tick: number(value, "deathTick"),
        emergence_phase: number(value, "emergencePhase"),
        emergence_tick: number(value, "emergenceTick"),
        heading_deg: number(value, "headingDeg"),
        hit_flash: number(value, "hitFlash"),
        pose: number(value, "pose"),
        position: sample_json::point(&value["position"]),
        vertical_offset: number(value, "verticalOffset"),
        visual_scale: number(value, "visualScale"),
        state: match value["state"].as_str().unwrap() {
            "emerging" => NativeMaggotState::Emerging,
            "death" => NativeMaggotState::Death,
            _ => panic!("fixture Maggot state"),
        },
    }
}

fn world(value: &Value) -> EnemyWorld {
    let mut world = EnemyWorld::new("presentation-source", 0);
    world.last_step_tick = Some(10);
    world.projectiles =
        value["enemyProjectiles"].as_array().unwrap().iter().map(projectile).collect();
    let mut painter_order = WorldPainterOrder::default();
    for source in value["deathEffects"].as_array().unwrap() {
        world.spawn_simple_death_effect(
            DeathEffectOwner { id: 1, position: sample_json::point(&source["position"]) },
            0,
            SimpleDeathEffectSpec {
                alpha: number(source, "alpha"),
                alpha_loss_per_tick: 0.0,
                atlas: EnemyEffectAtlas::BadGuys,
                blend_mode: EnemyEffectBlend::Normal,
                entry: id(source),
                kind: EnemyDeathEffectKind::Fade,
                lifetime_ticks: 100,
                role: "source-effect",
                scale: number(source, "scale"),
            },
            DeathEffectOverrides {
                presentation_owner: Some(DeathPresentationOwner::DirectPostWorld),
                ..Default::default()
            },
            &mut painter_order,
        );
        let effect = world.death_effects.last_mut().unwrap();
        effect.id = id(source);
        effect.age_ticks = number(source, "ageTicks") as u64;
        effect.height = number(source, "height");
        effect.rotation_deg = number(source, "rotationRadians") * 180.0 / PI;
        effect.scale_y = number(source, "scaleY");
        effect.entry = source["entry"].as_u64().unwrap() as usize;
    }
    for source in value["enemyProjectileEffects"].as_array().unwrap() {
        world.projectile_effects.push(EnemyProjectileEffect {
            kind: EnemyProjectileEffectKind::ArrowTumble,
            state: EnemyProjectileEffectState {
                age_ticks: number(source, "ageTicks") as u64,
                alpha: number(source, "alpha"),
                alpha_loss_per_tick: 0.0,
                angular_velocity_deg: 0.0,
                atlas: EnemyEffectAtlas::BadGuys,
                blend_mode: EnemyEffectBlend::Normal,
                entry: 2,
                id: id(source),
                last_step_tick: 0,
                light_registration: None,
                lifetime_ticks: 100,
                owner_actor_id: 1,
                owner_projectile_id: 1,
                painter_registration: registration(id(source)),
                phase_origin_ticks: 0,
                position: sample_json::point(&source["position"]),
                rotation_deg: number(source, "rotationRadians") * 180.0 / PI,
                scale: number(source, "scale"),
                spawn_tick: 0,
                tint: 0xffffff,
                velocity: Vec2::ZERO,
            },
        });
    }
    for source in value["mageLightningPulses"].as_array().unwrap() {
        world.mage_lightning_pulses.push(MageLightningPulse {
            enhanced_effects: source["enhancedEffects"].as_bool().unwrap_or(true),
            contact: MageLightningContact::World {
                position: sample_json::point(&source["contact"]["position"]),
            },
            endpoint: sample_json::point(&source["endpoint"]),
            id: id(source),
            light_registration: registration(id(source)),
            midpoint: sample_json::point(&source["midpoint"]),
            owner_actor_id: 1,
            painter_registrations: Vec::new(),
            seed: 1,
            source: sample_json::point(&source["source"]),
            tick: source["tick"].as_u64().unwrap(),
        });
    }
    for source in value["spiderSilks"].as_array().unwrap() {
        let value = &source["state"];
        let mut rng = NativeRng::new(1);
        let mut state =
            create_native_silk(Vec2::ZERO, Vec2::new(100.0, 100.0), Vec2::ZERO, 1.0, &mut rng)
                .state;
        state.age_ticks = number(value, "ageTicks") as u64;
        state.phase = number(value, "phase");
        state.height = number(value, "height");
        state.alpha = number(value, "alpha");
        state.wave_scale = number(value, "waveScale");
        state.position = sample_json::point(&value["position"]);
        state.drift = sample_json::point(&value["drift"]);
        state.ended = value["ended"].as_bool().unwrap();
        world.silks.push(EnemySilkActor {
            native_registration_order: 0,
            native_cell_binding_order: 0,
            id: id(source),
            owner_actor_id: 1,
            spawn_tick: 0,
            state,
            painter_registration: registration(id(source)),
        });
    }
    for source in value["silkFragments"].as_array().unwrap() {
        let value = &source["state"];
        world.silk_fragments.push(NativeFadeLineActor {
            id: id(source),
            spawn_tick: 0,
            state: NativeFadeLineState {
                start: sample_json::point(&value["start"]),
                middle: sample_json::point(&value["middle"]),
                end: sample_json::point(&value["end"]),
                opacity: number(value, "opacity"),
                opacity_loss_per_tick: 0.1,
                velocity: Vec2::ZERO,
            },
        });
    }
    for source in value["spiderRemains"].as_array().unwrap() {
        let value = &source["state"];
        let mut state = create_native_dead_spider(sample_json::point(&value["position"]), 0.0);
        state.life = number(value, "life");
        state.frame = number(value, "frame");
        state.decal = Some(SpiderDecal {
            entry: 140,
            position: sample_json::point(&value["decal"]["position"]),
            rotation_deg: 0.0,
            scale: number(&value["decal"], "scale"),
            alpha: number(&value["decal"], "alpha"),
        });
        world.spider_remains.push(SpiderRemains { id: id(source), spawn_tick: 0, state });
    }
    world.webbed_players.insert(
        "hero".into(),
        WebbedState {
            severity: number(&value["webbedPlayers"]["hero"], "severity"),
            strength: 0.0,
            cocoon_health: 0.0,
            hit_pulse: 0.0,
        },
    );
    world
}

#[test]
fn source_entity_births_projectiles_spiders_and_lightning_merge_without_authority_steps() {
    let source = fixture();
    let records = source["members"].as_array().unwrap();
    assert_eq!(records.len(), 15);
    for record in records {
        let older = world(&record["older"]);
        let newer = world(&record["newer"]);
        let blend = number(record, "blend");
        let tick = number(record, "tick");
        let mut output = endpoint(&older, &newer, blend).clone();
        let events = |value: &Value| -> Vec<EnemyEvent> {
            value["enemyEvents"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| {
                    let mut event = EnemyEvent::new(EnemyEventKind::AttackMarker, 1, 0);
                    event.event_id = id(value);
                    event
                })
                .collect()
        };
        let mut first = from_world(&older, 10.0, &events(&record["older"]), &|_| {
            unreachable!("no actors in member fixture")
        });
        let mut second = from_world(&newer, 15.0, &events(&record["newer"]), &|_| {
            unreachable!("no actors in member fixture")
        });
        // These snapshots already contain radians. Preserve those exact values across the test adapter,
        // just as the host-to-client projection does, rather than testing a degree round trip.
        for (value, metadata) in [(&record["older"], &mut first), (&record["newer"], &mut second)] {
            for effect in value["deathEffects"].as_array().unwrap() {
                metadata.death_effects.get_mut(&id(effect)).unwrap().rotation_radians =
                    number(effect, "rotationRadians");
            }
            for effect in value["enemyProjectileEffects"].as_array().unwrap() {
                metadata.projectile_effects.get_mut(&id(effect)).unwrap().rotation_radians =
                    number(effect, "rotationRadians");
            }
        }
        let metadata = interpolate(&mut output, &older, &newer, &first, &second, blend, tick);
        let expected = &record["expected"];
        for (actual, expected) in
            output.projectiles.iter().zip(expected["enemyProjectiles"].as_array().unwrap())
        {
            let state = actual.state();
            let actual = json!({"id":state.id,"ageTicks":metadata.projectile_age_ticks[&state.id],
                "position":sample_json::point_json(state.position),"speed":state.speed,"verticalOffset":state.vertical_offset,
                "visualPhaseDeg":state.visual_phase_deg,"visualScale":state.visual_scale});
            let expected = expected.as_object().unwrap();
            let selected: serde_json::Map<_, _> = actual
                .as_object()
                .unwrap()
                .keys()
                .map(|key| (key.clone(), expected[key].clone()))
                .collect();
            assert_source(&actual, &Value::Object(selected), "projectile");
            // The adapter derives Arrow heading through libm, as the real host projector does.
            assert!(
                (state.heading_deg - number(&Value::Object(expected.clone()), "headingDeg")).abs()
                    < 1e-9
            );
        }
        assert_eq!(
            output.projectiles.len(),
            expected["enemyProjectiles"].as_array().unwrap().len()
        );
        let deaths:Vec<_>=output.death_effects.iter().map(|effect| {
            let sample=metadata.death_effects[&effect.id];
            json!({"id":effect.id,"ageTicks":sample.age_ticks,"alpha":effect.alpha,"height":effect.height,
                "position":sample_json::point_json(effect.position),"rotationRadians":sample.rotation_radians,"scale":effect.scale,"scaleY":effect.scale_y,"entry":effect.entry,"painterRegistration":registration_json(effect.painter_registration)})
        }).collect();
        assert_source(&json!(deaths), &expected["deathEffects"], "deathEffects");
        let effects:Vec<_>=output.projectile_effects.iter().map(|effect| {
            let sample=metadata.projectile_effects[&effect.state.id];
            json!({"id":effect.state.id,"ageTicks":sample.age_ticks,"alpha":effect.state.alpha,
                "position":sample_json::point_json(effect.state.position),"rotationRadians":sample.rotation_radians,"scale":effect.state.scale,"lightRegistration":registration_json(effect.state.light_registration),"painterRegistration":registration_json(Some(effect.state.painter_registration))})
        }).collect();
        assert_source(&json!(effects), &expected["enemyProjectileEffects"], "projectileEffects");
        let silks:Vec<_>=output.silks.iter().map(|silk| json!({"id":silk.id,"state":{
            "ageTicks":metadata.silk_age_ticks[&silk.id],"phase":silk.state.phase,"height":silk.state.height,
            "alpha":silk.state.alpha,"waveScale":silk.state.wave_scale,"position":sample_json::point_json(silk.state.position),
            "drift":sample_json::point_json(silk.state.drift),"ended":silk.state.ended}})).collect();
        assert_source(&json!(silks), &expected["spiderSilks"], "spiderSilks");
        let fragments:Vec<_>=output.silk_fragments.iter().map(|fragment| json!({"id":fragment.id,"state":{
            "start":sample_json::point_json(fragment.state.start),"middle":sample_json::point_json(fragment.state.middle),
            "end":sample_json::point_json(fragment.state.end),"opacity":fragment.state.opacity}})).collect();
        assert_source(&json!(fragments), &expected["silkFragments"], "silkFragments");
        let remains:Vec<_>=output.spider_remains.iter().map(|remains| {
            let decal=remains.state.decal.as_ref().unwrap();
            json!({"id":remains.id,"state":{"life":remains.state.life,"position":sample_json::point_json(remains.state.position),
                "frame":remains.state.frame,"decal":{"position":sample_json::point_json(decal.position),"scale":decal.scale,"alpha":decal.alpha}}})
        }).collect();
        assert_source(&json!(remains), &expected["spiderRemains"], "spiderRemains");
        assert_eq!(
            output.webbed_players["hero"].severity,
            number(&expected["webbedPlayers"]["hero"], "severity")
        );
        let pulses: Vec<_> =
            output.mage_lightning_pulses.iter().map(|pulse| (pulse.id, pulse.tick)).collect();
        let expected_pulses: Vec<_> = expected["mageLightningPulses"]
            .as_array()
            .unwrap()
            .iter()
            .map(|pulse| (id(pulse), pulse["tick"].as_u64().unwrap()))
            .collect();
        assert_eq!(pulses, expected_pulses, "pulse birth window at {tick}");
        let expected_ids: BTreeSet<_> =
            expected["enemyEvents"].as_array().unwrap().iter().map(id).collect();
        assert_eq!(metadata.visible_event_ids, expected_ids, "events stay endpoint-discrete");
        let sample = interpolate_maggot(
            &maggot(&record["older"]["maggots"][0]),
            &maggot(&record["newer"]["maggots"][0]),
            blend,
        );
        let actual = json!({"id":1,"alpha":sample.alpha,"currentHealth":sample.current_health,"deathTick":sample.death_tick,
            "emergencePhase":sample.emergence_phase,"emergenceTick":sample.emergence_tick,"headingDeg":sample.heading_deg,
            "hitFlash":sample.hit_flash,"pose":sample.pose,"position":sample_json::point_json(sample.position),
            "verticalOffset":sample.vertical_offset,"visualScale":sample.visual_scale,
            "state":if sample.state==NativeMaggotState::Emerging {"emerging"} else {"death"}});
        let mut expected_maggot = expected["maggots"][0].clone();
        // The flat Maggot input contains only animation. Its actor registration stays
        // in the selected EnemyMaggotActor outside this source animation helper.
        expected_maggot.as_object_mut().unwrap().remove("lightRegistration");
        assert_source(&actual, &expected_maggot, "Maggot");
    }
}

#[test]
fn source_attack_flashes_use_fractional_age_through_their_lifetime_boundaries() {
    use solomon_sim::enemies::attack_presentation::{
        EnemyRawFireBurstKind, native_enemy_raw_fire_burst_sample_at_age,
        native_imp_landing_flare_sample_at_age,
    };
    let source = fixture();
    let records = source["attacks"].as_array().unwrap();
    assert_eq!(records.len(), 90);
    for record in records {
        let event_id = record["eventId"].as_u64().unwrap() as usize;
        let age = number(record, "ageTicks");
        let actual = if record["kind"] == "imp-landing" {
            json!(native_imp_landing_flare_sample_at_age(event_id,age).map(|sample| json!({
                "alpha":sample.alpha,"green":sample.green,"scaleX":sample.scale_x,"scaleY":sample.scale_y,
            })))
        } else {
            let kind = if record["kind"] == "imp-contact" {
                EnemyRawFireBurstKind::ImpContact
            } else {
                EnemyRawFireBurstKind::DemonBombMuzzle
            };
            json!(native_enemy_raw_fire_burst_sample_at_age(kind,event_id,age).map(|sample| json!({
                "frameEntry":sample.frame_entry,"frameRotationRadians":sample.frame_rotation_radians,
                "glowAlpha":sample.glow_alpha,"scale":sample.scale,"verticalOffset":sample.vertical_offset,
            })))
        };
        assert_source(&actual, &record["expected"], "auxiliary fractional age");
    }
}
