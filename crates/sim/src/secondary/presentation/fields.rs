//! Leviathan composites, Plane Orb meshes, Prismatic children and Magic Circle.

use super::draws::*;
use super::types::*;
use crate::Vec2;
use crate::float;
use crate::float::fround;
use crate::secondary::{
    NativeSecondaryActorKind, NativeSecondaryActorState, NativeSecondaryPlayerState,
};

#[must_use]
pub fn native_secondary_composite_owner_entries(
    actors: &[NativeSecondaryActorState],
    world_key: &str,
) -> Vec<(u64, u64)> {
    let mut owners = Vec::new();
    for actor in actors {
        if actor.world_key != world_key
            || actor.kind != NativeSecondaryActorKind::LeviathanAppendage
        {
            continue;
        }
        if let Some(parent) = actor.hit_target_ids.first()
            && actors
                .iter()
                .any(|candidate| candidate.id == *parent && candidate.world_key == world_key)
        {
            owners.push((actor.id, *parent));
        }
    }
    owners
}

#[must_use]
pub fn native_leviathan_composite_plan(scale: f64) -> NativeLeviathanCompositePlan {
    NativeLeviathanCompositePlan {
        clear: NativeLeviathanCompositeClear {
            color: 0,
            height: NATIVE_LEVIATHAN_RENDER_TARGET_SIZE / 2.0,
            width: NATIVE_LEVIATHAN_RENDER_TARGET_SIZE,
            x: 0.0,
            y: NATIVE_LEVIATHAN_RENDER_TARGET_SIZE / 2.0 + 32.0 * scale,
        },
        mask: NativeLeviathanCompositeMask {
            clip_size: 1000.0,
            clip_top: NATIVE_LEVIATHAN_RENDER_TARGET_SIZE / 2.0,
            entry: 39,
            scale,
        },
        outputs: [
            NativeLeviathanCompositeOutput { alpha: 1.0, blend: NativeSecondaryBlend::Normal },
            NativeLeviathanCompositeOutput { alpha: 0.5, blend: NativeSecondaryBlend::Add },
        ],
    }
}

#[must_use]
pub fn native_player_magic_shield_plan(
    player: Option<&NativeSecondaryPlayerState>,
    tick: f64,
) -> NativePlayerMagicShieldPlan {
    let Some(player) = player.filter(|player| player.magic_shield_absorb > 0.0) else {
        return NativePlayerMagicShieldPlan {
            alpha: 0.0,
            scale: NATIVE_PLAYER_MAGIC_SHIELD_SCALE,
            tint: WHITE,
            visible: false,
        };
    };
    let pulse = player.magic_shield_pulse_ticks as f64 * 0.05;
    NativePlayerMagicShieldPlan {
        alpha: 0.5 * (pulse.max(1.0) - 1.0) + 0.25,
        scale: fround(
            NATIVE_PLAYER_MAGIC_SHIELD_SCALE
                + (0.1_f32 as f64)
                    * float::sin(tick * 20.0 * std::f64::consts::PI / 180.0)
                    * pulse.min(1.0),
        ),
        tint: WHITE,
        visible: true,
    }
}

#[must_use]
pub fn plane_orb_mesh(
    actor: &NativeSecondaryActorState,
    frame: f64,
    enhanced_effects: bool,
) -> NativeSecondaryMeshDraw {
    let count = if enhanced_effects { 15 } else { 7 };
    let mut vertices = vec![0.0, 0.0];
    let mut uvs = vec![actor.position.x / 192.0, actor.position.y / 192.0];
    let mut indices = Vec::new();
    let mut colors = vec![0xffffffff];
    for segment in 0..count {
        let heading = frame % 360.0 + f64::from(segment) * 360.0 / f64::from(count);
        let radians = degrees_to_radians(heading);
        let x = fround(float::sin(radians));
        let y = fround(-float::cos(radians));
        for (index, radius) in [25.0 * actor.scale, 50.0 * actor.scale].into_iter().enumerate() {
            let local_x = fround(x * radius);
            let local_y = fround(y * radius * 0.8);
            vertices.extend([local_x, local_y]);
            colors.push(if index == 0 { 0xffffffff } else { 0 });
            uvs.extend([
                fround((actor.position.x + local_x) / 192.0),
                fround((actor.position.y + local_y) / 192.0),
            ]);
        }
    }
    for segment in 0..count {
        let inner = 1 + segment * 2;
        let outer = inner + 1;
        let next = 1 + ((segment + 1) % count) * 2;
        indices.extend([0, inner, next, inner, outer, next, outer, next, next + 1]);
    }
    NativeSecondaryMeshDraw {
        alpha: actor.alpha,
        blend: NativeSecondaryBlend::Normal,
        indices,
        role: "plane-orb-ether-plane-mesh".into(),
        texture: NativeSecondaryMeshTexture::EtherPlane,
        tint: WHITE,
        uvs,
        vertex_colors: colors,
        vertices,
    }
}

