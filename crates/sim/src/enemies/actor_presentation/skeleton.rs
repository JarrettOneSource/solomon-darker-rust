//! Skeleton, Archer and Mage layers from `native-enemy-skeleton-presentation.ts`.

use super::layers::*;
use super::model::*;
use super::sample::*;
use crate::Vec2;
use crate::enemies::EnemyToken;
use crate::enemies::config::{
    ArrowType, EnemyFamilyConfig, MageElement, SkeletonWeapon, desaturate_color,
};
use crate::enemies::model::{EnemyActorRead, EnemyBrain};
use crate::float;
use crate::float::fround;

pub(super) const HEADGEAR_BASES: [usize; 6] = [1477, 1531, 1549, 1495, 1513, 1567];

pub(super) fn skeleton_presentation(
    actor: &impl EnemyActorRead,
    facing: usize,
    context: EnemyActorAppearanceContext,
    sample: &NativeEnemyPresentationInput,
    art: &(impl EnemyArtPoints + ?Sized),
) -> EnemyFamilyPresentation {
    let EnemyFamilyConfig::Skeleton { weapon, armor, headgear } = actor.config.family else {
        unreachable!("Skeleton family")
    };
    let body_pose = sample.body_pose;
    let body_base = if armor {
        match weapon {
            SkeletonWeapon::Claw => 613,
            SkeletonWeapon::Pike => 991,
            _ => 919,
        }
    } else {
        match weapon {
            SkeletonWeapon::Claw => 1117,
            SkeletonWeapon::Pike => 1405,
            _ => 1333,
        }
    };
    let body_count = match weapon {
        SkeletonWeapon::Claw => {
            if armor {
                9
            } else {
                12
            }
        }
        SkeletonWeapon::Pike => 3,
        _ => 4,
    };
    let mut layers = vec![EnemySpriteLayer::new(
        EnemyArtAtlas::BadGuys,
        1585 + bounded_pose(sample.gait_pose, 7) * 18 + facing,
        "skeleton-limbs",
    )];
    if let Some(pose) = bank_pose(body_pose, body_count) {
        layers.push(EnemySpriteLayer::new(
            EnemyArtAtlas::BadGuys,
            body_base + pose * 18 + facing,
            "skeleton-body",
        ));
    }
    let weapon_base = match weapon {
        SkeletonWeapon::Sword => Some(1045),
        SkeletonWeapon::Mace | SkeletonWeapon::Flail => Some(847),
        SkeletonWeapon::Axe => Some(775),
        _ => None,
    };
    if let Some(base) = weapon_base
        && let Some(pose) = bank_pose(body_pose, 4)
    {
        layers.push(EnemySpriteLayer::new(
            EnemyArtAtlas::BadGuys,
            base + pose * 18 + facing,
            "skeleton-weapon",
        ));
    }
    let head_facing = (facing as i64 + i64::from(actor.head_facing_offset) + 18) as usize % 18;
    layers.push(EnemySpriteLayer::new(
        EnemyArtAtlas::BadGuys,
        HEADGEAR_BASES[headgear as usize] + head_facing,
        "skeleton-headgear",
    ));
    let mut segments = Vec::new();
    if matches!(weapon, SkeletonWeapon::Mace | SkeletonWeapon::Flail) {
        if let Some(index) = layers.iter().position(|layer| layer.role == "skeleton-weapon") {
            let first = point(art, EnemyArtAtlas::BadGuys, layers[index].entry, 0);
            if weapon == SkeletonWeapon::Mace {
                layers.insert(
                    index + 1,
                    EnemySpriteLayer {
                        offset: first,
                        scale: 1.0 / actor.config.scale,
                        ..EnemySpriteLayer::new(EnemyArtAtlas::BadGuys, 46, "skeleton-mace-head")
                    },
                );
            } else {
                let second = point(art, EnemyArtAtlas::BadGuys, layers[index].entry, 1);
                let orbit = body_pose == 0.0 && (stable_unit(actor, 74, 0) * 2.0).floor() == 1.0;
                let angle = radians(fround(
                    (actor.id as u32).wrapping_mul(35) as i32 as f64
                        - context.application_tick as f64 * 10.0,
                ));
                let end = if orbit {
                    Vec2::new(
                        first.x + float::sin(angle) * 20.0,
                        first.y - float::cos(angle) * 20.0 * 0.800000011920929,
                    )
                } else {
                    second
                };
                let phase = float::sin(radians(sample.body_gait_phase * 90.0));
                let heading = radians(actor.heading_deg);
                let head = if orbit {
                    end
                } else {
                    Vec2::new(
                        second.x - float::cos(heading) * phase * 3.0,
                        second.y - float::sin(heading) * phase * 3.0,
                    )
                };
                segments.push(EnemySegmentLayer {
                    alpha: 1.0,
                    before_role: "skeleton-flail-head".to_owned(),
                    blend: EnemyArtBlend::Normal,
                    start: Vec2::new(first.x / actor.config.scale, first.y / actor.config.scale),
                    end: Vec2::new(end.x / actor.config.scale, end.y / actor.config.scale),
                    role: "skeleton-flail-chain".to_owned(),
                    tint: 0x3f3f3f,
                    width: 3.0 / actor.config.scale,
                });
                layers.insert(
                    if orbit { index + 1 } else { 1 },
                    EnemySpriteLayer {
                        offset: head,
                        scale: 1.0 / actor.config.scale,
                        ..EnemySpriteLayer::new(EnemyArtAtlas::BadGuys, 46, "skeleton-flail-head")
                    },
                );
            }
        }
    } else if weapon == SkeletonWeapon::Pike
        && let Some(pose) = bank_pose(body_pose, 3)
    {
        let entry = 991 + pose * 18 + facing;
        let target = sample.pike_target_offset;
        let start = target.map_or_else(
            || point(art, EnemyArtAtlas::BadGuys, entry, 0),
            |target| Vec2::new(target.x, target.y - 25.0),
        );
        layers.insert(
            1,
            EnemySpriteLayer {
                stretch: Some(EnemyArtStretch {
                    start,
                    end: point(art, EnemyArtAtlas::BadGuys, entry, 1),
                }),
                scale_x: Some(7.0 / 8.0 / actor.config.scale),
                ..EnemySpriteLayer::new(
                    EnemyArtAtlas::BadGuys,
                    if target.is_some() { 56 } else { 54 },
                    "skeleton-pike-shaft",
                )
            },
        );
    }
    let (body, hit_body) = compose_family(
        actor,
        context.tick - actor.spawn_tick,
        layers,
        "skeleton",
        None,
        true,
        sample,
    );
    if actor.config.burning {
        let original = segments.clone();
        for pass in 1..=2 {
            for source in &original {
                segments.push(EnemySegmentLayer {
                    before_role: format!("{}:burn-glow-{pass}", source.before_role),
                    role: format!("{}:burn-glow-{pass}", source.role),
                    blend: EnemyArtBlend::Add,
                    ..source.clone()
                });
            }
        }
    }
    EnemyFamilyPresentation { body, hit_body, segments, ..Default::default() }
}

