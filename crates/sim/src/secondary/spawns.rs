//! Child constructors used during the ordered actor loop. Draw order and the few
//! constructor pre-ticks are observable through the shared native RNG.

use super::leviathan::native_leviathan_heading_vector;
use super::program::*;
use super::{
    NativeSecondaryActorKind as Kind, NativeSecondaryActorState as Actor,
    NativeSecondarySimulationState as State,
};
use crate::Vec2;
use crate::float;
use crate::float::{fround, hypot};

pub(super) enum EtherFadePresentation {
    Muzzle,
    Impact,
}

/// `spawnPlanewalkerMote`: one ordinary-life FadeMove per eligible active tick.
pub(super) fn planewalker_mote(
    state: &mut State,
    owner_id: &str,
    origin: Vec2,
    world_key: &str,
    enhanced: bool,
) {
    let angle = state.rng.draw_scaled_float(360.0, false);
    let radius = state.rng.draw_scaled_float(15.0, false);
    let vertical = state.rng.draw_scaled_float(5.0, false);
    let scale = state.rng.draw_float_range(0.5, 1.0);
    let speed = state.rng.draw_scaled_float(3.0, false);
    let multiplier = if enhanced {
        state.rng.draw_float_range(0.15, 0.3)
    } else {
        state.rng.draw_float_range(0.25, 0.45)
    };
    let radians = angle * std::f64::consts::PI / 180.0;
    let mut mote = actor(
        Kind::PlanewalkerMote,
        owner_id,
        world_key,
        Some(12),
        1,
        Vec2::new(
            fround(origin.x + fround(float::sin(radians) * radius)),
            fround(fround(origin.y - fround(float::cos(radians) * radius)) - vertical),
        ),
    );
    mote.enhanced = enhanced;
    mote.alpha = 1.0;
    mote.lifetime_ticks = 1000;
    mote.phase = fround((0.1_f32 as f64) * multiplier);
    mote.velocity =
        Vec2::new(fround(float::sin(radians) * speed), fround(-float::cos(radians) * speed));
    mote.rotation_radians = radius * std::f64::consts::PI / 180.0;
    mote.scale = scale;
    mote.frame = 11.0;
    state.spawn_actor(mote);
}

pub(super) fn ether_fade(
    state: &mut State,
    parent: &Actor,
    position: Vec2,
    presentation: EtherFadePresentation,
    tick: u64,
) {
    let (life, scale, decrement, lit) = match presentation {
        EtherFadePresentation::Muzzle => (1.0, 1.5, 0.05, 0),
        EtherFadePresentation::Impact => (2.0, 2.0, 0.1, 1),
    };
    let mut child = actor(
        Kind::EtherFade,
        &parent.owner_id,
        &parent.world_key,
        Some(11),
        parent.rank,
        position,
    );
    child.alpha = fround(life);
    child.lifetime_ticks = 19;
    child.quantity = tick as f64;
    child.scale = fround(scale);
    child.slow_factor = fround(decrement);
    child.variant = lit;
    state.spawn_actor(child);
}

pub(super) fn leviathan_mote(state: &mut State, parent: &Actor, current_scale: f64) {
    let heading = state.rng.draw_scaled_float(360.0, false);
    let vertical = state.rng.draw_scaled_float(5.0, false);
    let scale = state.rng.draw_scaled_float(0.5, false);
    let speed = state.rng.draw_scaled_float(3.0, false);
    let life = state.rng.draw_scaled_float(0.15, false);
    let direction = native_leviathan_heading_vector(heading);
    let velocity = Vec2::new(fround(speed * direction.x), fround(speed * direction.y));
    let decrement = fround((0.1_f32 as f64) * fround(2.0 + life));
    let birth = Vec2::new(
        fround(parent.position.x + current_scale * 20.0 * direction.x),
        fround(parent.position.y + current_scale * 20.0 * direction.y - vertical),
    );
    let mut child = actor(
        Kind::LeviathanMote,
        &parent.owner_id,
        &parent.world_key,
        Some(11),
        parent.rank,
        Vec2::new(fround(birth.x + velocity.x), fround(birth.y + velocity.y)),
    );
    child.alpha = fround(1.0 - decrement);
    child.lifetime_ticks = 10;
    child.quantity = 0.5;
    child.rotation_radians = heading * std::f64::consts::PI / 180.0;
    child.scale = fround(0.5 + scale);
    child.slow_factor = decrement;
    child.variant = 11;
    child.velocity =
        Vec2::new(fround(velocity.x * (0.95_f32 as f64)), fround(velocity.y * (0.95_f32 as f64)));
    state.spawn_actor(child);
}

