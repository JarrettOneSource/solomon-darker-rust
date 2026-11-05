//! Shared layer arithmetic from `renderer/native-enemy-layers.ts`.

use super::model::*;
use crate::Vec2;
use crate::enemies::EnemyToken;
use crate::enemies::model::EnemyActor;
use crate::float;

pub(super) fn facing(token: EnemyToken, heading: f64) -> usize {
    let (offset, step, count) = match token {
        EnemyToken::Coffin | EnemyToken::Portal => return 0,
        EnemyToken::DemonSkull => {
            return (((heading.round_ties_even() + 7.0) / 15.0).trunc() as i64).rem_euclid(24)
                as usize;
        }
        EnemyToken::Imp => (15.0, 30.0, 12),
        _ => (10.0, 20.0, 18),
    };
    (((heading + offset) / step).trunc() as i64).rem_euclid(count) as usize
}

pub(super) fn bank_pose(pose: f64, count: usize) -> Option<usize> {
    let pose = pose.floor();
    if pose >= 0.0 && pose < count as f64 { Some(pose as usize) } else { None }
}

pub(super) fn bounded_pose(pose: f64, maximum: usize) -> usize {
    pose.floor().clamp(0.0, maximum as f64) as usize
}

pub(super) fn point(
    art: &(impl EnemyArtPoints + ?Sized),
    atlas: EnemyArtAtlas,
    entry: usize,
    slot: usize,
) -> Vec2 {
    *art.points(atlas, entry)
        .get(slot)
        .unwrap_or_else(|| panic!("{}:{entry} is missing authored point {slot}", atlas.id()))
}

pub(super) fn rotate(point: Vec2, rotation: f64) -> Vec2 {
    let cosine = float::cos(rotation);
    let sine = float::sin(rotation);
    Vec2::new(point.x * cosine - point.y * sine, point.x * sine + point.y * cosine)
}

pub(super) fn midpoint(first: Vec2, second: Vec2) -> Vec2 {
    Vec2::new((first.x + second.x) / 2.0, (first.y + second.y) / 2.0)
}

pub(super) fn add_scaled(origin: Vec2, offset: Vec2, scale: f64) -> Vec2 {
    Vec2::new(origin.x + offset.x * scale, origin.y + offset.y * scale)
}

pub(super) fn radians(degrees: f64) -> f64 {
    degrees * std::f64::consts::PI / 180.0
}

pub(super) fn pack_rgb(red: f64, green: f64, blue: f64) -> u32 {
    let channel = |value: f64| (value.clamp(0.0, 1.0) * 255.0).trunc() as u32;
    (channel(red) << 16) | (channel(green) << 8) | channel(blue)
}

fn visual_word(actor: &EnemyActor, channel: u32, epoch: u32) -> u32 {
    let mut word = actor.id as u32
        ^ (actor.spawn_tick as u32).wrapping_add(1).wrapping_mul(0x9e3779b1)
        ^ channel.wrapping_add(1).wrapping_mul(0x85ebca6b)
        ^ epoch.wrapping_add(1).wrapping_mul(0xc2b2ae35);
    word ^= word >> 16;
    word = word.wrapping_mul(0x7feb352d);
    word ^= word >> 15;
    word = word.wrapping_mul(0x846ca68b);
    word ^ (word >> 16)
}

pub(super) fn stable_unit(actor: &EnemyActor, channel: u32, epoch: u32) -> f64 {
    f64::from(visual_word(actor, channel, epoch)) / 4_294_967_296.0
}

pub(super) fn stable_inclusive_unit(actor: &EnemyActor, channel: u32, epoch: u32) -> f64 {
    f64::from(visual_word(actor, channel, epoch)) / 4_294_967_295.0
}
