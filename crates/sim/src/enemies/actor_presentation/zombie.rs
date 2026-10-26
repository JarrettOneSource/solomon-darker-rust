//! Articulated Zombie banks and retained rotten gas/fly draws.

use super::layers::*;
use super::model::*;
use super::sample::*;
use crate::enemies::config::EnemyFamilyConfig;
use crate::enemies::model::{EnemyActorRead, EnemyBrain};
use crate::float;
use crate::{NativeRng, Vec2};

pub(super) fn zombie_presentation(
    actor: &impl EnemyActorRead,
    facing: usize,
    sample: &NativeEnemyPresentationInput,
    art: &(impl EnemyArtPoints + ?Sized),
) -> EnemyFamilyPresentation {
    let EnemyBrain::Zombie(brain) = actor.brain() else { unreachable!("Zombie brain") };
    let rear_pose = sample.zombie_rear_arm_pose.floor() as usize;
    let front_pose = sample.zombie_front_arm_pose.floor() as usize;
    let body_rotation = sample.zombie_body_rotation_radians;
    let socket_rotation = sample.zombie_arm_socket_rotation_radians;
    let front_rotation = sample.zombie_front_arm_rotation_radians;
    let rear_rotation = sample.zombie_rear_arm_rotation_radians;
    let big = brain.body_type == 3;
    let scale = if big { 1.149999976158142 } else { 1.0 };
    let heading = radians(actor.heading_deg);
    let forward =
        if big { Vec2::new(float::sin(heading), -float::cos(heading)) } else { Vec2::ZERO };
    let root = Vec2::new(0.0, if big { -8.0 } else { 0.0 });
    let body_entry = 2203 + brain.body_type as usize * 18 + facing;
    let root_point = |point: Vec2, rotation| {
        let rotated = rotate(point, rotation);
        Vec2::new(rotated.x + root.x, rotated.y + root.y)
    };
    let head = root_point(
        point(art, EnemyArtAtlas::BadGuys, 2293 + facing, usize::from(big)),
        body_rotation * 0.5,
    );
    let rear = root_point(point(art, EnemyArtAtlas::BadGuys, body_entry, 1), socket_rotation);
    let front = root_point(point(art, EnemyArtAtlas::BadGuys, body_entry, 2), socket_rotation);
    let torso = point(art, EnemyArtAtlas::BadGuys, 2203 + facing, 0);
    let mut body = vec![
        EnemySpriteLayer {
            offset: Vec2::new(
                forward.x * 4.0,
                forward.y * 4.0 - float::sin(radians(sample.stride_phase_deg * 0.5)).abs(),
            ),
            scale: if big { 2.0 } else { 1.0 },
            ..EnemySpriteLayer::new(
                EnemyArtAtlas::BadGuys,
                2365 + bounded_pose(sample.gait_pose, 7) * 18 + facing,
                "zombie-base",
            )
        },
        EnemySpriteLayer {
            offset: Vec2::new(
                root.x + torso.x - forward.x * 5.0,
                root.y + torso.y - forward.y * 5.0,
            ),
            rotation_radians: body_rotation,
            scale,
            ..EnemySpriteLayer::new(EnemyArtAtlas::BadGuys, body_entry, "zombie-body")
        },
        EnemySpriteLayer {
            offset: rear,
            rotation_radians: rear_rotation,
            scale,
            ..EnemySpriteLayer::new(
                EnemyArtAtlas::BadGuys,
                2095 + rear_pose * 18 + facing,
                "zombie-arm-rear",
            )
        },
        EnemySpriteLayer {
            offset: front,
            rotation_radians: front_rotation,
            scale,
            ..EnemySpriteLayer::new(
                EnemyArtAtlas::BadGuys,
                2149 + front_pose * 18 + facing,
                "zombie-arm-front",
            )
        },
    ];
    if big {
        for (part, socket, rotation) in
            [("rear", rear, rear_rotation), ("front", front, front_rotation)]
        {
            body.push(EnemySpriteLayer {
                offset: Vec2::new(socket.x - forward.x * 4.0, socket.y - forward.y * 4.0),
                rotation_radians: rotation,
                scale,
                ..EnemySpriteLayer::new(
                    EnemyArtAtlas::BadGuys,
                    2275 + facing,
                    format!("zombie-body-overlay-{part}"),
                )
            });
        }
    }
    body.push(EnemySpriteLayer {
        offset: head,
        rotation_radians: sample.zombie_head_rotation_radians,
        ..EnemySpriteLayer::new(
            EnemyArtAtlas::BadGuys,
            2293 + brain.head_type as usize * 18 + facing,
            "zombie-head",
        )
    });
    for layer in &mut body {
        layer.offset.x /= actor.config.scale;
        layer.offset.y /= actor.config.scale;
    }
    let mut family = EnemyFamilyPresentation::body(body);
    if matches!(actor.config.family, EnemyFamilyConfig::Zombie { rotten: true, .. }) {
        for mut layer in
            rotten_layers((sample.presentation_tick - actor.spawn_tick as f64).max(0.0))
        {
            layer.offset.y += root.y;
            family.after.push(layer.into_actor_space(actor.config.scale));
        }
    }
    family
}

fn rotten_layers(age: f64) -> Vec<EnemySpriteLayer> {
    let rotation = radians(age * 0.25);
    let tint = pack_rgb(0.05, 0.1, 0.05);
    let mut layers = vec![
        EnemySpriteLayer {
            alpha: 0.5,
            offset: Vec2::new(0.0, -15.0),
            scale: 1.5,
            tint,
            ..EnemySpriteLayer::new(EnemyArtAtlas::BadGuys, 65, "zombie-gas-cloud:puff")
        },
        EnemySpriteLayer {
            alpha: 0.5,
            offset: Vec2::new(0.0, -15.0),
            rotation_radians: rotation,
            scale_x: Some(1.5),
            scale_y: Some(1.2000000476837158),
            tint,
            ..EnemySpriteLayer::new(EnemyArtAtlas::BadGuys, 11, "zombie-gas-cloud:front")
        },
        EnemySpriteLayer {
            alpha: 0.5,
            offset: Vec2::new(0.0, -20.0),
            rotation_radians: rotation,
            blend: EnemyArtBlend::Add,
            scale_x: Some(-1.5),
            scale_y: Some(1.2000000476837158),
            tint,
            ..EnemySpriteLayer::new(EnemyArtAtlas::BadGuys, 11, "zombie-gas-cloud:mirrored")
        },
    ];
    let mut rng = NativeRng::new((age / 10.0).floor() as i64);
    let count = rng.draw_integer(16) + 5;
    for index in 0..count {
        let alpha = rng.draw_scaled_float(0.5, false);
        let radius = rng.draw_scaled_float(20.0, false);
        let doubled = rng.draw_integer(5) == 3;
        let angle = radians(rng.draw_scaled_float(360.0, false));
        let vertical = rng.draw_scaled_float(10.0, false);
        let radius = (radius + 10.0) * if doubled { 2.0 } else { 1.0 };
        layers.push(EnemySpriteLayer {
            alpha: alpha + 0.25,
            tint: 0,
            offset: Vec2::new(
                float::sin(angle) * radius,
                -float::cos(angle) * radius * 0.800000011920929 - vertical - 15.0,
            ),
            ..EnemySpriteLayer::new(EnemyArtAtlas::BadGuys, 26, format!("zombie-fly:{index}"))
        });
    }
    layers
}
