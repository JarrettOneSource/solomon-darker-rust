//! Terminal family presentation from `enemies/deaths.ts` and `death-effects.ts`.

use super::EnemyToken;
use super::config::EnemyFamilyConfig;
use super::effects::*;
use super::model::*;
use super::skeleton_death::skeleton_death_fragment_entries;
use super::world::EnemyWorld;
use crate::Vec2;
use crate::float::fround;
use crate::painter::WorldPainterOrder;

impl EnemyWorld {
    pub(super) fn spawn_unbind(
        &mut self,
        actor: &impl EnemyActorRead,
        tick: u64,
        order: &mut WorldPainterOrder,
    ) {
        let (base_alpha, loss) = match actor.config.enemy_token {
            EnemyToken::Skeleton | EnemyToken::SkeletonArcher | EnemyToken::SkeletonMage => {
                (0.75, 0.0225)
            }
            EnemyToken::Imp | EnemyToken::Wraith => (1.0, 0.025),
            EnemyToken::Zombie => (0.75, 0.05),
            EnemyToken::Coffin => (0.75, 0.045),
            EnemyToken::Heartmonger | EnemyToken::DireFaculty => (0.75, fround(0.0225)),
            _ => panic!("this enemy family does not create Anim_Unbind"),
        };
        let alpha = if actor.lethal_magic_damage { 1.25 } else { base_alpha };
        let rotation = self.draw_unit() * 360.0;
        let angular_offset = self.draw_unit() * 2.5;
        let clockwise = self.draw_unit() >= 0.5;
        let boss =
            matches!(actor.config.enemy_token, EnemyToken::Heartmonger | EnemyToken::DireFaculty);
        self.spawn_simple_death_effect(
            DeathEffectOwner::from(actor),
            tick,
            SimpleDeathEffectSpec {
                alpha,
                alpha_loss_per_tick: loss,
                atlas: EnemyEffectAtlas::BadGuys,
                blend_mode: EnemyEffectBlend::Normal,
                entry: 86,
                kind: EnemyDeathEffectKind::Unbind,
                lifetime_ticks: (alpha / loss).ceil() as u64,
                role: "death-unbind-star",
                scale: if actor.config.enemy_token == EnemyToken::Heartmonger {
                    3.0
                } else if boss {
                    2.0
                } else {
                    1.0
                },
            },
            DeathEffectOverrides {
                angular_velocity_deg: Some(if clockwise {
                    5.0 + angular_offset
                } else {
                    -5.0 + angular_offset
                }),
                position: Some(Vec2::new(
                    actor.position.x + if boss { 0.0 } else { 1.0 },
                    actor.position.y - 15.0,
                )),
                presentation_owner: Some(DeathPresentationOwner::LateWorldOverlay),
                rotation_deg: Some(rotation),
                ..Default::default()
            },
            order,
        );
    }

    #[allow(clippy::too_many_arguments)] // The source array constructor keeps its authored bank and clock together.
    pub(super) fn spawn_sprite_array(
        &mut self,
        owner: DeathEffectOwner,
        tick: u64,
        role: &str,
        entry: usize,
        count: usize,
        frame_ticks: f64,
        scale: f64,
        mut options: DeathEffectOverrides,
        order: &mut WorldPainterOrder,
    ) {
        options.first_entry = Some(entry);
        options.frame_count = Some(count);
        options.frame_ticks = Some(frame_ticks);
        if options.frame_velocity.is_none() {
            options.frame_velocity = Some(1.0 / frame_ticks);
        }
        self.spawn_simple_death_effect(
            owner,
            tick,
            SimpleDeathEffectSpec {
                alpha: 1.0,
                alpha_loss_per_tick: 0.0,
                atlas: EnemyEffectAtlas::BadGuys,
                blend_mode: EnemyEffectBlend::Add,
                entry,
                kind: EnemyDeathEffectKind::SpriteArray,
                lifetime_ticks: 1000,
                role,
                scale,
            },
            options,
            order,
        );
    }