pub(super) fn archer_presentation(
    actor: &impl EnemyActorRead,
    facing: usize,
    tick: u64,
    sample: &NativeEnemyPresentationInput,
    art: &(impl EnemyArtPoints + ?Sized),
) -> EnemyFamilyPresentation {
    let EnemyFamilyConfig::Archer { headgear, arrow_type, .. } = actor.config.family else {
        unreachable!("Archer family")
    };
    let EnemyBrain::Archer(brain) = actor.brain() else { unreachable!("Archer brain") };
    let limb_facing = super::layers::facing(EnemyToken::Skeleton, brain.strafe.limb_heading_deg);
    let body_entry = 451 + bounded_pose(sample.body_pose, 8) * 18 + facing;
    let head_facing = (facing as i64 + i64::from(actor.head_facing_offset) + 18) as usize % 18;
    let layers = vec![
        EnemySpriteLayer::new(
            EnemyArtAtlas::BadGuys,
            1585 + bounded_pose(sample.gait_pose, 7) * 18 + limb_facing,
            "archer-limbs",
        ),
        EnemySpriteLayer::new(EnemyArtAtlas::BadGuys, body_entry, "archer-body"),
        EnemySpriteLayer::new(
            EnemyArtAtlas::BadGuys,
            HEADGEAR_BASES[headgear as usize] + head_facing,
            "archer-headgear",
        ),
    ];
    let bow = point(art, EnemyArtAtlas::BadGuys, body_entry, 0);
    let age = tick - actor.spawn_tick;
    let held = if sample.body_pose.floor() == 8.0 || arrow_type == ArrowType::Normal {
        None
    } else {
        let fire = arrow_type == ArrowType::Fire;
        Some(EnemySpriteLayer {
            alpha: actor.lighting.charge.clamp(0.0, 1.0),
            blend: EnemyArtBlend::Add,
            offset: Vec2::new(bow.x, bow.y - 5.0),
            tint: if fire { 0xffffff } else { 0x007f00 },
            ..EnemySpriteLayer::new(
                EnemyArtAtlas::BadGuys,
                if fire { 255 + (age / 5 % 12) as usize } else { 271 + (age / 6 % 12) as usize },
                if fire { "archer-held-fire-arrow" } else { "archer-held-poison-arrow" },
            )
        })
    };
    let (body, hit_body) = compose_family(actor, age, layers, "archer", held, true, sample);
    EnemyFamilyPresentation { body, hit_body, ..Default::default() }
}

