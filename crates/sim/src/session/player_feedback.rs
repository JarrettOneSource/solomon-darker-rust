//! Player status particles and hurt voice requests from `boneyard-player-status.ts`
//! and `enemies/events.ts`. The caller retains returned events at the contact phase;
//! they do not enter the enemy world's ordinary per-tick event queue.

use crate::enemies::model::{
    DeathPresentationOwner, EnemyDeathEffect, EnemyDeathEffectKind, EnemyEffectAtlas,
    EnemyEffectBlend, EnemyEvent, EnemyEventKind, EnemySound,
};
use crate::enemies::world::EnemyWorld;
use crate::float::fround;
use crate::spells::targeting::direction_from_heading;
use crate::{NativeRng, Vec2};

pub(crate) struct PlayerRescueFeedbackInput<'a> {
    pub player_id: &'a str,
    pub position: Vec2,
    pub light_registration: crate::painter::PainterRegistration,
    pub tick: u64,
    pub world_key: &'a str,
    pub received: super::player_combat::PlayerEntityDamageResult,
}

/// The contact, poison and direct receivers share one accepted feedback program.
/// These returned semantic events are retained by the caller at its source phase.
pub(crate) fn materialize_player_rescue_feedback(
    world: &mut crate::boneyard::BoneyardWorld,
    secondary: &mut crate::secondary::NativeSecondarySimulationState,
    order: &mut crate::painter::WorldPainterOrder,
    flashes: &mut crate::screen_flash::NativeScreenFlashWriter,
    input: PlayerRescueFeedbackInput<'_>,
) -> Vec<EnemyEvent> {
    let mut events = Vec::new();
    for (accepted, kind) in [
        (input.received.hagatha_charms_lost, EnemyEventKind::PlayerCharmLost),
        (input.received.cheat_death_triggered, EnemyEventKind::PlayerCheatDeath),
    ] {
        if !accepted {
            continue;
        }
        let mut event = EnemyEvent::new(kind, 0, input.tick);
        event.event_id = world.enemies.next_event_id;
        world.enemies.next_event_id += 1;
        event.source_position = Some(input.position);
        event.target_player_id = Some(Some(input.player_id.into()));
        events.push(event);
    }
    if input.received.cheat_death_triggered {
        flashes.write(crate::screen_flash::NativeScreenFlashInput {
            tick: input.tick,
            world_key: input.world_key.into(),
            position: input.position,
            flash: crate::secondary::NativeSecondaryScreenFlashState::new(
                1.0,
                1.0,
                1.0,
                0.01_f32 as f64,
                false,
                1.0,
            ),
            only_if_clear: false,
        });
        secondary.spawn_player_rescue_shockwave(
            crate::secondary::NativePlayerRescueInput {
                light_registration: input.light_registration,
                owner_id: input.player_id,
                position: input.position,
                world_key: input.world_key,
            },
            order,
        );
    }
    events
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayerStatusKind {
    Cold,
    Poison,
}

pub struct PlayerStatusBurstRequest<'a> {
    pub actor_id: usize,
    pub player_id: &'a str,
    pub position: Vec2,
    pub status: PlayerStatusKind,
    pub tick: u64,
}

