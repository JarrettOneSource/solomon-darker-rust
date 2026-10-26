//! Boss sprite stacks and Ultra Banish gradients from `native-boss-spell-presentation.ts`
//! and `native-ultra-banish-presentation.ts`. Mesh-only spells retain their own view paths.

use super::actor_presentation::{EnemyArtAtlas, EnemyArtBlend, EnemySpriteLayer};
use super::boss_spell::{BossSpellKind, BossSpellRead};
use super::death_presentation::EnemyBanishGradient;
use super::presentation::native_death_magic_layers;
use crate::float;
use crate::{NativeRng, Vec2};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct EnemyBossSpellPlan {
    pub gradients: Vec<EnemyBanishGradient>,
    pub layers: Vec<EnemySpriteLayer>,
}

#[must_use]
pub fn native_boss_spell_plan(
    spell: &impl BossSpellRead,
    tick: u64,
    view_height: f64,
) -> EnemyBossSpellPlan {
    let spell_view = spell.view();
    let mut rng = NativeRng::new((spell_view.owner.id as u64 + tick) as i64);
    let layers = match spell_view.kind {
        BossSpellKind::UltraBanish { alpha, flash_alpha, .. } => {
            return ultra_banish_plan(*alpha, *flash_alpha, view_height, &mut rng);
        }
        BossSpellKind::UnholySoul { angle_deg, scale, height, life, .. } => {
            vec![EnemySpriteLayer {
                alpha: life.min(1.0),
                blend: EnemyArtBlend::Add,
                offset: Vec2::new(0.0, *height),
                rotation_radians: radians(angle_deg - 90.0),
                scale: *scale,
                ..layer(EnemyArtAtlas::Unholy, 3, "unholy-soul")
            }]
        }
        BossSpellKind::MouthBeamSegment { .. } => mouth_beam_spark(&mut rng),
        BossSpellKind::UnholyBurst { frame_phase, offset_y, .. } => {
            let frame = frame_phase.round_ties_even();
            if frame >= 14.0 {
                Vec::new()
            } else {
                vec![EnemySpriteLayer {
                    blend: EnemyArtBlend::Add,
                    offset: Vec2::new(0.0, *offset_y),
                    scale: 3.0,
                    tint: 0x8cff0c,
                    ..layer(EnemyArtAtlas::BadGuys, 420 + frame as usize, "unholy-impact-rise")
                }]
            }
        }
        BossSpellKind::GreenFire { fire, .. } => {
            let phase = fire.atlas_phase.round_ties_even();
            let scale = fire.fade_alpha * fire.scale * f64::from(1.1_f32);
            if phase >= 32.0 {
                Vec::new()
            } else {
                vec![EnemySpriteLayer {
                    alpha: (fire.draw_alpha * fire.life).min(1.0),
                    blend: EnemyArtBlend::Add,
                    offset: Vec2::new(0.0, -10.0),
                    scale,
                    scale_x: Some(scale * fire.horizontal_sign),
                    tint: 0x8cff0c,
                    ..layer(EnemyArtAtlas::Unholy, 9 + phase as usize, "green-fire")
                }]
            }
        }
        BossSpellKind::EyeLaser { phase_deg, .. } => eye_laser(*phase_deg, &mut rng),
        BossSpellKind::UnholySpit { progress, height, .. } => {
            unholy_spit(*progress, *height, &mut rng)
        }
        BossSpellKind::HeartmongerFlicker { phase_deg } => {
            let jitter = rng.draw_scaled_float(0.30000007152557373, false);
            let scale = float::sin(radians(*phase_deg)) * 5.0 * (f64::from(0.9_f32) + jitter);
            vec![EnemySpriteLayer {
                scale,
                scale_y: Some(scale * f64::from(0.8_f32)),
                ..layer(EnemyArtAtlas::BadGuys, 15, "heartmonger-flicker")
            }]
        }
        BossSpellKind::HeartmongerSoul { life_phase_deg, bob_phase_deg } => {
            let alpha = float::sin(radians(*life_phase_deg));
            let y = float::sin(radians(*bob_phase_deg)) * 5.0 - life_phase_deg * 0.5 - 50.0;
            let mut layers = Vec::new();
            for index in 0..2 {
                layers.push(EnemySpriteLayer {
                    alpha,
                    blend: EnemyArtBlend::Add,
                    offset: Vec2::new(0.0, y),
                    scale: f64::from(1.6_f32),
                    scale_x: Some((alpha * 0.25 + 0.75) * f64::from(1.6_f32)),
                    ..layer(EnemyArtAtlas::Heartmonger, 0, format!("heartmonger-soul-{index}"))
                });
            }
            layers
        }
        BossSpellKind::Blightning { .. } | BossSpellKind::TragicCircle { .. } => Vec::new(),
        BossSpellKind::DeathMagic { scale, alpha, .. } => {
            let mut layers = death_magic(*scale, tick as f64, &mut rng);
            for layer in &mut layers {
                layer.alpha = (layer.alpha * alpha).min(1.0);
            }
            layers
        }
        BossSpellKind::SkullMissile { phase_deg, remaining_ticks, .. } => {
            let mut layers = death_magic(1.25, *phase_deg, &mut rng);
            for layer in &mut layers {
                layer.alpha *= (remaining_ticks / 100.0).min(1.0);
            }
            layers
        }
        BossSpellKind::DarkFireball {
            arc_phase,
            arc_phase_step,
            heading_deg,
            remaining_ticks,
            ..
        } => {
            let height = -15.0
                - if *arc_phase_step > 0.0 { float::sin(radians(*arc_phase)) * 50.0 } else { 0.0 };
            let size = rng.draw_scaled_float(0.25, true);
            let alpha = (remaining_ticks / 100.0).min(1.0);
            vec![
                EnemySpriteLayer {
                    alpha: alpha * 0.5,
                    offset: Vec2::new(0.0, height),
                    scale: 2.0,
                    tint: 0xff0d00,
                    ..layer(EnemyArtAtlas::BadGuys, 15, "direball-glow")
                },
                EnemySpriteLayer {
                    alpha,
                    offset: Vec2::new(0.0, height),
                    rotation_radians: radians(heading_deg + 180.0),
                    scale: 1.25 + size,
                    tint: 0xff8000,
                    ..layer(
                        EnemyArtAtlas::BadGuys,
                        255 + (spell_view.owner.age_ticks % 12) as usize,
                        "direball-flame",
                    )
                },
            ]
        }
        BossSpellKind::RainOfBones { alpha, phase, rotation_sign, scale, .. } => {
            let tint = (((float::sin(radians(*phase)) * f64::from(0.1_f32) + f64::from(0.2_f32))
                * 255.0)
                .round() as u32)
                << 16;
            vec![
                EnemySpriteLayer {
                    alpha: alpha * f64::from(0.4_f32),
                    offset: Vec2::new(0.0, -175.0),
                    rotation_radians: radians(phase * 0.03125 * rotation_sign),
                    scale: scale * 5.0,
                    scale_y: Some(scale * 4.0),
                    tint,
                    ..layer(EnemyArtAtlas::BadGuys, 78, "acid-pain-cloud")
                },
                EnemySpriteLayer {
                    alpha: alpha * 0.75,
                    offset: Vec2::new(0.0, -175.0),
                    rotation_radians: radians(phase * -0.5),
                    scale: scale * 6.0,
                    scale_y: Some(scale * 6.0 * f64::from(0.8_f32)),
                    tint: 0x732626,
                    ..layer(EnemyArtAtlas::BadGuys, 1, "acid-pain-cloud-glow")
                },
            ]
        }
        BossSpellKind::FallingBone { entry, height, color_ramp, rotation_deg } => {
            let channel = (color_ramp * 255.0).round() as u32;
            vec![EnemySpriteLayer {
                offset: Vec2::new(0.0, *height),
                rotation_radians: radians(*rotation_deg),
                scale: f64::from(1.2_f32),
                tint: channel * 0x010101,
                ..layer(EnemyArtAtlas::BadGuys, *entry, "acid-pain-falling-bone")
            }]
        }
        BossSpellKind::DireFire(fire) => {
            let alpha = fire.alpha.min(1.0);
            let mut layers = Vec::new();
            if fire.glow {
                layers.push(EnemySpriteLayer {
                    alpha,
                    rotation_radians: radians(fire.rotation_signs[0] * fire.rotation_phases[0]),
                    scale: fire.scale * 1.5,
                    scale_y: Some(fire.scale * 1.5 * f64::from(0.8_f32)),
                    tint: 0,
                    ..layer(EnemyArtAtlas::BadGuys, 10, "dire-fire-ground")
                });
                layers.push(EnemySpriteLayer {
                    alpha,
                    rotation_radians: radians(-fire.rotation_signs[1] * fire.rotation_phases[1]),
                    scale: fire.scale * 2.0,
                    scale_y: Some(fire.scale * 2.0 * f64::from(0.8_f32)),
                    tint: 0x1a0000,
                    ..layer(EnemyArtAtlas::BadGuys, 11, "dire-fire-red-ground")
                });
            }
            let scale = fire.ramp * fire.scale * f64::from(1.1_f32);
            layers.push(EnemySpriteLayer {
                alpha,
                blend: EnemyArtBlend::Add,
                offset: Vec2::new(0.0, -10.0),
                scale,
                scale_x: Some(scale * fire.scale_sign),
                tint: 0x400000,
                ..layer(
                    EnemyArtAtlas::DeadHawg,
                    46 + fire.phase.trunc() as usize % 32,
                    "dire-fire-flame",
                )
            });
            layers
        }
    };
    EnemyBossSpellPlan { gradients: Vec::new(), layers }
}

