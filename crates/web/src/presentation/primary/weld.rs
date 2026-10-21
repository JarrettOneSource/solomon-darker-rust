//! Weld display fields from `client/primary-spell-transient-interpolation.ts`.

use solomon_sim::Vec2;
use solomon_sim::spells::weld::{
    NativeWeldHailChildActorState, NativeWeldPersistentActorState, NativeWeldWorldActor,
};

use crate::presentation::PresentationClocks;
use crate::presentation::math::{lerp, lerp_degrees, lerp_option, lerp_vec};

pub(super) fn same_kind(first: &NativeWeldWorldActor, next: &NativeWeldWorldActor) -> bool {
    if std::mem::discriminant(first) != std::mem::discriminant(next) {
        return false;
    }
    match (first, next) {
        (NativeWeldWorldActor::HailChild(first), NativeWeldWorldActor::HailChild(next)) => {
            std::mem::discriminant(first) == std::mem::discriminant(next)
        }
        (NativeWeldWorldActor::Persistent(first), NativeWeldWorldActor::Persistent(next)) => {
            std::mem::discriminant(first) == std::mem::discriminant(next)
        }
        _ => true,
    }
}

pub(super) fn project(
    output: &mut NativeWeldWorldActor,
    first: &NativeWeldWorldActor,
    next: &NativeWeldWorldActor,
    blend: f64,
    clocks: &mut PresentationClocks,
) {
    output.common_mut().direction =
        lerp_vec(first.common().direction, next.common().direction, blend);
    output.common_mut().origin = lerp_vec(first.common().origin, next.common().origin, blend);
    match (output, first, next) {
        (
            NativeWeldWorldActor::MeteorMarker(out),
            NativeWeldWorldActor::MeteorMarker(first),
            NativeWeldWorldActor::MeteorMarker(next),
        ) => {
            out.alpha = lerp(first.alpha, next.alpha, blend);
            out.rotation_degrees = lerp(first.rotation_degrees, next.rotation_degrees, blend);
            out.scale = lerp(first.scale, next.scale, blend);
        }
        (
            NativeWeldWorldActor::Channel(out),
            NativeWeldWorldActor::Channel(first),
            NativeWeldWorldActor::Channel(next),
        ) => {
            out.endpoint = nullable_vec(first.endpoint, next.endpoint, blend);
            out.midpoint = nullable_vec(first.midpoint, next.midpoint, blend);
        }
        (
            NativeWeldWorldActor::Impact(out),
            NativeWeldWorldActor::Impact(first),
            NativeWeldWorldActor::Impact(next),
        ) => {
            out.position = lerp_vec(first.position, next.position, blend);
            out.alpha = lerp(first.alpha, next.alpha, blend);
        }
        (
            NativeWeldWorldActor::BoulderDebris(out),
            NativeWeldWorldActor::BoulderDebris(first),
            NativeWeldWorldActor::BoulderDebris(next),
        ) => out.position = lerp_vec(first.position, next.position, blend),
        (
            NativeWeldWorldActor::FlameLashFade(out),
            NativeWeldWorldActor::FlameLashFade(first),
            NativeWeldWorldActor::FlameLashFade(next),
        ) => {
            out.position = lerp_vec(first.position, next.position, blend);
            out.alpha = lerp(first.alpha, next.alpha, blend);
        }
        (
            NativeWeldWorldActor::GroundSparkFade(out),
            NativeWeldWorldActor::GroundSparkFade(first),
            NativeWeldWorldActor::GroundSparkFade(next),
        ) => {
            out.seed.position = lerp_vec(first.seed.position, next.seed.position, blend);
            out.seed.alpha = lerp(first.seed.alpha, next.seed.alpha, blend);
        }
        (
            NativeWeldWorldActor::MeteorFlash(out),
            NativeWeldWorldActor::MeteorFlash(first),
            NativeWeldWorldActor::MeteorFlash(next),
        ) => {
            out.position = lerp_vec(first.position, next.position, blend);
            out.alpha = lerp(first.alpha, next.alpha, blend);
        }
        (
            NativeWeldWorldActor::HailChild(out),
            NativeWeldWorldActor::HailChild(first),
            NativeWeldWorldActor::HailChild(next),
        ) => hail_child(out, first, next, blend),
        (
            NativeWeldWorldActor::Steam(out),
            NativeWeldWorldActor::Steam(first),
            NativeWeldWorldActor::Steam(next),
        ) => {
            out.position = lerp_vec(first.position, next.position, blend);
            out.blue = lerp(first.blue, next.blue, blend);
            out.life = lerp(first.life, next.life, blend);
            out.phase = lerp(first.phase, next.phase, blend);
            out.remaining_distance = lerp(first.remaining_distance, next.remaining_distance, blend);
            out.scale = lerp(first.scale, next.scale, blend);
            out.stretch = lerp(first.stretch, next.stretch, blend);
            out.tint_fade = lerp(first.tint_fade, next.tint_fade, blend);
            out.velocity = lerp_vec(first.velocity, next.velocity, blend);
        }
        (
            NativeWeldWorldActor::HailRockFade(out),
            NativeWeldWorldActor::HailRockFade(first),
            NativeWeldWorldActor::HailRockFade(next),
        ) => out.position = lerp_vec(first.position, next.position, blend),
        (
            NativeWeldWorldActor::Meteor(out),
            NativeWeldWorldActor::Meteor(first),
            NativeWeldWorldActor::Meteor(next),
        ) => {
            out.position = lerp_vec(first.position, next.position, blend);
            out.body_scale = lerp(first.body_scale, next.body_scale, blend);
            out.fall_height = lerp(first.fall_height, next.fall_height, blend);
        }
        (
            NativeWeldWorldActor::Persistent(out),
            NativeWeldWorldActor::Persistent(first),
            NativeWeldWorldActor::Persistent(next),
        ) => persistent(out, first, next, blend, clocks),
        _ => {}
    }
}

