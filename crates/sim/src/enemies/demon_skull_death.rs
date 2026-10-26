//! Discorporeal death stream and banishment birth from `enemies/demon-skull-death.ts`.

use super::boss_spell::BossSpellKind;
use super::demon_skull_controller::NATIVE_DEMON_SKULL_DEATH_STREAM_TICKS;
use super::demon_skull_effects::UNHOLY_GREEN;
use super::effects::*;
use super::model::*;
use super::movement::positive_modulo;
use super::world::EnemyWorld;
use crate::Vec2;
use crate::float::fround;
use crate::painter::WorldPainterOrder;

pub fn step_dying_demon_skull(
    world: &mut EnemyWorld,
    actor: &mut EnemyActor,
    tick: u64,
    order: &mut WorldPainterOrder,
) {
    let EnemyBrain::DemonSkull(brain) = &actor.brain else {
        panic!("dying DemonSkull requires its brain")
    };
    let remaining = brain.death_countdown;
    let capabilities = brain.capabilities;
    let owner = DeathEffectOwner::from(&*actor);
    if actor.death_tick == 0 {
        world.demon_skull_encounter.death_stream_ticks_remaining =
            NATIVE_DEMON_SKULL_DEATH_STREAM_TICKS;
        let mut event = EnemyEvent::new(EnemyEventKind::EnemyStream, actor.id, tick);
        event.stream = Some("unholy-die".to_owned());
        world.emit_event(event);
        world.emit_action_sound(owner, tick, EnemySound::LightningStart, 0.5);
        world.emit_action_sound(owner, tick, EnemySound::LightningStart, 1.0);
        world.emit_action_sound(owner, tick, EnemySound::MagicShieldExplode, 1.0);
        flash(world, actor, tick, 1.0, fround(0.05));
        for multiplier in [fround(1.025), fround(1.035)] {
            world.spawn_simple_death_effect(
                owner,
                tick,
                SimpleDeathEffectSpec {
                    alpha: 1.0,
                    alpha_loss_per_tick: super::effects::NATIVE_BANISH_RING_ALPHA_LOSS,
                    atlas: EnemyEffectAtlas::DeadHawg,
                    entry: 16,
                    blend_mode: EnemyEffectBlend::Add,
                    kind: EnemyDeathEffectKind::FadeScalePerspective,
                    lifetime_ticks: 1000,
                    role: "discorporeal-death-ring",
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
        }
    }
    let body_pose = if remaining <= 600 { 2.0 } else { 1.0 };
    if remaining == 600 {
        flash(world, actor, tick, 1.0, fround(0.05));
    }
    let strength =
        10.0 * body_pose * body_pose * (1.0 - ((remaining as f64 - 500.0) / 800.0).clamp(0.0, 1.0));
    let displacement = world.random_enemy_offset(strength);
    let mut event = EnemyEvent::new(EnemyEventKind::EnemyCameraShake, actor.id, tick);
    event.source_position = Some(actor.position);
    event.camera_shake =
        Some(EnemyCameraShake { attenuation: CameraShakeAttenuation::Fixed, displacement });
    world.emit_event(event);
    let body_offset = world.random_enemy_offset(body_pose * 3.0);
    let mut countdown = remaining - 1;
    actor.heading_deg = positive_modulo(
        fround(actor.heading_deg + (1.0 - countdown as f64 / 1000.0) * 10.0),
        360.0,
    );
    if body_pose == 2.0 || world.steering_rng.draw_integer(4) == 1 {
        let angle = world.steering_rng.draw_scaled_float(360.0, false);
        let radius = fround(50.0 + world.steering_rng.draw_scaled_float(50.0, false));
        let offset = radial_vector(angle, radius);
        let rotation = fround(angle + world.steering_rng.draw_scaled_float(20.0, true));
        let scale = fround(1.0 + world.steering_rng.draw_scaled_float(1.0, false));
        let entry = 375 + world.steering_rng.draw_integer(2) as usize;
        world.spawn_simple_death_effect(
            owner,
            tick,
            SimpleDeathEffectSpec {
                alpha: 1.0,
                alpha_loss_per_tick: fround(0.02),
                atlas: EnemyEffectAtlas::BadGuys,
                entry,
                blend_mode: EnemyEffectBlend::Add,
                kind: EnemyDeathEffectKind::FadeAdditive,
                lifetime_ticks: 1000,
                role: "discorporeal-death-branch",
                scale,
            },
            DeathEffectOverrides {
                position: Some(Vec2::new(
                    actor.position.x + offset.x,
                    actor.position.y - 50.0 + offset.y,
                )),
                presentation_owner: Some(DeathPresentationOwner::PreWorldQueue),
                rotation_deg: Some(rotation),
                tint: Some(UNHOLY_GREEN),
                ..Default::default()
            },
            order,
        );
    }
    if world.demon_skull_encounter.death_stream_ticks_remaining == 0 && countdown > 0 {
        countdown = 0;
        let mega_death = capabilities & 16 != 0;
        let spell = world.create_boss_spell(
            actor.id,
            tick,
            0.0,
            actor.position,
            BossSpellKind::UltraBanish {
                alpha: 1.0,
                flash_alpha: 1.5,
                light_radius: 1.0,
                mega_death,
                remaining_ticks: if mega_death { 2000 } else { 150 },
            },
            order,
        );
        world.boss_spells.push(spell);
        for (sound, pitch) in [
            (EnemySound::LightningStart, 1.0),
            (EnemySound::DistortReality, 0.5),
            (EnemySound::FireyDeath, 0.75),
            (EnemySound::FireyDeath, 0.5),
        ] {
            world.emit_action_sound(owner, tick, sound, pitch);
        }
        let mut event = EnemyEvent::new(EnemyEventKind::EnemyScreenFlash, actor.id, tick);
        event.source_position = Some(actor.position);
        event.screen_flash = Some(EnemyScreenFlash {
            red: 0.25,
            green: 1.0,
            blue: 0.25,
            alpha: 1.0,
            decay_per_tick: fround(0.025),
            point_attenuated: false,
        });
        world.emit_event(event);
    }
    actor.body_pose = body_pose;
    actor.death_tick += 1;
    let EnemyBrain::DemonSkull(brain) = &mut actor.brain else { unreachable!() };
    brain.body_pose = body_pose;
    brain.body_offset = body_offset;
    brain.body_heading_deg = actor.body.heading_deg;
    brain.death_countdown = countdown;
}

fn flash(world: &mut EnemyWorld, actor: &impl EnemyActorRead, tick: u64, alpha: f64, loss: f64) {
    let mut event = EnemyEvent::new(EnemyEventKind::EnemyScreenFlash, actor.id, tick);
    event.source_position = Some(actor.position);
    event.screen_flash = Some(EnemyScreenFlash {
        red: 140.0 / 255.0,
        green: 1.0,
        blue: 12.0 / 255.0,
        alpha,
        decay_per_tick: loss,
        point_attenuated: false,
    });
    world.emit_event(event);
}