pub(super) fn mage_presentation(
    actor: &impl EnemyActorRead,
    facing: usize,
    context: EnemyActorAppearanceContext,
    sample: &NativeEnemyPresentationInput,
    art: &(impl EnemyArtPoints + ?Sized),
) -> EnemyFamilyPresentation {
    let EnemyFamilyConfig::Mage { headgear, cloak, element, .. } = actor.config.family else {
        unreachable!("Mage family")
    };
    let EnemyBrain::Mage(brain) = actor.brain() else { unreachable!("Mage brain") };
    let body_entry = 1729 + bounded_pose(sample.body_pose, 4) * 18 + facing;
    let mut source = vec![
        EnemySpriteLayer::new(
            EnemyArtAtlas::BadGuys,
            1585 + bounded_pose(sample.gait_pose, 7) * 18 + facing,
            "mage-limbs",
        ),
        EnemySpriteLayer::new(EnemyArtAtlas::BadGuys, body_entry, "mage-body"),
    ];
    if cloak {
        let entry = 1459 + facing;
        let index = if point(art, EnemyArtAtlas::BadGuys, entry, 0).x < 0.0 { 1 } else { 2 };
        source.insert(index, EnemySpriteLayer::new(EnemyArtAtlas::BadGuys, entry, "mage-cloak"));
    }
    let head_facing = (facing as i64 + i64::from(actor.head_facing_offset) + 18) as usize % 18;
    let head = EnemySpriteLayer::new(
        EnemyArtAtlas::BadGuys,
        HEADGEAR_BASES[headgear as usize] + head_facing,
        "mage-headgear",
    );
    let age = context.tick - actor.spawn_tick;
    let (mut body, mut hit_body) = compose_family(actor, age, source, "mage", None, true, sample);
    if brain.disabled_primary_ticks == 0 {
        let sockets = [
            point(art, EnemyArtAtlas::BadGuys, body_entry, 0),
            point(art, EnemyArtAtlas::BadGuys, body_entry, 1),
        ];
        for layer in
            mage_charge(element, actor.lighting.charge, age, sockets, context.application_tick)
        {
            body.push(layer.into_actor_space(actor.config.scale));
        }
    }
    let (head, hit_head) = compose_family(actor, age, vec![head], "mage", None, false, sample);
    body.extend(head);
    hit_body.extend(hit_head);
    EnemyFamilyPresentation { body, hit_body, ..Default::default() }
}

fn compose_family(
    actor: &impl EnemyActorRead,
    age: u64,
    source: Vec<EnemySpriteLayer>,
    family: &str,
    held: Option<EnemySpriteLayer>,
    include_fire: bool,
    sample: &NativeEnemyPresentationInput,
) -> (Vec<EnemySpriteLayer>, Vec<EnemySpriteLayer>) {
    let bob = float::sin(radians(sample.stride_phase_deg * 0.5)).abs();
    let body_height = stable_inclusive_unit(actor, 70, 0) * 3.0;
    let body_glow_tint = burning_tint(0.25 + stable_inclusive_unit(actor, 73, age as u32) * 0.75);
    let burning = actor.config.burning;
    let base_tint = native_skeleton_base_tint(actor, age, include_fire);
    let mut articulated = Vec::with_capacity(source.len());
    for mut layer in source {
        let y = if layer.role.ends_with("-limbs") {
            -bob
        } else if layer.role.ends_with("-body")
            || matches!(
                layer.role.as_str(),
                "skeleton-weapon" | "skeleton-mace-head" | "mage-cloak"
            )
        {
            -body_height - bob * 2.0
        } else if layer.role.ends_with("-headgear") {
            -body_height - 2.0 - bob * 3.0
        } else {
            0.0
        };
        layer.offset = Vec2::new(
            layer.offset.x / actor.config.scale,
            (layer.offset.y + y) / actor.config.scale,
        );
        if let Some(stretch) = &mut layer.stretch {
            stretch.start.x /= actor.config.scale;
            stretch.start.y /= actor.config.scale;
            stretch.end.x /= actor.config.scale;
            stretch.end.y /= actor.config.scale;
        }
        layer.tint = base_tint;
        articulated.push(layer);
    }
    let mut body = articulated.clone();
    if let Some(held) = held {
        let index =
            body.iter().position(|layer| layer.role.ends_with("-headgear")).unwrap_or(body.len());
        body.insert(index, held.into_actor_space(actor.config.scale));
    }
    if !burning {
        return (body, articulated);
    }
    let glow_tint = if include_fire {
        body_glow_tint
    } else {
        burning_tint(0.25 + stable_inclusive_unit(actor, 76, age as u32) * 0.75)
    };
    let flail_head = articulated.iter().position(|layer| layer.role == "skeleton-flail-head");
    if include_fire {
        let direction = radians(actor.heading_deg);
        let entry = 46 + (age / 2 % 32) as usize;
        for (part, offset, scale_x, scale_y) in [
            (
                "upper",
                Vec2::new(float::sin(direction) * 2.0, -40.0 - float::cos(direction) * 2.0),
                0.5,
                0.85,
            ),
            ("lower", Vec2::new(0.0, -20.0), 0.4, 0.75),
        ] {
            body.push(
                EnemySpriteLayer {
                    blend: EnemyArtBlend::Add,
                    offset,
                    scale_x: Some(scale_x),
                    scale_y: Some(scale_y),
                    tint: glow_tint,
                    ..EnemySpriteLayer::new(
                        EnemyArtAtlas::DeadHawg,
                        entry,
                        format!("{family}-burning-fire:{part}"),
                    )
                }
                .into_actor_space(actor.config.scale),
            );
        }
    }
    let mut hit_body = articulated.clone();
    for pass in 1..=2 {
        for (index, source) in articulated.iter().enumerate() {
            let mut glow = source.clone();
            glow.role = format!("{}:burn-glow-{pass}", source.role);
            glow.blend = EnemyArtBlend::Add;
            glow.tint = if flail_head.is_some_and(|head| pass == 2 || index >= head) {
                base_tint
            } else {
                glow_tint
            };
            body.push(glow.clone());
            hit_body.push(glow);
        }
    }
    (body, hit_body)
}

