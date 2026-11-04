//! Caster disable calls and continuing hand smoke from `enemies/dampen.ts`.

use super::effects::{DeathEffectOverrides, DeathEffectOwner, SimpleDeathEffectSpec};
use super::mage_attachments::{MAGE_LIGHTNING_ATTACHMENTS, MAGE_SECOND_HAND_ATTACHMENTS};
use super::model::*;
use super::world::EnemyWorld;
use crate::Vec2;
use crate::float;
use crate::float::fround;
use crate::painter::WorldPainterOrder;

impl EnemyWorld {
    /// Applies Dampen's selected enemy IDs in actor order. Its visual births consume
    /// the same steering stream as the native call, before the next actor epoch.
    pub fn dampen_casters(
        &mut self,
        target_ids: &[usize],
        tick: u64,
        order: Option<&mut WorldPainterOrder>,
    ) {
        if target_ids.is_empty() {
            return;
        }
        let mut standalone_order;
        let order = if let Some(order) = order {
            order
        } else {
            standalone_order = self.standalone_painter_order();
            &mut standalone_order
        };
        for index in 0..self.actors.len() {
            let mut actor = self.actors[index].clone();
            if !target_ids.contains(&actor.id)
                || actor.life_state != EnemyLifeState::Alive
                || !matches!(actor.brain, EnemyBrain::Mage(_) | EnemyBrain::Faculty(_))
            {
                continue;
            }
            let owner = DeathEffectOwner::from(&actor);
            self.spawn_simple_death_effect(
                owner,
                tick,
                SimpleDeathEffectSpec {
                    alpha: 1.0,
                    alpha_loss_per_tick: fround(0.02 * 0.25),
                    atlas: EnemyEffectAtlas::BadGuys,
                    blend_mode: EnemyEffectBlend::Normal,
                    entry: 15,
                    kind: EnemyDeathEffectKind::Fade,
                    lifetime_ticks: 201,
                    role: "dampen-caster-flash",
                    scale: 2.0,
                },
                DeathEffectOverrides {
                    presentation_owner: Some(DeathPresentationOwner::LateWorldOverlay),
                    ..Default::default()
                },
                order,
            );
            for _ in 0..72 {
                let entry = 10 + self.steering_rng.draw_integer(2) as usize;
                let speed = self.steering_rng.draw_scaled_float(3.0, false);
                let angle = self.steering_rng.draw_scaled_float(360.0, false)
                    * std::f64::consts::PI
                    / 180.0;
                let damping =
                    fround(if self.steering_rng.draw_integer(6) == 3 { 0.93 } else { 0.96 });
                let rotation = self.steering_rng.draw_scaled_float(360.0, false);
                let scale = fround(1.5 + self.steering_rng.draw_scaled_float(0.5, false));
                let loss =
                    fround(fround(0.01) + self.steering_rng.draw_scaled_float(fround(0.02), false));
                let tint = (self.steering_rng.draw_scaled_float(0.25, false) * 255.0).trunc()
                    as u32
                    * 0x010101;
                let presentation = if self.steering_rng.draw_integer(5) == 3 {
                    DeathPresentationOwner::WorldSorted
                } else {
                    DeathPresentationOwner::PreWorldQueue
                };
                self.spawn_simple_death_effect(
                    owner,
                    tick,
                    SimpleDeathEffectSpec {
                        alpha: 1.0,
                        alpha_loss_per_tick: loss,
                        atlas: EnemyEffectAtlas::BadGuys,
                        blend_mode: EnemyEffectBlend::Normal,
                        entry,
                        kind: EnemyDeathEffectKind::MoveFade,
                        lifetime_ticks: 101,
                        role: "dampen-caster-smoke",
                        scale,
                    },
                    DeathEffectOverrides {
                        presentation_owner: Some(presentation),
                        rotation_deg: Some(rotation),
                        tint: Some(tint),
                        velocity: Some(Vec2::new(
                            fround(float::sin(angle) * speed),
                            fround(-float::cos(angle) * speed),
                        )),
                        velocity_damping: Some(damping),
                        ..Default::default()
                    },
                    order,
                );
            }
            match &mut actor.brain {
                EnemyBrain::Faculty(brain) => {
                    actor.body_pose = 0.0;
                    brain.action = None;
                    brain.body_pose = 0.0;
                    brain.disabled_primary_ticks = 500;
                    brain.hand_mask = 0;
                    brain.lightning_active = false;
                    brain.phase = BossPhase::RangeControl;
                }
                EnemyBrain::Mage(brain) => {
                    actor.head_facing_offset = 0;
                    brain.action_progress = 0.0;
                    brain.disabled_primary_ticks = 600;
                    brain.marker_emitted = false;
                    brain.phase = MagePhase::RangeControl;
                }
                _ => unreachable!("Dampen admits only caster rows"),
            }
            self.actors[index] = actor;
        }
    }
}

