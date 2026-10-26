//! Skeleton fragments from Website `enemies/skeleton-death.ts`.

use super::config::{EnemyFamilyConfig, SkeletonWeapon};
use super::effects::{
    BouncerOptions, DeathEffectOverrides, DeathEffectOwner, SimpleDeathEffectSpec, radial_vector,
};
use super::model::*;
use super::world::EnemyWorld;
use crate::Vec2;
use crate::float::fround;
use crate::painter::WorldPainterOrder;

pub(super) const SKELETON_BASE_FRAGMENT_ENTRIES: [usize; 18] =
    [113, 113, 113, 115, 118, 121, 120, 119, 116, 121, 120, 119, 116, 117, 117, 117, 117, 117];

/// `skeletonDeathFragmentEntries`: FastCPU changes membership before any shuffle draw.
#[must_use]
pub fn skeleton_death_fragment_entries(enhanced_effects: bool) -> &'static [usize] {
    if enhanced_effects {
        &SKELETON_BASE_FRAGMENT_ENTRIES
    } else {
        &[113, 115, 118, 121, 120, 119, 116, 117, 117]
    }
}

impl EnemyWorld {
    pub(super) fn spawn_skeleton_shatter(
        &mut self,
        actor: &impl EnemyActorRead,
        tick: u64,
        enhanced_effects: bool,
        order: &mut WorldPainterOrder,
    ) {
        let owner = DeathEffectOwner::from(actor);
        let mut entries = skeleton_death_fragment_entries(enhanced_effects).to_vec();
        self.shuffle_bone_fragments(&mut entries);
        let mut angle = self.draw_unit() * 360.0;
        for entry in entries {
            self.spawn_skeleton_fragment(
                owner,
                tick,
                |_| entry,
                "skeleton-bone",
                |_| angle,
                FragmentClock { scale: 1.2, ..Default::default() },
                enhanced_effects,
                order,
            );
            angle += 72.0 + (self.draw_unit() * 20.0 - 10.0);
        }
        let headgear = match actor.config.family {
            EnemyFamilyConfig::Skeleton { headgear, .. }
            | EnemyFamilyConfig::Archer { headgear, .. }
            | EnemyFamilyConfig::Mage { headgear, .. } => headgear,
            _ => panic!("Skeleton shatter requires its family"),
        };
        if matches!(headgear, 1 | 2 | 4 | 5) {
            let first = 92 + usize::from(headgear - if headgear > 3 { 2 } else { 1 }) * 2;
            self.spawn_skeleton_fragment(
                owner,
                tick,
                |world| first + world.draw_integer(2) as usize,
                "skeleton-headgear-fragment",
                |world| world.draw_unit() * 360.0,
                FragmentClock { scale: 1.2, ..Default::default() },
                enhanced_effects,
                order,
            );
        }
        if let EnemyFamilyConfig::Skeleton { weapon, .. } = actor.config.family {
            let entry = match weapon {
                SkeletonWeapon::Axe => Some(2066),
                SkeletonWeapon::Flail => Some(2065),
                SkeletonWeapon::Mace => Some(2064),
                SkeletonWeapon::Sword => Some(2063),
                _ => None,
            };
            if let Some(entry) = entry {
                self.spawn_skeleton_fragment(
                    owner,
                    tick,
                    |_| entry,
                    "skeleton-weapon-fragment",
                    |world| world.draw_unit() * 360.0,
                    Default::default(),
                    enhanced_effects,
                    order,
                );
            } else if weapon == SkeletonWeapon::Pike {
                self.spawn_simple_death_effect(
                    owner,
                    tick,
                    SimpleDeathEffectSpec {
                        alpha: 1.0,
                        alpha_loss_per_tick: 1.0 / 25.0,
                        atlas: EnemyEffectAtlas::BadGuys,
                        blend_mode: EnemyEffectBlend::Add,
                        entry: 15,
                        kind: EnemyDeathEffectKind::Fade,
                        lifetime_ticks: 25,
                        role: "skeleton-pike-flash",
                        scale: 3.0,
                    },
                    Default::default(),
                    order,
                );
                for _ in 0..7 {
                    self.spawn_skeleton_fragment(
                        owner,
                        tick,
                        |_| 55,
                        "skeleton-pike-fragment",
                        |world| world.draw_unit() * 360.0,
                        FragmentClock { opacity_timer: 1.5, ..Default::default() },
                        enhanced_effects,
                        order,
                    );
                }
            }
        }
        self.spawn_bouncer(
            owner,
            tick,
            |world| 1819 + world.draw_integer(4) as usize,
            "skeleton-skull",
            |world| {
                let velocity = radial_vector(world.draw_unit() * 360.0, 2.0);
                BouncerOptions { velocity: Some(velocity), ..Default::default() }
            },
            enhanced_effects,
            order,
        );
        self.spawn_unbind(actor, tick, order);
    }

