//! Faculty, Heartmonger and Discorporeal body plans from their native presentation files.

use super::layers::*;
use super::model::*;
use super::sample::*;
use crate::enemies::EnemyToken;
use crate::enemies::config::EnemyFamilyConfig;
use crate::enemies::model::{EnemyActorRead, EnemyBrain, EnemyLifeState};
use crate::enemies::presentation::native_death_magic_layers;
use crate::float;
use crate::float::fround;
use crate::{NativeRng, Vec2};

pub(super) fn faculty(
    actor: &impl EnemyActorRead,
    facing: usize,
    tick: u64,
    sample: &NativeEnemyPresentationInput,
    art: &(impl EnemyArtPoints + ?Sized),
) -> EnemyFamilyPresentation {
    let EnemyFamilyConfig::Faculty(appearance) = &actor.config.family else {
        unreachable!("Faculty appearance")
    };
    let EnemyBrain::Faculty(brain) = actor.brain() else { unreachable!("Faculty brain") };
    let pose = sample.body_pose.trunc() as usize;
    let robe_pose = if pose > 0 { 4 + pose } else { sample.gait_pose.trunc() as usize };
    let body_entry = 1 + pose * 18 + facing;
    let bob = float::sin(radians(brain.light_phase * 2.0)) * 8.0 - 15.0;
    let jitter = float::sin(radians(brain.light_phase * 16.0)) * 0.5;
    let heading = radians(actor.heading_deg);
    let robe_offset = Vec2::new(float::sin(heading) * jitter, bob - float::cos(heading) * jitter);
    let body_tint =
        pack_rgb(appearance.body_color[0], appearance.body_color[1], appearance.body_color[2]);
    let head_tint =
        pack_rgb(appearance.head_color[0], appearance.head_color[1], appearance.head_color[2]);
    let scale = 1.0499999523162842;
    let mut layers = vec![
        EnemySpriteLayer {
            offset: Vec2::new(0.0, bob),
            scale,
            ..EnemySpriteLayer::new(EnemyArtAtlas::Faculty, body_entry, "faculty-body")
        },
        EnemySpriteLayer {
            alpha: appearance.body_color[3],
            offset: robe_offset,
            scale,
            tint: body_tint,
            ..EnemySpriteLayer::new(
                EnemyArtAtlas::Faculty,
                199 + robe_pose * 18 + facing,
                "faculty-robe",
            )
        },
        EnemySpriteLayer {
            alpha: appearance.head_color[3],
            offset: robe_offset,
            scale,
            tint: head_tint,
            ..EnemySpriteLayer::new(
                EnemyArtAtlas::Faculty,
                361 + robe_pose * 18 + facing,
                "faculty-robe-trim",
            )
        },
    ];
    let mut rng = NativeRng::new(actor.id as i64 + tick as i64);
    for hand in 0..2 {
        if brain.hand_mask & (1 << hand) == 0 {
            continue;
        }
        let anchor = point(art, EnemyArtAtlas::Faculty, body_entry, hand);
        let aura = native_death_magic_layers(
            Vec2::new(anchor.x, anchor.y + bob),
            1.5,
            sample.presentation_tick,
            &mut rng,
        );
        layers.extend(aura.into_iter().map(EnemySpriteLayer::from));
    }
    let head_offset = Vec2::new(0.0, bob + if appearance.headgear == 0 { -5.0 } else { 0.0 });
    if appearance.headgear == 0 {
        layers.push(EnemySpriteLayer {
            alpha: appearance.body_color[3],
            offset: head_offset,
            scale,
            tint: body_tint,
            ..EnemySpriteLayer::new(EnemyArtAtlas::Faculty, 91 + facing, "faculty-head-robe")
        });
        layers.push(EnemySpriteLayer {
            alpha: appearance.head_color[3],
            offset: head_offset,
            scale,
            tint: head_tint,
            ..EnemySpriteLayer::new(EnemyArtAtlas::Faculty, 109 + facing, "faculty-head")
        });
        if appearance.female {
            layers.push(EnemySpriteLayer {
                offset: head_offset,
                scale,
                ..EnemySpriteLayer::new(EnemyArtAtlas::Faculty, 127 + facing, "faculty-hair")
            });
        }
    } else {
        layers.push(EnemySpriteLayer {
            offset: head_offset,
            scale,
            ..EnemySpriteLayer::new(EnemyArtAtlas::Faculty, 145 + facing, "faculty-hat")
        });
        layers.push(EnemySpriteLayer {
            alpha: appearance.body_color[3],
            offset: head_offset,
            scale,
            tint: body_tint,
            ..EnemySpriteLayer::new(EnemyArtAtlas::Faculty, 163 + facing, "faculty-hat-robe")
        });
        layers.push(EnemySpriteLayer {
            alpha: appearance.head_color[3],
            offset: head_offset,
            scale,
            tint: head_tint,
            ..EnemySpriteLayer::new(EnemyArtAtlas::Faculty, 181 + facing, "faculty-hat-trim")
        });
    }
    if actor.life_state == EnemyLifeState::Dying {
        let distance = rng.draw_scaled_float(sample.death_tick / 25.0, false);
        let angle = radians(rng.draw_scaled_float(360.0, false));
        for layer in &mut layers {
            layer.offset.x += float::sin(angle) * distance;
            layer.offset.y -= float::cos(angle) * distance;
        }
    }
    EnemyFamilyPresentation::body(layers)
}

