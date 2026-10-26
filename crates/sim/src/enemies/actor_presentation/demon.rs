//! Planted Demon limbs, connectors and flames from `native-enemy-demon-presentation.ts`.

use super::layers::*;
use super::model::*;
use super::sample::*;
use crate::Vec2;
use crate::enemies::model::{
    DemonExtremity, DemonPhase, EnemyActorRead, EnemyBrain, EnemyLifeState,
};
use crate::float;

#[derive(Clone, Copy)]
pub(super) struct DemonArticulationVisual {
    pub front: Vec2,
    pub rear: Vec2,
    pub front_rotation: f64,
    pub rear_rotation: f64,
    pub vertical_offset: f64,
    pub controller_pose: usize,
}

pub(super) fn articulation(actor: &impl EnemyActorRead, tick: u64) -> DemonArticulationVisual {
    let EnemyBrain::Demon(brain) = actor.brain() else { unreachable!("Demon brain") };
    let pose = if brain.phase == DemonPhase::Bomb {
        [0, 0, 0, 1, 1, 1, 1, 1, 0][bounded_pose(brain.action_progress, 8)]
    } else {
        0
    };
    let sample_tick = if actor.life_state == EnemyLifeState::Dying {
        actor.death_started_tick.unwrap_or(tick)
    } else {
        tick
    };
    let wave = 2.0 * float::sin(radians(sample_tick as f64));
    let local_offset = |extremity: &DemonExtremity| {
        Vec2::new(
            (extremity.current.x - actor.position.x) / actor.config.scale,
            (extremity.current.y + extremity.lift_y - actor.position.y) / actor.config.scale,
        )
    };
    DemonArticulationVisual {
        front: local_offset(&brain.articulation.front),
        rear: local_offset(&brain.articulation.rear),
        front_rotation: radians(if pose == 1 {
            40.0
        } else {
            wave + brain.articulation.front_base_rotation_deg
        }),
        rear_rotation: radians(if pose == 1 {
            -40.0
        } else {
            wave + brain.articulation.rear_base_rotation_deg
        }),
        vertical_offset: -float::sin(radians((sample_tick - actor.spawn_tick) as f64 * 0.25)).abs()
            * 3.0,
        controller_pose: pose,
    }
}