    pub(super) fn spawn_portal_terminal_effects(
        &mut self,
        actor: &impl EnemyActorRead,
        tick: u64,
        enhanced_effects: bool,
        order: &mut WorldPainterOrder,
    ) {
        let owner = DeathEffectOwner::from(actor);
        self.spawn_sprite_array(
            owner,
            tick,
            "portal-terminal-array",
            1823,
            11,
            1.0,
            2.0,
            DeathEffectOverrides {
                frame_velocity: Some(0.5),
                presentation_owner: Some(DeathPresentationOwner::PreWorldQueue),
                ..Default::default()
            },
            order,
        );
        for index in 0..12 {
            let velocity = radial_vector(f64::from(index) / 12.0 * 360.0, 1.0);
            self.spawn_bouncer(
                owner,
                tick,
                |_| 27,
                "portal-black-smoke",
                |_| BouncerOptions {
                    kind: BouncerKind::Smoky,
                    opacity_timer: Some(1.5),
                    tint: Some(0),
                    velocity: Some(velocity),
                    ..Default::default()
                },
                enhanced_effects,
                order,
            );
        }
        for entry in [120, 144] {
            self.spawn_simple_death_effect(
                owner,
                tick,
                SimpleDeathEffectSpec {
                    alpha: 0.75,
                    alpha_loss_per_tick: 0.025,
                    atlas: EnemyEffectAtlas::DeadHawg,
                    blend_mode: EnemyEffectBlend::Normal,
                    entry,
                    kind: EnemyDeathEffectKind::FadeScale,
                    lifetime_ticks: 30,
                    role: "portal-decal",
                    scale: 1.0,
                },
                Default::default(),
                order,
            );
        }
    }

    pub(super) fn spawn_zombie_terminal_effects(
        &mut self,
        actor: &impl EnemyActorRead,
        tick: u64,
        enhanced_effects: bool,
        order: &mut WorldPainterOrder,
    ) {
        let EnemyFamilyConfig::Zombie { rotten, .. } = actor.config.family else {
            panic!("Zombie terminal requires its family")
        };
        let owner = DeathEffectOwner::from(actor);
        if rotten {
            let count = 6 + self.draw_integer(5);
            for index in 0..count {
                let delay = 25 + self.draw_integer(76);
                let opacity = 3.0 + self.draw_unit() * 3.0;
                let rotation = self.draw_unit() * 360.0;
                let scale = 0.75 + self.draw_unit() * 0.75;
                let distance = 75.0 + self.draw_unit() * 75.0;
                let offset = radial_vector(self.draw_unit() * 360.0, distance);
                self.spawn_simple_death_effect(
                    owner,
                    tick,
                    SimpleDeathEffectSpec {
                        alpha: (opacity * 0.25).min(1.0),
                        alpha_loss_per_tick: 0.01,
                        atlas: EnemyEffectAtlas::DeadHawg,
                        blend_mode: EnemyEffectBlend::Normal,
                        entry: 31,
                        kind: EnemyDeathEffectKind::LateSplat,
                        lifetime_ticks: (opacity / 0.01).ceil() as u64,
                        role: &format!("zombie-late-splat:{index}"),
                        scale,
                    },
                    DeathEffectOverrides {
                        opacity_timer: Some(opacity),
                        position: Some(Vec2::new(
                            owner.position.x + offset.x,
                            owner.position.y + offset.y,
                        )),
                        presentation_owner: Some(DeathPresentationOwner::PreWorldQueue),
                        rotation_deg: Some(rotation),
                        spawn_delay_ticks: Some(u64::from(delay)),
                        ..Default::default()
                    },
                    order,
                );
            }
        }
        let mut entries = vec![2094, 2089, 2092, 2090, 2091];
        if enhanced_effects {
            entries.extend([2090, 2091, 2090, 2091, 2094]);
        }
        if rotten {
            entries.extend([2090, 2091, 2090, 2091, 2094, 2090, 2091, 2090, 2091, 2094]);
        }
        entries.extend([2093, 2093]);
        shuffle_actor_fragments(self, &mut entries);
        let mut angle = self.draw_unit() * 360.0;
        let scale = if rotten { 2.0 } else { 1.0 };
        for entry in entries {
            self.spawn_bouncer(
                owner,
                tick,
                |_| entry,
                "zombie-fragment",
                |world| {
                    let direction = radial_vector(angle, 1.0);
                    let velocity =
                        Vec2::new(direction.x * 1.5 * 0.75 * scale, direction.y * 0.75 * scale);
                    let distance = 5.0 + world.draw_unit() * 10.0;
                    BouncerOptions {
                        position: Some(Vec2::new(
                            owner.position.x + velocity.x * (distance + 2.0),
                            owner.position.y + velocity.y * distance,
                        )),
                        velocity: Some(velocity),
                        ..Default::default()
                    }
                },
                enhanced_effects,
                order,
            );
            angle += 72.0 + (self.draw_unit() * 20.0 - 10.0);
        }
        let velocity = radial_vector(angle, 2.0);
        self.spawn_bouncer(
            owner,
            tick,
            |world| 2365 + world.draw_integer(144) as usize,
            "zombie-gait-fragment",
            |_| BouncerOptions {
                bounce_retention: Some(0.5),
                velocity: Some(velocity),
                ..Default::default()
            },
            enhanced_effects,
            order,
        );
        self.spawn_unbind(actor, tick, order);
        let rotation = self.draw_unit() * 360.0;
        let scale = (1.0 + self.draw_unit() * 0.25) * 1.5;
        self.spawn_simple_death_effect(
            owner,
            tick,
            SimpleDeathEffectSpec {
                alpha: 0.6,
                alpha_loss_per_tick: if enhanced_effects { 0.01 } else { 0.02 },
                atlas: EnemyEffectAtlas::DeadHawg,
                blend_mode: EnemyEffectBlend::Normal,
                entry: 30,
                kind: EnemyDeathEffectKind::FadePerspectiveClipped,
                lifetime_ticks: if enhanced_effects { 1000 } else { 500 },
                role: "zombie-clipped-fade",
                scale,
            },
            DeathEffectOverrides {
                alpha_multiplier: Some(0.6),
                opacity_timer: Some(10.0),
                presentation_owner: Some(DeathPresentationOwner::Background),
                rotation_deg: Some(rotation),
                ..Default::default()
            },
            order,
        );
    }

