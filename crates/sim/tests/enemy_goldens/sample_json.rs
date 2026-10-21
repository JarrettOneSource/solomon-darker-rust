//! Shared test-only source field projection, consumed by native endpoint and browser interpolation fixtures.
#![allow(dead_code)] // Separate test binaries use the reader and writer independently.
use serde_json::{Value, json};
use solomon_sim::Vec2;
use solomon_sim::enemies::actor_presentation::*;
use solomon_sim::enemies::model::FacultyActionKind;

pub fn point(value: &Value) -> Vec2 {
    Vec2::new(value["x"].as_f64().unwrap(), value["y"].as_f64().unwrap())
}

pub fn point_json(point: Vec2) -> Value {
    json!({"x":point.x,"y":point.y})
}

pub fn animation_json(sample: &NativeEnemyPresentationInput) -> Value {
    let state = match sample.state {
        NativeEnemyAnimationState::Idle => "idle",
        NativeEnemyAnimationState::Locomotion => "locomotion",
        NativeEnemyAnimationState::Action => "action",
        NativeEnemyAnimationState::Death => "death",
    };
    let coffin = match sample.coffin_state {
        NativeEnemyCoffinState::Hidden => "hidden",
        NativeEnemyCoffinState::Closed => "closed",
        NativeEnemyCoffinState::Opening => "opening",
        NativeEnemyCoffinState::Open => "open",
    };
    let mut animation = serde_json::Map::new();
    animation.insert("state".into(), json!(state));
    animation.insert("action".into(), json!(sample.action.map(NativeEnemyAction::id)));
    animation.insert("deathEpoch".into(), json!(sample.death_epoch));
    animation.insert("coffinState".into(), json!(coffin));
    animation.insert("actionProgress".into(), json!(sample.action_progress));
    animation.insert("alpha".into(), json!(sample.alpha));
    animation.insert("bodyGaitPhase".into(), json!(sample.body_gait_phase));
    animation.insert("bodyPose".into(), json!(sample.body_pose));
    animation.insert("coffinPose".into(), json!(sample.coffin_pose));
    animation.insert("coffinRotationRadians".into(), json!(sample.coffin_rotation_radians));
    animation.insert("coffinScaleX".into(), json!(sample.coffin_scale_x));
    animation.insert("deathTick".into(), json!(sample.death_tick));
    animation
        .insert("demonFrontRotationRadians".into(), json!(sample.demon_front_rotation_radians));
    animation.insert("demonRearRotationRadians".into(), json!(sample.demon_rear_rotation_radians));
    animation.insert("gaitPose".into(), json!(sample.gait_pose));
    animation.insert("hitFlash".into(), json!(sample.hit_flash));
    animation.insert("impBodyRotationRadians".into(), json!(sample.imp_body_rotation_radians));
    animation.insert("impEffectAlpha".into(), json!(sample.imp_effect_alpha));
    animation.insert("impEffectFrame".into(), json!(sample.imp_effect_frame));
    animation.insert("shadowLateralOffset".into(), json!(sample.shadow_lateral_offset));
    animation.insert("stridePhaseDeg".into(), json!(sample.stride_phase_deg));
    animation.insert("verticalOffset".into(), json!(sample.vertical_offset));
    animation.insert("zombieAngularOffsetDeg".into(), json!(sample.zombie_angular_offset_deg));
    animation.insert("zombieFrontArmPose".into(), json!(sample.zombie_front_arm_pose));
    animation.insert("zombieRearArmPose".into(), json!(sample.zombie_rear_arm_pose));
    animation
        .insert("zombieBodyRotationRadians".into(), json!(sample.zombie_body_rotation_radians));
    animation.insert(
        "zombieArmSocketRotationRadians".into(),
        json!(sample.zombie_arm_socket_rotation_radians),
    );
    animation.insert(
        "zombieFrontArmRotationRadians".into(),
        json!(sample.zombie_front_arm_rotation_radians),
    );
    animation.insert(
        "zombieRearArmRotationRadians".into(),
        json!(sample.zombie_rear_arm_rotation_radians),
    );
    animation
        .insert("zombieHeadRotationRadians".into(), json!(sample.zombie_head_rotation_radians));
    animation.insert(
        "demonFrontExtremityOffset".into(),
        point_json(sample.demon_front_extremity_offset),
    );
    animation
        .insert("demonRearExtremityOffset".into(), point_json(sample.demon_rear_extremity_offset));
    animation.insert("demonShadowOffset".into(), point_json(sample.demon_shadow_offset));
    animation.insert("pikeTargetOffset".into(), json!(sample.pike_target_offset.map(point_json)));
    animation.insert("spider".into(),json!(sample.spider.map(|spider| json!({
        "bodyHeadingDeg":spider.body_heading_deg,"outlineAlpha":spider.outline_alpha,"outlineTint":spider.outline_tint,
    }))));
    let effects: Vec<_> = sample
        .effects
        .iter()
        .map(|effect| {
            json!({
                "id":effect.id,"role":effect.layer.role.rsplit(':').next().unwrap(),
                "alpha":effect.layer.alpha,"offset":point_json(effect.layer.offset),
                "rotationRadians":effect.layer.rotation_radians,"scale":effect.layer.scale,
            })
        })
        .collect();
    animation.insert("effects".into(), json!(effects));
    Value::Object(animation)
}

