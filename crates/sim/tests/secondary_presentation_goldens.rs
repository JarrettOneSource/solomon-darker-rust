//! Source-shaped draw plans over actual secondary states and boundary snapshots.
#![cfg(feature = "persistence")]

#[path = "support/native_bits.rs"]
#[allow(dead_code)]
mod native_bits;

use serde_json::{Value, json};
use solomon_sim::enemies::presentation::*;
use solomon_sim::secondary::golem::{NativeGolemPhase, NativeSecondaryGolemState};
use solomon_sim::secondary::presentation::*;
use solomon_sim::secondary::{
    NativeSecondaryActorKind, NativeSecondaryActorState, NativeSecondaryPlayerState,
};
use solomon_sim::spells::presentation::{SpellAtlas, SpellBlend};
use solomon_sim::{NativeRng, Vec2};

fn point(value: &Value) -> Vec2 {
    Vec2::new(value["x"].as_f64().unwrap(), value["y"].as_f64().unwrap())
}
fn point_snapshot(value: Vec2) -> Value {
    json!({"x":value.x,"y":value.y})
}
fn read_rng(value: &Value) -> NativeRng {
    let mut rng = NativeRng::new(0);
    rng.index_a = value["indexA"].as_u64().unwrap() as usize;
    rng.index_b = value["indexB"].as_u64().unwrap() as usize;
    for (word, value) in rng.words.iter_mut().zip(value["words"].as_array().unwrap()) {
        *word = value.as_u64().unwrap() as u32;
    }
    rng
}
fn rng_snapshot(value: &NativeRng) -> Value {
    json!({"indexA":value.index_a,"indexB":value.index_b,"words":&value.words[..]})
}
fn read_actor(value: &Value) -> NativeSecondaryActorState {
    let kind = NativeSecondaryActorKind::ALL
        .into_iter()
        .find(|kind| Some(kind.as_str()) == value["kind"].as_str())
        .unwrap();
    let mut actor = NativeSecondaryActorState::new(
        value["id"].as_u64().unwrap(),
        kind,
        value["ownerId"].as_str().unwrap().into(),
        value["worldKey"].as_str().unwrap().into(),
        value["skillId"].as_u64().map(|id| id as usize),
    );
    actor.age_ticks = value["ageTicks"].as_u64().unwrap();
    actor.alpha = value["alpha"].as_f64().unwrap();
    actor.damage = value["damage"].as_f64().unwrap();
    actor.enhanced = value["enhanced"].as_bool().unwrap();
    actor.endpoint = point(&value["endpoint"]);
    actor.frame = value["frame"].as_f64().unwrap();
    actor.freeze_ticks = value["freezeTicks"].as_u64().unwrap();
    actor.golem = value["golem"].as_object().map(|_| golem(&value["golem"]));
    actor.golem_death =
        value["golemDeath"].as_object().map(|_| native_bits::input(&value["golemDeath"]));
    actor.ether_drain =
        value["etherDrain"].as_object().map(|_| native_bits::input(&value["etherDrain"]));
    actor.hit_target_ids =
        value["hitTargetIds"].as_array().unwrap().iter().map(|id| id.as_u64().unwrap()).collect();
    actor.lifetime_ticks = value["lifetimeTicks"].as_u64().unwrap();
    actor.midpoint = point(&value["midpoint"]);
    actor.phase = value["phase"].as_f64().unwrap();
    actor.position = point(&value["position"]);
    actor.presentation_rng =
        value["presentationRng"].as_object().map(|_| read_rng(&value["presentationRng"]));
    actor.quantity = value["quantity"].as_f64().unwrap();
    actor.radius = value["radius"].as_f64().unwrap();
    actor.rank = value["rank"].as_u64().unwrap() as u32;
    actor.rotation_radians = value["rotationRadians"].as_f64().unwrap();
    actor.scale = value["scale"].as_f64().unwrap();
    actor.slow_factor = value["slowFactor"].as_f64().unwrap();
    actor.target_id = value["targetId"].as_u64().map(|id| id as usize);
    actor.variant = value["variant"].as_i64().unwrap() as i32;
    actor.velocity = point(&value["velocity"]);
    actor
}
fn blend(value: NativeSecondaryBlend) -> &'static str {
    match value {
        NativeSecondaryBlend::Normal => "normal",
        NativeSecondaryBlend::Add => "add",
    }
}
fn sprite(value: &NativeSecondarySpriteDraw) -> Value {
    let mut result = json!({
        "alpha": value.alpha,
        "atlas": value.atlas.as_str(),
        "blend": blend(value.blend),
        "entry": value.entry,
        "offset": point_snapshot(value.offset),
        "role": value.role,
        "rotationRadians": value.rotation_radians,
        "scaleX": value.scale_x,
        "scaleY": value.scale_y,
        "tint": value.tint,
    });
    if let Some(mode) = value.color_mode {
        result["colorMode"] = json!(match mode {
            NativeSecondaryColorMode::AlphaMask => "alpha-mask",
            NativeSecondaryColorMode::Texture => "texture",
        });
    }
    result
}
fn plan(value: &NativeSecondaryPresentationPlan) -> Value {
    let gradients = value
        .gradients
        .iter()
        .map(|draw| {
            json!({
                "bottomAlpha": draw.bottom_alpha,
                "bottomColor": draw.bottom_color,
                "height": draw.height,
                "role": draw.role,
                "topAlpha": draw.top_alpha,
                "topColor": draw.top_color,
                "topLeft": point_snapshot(draw.top_left),
                "width": draw.width,
            })
        })
        .collect::<Vec<_>>();
    let meshes = value
        .meshes
        .iter()
        .map(|draw| {
            json!({
                "alpha": draw.alpha,
                "blend": blend(draw.blend),
                "indices": draw.indices,
                "role": draw.role,
                "texture": "ether-plane",
                "tint": draw.tint,
                "uvs": draw.uvs,
                "vertexColors": draw.vertex_colors,
                "vertices": draw.vertices,
            })
        })
        .collect::<Vec<_>>();
    let quads = value
        .quads
        .iter()
        .map(|draw| {
            json!({
                "alpha": draw.alpha,
                "atlas": draw.atlas.map(NativeSecondaryAtlas::as_str),
                "blend": blend(draw.blend),
                "entry": draw.entry,
                "role": draw.role,
                "vertices": draw.vertices,
                "tint": draw.tint,
            })
        })
        .collect::<Vec<_>>();
    let composite = value.storm_composite.as_ref().map(|composite| {
        json!({
            "draws": composite.draws.iter().map(sprite).collect::<Vec<_>>(),
            "offset": point_snapshot(composite.offset),
            "scale": composite.scale,
        })
    });
    json!({
        "backgroundDraws": value.background_draws.iter().map(sprite).collect::<Vec<_>>(),
        "draws": value.draws.iter().map(sprite).collect::<Vec<_>>(),
        "gradients": gradients,
        "meshes": meshes,
        "quads": quads,
        "queueFamily": match value.queue_family {
            NativeSecondaryQueueFamily::OrdinaryDynamic => "ordinary-dynamic",
            NativeSecondaryQueueFamily::Zanim => "zanim",
        },
        "root": point_snapshot(value.root),
        "sortBias": value.sort_bias,
        "stormComposite": composite,
        "underlayDraws": value.underlay_draws.iter().map(sprite).collect::<Vec<_>>(),
        "worldY": value.world_y,
    })
}
fn enemy_layer(value: &NativeEnemyProjectileLayer) -> Value {
    let mut result = json!({
        "alpha": value.alpha,
        "atlas": match value.atlas {
            SpellAtlas::BadGuys => "BadGuys",
            SpellAtlas::DeadHawg => "DeadHawg",
        },
        "blendMode": match value.blend {
            SpellBlend::Normal => "normal",
            SpellBlend::Add => "add",
        },
        "entry": value.entry,
        "offset": point_snapshot(value.offset),
        "role": value.role,
        "rotationRadians": value.rotation_radians,
        "scale": value.scale,
        "tint": value.tint,
    });
    if let Some(scale) = value.scale_y {
        result["scaleY"] = json!(scale);
    }
    result
}
fn assert_snapshot(actual: &Value, expected: &Value, label: &str) {
    match (actual, expected) {
        (Value::Number(a), Value::Number(b)) => {
            let actual = a.as_f64().unwrap();
            let expected = b.as_f64().unwrap();
            if label.ends_with("offset.x") || label.ends_with("offset.y") {
                // Unrounded sine/cosine offsets use the repository's trig tolerance.
                assert!((actual - expected).abs() <= 1e-9, "{label}: {actual} != {expected}");
            } else {
                assert_eq!(actual, expected, "{label}");
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len(), "{label} length");
            for (index, (a, b)) in a.iter().zip(b).enumerate() {
                assert_snapshot(a, b, &format!("{label}[{index}]"));
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(a.len(), b.len(), "{label} fields");
            for (key, b) in b {
                assert_snapshot(
                    a.get(key).unwrap_or_else(|| panic!("{label}.{key} is absent")),
                    b,
                    &format!("{label}.{key}"),
                );
            }
        }
        _ => assert_eq!(actual, expected, "{label}"),
    }
}

#[test]
fn fractional_secondary_display_ages_match_source_rounding() {
    use solomon_sim::boneyard::lighting::secondary::{
        native_secondary_misc_light_source_at_age, native_secondary_provider_light_source_at_age,
    };

    let fixture: Value = serde_json::from_str(include_str!(
        "fixtures/website/secondary-fractional-presentation-goldens.json"
    ))
    .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let actor = read_actor(&case["actor"]);
        let actual = native_secondary_presentation_plan_at_age(
            &actor,
            case["ageTicks"].as_f64().unwrap(),
            case["frame"].as_f64().unwrap(),
            case["pointGain"].as_f64().unwrap(),
            case["enhancedEffects"].as_bool().unwrap_or(actor.enhanced),
        );
        assert_snapshot(&plan(&actual), &case["plan"], case["name"].as_str().unwrap());
    }
    for case in fixture["lights"].as_array().unwrap() {
        let actor = read_actor(&case["actor"]);
        let age = case["ageTicks"].as_f64().unwrap();
        let provider = native_secondary_provider_light_source_at_age(
            &actor,
            age,
            case["frame"].as_u64().unwrap(),
            true,
            0.75,
        );
        let misc = native_secondary_misc_light_source_at_age(&actor, age);
        for (name, source) in [("provider", provider), ("misc", misc)] {
            let actual = match source {
                Some(source) => json!({
                    "castsDirectionalShadow": source.casts_directional_shadow,
                    "intensity": source.intensity,
                    "position": point_snapshot(source.position),
                    "radius": source.radius,
                }),
                None => Value::Null,
            };
            assert_snapshot(&actual, &case[name], actor.kind.as_str());
        }
    }
}

