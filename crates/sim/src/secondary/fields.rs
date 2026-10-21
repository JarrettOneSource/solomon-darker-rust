//! Magic Circle and Trap contacts, including their target-owned electric light.

use super::casting::magic_trap_screen_flash;
use super::program::*;
use super::step::{BurnRequest, StepEngine};
use super::{
    NativeSecondaryActorKind as Kind, NativeSecondaryActorState as Actor,
    NativeSecondaryAudioCue as Cue, NativeSecondaryDamageKind as Damage,
    NativeSecondaryEventKind as Event, NativeSecondaryTargetEffectPatch as Patch,
    NativeSecondaryWorld,
};
use crate::Vec2;
use crate::float::{fround, hypot};

pub(super) fn step<W: NativeSecondaryWorld>(
    engine: &mut StepEngine<'_, '_, '_, W>,
    source: &Actor,
    actor: &mut Actor,
    retain: &mut bool,
) {
    let owner = &engine.context.players[&actor.owner_id];
    match actor.kind {
        Kind::MagicCircle => {
            actor.alpha = if actor.age_ticks < 1500 {
                magic_circle_visual_tick(&mut engine.state.rng, engine.context.tick)
            } else {
                0.0
            };
            if actor.age_ticks < 1500 && actor.age_ticks.is_multiple_of(10) {
                if inside_rectangle(actor.position, owner.character.position, 210.0, 168.0) {
                    *engine.output.mana_recovered.entry(actor.owner_id.clone()).or_default() +=
                        actor.quantity * 2.0;
                    magic_circle_player_flash(
                        engine.state,
                        &actor.owner_id,
                        &actor.world_key,
                        Vec2::new(owner.character.position.x, owner.character.position.y - 15.0),
                        1,
                    );
                }
                for target in engine.candidates(actor, hypot(210.0, 168.0)) {
                    if inside_rectangle(actor.position, target.position, 210.0, 168.0) {
                        engine.merge(
                            actor,
                            target.id,
                            Patch {
                                circle_slow_factor: Some((1.0 - actor.slow_factor).max(0.0)),
                                circle_slow_ticks: Some(20),
                                ..Patch::default()
                            },
                        );
                    }
                }
            }
            if actor.age_ticks == 2 {
                let mut event =
                    actor_event(actor, engine.context.tick, Event::Pulse, Some(Cue::MagicCircle));
                event.screen_flash = Some(screen_flash(0.75, 1.0, 1.0, 0.1, true));
                engine.state.emit_event(event);
            }
        }
        Kind::MagicCirclePlayerFlash => {
            actor.alpha = fround(source.alpha - (0.05_f32 as f64));
            actor.position =
                Vec2::new(owner.character.position.x, owner.character.position.y - 15.0);
            actor.scale = fround(source.scale * (1.1_f32 as f64));
            *retain = actor.alpha > 0.0;
        }
        Kind::MagicTrap => {
            actor.frame = fround(source.frame + 0.25);
            if actor.frame >= 8.0 {
                actor.frame = fround(actor.frame - 8.0);
            }
            let charge = fround(source.scale + (1.0_f32 / (8.0 * 100.0)) as f64).min(1.0);
            actor.scale = charge;
            if actor.age_ticks.is_multiple_of(25) {
                let armed = engine
                    .candidates(actor, hypot(65.0, 65.0))
                    .iter()
                    .any(|target| inside_rectangle(actor.position, target.position, 65.0, 65.0));
                if armed {
                    let presentation = engine.state.rng.clone();
                    advance_rng(&mut engine.state.rng, 502);
                    let mut burst = super::program::actor(
                        Kind::MagicTrapBurst,
                        &actor.owner_id,
                        &actor.world_key,
                        Some(50),
                        1,
                        actor.position,
                    );
                    burst.lifetime_ticks = 116;
                    burst.presentation_rng = Some(presentation);
                    burst.variant = actor.variant;
                    engine.state.spawn_actor(burst);
                    let mut event =
                        actor_event(actor, engine.context.tick, Event::Impact, Some(Cue::Trap));
                    event.camera_magnitude = 1.25;
                    event.screen_flash =
                        Some(magic_trap_screen_flash(actor.variant as usize, 0.05, true));
                    engine.state.emit_event(event);
                    let amount = fround(actor.damage * charge);
                    for target in engine.candidates(actor, hypot(150.0, 150.0)) {
                        if !inside_rectangle(actor.position, target.position, 150.0, 150.0) {
                            continue;
                        }
                        if actor.variant == 2 {
                            engine.electric_burns.push(BurnRequest {
                                actor: actor.clone(),
                                damage: amount,
                                target,
                            });
                            continue;
                        }
                        let damage = match actor.variant {
                            1 => Damage::Fire,
                            3 => Damage::Ice,
                            _ => Damage::Magic,
                        };
                        engine.damage(actor, &target, amount, damage);
                        if actor.variant == 3 {
                            engine.merge(actor,target.id,Patch {cold_slow_ticks:Some((400.0*charge).trunc().max(50.0) as u64),cold_slow_material:Some(true),cold_slow_factor:Some(actor.slow_factor),..Patch::default()});
                        }
                        if actor.variant == 1 {
                            engine.fire_burns.push(BurnRequest {
                                actor: actor.clone(),
                                damage: owner.fire_burn_damage,
                                target,
                            });
                        }
                    }
                    *retain = false;
                }
            }
            let mut shimmer = fround(source.phase * (0.9_f32 as f64));
            if shimmer < (0.1_f32 as f64) {
                shimmer = 0.0;
            }
            actor.phase = shimmer;
            if shimmer > 0.0 {
                let rotation = engine.state.rng.draw_scaled_float(360.0, false);
                let alpha = engine.state.rng.draw_scaled_float(0.25, false);
                let mut child = super::program::actor(
                    Kind::MagicTrapShimmer,
                    &actor.owner_id,
                    &actor.world_key,
                    Some(50),
                    1,
                    actor.position,
                );
                child.alpha = fround(0.75 + alpha);
                child.lifetime_ticks = 20;
                child.rotation_radians = rotation * std::f64::consts::PI / 180.0;
                child.scale = fround(shimmer * 3.0);
                child.variant = actor.variant;
                engine.state.spawn_actor(child);
            }
        }
        Kind::MagicTrapShimmer => {
            actor.alpha = fround(source.alpha - (0.05_f32 as f64));
            *retain = actor.alpha > 0.0;
        }
        Kind::ElectricBurn => {
            let Some(target) =
                actor.target_id.and_then(|id| engine.world.target(&actor.world_key, id))
            else {
                *retain = false;
                return;
            };
            let light = engine.state.rng.draw_scaled_float(0.25, true);
            let gate = engine.state.rng.draw_integer(3);
            let scalar = if gate == 1 {
                fround(0.25 + engine.state.rng.draw_scaled_float(0.5, false))
            } else {
                0.0
            };
            actor.alpha = 1.0;
            actor.light_registration = Some(target.light_registration);
            actor.phase = scalar;
            actor.position = target.position;
            actor.radius = fround(0.5 + light);
            engine.damage(actor, &target, actor.damage, Damage::Lightning);
        }
        _ => unreachable!("field family dispatcher"),
    }
}