pub fn emit_player_status_burst(
    world: &mut EnemyWorld,
    request: PlayerStatusBurstRequest<'_>,
    rng: &mut NativeRng,
) -> EnemyEvent {
    let (tint, damping, role, sound) = match request.status {
        PlayerStatusKind::Cold => {
            (0x80bfff, 0.93_f32 as f64, "player-status-cold", EnemySound::Frosted)
        }
        PlayerStatusKind::Poison => {
            (0x80ff80, 0.95_f32 as f64, "player-status-poison", EnemySound::Poisoned)
        }
    };
    for heading in (0..360).step_by(30) {
        let rotation = rng.draw_scaled_float(360.0, false);
        let scale = rng.draw_scaled_float(0.75, false);
        let radius = rng.draw_scaled_float(20.0, false);
        let position_angle = rng.draw_scaled_float(10.0, true);
        let speed = rng.draw_scaled_float(3.0, false);
        let velocity_angle = rng.draw_scaled_float(10.0, true);
        let radial = direction_from_heading(fround(f64::from(heading) + position_angle));
        let direction = direction_from_heading(fround(f64::from(heading) + velocity_angle));
        let distance = fround(10.0 + radius);
        // Unlike generic pre-world births, these particles set lastStepTick to
        // this contact tick. The first particle step occurs on the next tick.
        world.death_effects.push(EnemyDeathEffect {
            age_ticks: 0,
            alpha: 0.5,
            alpha_loss_per_tick: 0.00625_f32 as f64,
            alpha_multiplier: 1.0,
            angular_velocity_deg: 0.0,
            atlas: EnemyEffectAtlas::BadGuys,
            blend_mode: EnemyEffectBlend::Add,
            bounce_retention: 0.0,
            bounce_velocity: 0.0,
            entry: 10,
            first_entry: 10,
            frame_count: 1,
            frame_phase: 0.0,
            frame_ticks: 1.0,
            frame_velocity: 0.0,
            frame_velocity_damping: 1.0,
            height: 0.0,
            id: world.next_death_effect_id,
            kind: EnemyDeathEffectKind::MoveFadePerspective,
            last_step_tick: Some(request.tick),
            lifetime_ticks: 81,
            opacity_timer: 0.5,
            owner_actor_id: request.actor_id,
            painter_registration: None,
            painter_sort_bias: None,
            position: Vec2::new(
                fround(request.position.x + fround(radial.x * distance)),
                fround(request.position.y + fround(radial.y * distance)),
            ),
            presentation_owner: DeathPresentationOwner::PreWorldQueue,
            role: role.into(),
            rotation_deg: rotation,
            scale: fround(0.75 + scale),
            scale_y: fround(0.75 + scale),
            scale_multiplier: 1.0,
            scrap_oscillation: None,
            shadow: false,
            spawn_tick: request.tick,
            tint,
            velocity: Vec2::new(
                fround(direction.x * speed),
                fround(fround(direction.y * speed) * (0.8_f32 as f64)),
            ),
            velocity_damping: damping,
            vertical_velocity: 0.0,
        });
        world.next_death_effect_id += 1;
    }
    let mut event = player_sound_event(
        world,
        request.actor_id,
        request.player_id,
        request.position,
        request.tick,
        EnemyEventKind::PlayerStatusSound,
    );
    event.gain_scale = Some(1.0);
    event.pitch = Some(1.5);
    event.sound = Some(sound);
    event
}

pub struct PlayerDamageSoundRequest<'a> {
    pub actor_id: usize,
    pub current_health: f64,
    pub player_id: &'a str,
    pub position: Vec2,
    pub tick: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerDamageSoundResult {
    pub deadline_tick: u64,
    pub event: EnemyEvent,
}

pub fn emit_boneyard_player_damage_sound(
    world: &mut EnemyWorld,
    request: PlayerDamageSoundRequest<'_>,
    rng: &mut NativeRng,
) -> PlayerDamageSoundResult {
    let delay = rng.draw_integer(41);
    let sound = match rng.draw_integer(3) {
        0 => EnemySound::WizardOuch1,
        1 => EnemySound::WizardOuch2,
        2 => EnemySound::WizardOuch3,
        _ => unreachable!("native Ouch sound draw is in 0..3"),
    };
    let mut event = player_sound_event(
        world,
        request.actor_id,
        request.player_id,
        request.position,
        request.tick,
        EnemyEventKind::PlayerDamageSound,
    );
    let health_scalar = ((request.current_health - 25.0) / 20.0).clamp(0.0, 1.0);
    event.gain_scale = Some(0.25 + 0.75 * (1.0 - health_scalar));
    event.pitch = Some(1.0);
    event.sound = Some(sound);
    // Deadline scales the absolute source tick. This intentionally differs from
    // adding a scaled delay to now, and uses the source's extra f32 health store.
    let deadline_tick = fround(
        (request.tick as f64 + 20.0 + f64::from(delay))
            * fround((request.current_health - 25.0) / 20.0).clamp(0.0, 1.0),
    )
    .trunc() as u64;
    PlayerDamageSoundResult { deadline_tick, event }
}

#[must_use]
pub const fn native_wizard_ouch_cooldown_ready(tick: u64, deadline_tick: u64) -> bool {
    tick > deadline_tick
}

pub(super) fn player_sound_event(
    world: &mut EnemyWorld,
    actor_id: usize,
    player_id: &str,
    position: Vec2,
    tick: u64,
    kind: EnemyEventKind,
) -> EnemyEvent {
    let mut event = EnemyEvent::new(kind, actor_id, tick);
    event.event_id = world.next_event_id;
    event.source_position = Some(position);
    event.target_player_id = Some(Some(player_id.into()));
    world.next_event_id += 1;
    event
}
