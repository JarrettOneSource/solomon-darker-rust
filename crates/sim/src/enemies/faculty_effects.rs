//! Faculty smoke and spell particles from `enemies/faculty-effects.ts` and the shared
//! smoke constructor in `enemies/faculty-death.ts`.

use super::config::desaturate_color;
use super::effects::{
    DeathEffectOverrides, DeathEffectOwner, SimpleDeathEffectSpec, radial_vector,
};
use super::faculty_spells::heading_vector;
use super::model::{
    DeathPresentationOwner, EnemyDeathEffectKind, EnemyEffectAtlas, EnemyEffectBlend,
};
use super::world::EnemyWorld;
use crate::Vec2;
use crate::float;
use crate::float::{fround, hypot};
use crate::painter::WorldPainterOrder;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FacultySmokePhase {
    Living,
    Start,
    Dying,
    Terminal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FacultyProjectileSmoke {
    SkullTrail,
    DarkTrail,
    SkullImpact,
    DarkImpact,
    DarkRingImpact,
}

impl FacultyProjectileSmoke {
    const fn role(self) -> &'static str {
        match self {
            Self::SkullTrail => "skull-trail",
            Self::DarkTrail => "dark-trail",
            Self::SkullImpact => "skull-impact",
            Self::DarkImpact => "dark-impact",
            Self::DarkRingImpact => "dark-ring-impact",
        }
    }
}

pub fn spawn_faculty_smoke(
    world: &mut EnemyWorld,
    owner: DeathEffectOwner,
    tick: u64,
    phase: FacultySmokePhase,
    order: &mut WorldPainterOrder,
) {
    let entry = 10 + world.draw_integer(2) as usize;
    let speed = world.draw_unit() * if phase == FacultySmokePhase::Living { 4.0 } else { 8.0 };
    let velocity = radial_vector(world.draw_unit() * 360.0, speed);
    let rare_damping = world.draw_integer(6) == 3;
    let velocity_damping = if phase == FacultySmokePhase::Terminal {
        if rare_damping { 0.980_000_019_073_486_3 } else { 0.949_999_988_079_071 }
    } else if rare_damping {
        0.949_999_988_079_071
    } else {
        0.899_999_976_158_142_1
    };
    let rotation_deg = world.draw_unit() * 360.0;
    let scale = if matches!(phase, FacultySmokePhase::Living | FacultySmokePhase::Dying) {
        fround(0.899_999_976_158_142_1 + world.draw_unit() * 0.200_000_047_683_715_82)
    } else {
        fround(1.5 + world.draw_unit() * 0.5)
    };
    let alpha_loss = fround(
        (0.009_999_999_776_482_582 + world.draw_unit() * 0.019_999_999_552_965_164)
            * if phase == FacultySmokePhase::Terminal { 0.5 } else { 1.0 },
    );
    let tint = ((world.draw_unit() * 0.150_000_005_960_464_48 * 255.0).round() as u32) << 16;
    let presentation_owner = if world.draw_integer(5) == 3 {
        DeathPresentationOwner::WorldSorted
    } else {
        DeathPresentationOwner::PreWorldQueue
    };
    let role = match phase {
        FacultySmokePhase::Living => "faculty-living-smoke",
        FacultySmokePhase::Start => "faculty-start-smoke",
        FacultySmokePhase::Dying => "faculty-dying-smoke",
        FacultySmokePhase::Terminal => "faculty-terminal-smoke",
    };
    smoke(
        world,
        owner,
        tick,
        entry,
        role,
        scale,
        alpha_loss,
        DeathEffectOverrides {
            opacity_timer: Some(if phase == FacultySmokePhase::Terminal { 3.0 } else { 2.0 }),
            presentation_owner: Some(presentation_owner),
            rotation_deg: Some(rotation_deg),
            tint: Some(tint),
            velocity: Some(velocity),
            velocity_damping: Some(velocity_damping),
            ..DeathEffectOverrides::default()
        },
        order,
    );
}