pub fn spawn_dampened_mage_smoke(
    world: &mut EnemyWorld,
    actor: &EnemyActor,
    tick: u64,
    order: &mut WorldPainterOrder,
) {
    let EnemyBrain::Mage(brain) = &actor.brain else { return };
    if brain.disabled_primary_ticks == 0
        || actor.life_state != EnemyLifeState::Alive
        || actor.config.base_speed * actor.staff_movement_factor == 0.0
    {
        return;
    }
    let pose = if brain.phase == MagePhase::Cast {
        super::mage::mage_body_pose(brain.cast_program, brain.action_progress)
    } else {
        actor.body_pose
    };
    let pose = pose.floor().clamp(0.0, 4.0) as usize;
    let facing = (((actor.heading_deg + 10.0) / 20.0).trunc() as i64).rem_euclid(18) as usize;
    for slot in 0..2 {
        if world.steering_rng.draw_integer(5) != 1 {
            continue;
        }
        let entry = if world.steering_rng.draw_integer(2) == 1 { 10 } else { 11 };
        let scale = fround(
            fround(0.45)
                + world.steering_rng.draw_scaled_float(fround(fround(0.65) - fround(0.45)), false),
        );
        let tint = (world.steering_rng.draw_scaled_float(fround(0.1), false) * 255.0).trunc()
            as u32
            * 0x010101;
        let rotation = world.steering_rng.draw_scaled_float(360.0, false);
        let attachment = if slot == 0 {
            MAGE_LIGHTNING_ATTACHMENTS[pose][facing]
        } else {
            MAGE_SECOND_HAND_ATTACHMENTS[pose][facing]
        };
        let radius = world.steering_rng.draw_scaled_float(5.0, false);
        let angle =
            world.steering_rng.draw_scaled_float(360.0, false) * std::f64::consts::PI / 180.0;
        let alpha = fround(0.25 + world.steering_rng.draw_scaled_float(0.75, false));
        world.spawn_simple_death_effect(
            DeathEffectOwner::from(actor),
            tick,
            SimpleDeathEffectSpec {
                alpha,
                alpha_loss_per_tick: fround(fround(0.02) * fround(0.1)),
                atlas: EnemyEffectAtlas::BadGuys,
                blend_mode: EnemyEffectBlend::Normal,
                entry,
                kind: EnemyDeathEffectKind::Fade,
                lifetime_ticks: 501,
                role: &format!("dampened-mage-hand-{slot}"),
                scale,
            },
            DeathEffectOverrides {
                painter_sort_bias: Some(0.0),
                position: Some(Vec2::new(
                    fround(fround(actor.position.x + attachment.x) + float::sin(angle) * radius),
                    fround(
                        fround(actor.position.y + attachment.y - 5.0) - float::cos(angle) * radius,
                    ),
                )),
                rotation_deg: Some(rotation),
                tint: Some(tint),
                ..Default::default()
            },
            order,
        );
    }
}