fn layer(atlas: EnemyArtAtlas, entry: usize, role: impl Into<String>) -> EnemySpriteLayer {
    EnemySpriteLayer::new(atlas, entry, role)
}

fn radians(degrees: f64) -> f64 {
    degrees * std::f64::consts::PI / 180.0
}

fn death_magic(scale: f64, phase: f64, rng: &mut NativeRng) -> Vec<EnemySpriteLayer> {
    native_death_magic_layers(Vec2::ZERO, scale, phase, rng)
        .into_iter()
        .map(EnemySpriteLayer::from)
        .collect()
}

fn mouth_beam_spark(rng: &mut NativeRng) -> Vec<EnemySpriteLayer> {
    if rng.draw_integer(8) != 1 {
        return Vec::new();
    }
    let size = rng.draw_scaled_float(f64::from(0.22_f32), false);
    let radius = rng.draw_scaled_float(50.0, false);
    let angle = radians(rng.draw_scaled_float(360.0, false));
    vec![EnemySpriteLayer {
        tint: 0x8cff0c,
        offset: Vec2::new(float::sin(angle) * radius, -float::cos(angle) * radius),
        scale: (0.25 - size) * 2.0,
        ..layer(EnemyArtAtlas::Unholy, 0, "mouth-beam-spark")
    }]
}