pub(super) fn plane_orb_birth(state: &mut State, owner: &str, world_key: &str, origin: Vec2) {
    for heading in (0..360).step_by(40) {
        let heading = f64::from(heading);
        let radius = state.rng.draw_scaled_float(100.0, false);
        let jitter = state.rng.draw_scaled_float(10.0, true);
        let scale = state.rng.draw_scaled_float(4.0, false);
        let speed = state.rng.draw_scaled_float(5.0, false);
        let life = state.rng.draw_scaled_float(0.05, false);
        let direction = native_leviathan_heading_vector(heading + jitter);
        let velocity_direction = native_leviathan_heading_vector(heading);
        let mut child = actor(
            Kind::PlaneOrbParticle,
            owner,
            world_key,
            Some(12),
            1,
            Vec2::new(
                fround(origin.x + radius * direction.x),
                fround(origin.y + radius * direction.y),
            ),
        );
        child.lifetime_ticks = 200;
        child.quantity = 0.5;
        child.rotation_radians = heading * std::f64::consts::PI / 180.0;
        child.scale = fround(1.0 + scale);
        child.slow_factor = fround((0.1_f32 as f64) * fround(0.1 + life));
        child.variant = 11;
        child.velocity =
            Vec2::new(fround(-speed * velocity_direction.x), fround(-speed * velocity_direction.y));
        state.spawn_actor(child);
        for _ in 0..2 {
            let radius = state.rng.draw_scaled_float(100.0, false);
            let jitter = state.rng.draw_scaled_float(10.0, true);
            let scale = state.rng.draw_scaled_float(4.0, false);
            let green = state.rng.draw_scaled_float(0.8, false);
            let speed = state.rng.draw_scaled_float(5.0, false);
            let life = state.rng.draw_scaled_float(0.25, false);
            let direction = native_leviathan_heading_vector(heading + jitter);
            let mut child = actor(
                Kind::PlaneOrbParticle,
                owner,
                world_key,
                Some(12),
                1,
                Vec2::new(
                    fround(origin.x + radius * direction.x),
                    fround(origin.y + radius * direction.y),
                ),
            );
            child.lifetime_ticks = 200;
            child.quantity = fround(0.5 + green);
            child.rotation_radians = heading * std::f64::consts::PI / 180.0;
            child.scale = fround(fround(1.0 + scale) * 0.5);
            child.slow_factor = fround((0.1_f32 as f64) * fround(0.25 + life));
            child.variant = 45;
            child.velocity = Vec2::new(
                fround(-fround(5.0 + speed) * velocity_direction.x),
                fround(-fround(5.0 + speed) * velocity_direction.y),
            );
            state.spawn_actor(child);
        }
    }
}

pub(super) fn plane_orb_mote(state: &mut State, parent: &Actor) {
    let heading = state.rng.draw_scaled_float(360.0, false);
    let vertical = state.rng.draw_scaled_float(5.0, false);
    let scale = state.rng.draw_scaled_float(0.5, false);
    let speed = state.rng.draw_scaled_float(3.0, false);
    let life = state.rng.draw_scaled_float(0.15, false);
    let direction = native_leviathan_heading_vector(heading);
    let distance = fround(parent.scale * 20.0);
    let mut child = actor(
        Kind::PlaneOrbParticle,
        &parent.owner_id,
        &parent.world_key,
        Some(12),
        1,
        Vec2::new(
            fround(parent.position.x + distance * direction.x),
            fround(parent.position.y + distance * direction.y - 15.0 - vertical),
        ),
    );
    child.lifetime_ticks = 200;
    child.quantity = 0.5;
    child.rotation_radians = heading * std::f64::consts::PI / 180.0;
    child.scale = fround(0.5 + scale);
    child.slow_factor = fround((0.1_f32 as f64) * fround(0.15 + life));
    child.variant = 11;
    child.velocity = Vec2::new(fround(speed * direction.x), fround(speed * direction.y));
    state.spawn_actor(child);
}

pub(super) fn drain_cloud(state: &mut State, parent: &Actor) {
    let scale = state.rng.draw_scaled_float(1.5, false);
    let rotation = state.rng.draw_scaled_float(360.0, false);
    let alpha = state.rng.draw_scaled_float(0.15_f32 as f64, false);
    let speed = state.rng.draw_scaled_float(3.0, false);
    let record = state.rng.draw_integer(2);
    let distance = state.rng.draw_scaled_float(100.0, false);
    let radial = native_unit_vector(&mut state.rng);
    let position = Vec2::new(
        fround(parent.position.x + radial.x * distance),
        fround(parent.position.y + radial.y * distance),
    );
    let distance = hypot(position.x - parent.position.x, position.y - parent.position.y);
    let speed = fround(5.0 + speed);
    let remaining = fround(distance - 20.0);
    let phase_step = if remaining >= 0.0 { (180.0 / (remaining / speed)).max(1.0) } else { 10.0 };
    let mut child = actor(
        Kind::EtherDrainCloud,
        &parent.owner_id,
        &parent.world_key,
        Some(74),
        parent.rank,
        position,
    );
    child.alpha = fround(0.1 + alpha);
    child.endpoint = parent.position;
    child.lifetime_ticks = 400;
    child.quantity = fround(speed * 0.5);
    child.rotation_radians = rotation * std::f64::consts::PI / 180.0;
    child.scale = fround(1.0 + scale);
    child.slow_factor = fround(phase_step * 0.5);
    child.variant = record;
    child.velocity = unit(position, parent.position);
    state.spawn_actor(child);
}

