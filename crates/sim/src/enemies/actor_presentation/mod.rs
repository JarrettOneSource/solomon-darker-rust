//! Authoritative enemy stock-art plans from the pinned Website's enemy presenters.
//! These consume actual family state and registered art sockets; no rendering clock
//! or gameplay random stream is hidden in the view.

mod bosses;
mod demon;
mod layers;
mod model;
mod sample;
mod skeleton;
mod sprites;
mod zombie;
pub use model::*;
pub use sample::*;
pub use skeleton::native_skeleton_base_tint;

use crate::Vec2;
use crate::enemies::EnemyToken;
use crate::enemies::model::*;

#[must_use]
pub fn native_enemy_actor_plan(
    actor: &impl EnemyActorRead,
    context: EnemyActorAppearanceContext,
    art: &(impl EnemyArtPoints + ?Sized),
) -> EnemyActorPresentationPlan {
    let sample = native_enemy_presentation_input(actor, context);
    native_enemy_actor_plan_with_input(actor, context, &sample, art)
}

/// Draw an already sampled/interpolated animation without resampling brain state.
#[must_use]
pub fn native_enemy_actor_plan_with_input(
    actor: &impl EnemyActorRead,
    context: EnemyActorAppearanceContext,
    sample: &NativeEnemyPresentationInput,
    art: &(impl EnemyArtPoints + ?Sized),
) -> EnemyActorPresentationPlan {
    let token = actor.config.enemy_token;
    let root_scale = match token {
        EnemyToken::Portal
        | EnemyToken::Coffin
        | EnemyToken::Spider
        | EnemyToken::Cocoon
        | EnemyToken::DireFaculty => 1.0,
        _ => actor.config.scale,
    };
    let heading = match actor.brain() {
        EnemyBrain::Zombie(_) => actor.heading_deg + sample.zombie_angular_offset_deg,
        EnemyBrain::Faculty(brain) => brain.body_heading_deg,
        _ => actor.heading_deg,
    };
    let facing = layers::facing(token, heading);
    let spawn_age_ticks = context.tick.saturating_sub(actor.spawn_tick);
    let family = if matches!(actor.brain(), EnemyBrain::Spider(brain) if brain.phase == SpiderPhase::Captured)
    {
        EnemyFamilyPresentation::default()
    } else if actor.life_state == EnemyLifeState::Dying {
        match token {
            EnemyToken::Demon => demon::death(actor, sample),
            EnemyToken::DemonSkull => bosses::demon_skull(actor, context.tick, art),
            EnemyToken::DireFaculty => bosses::faculty(actor, facing, context.tick, sample, art),
            _ => EnemyFamilyPresentation::default(),
        }
    } else {
        match token {
            EnemyToken::Skeleton => {
                skeleton::skeleton_presentation(actor, facing, context, sample, art)
            }
            EnemyToken::SkeletonArcher => {
                skeleton::archer_presentation(actor, facing, context.tick, sample, art)
            }
            EnemyToken::SkeletonMage => {
                skeleton::mage_presentation(actor, facing, context, sample, art)
            }
            EnemyToken::Zombie => zombie::zombie_presentation(actor, facing, sample, art),
            EnemyToken::Imp => sprites::imp(actor, facing, sample),
            EnemyToken::Portal => sprites::portal(actor, sample),
            EnemyToken::Wraith => sprites::wraith(actor, facing),
            EnemyToken::Coffin => sprites::coffin(actor, sample),
            EnemyToken::Spider => sprites::spider(actor, facing, sample),
            EnemyToken::Cocoon => EnemyFamilyPresentation::default(),
            EnemyToken::Demon => demon::presentation(actor, facing, sample, art),
            EnemyToken::DemonSkull => bosses::demon_skull(actor, context.tick, art),
            EnemyToken::DireFaculty => bosses::faculty(actor, facing, context.tick, sample, art),
            EnemyToken::Heartmonger => bosses::heartmonger(actor, facing, sample, art),
        }
    };
    let alpha = sample.alpha.clamp(0.0, 1.0);
    let hit_flash =
        if token == EnemyToken::Portal { 0.0 } else { sample.hit_flash.clamp(0.0, 1.0) };
    let vertical = sample.vertical_offset;
    let vertical = if token == EnemyToken::Demon { vertical } else { vertical / root_scale };
    let transform = |mut layer: EnemySpriteLayer| {
        let offset = if layer.apply_vertical_offset { vertical } else { 0.0 };
        layer.alpha *= alpha;
        layer.offset.y += offset;
        if let Some(stretch) = &mut layer.stretch {
            stretch.start.y += offset;
            stretch.end.y += offset;
        }
        layer
    };
    let mut layers = Vec::new();
    layers.extend(family.before.into_iter().map(transform));
    layers.extend(family.body.into_iter().map(transform));
    if hit_flash > 0.0 {
        for source in family.hit_body {
            let mut layer = transform(source);
            if layer.alpha <= 0.0 {
                continue;
            }
            layer.alpha *= hit_flash;
            layer.role = format!("hit:{}", layer.role);
            layer.tint = puppet_hit_tint(context.complex_lighting, layer.tint);
            layer.texture_diffuse = true;
            layers.push(layer);
        }
    }
    layers.extend(family.after.into_iter().map(transform));
    for effect in &sample.effects {
        layers.push(transform(effect.layer.clone().into_actor_space(root_scale)));
    }
    let mut segments = Vec::new();
    let mut hit_segments = Vec::new();
    for mut segment in family.segments {
        segment.alpha *= alpha;
        segment.start.y += vertical;
        segment.end.y += vertical;
        segments.push(segment.clone());
        if hit_flash > 0.0 {
            segment.alpha *= hit_flash;
            segment.role = format!("hit:{}", segment.role);
            segment.before_role = format!("hit:{}", segment.before_role);
            segment.tint = puppet_hit_tint(context.complex_lighting, segment.tint);
            hit_segments.push(segment);
        }
    }
    // Source segments retain the original pass first, then all hit segments.
    segments.extend(hit_segments);
    let attack_effect_origin = attack_effect_origin(actor, &layers, art);
    EnemyActorPresentationPlan {
        attack_effect_origin,
        facing,
        root_scale,
        layers,
        segments,
        spawn_age_ticks,
    }
}

