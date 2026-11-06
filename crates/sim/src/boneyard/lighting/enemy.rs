//! Light providers for enemies, enemy projectiles and enemy projectile effects. The
//! recorded `entity-light-goldens.json` fixture pins which lights each entity submits and
//! their presentation random draws.

use crate::Vec2;
use crate::enemies::EnemyToken;
use crate::enemies::model::EnemyActor;
use crate::enemies::model::{EnemyBrain, EnemyLifeState, EnemyLighting};
use crate::enemies::projectiles::{EnemyProjectileKind, EnemyProjectilePayload};
use crate::float::fround;
use crate::painter::WorldManagerLane;
use crate::random_domain::random_int_from_word;

use super::{
    BoneyardLightSource, fire_patch_light_source, missile_light_source, presentation_random,
    presentation_random_word, presentation_signed_random,
};

/// The enemy values its lights read, copied from an `EnemyActor`.
#[derive(Clone, Copy, Debug)]
pub struct EnemyLightOwner {
    pub burning: bool,
    pub scale: f64,
    pub demon_skull_body_offset: Vec2,
    pub id: usize,
    pub enemy_token: EnemyToken,
    pub position: Vec2,
    pub dying: bool,
    pub alpha: f64,
    pub body_pose: f64,
    pub lighting: EnemyLighting,
}

impl From<&EnemyActor> for EnemyLightOwner {
    fn from(actor: &EnemyActor) -> Self {
        Self {
            burning: actor.config.burning,
            scale: actor.config.scale,
            demon_skull_body_offset: match &actor.brain {
                EnemyBrain::DemonSkull(brain) => brain.body_offset,
                _ => Vec2::ZERO,
            },
            id: actor.id,
            enemy_token: actor.config.enemy_token,
            position: actor.position,
            dying: actor.life_state == EnemyLifeState::Dying,
            alpha: match &actor.brain {
                EnemyBrain::Portal(brain) => brain.alpha,
                _ => 1.0,
            },
            body_pose: actor.body_pose,
            lighting: actor.lighting,
        }
    }
}

/// A light source and the world-manager lane (actor or transient) that submits it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoneyardLightProviderCandidate {
    pub lane: WorldManagerLane,
    pub source: BoneyardLightSource,
}

#[must_use]
pub fn enemy_light_sources(
    enemy: EnemyLightOwner,
    frame: u64,
    multiple_shadows: bool,
) -> Vec<BoneyardLightSource> {
    if enemy.enemy_token == EnemyToken::Zombie || enemy.lighting.provider_copies == 0 {
        return Vec::new();
    }
    if enemy.enemy_token == EnemyToken::SkeletonArcher && !enemy.burning && enemy.body_pose == 9.0 {
        return Vec::new();
    }
    let mut sources = Vec::with_capacity(enemy.lighting.provider_copies as usize);
    for copy in 0..enemy.lighting.provider_copies {
        let salt = (u32::from(copy) + 1).wrapping_mul(0x045d_9f3b) ^ enemy.id as u32;
        let source = match enemy.enemy_token {
            EnemyToken::DemonSkull => BoneyardLightSource::new(
                Vec2::new(
                    enemy.position.x + enemy.demon_skull_body_offset.x,
                    enemy.position.y + enemy.demon_skull_body_offset.y,
                ),
                enemy.scale * 2.0,
                enemy.lighting.glow,
                multiple_shadows,
            ),
            EnemyToken::DireFaculty => BoneyardLightSource::new(
                enemy.position,
                fround(0.75 + presentation_signed_random(frame, salt ^ 0x479fa0, 0.1_f32 as f64)),
                if presentation_random(frame, salt ^ 0x479f80, 1.0) < 0.5 {
                    0.0
                } else {
                    enemy.lighting.glow
                },
                multiple_shadows,
            ),
            EnemyToken::Heartmonger => BoneyardLightSource::new(
                enemy.position,
                0.75,
                fround(0.5 + presentation_random(frame, salt ^ 0x47a040, 0.100_000_023_841_857_91)),
                multiple_shadows,
            ),
            EnemyToken::Skeleton => ordinary_skeleton_light(enemy, frame, salt, multiple_shadows),
            EnemyToken::SkeletonArcher | EnemyToken::SkeletonMage => {
                if enemy.burning {
                    ordinary_skeleton_light(enemy, frame, salt, multiple_shadows)
                } else {
                    BoneyardLightSource::new(
                        enemy.position,
                        fround(
                            fround(enemy.lighting.charge)
                                * fround(
                                    0.5 + presentation_signed_random(frame, salt ^ 0x4783e0, 0.1),
                                ),
                        ),
                        fround(0.75 * fround(enemy.lighting.charge)),
                        multiple_shadows,
                    )
                }
            }
            EnemyToken::Imp => BoneyardLightSource::new(
                enemy.position,
                fround(0.25 + presentation_signed_random(frame, salt ^ 0x478cc1, 0.1)),
                fround(
                    fround(enemy.lighting.glow)
                        * fround(0.75 + presentation_random(frame, salt ^ 0x478cc0, 0.25)),
                ),
                false,
            ),
            EnemyToken::Wraith => BoneyardLightSource::new(
                enemy.position,
                0.5,
                fround(
                    fround(enemy.lighting.glow)
                        * fround(0.5 + presentation_random(frame, salt ^ 0x478e00, 0.5)),
                ),
                multiple_shadows,
            ),
            EnemyToken::Demon => BoneyardLightSource::new(
                enemy.position,
                fround(1.5 + presentation_signed_random(frame, salt ^ 0x479471, 0.25)),
                if enemy.dying {
                    fround(0.5 + presentation_random(frame, salt ^ 0x479470, 0.5))
                } else {
                    1.0
                },
                multiple_shadows,
            ),
            EnemyToken::Coffin => BoneyardLightSource::new(
                enemy.position,
                0.65,
                fround(
                    1.0 - f64::from(random_int_from_word(
                        presentation_random_word(frame, salt ^ 0x479ea0),
                        9,
                    )) * 0.1,
                ),
                multiple_shadows,
            ),
            EnemyToken::Portal => BoneyardLightSource::new(
                enemy.position,
                fround(
                    fround(enemy.alpha)
                        * fround(
                            0.899_999_976_158_142_1
                                + presentation_random(
                                    frame,
                                    salt ^ 0x47bed0,
                                    0.350_000_023_841_857_9,
                                ),
                        ),
                ),
                fround(enemy.alpha),
                multiple_shadows,
            ),
            EnemyToken::Spider | EnemyToken::Cocoon | EnemyToken::Zombie => continue,
        };
        sources.push(source);
    }
    sources
}

