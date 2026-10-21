//! Entity providers are exact before the manager's common submission rounding.

mod support;

use serde_json::{Value, json};
use solomon_sim::Vec2;
use solomon_sim::boneyard::lighting::NativeBoneyardLightSource;
use solomon_sim::boneyard::lighting::enemy::*;
use solomon_sim::boneyard::lighting::secondary::*;
use solomon_sim::boneyard::lighting::weld::*;
use solomon_sim::enemies::EnemyToken;
use solomon_sim::enemies::model::EnemyLighting;
use solomon_sim::enemies::projectiles::{EnemyProjectileKind, EnemyProjectilePayload};
use solomon_sim::painter::WorldManagerLane;
use solomon_sim::secondary::{NativeSecondaryActorKind, NativeSecondaryActorState};

fn point(value: &Value) -> Vec2 {
    Vec2::new(value["x"].as_f64().unwrap(), value["y"].as_f64().unwrap())
}

fn source(value: NativeBoneyardLightSource) -> Value {
    let mut result = json!({"castsDirectionalShadow":value.casts_directional_shadow,"intensity":value.intensity,
        "position":{"x":value.position.x,"y":value.position.y},"radius":value.radius});
    if let Some(scale) = value.raster_scale {
        result["rasterScale"] = json!(scale);
    }
    result
}

fn candidate(value: Option<NativeBoneyardLightProviderCandidate>) -> Value {
    value.map_or(Value::Null,|value|json!({"lane":match value.lane { WorldManagerLane::Actor => "actor",WorldManagerLane::Transient => "transient" },"source":source(value.source)}))
}