#[must_use]
pub fn prismatic_wave_draws(
    actor: &NativeSecondaryActorState,
    frame: f64,
) -> Vec<NativeSecondarySpriteDraw> {
    prismatic_wave_draws_at_age(actor, actor.age_ticks, frame)
}

pub(super) fn prismatic_wave_draws_at_age(
    actor: &NativeSecondaryActorState,
    age: u64,
    frame: f64,
) -> Vec<NativeSecondarySpriteDraw> {
    let source = actor
        .presentation_rng
        .as_ref()
        .expect("Prismatic presentation retains its post-cast RNG state");
    let completed = age.min(100);
    let initial = fround(actor.phase - actor.slow_factor * 6.0 * completed as f64);
    let mut draws = Vec::new();
    if age < 100 {
        let mut rng = source.clone();
        super::super::program::advance_rng(
            &mut rng,
            (completed * 19 + frame.trunc().max(0.0) as u64) as usize,
        );
        let flicker = rng.draw_scaled_float(0.5, false);
        let mut draw = NativeSecondarySpriteDraw::new(actor, NativeSecondaryAtlas::BadGuys, 58);
        draw.alpha = fround(0.5 * actor.alpha * fround(0.5 + flicker));
        draw.blend = NativeSecondaryBlend::Add;
        draw.role = "prismatic-spray-core".into();
        draw.rotation_radians = degrees_to_radians(actor.phase * actor.slow_factor);
        draw.scale_x = actor.slow_factor * actor.scale * 1.5;
        draw.scale_y = actor.scale * 1.2;
        draws.push(draw);
    }
    let first = age.saturating_sub(66).max(1);
    let last = age.min(100);
    if first > last {
        return draws;
    }
    let mut rng = source.clone();
    super::super::program::advance_rng(&mut rng, ((first - 1) * 19) as usize);
    const TINTS: [u32; 5] = [0xff8080, 0xffbf80, 0xffff80, 0x80ff80, 0x80ffff];
    for emission in first..=last {
        rng.draw_scaled_float(5.0, true);
        let mut radius = 2.0;
        for tick in 1..=emission {
            radius =
                fround(radius + if tick <= 50 { 0.065_f32 as f64 } else { -(0.075_f32 as f64) });
        }
        let heading = fround(initial + actor.slow_factor * 6.0 * emission as f64);
        let radians = degrees_to_radians(heading);
        let direction = Vec2::new(fround(float::sin(radians)), fround(-float::cos(radians)));
        let elapsed = age - emission;
        for child in 0..2 {
            let color = rng.draw_integer(5) as usize;
            let distance = rng.draw_scaled_float(radius * 60.0, false);
            let rotation = rng.draw_scaled_float(360.0, false);
            let scale = rng.draw_scaled_float(0.75, false);
            let life = rng.draw_scaled_float(1.0, false);
            let alpha = repeated_float_decay(fround(0.25 + life), 0.025, elapsed);
            if alpha <= 0.0 {
                continue;
            }
            let distance = fround(radius * 30.0 + distance);
            let scale = fround(0.25 + scale);
            let mut draw =
                NativeSecondarySpriteDraw::new(actor, NativeSecondaryAtlas::BadGuys, 111);
            draw.alpha = alpha.min(1.0);
            draw.blend = NativeSecondaryBlend::Add;
            draw.offset = Vec2::new(fround(direction.x * distance), fround(direction.y * distance));
            draw.role = format!("prismatic-radial-{emission}-{child}");
            draw.rotation_radians = degrees_to_radians(rotation);
            draw.scale_x = scale;
            draw.scale_y = scale;
            draw.tint = TINTS[color];
            draws.push(draw);
        }
        let color = rng.draw_integer(5) as usize;
        let distance = rng.draw_scaled_float(radius * 30.0, false);
        let selector = rng.draw_integer(2);
        let rotation = rng.draw_scaled_float(360.0, false);
        let scale = rng.draw_scaled_float(2.0, false);
        let speed = rng.draw_scaled_float(0.85, false);
        let life = rng.draw_scaled_float(0.5, false);
        let alpha = repeated_float_decay(fround(0.5 + life), 0.015, elapsed);
        if alpha <= 0.0 {
            continue;
        }
        let distance = fround(radius * 50.0 + distance);
        let speed = fround(0.15 + speed);
        let scale = fround(1.0 + scale);
        let mut draw = NativeSecondarySpriteDraw::new(
            actor,
            NativeSecondaryAtlas::BadGuys,
            if selector == 1 { 10 } else { 11 },
        );
        draw.alpha = alpha.min(1.0);
        draw.blend = NativeSecondaryBlend::Add;
        draw.offset = Vec2::new(
            fround(direction.x * fround(distance + speed * elapsed as f64)),
            fround(direction.y * fround(distance + speed * elapsed as f64)),
        );
        draw.role = format!("prismatic-moving-{emission}");
        draw.rotation_radians = degrees_to_radians(rotation);
        draw.scale_x = scale;
        draw.scale_y = fround(scale * 0.8);
        draw.tint = TINTS[color];
        draws.push(draw);
    }
    draws
}