/// `nativeSkeletonBaseTint` is shared with EtherDrain's accepted head capture.
#[must_use]
pub fn native_skeleton_base_tint(actor: &impl EnemyActorRead, age: u64, include_fire: bool) -> u32 {
    if !include_fire && actor.lighting.charge > 0.0 {
        return 0xffffff;
    }
    if actor.config.burning {
        let green = if include_fire {
            stable_inclusive_unit(actor, 72, age as u32) * 0.5
        } else {
            0.25 + stable_inclusive_unit(actor, 73, age as u32) * 0.75
        };
        return burning_tint(green);
    }
    let neutral = 1.0 - stable_inclusive_unit(actor, 71, 0) * 0.15;
    pack_rgb(neutral, neutral, 1.0)
}

fn burning_tint(green: f64) -> u32 {
    let color = desaturate_color([1.0, fround(green), 0.0, 1.0], 0.5);
    pack_rgb(color[0], color[1], color[2])
}

fn mage_charge(
    element: MageElement,
    charge: f64,
    age: u64,
    points: [Vec2; 2],
    application_tick: u64,
) -> Vec<EnemySpriteLayer> {
    let strength = fround(fround(charge.clamp(0.0, 1.0)).powi(2));
    if strength == 0.0 {
        return Vec::new();
    }
    let sockets = points.map(|point| Vec2::new(point.x, point.y - 5.0));
    let mut layers = Vec::new();
    match element {
        MageElement::Fire | MageElement::Lightning => {
            let fire = element == MageElement::Fire;
            for (index, offset) in sockets.into_iter().enumerate() {
                layers.push(EnemySpriteLayer {
                    alpha: strength,
                    blend: EnemyArtBlend::Add,
                    offset,
                    scale: if fire { 1.25 } else { 0.5 },
                    ..EnemySpriteLayer::new(
                        EnemyArtAtlas::BadGuys,
                        if fire {
                            255 + (age / 5 % 12) as usize
                        } else {
                            1836 + ((application_tick / 8 + index as u64 * 10) % 4) as usize
                        },
                        format!(
                            "mage-{}-charge:{index}:sheet",
                            if fire { "fire" } else { "lightning" }
                        ),
                    )
                });
            }
            for (index, offset) in sockets.into_iter().enumerate() {
                layers.push(EnemySpriteLayer {
                    blend: EnemyArtBlend::Add,
                    offset,
                    scale: if fire { 1.0 } else { 0.75 },
                    tint: if fire { pack_rgb(1.0, 0.5, 0.0) } else { pack_rgb(0.75, 1.0, 1.0) },
                    ..EnemySpriteLayer::new(
                        EnemyArtAtlas::BadGuys,
                        70,
                        format!(
                            "mage-{}-charge:{index}:glow",
                            if fire { "fire" } else { "lightning" }
                        ),
                    )
                });
            }
        }
        MageElement::Frost | MageElement::Poison => {
            for (index, offset) in sockets.into_iter().enumerate() {
                let frost = element == MageElement::Frost;
                layers.push(EnemySpriteLayer {
                    alpha: strength,
                    blend: EnemyArtBlend::Add,
                    offset,
                    rotation_radians: radians(age as f64 * 2.0),
                    ..EnemySpriteLayer::new(
                        EnemyArtAtlas::BadGuys,
                        if frost { 381 } else { 382 },
                        format!("mage-{}-charge:{index}", if frost { "frost" } else { "poison" }),
                    )
                });
            }
        }
    }
    layers
}