#[test]
fn all_secondary_stock_draw_plans_replay_source_actor_snapshots() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/secondary-presentation-goldens.json"))
            .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let actor = read_actor(&case["actor"]);
        let actual = native_secondary_presentation_plan(
            &actor,
            case["frame"].as_f64().unwrap(),
            case["pointGain"].as_f64().unwrap(),
        );
        assert_snapshot(&plan(&actual), &case["plan"], case["name"].as_str().unwrap());
    }
    for case in fixture["owners"].as_array().unwrap() {
        let actors = case["actors"].as_array().unwrap().iter().map(read_actor).collect::<Vec<_>>();
        let entries =
            native_secondary_composite_owner_entries(&actors, case["worldKey"].as_str().unwrap());
        assert_eq!(json!(entries), case["entries"], "live composite ownership");
    }
    for case in fixture["shields"].as_array().unwrap() {
        let value = &case["player"];
        let player = NativeSecondaryPlayerState {
            magic_shield_absorb: value["magicShieldAbsorb"].as_f64().unwrap(),
            magic_shield_pulse_ticks: value["magicShieldPulseTicks"].as_u64().unwrap(),
            ..NativeSecondaryPlayerState::default()
        };
        let actual = native_player_magic_shield_plan(Some(&player), case["tick"].as_f64().unwrap());
        assert_snapshot(
            &json!({
                "alpha": actual.alpha,
                "scale": actual.scale,
                "tint": actual.tint,
                "visible": actual.visible,
            }),
            &case["plan"],
            "Shield plan",
        );
    }
    for case in fixture["composites"].as_array().unwrap() {
        let actual = native_leviathan_composite_plan(case["scale"].as_f64().unwrap());
        assert_snapshot(
            &json!({
                "clear": {
                    "blend": "multiply",
                    "color": actual.clear.color,
                    "height": actual.clear.height,
                    "width": actual.clear.width,
                    "x": actual.clear.x,
                    "y": actual.clear.y,
                },
                "mask": {
                    "blend": "multiply",
                    "clipSize": actual.mask.clip_size,
                    "clipTop": actual.mask.clip_top,
                    "entry": actual.mask.entry,
                    "scale": actual.mask.scale,
                },
                "outputs": actual.outputs.map(|output| json!({
                    "alpha": output.alpha,
                    "blend": blend(output.blend),
                })),
            }),
            &case["plan"],
            "Leviathan composite",
        );
    }
    for case in fixture["facing"].as_array().unwrap() {
        assert_eq!(
            native_golem_facing(case["heading"].as_f64().unwrap()),
            case["facing"].as_u64().unwrap() as usize
        );
    }
    // Complete these independent projectile/DeathMagic rows even when one
    // differs, so the shared replay reports every remaining source boundary.
    let mut additional_failures = Vec::new();
    for case in fixture["projectiles"].as_array().unwrap() {
        let value = &case["actor"];
        let kind = match value["kind"].as_str().unwrap() {
            "arrow" => NativeEnemyProjectileKind::Arrow,
            "firebolt" => NativeEnemyProjectileKind::Firebolt,
            "guided-missile" => NativeEnemyProjectileKind::GuidedMissile,
            "demon-bomb" => NativeEnemyProjectileKind::DemonBomb,
            "poison-pool" => NativeEnemyProjectileKind::PoisonPool,
            other => panic!("unknown kind {other}"),
        };
        let payload = match value["payload"].as_str().unwrap() {
            "normal" => NativeEnemyProjectilePayload::Normal,
            "fire" => NativeEnemyProjectilePayload::Fire,
            "poison" => NativeEnemyProjectilePayload::Poison,
            "cold" => NativeEnemyProjectilePayload::Cold,
            "none" => NativeEnemyProjectilePayload::None,
            other => panic!("unknown payload {other}"),
        };
        let actual = native_enemy_projectile_plan(
            &NativeEnemyProjectileVisual {
                age_ticks: value["ageTicks"].as_u64().unwrap(),
                heading_degrees: value["headingDeg"].as_f64().unwrap(),
                id: value["id"].as_u64().unwrap(),
                kind,
                lifetime_ticks: value["lifetimeTicks"].as_u64().unwrap(),
                payload,
                position: point(&value["position"]),
                speed: value["speed"].as_f64().unwrap(),
                vertical_offset: value["verticalOffset"].as_f64().unwrap(),
                visual_phase_degrees: value["visualPhaseDeg"].as_f64().unwrap(),
                visual_scale: value["visualScale"].as_f64().unwrap(),
            },
            case["tick"].as_f64().unwrap(),
        );
        let context = format!(
            "Enemy projectile {kind:?}/{payload:?} age{} height{}",
            value["ageTicks"], value["verticalOffset"]
        );
        collect_snapshot_failure(
            &json!({
                "layers": actual.layers.iter().map(enemy_layer).collect::<Vec<_>>(),
                "underlays": actual.underlays.iter().map(enemy_layer).collect::<Vec<_>>(),
                "position": point_snapshot(actual.position),
                "streak": actual.streak.map(|streak| json!({
                    "alpha": streak.alpha,
                    "end": point_snapshot(streak.end),
                    "start": point_snapshot(streak.start),
                    "width": streak.width,
                })),
            }),
            &case["plan"],
            &context,
            &mut additional_failures,
        );
    }
    for case in fixture["deathMagic"].as_array().unwrap() {
        let mut rng = NativeRng::new(case["seed"].as_i64().unwrap());
        let layers = native_death_magic_layers(
            Vec2::new(0.25, -0.5),
            1.25,
            case["phase"].as_f64().unwrap(),
            &mut rng,
        );
        collect_snapshot_failure(
            &json!({
                "layers": layers.iter().map(enemy_layer).collect::<Vec<_>>(),
                "rng": rng_snapshot(&rng),
            }),
            &case["result"],
            &format!("DeathMagic seed{} phase{}", case["seed"], case["phase"]),
            &mut additional_failures,
        );
    }
    assert!(
        additional_failures.is_empty(),
        "source presenter cases diverged: {}",
        additional_failures.join(", ")
    );
}

