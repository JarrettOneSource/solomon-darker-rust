//! Field selection from `client/primary-spell-transient-interpolation.ts`.

use solomon_sim::harden_effects::NativeHardenEffect;
use solomon_sim::spells::model::{PrimarySpellTransient, PrimaryTransientRef};
use solomon_sim::spells::staff::{
    NativePlayerStaffAction, NativePlayerStaffTransient, NativePlayerStaffVfx, StaffActionRef,
    StaffTransientRef, StaffVfxRef,
};

use crate::presentation::PresentationClocks;
use crate::presentation::math::{lerp, lerp_degrees, lerp_forward_cycle, lerp_vec};

pub(super) fn project(
    first: PrimaryTransientRef<'_>,
    next: PrimaryTransientRef<'_>,
    blend: f64,
    clocks: &mut PresentationClocks,
) -> PrimarySpellTransient {
    let mut output = if blend < 1.0 { first.snapshot() } else { next.snapshot() };
    if !same_kind(first, next) {
        return output;
    }
    let age = lerp(first.common().age_ticks as f64, next.common().age_ticks as f64, blend);
    output.common_mut().age_ticks = age.floor() as u64;
    clocks.primary.insert(output.common().id, age);
    match (&mut output, first, next) {
        (
            PrimarySpellTransient::EarthBoulderBit(out),
            PrimaryTransientRef::EarthBoulderBit(first),
            PrimaryTransientRef::EarthBoulderBit(next),
        ) => {
            out.origin = lerp_vec(first.origin, next.origin, blend);
            out.position = lerp_vec(first.position, next.position, blend);
        }
        (
            PrimarySpellTransient::EarthImpact(out),
            PrimaryTransientRef::EarthImpact(first),
            PrimaryTransientRef::EarthImpact(next),
        ) => out.origin = lerp_vec(first.origin, next.origin, blend),
        (
            PrimarySpellTransient::EarthCalledRock(out),
            PrimaryTransientRef::EarthCalledRock(first),
            PrimaryTransientRef::EarthCalledRock(next),
        ) => {
            out.fall_velocity = lerp(first.fall_velocity, next.fall_velocity, blend);
            out.height = lerp(first.height, next.height, blend);
            out.position = lerp_vec(first.position, next.position, blend);
            out.rotation = lerp(first.rotation, next.rotation, blend);
            out.speed = lerp(first.speed, next.speed, blend);
        }
        (
            PrimarySpellTransient::EtherBlast(out),
            PrimaryTransientRef::EtherBlast(first),
            PrimaryTransientRef::EtherBlast(next),
        ) => out.origin = lerp_vec(first.origin, next.origin, blend),
        (
            PrimarySpellTransient::FireEmber(out),
            PrimaryTransientRef::FireEmber(first),
            PrimaryTransientRef::FireEmber(next),
        ) => {
            out.height = lerp(first.height, next.height, blend);
            out.horizontal_velocity =
                lerp_vec(first.horizontal_velocity, next.horizontal_velocity, blend);
            out.life = lerp(first.life, next.life, blend);
            out.phase = lerp_forward_cycle(first.phase, next.phase, blend, 4.0);
            out.position = lerp_vec(first.position, next.position, blend);
            out.vertical_velocity = lerp(first.vertical_velocity, next.vertical_velocity, blend);
        }
        (
            PrimarySpellTransient::FireGoodImp(out),
            PrimaryTransientRef::FireGoodImp(first),
            PrimaryTransientRef::FireGoodImp(next),
        ) => {
            out.body_rotation_degrees =
                lerp(first.body_rotation_degrees, next.body_rotation_degrees, blend);
            out.body_scale = lerp(first.body_scale, next.body_scale, blend);
            if let (Some(first), Some(next)) = (first.contact_age_ticks, next.contact_age_ticks) {
                let age = lerp(first as f64, next as f64, blend);
                out.contact_age_ticks = Some(age.floor() as u64);
                clocks.primary_contact.insert(out.common.id, age);
            }
            out.contact_scale = lerp(first.contact_scale, next.contact_scale, blend);
            out.effect_alpha = lerp(first.effect_alpha, next.effect_alpha, blend);
            out.effect_phase = lerp(first.effect_phase, next.effect_phase, blend);
            out.flight_speed = lerp(first.flight_speed, next.flight_speed, blend);
            out.heading_degrees = lerp(first.heading_degrees, next.heading_degrees, blend);
            out.light_glow = lerp(first.light_glow, next.light_glow, blend);
            out.position = lerp_vec(first.position, next.position, blend);
            let remaining = lerp(first.remaining_ticks as f64, next.remaining_ticks as f64, blend);
            out.remaining_ticks = remaining.floor() as u64;
            clocks.primary_remaining.insert(out.common.id, remaining);
            out.vertical_offset = lerp(first.vertical_offset, next.vertical_offset, blend);
            out.vertical_velocity = lerp(first.vertical_velocity, next.vertical_velocity, blend);
        }
        (
            PrimarySpellTransient::AirHurricane(out),
            PrimaryTransientRef::AirHurricane(first),
            PrimaryTransientRef::AirHurricane(next),
        ) => {
            out.charge = lerp(first.charge, next.charge, blend);
            out.contact_charge = lerp(first.contact_charge, next.contact_charge, blend);
            for (index, lane) in out.lanes.iter_mut().enumerate() {
                lane.angle_degrees =
                    lerp(first.lanes[index].angle_degrees, next.lanes[index].angle_degrees, blend);
            }
            out.phase_degrees = lerp(first.phase_degrees, next.phase_degrees, blend);
            out.position = lerp_vec(first.position, next.position, blend);
        }
        (
            PrimarySpellTransient::Water(out),
            PrimaryTransientRef::Water(first),
            PrimaryTransientRef::Water(next),
        ) => {
            out.direction = lerp_vec(first.direction, next.direction, blend);
            out.origin = lerp_vec(first.origin, next.origin, blend);
        }
        (
            PrimarySpellTransient::WaterHail(out),
            PrimaryTransientRef::WaterHail(first),
            PrimaryTransientRef::WaterHail(next),
        ) => {
            out.height = lerp(first.height, next.height, blend);
            out.horizontal_velocity =
                lerp_vec(first.horizontal_velocity, next.horizontal_velocity, blend);
            out.life = lerp(first.life, next.life, blend);
            out.position = lerp_vec(first.position, next.position, blend);
            out.rotation_degrees = lerp(first.rotation_degrees, next.rotation_degrees, blend);
            out.vertical_velocity = lerp(first.vertical_velocity, next.vertical_velocity, blend);
        }
        (
            PrimarySpellTransient::WaterAura(out),
            PrimaryTransientRef::WaterAura(first),
            PrimaryTransientRef::WaterAura(next),
        ) => out.origin = lerp_vec(first.origin, next.origin, blend),
        (
            PrimarySpellTransient::Harden(out),
            PrimaryTransientRef::Harden(first),
            PrimaryTransientRef::Harden(next),
        ) => harden(out, first, next, blend),
        (
            PrimarySpellTransient::Staff(out),
            PrimaryTransientRef::Staff(first),
            PrimaryTransientRef::Staff(next),
        ) => staff(out, first, next, blend),
        (
            PrimarySpellTransient::Weld(out),
            PrimaryTransientRef::Weld(first),
            PrimaryTransientRef::Weld(next),
        ) => super::weld::project(out, first, next, blend, clocks),
        _ => {}
    }
    output
}