#[test]
fn enemy_projectile_secondary_and_weld_providers_replay_source_values() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/entity-light-goldens.json")).unwrap();
    for (index, row) in fixture["enemies"].as_array().unwrap().iter().enumerate() {
        let value = &row["owner"];
        let owner = NativeEnemyLightOwner {
            burning: value["burning"].as_bool().unwrap(),
            scale: value["scale"].as_f64().unwrap(),
            demon_skull_body_offset: point(&value["demonSkull"]["bodyOffset"]),
            id: value["id"].as_u64().unwrap() as usize,
            enemy_token: EnemyToken::from_id(value["enemyToken"].as_str().unwrap()).unwrap(),
            position: point(&value["position"]),
            dying: value["animation"]["state"] == "death",
            alpha: value["animation"]["alpha"].as_f64().unwrap(),
            body_pose: value["animation"]["bodyPose"].as_f64().unwrap(),
            lighting: EnemyLighting {
                charge: value["lighting"]["charge"].as_f64().unwrap(),
                glow: value["lighting"]["glow"].as_f64().unwrap(),
                provider_copies: value["lighting"]["providerCopies"].as_u64().unwrap() as u8,
            },
        };
        let sources = native_enemy_light_sources(
            owner,
            row["frame"].as_u64().unwrap(),
            row["multipleShadows"].as_bool().unwrap(),
        );
        support::assert_website_json(
            &json!(sources.into_iter().map(source).collect::<Vec<_>>()),
            &row["expected"],
            &format!("enemy[{index}]"),
            &[],
        );
    }
    for (index, row) in fixture["projectiles"].as_array().unwrap().iter().enumerate() {
        let value = &row["owner"];
        let kind = match value["kind"].as_str().unwrap() {
            "arrow" => EnemyProjectileKind::Arrow,
            "firebolt" => EnemyProjectileKind::Firebolt,
            "guided-missile" => EnemyProjectileKind::GuidedMissile,
            "demon-bomb" => EnemyProjectileKind::DemonBomb,
            "poison-pool" => EnemyProjectileKind::PoisonPool,
            _ => panic!("unknown source projectile"),
        };
        let payload = match value["payload"].as_str().unwrap() {
            "none" => EnemyProjectilePayload::None,
            "normal" => EnemyProjectilePayload::Normal,
            "fire" => EnemyProjectilePayload::Fire,
            "cold" => EnemyProjectilePayload::Cold,
            "poison" => EnemyProjectilePayload::Poison,
            _ => panic!("unknown payload"),
        };
        let actual = native_enemy_projectile_light_provider(
            kind,
            payload,
            value["id"].as_u64().unwrap() as usize,
            point(&value["position"]),
            row["frame"].as_u64().unwrap(),
            row["multipleShadows"].as_bool().unwrap(),
        );
        support::assert_website_json(
            &candidate(actual),
            &row["expected"],
            &format!("projectile[{index}]"),
            &[],
        );
    }
    for (index, row) in fixture["effects"].as_array().unwrap().iter().enumerate() {
        let value = &row["owner"];
        let kind = match value["kind"].as_str().unwrap() {
            "demon-fire" => NativeEnemyProjectileEffectLightKind::DemonFire,
            "demon-explosion-lit-array" => {
                NativeEnemyProjectileEffectLightKind::DemonExplosionLitArray
            }
            "fire-burst" => NativeEnemyProjectileEffectLightKind::FireBurst,
            "guided-impact" => NativeEnemyProjectileEffectLightKind::GuidedImpact,
            _ => NativeEnemyProjectileEffectLightKind::Unlit,
        };
        let actual = native_enemy_projectile_effect_light_provider(
            kind,
            value["ageTicks"].as_u64().unwrap(),
            value["alpha"].as_f64().unwrap(),
            point(&value["position"]),
            !value["lightRegistration"].is_null(),
            row["multipleShadows"].as_bool().unwrap(),
            row["pointGain"].as_f64().unwrap(),
        );
        support::assert_website_json(
            &candidate(actual),
            &row["expected"],
            &format!("effect[{index}]"),
            &[],
        );
    }
    for (index, row) in fixture["secondary"].as_array().unwrap().iter().enumerate() {
        let value = &row["owner"];
        let kind = NativeSecondaryActorKind::ALL
            .into_iter()
            .find(|kind| kind.as_str() == value["kind"].as_str().unwrap())
            .unwrap();
        let mut actor = NativeSecondaryActorState::new(
            value["id"].as_u64().unwrap(),
            kind,
            "local".into(),
            "boneyard:test".into(),
            None,
        );
        actor.age_ticks = value["ageTicks"].as_u64().unwrap();
        actor.variant = value["variant"].as_i64().unwrap() as i32;
        actor.position = point(&value["position"]);
        actor.radius = value["radius"].as_f64().unwrap();
        actor.alpha = value["alpha"].as_f64().unwrap();
        actor.phase = value["phase"].as_f64().unwrap();
        actor.scale = value["scale"].as_f64().unwrap();
        actor.slow_factor = value["slowFactor"].as_f64().unwrap();
        let actual = native_secondary_provider_light_source(
            &actor,
            row["frame"].as_u64().unwrap(),
            row["multipleShadows"].as_bool().unwrap(),
            row["pointGain"].as_f64().unwrap(),
        );
        support::assert_website_json(
            &actual.map_or(Value::Null, source),
            &row["expected"],
            &format!("secondary[{index}]"),
            &[],
        );
        support::assert_website_json(
            &native_secondary_misc_light_source(&actor).map_or(Value::Null, source),
            &row["misc"],
            &format!("secondary-misc[{index}]"),
            &[],
        );
    }
    for (index, row) in fixture["weld"].as_array().unwrap().iter().enumerate() {
        let value = &row["owner"];
        let actual = match row["kind"].as_str().unwrap() {
            "projectile" => Some(native_weld_projectile_light_source(
                value["buildId"].as_u64().unwrap() as u32,
                value["id"].as_u64().unwrap() as usize,
                point(&value["position"]),
                row["frame"].as_u64().unwrap(),
                row["multipleShadows"].as_bool().unwrap(),
            )),
            "rock" => Some(native_weld_rock_light_source(
                point(&value["origin"]),
                value["scale"].as_f64().unwrap(),
                row["multipleShadows"].as_bool().unwrap(),
            )),
            "meteor" => native_weld_meteor_light_source(
                point(&value["position"]),
                value["fallHeight"].as_f64().unwrap(),
                value["phase"] == "impact",
                value["impactTicksRemaining"].as_u64().unwrap(),
                value["impactRadiusScalar"].as_f64().unwrap(),
            ),
            _ => panic!("unknown source weld light"),
        };
        support::assert_website_json(
            &actual.map_or(Value::Null, source),
            &row["expected"],
            &format!("weld[{index}]"),
            &[],
        );
    }
}
