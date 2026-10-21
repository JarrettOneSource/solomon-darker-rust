//! Steam channel particles and private detonation fans from `native-weld-steam.ts`.

use super::model::{NativeWeldActorBase, NativeWeldActorBirth};
use crate::float::{fround, hypot};
use crate::heading::heading_degrees_from_vector;
use crate::spells::fire::NativeFireExplosionState;
use crate::spells::model::FireExplosionPresentation;
use crate::spells::targeting::direction_from_heading;
use crate::{NativeRng, Vec2};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum NativeWeldSteamVariant {
    Normal,
    Over,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NativeWeldSteamActorState {
    pub common: NativeWeldActorBase,
    pub alpha_multiplier: f64,
    pub blue: f64,
    pub color_rise: f64,
    pub contact_damage: f64,
    pub contact_due: bool,
    pub contact_enabled: bool,
    pub contact_ticks_remaining: i64,
    pub life: f64,
    pub life_loss: f64,
    pub phase: f64,
    pub position: Vec2,
    pub remaining_distance: f64,
    pub rotation_degrees: f64,
    pub scale: f64,
    pub stretch: f64,
    pub terminal_position: Vec2,
    pub tint_fade: f64,
    pub variant: NativeWeldSteamVariant,
    pub velocity: Vec2,
}

struct NativeWeldSteamConstructorProgram {
    color_rise: f64,
    contact_ticks_remaining: i64,
    life: f64,
    life_loss: f64,
    scale: f64,
    stretch: f64,
    tint_fade: f64,
}

pub struct SpawnNativeWeldSteamInput<'a> {
    pub birth: NativeWeldActorBirth<'a>,
    pub damage: f64,
    pub query_origin: Vec2,
    pub underpowered: bool,
    pub obstruction_point: &'a dyn Fn(Vec2, Vec2) -> Option<Vec2>,
}

#[must_use]
pub fn spawn_native_weld_steam_actor(
    input: SpawnNativeWeldSteamInput<'_>,
    rng: &mut NativeRng,
) -> Option<NativeWeldSteamActorState> {
    let birth = input.birth;
    // Retail 0x00542F02 admits particle construction on the even update lane.
    if !birth.tick.is_multiple_of(2) {
        return None;
    }
    let gate = rng.draw_integer(7);
    let variant = if gate == 1 && !input.underpowered {
        NativeWeldSteamVariant::Over
    } else {
        NativeWeldSteamVariant::Normal
    };
    let constructor = draw_steam_constructor(rng, variant);
    let offset_distance = rng.draw_scaled_float(10.0, false);
    let offset_heading = rng.draw_scaled_float(4.5, true);
    let heading = heading_degrees_from_vector(birth.direction.x, birth.direction.y);
    let offset_direction = direction_from_heading(heading + offset_heading);
    let movement_direction = direction_from_heading(heading);
    let widen = if input.underpowered { 0.0 } else { birth.vector[2] };
    let movement_factor = fround(1.0 + widen * 0.02_f32 as f64);
    let movement_speed = fround(
        (if variant == NativeWeldSteamVariant::Over { 6.0 } else { 5.4_f32 as f64 })
            * movement_factor,
    );
    let base_origin = Vec2::new(
        fround(birth.origin.x - birth.direction.x * 15.0),
        fround(birth.origin.y - birth.direction.y * 15.0),
    );
    let position = Vec2::new(
        fround(base_origin.x + offset_direction.x * offset_distance),
        fround(base_origin.y + offset_direction.y * offset_distance),
    );
    let velocity = Vec2::new(
        fround(movement_direction.x * movement_speed),
        fround(movement_direction.y * movement_speed),
    );
    let predicted_end = Vec2::new(
        fround(input.query_origin.x + velocity.x * (constructor.life / constructor.life_loss)),
        fround(input.query_origin.y + velocity.y * (constructor.life / constructor.life_loss)),
    );
    let terminal = (input.obstruction_point)(input.query_origin, predicted_end);
    let mut remaining_distance = 9_999_999.0;
    if let Some(terminal) = terminal {
        remaining_distance = fround(hypot(terminal.x - position.x, terminal.y - position.y));
        if squared_distance(input.query_origin, terminal)
            < squared_distance(input.query_origin, position)
        {
            remaining_distance = 0.0;
        }
    }
    Some(NativeWeldSteamActorState {
        common: birth.actor(),
        alpha_multiplier: if input.underpowered { 0.25 } else { 1.0 },
        blue: 0.75,
        color_rise: constructor.color_rise,
        contact_damage: input.damage,
        contact_due: false,
        contact_enabled: variant == NativeWeldSteamVariant::Normal && !input.underpowered,
        contact_ticks_remaining: constructor.contact_ticks_remaining,
        life: constructor.life,
        life_loss: constructor.life_loss,
        phase: 0.0,
        position,
        remaining_distance,
        rotation_degrees: ((heading + offset_heading) % 360.0 + 360.0) % 360.0,
        scale: constructor.scale,
        stretch: constructor.stretch,
        terminal_position: terminal.unwrap_or(position),
        tint_fade: constructor.tint_fade,
        variant,
        velocity,
    })
}

