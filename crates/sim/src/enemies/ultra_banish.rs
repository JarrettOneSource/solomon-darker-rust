//! Discorporeal terminal spell lifetimes from `enemies/ultra-banish.ts`.

use super::boss_spell::{BossSpellKind, NativeBossSpell};
use super::demon_skull_effects::UNHOLY_GREEN;
use super::effects::{
    BouncerOptions, DeathEffectOverrides, DeathEffectOwner, SimpleDeathEffectSpec, radial_vector,
};
use super::model::*;
use super::world::{EnemyStepContext, EnemyWorld};
use crate::Vec2;
use crate::float;
use crate::float::fround;
use crate::painter::WorldPainterOrder;

pub fn step_ultra_banish_spell(
    world: &mut EnemyWorld,
    mut spell: NativeBossSpell,
    context: &EnemyStepContext<'_>,
    has_long_ultra_banish: bool,
    order: &mut WorldPainterOrder,
) -> Option<NativeBossSpell> {
    spell.owner.age_ticks += 1;
    let owner = DeathEffectOwner { id: spell.owner.owner_actor_id, position: spell.owner.position };
    match &mut spell.kind {
        BossSpellKind::UnholySoul {
            origin,
            angle_deg,
            angular_speed,
            scale,
            height,
            vertical_speed,
            life,
            fast,
        } => {
            *scale = fround(*scale + fround(0.01)).min(1.0);
            *angle_deg = fround(*angle_deg - *angular_speed);
            *height = fround(*height - *vertical_speed);
            *life = fround(*life - fround(0.01));
            world.steering_rng.draw_scaled_float(1.0, false);
            if *fast {
                world.steering_rng.draw_scaled_float(2.0, false);
                *angle_deg = fround(*angle_deg - *angular_speed);
            }
            if *angle_deg < 0.0 {
                *angle_deg = fround(*angle_deg + 360.0);
            }
            if *life <= 0.0 {
                return None;
            }
            let angle = *angle_deg * std::f64::consts::PI / 180.0;
            spell.owner.position = Vec2::new(
                fround(origin.x + float::sin(angle) * 100.0),
                fround(origin.y - float::cos(angle) * 100.0),
            );
            if !has_long_ultra_banish {
                *life = fround(life.min(fround(1.1)) - fround(0.01));
                *angular_speed = fround(*angular_speed * fround(1.05));
                *vertical_speed = fround(*vertical_speed * fround(1.05));
            }
            Some(spell)
        }
        BossSpellKind::UltraBanish {
            remaining_ticks,
            alpha,
            flash_alpha,
            light_radius,
            mega_death,
        } => {
            *light_radius = fround(1.0 + world.steering_rng.draw_scaled_float(1.0, false));
            let displacement = world.random_enemy_offset(15.0);
            let mut event =
                EnemyEvent::new(EnemyEventKind::EnemyCameraShake, owner.id, context.tick);
            event.source_position = Some(owner.position);
            event.camera_shake =
                Some(EnemyCameraShake { attenuation: CameraShakeAttenuation::Fixed, displacement });
            world.emit_event(event);
            if remaining_ticks.is_multiple_of(25) || world.steering_rng.draw_integer(40) == 3 {
                let mut event =
                    EnemyEvent::new(EnemyEventKind::EnemyScreenFlash, owner.id, context.tick);
                event.source_position = Some(owner.position);
                event.screen_flash_only_if_clear = Some(true);
                event.screen_flash = Some(EnemyScreenFlash {
                    red: 0.25,
                    green: 1.0,
                    blue: 0.25,
                    alpha: *alpha * 0.5,
                    decay_per_tick: fround(0.1),
                    point_attenuated: false,
                });
                world.emit_event(event);
                let multiplier = fround(
                    fround(1.045) + world.steering_rng.draw_scaled_float(fround(0.025), false),
                );
                world.spawn_simple_death_effect(
                    owner,
                    context.tick,
                    SimpleDeathEffectSpec {
                        alpha: 1.0,
                        alpha_loss_per_tick: super::effects::NATIVE_BANISH_RING_ALPHA_LOSS,
                        atlas: EnemyEffectAtlas::DeadHawg,
                        entry: 16,
                        blend_mode: EnemyEffectBlend::Add,
                        kind: EnemyDeathEffectKind::FadeScalePerspective,
                        lifetime_ticks: 1000,
                        role: "ultra-banish-ring",
                        scale: 2.0,
                    },
                    DeathEffectOverrides {
                        opacity_timer: Some(3.0),
                        presentation_owner: Some(DeathPresentationOwner::PreWorldQueue),
                        scale_multiplier: Some(multiplier),
                        tint: Some(UNHOLY_GREEN),
                        ..Default::default()
                    },
                    order,
                );
                // The random gate runs before the mega-death check in the source.
                if *remaining_ticks > 100 && world.steering_rng.draw_integer(2) == 1 && *mega_death
                {
                    let angle_deg = world.steering_rng.draw_scaled_float(360.0, false);
                    let angular_speed =
                        fround(3.0 + world.steering_rng.draw_scaled_float(3.0, false));
                    let vertical_speed =
                        fround((2.0 + world.steering_rng.draw_scaled_float(3.0, false)) * 1.25);
                    let child_owner = world.create_boss_spell_owner(
                        owner.id,
                        context.tick,
                        0.0,
                        Vec2::ZERO,
                        crate::painter::WorldManagerLane::Transient,
                        order,
                    );
                    let fast = world.steering_rng.draw_integer(50) == 1;
                    world.boss_spells.push(NativeBossSpell {
                        owner: child_owner,
                        kind: BossSpellKind::UnholySoul {
                            origin: owner.position,
                            angle_deg,
                            angular_speed,
                            height: 0.0,
                            vertical_speed,
                            scale: 0.0,
                            life: 10.0,
                            fast,
                        },
                    });
                }
            }
            *remaining_ticks -= 1;
            if *remaining_ticks <= 100 {
                *alpha = fround(*alpha - fround(0.01));
            }
            if *alpha <= 0.0 {
                return None;
            }
            if *mega_death
                && *remaining_ticks > 100
                && (context.enhanced_effects || remaining_ticks.is_multiple_of(3))
            {
                world.spawn_bouncer(
                    owner,
                    context.tick,
                    |world| 1819 + world.steering_rng.draw_integer(4) as usize,
                    "ultra-banish-bone",
                    |world| {
                        let speed = fround(2.0 + world.steering_rng.draw_scaled_float(0.25, false));
                        let heading = world.steering_rng.draw_scaled_float(360.0, false);
                        let velocity = radial_vector(
                            heading,
                            speed
                                * if world.steering_rng.draw_integer(20) == 3 { 1.5 } else { 1.0 },
                        );
                        BouncerOptions {
                            bounce_velocity_multiplier: Some(4.0),
                            opacity_timer: Some(20.0),
                            presentation_owner: Some(DeathPresentationOwner::Background),
                            velocity: Some(velocity),
                            ..Default::default()
                        }
                    },
                    context.enhanced_effects,
                    order,
                );
            }
            *flash_alpha = fround(*flash_alpha - fround(0.05)).max(0.0);
            if *flash_alpha <= 0.0 && world.steering_rng.draw_integer(1000) == 3 {
                *flash_alpha = 1.0;
            }
            Some(spell)
        }
        _ => panic!("UltraBanish spell required"),
    }
}