#[must_use]
pub fn magic_circle_ring_draws(
    actor: &NativeSecondaryActorState,
) -> Vec<NativeSecondarySpriteDraw> {
    magic_circle_ring_draws_at_age(actor, actor.age_ticks)
}

pub(super) fn magic_circle_ring_draws_at_age(
    actor: &NativeSecondaryActorState,
    age: u64,
) -> Vec<NativeSecondarySpriteDraw> {
    let mut rng = actor
        .presentation_rng
        .clone()
        .expect("Magic Circle presentation retains its pre-tick RNG state");
    let last = age.min(1499);
    let first = last.saturating_sub(19);
    let base = actor.phase.trunc() as i64;
    let mut words = 0;
    for update in 0..first {
        words += 2 + (1 + ((base + update as i64) & 1)) * 5;
    }
    super::super::program::advance_rng(&mut rng, words as usize);
    let mut draws = Vec::new();
    for emission in first..=last {
        rng.draw_scaled_float(0.25, true);
        let global = base + emission as i64;
        let count = 1 + (global & 1);
        let remaining = (1499 - emission) as f64 / 100.0;
        let remaining = remaining.min(1.0);
        for child in 0..count {
            let life = rng.draw_scaled_float(0.5, false);
            let scale = rng.draw_scaled_float(0.025, false);
            let magnitude = rng.draw_scaled_float(1.0, false);
            let velocity = rng.draw_sign(fround(0.5 + magnitude));
            let mut rotation = rng.draw_scaled_float(360.0, false);
            let elapsed = age - emission;
            let alpha = repeated_float_decay(fround(remaining * fround(0.5 + life)), 0.05, elapsed);
            if alpha <= 0.0 {
                continue;
            }
            let factor = fround(0.975 + scale);
            for _ in 0..elapsed {
                rotation = fround(rotation + velocity);
            }
            let mut draw = NativeSecondarySpriteDraw::new(actor, NativeSecondaryAtlas::BadGuys, 48);
            draw.alpha = alpha.min(1.0);
            draw.blend = NativeSecondaryBlend::Add;
            draw.role = format!("magic-circle-ring-{emission}-{child}");
            draw.rotation_radians = degrees_to_radians(rotation);
            draw.scale_x = fround(actor.scale * factor);
            draw.scale_y = fround(fround(actor.scale * (0.8_f32 as f64)) * factor);
            draws.push(draw);
        }
    }
    draws
}