pub fn spawn_faculty_projectile_smoke(
    world: &mut EnemyWorld,
    owner: DeathEffectOwner,
    tick: u64,
    profile: FacultyProjectileSmoke,
    arc_phase: f64,
    order: &mut WorldPainterOrder,
) {
    let trail =
        matches!(profile, FacultyProjectileSmoke::SkullTrail | FacultyProjectileSmoke::DarkTrail);
    let ring = profile == FacultyProjectileSmoke::DarkRingImpact;
    let count = match profile {
        FacultyProjectileSmoke::SkullTrail => 1,
        FacultyProjectileSmoke::DarkTrail => 2,
        FacultyProjectileSmoke::DarkRingImpact => 36,
        _ => 72,
    };
    for index in 0..count {
        let entry = 10 + world.steering_rng.draw_integer(2) as usize;
        let mut position = owner.position;
        if profile == FacultyProjectileSmoke::DarkTrail {
            let radius = world.steering_rng.draw_scaled_float(10.0, false);
            let offset =
                scaled(heading_vector(world.steering_rng.draw_scaled_float(360.0, false)), radius);
            position = Vec2::new(
                fround(position.x + offset.x),
                fround(
                    position.y + offset.y
                        - 15.0
                        - float::sin(arc_phase * std::f64::consts::PI / 180.0) * 50.0,
                ),
            );
        }
        let speed = if trail {
            world.steering_rng.draw_scaled_float(4.0, false)
        } else if ring {
            world.steering_rng.draw_scaled_float(6.0, false)
        } else {
            fround(3.0 + world.steering_rng.draw_scaled_float(3.0, false))
        };
        let heading = if trail || ring {
            world.steering_rng.draw_scaled_float(360.0, false)
        } else {
            f64::from(index) * 10.0 + world.steering_rng.draw_scaled_float(3.0, true)
        };
        let velocity = scaled(heading_vector(heading), speed);
        let rare = world.steering_rng.draw_integer(6) == 3;
        let velocity_damping = if ring {
            if rare { 0.980_000_019_073_486_3 } else { 0.949_999_988_079_071 }
        } else if rare {
            0.949_999_988_079_071
        } else {
            0.899_999_976_158_142_1
        };
        let rotation_deg = world.steering_rng.draw_scaled_float(360.0, false);
        let scale = if ring {
            fround(1.5 + world.steering_rng.draw_scaled_float(0.5, false))
        } else {
            fround(
                fround(
                    0.600_000_023_841_857_9
                        + world.steering_rng.draw_scaled_float(0.199_999_988_079_071_04, false),
                ) * if profile == FacultyProjectileSmoke::DarkTrail { 2.5 } else { 1.0 },
            )
        };
        let alpha_loss = fround(
            fround(
                0.009_999_999_776_482_582
                    + world.steering_rng.draw_scaled_float(0.019_999_999_552_965_164, false),
            ) * if !trail && !ring { 0.5 } else { 1.0 },
        );
        let red = world.steering_rng.draw_scaled_float(0.150_000_005_960_464_48, false);
        let world_sorted = world.steering_rng.draw_integer(5) == 3;
        smoke(
            world,
            owner,
            tick,
            entry,
            profile.role(),
            scale,
            alpha_loss,
            DeathEffectOverrides {
                position: Some(position),
                opacity_timer: Some(if ring {
                    2.0
                } else if profile == FacultyProjectileSmoke::DarkTrail {
                    1.25
                } else {
                    1.0
                }),
                presentation_owner: Some(if world_sorted {
                    DeathPresentationOwner::WorldSorted
                } else {
                    DeathPresentationOwner::PreWorldQueue
                }),
                rotation_deg: Some(rotation_deg),
                tint: Some(((red * 255.0).round() as u32) << 16),
                velocity: Some(velocity),
                velocity_damping: Some(velocity_damping),
                ..DeathEffectOverrides::default()
            },
            order,
        );
    }
}

