use super::*;
use solomon_sim::NativeRng;
use solomon_sim::enemies::demon_skull_controller::*;
use solomon_sim::enemies::faculty_controller::*;
use solomon_sim::enemies::faculty_spells::*;
use solomon_sim::enemies::guided_missile::*;
use solomon_sim::enemies::model::{EnemyBrain, FacultyActionKind, FacultyBrain};
use solomon_sim::enemies::path::NativeEnemyPathState;

fn action_kind(value: &str) -> FacultyActionKind {
    match value {
        "throw" => FacultyActionKind::Throw,
        "two-hand" => FacultyActionKind::TwoHand,
        "lightning" => FacultyActionKind::Lightning,
        _ => panic!("recorded Faculty action kind"),
    }
}

fn action_kind_json(kind: FacultyActionKind) -> Value {
    json!(match kind {
        FacultyActionKind::Throw => "throw",
        FacultyActionKind::TwoHand => "two-hand",
        FacultyActionKind::Lightning => "lightning",
    })
}

fn faculty_state_json(brain: &FacultyBrain) -> Value {
    let mut state = brain_json(&EnemyBrain::Faculty(brain.clone()));
    let object = state.as_object_mut().unwrap();
    object.remove("family");
    object.remove("phase");
    state
}

fn skull_state_json(brain: &solomon_sim::enemies::model::DemonSkullBrain) -> Value {
    let mut state = brain_json(&EnemyBrain::DemonSkull(brain.clone()));
    for field in ["family", "phase", "deathCountdown", "actions"] {
        state.as_object_mut().unwrap().remove(field);
    }
    state
}

pub(super) fn dark_fireball_json(state: NativeDarkFireballState) -> Value {
    json!({"ageTicks":state.age_ticks,"arcPhase":state.arc_phase,"arcPhaseStep":state.arc_phase_step,
        "headingDeg":state.heading_deg,"position":point_json(state.position),
        "remainingTicks":state.remaining_ticks,"velocity":point_json(state.velocity)})
}

pub(super) fn rain_state_json(state: NativeRainOfBonesState) -> Value {
    json!({"alpha":state.alpha,"phase":state.phase,"position":point_json(state.position),
        "remainingTicks":state.remaining_ticks,"rotationSign":state.rotation_sign,"scale":state.scale})
}

pub(super) fn dire_fire_json(state: NativeDireFireState) -> Value {
    json!({"alpha":state.alpha,"glow":state.glow,"phase":state.phase,"ramp":state.ramp,
        "rotationPhases":state.rotation_phases,"rotationRates":state.rotation_rates,
        "rotationSigns":state.rotation_signs,"scale":state.scale,"scaleSign":state.scale_sign})
}

pub(super) fn guided_missile_json(state: NativeGuidedMissileState) -> Value {
    json!({"headingDeg":state.heading_deg,"minimumSpeed":state.minimum_speed,"phaseDeg":state.phase_deg,
        "position":point_json(state.position),"remainingTicks":state.remaining_ticks,
        "speed":state.speed,"turnRate":state.turn_rate,"visualScale":state.visual_scale})
}