fn nullable_vec(first: Option<Vec2>, next: Option<Vec2>, blend: f64) -> Option<Vec2> {
    match (first, next) {
        (Some(first), Some(next)) => Some(lerp_vec(first, next, blend)),
        _ if blend < 1.0 => first,
        _ => next,
    }
}

fn hail_child(
    output: &mut NativeWeldHailChildActorState,
    first: &NativeWeldHailChildActorState,
    next: &NativeWeldHailChildActorState,
    blend: f64,
) {
    match (output, first, next) {
        (
            NativeWeldHailChildActorState::Flash(out),
            NativeWeldHailChildActorState::Flash(first),
            NativeWeldHailChildActorState::Flash(next),
        ) => {
            out.position = lerp_vec(first.position, next.position, blend);
            out.alpha = lerp(first.alpha, next.alpha, blend);
        }
        (
            NativeWeldHailChildActorState::Line(out),
            NativeWeldHailChildActorState::Line(first),
            NativeWeldHailChildActorState::Line(next),
        ) => {
            out.alpha = lerp(first.alpha, next.alpha, blend);
            out.end = lerp_vec(first.end, next.end, blend);
            out.start = lerp_vec(first.start, next.start, blend);
        }
        (
            NativeWeldHailChildActorState::TerrainParticle(out),
            NativeWeldHailChildActorState::TerrainParticle(first),
            NativeWeldHailChildActorState::TerrainParticle(next),
        ) => {
            out.position = lerp_vec(first.position, next.position, blend);
            out.alpha = lerp(first.alpha, next.alpha, blend);
            out.velocity = lerp_vec(first.velocity, next.velocity, blend);
        }
        (
            NativeWeldHailChildActorState::TerrainBouncer(out),
            NativeWeldHailChildActorState::TerrainBouncer(first),
            NativeWeldHailChildActorState::TerrainBouncer(next),
        ) => {
            out.position = lerp_vec(first.position, next.position, blend);
            out.alpha = lerp(first.alpha, next.alpha, blend);
            out.height = lerp(first.height, next.height, blend);
            out.rotation_degrees =
                lerp_degrees(first.rotation_degrees, next.rotation_degrees, blend);
            out.velocity = lerp_vec(first.velocity, next.velocity, blend);
        }
        _ => {}
    }
}

fn persistent(
    output: &mut NativeWeldPersistentActorState,
    first: &NativeWeldPersistentActorState,
    next: &NativeWeldPersistentActorState,
    blend: f64,
    clocks: &mut PresentationClocks,
) {
    match (output, first, next) {
        (
            NativeWeldPersistentActorState::EtherealBoulder(out),
            NativeWeldPersistentActorState::EtherealBoulder(first),
            NativeWeldPersistentActorState::EtherealBoulder(next),
        ) => {
            out.scale = lerp(first.scale, next.scale, blend);
            out.shell_scale = lerp(first.shell_scale, next.shell_scale, blend);
            out.velocity = lerp_vec(first.velocity, next.velocity, blend);
        }
        (
            NativeWeldPersistentActorState::Hailstones(out),
            NativeWeldPersistentActorState::Hailstones(first),
            NativeWeldPersistentActorState::Hailstones(next),
        ) => {
            let age = lerp_option(
                first.release_age_ticks.map(|age| age as f64),
                next.release_age_ticks.map(|age| age as f64),
                blend,
            );
            out.release_age_ticks = age.map(|age| age.floor() as u64);
            if let Some(age) = age {
                clocks.primary_release.insert(out.common.owner.id, age);
            }
            out.release_fade_scale =
                lerp_option(first.release_fade_scale, next.release_fade_scale, blend);
            out.scale = lerp(first.scale, next.scale, blend);
        }
        _ => {}
    }
}