pub struct NativeWeldSteamDetonationInput<'a> {
    pub explode_damage: f64,
    pub explode_radius: f64,
    pub fragment_count: u32,
    pub fragment_damage: f64,
    pub first_fragment_id: u64,
    pub origin: Vec2,
    pub owner_id: &'a str,
    pub private_seed: u32,
    pub tick: u64,
    pub world_key: &'a str,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeWeldSteamExplosionState {
    pub explosion: NativeFireExplosionState,
    pub sound_pitch: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeWeldSteamDetonation {
    pub explosion: Option<NativeWeldSteamExplosionState>,
    pub fragments: Vec<NativeWeldSteamActorState>,
    pub next_id: u64,
}

pub fn create_native_weld_steam_detonation(
    input: NativeWeldSteamDetonationInput<'_>,
    rng: &mut NativeRng,
) -> NativeWeldSteamDetonation {
    assert!(input.first_fragment_id > 0, "Steam detonation first fragment id must be positive");
    assert!(
        [input.explode_damage, input.explode_radius, input.fragment_damage]
            .iter()
            .all(|value| value.is_finite() && *value >= 0.0),
        "Steam detonation damage and radius must be finite and nonnegative",
    );
    if input.explode_radius <= 0.0 {
        return NativeWeldSteamDetonation {
            explosion: None,
            fragments: Vec::new(),
            next_id: input.first_fragment_id,
        };
    }
    let sound_pitch = rng.draw_scaled_float(0.1_f32 as f64, true);
    let visual_scale = fround(fround(input.explode_radius - 10.0) * 0.18_f32 as f64 + 1.0);
    let explosion = NativeFireExplosionState {
        burn_damage: 0.0,
        damage: fround(input.explode_damage * 0.5),
        footprint_dimension: fround(visual_scale * 110.0),
        origin: input.origin,
        owner_id: input.owner_id.into(),
        presentation: FireExplosionPresentation::Steam,
        visual_scale,
        world_key: input.world_key.into(),
    };
    let mut fragments = Vec::new();
    if input.fragment_count > 0 {
        // The complete fan consumes a projectile-private stream. Only the sound
        // pitch above advances combat RNG, including when no fragments exist.
        let mut private_rng = NativeRng::new(i64::from(input.private_seed));
        let start_heading = private_rng.draw_scaled_float(360.0, false);
        let heading_step = fround(360.0 / f64::from(input.fragment_count));
        for fragment_index in 0..input.fragment_count {
            let base_heading = fround(start_heading + f64::from(fragment_index) * heading_step);
            for _ in 0..3 {
                let constructor =
                    draw_steam_constructor(&mut private_rng, NativeWeldSteamVariant::Normal);
                let offset_distance = private_rng.draw_scaled_float(10.0, false);
                let offset_heading = private_rng.draw_scaled_float(25.0, true);
                let speed_draw = private_rng.draw_scaled_float(0.1_f32 as f64, false);
                let direction = direction_from_heading(base_heading);
                let offset_direction = direction_from_heading(base_heading + offset_heading);
                let position = Vec2::new(
                    fround(input.origin.x + offset_direction.x * offset_distance),
                    fround(input.origin.y + offset_direction.y * offset_distance),
                );
                let mut velocity = Vec2::new(fround(direction.x * 4.0), fround(direction.y * 4.0));
                for factor in [0.9_f32 as f64, 1.5, fround(0.9 + speed_draw)] {
                    velocity.x = fround(velocity.x * factor);
                    velocity.y = fround(velocity.y * factor);
                }
                velocity.y = fround(velocity.y * 0.8_f32 as f64);
                let birth = NativeWeldActorBirth {
                    build_id: 1005,
                    direction,
                    id: input.first_fragment_id + fragments.len() as u64,
                    origin: input.origin,
                    owner_id: input.owner_id,
                    tick: input.tick,
                    vector: &[0.0; 8],
                    world_key: input.world_key,
                };
                fragments.push(NativeWeldSteamActorState {
                    common: birth.actor(),
                    alpha_multiplier: 1.0,
                    blue: 0.75,
                    color_rise: constructor.color_rise,
                    contact_damage: fround(input.fragment_damage / 100.0),
                    contact_due: false,
                    contact_enabled: true,
                    contact_ticks_remaining: constructor.contact_ticks_remaining,
                    life: constructor.life,
                    life_loss: constructor.life_loss,
                    phase: 0.0,
                    position,
                    remaining_distance: 10_000_000.0,
                    rotation_degrees: heading_degrees_from_vector(velocity.x, velocity.y),
                    scale: constructor.scale,
                    stretch: fround(constructor.stretch * 0.6_f32 as f64),
                    terminal_position: Vec2::ZERO,
                    tint_fade: constructor.tint_fade,
                    variant: NativeWeldSteamVariant::Normal,
                    velocity,
                });
            }
        }
    }
    NativeWeldSteamDetonation {
        explosion: Some(NativeWeldSteamExplosionState {
            explosion,
            sound_pitch: fround(1.0 + sound_pitch),
        }),
        next_id: input.first_fragment_id + fragments.len() as u64,
        fragments,
    }
}