fn eye_laser(phase_deg: f64, rng: &mut NativeRng) -> Vec<EnemySpriteLayer> {
    rng.draw_scaled_float(0.25, true);
    let size = rng.draw_scaled_float(f64::from(0.15_f32), true);
    let glow_size = rng.draw_scaled_float(f64::from(0.15_f32), true);
    vec![
        EnemySpriteLayer {
            blend: EnemyArtBlend::Add,
            rotation_radians: radians(phase_deg),
            scale: (f64::from(0.35_f32) + size) * 2.0,
            tint: 0x8cff0c,
            ..layer(EnemyArtAtlas::Unholy, 1, "eye-laser")
        },
        EnemySpriteLayer {
            alpha: f64::from(0.65_f32),
            blend: EnemyArtBlend::Add,
            scale: (0.5 + glow_size) * 2.0,
            tint: 0x8cff0c,
            ..layer(EnemyArtAtlas::Unholy, 2, "eye-laser-glow")
        },
    ]
}

fn unholy_spit(progress: f64, height: f64, rng: &mut NativeRng) -> Vec<EnemySpriteLayer> {
    let offset = Vec2::new(0.0, -float::sin(progress * std::f64::consts::PI) * height);
    let mut layers = Vec::new();
    for (index, scale) in [3.0, 3.0, 2.0].into_iter().enumerate() {
        let entry = 5 + rng.draw_integer(4) as usize;
        layers.push(EnemySpriteLayer {
            blend: if index == 0 { EnemyArtBlend::Normal } else { EnemyArtBlend::Add },
            offset,
            scale,
            ..layer(EnemyArtAtlas::Unholy, entry, format!("unholy-spit-{index}"))
        });
    }
    layers.push(EnemySpriteLayer {
        blend: EnemyArtBlend::Add,
        offset,
        scale: (1.0 + rng.draw_scaled_float(1.0, false)) * 2.0,
        tint: 0x8cff0c,
        ..layer(EnemyArtAtlas::Unholy, 2, "unholy-spit-glow")
    });
    layers
}

fn ultra_banish_plan(
    alpha: f64,
    flash_alpha: f64,
    view_height: f64,
    rng: &mut NativeRng,
) -> EnemyBossSpellPlan {
    let progress = alpha - rng.draw_scaled_float(f64::from(0.2_f32), false);
    let scale = 5.0 + rng.draw_scaled_float(4.0, false);
    let offset_y = -50.0 * scale;
    let upper = view_height * 0.5 * scale;
    let lower = 50.0 * scale;
    let mut values = vec![progress];
    if flash_alpha > 0.0 {
        values.push(flash_alpha);
    }
    let mut gradients = Vec::new();
    for value in values {
        for (half_width, extent, color, alpha) in [
            (10.0 * scale, upper, 0x80ff00, (value * 0.5).min(1.0)),
            (5.0 * value * scale, upper, 0xbfff00, (value * 0.5).min(1.0)),
            (2.0 * value * scale, upper * 0.75, 0xffffff, (value * 0.75).min(1.0)),
        ] {
            if half_width <= 0.0 {
                continue;
            }
            gradients.push(EnemyBanishGradient {
                position: Vec2::new(-half_width, offset_y - extent),
                width: half_width * 2.0,
                height: extent,
                start_color: 0,
                end_color: color,
                start_alpha: 1.0,
                end_alpha: alpha,
            });
            gradients.push(EnemyBanishGradient {
                position: Vec2::new(-half_width, offset_y),
                width: half_width * 2.0,
                height: lower,
                start_color: color,
                end_color: 0,
                start_alpha: alpha,
                end_alpha: 1.0,
            });
        }
    }
    let core_scale = (5.0 + rng.draw_scaled_float(1.0, false)) * progress * 2.0;
    let tint = (((progress * 0.75).clamp(0.0, 1.0) * 255.0).round() as u32) << 16 | 0x00ff00;
    let mut layers = Vec::new();
    for index in 0..2 {
        layers.push(EnemySpriteLayer {
            alpha: progress.clamp(0.0, 1.0),
            blend: EnemyArtBlend::Add,
            scale: core_scale.max(0.0),
            tint,
            ..layer(EnemyArtAtlas::BadGuys, 15, format!("ultra-banish-core-{index}"))
        });
    }
    EnemyBossSpellPlan { gradients, layers }
}