fn same_kind(first: PrimaryTransientRef<'_>, next: PrimaryTransientRef<'_>) -> bool {
    if std::mem::discriminant(&first) != std::mem::discriminant(&next) {
        return false;
    }
    match (first, next) {
        (PrimaryTransientRef::Weld(first), PrimaryTransientRef::Weld(next)) => {
            super::weld::same_kind(first, next)
        }
        (PrimaryTransientRef::Staff(first), PrimaryTransientRef::Staff(next)) => {
            first.kind() == next.kind()
        }
        (PrimaryTransientRef::Harden(first), PrimaryTransientRef::Harden(next)) => {
            std::mem::discriminant(first) == std::mem::discriminant(next)
        }
        _ => true,
    }
}

fn harden(
    output: &mut NativeHardenEffect,
    first: &NativeHardenEffect,
    next: &NativeHardenEffect,
    blend: f64,
) {
    match (output, first, next) {
        (
            NativeHardenEffect::Burst { alpha, position, .. },
            NativeHardenEffect::Burst { alpha: first_alpha, position: first_position, .. },
            NativeHardenEffect::Burst { alpha: next_alpha, position: next_position, .. },
        ) => {
            *alpha = lerp(*first_alpha, *next_alpha, blend);
            *position = lerp_vec(*first_position, *next_position, blend);
        }
        (
            NativeHardenEffect::Shard { body, .. },
            NativeHardenEffect::Shard { body: first, .. },
            NativeHardenEffect::Shard { body: next, .. },
        ) => {
            body.height = lerp(first.height, next.height, blend);
            body.life = lerp(first.life, next.life, blend);
            body.position = lerp_vec(first.position, next.position, blend);
            body.rotation_degrees = lerp(first.rotation_degrees, next.rotation_degrees, blend);
            body.velocity = lerp_vec(first.velocity, next.velocity, blend);
            body.vertical_velocity = lerp(first.vertical_velocity, next.vertical_velocity, blend);
        }
        _ => {}
    }
}