pub fn spawn_blightning_smoke(
    world: &mut EnemyWorld,
    owner: DeathEffectOwner,
    tick: u64,
    source: Vec2,
    endpoint: Vec2,
    order: &mut WorldPainterOrder,
) {
    for index in 0..3 {
        let entry = 10 + world.steering_rng.draw_integer(2) as usize;
        let mut position = source;
        let velocity = if index == 0 {
            let speed = world.steering_rng.draw_scaled_float(8.0, false);
            scaled(heading_vector(world.steering_rng.draw_scaled_float(360.0, false)), speed)
        } else {
            let t = world.steering_rng.draw_scaled_float(1.0, false);
            let dx = endpoint.x - source.x;
            let dy = endpoint.y - source.y;
            position = Vec2::new(fround(source.x + dx * t), fround(source.y + dy * t));
            let speed = world.steering_rng.draw_scaled_float(10.0, true);
            let length = hypot(dx, dy);
            if length == 0.0 {
                Vec2::ZERO
            } else {
                Vec2::new(fround(dy / length * speed), fround(-dx / length * speed))
            }
        };
        let rare = world.steering_rng.draw_integer(6) == 3;
        let damping = if index == 0 {
            if rare { 0.920_000_016_689_300_5 } else { 0.879_999_995_231_628_4 }
        } else if rare {
            0.899_999_976_158_142_1
        } else {
            0.850_000_023_841_857_9
        };
        let rotation_deg = world.steering_rng.draw_scaled_float(360.0, false);
        let scale = fround(1.5 + world.steering_rng.draw_scaled_float(0.5, false));
        let loss = fround(
            0.009_999_999_776_482_582
                + world.steering_rng.draw_scaled_float(0.019_999_999_552_965_164, false),
        );
        let red = world.steering_rng.draw_scaled_float(0.150_000_005_960_464_48, false);
        smoke(
            world,
            owner,
            tick,
            entry,
            if index == 0 { "blightning-source-smoke" } else { "blightning-path-smoke" },
            scale,
            loss,
            DeathEffectOverrides {
                opacity_timer: Some(if index == 0 { 1.0 } else { 1.25 }),
                position: Some(position),
                presentation_owner: Some(DeathPresentationOwner::PreWorldQueue),
                rotation_deg: Some(rotation_deg),
                tint: Some(((red * 255.0).round() as u32) << 16),
                velocity: Some(velocity),
                velocity_damping: Some(damping),
                ..DeathEffectOverrides::default()
            },
            order,
        );
    }
}