pub(super) fn quake(state: &mut State, parent: &Actor, intensity: f64) {
    let rotation = state.rng.draw_scaled_float(360.0, false);
    let selector = state.rng.draw_integer(4);
    let scale = fround(2.0 + f64::from(selector));
    let mut child = actor(
        Kind::EarthquakeQuake,
        &parent.owner_id,
        &parent.world_key,
        Some(41),
        1,
        parent.position,
    );
    child.alpha = fround(intensity * intensity * intensity);
    child.lifetime_ticks = 180;
    child.rotation_radians = rotation * std::f64::consts::PI / 180.0;
    child.scale = scale;
    child.slow_factor = fround((0.8_f32 as f64) * scale);
    state.spawn_actor(child);
}

pub(super) fn advance_dust(child: &mut Actor) {
    child.phase = fround(child.phase + 0.5);
    child.alpha =
        fround(float::sin(child.phase * std::f64::consts::PI / 180.0).abs() * child.quantity);
    child.position = Vec2::new(
        fround(child.position.x + child.velocity.x),
        fround(child.position.y + child.velocity.y),
    );
}

pub(super) fn dust(state: &mut State, parent: &Actor, scenery: &Actor) {
    let speed = state.rng.draw_scaled_float(0.25, false);
    let magnitude = state.rng.draw_scaled_float(0.5, false);
    let rotation = state.rng.draw_scaled_float(360.0, false);
    let scale = state.rng.draw_scaled_float(2.0, false);
    let distance = state.rng.draw_scaled_float(30.0, false);
    let direction = native_unit_vector(&mut state.rng);
    let mut child = actor(
        Kind::EarthquakeDust,
        &parent.owner_id,
        &parent.world_key,
        Some(41),
        1,
        Vec2::new(
            fround(scenery.position.x + direction.x * distance),
            fround(scenery.position.y + direction.y * distance),
        ),
    );
    child.enhanced = true;
    child.lifetime_ticks = 360;
    child.quantity = fround(0.5 + magnitude);
    child.rotation_radians = rotation * std::f64::consts::PI / 180.0;
    child.scale = fround(2.0 + scale);
    child.target_id = scenery.target_id;
    child.velocity = Vec2::new(fround((speed + 0.25) / 3.0), 0.0);
    advance_dust(&mut child);
    state.spawn_actor(child);
}

pub(super) fn quake_debris(state: &mut State, parent: &Actor) {
    let heading = state.rng.draw_scaled_float(360.0, false);
    let bounce = state.rng.draw_scaled_float(3.0, false);
    state.rng.draw_scaled_float(50.0, false);
    let rotation = state.rng.draw_scaled_float(360.0, false);
    let rotation_step = state.rng.draw_scaled_float(10.0, false);
    let record = state.rng.draw_integer(3);
    let radius = state.rng.draw_scaled_float(300.0, false);
    let direction = native_leviathan_heading_vector(heading);
    let bounce = fround(-(2.0 + bounce));
    let vertical_factor = state.rng.draw_scaled_float(1.5, false);
    let vertical = fround(fround(fround(vertical_factor + 0.75) * bounce) * 0.5);
    let height = state.rng.draw_scaled_float(50.0, false);
    let offset = state.rng.draw_scaled_float(15.0, false);
    let first_scale = state.rng.draw_scaled_float(0.75, false);
    let mut scale = 0.45_f32 as f64;
    if fround(0.5 + first_scale) >= 0.45 {
        scale = fround(0.5 + state.rng.draw_scaled_float(0.75, false));
    }
    scale = scale.min(0.75_f32 as f64);
    let multiplier = state.rng.draw_scaled_float(0.35, false);
    let speed = state.rng.draw_scaled_float(1.5, false);
    scale = fround(scale * fround(0.3 + multiplier));
    let speed = fround(1.5 + speed);
    let distance = fround(radius + offset);
    let mut child = actor(
        Kind::EarthquakeDebris,
        &parent.owner_id,
        &parent.world_key,
        Some(41),
        1,
        Vec2::new(
            fround(parent.position.x + direction.x * distance),
            fround(parent.position.y + direction.y * distance),
        ),
    );
    child.alpha = if parent.enhanced { 10.0 } else { 2.0 };
    child.enhanced = parent.enhanced;
    child.endpoint = Vec2::new(vertical, fround(1.0 + rotation_step));
    child.lifetime_ticks = MAX_NATIVE_TICKS;
    child.phase = fround(-height);
    child.quantity = bounce;
    child.rotation_radians = rotation * std::f64::consts::PI / 180.0;
    child.scale = scale;
    child.variant = record;
    child.velocity = Vec2::new(
        fround(direction.x * speed),
        fround(fround(direction.y * (0.8_f32 as f64)) * speed),
    );
    state.spawn_actor(child);
}
