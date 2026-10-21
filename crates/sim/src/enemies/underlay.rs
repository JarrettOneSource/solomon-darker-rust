//! Enemy +0x28 draws from `renderer/native-enemy-underlay.ts`, before light-map multiply.

use super::EnemyToken;
use super::actor_presentation::{EnemyArtAtlas, EnemyArtBlend, EnemyArtRecords};
use super::model::{CoffinPhase, EnemyActor, EnemyBrain, SpiderPhase};
use crate::Vec2;
use crate::boneyard::shadows::NativeShadowRecord;
use crate::float;
use crate::float::fround;

#[derive(Clone, Debug, PartialEq)]
pub struct EnemyUnderlayLayer {
    pub alphas: [f64; 4],
    pub atlas: EnemyArtAtlas,
    pub blend: EnemyArtBlend,
    pub entry: usize,
    pub role: String,
    pub tint: u32,
    /// Native order: top left, top right, bottom left, bottom right.
    pub vertices: [Vec2; 4],
}

#[must_use]
pub fn native_enemy_directional_shadow_family(token: EnemyToken) -> bool {
    matches!(
        token,
        EnemyToken::Skeleton
            | EnemyToken::SkeletonArcher
            | EnemyToken::SkeletonMage
            | EnemyToken::Zombie
            | EnemyToken::DireFaculty
            | EnemyToken::Heartmonger
            | EnemyToken::Spider
    )
}

#[must_use]
pub fn native_enemy_underlay_plan(
    actor: &EnemyActor,
    tick: u64,
    records: &[NativeShadowRecord],
    light_scalar: f64,
    complex_shadows: bool,
    art: &(impl EnemyArtRecords + ?Sized),
) -> Vec<EnemyUnderlayLayer> {
    let sample = super::actor_presentation::native_enemy_presentation_input(
        actor,
        super::actor_presentation::EnemyActorAppearanceContext {
            tick,
            application_tick: tick,
            complex_lighting: true,
            spider_exposure: 0.0,
            spider_target: None,
            pike_target_position: None,
        },
    );
    native_enemy_underlay_plan_with_input(
        actor,
        tick,
        records,
        light_scalar,
        complex_shadows,
        art,
        &sample,
    )
}