    pub(super) fn spawn_wraith_terminal_effects(
        &mut self,
        actor: &impl EnemyActorRead,
        tick: u64,
        enhanced_effects: bool,
        order: &mut WorldPainterOrder,
    ) {
        let owner = DeathEffectOwner::from(actor);
        let mut angle = self.draw_unit() * 360.0;
        for index in 0..12 {
            let entry = if self.draw_integer(2) == 0 { 11 } else { 10 };
            let speed = 2.0 + self.draw_unit() * 2.0;
            let velocity = radial_vector(angle + (self.draw_unit() * 2.0 - 1.0) * 10.0, speed);
            let scale = 1.5 + self.draw_unit() * 0.5;
            self.spawn_simple_death_effect(
                owner,
                tick,
                SimpleDeathEffectSpec {
                    alpha: 1.0,
                    alpha_loss_per_tick: 0.025,
                    atlas: EnemyEffectAtlas::BadGuys,
                    blend_mode: EnemyEffectBlend::Normal,
                    entry,
                    kind: EnemyDeathEffectKind::MoveFade,
                    lifetime_ticks: 40,
                    role: &format!("wraith-dissolve-ray:{index}"),
                    scale,
                },
                DeathEffectOverrides {
                    position: Some(Vec2::new(
                        owner.position.x + velocity.x * 10.0,
                        owner.position.y + velocity.y * 10.0,
                    )),
                    rotation_deg: Some(angle),
                    velocity: Some(velocity),
                    velocity_damping: Some(0.8),
                    ..Default::default()
                },
                order,
            );
            angle += 30.0;
        }
        self.spawn_simple_death_effect(
            owner,
            tick,
            SimpleDeathEffectSpec {
                alpha: 1.0,
                alpha_loss_per_tick: super::effects::NATIVE_FADE_ALPHA_LOSS,
                atlas: EnemyEffectAtlas::BadGuys,
                blend_mode: EnemyEffectBlend::Add,
                entry: 20,
                kind: EnemyDeathEffectKind::FadeScale,
                lifetime_ticks: 20,
                role: "wraith-dissolve-core",
                scale: 1.0,
            },
            DeathEffectOverrides {
                opacity_timer: Some(2.0),
                position: Some(Vec2::new(
                    fround(owner.position.x),
                    fround(owner.position.y - 15.0),
                )),
                scale_multiplier: Some(fround(1.02)),
                ..Default::default()
            },
            order,
        );
        for index in 0..12 {
            self.spawn_bouncer(
                owner,
                tick,
                |_| 27,
                &format!("wraith-dissolve-bouncer:{index}"),
                |world| {
                    let direction = radial_vector(angle, 1.0);
                    let velocity = Vec2::new(direction.x * 1.5, direction.y);
                    let distance = 15.0 + world.draw_unit() * 10.0;
                    BouncerOptions {
                        opacity_timer: Some(1.5),
                        position: Some(Vec2::new(
                            owner.position.x + velocity.x * (distance + 2.0),
                            owner.position.y + velocity.y * distance,
                        )),
                        velocity: Some(velocity),
                        ..Default::default()
                    }
                },
                enhanced_effects,
                order,
            );
            angle += 30.0 + (self.draw_unit() * 2.0 - 1.0) * 10.0;
        }
        let mut entries = skeleton_death_fragment_entries(enhanced_effects).to_vec();
        shuffle_actor_fragments(self, &mut entries);
        angle = actor.heading_deg;
        for entry in entries {
            self.spawn_bouncer(
                owner,
                tick,
                |_| entry,
                "wraith-smoky-fragment",
                |world| {
                    let direction =
                        radial_vector(angle + (world.draw_unit() * 2.0 - 1.0) * 45.0, 1.0);
                    let velocity = Vec2::new(direction.x * 1.5, direction.y);
                    let distance = 15.0 + world.draw_unit() * 30.0;
                    BouncerOptions {
                        kind: if enhanced_effects {
                            BouncerKind::Smoky
                        } else {
                            BouncerKind::Plain
                        },
                        position: Some(Vec2::new(
                            owner.position.x + velocity.x * (distance + 2.0),
                            owner.position.y + velocity.y * distance,
                        )),
                        velocity: Some(velocity),
                        ..Default::default()
                    }
                },
                enhanced_effects,
                order,
            );
            angle += 72.0 + (self.draw_unit() * 2.0 - 1.0) * 10.0;
        }
        self.spawn_bouncer(
            owner,
            tick,
            |world| 1819 + world.draw_integer(4) as usize,
            "wraith-skull",
            |world| {
                let height = -(10.0 + world.draw_unit() * 10.0);
                let velocity =
                    radial_vector(actor.heading_deg + (world.draw_unit() * 2.0 - 1.0) * 10.0, 5.0);
                BouncerOptions {
                    bounce_retention: Some(0.7),
                    height: Some(height),
                    kind: BouncerKind::Smoky,
                    velocity: Some(velocity),
                    ..Default::default()
                }
            },
            enhanced_effects,
            order,
        );
        self.spawn_unbind(actor, tick, order);
    }