pub(super) fn presentation(
    actor: &impl EnemyActorRead,
    facing: usize,
    sample: &NativeEnemyPresentationInput,
    art: &(impl EnemyArtPoints + ?Sized),
) -> EnemyFamilyPresentation {
    let pose = sampled_articulation(sample);
    let origin = midpoint(pose.front, pose.rear);
    let controller_entry = 19 + pose.controller_pose * 18 + facing;
    let endpoint_entry = 62 + facing;
    let connector_entry = 98 + facing;
    let endpoint_point = point(art, EnemyArtAtlas::Demon, endpoint_entry, 0);
    let endpoints = |name: &str, endpoint: Vec2, slot| {
        let controller = point(art, EnemyArtAtlas::Demon, controller_entry, slot);
        [
            EnemySpriteLayer {
                apply_vertical_offset: false,
                offset: endpoint,
                scale: 0.8,
                ..EnemySpriteLayer::new(
                    EnemyArtAtlas::Demon,
                    endpoint_entry,
                    format!("demon-{name}-extremity"),
                )
            },
            EnemySpriteLayer {
                apply_vertical_offset: false,
                stretch: Some(EnemyArtStretch {
                    start: Vec2::new(
                        origin.x + controller.x * 1.2,
                        origin.y + controller.y * 1.2 - 5.0,
                    ),
                    end: Vec2::new(endpoint.x + endpoint_point.x, endpoint.y + endpoint_point.y),
                }),
                ..EnemySpriteLayer::new(
                    EnemyArtAtlas::Demon,
                    connector_entry,
                    format!("demon-{name}-connector"),
                )
            },
        ]
    };
    let rear = endpoints("rear", pose.rear, 6);
    let front = endpoints("front", pose.front, 7);
    let mut planted = Vec::with_capacity(4);
    if pose.rear.y <= pose.front.y {
        planted.extend(rear);
        planted.extend(front);
    } else {
        planted.extend(front);
        planted.extend(rear);
    }
    let controller = EnemySpriteLayer {
        offset: origin,
        scale: 1.2,
        ..EnemySpriteLayer::new(EnemyArtAtlas::Demon, controller_entry, "demon-controller-body")
    };
    let front_upper = EnemySpriteLayer {
        offset: add_scaled(origin, point(art, EnemyArtAtlas::Demon, controller_entry, 0), 1.5),
        rotation_radians: pose.front_rotation,
        scale: 1.2,
        ..EnemySpriteLayer::new(EnemyArtAtlas::Demon, 1 + facing, "demon-front-upper-limb")
    };
    let rear_upper = EnemySpriteLayer {
        offset: add_scaled(origin, point(art, EnemyArtAtlas::Demon, controller_entry, 1), 1.5),
        rotation_radians: pose.rear_rotation,
        scale: 1.2,
        scale_x: Some(-1.2),
        scale_y: Some(1.2),
        ..EnemySpriteLayer::new(
            EnemyArtAtlas::Demon,
            1 + (17 - facing) % 18,
            "demon-rear-upper-limb",
        )
    };
    let attached_point = point(art, EnemyArtAtlas::Demon, controller_entry, 5);
    let attached = EnemySpriteLayer {
        offset: add_scaled(origin, attached_point, 1.2),
        scale: 1.2,
        ..EnemySpriteLayer::new(EnemyArtAtlas::Demon, 80 + facing, "demon-attached-late")
    };
    let points = art.points(EnemyArtAtlas::Demon, controller_entry);
    let bases = [
        points[2],
        points[3],
        points[4],
        midpoint(points[0], points[1]),
        midpoint(points[1], points[2]),
    ];
    let mut behind = Vec::new();
    let mut in_front = Vec::new();
    let split_y = origin.y + attached_point.y * 1.5;
    for (index, base) in bases.into_iter().enumerate() {
        let magnitude = stable_unit(actor, 140 + index as u32 * 3, 0) * 4.0;
        let angle = stable_unit(actor, 141 + index as u32 * 3, 0) * std::f64::consts::PI * 2.0;
        let phase = stable_unit(actor, 142 + index as u32 * 3, 0) * 32.0;
        let entry = 46
            + ((phase + (sample.presentation_tick - actor.spawn_tick as f64).max(0.0) * 0.25)
                % 32.0)
                .floor() as usize;
        let flame = EnemySpriteLayer {
            blend: EnemyArtBlend::Add,
            offset: Vec2::new(
                origin.x + base.x * 1.5 + float::cos(angle) * magnitude,
                origin.y + base.y * 1.5 + float::sin(angle) * magnitude,
            ),
            scale: [0.5, 1.1, 0.5, 0.8, 0.8][index],
            ..EnemySpriteLayer::new(EnemyArtAtlas::DeadHawg, entry, format!("demon-flame:{index}"))
        };
        if flame.offset.y < split_y { behind.push(flame) } else { in_front.push(flame) }
    }
    let mut hit_body = planted.clone();
    hit_body.extend([
        controller.clone(),
        front_upper.clone(),
        rear_upper.clone(),
        attached.clone(),
    ]);
    let mut body = planted;
    body.extend([controller, front_upper, rear_upper]);
    body.extend(behind);
    body.push(attached);
    body.extend(in_front);
    EnemyFamilyPresentation { body, hit_body, ..Default::default() }
}

pub(super) fn death(
    actor: &impl EnemyActorRead,
    sample: &NativeEnemyPresentationInput,
) -> EnemyFamilyPresentation {
    let pose = sampled_articulation(sample);
    let offset = midpoint(pose.front, pose.rear);
    let facing = (((actor.heading_deg + 26.0) / 52.0).trunc() as i64).rem_euclid(7) as usize;
    let fade = (1.0 - sample.death_tick / 100.0).max(0.0);
    let mut layers = Vec::new();
    for (role, alpha) in [
        ("demon-death-body", 1.0),
        ("demon-death-additive", fade),
        ("demon-death-alpha-pass", fade),
    ] {
        layers.push(EnemySpriteLayer {
            alpha,
            apply_vertical_offset: false,
            blend: EnemyArtBlend::Add,
            offset,
            scale: 1.2,
            ..EnemySpriteLayer::new(EnemyArtAtlas::Demon, 55 + facing, role)
        });
    }
    EnemyFamilyPresentation::body(layers)
}

fn sampled_articulation(sample: &NativeEnemyPresentationInput) -> DemonArticulationVisual {
    DemonArticulationVisual {
        front: sample.demon_front_extremity_offset,
        rear: sample.demon_rear_extremity_offset,
        front_rotation: sample.demon_front_rotation_radians,
        rear_rotation: sample.demon_rear_rotation_radians,
        vertical_offset: sample.vertical_offset,
        controller_pose: sample.body_pose as usize,
    }
}