pub(super) fn heartmonger(
    actor: &impl EnemyActorRead,
    facing: usize,
    sample: &NativeEnemyPresentationInput,
    art: &(impl EnemyArtPoints + ?Sized),
) -> EnemyFamilyPresentation {
    let EnemyBrain::Heartmonger(brain) = actor.brain() else { unreachable!("Heartmonger brain") };
    let legs = 200 + sample.gait_pose.trunc() as usize * 18 + facing;
    let torso = 146 + sample.body_pose.trunc() as usize * 18 + facing;
    let head = 110 + brain.head_variant as usize * 18 + facing;
    let torso_point = point(art, EnemyArtAtlas::Heartmonger, legs, 2);
    let head_point = point(art, EnemyArtAtlas::Heartmonger, torso, 0);
    EnemyFamilyPresentation::body(vec![
        EnemySpriteLayer {
            scale: 1.0499999523162842,
            ..EnemySpriteLayer::new(EnemyArtAtlas::Heartmonger, legs, "heartmonger-legs")
        },
        EnemySpriteLayer {
            offset: Vec2::new(
                torso_point.x / actor.config.scale,
                torso_point.y / actor.config.scale,
            ),
            scale: 1.0499999523162842,
            ..EnemySpriteLayer::new(EnemyArtAtlas::Heartmonger, torso, "heartmonger-torso")
        },
        EnemySpriteLayer {
            offset: Vec2::new(
                (torso_point.x + head_point.x) / actor.config.scale,
                (torso_point.y + head_point.y) / actor.config.scale,
            ),
            scale: 1.3300000429153442 * 1.0499999523162842,
            ..EnemySpriteLayer::new(EnemyArtAtlas::Heartmonger, head, "heartmonger-head")
        },
    ])
}