pub(super) fn boss_spell_json(spell: &solomon_sim::enemies::boss_spell::NativeBossSpell) -> Value {
    use solomon_sim::enemies::boss_spell::BossSpellKind;
    let owner = spell.owner;
    let mut record = json!({"ageTicks":owner.age_ticks,"damage":owner.damage,"id":owner.id,
        "painterRegistration":painter_registration_json(owner.painter_registration),
        "ownerActorId":owner.owner_actor_id,"position":point_json(owner.position),
        "spawnTick":owner.spawn_tick,"kind":spell.kind.id()});
    let fields = match &spell.kind {
        BossSpellKind::UltraBanish {
            remaining_ticks,
            alpha,
            flash_alpha,
            light_radius,
            mega_death,
        } => json!({
            "remainingTicks":remaining_ticks,"alpha":alpha,"flashAlpha":flash_alpha,"lightRadius":light_radius,"megaDeath":mega_death}),
        BossSpellKind::UnholySoul {
            origin,
            angle_deg,
            angular_speed,
            scale,
            height,
            vertical_speed,
            life,
            fast,
        } => json!({
            "origin":point_json(*origin),"angleDeg":angle_deg,"angularSpeed":angular_speed,"scale":scale,"height":height,"verticalSpeed":vertical_speed,"life":life,"fast":fast}),
        BossSpellKind::EyeLaser { heading_deg, phase_deg, velocity } => {
            json!({"headingDeg":heading_deg,"phaseDeg":phase_deg,"velocity":point_json(*velocity)})
        }
        BossSpellKind::UnholySpit {
            origin,
            travel,
            spread_direction,
            progress,
            progress_per_tick,
            height,
            spawn_fire,
            spawn_imps,
        } => json!({
            "origin":point_json(*origin),"travel":point_json(*travel),"spreadDirection":point_json(*spread_direction),"progress":progress,
            "progressPerTick":progress_per_tick,"height":height,"spawnFire":spawn_fire,"spawnImps":spawn_imps}),
        BossSpellKind::GreenFire { fire, glow } => {
            json!({"fire":projectile_cases::fire_patch_json(fire),"glow":glow})
        }
        BossSpellKind::UnholyBurst {
            frame_phase,
            frame_velocity,
            offset_y,
            offset_step_y,
            light_intensity,
        } => json!({
            "framePhase":frame_phase,"frameVelocity":frame_velocity,"offsetY":offset_y,"offsetStepY":offset_step_y,"lightIntensity":light_intensity}),
        BossSpellKind::MouthBeamSegment { vertices, uv_offset, start_alpha, end_alpha } => json!({
            "vertices":vertices.map(point_json),"uvOffset":uv_offset,"startAlpha":start_alpha,"endAlpha":end_alpha}),
        BossSpellKind::HeartmongerFlicker { phase_deg } => json!({"phaseDeg":phase_deg}),
        BossSpellKind::HeartmongerSoul { life_phase_deg, bob_phase_deg } => {
            json!({"lifePhaseDeg":life_phase_deg,"bobPhaseDeg":bob_phase_deg})
        }
        BossSpellKind::Blightning { endpoint, midpoint, enhanced_effects } => {
            json!({"endpoint":point_json(*endpoint),"midpoint":point_json(*midpoint),"enhancedEffects":enhanced_effects})
        }
        BossSpellKind::DeathMagic {
            scale,
            alpha,
            alpha_loss_per_tick,
            painter_sort_bias,
            light,
        } => json!({
            "scale":scale,"alpha":alpha,"alphaLossPerTick":alpha_loss_per_tick,"painterSortBias":painter_sort_bias,
            "light":light.map(|light|json!({"radius":light.radius,"intensity":light.intensity,"lossPerTick":light.loss_per_tick}))}),
        BossSpellKind::FallingBone { entry, height, color_ramp, rotation_deg } => json!({
            "entry":entry,"height":height,"colorRamp":color_ramp,"rotationDeg":rotation_deg}),
        BossSpellKind::SkullMissile { target_player_id, .. } => {
            let mut fields = guided_missile_json(spell.guided_missile_state());
            fields["targetPlayerId"] = json!(target_player_id);
            fields
        }
        BossSpellKind::DarkFireball { ground_fire_damage, .. } => {
            let mut fields = dark_fireball_json(spell.dark_fireball_state());
            fields["groundFireDamage"] = json!(ground_fire_damage);
            fields
        }
        BossSpellKind::RainOfBones { .. } => rain_state_json(spell.rain_state()),
        BossSpellKind::TragicCircle { remaining_ticks } => {
            json!({"remainingTicks":remaining_ticks})
        }
        BossSpellKind::DireFire(state) => dire_fire_json(*state),
    };
    for (key, value) in fields.as_object().unwrap() {
        record[key] = value.clone();
    }
    record
}