pub fn step_native_weld_steam_actor(actor: &mut NativeWeldSteamActorState) -> bool {
    let speed = hypot(actor.velocity.x, actor.velocity.y);
    actor.remaining_distance = fround(actor.remaining_distance - speed);
    if actor.remaining_distance < 0.0 {
        actor.velocity = Vec2::ZERO;
        actor.position = actor.terminal_position;
        actor.remaining_distance = 999_999.0;
    }
    actor.position = Vec2::new(
        fround(actor.position.x + actor.velocity.x),
        fround(actor.position.y + actor.velocity.y),
    );
    actor.life = fround(actor.life - actor.life_loss);
    if actor.life < 0.0 {
        return false;
    }
    let velocity_factor = if actor.life >= 1.0 { 0.96_f32 as f64 } else { 0.88_f32 as f64 };
    if actor.life < 1.0 {
        actor.stretch = fround(actor.stretch * 0.95_f32 as f64);
        actor.scale = fround(actor.scale + 0.01_f32 as f64);
        actor.life = fround(actor.life + fround(actor.life_loss * 0.5));
    }
    actor.contact_due = false;
    if actor.life > 0.5 && actor.contact_enabled {
        actor.contact_ticks_remaining -= 1;
        if actor.contact_ticks_remaining < 1 {
            actor.contact_due = true;
            actor.contact_ticks_remaining = 10;
        }
    }
    actor.common.owner.age_ticks += 1;
    actor.blue = fround(actor.blue - 0.125);
    actor.phase = fround(actor.phase + actor.color_rise);
    actor.tint_fade = fround(actor.tint_fade - 0.25).max(0.0);
    actor.velocity = Vec2::new(
        fround(actor.velocity.x * velocity_factor),
        fround(actor.velocity.y * velocity_factor),
    );
    true
}

fn draw_steam_constructor(
    rng: &mut NativeRng,
    variant: NativeWeldSteamVariant,
) -> NativeWeldSteamConstructorProgram {
    let life_draw = rng.draw_scaled_float(0.05_f32 as f64, false);
    let loss_draw = rng.draw_scaled_float(0.1_f32 as f64, false);
    let scale_draw = rng.draw_scaled_float(0.75, false);
    let stretch_draw = rng.draw_scaled_float(1.0, false);
    let blue_draw = rng.draw_scaled_float(0.1_f32 as f64, false);
    let contact_clock = rng.draw_integer(10);
    let mut life = fround(fround(fround(life_draw + 1.25) + 1.0) * 0.5);
    let life_loss = fround(fround(loss_draw + 0.01_f32 as f64) * 0.5);
    let mut color_rise = 0.15_f32 as f64;
    let mut tint_fade = fround(1.0 + blue_draw);
    if variant == NativeWeldSteamVariant::Over {
        life = fround(life * 0.65_f32 as f64);
        color_rise = fround(color_rise * 0.5);
        tint_fade = 0.0;
    }
    let scale = fround(scale_draw + 0.65_f32 as f64);
    NativeWeldSteamConstructorProgram {
        color_rise,
        contact_ticks_remaining: i64::from(contact_clock),
        life,
        life_loss,
        scale,
        stretch: fround(fround(stretch_draw + 2.0) * scale),
        tint_fade,
    }
}

fn squared_distance(first: Vec2, second: Vec2) -> f64 {
    let dx = first.x - second.x;
    let dy = first.y - second.y;
    dx * dx + dy * dy
}