pub fn spawn_tragic_circle_effects(
    world: &mut EnemyWorld,
    owner: DeathEffectOwner,
    tick: u64,
    remaining_ticks: u64,
    order: &mut WorldPainterOrder,
) {
    world.steering_rng.draw_scaled_float(0.25, true);
    let red = fround(0.75 + world.steering_rng.draw_scaled_float(0.5, false));
    let color = desaturate_color([red, 0.0, 0.0, 1.0], 0.5);
    let tint = (((color[0] * 255.0).round() as u32) << 16)
        | (((color[1] * 255.0).round() as u32) << 8)
        | (color[2] * 255.0).round() as u32;
    for _ in 0..1 + (tick & 1) {
        for shadow in [false, true] {
            let alpha = fround(
                (remaining_ticks as f64 / 100.0).min(1.0)
                    * fround(0.5 + world.steering_rng.draw_scaled_float(0.5, false)),
            );
            let mut scale = fround(
                4.0 * fround(
                    0.975_000_023_841_857_9
                        + world.steering_rng.draw_scaled_float(0.025_000_000_372_529_03, false),
                ),
            );
            if shadow {
                scale = fround(scale * 1.034_999_966_621_399);
            }
            let magnitude = fround(0.5 + world.steering_rng.draw_scaled_float(1.0, false));
            let angular_velocity = world.steering_rng.draw_sign(magnitude);
            let rotation = world.steering_rng.draw_scaled_float(360.0, false);
            world.spawn_simple_death_effect(
                owner,
                tick,
                SimpleDeathEffectSpec {
                    alpha,
                    alpha_loss_per_tick: 0.05,
                    atlas: EnemyEffectAtlas::BadGuys,
                    blend_mode: EnemyEffectBlend::Normal,
                    entry: 48,
                    kind: EnemyDeathEffectKind::MoveFadePerspective,
                    lifetime_ticks: 20,
                    role: if shadow { "tragic-circle-shadow" } else { "tragic-circle-ring" },
                    scale,
                },
                DeathEffectOverrides {
                    angular_velocity_deg: Some(angular_velocity),
                    presentation_owner: Some(DeathPresentationOwner::PreWorldQueue),
                    rotation_deg: Some(rotation),
                    tint: Some(if shadow { 0 } else { tint }),
                    velocity_damping: Some(1.0),
                    ..DeathEffectOverrides::default()
                },
                order,
            );
        }
    }
    let phase = world.steering_rng.draw_scaled_float(360.0, false);
    for angle in (0..=90).step_by(10) {
        let entry = 10 + world.steering_rng.draw_integer(2) as usize;
        let direction = heading_vector(f64::from(angle) + phase);
        let position = Vec2::new(
            fround(owner.position.x + direction.x * 200.0),
            fround(owner.position.y + direction.y * 200.0 * 0.800_000_011_920_929),
        );
        let speed = fround(3.0 + world.steering_rng.draw_scaled_float(3.0, false));
        let heading =
            f64::from(angle) + phase + 90.0 + world.steering_rng.draw_scaled_float(3.0, true);
        let mut velocity = scaled(heading_vector(heading), speed);
        let mut damping =
            if world.steering_rng.draw_integer(6) == 3 { 0.850_000_023_841_857_9 } else { 0.5 };
        let mut loss = fround(
            world.steering_rng.draw_scaled_float(0.019_999_999_552_965_164, false)
                + 0.009_999_999_776_482_582,
        );
        if world.steering_rng.draw_integer(2) == 1 {
            let speed = fround(1.0 + world.steering_rng.draw_scaled_float(2.0, false));
            let heading =
                f64::from(angle) + phase + world.steering_rng.draw_scaled_float(3.0, true);
            velocity = scaled(heading_vector(heading), -speed);
            damping = 0.980_000_019_073_486_3;
            loss = fround(loss * 3.0);
        }
        let rotation = world.steering_rng.draw_scaled_float(360.0, false);
        let scale = fround(
            world.steering_rng.draw_scaled_float(0.199_999_988_079_071_04, false)
                + 0.600_000_023_841_857_9,
        );
        let red = world.steering_rng.draw_scaled_float(0.150_000_005_960_464_48, false);
        let world_sorted = world.steering_rng.draw_integer(5) == 1;
        smoke(
            world,
            owner,
            tick,
            entry,
            "tragic-circle-smoke",
            scale,
            loss,
            DeathEffectOverrides {
                opacity_timer: Some(2.0),
                position: Some(position),
                presentation_owner: Some(if world_sorted {
                    DeathPresentationOwner::WorldSorted
                } else {
                    DeathPresentationOwner::PreWorldQueue
                }),
                rotation_deg: Some(rotation),
                tint: Some(((red * 255.0).round() as u32) << 16),
                velocity: Some(velocity),
                velocity_damping: Some(damping),
                ..DeathEffectOverrides::default()
            },
            order,
        );
    }
}

#[allow(clippy::too_many_arguments)] // Mirrors the one smoke constructor shared by source profiles.
fn smoke(
    world: &mut EnemyWorld,
    owner: DeathEffectOwner,
    tick: u64,
    entry: usize,
    role: &str,
    scale: f64,
    alpha_loss_per_tick: f64,
    options: DeathEffectOverrides,
    order: &mut WorldPainterOrder,
) {
    world.spawn_simple_death_effect(
        owner,
        tick,
        SimpleDeathEffectSpec {
            alpha: 1.0,
            alpha_loss_per_tick,
            atlas: EnemyEffectAtlas::BadGuys,
            blend_mode: EnemyEffectBlend::Normal,
            entry,
            kind: EnemyDeathEffectKind::MoveFade,
            lifetime_ticks: 1000,
            role,
            scale,
        },
        options,
        order,
    );
}

fn scaled(direction: Vec2, magnitude: f64) -> Vec2 {
    Vec2::new(fround(direction.x * magnitude), fround(direction.y * magnitude))
}