    pub(super) fn spawn_heartmonger_shatter(
        &mut self,
        actor: &impl EnemyActorRead,
        tick: u64,
        enhanced_effects: bool,
        order: &mut WorldPainterOrder,
    ) {
        let owner = DeathEffectOwner::from(actor);
        let mut entries = if enhanced_effects {
            vec![
                113, 113, 113, 113, 113, 115, 118, 121, 120, 119, 116, 121, 120, 119, 116, 121,
                120, 119, 116,
            ]
        } else {
            vec![113, 113, 113, 115, 118, 121, 120, 119, 116, 121, 120, 119, 116]
        };
        entries.extend(std::iter::repeat_n(117, if enhanced_effects { 11 } else { 6 }));
        self.shuffle_bone_fragments(&mut entries);
        let mut angle = self.draw_unit() * 360.0;
        for entry in entries {
            self.spawn_skeleton_fragment(
                owner,
                tick,
                |_| entry,
                "heartmonger-bone",
                |_| angle,
                FragmentClock {
                    scale: fround(fround(1.200_000_047_683_715_8) * 1.350_000_023_841_858),
                    opacity_timer: 15.0,
                    ..Default::default()
                },
                enhanced_effects,
                order,
            );
            angle = fround(angle + 72.0 + (self.draw_unit() * 2.0 - 1.0) * 10.0);
        }
        for _ in 0..20 {
            self.spawn_skeleton_fragment(
                owner,
                tick,
                |world| 172 + world.draw_integer(3) as usize,
                "heartmonger-splinter",
                |world| world.draw_unit() * 360.0,
                FragmentClock {
                    scale: 1.200_000_047_683_715_8,
                    opacity_timer: 15.0,
                    minimum_distance: 5.0,
                    distance_range: 30.0,
                },
                enhanced_effects,
                order,
            );
        }
        for _ in 0..7 {
            self.spawn_skeleton_fragment(
                owner,
                tick,
                |_| 29,
                "heartmonger-fragment",
                |world| world.draw_unit() * 360.0,
                FragmentClock {
                    scale: 1.200_000_047_683_715_8,
                    opacity_timer: 15.0,
                    minimum_distance: 5.0,
                    distance_range: 30.0,
                },
                enhanced_effects,
                order,
            );
        }
        let velocity = radial_vector(angle, 2.0);
        self.spawn_bouncer(
            owner,
            tick,
            |world| 1819 + world.draw_integer(4) as usize,
            "heartmonger-skull",
            |_| BouncerOptions {
                opacity_timer: Some(if enhanced_effects { 15.0 } else { 2.0 }),
                scale: Some(1.350_000_023_841_858),
                velocity: Some(velocity),
                ..Default::default()
            },
            enhanced_effects,
            order,
        );
        self.spawn_unbind(actor, tick, order);
        self.spawn_heartmonger_departure(actor, tick, order);
    }