fn attack_effect_origin(
    actor: &impl EnemyActorRead,
    layers: &[EnemySpriteLayer],
    art: &(impl EnemyArtPoints + ?Sized),
) -> Option<Vec2> {
    use crate::enemies::attack_presentation::{
        native_demon_bomb_bearing, native_demon_bomb_muzzle_origin, native_imp_contact_burst_origin,
    };
    if actor.config.enemy_token == EnemyToken::Imp {
        return Some(native_imp_contact_burst_origin(actor.position, actor.heading_deg));
    }
    if actor.config.enemy_token != EnemyToken::Demon {
        return None;
    }
    let controller = layers.iter().find(|layer| layer.role == "demon-controller-body")?;
    let pose = (controller.entry - 19) / 18;
    let entry = 19 + pose * 18 + (native_demon_bomb_bearing(actor.heading_deg) / 20.0) as usize;
    let point = art
        .points(EnemyArtAtlas::Demon, entry)
        .get(5)
        .expect("Demon controller requires authored muzzle point 5");
    Some(native_demon_bomb_muzzle_origin(
        actor.position,
        actor.heading_deg,
        Vec2::new(point.x + controller.offset.x * actor.config.scale, point.y),
        controller.offset.y * actor.config.scale,
    ))
}

pub(super) fn puppet_hit_tint(complex_lighting: bool, tint: u32) -> u32 {
    let red = if complex_lighting { f64::from(0.65_f32) } else { 1.0 };
    ((((tint >> 16) & 255) as f64 * red).trunc() as u32) << 16
}

/// Coffin children register and paint independently from the parent body.
#[must_use]
pub fn native_maggot_plan(
    maggot: &EnemyMaggotActor,
    tick: u64,
    complex_lighting: bool,
) -> Vec<EnemySpriteLayer> {
    let sample = native_maggot_presentation_input(maggot, tick);
    native_maggot_plan_with_input(maggot, &sample, complex_lighting)
}

