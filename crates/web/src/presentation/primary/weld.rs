//! Weld display fields from `client/primary-spell-transient-interpolation.ts`.

use solomon_sim::Vec2;
use solomon_sim::spells::weld::{
    NativeWeldHailChildActorState, NativeWeldPersistentActorState, NativeWeldWorldActor,
    WeldHailChildRef, WeldPersistentRef, WeldWorldRef,
};

use crate::presentation::PresentationClocks;
use crate::presentation::math::{lerp, lerp_degrees, lerp_option, lerp_vec};

pub(super) fn same_kind(first: WeldWorldRef<'_>, next: WeldWorldRef<'_>) -> bool {
    if std::mem::discriminant(&first) != std::mem::discriminant(&next) {
        return false;
    }
    match (first, next) {
        (WeldWorldRef::HailChild(first), WeldWorldRef::HailChild(next)) => {
            std::mem::discriminant(&first) == std::mem::discriminant(&next)
        }
        (WeldWorldRef::Persistent(first), WeldWorldRef::Persistent(next)) => {
            std::mem::discriminant(&first) == std::mem::discriminant(&next)
        }
        _ => true,
    }
}

pub(super) fn project(
    output: &mut NativeWeldWorldActor,
    first: WeldWorldRef<'_>,
    next: WeldWorldRef<'_>,
    blend: f64,
    clocks: &mut PresentationClocks,
) {
    output.common_mut().direction =
        lerp_vec(first.common().direction, next.common().direction, blend);
    output.common_mut().origin = lerp_vec(first.common().origin, next.common().origin, blend);
    match (output, first, next) {
        (
            NativeWeldWorldActor::MeteorMarker(out),
            WeldWorldRef::MeteorMarker(first),
            WeldWorldRef::MeteorMarker(next),
        ) => {
            out.alpha = lerp(first.alpha, next.alpha, blend);
            out.rotation_degrees = lerp(first.rotation_degrees, next.rotation_degrees, blend);
            out.scale = lerp(first.scale, next.scale, blend);
        }
        (
            NativeWeldWorldActor::Channel(out),
            WeldWorldRef::Channel(first),
            WeldWorldRef::Channel(next),
        ) => {
            out.endpoint = nullable_vec(first.endpoint, next.endpoint, blend);
            out.midpoint = nullable_vec(first.midpoint, next.midpoint, blend);
        }
        (
            NativeWeldWorldActor::Impact(out),
            WeldWorldRef::Impact(first),
            WeldWorldRef::Impact(next),
        ) => {
            out.position = lerp_vec(first.position, next.position, blend);
            out.alpha = lerp(first.alpha, next.alpha, blend);
        }
        (
            NativeWeldWorldActor::BoulderDebris(out),
            WeldWorldRef::BoulderDebris(first),
            WeldWorldRef::BoulderDebris(next),
        ) => out.position = lerp_vec(first.position, next.position, blend),
        (
            NativeWeldWorldActor::FlameLashFade(out),
            WeldWorldRef::FlameLashFade(first),
            WeldWorldRef::FlameLashFade(next),
        ) => {
            out.position = lerp_vec(first.position, next.position, blend);
            out.alpha = lerp(first.alpha, next.alpha, blend);
        }
        (
            NativeWeldWorldActor::GroundSparkFade(out),
            WeldWorldRef::GroundSparkFade(first),
            WeldWorldRef::GroundSparkFade(next),
        ) => {
            out.seed.position = lerp_vec(first.seed.position, next.seed.position, blend);
            out.seed.alpha = lerp(first.seed.alpha, next.seed.alpha, blend);
        }
        (
            NativeWeldWorldActor::MeteorFlash(out),
            WeldWorldRef::MeteorFlash(first),
            WeldWorldRef::MeteorFlash(next),
        ) => {
            out.position = lerp_vec(first.position, next.position, blend);
            out.alpha = lerp(first.alpha, next.alpha, blend);
        }
        (
            NativeWeldWorldActor::HailChild(out),
            WeldWorldRef::HailChild(first),
            WeldWorldRef::HailChild(next),
        ) => hail_child(out, first, next, blend),
        (
            NativeWeldWorldActor::Steam(out),
            WeldWorldRef::Steam(first),
            WeldWorldRef::Steam(next),
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
            WeldWorldRef::HailRockFade(first),
            WeldWorldRef::HailRockFade(next),
        ) => out.position = lerp_vec(first.position, next.position, blend),
        (
            NativeWeldWorldActor::Meteor(out),
            WeldWorldRef::Meteor(first),
            WeldWorldRef::Meteor(next),
        ) => {
            out.position = lerp_vec(first.position, next.position, blend);
            out.body_scale = lerp(first.body_scale, next.body_scale, blend);
            out.fall_height = lerp(first.fall_height, next.fall_height, blend);
        }
        (
            NativeWeldWorldActor::Persistent(out),
            WeldWorldRef::Persistent(first),
            WeldWorldRef::Persistent(next),
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
    first: WeldHailChildRef<'_>,
    next: WeldHailChildRef<'_>,
    blend: f64,
) {
    match (output, first, next) {
        (
            NativeWeldHailChildActorState::Flash(out),
            WeldHailChildRef::Flash(first),
            WeldHailChildRef::Flash(next),
        ) => {
            out.position = lerp_vec(first.position, next.position, blend);
            out.alpha = lerp(first.alpha, next.alpha, blend);
        }
        (
            NativeWeldHailChildActorState::Line(out),
            WeldHailChildRef::Line(first),
            WeldHailChildRef::Line(next),
        ) => {
            out.alpha = lerp(first.alpha, next.alpha, blend);
            out.end = lerp_vec(first.end, next.end, blend);
            out.start = lerp_vec(first.start, next.start, blend);
        }
        (
            NativeWeldHailChildActorState::TerrainParticle(out),
            WeldHailChildRef::TerrainParticle(first),
            WeldHailChildRef::TerrainParticle(next),
        ) => {
            out.position = lerp_vec(first.position, next.position, blend);
            out.alpha = lerp(first.alpha, next.alpha, blend);
            out.velocity = lerp_vec(first.velocity, next.velocity, blend);
        }
        (
            NativeWeldHailChildActorState::TerrainBouncer(out),
            WeldHailChildRef::TerrainBouncer(first),
            WeldHailChildRef::TerrainBouncer(next),
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
    first: WeldPersistentRef<'_>,
    next: WeldPersistentRef<'_>,
    blend: f64,
    clocks: &mut PresentationClocks,
) {
    match (output, first, next) {
        (
            NativeWeldPersistentActorState::EtherealBoulder(out),
            WeldPersistentRef::EtherealBoulder(first),
            WeldPersistentRef::EtherealBoulder(next),
        ) => {
            out.scale = lerp(first.scale, next.scale, blend);
            out.shell_scale = lerp(first.shell_scale, next.shell_scale, blend);
            out.velocity = lerp_vec(first.velocity, next.velocity, blend);
        }
        (
            NativeWeldPersistentActorState::Hailstones(out),
            WeldPersistentRef::Hailstones(first),
            WeldPersistentRef::Hailstones(next),
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
