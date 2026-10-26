//! Reconstructs stock burst children from their captured birth RNG. These draws
//! never advance the live simulation's RNG or invent renderer-local actor clocks.

use super::draws::*;
use super::types::*;
use crate::float;
use crate::float::fround;
use crate::secondary::SecondaryActorRead;
use crate::{NativeRng, Vec2};

pub(super) const MAGIC_TRAP_FULL_DRAW_THRESHOLD: f64 = 0.99_f32 as f64;
pub(super) const FIRE_DRAW_SCALE: f64 = 1.1_f32 as f64;

pub(super) fn native_ether_fade_scalar(initial: f64, decrement: f64, age: f64) -> f64 {
    let mut life = fround(initial);
    for _ in 0..=age.floor().max(0.0) as u64 {
        life = fround(life - fround(decrement));
    }
    life.max(0.0)
}

#[must_use]
pub fn dampen_draws(actor: &impl SecondaryActorRead) -> Vec<NativeSecondarySpriteDraw> {
    dampen_draws_at_age(actor, actor.age_ticks)
}

pub(super) fn dampen_draws_at_age(
    actor: &impl SecondaryActorRead,
    age: u64,
) -> Vec<NativeSecondarySpriteDraw> {
    let mut rng = actor
        .presentation_rng
        .clone()
        .expect("Dampen presentation retains its post-gameplay RNG state");
    let mut draws = Vec::new();
    for heading in 0..360 {
        let record = rng.draw_integer(2);
        let speed = rng.draw_scaled_float(4.0, false);
        let drag = rng.draw_integer(6);
        let rotation = rng.draw_scaled_float(360.0, false);
        let scale = rng.draw_scaled_float(0.5, false);
        let loss = rng.draw_scaled_float(0.02, false);
        let gray = rng.draw_scaled_float(0.25, false);
        rng.draw_integer(5);
        let alpha = repeated_float_decay(1.0, fround(0.01 + loss), age);
        if alpha <= 0.0 || heading % 10 != 0 {
            continue;
        }
        let drag = if drag == 3 { 0.93_f32 as f64 } else { 0.96_f32 as f64 };
        let mut distance = 0.0;
        let mut velocity = fround(6.0 + speed);
        for _ in 0..age {
            distance = fround(distance + velocity);
            velocity = fround(velocity * drag);
        }
        let radians = degrees_to_radians(f64::from(heading));
        let scale = fround(1.5 + scale);
        let mut draw = NativeSecondarySpriteDraw::new(
            actor,
            NativeSecondaryAtlas::BadGuys,
            (10 + record) as usize,
        );
        draw.alpha = alpha;
        draw.offset = if distance == 0.0 {
            Vec2::ZERO
        } else {
            Vec2::new(
                fround(float::sin(radians) * distance),
                fround(-float::cos(radians) * distance),
            )
        };
        draw.role = format!("dampen-move-fade-{heading}");
        draw.rotation_radians = degrees_to_radians(rotation);
        draw.scale_x = scale;
        draw.scale_y = scale;
        draw.tint = pack_normalized_rgb(gray, gray, gray);
        draws.push(draw);
    }
    for index in 0..30 {
        let rotation = rng.draw_scaled_float(360.0, false);
        let scale = rng.draw_scaled_float(4.75, false);
        let life = rng.draw_scaled_float(1.0, false);
        let alpha = repeated_float_decay(fround(0.5 + life), 0.1, age);
        if alpha <= 0.0 || index % 10 != 0 {
            continue;
        }
        let scale = fround(0.75 + scale);
        let mut draw = NativeSecondarySpriteDraw::new(actor, NativeSecondaryAtlas::BadGuys, 48);
        draw.alpha = alpha.min(1.0);
        draw.blend = NativeSecondaryBlend::Add;
        draw.role = format!("dampen-additive-{index}");
        draw.rotation_radians = degrees_to_radians(rotation);
        draw.scale_x = scale;
        draw.scale_y = fround(scale * 0.8);
        draws.push(draw);
    }
    draws
}

#[must_use]
pub fn shield_explosion_draws(actor: &impl SecondaryActorRead) -> Vec<NativeSecondarySpriteDraw> {
    shield_explosion_draws_at_age(actor, actor.age_ticks)
}