fn staff(
    output: &mut NativePlayerStaffTransient,
    first: StaffTransientRef<'_>,
    next: StaffTransientRef<'_>,
    blend: f64,
) {
    match (output, first, next) {
        (
            NativePlayerStaffTransient::Action(NativePlayerStaffAction::Melee(out)),
            StaffTransientRef::Action(StaffActionRef::Melee(first)),
            StaffTransientRef::Action(StaffActionRef::Melee(next)),
        ) => {
            out.base.heading_degrees =
                lerp_degrees(first.base.heading_degrees, next.base.heading_degrees, blend);
            out.base.origin = lerp_vec(first.base.origin, next.base.origin, blend);
            out.progress = lerp(first.progress, next.progress, blend);
        }
        (
            NativePlayerStaffTransient::Action(NativePlayerStaffAction::Spin(out)),
            StaffTransientRef::Action(StaffActionRef::Spin(first)),
            StaffTransientRef::Action(StaffActionRef::Spin(next)),
        ) => {
            out.base.heading_degrees =
                lerp_degrees(first.base.heading_degrees, next.base.heading_degrees, blend);
            out.base.origin = lerp_vec(first.base.origin, next.base.origin, blend);
            out.countdown = lerp(first.countdown, next.countdown, blend);
        }
        (
            NativePlayerStaffTransient::Contact(out),
            StaffTransientRef::Contact(first),
            StaffTransientRef::Contact(next),
        ) => out.origin = lerp_vec(first.origin, next.origin, blend),
        (
            NativePlayerStaffTransient::Knockback(out),
            StaffTransientRef::Knockback(first),
            StaffTransientRef::Knockback(next),
        ) => {
            out.remaining_distance = lerp(first.remaining_distance, next.remaining_distance, blend)
        }
        (
            NativePlayerStaffTransient::Vfx(NativePlayerStaffVfx::Smoke(out)),
            StaffTransientRef::Vfx(StaffVfxRef::Smoke(first)),
            StaffTransientRef::Vfx(StaffVfxRef::Smoke(next)),
        ) => {
            out.alpha = lerp(first.alpha, next.alpha, blend);
            out.position = lerp_vec(first.position, next.position, blend);
            out.rotation_degrees =
                lerp_degrees(first.rotation_degrees, next.rotation_degrees, blend);
        }
        (
            NativePlayerStaffTransient::Vfx(NativePlayerStaffVfx::MoveFade(out)),
            StaffTransientRef::Vfx(StaffVfxRef::MoveFade(first)),
            StaffTransientRef::Vfx(StaffVfxRef::MoveFade(next)),
        ) => {
            out.alpha = lerp(first.alpha, next.alpha, blend);
            out.position = lerp_vec(first.position, next.position, blend);
            out.velocity = lerp_vec(first.velocity, next.velocity, blend);
        }
        (
            NativePlayerStaffTransient::Vfx(NativePlayerStaffVfx::PerspectiveFade(out)),
            StaffTransientRef::Vfx(StaffVfxRef::PerspectiveFade(first)),
            StaffTransientRef::Vfx(StaffVfxRef::PerspectiveFade(next)),
        ) => out.alpha = lerp(first.alpha, next.alpha, blend),
        _ => {}
    }
}
