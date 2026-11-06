//! Which fields of a Weld world actor, including Hail children and persistent actors,
//! blend between two snapshots.

use solomon_sim::Vec2;
use solomon_sim::spells::weld::{
    WeldHailChildActorState, WeldPersistentActorState, WeldWorldActor,
};

use crate::presentation::PresentationClocks;
use crate::presentation::math::{lerp, lerp_degrees, lerp_option, lerp_vec};

pub(super) fn same_kind(first: &WeldWorldActor, next: &WeldWorldActor) -> bool {
    if std::mem::discriminant(first) != std::mem::discriminant(next) {
        return false;
    }
    match (first, next) {
        (WeldWorldActor::HailChild(first), WeldWorldActor::HailChild(next)) => {
            std::mem::discriminant(first) == std::mem::discriminant(next)
        }
        (WeldWorldActor::Persistent(first), WeldWorldActor::Persistent(next)) => {
            std::mem::discriminant(first) == std::mem::discriminant(next)
        }
        _ => true,
    }
}

pub(super) fn project(
    output: &mut WeldWorldActor,
    first: &WeldWorldActor,
    next: &WeldWorldActor,
    blend: f64,
    clocks: &mut PresentationClocks,
) {
    output.common_mut().direction =
        lerp_vec(first.common().direction, next.common().direction, blend);
    output.common_mut().origin = lerp_vec(first.common().origin, next.common().origin, blend);
    match (output, first, next) {
        (
            WeldWorldActor::MeteorMarker(out),
            WeldWorldActor::MeteorMarker(first),
            WeldWorldActor::MeteorMarker(next),
        ) => {
            out.alpha = lerp(first.alpha, next.alpha, blend);
            out.rotation_degrees = lerp(first.rotation_degrees, next.rotation_degrees, blend);
            out.scale = lerp(first.scale, next.scale, blend);
        }
        (
            WeldWorldActor::Channel(out),
            WeldWorldActor::Channel(first),
            WeldWorldActor::Channel(next),
        ) => {
            out.endpoint = nullable_vec(first.endpoint, next.endpoint, blend);
            out.midpoint = nullable_vec(first.midpoint, next.midpoint, blend);
        }
        (
            WeldWorldActor::Impact(out),
            WeldWorldActor::Impact(first),
            WeldWorldActor::Impact(next),
        ) => {
            out.position = lerp_vec(first.position, next.position, blend);
            out.alpha = lerp(first.alpha, next.alpha, blend);
        }
        (
            WeldWorldActor::BoulderDebris(out),
            WeldWorldActor::BoulderDebris(first),
            WeldWorldActor::BoulderDebris(next),
        ) => out.position = lerp_vec(first.position, next.position, blend),
        (
            WeldWorldActor::FlameLashFade(out),
            WeldWorldActor::FlameLashFade(first),
            WeldWorldActor::FlameLashFade(next),
        ) => {
            out.position = lerp_vec(first.position, next.position, blend);
            out.alpha = lerp(first.alpha, next.alpha, blend);
        }
        (
            WeldWorldActor::GroundSparkFade(out),
            WeldWorldActor::GroundSparkFade(first),
            WeldWorldActor::GroundSparkFade(next),
        ) => {
            out.seed.position = lerp_vec(first.seed.position, next.seed.position, blend);
            out.seed.alpha = lerp(first.seed.alpha, next.seed.alpha, blend);
        }
        (
            WeldWorldActor::MeteorFlash(out),
            WeldWorldActor::MeteorFlash(first),
            WeldWorldActor::MeteorFlash(next),
        ) => {
            out.position = lerp_vec(first.position, next.position, blend);
            out.alpha = lerp(first.alpha, next.alpha, blend);
        }
        (
            WeldWorldActor::HailChild(out),
            WeldWorldActor::HailChild(first),
            WeldWorldActor::HailChild(next),
        ) => hail_child(out, first, next, blend),
        (WeldWorldActor::Steam(out), WeldWorldActor::Steam(first), WeldWorldActor::Steam(next)) => {
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
            WeldWorldActor::HailRockFade(out),
            WeldWorldActor::HailRockFade(first),
            WeldWorldActor::HailRockFade(next),
        ) => out.position = lerp_vec(first.position, next.position, blend),
        (
            WeldWorldActor::Meteor(out),
            WeldWorldActor::Meteor(first),
            WeldWorldActor::Meteor(next),
        ) => {
            out.position = lerp_vec(first.position, next.position, blend);
            out.body_scale = lerp(first.body_scale, next.body_scale, blend);
            out.fall_height = lerp(first.fall_height, next.fall_height, blend);
        }
        (
            WeldWorldActor::Persistent(out),
            WeldWorldActor::Persistent(first),
            WeldWorldActor::Persistent(next),
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
    output: &mut WeldHailChildActorState,
    first: &WeldHailChildActorState,
    next: &WeldHailChildActorState,
    blend: f64,
) {
    match (output, first, next) {
        (
            WeldHailChildActorState::Flash(out),
            WeldHailChildActorState::Flash(first),
            WeldHailChildActorState::Flash(next),
        ) => {
            out.position = lerp_vec(first.position, next.position, blend);
            out.alpha = lerp(first.alpha, next.alpha, blend);
        }
        (
            WeldHailChildActorState::Line(out),
            WeldHailChildActorState::Line(first),
            WeldHailChildActorState::Line(next),
        ) => {
            out.alpha = lerp(first.alpha, next.alpha, blend);
            out.end = lerp_vec(first.end, next.end, blend);
            out.start = lerp_vec(first.start, next.start, blend);
        }
        (
            WeldHailChildActorState::TerrainParticle(out),
            WeldHailChildActorState::TerrainParticle(first),
            WeldHailChildActorState::TerrainParticle(next),
        ) => {
            out.position = lerp_vec(first.position, next.position, blend);
            out.alpha = lerp(first.alpha, next.alpha, blend);
            out.velocity = lerp_vec(first.velocity, next.velocity, blend);
        }
        (
            WeldHailChildActorState::TerrainBouncer(out),
            WeldHailChildActorState::TerrainBouncer(first),
            WeldHailChildActorState::TerrainBouncer(next),
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
    output: &mut WeldPersistentActorState,
    first: &WeldPersistentActorState,
    next: &WeldPersistentActorState,
    blend: f64,
    clocks: &mut PresentationClocks,
) {
    match (output, first, next) {
        (
            WeldPersistentActorState::EtherealBoulder(out),
            WeldPersistentActorState::EtherealBoulder(first),
            WeldPersistentActorState::EtherealBoulder(next),
        ) => {
            out.scale = lerp(first.scale, next.scale, blend);
            out.shell_scale = lerp(first.shell_scale, next.shell_scale, blend);
            out.velocity = lerp_vec(first.velocity, next.velocity, blend);
        }
        (
            WeldPersistentActorState::Hailstones(out),
            WeldPersistentActorState::Hailstones(first),
            WeldPersistentActorState::Hailstones(next),
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