fn collect_snapshot_failure(
    actual: &Value,
    expected: &Value,
    context: &str,
    failures: &mut Vec<String>,
) {
    if std::panic::catch_unwind(|| assert_snapshot(actual, expected, context)).is_err() {
        failures.push(context.to_owned());
    }
}

fn golem(value: &Value) -> NativeSecondaryGolemState {
    NativeSecondaryGolemState {
        action_heading_offset_degrees: value["actionHeadingOffsetDegrees"].as_f64().unwrap(),
        action_duration_ticks: value["actionDurationTicks"].as_u64().unwrap(),
        action_tick: value["actionTick"].as_u64().unwrap(),
        circle_slow_ticks: value["circleSlowTicks"].as_u64().unwrap(),
        current_health: value["currentHealth"].as_f64().unwrap(),
        damage_maximum: value["damageMaximum"].as_f64().unwrap(),
        iron: value["iron"].as_bool().unwrap(),
        gait_tick: value["gaitTick"].as_u64().unwrap(),
        left_connector_offset: point(&value["leftConnectorOffset"]),
        left_foot: point(&value["leftFoot"]),
        left_foot_bob: point(&value["leftFootBob"]),
        left_foot_next: point(&value["leftFootNext"]),
        left_foot_previous: point(&value["leftFootPrevious"]),
        left_foot_progress: value["leftFootProgress"].as_f64().unwrap(),
        left_foot_rotation_degrees: value["leftFootRotationDegrees"].as_f64().unwrap(),
        left_limb_mode: value["leftLimbMode"].as_u64().unwrap() as u32,
        maximum_health: value["maximumHealth"].as_f64().unwrap(),
        orbit_direction: value["orbitDirection"].as_f64().unwrap(),
        orbit_heading_radians: value["orbitHeadingRadians"].as_f64(),
        phase: match value["phase"].as_str().unwrap() {
            "active" => NativeGolemPhase::Active,
            "assembly" => NativeGolemPhase::Assembly,
            "attack" => NativeGolemPhase::Attack,
            "provoke" => NativeGolemPhase::Provoke,
            other => panic!("unknown phase {other}"),
        },
        pose_variant: value["poseVariant"].as_u64().unwrap() as u32,
        provoke_roll_bound: value["provokeRollBound"].as_i64().unwrap() as i32,
        reflect_factor: value["reflectFactor"].as_f64().unwrap(),
        right_connector_offset: point(&value["rightConnectorOffset"]),
        right_foot: point(&value["rightFoot"]),
        right_foot_bob: point(&value["rightFootBob"]),
        right_foot_next: point(&value["rightFootNext"]),
        right_foot_previous: point(&value["rightFootPrevious"]),
        right_foot_progress: value["rightFootProgress"].as_f64().unwrap(),
        right_foot_rotation_degrees: value["rightFootRotationDegrees"].as_f64().unwrap(),
        right_limb_mode: value["rightLimbMode"].as_u64().unwrap() as u32,
        target_poll_ticks_remaining: value["targetPollTicksRemaining"].as_i64().unwrap() as i32,
    }
}