pub(super) fn shield_explosion_draws_at_age(
    actor: &impl SecondaryActorRead,
    age: u64,
) -> Vec<NativeSecondarySpriteDraw> {
    let rng = actor
        .presentation_rng
        .as_ref()
        .expect("Explosive Shield presentation retains its construction RNG state");
    let mut draws = Vec::new();
    let flash = repeated_float_decay(1.0, 0.1, age);
    if flash > 0.0 {
        let mut draw = NativeSecondarySpriteDraw::new(actor, NativeSecondaryAtlas::BadGuys, 15);
        draw.alpha = flash.min(1.0);
        draw.offset = Vec2::new(0.0, -25.0);
        draw.role = "explosive-shield-center-flash".into();
        draw.scale_x = 12.0;
        draw.scale_y = 12.0;
        draws.push(draw);
    }
    let ring = repeated_float_decay(1.5, 0.05, age);
    if ring > 0.0 {
        let scale = repeated_float_multiply(2.5, 1.01, age);
        let mut draw = NativeSecondarySpriteDraw::new(actor, NativeSecondaryAtlas::Clothes, 2);
        draw.alpha = ring.min(1.0);
        draw.blend = NativeSecondaryBlend::Add;
        draw.offset = Vec2::new(0.0, -35.0);
        draw.role = "explosive-shield-expanding-ring".into();
        draw.scale_x = scale;
        draw.scale_y = scale;
        draws.push(draw);
    }
    draws.extend(fuzzy_spear_burst_draws(actor, age, rng, "explosive-shield", false));
    draws
}

#[must_use]
pub fn magic_trap_burst_draws(actor: &impl SecondaryActorRead) -> Vec<NativeSecondarySpriteDraw> {
    magic_trap_burst_draws_at_age(actor, actor.age_ticks)
}

pub(super) fn magic_trap_burst_draws_at_age(
    actor: &impl SecondaryActorRead,
    age: u64,
) -> Vec<NativeSecondarySpriteDraw> {
    let rng = actor
        .presentation_rng
        .as_ref()
        .expect("Magic Trap presentation retains its construction RNG state");
    let mut draws = Vec::new();
    let flash = repeated_float_decay(1.0, 0.1, age);
    if flash > 0.0 {
        let mut draw = NativeSecondarySpriteDraw::new(actor, NativeSecondaryAtlas::BadGuys, 15);
        draw.alpha = flash.min(1.0);
        draw.offset = Vec2::new(0.0, -25.0);
        draw.role = "magic-trap-center-flash".into();
        draw.scale_x = 6.0;
        draw.scale_y = 6.0;
        draws.push(draw);
    }
    draws.extend(fuzzy_spear_burst_draws(actor, age, rng, "magic-trap", false));
    draws
}

#[must_use]
pub fn mindblast_burst_draws(actor: &impl SecondaryActorRead) -> Vec<NativeSecondarySpriteDraw> {
    mindblast_burst_draws_at_age(actor, actor.age_ticks)
}

pub(super) fn mindblast_burst_draws_at_age(
    actor: &impl SecondaryActorRead,
    age: u64,
) -> Vec<NativeSecondarySpriteDraw> {
    let rng = actor
        .presentation_rng
        .as_ref()
        .expect("Mindblast presentation retains its construction RNG state");
    let mut draws = Vec::new();
    let core = repeated_float_decay(1.0, 0.025, age);
    if core > 0.0 {
        let mut draw = NativeSecondarySpriteDraw::new(actor, NativeSecondaryAtlas::BadGuys, 15);
        draw.alpha = core.min(1.0);
        draw.offset = Vec2::new(0.0, -25.0);
        draw.role = "mindblast-center-flash".into();
        draw.scale_x = 54.0;
        draw.scale_y = 54.0;
        draws.push(draw);
    }
    for (index, growth) in [1.1, 1.05, 1.025].into_iter().enumerate() {
        let alpha = repeated_float_decay(1.5, 0.025, age);
        if alpha <= 0.0 {
            continue;
        }
        let mut draw = NativeSecondarySpriteDraw::new(actor, NativeSecondaryAtlas::Clothes, 2);
        draw.alpha = alpha.min(1.0);
        draw.blend = NativeSecondaryBlend::Add;
        draw.offset = Vec2::new(0.0, -35.0);
        draw.role = format!("mindblast-expanding-ring-{index}");
        draw.scale_x = repeated_float_multiply(4.5, growth, age);
        draw.scale_y = draw.scale_x;
        draw.tint = 0x00ffff;
        draws.push(draw);
    }
    draws.extend(fuzzy_spear_burst_draws(actor, age, rng, "mindblast", true));
    draws
}