fn ordinary_skeleton_light(
    enemy: EnemyLightOwner,
    frame: u64,
    salt: u32,
    multiple_shadows: bool,
) -> BoneyardLightSource {
    BoneyardLightSource::new(
        enemy.position,
        0.5,
        fround(
            fround(enemy.lighting.glow)
                * fround(0.5 + presentation_random(frame, salt ^ 0x4779e0, 0.5)),
        ),
        multiple_shadows,
    )
}

#[must_use]
pub fn enemy_projectile_light_provider(
    kind: EnemyProjectileKind,
    payload: EnemyProjectilePayload,
    id: usize,
    position: Vec2,
    frame: u64,
    multiple_shadows: bool,
) -> Option<BoneyardLightProviderCandidate> {
    let (lane, source) = match kind {
        EnemyProjectileKind::Arrow => {
            if payload != EnemyProjectilePayload::Fire {
                return None;
            }
            (WorldManagerLane::Transient, fire_projectile_light(id, position, frame))
        }
        EnemyProjectileKind::Firebolt => {
            (WorldManagerLane::Transient, fire_projectile_light(id, position, frame))
        }
        EnemyProjectileKind::GuidedMissile => (
            WorldManagerLane::Actor,
            missile_light_source(id as u32, position, frame, multiple_shadows),
        ),
        EnemyProjectileKind::DemonBomb => (
            WorldManagerLane::Actor,
            BoneyardLightSource::new(
                position,
                0.6,
                fround(1.0 - presentation_random(frame, id as u32 ^ 0x5e98e0, 0.25)),
                false,
            ),
        ),
        EnemyProjectileKind::PoisonPool => return None,
    };
    Some(BoneyardLightProviderCandidate { lane, source })
}

fn fire_projectile_light(id: usize, position: Vec2, frame: u64) -> BoneyardLightSource {
    BoneyardLightSource::new(
        position,
        fround(0.5 + presentation_signed_random(frame, id as u32 ^ 0x5e6140, 0.25)),
        0.85,
        false,
    )
}

/// The light an enemy projectile effect gives off; `Unlit` effects give none.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnemyProjectileEffectLightKind {
    DemonFire,
    DemonExplosionLitArray,
    FireBurst,
    GuidedImpact,
    Unlit,
}

#[must_use]
pub fn enemy_projectile_effect_light_provider(
    kind: EnemyProjectileEffectLightKind,
    age_ticks: u64,
    alpha: f64,
    position: Vec2,
    has_light_registration: bool,
    multiple_shadows: bool,
    point_gain: f64,
) -> Option<BoneyardLightProviderCandidate> {
    let (lane, source) = match kind {
        EnemyProjectileEffectLightKind::DemonFire => {
            if age_ticks == 0 || !has_light_registration {
                return None;
            }
            (WorldManagerLane::Actor, fire_patch_light_source(position, alpha, multiple_shadows))
        }
        EnemyProjectileEffectLightKind::DemonExplosionLitArray => (
            WorldManagerLane::Transient,
            BoneyardLightSource::new(position, fround(2.0 * point_gain), 1.0, multiple_shadows),
        ),
        EnemyProjectileEffectLightKind::FireBurst => (
            WorldManagerLane::Transient,
            BoneyardLightSource::new(
                position,
                1.5,
                (1.0 - (0.04_f32 as f64) * age_ticks as f64).max(0.0),
                false,
            ),
        ),
        EnemyProjectileEffectLightKind::GuidedImpact => (
            WorldManagerLane::Transient,
            BoneyardLightSource::new(
                position,
                0.75,
                (1.0 - (0.05_f32 as f64) * age_ticks as f64).max(0.0),
                false,
            ),
        ),
        EnemyProjectileEffectLightKind::Unlit => return None,
    };
    Some(BoneyardLightProviderCandidate { lane, source })
}
