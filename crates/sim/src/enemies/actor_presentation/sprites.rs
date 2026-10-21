//! Direct bank families from `native-enemy-sprite-presentation.ts`.

use super::layers::*;
use super::model::*;
use super::sample::*;
use crate::Vec2;
use crate::enemies::EnemyToken;
use crate::enemies::model::EnemyActor;

pub(super) fn imp(
    actor: &EnemyActor,
    facing: usize,
    sample: &NativeEnemyPresentationInput,
) -> EnemyFamilyPresentation {
    let green = actor.config.native_type_id == 2044;
    let atlas = if green { EnemyArtAtlas::Unholy } else { EnemyArtAtlas::BadGuys };
    let frame = sample.imp_effect_frame.floor().clamp(0.0, 9.0) as usize;
    EnemyFamilyPresentation::body(vec![
        EnemySpriteLayer {
            blend: EnemyArtBlend::Add,
            rotation_radians: sample.imp_body_rotation_radians,
            ..EnemySpriteLayer::new(
                atlas,
                (if green { 41 } else { 285 }) + sample.body_pose as usize % 4 * 12 + facing,
                "imp-body",
            )
        },
        EnemySpriteLayer {
            blend: EnemyArtBlend::Add,
            alpha: sample.imp_effect_alpha,
            offset: Vec2::new(0.0, -10.0 / actor.config.scale),
            ..EnemySpriteLayer::new(
                atlas,
                (if green { 89 } else { 333 }) + frame,
                "imp-upper-effect",
            )
        },
    ])
}

pub(super) fn portal(
    _actor: &EnemyActor,
    sample: &NativeEnemyPresentationInput,
) -> EnemyFamilyPresentation {
    if sample.alpha <= 0.0 {
        return EnemyFamilyPresentation::default();
    }
    let height = 1.0 + sample.alpha * 2.0;
    let mut layers = vec![EnemySpriteLayer {
        blend: EnemyArtBlend::Add,
        offset: Vec2::new(0.0, 15.0 - 32.0 * height),
        scale: sample.stride_phase_deg.max(0.0),
        scale_x: Some(sample.stride_phase_deg.max(0.0)),
        scale_y: Some(height),
        ..EnemySpriteLayer::new(
            EnemyArtAtlas::DeadHawg,
            46 + sample.body_pose.floor() as usize % 32,
            "portal-body",
        )
    }];
    let hit = sample.hit_flash;
    if hit > 0.0 {
        layers.push(EnemySpriteLayer {
            blend: EnemyArtBlend::Add,
            scale: 2.0,
            ..EnemySpriteLayer::new(
                EnemyArtAtlas::BadGuys,
                401 + (((1.0 - hit.clamp(0.0, 1.0)) * 19.0).floor() as usize).min(18),
                "portal-hurt",
            )
        });
    }
    EnemyFamilyPresentation::body(layers)
}

pub(super) fn wraith(actor: &EnemyActor, facing: usize) -> EnemyFamilyPresentation {
    EnemyFamilyPresentation::body(vec![EnemySpriteLayer {
        blend: EnemyArtBlend::Add,
        offset: Vec2::new(0.0, 15.0 / actor.config.scale),
        scale: 2.0,
        ..EnemySpriteLayer::new(EnemyArtAtlas::BadGuys, 2070 + facing, "wraith-body")
    }])
}

pub(super) fn coffin(
    _actor: &EnemyActor,
    sample: &NativeEnemyPresentationInput,
) -> EnemyFamilyPresentation {
    let state = match sample.coffin_state {
        NativeEnemyCoffinState::Hidden => return EnemyFamilyPresentation::default(),
        NativeEnemyCoffinState::Closed => "closed",
        NativeEnemyCoffinState::Opening => "opening",
        NativeEnemyCoffinState::Open => "open",
    };
    EnemyFamilyPresentation::body(vec![EnemySpriteLayer {
        rotation_radians: sample.coffin_rotation_radians,
        scale_x: Some(sample.coffin_scale_x),
        ..EnemySpriteLayer::new(
            EnemyArtAtlas::BadGuys,
            175 + bounded_pose(sample.coffin_pose, 12),
            format!("coffin-{state}"),
        )
    }])
}

pub(super) fn spider(
    _actor: &EnemyActor,
    facing: usize,
    sample: &NativeEnemyPresentationInput,
) -> EnemyFamilyPresentation {
    let appearance = sample.spider.expect("Spider draw sample contains its outline appearance");
    let pose = sample.body_pose;
    assert!(
        pose.fract() == 0.0 && (0.0..=3.0).contains(&pose),
        "Spider leg pose is outside its four banks"
    );
    let legs = EnemySpriteLayer::new(
        EnemyArtAtlas::BadGuys,
        1840 + pose as usize * 18 + facing,
        "spider-legs",
    );
    let body = EnemySpriteLayer::new(
        EnemyArtAtlas::BadGuys,
        1912 + super::layers::facing(EnemyToken::Skeleton, appearance.body_heading_deg),
        "spider-body",
    );
    let mut layers = vec![legs.clone()];
    if appearance.outline_alpha > 0.0 {
        layers.push(EnemySpriteLayer {
            alpha: appearance.outline_alpha,
            blend: EnemyArtBlend::Add,
            tint: appearance.outline_tint,
            ..EnemySpriteLayer::new(
                EnemyArtAtlas::BadGuys,
                1930 + pose as usize * 18 + facing,
                "spider-outline",
            )
        });
    }
    layers.push(body.clone());
    EnemyFamilyPresentation { body: layers, hit_body: vec![legs, body], ..Default::default() }
}