#[test]
fn faculty_actions_controllers_and_private_spell_lifetimes_match_the_website() {
    let golden: Value = serde_json::from_str(&fixture("enemy-boss-goldens.json")).unwrap();
    for case in golden["facultyActions"].as_array().unwrap() {
        let mut rng = NativeRng::new(case["seed"].as_i64().unwrap());
        let kind = action_kind(case["kind"].as_str().unwrap());
        let mut action = create_native_faculty_action(kind, &mut rng);
        assert_json_equal(&faculty_action_json(&action), &case["initial"], "Faculty action birth");
        assert_json_equal(&json!(rng_record(&rng)), &case["rng"], "Faculty action RNG");
        for expected in case["samples"].as_array().unwrap() {
            let step = step_native_faculty_action(&action, number(case, "speed"));
            let actual = json!({"action":faculty_action_json(&step.action),"bodyPose":step.body_pose,
                "complete":step.complete,"dispatch":step.dispatch,"lockedFacing":step.locked_facing,"marker":step.marker});
            assert_json_equal(&actual, expected, "Faculty action update");
            action = step.action;
        }
    }
    for case in golden["facultyControllers"].as_array().unwrap() {
        let mut rng = NativeRng::new(case["seed"].as_i64().unwrap());
        let mut brain = create_native_faculty(&mut rng);
        register_native_faculty(
            &mut brain,
            &mut rng,
            case["memberCount"].as_u64().unwrap() as usize,
        );
        let mut path = NativeEnemyPathState::new(&mut rng);
        brain.secondary_cooldown = -1;
        brain.completed_dispatches = 1;
        brain.primary_poll = 0;
        assert_json_equal(
            &faculty_state_json(&brain),
            &case["initial"],
            "Faculty controller birth",
        );
        let mut samples = case["samples"].as_array().unwrap().iter().peekable();
        for tick in 1..=2600 {
            advance_native_faculty_orbit(&mut path, brain.orbit_sign, &mut rng);
            let context = NativeFacultyContext {
                advance_action: tick % 400 < 350,
                actor_time_scale: if tick % 500 < 30 {
                    0.0
                } else if tick % 500 < 150 {
                    0.5
                } else {
                    1.0
                },
                line_of_sight: tick % 700 < 600,
                target_distance_squared: if tick % 600 < 50 {
                    None
                } else if tick % 600 < 100 {
                    Some(10_000.0)
                } else {
                    Some(180.0 * 180.0)
                },
                tick,
            };
            let step = step_native_faculty(&mut brain, &mut rng, integer(case, "primary"), context);
            let Some(expected) =
                samples.peek().filter(|sample| sample["tick"].as_u64() == Some(tick))
            else {
                continue;
            };
            let label = format!("Faculty primary {} tick {tick}", case["primary"]);
            assert_json_equal(&faculty_state_json(&brain), &expected["state"], &label);
            assert_json_equal(&path_json(&path), &expected["path"], &label);
            assert_json_equal(&json!(rng_record(&rng)), &expected["rng"], &label);
            assert_json_equal(&json!(step.marker), &expected["marker"], &label);
            assert_json_equal(
                &step.dispatch.map(action_kind_json).unwrap_or(Value::Null),
                &expected["dispatch"],
                &label,
            );
            samples.next();
        }
        assert!(samples.next().is_none());
    }
    for case in golden["darkFireballs"].as_array().unwrap() {
        let mut rng = NativeRng::new(case["seed"].as_i64().unwrap());
        let states = create_native_dark_fireballs(
            Vec2::new(125.0, 240.0),
            77.0,
            case["ring"].as_bool().unwrap(),
            case["targetDistance"].as_f64(),
            &mut rng,
        );
        assert_json_equal(&json!(rng_record(&rng)), &case["rng"], "dark ring birth RNG");
        assert_eq!(states.len(), case["trajectories"].as_array().unwrap().len());
        for (mut state, trajectory) in
            states.into_iter().zip(case["trajectories"].as_array().unwrap())
        {
            assert_json_equal(
                &dark_fireball_json(state),
                &trajectory["initial"],
                "dark fireball birth",
            );
            let mut prior_tick = 0;
            for sample in trajectory["samples"].as_array().unwrap() {
                for tick in prior_tick + 1..=sample["tick"].as_u64().unwrap() {
                    let step =
                        step_native_dark_fireball(state, if tick % 20 == 0 { 0.5 } else { 1.0 });
                    state = step.spell;
                    if tick == sample["tick"].as_u64().unwrap() {
                        assert_json_equal(
                            &json!(step.impact),
                            &sample["impact"],
                            "dark arc impact",
                        );
                    }
                }
                assert_json_equal(
                    &dark_fireball_json(state),
                    &sample["state"],
                    "dark fireball trajectory",
                );
                prior_tick = sample["tick"].as_u64().unwrap();
            }
        }
    }
    for case in golden["rainOfBones"].as_array().unwrap() {
        let mut rng = NativeRng::new(case["seed"].as_i64().unwrap());
        let mut state = create_native_rain_of_bones(Vec2::new(125.0, 240.0), &mut rng);
        assert_json_equal(&rain_state_json(state), &case["initial"], "rain birth");
        assert_json_equal(&json!(rng_record(&rng)), &case["initialRng"], "rain birth RNG");
        let mut samples = case["samples"].as_array().unwrap().iter().peekable();
        for tick in 1..=1400 {
            let step = step_native_rain_of_bones(state, &mut rng);
            state = step.state;
            let Some(expected) =
                samples.peek().filter(|sample| sample["tick"].as_u64() == Some(tick))
            else {
                continue;
            };
            let bone = step.bone.map(|bone| json!({"entry":bone.entry,"position":point_json(bone.position),"rotationDeg":bone.rotation_deg}));
            assert_json_equal(&json!(bone), &expected["bone"], "rain bone");
            assert_json_equal(&rain_state_json(state), &expected["state"], "rain state");
            assert_json_equal(&json!(rng_record(&rng)), &expected["rng"], "rain RNG");
            samples.next();
            if state.alpha <= 0.0 {
                break;
            }
        }
        assert!(samples.next().is_none());
    }
    for case in golden["direFires"].as_array().unwrap() {
        let mut rng = NativeRng::new(case["seed"].as_i64().unwrap());
        let mut state = create_native_dire_fire(&mut rng);
        assert_json_equal(&dire_fire_json(state), &case["initial"], "Dire Fire birth");
        assert_json_equal(&json!(rng_record(&rng)), &case["initialRng"], "Dire Fire birth RNG");
        let mut prior_tick = 0;
        for sample in case["samples"].as_array().unwrap() {
            for _ in prior_tick + 1..=sample["tick"].as_u64().unwrap() {
                state = step_native_dire_fire(state, &mut rng);
            }
            assert_json_equal(&dire_fire_json(state), &sample["state"], "Dire Fire state");
            assert_json_equal(&json!(rng_record(&rng)), &sample["rng"], "Dire Fire RNG");
            prior_tick = sample["tick"].as_u64().unwrap();
        }
    }
    for case in golden["guidedMissiles"].as_array().unwrap() {
        let mut rng = NativeRng::new(case["seed"].as_i64().unwrap());
        let mut state = create_native_guided_missile(&mut rng, Vec2::new(125.0, 240.0), 359.0, 1.5);
        assert_json_equal(&guided_missile_json(state), &case["initial"], "skull missile birth");
        assert_json_equal(&json!(rng_record(&rng)), &case["rng"], "skull missile birth RNG");
        let mut prior_tick = 0;
        for sample in case["samples"].as_array().unwrap() {
            for tick in prior_tick + 1..=sample["tick"].as_u64().unwrap() {
                let target = if tick % 200 < 100 { Some(Vec2::new(500.0, 150.0)) } else { None };
                let step = step_native_guided_missile(
                    state,
                    target,
                    if tick % 20 == 0 { 0.5 } else { 1.0 },
                );
                state = step.state;
                if tick == sample["tick"].as_u64().unwrap() {
                    assert_json_equal(
                        &point_json(step.terrain_probe),
                        &sample["terrainProbe"],
                        "skull missile probe",
                    );
                }
            }
            assert_json_equal(&guided_missile_json(state), &sample["state"], "skull missile state");
            prior_tick = sample["tick"].as_u64().unwrap();
        }
    }
    for sample in golden["contacts"].as_array().unwrap() {
        let origin = Vec2::new(125.0, 240.0);
        let point = point_from_json(&sample["point"]);
        assert_eq!(
            native_blightning_contains(origin, number(sample, "heading"), point),
            sample["blightning"].as_bool().unwrap()
        );
        assert_eq!(
            native_tragic_circle_contains(origin, point),
            sample["tragicCircle"].as_bool().unwrap()
        );
    }
    for case in golden["demonSkullActions"].as_array().unwrap() {
        let kind = match case["kind"].as_str().unwrap() {
            "scream" => DemonSkullActionKind::Scream,
            "bite" => DemonSkullActionKind::Bite,
            "eyes" => DemonSkullActionKind::Eyes,
            "mouth" => DemonSkullActionKind::Mouth,
            "spit" => DemonSkullActionKind::Spit,
            "flair" => DemonSkullActionKind::Flair,
            _ => panic!("source DemonSkull action"),
        };
        let action = create_native_demon_skull_action(
            kind,
            177.0,
            Some(231.0),
            1.25,
            case["seed"].as_u64().unwrap() as u32,
        );
        assert_json_equal(
            &demon_skull_action_json(&action),
            &case["action"],
            "DemonSkull action birth",
        );
    }
    for case in golden["demonSkullControllers"].as_array().unwrap() {
        let mut rng = NativeRng::new(case["seed"].as_i64().unwrap());
        let mut brain = create_native_demon_skull(&mut rng, 1.25, integer(case, "capabilities"));
        assert_json_equal(&skull_state_json(&brain), &case["initial"], "DemonSkull birth");
        assert_json_equal(&json!(rng_record(&rng)), &case["initialRng"], "DemonSkull birth RNG");
        let mut last_action_tick = -1000_i64;
        let mut samples = case["samples"].as_array().unwrap().iter().peekable();
        for tick in 1..=2600_i64 {
            prepare_native_demon_skull_tick(&mut brain, tick >= 10);
            let step = step_native_demon_skull_controller(
                &mut brain,
                &mut rng,
                NativeDemonSkullControllerContext {
                    alternate_player: tick % 500 < 100,
                    has_action: tick - last_action_tick < 100,
                    heading_deg: if tick % 250 < 125 { 30.0 } else { 200.0 },
                    health_ratio: if tick < 500 {
                        1.0
                    } else if tick < 1000 {
                        0.4
                    } else if tick < 1500 {
                        0.2
                    } else {
                        0.1
                    },
                    line_of_sight: tick % 300 < 240,
                    movement_scale: 1.25,
                    scream_playing: tick % 500 < 275,
                    target_distance_squared: if tick % 400 < 50 {
                        None
                    } else if tick % 400 < 200 {
                        Some(200.0 * 200.0)
                    } else {
                        Some(500.0 * 500.0)
                    },
                },
            );
            if let Some(kind) = step.begin_action {
                last_action_tick = tick;
                if !matches!(kind, DemonSkullActionKind::Scream | DemonSkullActionKind::Flair) {
                    brain.target_speed = 5.0;
                }
            }
            let Some(expected) =
                samples.peek().filter(|sample| sample["tick"].as_i64() == Some(tick))
            else {
                continue;
            };
            let label = format!("DemonSkull capabilities {} tick {tick}", case["capabilities"]);
            assert_json_equal(&skull_state_json(&brain), &expected["state"], &label);
            assert_json_equal(&json!(rng_record(&rng)), &expected["rng"], &label);
            assert_json_equal(
                &json!(step.begin_action.map(DemonSkullActionKind::id)),
                &expected["beginAction"],
                &label,
            );
            assert_json_equal(
                &json!(step.increment_attack_seed),
                &expected["incrementAttackSeed"],
                &label,
            );
            assert_json_equal(&json!(step.landing), &expected["landing"], &label);
            assert_json_equal(&json!(step.scream_shake), &expected["screamShake"], &label);
            samples.next();
        }
        assert!(samples.next().is_none());
    }
}
