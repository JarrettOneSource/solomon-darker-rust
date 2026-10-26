//! Flat draw samples from `host/project-boneyard-enemies.ts::projectAnimation`.
//! The client interpolates these outputs, rather than advancing an enemy brain.

use super::layers::radians;
use super::model::*;
use crate::Vec2;
use crate::enemies::model::*;
use crate::float;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NativeEnemyAnimationState {
    #[default]
    Idle,
    Locomotion,
    Action,
    Death,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeEnemyAction {
    SkeletonClawA,
    SkeletonClawB,
    SkeletonWeapon,
    SkeletonPike,
    ArcherShot,
    MageCastShort,
    MageCastLong,
    ZombieBeat,
    WraithDrain,
    DemonBomb,
    Faculty(FacultyActionKind),
    DemonSkullBite,
    DemonSkullEyes,
    DemonSkullMouth,
    DemonSkullSpit,
    DemonSkullFlair,
    DemonSkullScream,
}

impl NativeEnemyAction {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::SkeletonClawA => "skeleton-claw-a",
            Self::SkeletonClawB => "skeleton-claw-b",
            Self::SkeletonWeapon => "skeleton-weapon",
            Self::SkeletonPike => "skeleton-pike",
            Self::ArcherShot => "archer-shot",
            Self::MageCastShort => "mage-cast-short",
            Self::MageCastLong => "mage-cast-long",
            Self::ZombieBeat => "zombie-beat",
            Self::WraithDrain => "wraith-drain",
            Self::DemonBomb => "demon-bomb",
            Self::Faculty(FacultyActionKind::Throw) => "faculty-throw",
            Self::Faculty(FacultyActionKind::TwoHand) => "faculty-two-hand",
            Self::Faculty(FacultyActionKind::Lightning) => "faculty-lightning",
            Self::DemonSkullBite => "demon-skull-bite",
            Self::DemonSkullEyes => "demon-skull-eyes",
            Self::DemonSkullMouth => "demon-skull-mouth",
            Self::DemonSkullSpit => "demon-skull-spit",
            Self::DemonSkullFlair => "demon-skull-flair",
            Self::DemonSkullScream => "demon-skull-scream",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NativeEnemyCoffinState {
    Hidden,
    #[default]
    Closed,
    Opening,
    Open,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeEnemySpiderInput {
    pub body_heading_deg: f64,
    pub outline_alpha: f64,
    pub outline_tint: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeEnemyEffectInput {
    pub id: usize,
    pub layer: EnemySpriteLayer,
}

/// Renderer-only animation values. Whole-tick authority remains in `EnemyActor`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NativeEnemyPresentationInput {
    pub presentation_tick: f64,
    pub state: NativeEnemyAnimationState,
    pub action: Option<NativeEnemyAction>,
    pub death_epoch: u64,
    pub action_progress: f64,
    pub alpha: f64,
    pub body_gait_phase: f64,
    pub body_pose: f64,
    pub coffin_pose: f64,
    pub coffin_rotation_radians: f64,
    pub coffin_scale_x: f64,
    pub coffin_state: NativeEnemyCoffinState,
    pub death_tick: f64,
    pub demon_front_extremity_offset: Vec2,
    pub demon_front_rotation_radians: f64,
    pub demon_rear_extremity_offset: Vec2,
    pub demon_rear_rotation_radians: f64,
    pub demon_shadow_offset: Vec2,
    pub effects: Vec<NativeEnemyEffectInput>,
    pub gait_pose: f64,
    pub hit_flash: f64,
    pub imp_body_rotation_radians: f64,
    pub imp_effect_alpha: f64,
    pub imp_effect_frame: f64,
    pub pike_target_offset: Option<Vec2>,
    pub shadow_lateral_offset: f64,
    pub spider: Option<NativeEnemySpiderInput>,
    pub stride_phase_deg: f64,
    pub vertical_offset: f64,
    pub zombie_angular_offset_deg: f64,
    pub zombie_front_arm_pose: f64,
    pub zombie_rear_arm_pose: f64,
    pub zombie_body_rotation_radians: f64,
    pub zombie_arm_socket_rotation_radians: f64,
    pub zombie_front_arm_rotation_radians: f64,
    pub zombie_rear_arm_rotation_radians: f64,
    pub zombie_head_rotation_radians: f64,
}

#[must_use]
pub fn native_enemy_presentation_input(
    actor: &impl EnemyActorRead,
    context: EnemyActorAppearanceContext,
) -> NativeEnemyPresentationInput {
    let action = if actor.life_state == EnemyLifeState::Alive { action(actor) } else { None };
    let state = if actor.life_state == EnemyLifeState::Dying {
        NativeEnemyAnimationState::Death
    } else if action.is_some() {
        NativeEnemyAnimationState::Action
    } else if actor.last_movement_tick.is_some_and(|tick| {
        context.tick.saturating_sub(tick) <= super::super::movement::MOVEMENT_CADENCE_TICKS
    }) {
        NativeEnemyAnimationState::Locomotion
    } else {
        NativeEnemyAnimationState::Idle
    };
    let mut sample = NativeEnemyPresentationInput {
        presentation_tick: context.tick as f64,
        state,
        action,
        death_epoch: actor.death_epoch.unwrap_or(0),
        alpha: 1.0,
        body_gait_phase: actor.body_gait_phase,
        body_pose: actor.body_pose,
        coffin_scale_x: 1.0,
        death_tick: actor.death_tick as f64,
        gait_pose: actor.gait_pose,
        hit_flash: actor.hit_feedback.alpha(context.tick as f64),
        imp_effect_frame: -1.0,
        shadow_lateral_offset: actor.shadow_lateral_offset,
        stride_phase_deg: actor.stride_phase_deg,
        ..Default::default()
    };
    match actor.brain() {
        EnemyBrain::Skeleton(brain) => {
            sample.action_progress = brain.action_progress;
            sample.vertical_offset = brain.vertical_offset;
            sample.pike_target_offset = brain.pike.as_ref().map(|pike| {
                let point = context.pike_target_position.unwrap_or(pike.position);
                Vec2::new(point.x - actor.position.x, point.y - actor.position.y)
            });
        }
        EnemyBrain::Archer(brain) => {
            sample.action_progress = brain.action_progress;
            sample.vertical_offset = brain.vertical_offset;
        }
        EnemyBrain::Mage(brain) => {
            sample.action_progress = brain.action_progress;
            sample.vertical_offset = brain.vertical_offset;
        }
        EnemyBrain::Imp(brain) => {
            sample.body_pose = brain.body_variant as f64;
            sample.imp_body_rotation_radians = radians(brain.body_rotation_deg);
            sample.imp_effect_alpha = brain.effect_alpha;
            sample.imp_effect_frame = brain.effect_phase.floor().clamp(0.0, 9.0);
            sample.vertical_offset = brain.vertical_offset;
        }
        EnemyBrain::Portal(brain) => {
            sample.alpha = brain.alpha;
            sample.hit_flash = (brain.hurt_ticks_remaining as f64 / 24.0).min(1.0);
        }
        EnemyBrain::Zombie(brain) => {
            sample.action_progress = brain.action_progress;
            sample.vertical_offset = brain.vertical_velocity;
            sample.zombie_angular_offset_deg = brain.angular_offset_deg;
            let active = brain.phase == ZombiePhase::Swipe;
            let pose = if brain.action_progress <= 50.0 {
                1.0
            } else if brain.action_progress < 100.0 {
                2.0
            } else {
                0.0
            };
            sample.zombie_front_arm_pose =
                if active && brain.attack_side == 1 { pose } else { 0.0 };
            sample.zombie_rear_arm_pose = if active && brain.attack_side == 0 { pose } else { 0.0 };
            let idle = (float::sin(radians(brain.body_phase_deg)) * 45.0 / 10.0).trunc() * 10.0;
            let head = (float::sin(radians(brain.head_phase_deg * 0.5)) * 20.0 / 5.0).trunc() * 5.0
                + brain.head_base_rotation_deg;
            let swing = if active { (brain.action_swing / 10.0).trunc() * 10.0 } else { 0.0 };
            let lean = if active { brain.action_swing / 3.0 } else { 0.0 };
            sample.zombie_body_rotation_radians = radians(idle);
            sample.zombie_arm_socket_rotation_radians =
                radians((idle + if brain.attack_side == 0 { -lean } else { lean }) * 0.5);
            sample.zombie_front_arm_rotation_radians = radians(
                brain.front_arm_base_rotation_deg
                    + if brain.attack_side == 1 { swing } else { 0.0 },
            );
            sample.zombie_rear_arm_rotation_radians = radians(
                -brain.rear_arm_base_rotation_deg
                    - if brain.attack_side == 0 { swing } else { 0.0 },
            );
            sample.zombie_head_rotation_radians = radians(head);
        }
        EnemyBrain::Wraith(brain) => {
            sample.action_progress = if brain.contact_cooldown_ticks == 0 {
                0.0
            } else {
                50.0 - brain.contact_cooldown_ticks as f64
            };
        }
        EnemyBrain::Demon(brain) => {
            let pose = super::demon::articulation(actor, context.tick);
            sample.action_progress = brain.action_progress;
            sample.body_pose = pose.controller_pose as f64;
            sample.demon_front_extremity_offset = pose.front;
            sample.demon_rear_extremity_offset = pose.rear;
            sample.demon_front_rotation_radians = pose.front_rotation;
            sample.demon_rear_rotation_radians = pose.rear_rotation;
            sample.demon_shadow_offset = Vec2::new(
                (brain.articulation.front.current.x + brain.articulation.rear.current.x) * 0.5
                    - actor.position.x,
                (brain.articulation.front.current.y + brain.articulation.rear.current.y) * 0.5
                    - actor.position.y,
            );
            sample.vertical_offset =
                if actor.life_state == EnemyLifeState::Dying { 0.0 } else { pose.vertical_offset };
        }
        EnemyBrain::Coffin(brain) => {
            let (pose, state, vertical) = match brain.phase {
                CoffinPhase::Hidden => (0.0, NativeEnemyCoffinState::Hidden, 15.0),
                CoffinPhase::Rising => (
                    (brain.phase_tick as f64 * 0.3).min(3.0),
                    NativeEnemyCoffinState::Closed,
                    15.0 * brain.phase_ticks_remaining as f64 / 10.0,
                ),
                CoffinPhase::Holding => (3.0, NativeEnemyCoffinState::Closed, 0.0),
                CoffinPhase::Opening => (
                    (3.0 + brain.phase_tick as f64 * 0.2).min(12.0),
                    NativeEnemyCoffinState::Opening,
                    0.0,
                ),
                CoffinPhase::Open | CoffinPhase::Death => (12.0, NativeEnemyCoffinState::Open, 0.0),
            };
            sample.coffin_pose = pose;
            sample.coffin_state = state;
            sample.coffin_rotation_radians = radians(brain.launch_rotation_deg);
            sample.coffin_scale_x = f64::from(brain.launch_scale);
            sample.vertical_offset = vertical;
            sample.action_progress = if brain.phase == CoffinPhase::Opening { pose } else { 0.0 };
        }
        EnemyBrain::Spider(brain) => {
            let distance = context.spider_target.map_or(f64::INFINITY, |target| {
                crate::float::hypot(
                    target.position.x - actor.position.x,
                    target.position.y - actor.position.y,
                )
            });
            sample.spider = Some(NativeEnemySpiderInput {
                body_heading_deg: ((brain.body_heading_deg % 360.0) + 360.0) % 360.0,
                outline_alpha: if context.spider_target.is_some()
                    && context.spider_exposure > 0.5
                    && distance < 175.0
                {
                    1.0 - (distance - 125.0).max(0.0) / 50.0
                } else {
                    0.0
                },
                outline_tint: context.spider_target.map_or(0, |target| target.primary_tint),
            });
            sample.vertical_offset = brain.vertical_offset;
        }
        EnemyBrain::Faculty(brain) => {
            sample.action_progress = brain.action.as_ref().map_or(0.0, |action| action.progress);
            sample.gait_pose = brain.gait_phase;
        }
        EnemyBrain::Heartmonger(brain) => sample.gait_pose = brain.leg_phase.trunc(),
        EnemyBrain::DemonSkull(brain) => {
            if let Some(DemonSkullAction {
                program: DemonSkullActionProgram::Bite { progress, .. },
                ..
            }) = brain.actions.first()
            {
                sample.action_progress = *progress;
            }
        }
        EnemyBrain::Cocoon(_) => {}
    }
    if actor.life_state == EnemyLifeState::Alive && actor.shield_health > 0.0 {
        let wobble = actor.shield_pulse.min(1.0);
        sample.effects.push(NativeEnemyEffectInput {
            id: actor.id * 4,
            layer: EnemySpriteLayer {
                alpha: 0.5 * (actor.shield_pulse.max(1.0) - 1.0) + 0.25,
                blend: EnemyArtBlend::Add,
                offset: Vec2::new(0.0, -30.0),
                scale: 1.5 + 0.1 * float::sin(radians(context.tick as f64 * 20.0)) * wobble,
                ..EnemySpriteLayer::new(
                    EnemyArtAtlas::BadGuys,
                    49,
                    format!("effect:{}:magic-shield", actor.id * 4),
                )
            },
        });
    }
    sample
}

fn action(actor: &impl EnemyActorRead) -> Option<NativeEnemyAction> {
    use NativeEnemyAction as Action;
    match actor.brain() {
        EnemyBrain::Skeleton(brain) if brain.phase == SkeletonPhase::Attack => {
            Some(match brain.action {
                SkeletonAction::Pike => Action::SkeletonPike,
                SkeletonAction::Weapon => Action::SkeletonWeapon,
                _ if matches!(
                    actor.config.family,
                    crate::enemies::config::EnemyFamilyConfig::Skeleton { armor: true, .. }
                ) =>
                {
                    Action::SkeletonClawB
                }
                _ => Action::SkeletonClawA,
            })
        }
        EnemyBrain::Archer(brain) if brain.phase == ArcherPhase::Attack => Some(Action::ArcherShot),
        EnemyBrain::Mage(brain) if brain.phase == MagePhase::Cast => {
            Some(if brain.cast_program == MageCastProgram::Long {
                Action::MageCastLong
            } else {
                Action::MageCastShort
            })
        }
        EnemyBrain::Zombie(brain) if brain.phase == ZombiePhase::Swipe => Some(Action::ZombieBeat),
        EnemyBrain::Wraith(brain) if brain.contact_cooldown_ticks > 0 => Some(Action::WraithDrain),
        EnemyBrain::Demon(brain) if brain.phase == DemonPhase::Bomb => Some(Action::DemonBomb),
        EnemyBrain::Faculty(brain) => {
            brain.action.as_ref().map(|action| Action::Faculty(action.kind))
        }
        EnemyBrain::DemonSkull(brain) => brain.actions.first().map(|action| match action.program {
            DemonSkullActionProgram::Bite { .. } => Action::DemonSkullBite,
            DemonSkullActionProgram::Eyes { .. } => Action::DemonSkullEyes,
            DemonSkullActionProgram::Mouth { .. } => Action::DemonSkullMouth,
            DemonSkullActionProgram::Spit { .. } => Action::DemonSkullSpit,
            DemonSkullActionProgram::Flair { .. } => Action::DemonSkullFlair,
            DemonSkullActionProgram::Scream => Action::DemonSkullScream,
        }),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeMaggotState {
    Bite,
    Death,
    Emerging,
    Crawl,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeMaggotPresentationInput {
    pub alpha: f64,
    pub current_health: f64,
    pub death_tick: f64,
    pub emergence_tick: f64,
    pub emergence_phase: f64,
    pub heading_deg: f64,
    pub hit_flash: f64,
    pub pose: f64,
    pub position: Vec2,
    pub state: NativeMaggotState,
    pub vertical_offset: f64,
    pub visual_scale: f64,
}

#[must_use]
pub fn native_maggot_presentation_input(
    maggot: &EnemyMaggotActor,
    tick: u64,
) -> NativeMaggotPresentationInput {
    let state = if maggot.last_attack_tick.is_some_and(|attack| tick.saturating_sub(attack) < 6) {
        NativeMaggotState::Bite
    } else if maggot.life_state == EnemyLifeState::Dying {
        NativeMaggotState::Death
    } else if maggot.movement_phase == MaggotMovementPhase::Emerging {
        NativeMaggotState::Emerging
    } else {
        NativeMaggotState::Crawl
    };
    NativeMaggotPresentationInput {
        alpha: 1.0,
        current_health: maggot.current_health,
        death_tick: maggot.death_tick as f64,
        emergence_tick: maggot.emergence_tick as f64,
        emergence_phase: maggot.emergence_phase,
        heading_deg: maggot.heading_deg,
        hit_flash: maggot.hit_feedback.alpha(tick as f64),
        pose: maggot.gait_pose,
        position: maggot.position,
        state,
        vertical_offset: maggot.vertical_offset,
        visual_scale: maggot.visual_scale,
    }
}
