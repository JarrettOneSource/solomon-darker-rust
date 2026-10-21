//! Shared compositors from `renderer/native-enemy-projectile-presentation.ts`
//! and `renderer/native-death-magic-presentation.ts`. Secondary Dampen reuses
//! these stock layers; `tests/secondary_presentation_goldens.rs` replays both.

use crate::float;
use crate::float::fround;
use crate::spells::presentation::{SpellAtlas, SpellBlend};
use crate::{NativeRng, Vec2};

/// Fractional display fields shared by projectile and terminal snapshot effects.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeEnemyEffectPresentationInput {
    pub age_ticks: f64,
    pub rotation_radians: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeEnemyProjectileKind {
    Arrow,
    Firebolt,
    GuidedMissile,
    DemonBomb,
    PoisonPool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeEnemyProjectilePayload {
    Normal,
    Fire,
    Poison,
    Cold,
    None,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeEnemyProjectileVisual {
    pub age_ticks: u64,
    pub heading_degrees: f64,
    pub id: u64,
    pub kind: NativeEnemyProjectileKind,
    pub lifetime_ticks: u64,
    pub payload: NativeEnemyProjectilePayload,
    pub position: Vec2,
    pub speed: f64,
    pub vertical_offset: f64,
    pub visual_phase_degrees: f64,
    pub visual_scale: f64,
}
#[derive(Clone, Debug, PartialEq)]
pub struct NativeEnemyProjectileLayer {
    pub alpha: f64,
    pub atlas: SpellAtlas,
    pub blend: SpellBlend,
    pub entry: usize,
    pub offset: Vec2,
    pub role: String,
    pub rotation_radians: f64,
    pub scale: f64,
    /// General DeathMagic layers omit this override; projectile layers supply it.
    pub scale_y: Option<f64>,
    pub tint: u32,
}
impl NativeEnemyProjectileLayer {
    fn new(atlas: SpellAtlas, entry: usize, role: &str) -> Self {
        Self {
            alpha: 1.0,
            atlas,
            blend: SpellBlend::Normal,
            entry,
            offset: Vec2::ZERO,
            role: role.into(),
            rotation_radians: 0.0,
            scale: 1.0,
            scale_y: None,
            tint: 0xffffff,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeEnemyProjectileStreak {
    pub alpha: f64,
    pub end: Vec2,
    pub start: Vec2,
    pub width: f64,
}
#[derive(Clone, Debug, PartialEq)]
pub struct NativeEnemyProjectilePlan {
    pub layers: Vec<NativeEnemyProjectileLayer>,
    pub underlays: Vec<NativeEnemyProjectileLayer>,
    pub position: Vec2,
    pub streak: Option<NativeEnemyProjectileStreak>,
}

pub fn native_enemy_projectile_plan(
    projectile: &NativeEnemyProjectileVisual,
    tick: f64,
) -> NativeEnemyProjectilePlan {
    native_enemy_projectile_plan_at_age(projectile, tick, projectile.age_ticks as f64)
}

#[must_use]
pub fn native_enemy_projectile_plan_at_age(
    projectile: &NativeEnemyProjectileVisual,
    tick: f64,
    age_ticks: f64,
) -> NativeEnemyProjectilePlan {
    assert!(
        tick.is_finite() && tick >= 0.0,
        "enemy projectile presentation tick is finite and non-negative"
    );
    let tick = tick.floor() as u64;
    let age = age_ticks.max(0.0).floor() as u64;
    let heading = degrees(projectile.heading_degrees);
    let mut layers = Vec::new();
    let mut underlays = Vec::new();
    let mut streak = None;
    use NativeEnemyProjectileKind as Kind;
    use NativeEnemyProjectilePayload as Payload;
    match projectile.kind {
        Kind::Arrow => {
            let payload = match projectile.payload {
                Payload::Normal => "normal",
                Payload::Fire => "fire",
                Payload::Poison => "poison",
                _ => panic!("unsupported Arrow payload"),
            };
            let rotation = degrees(projectile.visual_phase_degrees);
            let alpha = projectile.visual_scale.min(1.0);
            let mut body = NativeEnemyProjectileLayer::new(
                SpellAtlas::BadGuys,
                2,
                &format!("arrow-{payload}"),
            );
            body.alpha = alpha;
            body.offset = Vec2::new(0.0, projectile.vertical_offset);
            body.rotation_radians = rotation;
            body.scale = 1.25;
            layers.push(body);
            if projectile.vertical_offset <= -20.0 {
                let x = float::sin(heading) * projectile.speed;
                let y = -float::cos(heading) * projectile.speed;
                let age = age_ticks.min(35.0);
                streak = Some(NativeEnemyProjectileStreak {
                    alpha: 0.5
                        * (fround(0.8) + unit(projectile.id, tick, 2) * fround(1.0 - fround(0.8))),
                    start: Vec2::new(
                        fround(-5.0 * x),
                        fround(projectile.vertical_offset - 5.0 * y),
                    ),
                    end: Vec2::new(fround(-age * x), fround(projectile.vertical_offset - age * y)),
                    width: 2.0,
                });
            }
            if projectile.payload != Payload::Normal {
                let height = if projectile.vertical_offset == 0.0 { 0.35 } else { 1.0 };
                let scale = 0.5 + unit(projectile.id, tick, 0) * 0.5 * height;
                let mut overlay = NativeEnemyProjectileLayer::new(
                    SpellAtlas::BadGuys,
                    if projectile.payload == Payload::Fire {
                        255 + (tick / 5 % 12) as usize
                    } else {
                        271 + (age / 6 % 12) as usize
                    },
                    if projectile.payload == Payload::Fire {
                        "arrow-fire-overlay"
                    } else {
                        "arrow-poison-overlay"
                    },
                );
                overlay.alpha = if projectile.payload == Payload::Fire {
                    fround(fround(alpha) * fround(alpha))
                } else {
                    alpha
                };
                overlay.blend = SpellBlend::Add;
                overlay.offset = Vec2::new(0.0, projectile.vertical_offset);
                overlay.scale = scale;
                if projectile.payload == Payload::Fire {
                    overlay.rotation_radians = rotation + std::f64::consts::PI;
                } else {
                    overlay.tint = 0x008000;
                }
                layers.push(overlay);
            }
            let mut shadow = NativeEnemyProjectileLayer::new(
                SpellAtlas::BadGuys,
                if projectile.vertical_offset == 0.0 { 3 } else { 2 },
                "arrow-ground-shadow",
            );
            shadow.alpha = alpha;
            shadow.rotation_radians = heading;
            shadow.tint = 0;
            underlays.push(shadow);
        }
        Kind::Firebolt => {
            assert_eq!(projectile.payload, Payload::Fire, "Firebolt payload is Fire");
            let alpha = lifetime_alpha(projectile, age);
            let mut glow =
                NativeEnemyProjectileLayer::new(SpellAtlas::BadGuys, 15, "firebolt-orange-glow");
            glow.alpha = alpha * 0.5;
            glow.blend = SpellBlend::Add;
            glow.offset = Vec2::new(0.0, -15.0);
            glow.scale = 2.0;
            glow.tint = 0xff8000;
            let mut body = NativeEnemyProjectileLayer::new(
                SpellAtlas::BadGuys,
                255 + (age % 12) as usize,
                "firebolt-body",
            );
            body.alpha = alpha;
            body.blend = SpellBlend::Add;
            body.offset = Vec2::new(0.0, -15.0);
            body.rotation_radians = heading + std::f64::consts::PI;
            body.scale = 1.0 + unit(projectile.id, tick, 1) * 0.5;
            layers.extend([glow, body]);
        }
        Kind::GuidedMissile => {
            let (payload, entry, tint) = match projectile.payload {
                Payload::Cold => ("cold", 110, 0x4080ff),
                Payload::Poison => ("poison", 111, 0x40ff40),
                _ => panic!("unsupported guided missile payload"),
            };
            let alpha = lifetime_alpha(projectile, age);
            let phase = projectile.visual_phase_degrees;
            let mut body = NativeEnemyProjectileLayer::new(
                SpellAtlas::BadGuys,
                entry,
                &format!("guided-missile-{payload}-body"),
            );
            body.alpha = alpha * (0.5 + unit(projectile.id, tick, 0) * 0.5);
            body.blend = SpellBlend::Add;
            body.offset = Vec2::new(0.0, -15.0);
            body.scale =
                1.1 + float::sin(degrees(phase * 15.0)).abs() * 0.15 * projectile.visual_scale;
            let mut aura = NativeEnemyProjectileLayer::new(
                SpellAtlas::BadGuys,
                112,
                &format!("guided-missile-{payload}-aura"),
            );
            aura.alpha = alpha * float::sin(degrees(phase * 6.0)).abs() * 0.55;
            aura.blend = SpellBlend::Add;
            aura.offset = Vec2::new(0.0, -15.0);
            aura.rotation_radians = degrees(phase * 0.5);
            aura.scale = (1.0 + unit(projectile.id, tick, 1) * 0.3) * projectile.visual_scale;
            aura.tint = tint;
            layers.extend([body, aura]);
        }
        Kind::DemonBomb => {
            assert_eq!(projectile.payload, Payload::None, "Demon Bomb has no payload");
            for index in 0..3 {
                let mut draw = NativeEnemyProjectileLayer::new(
                    SpellAtlas::BadGuys,
                    267 + (word(projectile.id, tick, index) % 4) as usize,
                    &format!("demon-bomb-sample-{index}"),
                );
                draw.blend = if index == 0 { SpellBlend::Normal } else { SpellBlend::Add };
                draw.offset = Vec2::new(0.0, projectile.vertical_offset);
                draw.scale = if index < 2 { 2.0 } else { 1.5 };
                layers.push(draw);
            }
            if projectile.speed <= 2.0 {
                let mut draw = NativeEnemyProjectileLayer::new(
                    SpellAtlas::DeadHawg,
                    46 + (tick / 2 % 32) as usize,
                    "demon-bomb-ground",
                );
                draw.alpha = if projectile.speed <= 1.0 {
                    1.0
                } else {
                    (1.0 - projectile.speed * 0.5).max(0.0)
                };
                draw.blend = SpellBlend::Add;
                draw.offset = Vec2::new(0.0, -20.0);
                draw.scale_y = Some(0.5);
                layers.push(draw);
            }
            let mut glow =
                NativeEnemyProjectileLayer::new(SpellAtlas::BadGuys, 15, "demon-bomb-ground-glow");
            glow.alpha = 0.25;
            glow.blend = SpellBlend::Add;
            glow.scale_y = Some(0.8_f32 as f64);
            glow.tint = 0xff8000;
            underlays.push(glow);
        }
        Kind::PoisonPool => {
            assert_eq!(projectile.payload, Payload::Poison, "Poison Pool payload is Poison");
            let mut alpha = 1.0;
            for _ in 0..age.saturating_sub(3000).saturating_add(u64::from(age >= 3000)) {
                alpha = fround(alpha - (0.005_f32 as f64)).max(0.0);
                if alpha == 0.0 {
                    break;
                }
            }
            let scale = projectile.visual_scale;
            let pulse = float::sin(degrees(age as f64)) * 0.25 + 0.75;
            let mut outer =
                NativeEnemyProjectileLayer::new(SpellAtlas::DeadHawg, 0, "poison-pool-outer");
            outer.alpha = 0.5 * alpha;
            outer.scale = scale;
            let mut inner =
                NativeEnemyProjectileLayer::new(SpellAtlas::DeadHawg, 0, "poison-pool-inner");
            inner.alpha = pulse * alpha;
            inner.scale = (scale - 0.6).max(0.0) * scale * 0.75;
            underlays.extend([outer, inner]);
        }
    }
    for layer in layers.iter_mut().chain(&mut underlays) {
        if layer.scale_y.is_none() {
            layer.scale_y = Some(layer.scale);
        }
    }
    NativeEnemyProjectilePlan { layers, underlays, position: projectile.position, streak }
}

fn degrees(value: f64) -> f64 {
    value * std::f64::consts::PI / 180.0
}
fn lifetime_alpha(projectile: &NativeEnemyProjectileVisual, age: u64) -> f64 {
    (projectile.lifetime_ticks.saturating_sub(age) as f64 / 100.0).min(1.0)
}
fn word(id: u64, tick: u64, channel: u32) -> u32 {
    let mut value = id as u32
        ^ (tick as u32).wrapping_add(1).wrapping_mul(0x9e3779b1)
        ^ channel.wrapping_add(1).wrapping_mul(0x85ebca6b);
    value ^= value >> 16;
    value = value.wrapping_mul(0x7feb352d);
    value ^ (value >> 15)
}
fn unit(id: u64, tick: u64, channel: u32) -> f64 {
    crate::random_domain::native_random_float_from_word(word(id, tick, channel), 1.0)
}

pub fn native_death_magic_layers(
    position: Vec2,
    scale: f64,
    phase: f64,
    rng: &mut NativeRng,
) -> Vec<NativeEnemyProjectileLayer> {
    let mut layers = Vec::new();
    for pass in 0..2 {
        let pulse = float::sin(degrees(phase * 15.0)).abs();
        let mut outer = NativeEnemyProjectileLayer::new(
            SpellAtlas::BadGuys,
            110,
            &format!("death-magic-outer-disk-{pass}"),
        );
        outer.alpha = fround((0.2_f32 as f64) + rng.draw_scaled_float(0.25, false));
        outer.offset = position;
        outer.scale = (pulse * (0.15_f32 as f64) + 2.5) * scale;
        outer.tint = 0;
        layers.push(outer);
        let mut inner = NativeEnemyProjectileLayer::new(
            SpellAtlas::BadGuys,
            110,
            &format!("death-magic-inner-disk-{pass}"),
        );
        inner.alpha = fround((0.35_f32 as f64) + rng.draw_scaled_float(0.55_f32 as f64, false));
        inner.offset = position;
        inner.scale = (pulse * (0.15_f32 as f64) + 1.5) * scale;
        inner.tint = 0;
        layers.push(inner);
        let red = fround(0.75 + rng.draw_scaled_float(0.5, false));
        let luminance = fround(red * (0.3086_f32 as f64));
        let base = luminance * 0.5;
        let red = fround(base + red * 0.5).clamp(0.0, 1.0);
        let green = fround(base).clamp(0.0, 1.0);
        let tint = ((red * 255.0 + 0.5).floor() as u32) << 16
            | ((green * 255.0 + 0.5).floor() as u32) << 8
            | (green * 255.0 + 0.5).floor() as u32;
        let mut center = NativeEnemyProjectileLayer::new(
            SpellAtlas::BadGuys,
            111,
            &format!("death-magic-center-{pass}"),
        );
        center.alpha = float::sin(degrees(phase * 5.0)).abs() * (0.35_f32 as f64);
        center.blend = SpellBlend::Add;
        center.offset = position;
        center.rotation_radians = degrees(float::sin(degrees(phase)) * scale * 50.0);
        center.scale = (1.0 + rng.draw_scaled_float(0.1_f32 as f64, false)) * scale;
        center.tint = tint;
        layers.push(center);
        let count = rng.draw_integer(10);
        for spark in 0..count + 2 {
            let alpha = rng.draw_scaled_float(0.75, false);
            let distance = rng.draw_scaled_float(scale * 20.0, false);
            let angle = degrees(rng.draw_scaled_float(360.0, false));
            let size = (0.25 + rng.draw_scaled_float(0.2_f32 as f64, false)) * scale;
            let rotation = degrees(rng.draw_scaled_float(360.0, false));
            let mut draw = NativeEnemyProjectileLayer::new(
                SpellAtlas::BadGuys,
                111,
                &format!("death-magic-spark-{pass}-{spark}"),
            );
            draw.alpha = alpha;
            draw.blend = SpellBlend::Add;
            draw.offset = Vec2::new(
                position.x + float::sin(angle) * distance,
                position.y - float::cos(angle) * distance,
            );
            draw.rotation_radians = rotation;
            draw.scale = size;
            draw.tint = tint;
            layers.push(draw);
        }
        let mut star = NativeEnemyProjectileLayer::new(
            SpellAtlas::BadGuys,
            112,
            &format!("death-magic-star-{pass}"),
        );
        star.alpha = float::sin(degrees(phase * 8.0)).abs() * (0.55_f32 as f64);
        star.blend = SpellBlend::Add;
        star.offset = position;
        star.rotation_radians = degrees(scale * 50.0 * phase);
        star.scale = (1.0 + rng.draw_scaled_float(0.3_f32 as f64, false)) * scale;
        star.tint = tint;
        layers.push(star);
    }
    layers
}