fn fuzzy_spear_burst_draws(
    actor: &impl SecondaryActorRead,
    age: u64,
    source: &NativeRng,
    prefix: &str,
    mindblast: bool,
) -> Vec<NativeSecondarySpriteDraw> {
    let mut rng = source.clone();
    let mut draws = Vec::new();
    for index in 0..2 {
        let rotation = rng.draw_scaled_float(360.0, false);
        let rate = if mindblast {
            fround(if index == 0 { 0.075 } else { 0.1125 })
        } else {
            fround(fround(f64::from(index) * 0.1 + 0.2) * 0.75)
        };
        let mut frame = 0.0;
        for _ in 0..age {
            frame = fround(frame + rate);
        }
        if frame >= 10.0 {
            continue;
        }
        let mut draw = NativeSecondarySpriteDraw::new(
            actor,
            NativeSecondaryAtlas::BadGuys,
            158 + frame.floor() as usize,
        );
        draw.alpha = 1.0;
        draw.blend = NativeSecondaryBlend::Add;
        if !mindblast {
            draw.offset = Vec2::new(0.0, -35.0);
        }
        draw.role = format!("{prefix}-sprite-array-{index}");
        draw.rotation_radians = degrees_to_radians(rotation);
        draw.scale_x = if mindblast { 10.0 } else { 6.0 };
        draw.scale_y = draw.scale_x;
        draws.push(draw);
    }
    for index in 0..100 {
        let heading = rng.draw_scaled_float(360.0, false);
        let speed = rng.draw_scaled_float(2.0, false);
        let double = rng.draw_integer(5);
        let alpha = rng.draw_scaled_float(1.0, false);
        let scale = rng.draw_scaled_float(1.5, false);
        let life =
            repeated_float_decay(fround(1.0 + alpha), if mindblast { 0.00875 } else { 0.035 }, age);
        if life <= 0.0 {
            continue;
        }
        let radians = degrees_to_radians(heading);
        let direction = Vec2::new(fround(float::sin(radians)), fround(-float::cos(radians)));
        let factor = if double == 2 { 2.0 } else { 1.0 };
        let mut velocity = Vec2::new(
            fround(direction.x * fround(3.0 + speed) * factor),
            fround(direction.y * fround(3.0 + speed) * factor),
        );
        let mut offset = Vec2::new(fround(direction.x * 75.0), fround(direction.y * 75.0));
        for _ in 0..age {
            offset = Vec2::new(fround(offset.x + velocity.x), fround(offset.y + velocity.y));
            velocity = Vec2::new(
                fround(velocity.x * (0.95_f32 as f64)),
                fround(velocity.y * (0.95_f32 as f64)),
            );
        }
        let sign = if hash_unit(actor.id + index, age * 101 + index) < 0.5 { -1.0 } else { 1.0 };
        let mut base = NativeSecondarySpriteDraw::new(actor, NativeSecondaryAtlas::BadGuys, 17);
        base.alpha = life.min(1.0);
        base.blend = NativeSecondaryBlend::Add;
        base.offset = offset;
        base.role = format!("{prefix}-fuzzy-spear-base-{index}");
        base.rotation_radians = radians;
        base.scale_x = sign;
        base.scale_y = 1.0;
        if mindblast {
            base.tint = 0x00ffff;
        }
        let mut glow = base.clone();
        glow.entry = 74;
        glow.role = format!("{prefix}-fuzzy-spear-glow-{index}");
        glow.scale_x = fround(2.0 + scale);
        glow.scale_y = glow.scale_x;
        draws.extend([base, glow]);
    }
    draws
}

pub(super) fn magic_trap_tint(selector: i32) -> u32 {
    let color = match selector {
        0 => [1.0, 0.1, 1.0],
        1 => [1.0, 0.35, 0.1],
        2 => [0.1, 1.0, 1.0],
        3 => [0.1, 0.5, 1.0],
        4 => [0.1, 1.0, 0.1],
        5 => [1.0, 0.5, 0.1],
        6 => [0.1, 0.5, 0.5],
        7 => [0.75, 0.75, 0.75],
        8 => [1.0, 1.0, 1.0],
        _ => panic!("invalid native Magic Trap selector {selector}"),
    };
    pack_normalized_rgb(color[0], color[1], color[2])
}