pub fn animation_from_json(value: &Value) -> NativeEnemyPresentationInput {
    let mut sample = NativeEnemyPresentationInput {
        state: match value["state"].as_str().unwrap() {
            "idle" => NativeEnemyAnimationState::Idle,
            "locomotion" => NativeEnemyAnimationState::Locomotion,
            "action" => NativeEnemyAnimationState::Action,
            "death" => NativeEnemyAnimationState::Death,
            state => panic!("unknown source animation state {state}"),
        },
        death_epoch: value["deathEpoch"].as_u64().unwrap(),
        coffin_state: match value["coffinState"].as_str().unwrap() {
            "hidden" => NativeEnemyCoffinState::Hidden,
            "closed" => NativeEnemyCoffinState::Closed,
            "opening" => NativeEnemyCoffinState::Opening,
            "open" => NativeEnemyCoffinState::Open,
            state => panic!("unknown source Coffin state {state}"),
        },
        ..Default::default()
    };
    let actions = [
        NativeEnemyAction::SkeletonClawA,
        NativeEnemyAction::SkeletonClawB,
        NativeEnemyAction::SkeletonWeapon,
        NativeEnemyAction::SkeletonPike,
        NativeEnemyAction::ArcherShot,
        NativeEnemyAction::MageCastShort,
        NativeEnemyAction::MageCastLong,
        NativeEnemyAction::ZombieBeat,
        NativeEnemyAction::WraithDrain,
        NativeEnemyAction::DemonBomb,
        NativeEnemyAction::Faculty(FacultyActionKind::Throw),
        NativeEnemyAction::Faculty(FacultyActionKind::TwoHand),
        NativeEnemyAction::Faculty(FacultyActionKind::Lightning),
        NativeEnemyAction::DemonSkullBite,
        NativeEnemyAction::DemonSkullEyes,
        NativeEnemyAction::DemonSkullMouth,
        NativeEnemyAction::DemonSkullSpit,
        NativeEnemyAction::DemonSkullFlair,
        NativeEnemyAction::DemonSkullScream,
    ];
    sample.action = value["action"]
        .as_str()
        .map(|action| *actions.iter().find(|candidate| candidate.id() == action).unwrap());
    sample.action_progress = value["actionProgress"].as_f64().unwrap();
    sample.alpha = value["alpha"].as_f64().unwrap();
    sample.body_gait_phase = value["bodyGaitPhase"].as_f64().unwrap();
    sample.body_pose = value["bodyPose"].as_f64().unwrap();
    sample.coffin_pose = value["coffinPose"].as_f64().unwrap();
    sample.coffin_rotation_radians = value["coffinRotationRadians"].as_f64().unwrap();
    sample.coffin_scale_x = value["coffinScaleX"].as_f64().unwrap();
    sample.death_tick = value["deathTick"].as_f64().unwrap();
    sample.demon_front_rotation_radians = value["demonFrontRotationRadians"].as_f64().unwrap();
    sample.demon_rear_rotation_radians = value["demonRearRotationRadians"].as_f64().unwrap();
    sample.gait_pose = value["gaitPose"].as_f64().unwrap();
    sample.hit_flash = value["hitFlash"].as_f64().unwrap();
    sample.imp_body_rotation_radians = value["impBodyRotationRadians"].as_f64().unwrap();
    sample.imp_effect_alpha = value["impEffectAlpha"].as_f64().unwrap();
    sample.imp_effect_frame = value["impEffectFrame"].as_f64().unwrap();
    sample.shadow_lateral_offset = value["shadowLateralOffset"].as_f64().unwrap();
    sample.stride_phase_deg = value["stridePhaseDeg"].as_f64().unwrap();
    sample.vertical_offset = value["verticalOffset"].as_f64().unwrap();
    sample.zombie_angular_offset_deg = value["zombieAngularOffsetDeg"].as_f64().unwrap();
    sample.zombie_front_arm_pose = value["zombieFrontArmPose"].as_f64().unwrap();
    sample.zombie_rear_arm_pose = value["zombieRearArmPose"].as_f64().unwrap();
    sample.zombie_body_rotation_radians = value["zombieBodyRotationRadians"].as_f64().unwrap();
    sample.zombie_arm_socket_rotation_radians =
        value["zombieArmSocketRotationRadians"].as_f64().unwrap();
    sample.zombie_front_arm_rotation_radians =
        value["zombieFrontArmRotationRadians"].as_f64().unwrap();
    sample.zombie_rear_arm_rotation_radians =
        value["zombieRearArmRotationRadians"].as_f64().unwrap();
    sample.zombie_head_rotation_radians = value["zombieHeadRotationRadians"].as_f64().unwrap();
    sample.demon_front_extremity_offset = point(&value["demonFrontExtremityOffset"]);
    sample.demon_rear_extremity_offset = point(&value["demonRearExtremityOffset"]);
    sample.demon_shadow_offset = point(&value["demonShadowOffset"]);
    sample.pike_target_offset =
        value["pikeTargetOffset"].as_object().map(|_| point(&value["pikeTargetOffset"]));
    sample.spider = value["spider"].as_object().map(|_| NativeEnemySpiderInput {
        body_heading_deg: value["spider"]["bodyHeadingDeg"].as_f64().unwrap(),
        outline_alpha: value["spider"]["outlineAlpha"].as_f64().unwrap(),
        outline_tint: value["spider"]["outlineTint"].as_u64().unwrap() as u32,
    });
    sample.effects = value["effects"]
        .as_array()
        .unwrap()
        .iter()
        .map(|effect| NativeEnemyEffectInput {
            id: effect["id"].as_u64().unwrap() as usize,
            layer: EnemySpriteLayer {
                alpha: effect["alpha"].as_f64().unwrap(),
                offset: point(&effect["offset"]),
                rotation_radians: effect["rotationRadians"].as_f64().unwrap(),
                scale: effect["scale"].as_f64().unwrap(),
                ..EnemySpriteLayer::new(
                    EnemyArtAtlas::BadGuys,
                    49,
                    effect["role"].as_str().unwrap(),
                )
            },
        })
        .collect();
    sample
}