    #[allow(clippy::too_many_arguments)] // Source fragment bank, angle writer and clock draw at different constructor boundaries.
    fn spawn_skeleton_fragment(
        &mut self,
        owner: DeathEffectOwner,
        tick: u64,
        entry: impl FnOnce(&mut Self) -> usize,
        role: &str,
        angle: impl FnOnce(&mut Self) -> f64,
        clock: FragmentClock,
        enhanced_effects: bool,
        order: &mut WorldPainterOrder,
    ) {
        self.spawn_bouncer(
            owner,
            tick,
            entry,
            role,
            |world| {
                let direction = radial_vector(angle(world), 1.0);
                let velocity = Vec2::new(direction.x * 1.5, direction.y);
                let distance =
                    fround(clock.minimum_distance + world.draw_unit() * clock.distance_range);
                BouncerOptions {
                    opacity_timer: Some(
                        if !enhanced_effects
                            && (clock.opacity_timer == 10.0 || clock.opacity_timer == 15.0)
                        {
                            2.0
                        } else {
                            clock.opacity_timer
                        },
                    ),
                    position: Some(Vec2::new(
                        owner.position.x + velocity.x * (distance + 2.0),
                        owner.position.y + velocity.y * distance,
                    )),
                    scale: Some(clock.scale),
                    velocity: Some(velocity),
                    ..Default::default()
                }
            },
            enhanced_effects,
            order,
        );
    }
    pub(super) fn spawn_skeleton_armor_break(
        &mut self,
        actor: &impl EnemyActorRead,
        tick: u64,
        enhanced_effects: bool,
        order: &mut WorldPainterOrder,
    ) {
        let mut pieces = [4, 5, 6, 7, 8];
        self.shuffle_bone_fragments(&mut pieces);
        let mut angle = self.draw_unit() * 360.0;
        let owner = DeathEffectOwner::from(actor);
        for piece in pieces {
            let entry = 92 + piece * 2 + self.draw_integer(2) as usize;
            self.spawn_bouncer(
                owner,
                tick,
                |_| entry,
                "skeleton-armor-fragment",
                |world| {
                    let direction = radial_vector(angle, 1.0);
                    let velocity = Vec2::new(fround(direction.x * 1.5), fround(direction.y));
                    let distance = fround(15.0 + world.draw_unit() * 10.0);
                    BouncerOptions {
                        bounce_velocity_multiplier: Some(1.2_f32 as f64),
                        position: Some(Vec2::new(
                            fround(
                                fround(actor.position.x + fround(distance * velocity.x))
                                    + velocity.x * 2.0,
                            ),
                            fround(actor.position.y + fround(distance * velocity.y)),
                        )),
                        velocity: Some(velocity),
                        ..BouncerOptions::default()
                    }
                },
                enhanced_effects,
                order,
            );
            angle = fround(angle + 72.0 + (self.draw_unit() * 2.0 - 1.0) * 10.0);
        }
        self.spawn_simple_death_effect(
            owner,
            tick,
            SimpleDeathEffectSpec {
                alpha: 1.0,
                alpha_loss_per_tick: 1.0 / 25.0,
                atlas: EnemyEffectAtlas::BadGuys,
                blend_mode: EnemyEffectBlend::Add,
                entry: 15,
                kind: EnemyDeathEffectKind::Fade,
                lifetime_ticks: 25,
                role: "skeleton-armor-flash",
                scale: 3.0,
            },
            DeathEffectOverrides {
                position: Some(Vec2::new(actor.position.x, actor.position.y - 35.0)),
                ..DeathEffectOverrides::default()
            },
            order,
        );
    }

    /// `Array<int>::Shuffle` uses a private signed 32-bit mixer after one actor-stream draw.
    pub(super) fn shuffle_bone_fragments(&mut self, entries: &mut [usize]) {
        let mut seed = self.draw_integer(100_000) as i32;
        for index in 0..entries.len() {
            seed ^= seed.wrapping_shl(21);
            seed ^= seed >> 11;
            let mixed = (seed ^ seed.wrapping_shl(4)).wrapping_mul(0x0a67_cfcf);
            let magnitude = mixed.unsigned_abs();
            seed = magnitude as i32;
            entries.swap(index, magnitude as usize % entries.len());
        }
    }
}

struct FragmentClock {
    scale: f64,
    opacity_timer: f64,
    minimum_distance: f64,
    distance_range: f64,
}
impl Default for FragmentClock {
    fn default() -> Self {
        Self { scale: 1.0, opacity_timer: 10.0, minimum_distance: 15.0, distance_range: 10.0 }
    }
}