    pub(super) fn spawn_coffin_terminal_effects(
        &mut self,
        actor: &impl EnemyActorRead,
        tick: u64,
        enhanced_effects: bool,
        order: &mut WorldPainterOrder,
    ) {
        let owner = DeathEffectOwner::from(actor);
        let mut entries = skeleton_death_fragment_entries(enhanced_effects).to_vec();
        shuffle_actor_fragments(self, &mut entries);
        let mut angle = self.draw_unit() * 360.0;
        for entry in entries {
            self.spawn_bouncer(
                owner,
                tick,
                |_| entry,
                "coffin-bone",
                |world| {
                    let direction = radial_vector(angle, 1.0);
                    let velocity = Vec2::new(direction.x * 1.5, direction.y);
                    let distance = 15.0 + world.draw_unit() * 10.0;
                    BouncerOptions {
                        position: Some(Vec2::new(
                            owner.position.x + velocity.x * (distance + 2.0),
                            owner.position.y + velocity.y * distance,
                        )),
                        velocity: Some(velocity),
                        ..Default::default()
                    }
                },
                enhanced_effects,
                order,
            );
            angle += 72.0 + (self.draw_unit() * 2.0 - 1.0) * 10.0;
        }
        for extra in [false, true] {
            let count = if extra { 12 + self.draw_integer(4) } else { 40 + self.draw_integer(11) };
            for index in 0..count {
                self.spawn_bouncer(
                    owner,
                    tick,
                    |world| {
                        if extra {
                            2067 + world.draw_integer(3) as usize
                        } else {
                            2013 + world.draw_integer(50) as usize
                        }
                    },
                    &format!("coffin-{}-fragment:{index}", if extra { "extra" } else { "main" }),
                    |world| {
                        let speed = if extra {
                            1.0 + world.draw_unit()
                        } else {
                            0.5 + world.draw_unit() * 0.5
                        };
                        let direction = radial_vector(angle, speed);
                        let velocity = Vec2::new(direction.x * 1.5, direction.y);
                        let distance = 15.0 + world.draw_unit() * 10.0;
                        BouncerOptions {
                            bounce_velocity_scale: Some(2.0),
                            position: Some(Vec2::new(
                                owner.position.x + velocity.x * (distance + 2.0),
                                owner.position.y + velocity.y * distance,
                            )),
                            velocity: Some(velocity),
                            ..Default::default()
                        }
                    },
                    enhanced_effects,
                    order,
                );
                angle += 72.0 + (self.draw_unit() * 2.0 - 1.0) * 10.0;
            }
        }
        let velocity = radial_vector(angle, 2.0);
        self.spawn_bouncer(
            owner,
            tick,
            |world| 1819 + world.draw_integer(4) as usize,
            "coffin-skull",
            |_| BouncerOptions { velocity: Some(velocity), ..Default::default() },
            enhanced_effects,
            order,
        );
        self.spawn_unbind(actor, tick, order);
    }
}

pub(super) fn shuffle_actor_fragments(world: &mut EnemyWorld, entries: &mut [usize]) {
    for index in 0..entries.len() {
        let other = world.draw_integer(entries.len() as u32) as usize;
        entries.swap(index, other);
    }
}