pub(super) fn demon_skull(
    actor: &impl EnemyActorRead,
    tick: u64,
    art: &(impl EnemyArtPoints + ?Sized),
) -> EnemyFamilyPresentation {
    let EnemyBrain::DemonSkull(brain) = actor.brain() else { unreachable!("DemonSkull brain") };
    let mut rng = NativeRng::new(actor.id as i64 + tick as i64);
    let bob = fround(float::sin(radians(brain.body_phase_deg)) * 10.0 - 25.0);
    let scale = actor.config.scale;
    let offset = Vec2::new(
        (brain.body_offset.x + brain.jitter.x) / scale,
        (brain.body_offset.y + brain.jitter.y + bob) / scale,
    );
    let pose = brain.body_pose as usize;
    let facing = super::layers::facing(EnemyToken::DemonSkull, brain.body_heading_deg);
    let overlay = 171 + pose * 24 + facing;
    let body_tint = pack_rgb(0.6700000166893005, 1.0, 0.05000000074505806);
    let mut layers = Vec::new();
    let mut front_eyes = Vec::new();
    if brain.eye_charge > 0.0 {
        for index in 0..2 {
            let socket = point(art, EnemyArtAtlas::Unholy, 99 + pose * 24 + facing, index);
            let eye = Vec2::new(
                fround(brain.body_offset.x + socket.x * 2.0 * scale),
                fround(brain.body_offset.y + bob + socket.y * 2.0 * scale),
            );
            let behind = eye.y + 45.0 < brain.body_offset.y + bob;
            let glow = EnemySpriteLayer {
                alpha: brain.eye_charge,
                blend: if behind { EnemyArtBlend::Normal } else { EnemyArtBlend::Add },
                scale: 4.0 / scale,
                offset: Vec2::new(eye.x / scale + offset.x, eye.y / scale + offset.y),
                ..EnemySpriteLayer::new(
                    EnemyArtAtlas::Unholy,
                    2,
                    format!("discorporeal-eye-charge-{index}"),
                )
            };
            if behind { layers.push(glow) } else { front_eyes.push(glow) }
        }
    }
    let dying = actor.life_state == EnemyLifeState::Dying;
    if dying || pose >= 2 || brain.flair_glow > 0.0 {
        layers.push(EnemySpriteLayer {
            alpha: if dying || pose >= 2 { 1.0 } else { brain.flair_glow },
            offset,
            scale: (7.0 + rng.draw_scaled_float(2.0, false)) / scale,
            tint: 0x8cff0c,
            ..EnemySpriteLayer::new(EnemyArtAtlas::Unholy, 2, "discorporeal-flair")
        });
        if pose == 2 {
            let alpha = 0.5 + rng.draw_scaled_float(0.5, false);
            let size = (7.0 + rng.draw_scaled_float(2.0, false)) * 1.5 / scale;
            layers.push(EnemySpriteLayer {
                alpha,
                offset,
                scale: size,
                tint: 0x8cff0c,
                ..EnemySpriteLayer::new(EnemyArtAtlas::Unholy, 2, "discorporeal-death-flair")
            });
        }
    }
    let rotation = radians(brain.spin);
    layers.push(EnemySpriteLayer {
        offset,
        rotation_radians: rotation,
        scale: 2.0,
        ..EnemySpriteLayer::new(EnemyArtAtlas::Unholy, 99 + pose * 24 + facing, "discorporeal-body")
    });
    if pose < 2 {
        let alpha = 0.75 + float::sin(radians(brain.flicker_phase_deg)) * 0.25;
        for (role, size) in [
            ("discorporeal-body-glow", 3.799999952316284),
            ("discorporeal-body-glow-outer", 4.0 / scale),
        ] {
            layers.push(EnemySpriteLayer {
                alpha,
                blend: EnemyArtBlend::Add,
                offset,
                rotation_radians: rotation,
                scale: size,
                tint: body_tint,
                ..EnemySpriteLayer::new(EnemyArtAtlas::Unholy, overlay, role)
            });
        }
    } else {
        let alpha = 0.5 + rng.draw_scaled_float(0.5, false);
        let size = (5.0 + rng.draw_scaled_float(0.25, true)) / scale;
        layers.push(EnemySpriteLayer {
            alpha,
            blend: EnemyArtBlend::Add,
            offset: Vec2::new(offset.x, offset.y - 35.0 / scale),
            scale: size,
            tint: 0x8cff0c,
            ..EnemySpriteLayer::new(EnemyArtAtlas::Unholy, 2, "discorporeal-death-core")
        });
    }
    if pose < 2 && brain.charge_glow > 0.0 {
        let size = (4.25 + rng.draw_scaled_float(0.1, true)) / scale;
        layers.push(EnemySpriteLayer {
            alpha: brain.charge_glow,
            blend: EnemyArtBlend::Add,
            offset,
            rotation_radians: rotation,
            scale: size,
            tint: body_tint,
            ..EnemySpriteLayer::new(EnemyArtAtlas::Unholy, overlay, "discorporeal-charge-glow")
        });
        layers.push(EnemySpriteLayer {
            alpha: brain.charge_glow * 0.75,
            blend: EnemyArtBlend::Add,
            offset,
            rotation_radians: rotation,
            scale: (4.0 + brain.charge_glow) / scale,
            tint: body_tint,
            ..EnemySpriteLayer::new(EnemyArtAtlas::Unholy, overlay, "discorporeal-charge-outer")
        });
    }
    layers.extend(front_eyes);
    EnemyFamilyPresentation { body: layers, ..Default::default() }
}