#[must_use]
pub fn native_maggot_plan_with_input(
    maggot: &EnemyMaggotActor,
    sample: &NativeMaggotPresentationInput,
    complex_lighting: bool,
) -> Vec<EnemySpriteLayer> {
    let biting = sample.state == NativeMaggotState::Bite;
    let emerging = sample.state == NativeMaggotState::Emerging;
    let (atlas, entry) = if biting {
        (
            EnemyArtAtlas::BadGuys,
            202 + 18 + layers::facing(EnemyToken::Skeleton, sample.heading_deg),
        )
    } else if sample.state == NativeMaggotState::Death {
        (EnemyArtAtlas::DeadHawg, 28)
    } else if emerging {
        let heading = crate::heading::heading_degrees_from_vector(
            maggot.launch_velocity.x,
            maggot.launch_velocity.y,
        );
        let orientation = (((heading + 18.0) / 36.0).floor() as i64).rem_euclid(10) as usize;
        (
            EnemyArtAtlas::BadGuys,
            2013 + layers::bounded_pose(sample.emergence_phase, 4) * 10 + orientation,
        )
    } else {
        (
            EnemyArtAtlas::BadGuys,
            202 + layers::bounded_pose(sample.pose, 1) * 18
                + layers::facing(EnemyToken::Skeleton, sample.heading_deg),
        )
    };
    let role = if emerging {
        format!(
            "maggot-body-emerging-{}",
            match maggot.launch_trajectory {
                MaggotLaunchTrajectory::Edge => "edge",
                MaggotLaunchTrajectory::Lid => "lid",
            }
        )
    } else {
        "maggot-body".to_owned()
    };
    let body = EnemySpriteLayer {
        alpha: sample.alpha.clamp(0.0, 1.0),
        offset: Vec2::new(0.0, sample.vertical_offset),
        scale: sample.visual_scale,
        ..EnemySpriteLayer::new(atlas, entry, role)
    };
    let mut layers = vec![body.clone()];
    let hit = sample.hit_flash.clamp(0.0, 1.0);
    if hit > 0.0 {
        layers.push(EnemySpriteLayer {
            alpha: sample.alpha.clamp(0.0, 1.0) * hit,
            role: "hit:maggot-body".to_owned(),
            texture_diffuse: true,
            tint: puppet_hit_tint(complex_lighting, 0xffffff),
            ..body
        });
    }
    layers
}

/// `nativeEtherDrainCapturedImage`: the pre-hit family art survives terminal
/// dispatch. Families without a capturable image still flush the field manager.
#[must_use]
pub fn native_ether_drain_captured_image(
    actor: &impl EnemyActorRead,
    tick: u64,
) -> Option<crate::secondary::ether_drain::NativeEtherDrainCapturedImage> {
    use crate::enemies::config::EnemyFamilyConfig;
    use crate::secondary::ether_drain::{NativeEtherDrainAtlas, NativeEtherDrainCapturedImage};
    let facing = layers::facing(EnemyToken::Skeleton, actor.heading_deg);
    let (atlas, entry, tint, body_y_offset) = match actor.config.enemy_token {
        EnemyToken::Skeleton | EnemyToken::SkeletonArcher | EnemyToken::SkeletonMage => {
            let headgear = match actor.config.family {
                EnemyFamilyConfig::Skeleton { headgear, .. }
                | EnemyFamilyConfig::Archer { headgear, .. }
                | EnemyFamilyConfig::Mage { headgear, .. } => headgear,
                _ => unreachable!("Skeleton family has its retained headgear"),
            };
            let head_facing =
                (facing as i64 + i64::from(actor.head_facing_offset) + 18) as usize % 18;
            (
                NativeEtherDrainAtlas::BadGuys,
                skeleton::HEADGEAR_BASES[headgear as usize] + head_facing,
                native_skeleton_base_tint(actor, tick.saturating_sub(actor.spawn_tick), false),
                23,
            )
        }
        EnemyToken::Zombie => {
            let EnemyBrain::Zombie(brain) = actor.brain() else { return None };
            if brain.head_type > 2 {
                return None;
            }
            (
                NativeEtherDrainAtlas::BadGuys,
                2293 + usize::from(brain.head_type) * 18 + facing,
                0xffffff,
                0,
            )
        }
        EnemyToken::Demon => (NativeEtherDrainAtlas::Demon, 80 + facing, 0xffffff, 0),
        _ => return None,
    };
    Some(NativeEtherDrainCapturedImage { atlas, entry, tint, body_y_offset })
}