#[must_use]
pub fn native_enemy_underlay_plan_with_input(
    actor: &EnemyActor,
    tick: u64,
    records: &[NativeShadowRecord],
    light_scalar: f64,
    complex_shadows: bool,
    art: &(impl EnemyArtRecords + ?Sized),
    sample: &super::actor_presentation::NativeEnemyPresentationInput,
) -> Vec<EnemyUnderlayLayer> {
    let token = actor.config.enemy_token;
    if matches!(token, EnemyToken::Imp | EnemyToken::Cocoon | EnemyToken::Wraith)
        || matches!(&actor.brain, EnemyBrain::Spider(brain) if brain.phase == SpiderPhase::Captured)
    {
        return Vec::new();
    }
    if let EnemyBrain::DemonSkull(brain) = &actor.brain {
        return vec![sprite_quad(
            art,
            EnemyArtAtlas::BadGuys,
            67,
            "demon-skull-shadow",
            brain.body_offset,
            4.0 * actor.config.scale,
            4.0 * actor.config.scale,
            1.0,
            0xffffff,
            EnemyArtBlend::Normal,
        )];
    }
    if let EnemyBrain::Demon(_) = &actor.brain {
        let offset = sample.demon_shadow_offset;
        return vec![sprite_quad(
            art,
            EnemyArtAtlas::BadGuys,
            67,
            "demon-shadow",
            offset,
            2.0,
            2.0,
            0.5,
            0xffffff,
            EnemyArtBlend::Normal,
        )];
    }
    if let EnemyBrain::Coffin(brain) = &actor.brain {
        if complex_shadows || brain.phase == CoffinPhase::Hidden {
            return Vec::new();
        }
        let pose = sample.coffin_pose;
        let entry = 383 + pose.trunc().clamp(0.0, 9.0) as usize;
        let mut layer = sprite_quad(
            art,
            EnemyArtAtlas::BadGuys,
            entry,
            "coffin-shadow",
            Vec2::ZERO,
            1.0,
            1.0,
            1.0,
            0,
            EnemyArtBlend::Normal,
        );
        let shift = art.record(EnemyArtAtlas::BadGuys, entry).height * 0.25;
        let angle = sample.coffin_rotation_radians;
        for (index, vertex) in layer.vertices.iter_mut().enumerate() {
            if index < 2 {
                vertex.x += shift;
                vertex.y += shift * 1.25;
            }
            let x = vertex.x * f64::from(brain.launch_scale);
            let y = vertex.y;
            *vertex = Vec2::new(
                fround(x * float::cos(angle) - y * float::sin(angle)),
                fround(x * float::sin(angle) + y * float::cos(angle)),
            );
        }
        return vec![layer];
    }
    if let EnemyBrain::Portal(_) = &actor.brain {
        if sample.alpha <= 0.0 {
            return Vec::new();
        }
        let age = tick.saturating_sub(actor.spawn_tick) + 1;
        let pulse = fround(0.5 + float::sin(age as f64 * 3.0 * std::f64::consts::PI / 180.0) * 0.5);
        let green = pulse * 0.5;
        let blue = pulse / 3.0;
        let luminance = fround(
            f64::from(0.3086_f32) + green * f64::from(0.6094_f32) + blue * f64::from(0.082_f32),
        );
        let channel = |value| (fround((luminance + value) * 0.5) * 255.0).trunc() as u32;
        let tint = channel(1.0) << 16 | channel(green) << 8 | channel(blue);
        let scale = sample.stride_phase_deg;
        return vec![
            sprite_quad(
                art,
                EnemyArtAtlas::DeadHawg,
                18,
                "portal-ground-shadow",
                Vec2::ZERO,
                0.5,
                0.5,
                sample.alpha,
                0,
                EnemyArtBlend::Normal,
            ),
            sprite_quad(
                art,
                EnemyArtAtlas::DeadHawg,
                22,
                "portal-ground-glow",
                Vec2::ZERO,
                scale,
                scale * f64::from(0.8_f32),
                sample.alpha,
                tint,
                EnemyArtBlend::Add,
            ),
            sprite_quad(
                art,
                EnemyArtAtlas::DeadHawg,
                180 + sample.gait_pose.trunc() as usize % 20,
                "portal-ground-aura",
                Vec2::ZERO,
                scale,
                scale * f64::from(0.8_f32),
                sample.alpha,
                0xffffff,
                EnemyArtBlend::Add,
            ),
        ];
    }
    let size = if token == EnemyToken::Heartmonger { 2.0 } else { 1.0 };
    let heading = match &actor.brain {
        EnemyBrain::Archer(brain) => brain.strafe.limb_heading_deg,
        _ => actor.heading_deg,
    } * std::f64::consts::PI
        / 180.0;
    let mut offset = Vec2::new(-5.0 * float::sin(heading), 5.0 * float::cos(heading));
    if token == EnemyToken::Spider {
        // Spider temporarily advances its native root before the shared shadow callback.
        offset.x += fround(actor.position.x + fround(10.0 * fround(float::sin(heading))))
            - actor.position.x;
        offset.y += fround(actor.position.y - fround(10.0 * fround(float::cos(heading))))
            - actor.position.y;
    }
    let mut layers = Vec::new();
    if complex_shadows {
        for record in records {
            let width = size * 10.0;
            let spread = (1.0 - record.distance_fraction).powi(3) * 10.0;
            let length = record.projection_distance.min(150.0);
            let gait = sample.shadow_lateral_offset;
            let perpendicular = Vec2::new(record.direction.y, -record.direction.x);
            let point = |along, across| {
                Vec2::new(
                    fround(offset.x + record.direction.x * along + perpendicular.x * across),
                    fround(offset.y + record.direction.y * along + perpendicular.y * across),
                )
            };
            let base = record.base_alpha * light_scalar;
            let tip = (1.0 - record.behind_scalar) * (1.0 - record.distance_fraction);
            layers.push(EnemyUnderlayLayer {
                alphas: [tip, tip, base, base],
                atlas: EnemyArtAtlas::BadGuys,
                blend: EnemyArtBlend::Normal,
                entry: 80,
                role: "enemy-directional-shadow".to_owned(),
                tint: 0,
                vertices: [
                    point(length, gait + width + spread),
                    point(length, gait - width - spread),
                    point(0.0, width),
                    point(0.0, -width),
                ],
            });
        }
    }
    let scale = size * if complex_shadows { f64::from(0.8_f32) } else { 1.0 };
    layers.push(sprite_quad(
        art,
        EnemyArtAtlas::BadGuys,
        67,
        "enemy-ground-shadow",
        offset,
        scale,
        scale,
        1.0,
        0xffffff,
        EnemyArtBlend::Normal,
    ));
    layers
}

// The arguments are the source sprite command, including independent native scales.
#[allow(clippy::too_many_arguments)]
fn sprite_quad(
    art: &(impl EnemyArtRecords + ?Sized),
    atlas: EnemyArtAtlas,
    entry: usize,
    role: &str,
    offset: Vec2,
    scale_x: f64,
    scale_y: f64,
    alpha: f64,
    tint: u32,
    blend: EnemyArtBlend,
) -> EnemyUnderlayLayer {
    let record = art.record(atlas, entry);
    let left = fround(offset.x - record.anchor_x * scale_x);
    let right = fround(offset.x + (record.width - record.anchor_x) * scale_x);
    let top = fround(offset.y - record.anchor_y * scale_y);
    let bottom = fround(offset.y + (record.height - record.anchor_y) * scale_y);
    EnemyUnderlayLayer {
        alphas: [alpha; 4],
        atlas,
        blend,
        entry,
        role: role.to_owned(),
        tint,
        vertices: [
            Vec2::new(left, top),
            Vec2::new(right, top),
            Vec2::new(left, bottom),
            Vec2::new(right, bottom),
        ],
    }
}
