//! Enemy death-effect births and stepping from Website `enemies/death-effects.ts` and
//! `core-server/boneyard-transient-effects.ts`. Native draw order is retained at birth,
//! including the immediate Bouncer tick before it enters the world list.

use super::model::*;
use super::world::EnemyWorld;
use crate::Vec2;
use crate::float;
use crate::float::fround;
use crate::painter::{WorldManagerLane, WorldPainterOrder};

// Source `boneyard-transient-effects.ts` keeps these native Fade stores as singles.
pub const NATIVE_FADE_ALPHA_LOSS: f64 = 0.1_f32 as f64;
pub const NATIVE_BANISH_RING_ALPHA_LOSS: f64 = (0.1_f32 * 0.25_f32) as f64;
pub const NATIVE_TRAGIC_CONTACT_ALPHA_LOSS: f64 = (0.1_f32 * 0.5_f32) as f64;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DeathEffectOwner {
    pub id: usize,
    pub position: Vec2,
}

impl From<&EnemyActor> for DeathEffectOwner {
    fn from(actor: &EnemyActor) -> Self {
        Self { id: actor.id, position: actor.position }
    }
}

/// The required fields of `spawnSimpleDeathEffect`; source optional fields are separate
/// so a caller cannot accidentally omit the atlas, lifetime or fade program.
pub struct SimpleDeathEffectSpec<'a> {
    pub alpha: f64,
    pub alpha_loss_per_tick: f64,
    pub atlas: EnemyEffectAtlas,
    pub blend_mode: EnemyEffectBlend,
    pub entry: usize,
    pub kind: EnemyDeathEffectKind,
    pub lifetime_ticks: u64,
    pub role: &'a str,
    pub scale: f64,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DeathEffectOverrides {
    pub painter_sort_bias: Option<f64>,
    pub scrap_oscillation: Option<ScrapOscillation>,
    pub alpha_multiplier: Option<f64>,
    pub angular_velocity_deg: Option<f64>,
    pub first_entry: Option<usize>,
    pub frame_count: Option<usize>,
    pub frame_phase: Option<f64>,
    pub frame_velocity: Option<f64>,
    pub frame_velocity_damping: Option<f64>,
    pub frame_ticks: Option<f64>,
    pub opacity_timer: Option<f64>,
    pub position: Option<Vec2>,
    pub presentation_owner: Option<DeathPresentationOwner>,
    pub rotation_deg: Option<f64>,
    pub scale_y: Option<f64>,
    pub scale_multiplier: Option<f64>,
    pub spawn_delay_ticks: Option<u64>,
    pub tint: Option<u32>,
    pub velocity: Option<Vec2>,
    pub velocity_damping: Option<f64>,
}

#[derive(Clone, Copy, Debug, Default)]
pub enum BouncerKind {
    #[default]
    Plain,
    Smoky,
    BlackSmoky,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct BouncerOptions {
    pub bounce_velocity_multiplier: Option<f64>,
    pub presentation_owner: Option<DeathPresentationOwner>,
    pub atlas: Option<EnemyEffectAtlas>,
    pub bounce_retention: Option<f64>,
    pub bounce_velocity_scale: Option<f64>,
    pub height_scale: Option<f64>,
    pub kind: BouncerKind,
    pub opacity_timer: Option<f64>,
    pub position: Option<Vec2>,
    pub height: Option<f64>,
    pub scale: Option<f64>,
    pub scale_y: Option<f64>,
    pub tint: Option<u32>,
    pub velocity: Option<Vec2>,
}

impl EnemyWorld {
    pub fn spawn_simple_death_effect(
        &mut self,
        owner: DeathEffectOwner,
        tick: u64,
        spec: SimpleDeathEffectSpec<'_>,
        options: DeathEffectOverrides,
        order: &mut WorldPainterOrder,
    ) {
        let presentation_owner =
            options.presentation_owner.unwrap_or(DeathPresentationOwner::WorldSorted);
        let painter_registration = if presentation_owner == DeathPresentationOwner::WorldSorted {
            Some(order.register(WorldManagerLane::Transient))
        } else {
            None
        };
        let last_step_tick = if presentation_owner == DeathPresentationOwner::PreWorldQueue {
            tick.checked_sub(1)
        } else {
            Some(tick)
        };
        self.death_effects.push(EnemyDeathEffect {
            age_ticks: 0,
            alpha: spec.alpha,
            alpha_multiplier: options.alpha_multiplier.unwrap_or(1.0),
            alpha_loss_per_tick: spec.alpha_loss_per_tick,
            angular_velocity_deg: options.angular_velocity_deg.unwrap_or(0.0),
            atlas: spec.atlas,
            blend_mode: spec.blend_mode,
            bounce_retention: 0.0,
            bounce_velocity: 0.0,
            entry: spec.entry,
            first_entry: options.first_entry.unwrap_or(spec.entry),
            frame_count: options.frame_count.unwrap_or(1),
            frame_phase: options.frame_phase.unwrap_or(0.0),
            frame_velocity: options.frame_velocity.unwrap_or(0.0),
            frame_velocity_damping: options.frame_velocity_damping.unwrap_or(1.0),
            frame_ticks: options.frame_ticks.unwrap_or(1.0),
            height: 0.0,
            id: self.next_death_effect_id,
            kind: spec.kind,
            last_step_tick,
            lifetime_ticks: spec.lifetime_ticks,
            opacity_timer: options.opacity_timer.unwrap_or(spec.alpha),
            owner_actor_id: owner.id,
            painter_registration,
            presentation_owner,
            position: options.position.unwrap_or(owner.position),
            role: spec.role.to_owned(),
            rotation_deg: options.rotation_deg.unwrap_or(0.0),
            scale: spec.scale,
            scale_y: options.scale_y.unwrap_or(spec.scale),
            scale_multiplier: options.scale_multiplier.unwrap_or(1.0),
            shadow: false,
            spawn_tick: tick + options.spawn_delay_ticks.unwrap_or(0),
            tint: options.tint.unwrap_or(0xffffff),
            vertical_velocity: 0.0,
            velocity: options.velocity.unwrap_or(Vec2::ZERO),
            velocity_damping: options.velocity_damping.unwrap_or(1.0),
            painter_sort_bias: options.painter_sort_bias,
            scrap_oscillation: options.scrap_oscillation,
        });
        self.next_death_effect_id += 1;
    }

    /// The four constructor draws precede both callbacks. Entry-bank selection and
    /// custom trajectories therefore consume randomness at their native call sites.
    #[allow(clippy::too_many_arguments)] // Source work policy accompanies both constructor callbacks.
    pub fn spawn_bouncer(
        &mut self,
        owner: DeathEffectOwner,
        tick: u64,
        entry: impl FnOnce(&mut Self) -> usize,
        role: &str,
        options: impl FnOnce(&mut Self) -> BouncerOptions,
        enhanced_effects: bool,
        order: &mut WorldPainterOrder,
    ) {
        let constructor_vertical_velocity = -(self.draw_unit() * 3.0 + 2.0);
        let constructor_height = -self.draw_unit() * 20.0;
        let rotation_deg = self.draw_unit() * 360.0;
        let angular_velocity_deg = self.draw_unit() * 10.0 + 1.0;
        let entry = entry(self);
        let options = options(self);
        let vertical_velocity =
            constructor_vertical_velocity * options.bounce_velocity_scale.unwrap_or(1.0);
        let opacity_timer =
            options.opacity_timer.unwrap_or(if enhanced_effects { 10.0 } else { 2.0 });
        let presentation_owner =
            options.presentation_owner.unwrap_or(DeathPresentationOwner::WorldSorted);
        let painter_registration = if presentation_owner == DeathPresentationOwner::WorldSorted {
            Some(order.register(WorldManagerLane::Transient))
        } else {
            None
        };
        let kind = match options.kind {
            BouncerKind::Plain => EnemyDeathEffectKind::Bouncer,
            BouncerKind::Smoky => EnemyDeathEffectKind::SmokyBouncer,
            BouncerKind::BlackSmoky => EnemyDeathEffectKind::BlackSmokyBouncer,
        };
        let mut effect = EnemyDeathEffect {
            age_ticks: 0,
            alpha: 1.0,
            alpha_multiplier: 1.0,
            alpha_loss_per_tick: 0.015,
            angular_velocity_deg,
            atlas: options.atlas.unwrap_or(EnemyEffectAtlas::BadGuys),
            blend_mode: EnemyEffectBlend::Normal,
            bounce_retention: options.bounce_retention.unwrap_or(0.65),
            bounce_velocity: vertical_velocity * options.bounce_velocity_multiplier.unwrap_or(1.0),
            entry,
            first_entry: entry,
            frame_count: 1,
            frame_phase: 0.0,
            frame_velocity: 0.0,
            frame_velocity_damping: 1.0,
            frame_ticks: 1.0,
            height: options
                .height
                .unwrap_or(constructor_height * options.height_scale.unwrap_or(1.0)),
            id: self.next_death_effect_id,
            kind,
            last_step_tick: Some(tick),
            lifetime_ticks: (opacity_timer / 0.015 * 1.5).ceil() as u64 + 1,
            opacity_timer,
            owner_actor_id: owner.id,
            painter_registration,
            presentation_owner,
            position: options.position.unwrap_or(owner.position),
            role: role.to_owned(),
            rotation_deg,
            scale: options.scale.unwrap_or(1.0),
            scale_y: options.scale_y.or(options.scale).unwrap_or(1.0),
            scale_multiplier: 1.0,
            shadow: enhanced_effects,
            spawn_tick: tick,
            tint: options.tint.unwrap_or(0xffffff),
            vertical_velocity,
            velocity: options.velocity.unwrap_or(Vec2::ZERO),
            velocity_damping: 1.0,
            painter_sort_bias: None,
            scrap_oscillation: None,
        };
        self.next_death_effect_id += 1;
        if self.step_death_effect(&mut effect, tick, true) {
            let smoke = self
                .bouncer_emits_smoke(&effect)
                .then(|| self.smoky_bouncer_birth(&effect, tick, order));
            self.death_effects.push(effect);
            if let Some(smoke) = smoke {
                self.death_effects.push(smoke);
            }
        }
    }

    /// `stepDeathEffects`: visit all retained rows in source order at each represented
    /// tick, then append that tick's smoke births. Skipping directly to the final age
    /// would consume a different random sequence on bounces.
    pub fn step_death_effects(&mut self, tick: u64, order: &mut WorldPainterOrder) {
        let mut effects = std::mem::take(&mut self.death_effects);
        let Some(first) =
            effects.iter().map(|effect| effect.last_step_tick.map_or(0, |last| last + 1)).min()
        else {
            return;
        };
        for step_tick in first..=tick {
            let mut births = Vec::new();
            effects.retain_mut(|effect| {
                if effect.last_step_tick.is_some_and(|last| last >= step_tick) {
                    return true;
                }
                if !self.step_death_effect(effect, step_tick, false) {
                    return false;
                }
                if self.bouncer_emits_smoke(effect) {
                    births.push(self.smoky_bouncer_birth(effect, step_tick, order));
                }
                true
            });
            effects.extend(births);
        }
        self.death_effects = effects;
    }

    pub fn step_pre_world_effect_births(&mut self, tick: u64) {
        let mut effects = std::mem::take(&mut self.death_effects);
        effects.retain_mut(|effect| {
            if effect.presentation_owner == DeathPresentationOwner::PreWorldQueue
                && effect.spawn_tick == tick
                && effect.last_step_tick.is_none_or(|last| last < tick)
            {
                self.step_death_effect(effect, tick, true)
            } else {
                true
            }
        });
        self.death_effects = effects;
    }

    fn step_death_effect(
        &mut self,
        effect: &mut EnemyDeathEffect,
        tick: u64,
        update_at_birth: bool,
    ) -> bool {
        if tick < effect.spawn_tick
            || (tick == effect.spawn_tick
                && !update_at_birth
                && effect.presentation_owner != DeathPresentationOwner::PreWorldQueue)
        {
            effect.age_ticks = 0;
            effect.last_step_tick = Some(tick);
            return true;
        }
        let age = tick.saturating_sub(effect.spawn_tick);
        if age >= effect.lifetime_ticks {
            return false;
        }
        if effect.kind == EnemyDeathEffectKind::BoulderBit {
            return super::coffin_emergence::step_coffin_emergence(
                effect,
                tick,
                &mut self.steering_rng,
            );
        }
        if effect.kind == EnemyDeathEffectKind::MoveFadeSin {
            let phase = fround(effect.frame_phase + effect.frame_velocity);
            if phase >= 180.0 {
                return false;
            }
            effect.age_ticks = age;
            effect.frame_phase = phase;
            effect.last_step_tick = Some(tick);
            effect.alpha = fround(
                float::sin(fround(phase * (std::f32::consts::PI as f64) / 180.0))
                    * effect.alpha_multiplier,
            )
            .min(1.0);
            effect.position = Vec2::new(
                fround(effect.position.x + effect.velocity.x),
                fround(effect.position.y + effect.velocity.y),
            );
            return true;
        }
        if matches!(
            effect.kind,
            EnemyDeathEffectKind::Bouncer
                | EnemyDeathEffectKind::SmokyBouncer
                | EnemyDeathEffectKind::BlackSmokyBouncer
        ) {
            let mut motion = crate::death_animations::NativeBouncerMotion {
                bounce_velocity: effect.bounce_velocity,
                height: effect.height,
                position: effect.position,
                rotation_degrees: effect.rotation_deg,
                rotation_step_degrees: effect.angular_velocity_deg,
                velocity: effect.velocity,
                vertical_velocity: effect.vertical_velocity,
            };
            let skipped =
                crate::death_animations::step_native_bouncer_motion(&mut motion, tick, self);
            if !skipped {
                effect.opacity_timer = fround(effect.opacity_timer - effect.alpha_loss_per_tick);
            }
            effect.bounce_velocity = motion.bounce_velocity;
            effect.height = motion.height;
            effect.position = motion.position;
            effect.rotation_deg = motion.rotation_degrees;
            effect.angular_velocity_deg = motion.rotation_step_degrees;
            effect.velocity = motion.velocity;
            effect.vertical_velocity = motion.vertical_velocity;
            if effect.opacity_timer <= 0.0
                || (effect.kind == EnemyDeathEffectKind::BlackSmokyBouncer && effect.height >= 0.0)
            {
                return false;
            }
            effect.age_ticks = age;
            effect.alpha = effect.opacity_timer.min(1.0);
            effect.last_step_tick = Some(tick);
            return true;
        }
        if effect.kind == EnemyDeathEffectKind::Scrap {
            let oscillation =
                effect.scrap_oscillation.as_mut().expect("Scrap lost its oscillation state");
            let velocity = Vec2::new(
                fround(effect.velocity.x * (0.92_f32 as f64)),
                fround(effect.velocity.y * (0.92_f32 as f64)),
            );
            let mut position = Vec2::new(
                fround(effect.position.x + effect.velocity.x),
                fround(fround(effect.position.y + effect.velocity.y) + (0.1_f32 as f64)),
            );
            if velocity.x * velocity.x + velocity.y * velocity.y < 0.25 {
                position.y = fround(position.y + (0.1_f32 as f64));
                position.x = fround(position.x + (self.draw_unit() * 2.0 - 1.0) * (0.2_f32 as f64));
                effect.opacity_timer = fround(effect.opacity_timer - (0.01_f32 as f64));
            }
            if effect.opacity_timer <= 0.0 {
                return false;
            }
            oscillation.phase_deg = fround(oscillation.phase_deg + oscillation.step_deg);
            effect.age_ticks = age;
            effect.alpha = effect.opacity_timer.min(1.0);
            effect.last_step_tick = Some(tick);
            effect.position = position;
            effect.velocity = velocity;
            effect.rotation_deg = fround(
                float::sin(oscillation.phase_deg * std::f64::consts::PI / 180.0)
                    * oscillation.amplitude_deg,
            );
            return true;
        }
        effect.opacity_timer = fround(effect.opacity_timer - effect.alpha_loss_per_tick);
        if effect.opacity_timer <= 0.0 {
            return false;
        }
        match effect.kind {
            EnemyDeathEffectKind::MoveFadePerspective => {
                effect.position = Vec2::new(
                    fround(effect.position.x + effect.velocity.x),
                    fround(effect.position.y + effect.velocity.y),
                );
                effect.velocity = Vec2::new(
                    fround(effect.velocity.x * effect.velocity_damping),
                    fround(effect.velocity.y * effect.velocity_damping),
                );
            }
            EnemyDeathEffectKind::MoveFade => {
                effect.position.x += effect.velocity.x;
                effect.position.y += effect.velocity.y;
                effect.velocity.x *= effect.velocity_damping;
                effect.velocity.y *= effect.velocity_damping;
            }
            EnemyDeathEffectKind::SpriteArray => {
                effect.frame_phase = fround(effect.frame_phase + effect.frame_velocity);
                if effect.frame_phase > effect.frame_count as f64 {
                    return false;
                }
                effect.frame_velocity =
                    fround(effect.frame_velocity * effect.frame_velocity_damping);
                effect.entry = effect.first_entry
                    + effect
                        .frame_phase
                        .round_ties_even()
                        .max(0.0)
                        .min(effect.frame_count as f64 - 1.0) as usize;
                if effect.velocity.x != 0.0 || effect.velocity.y != 0.0 {
                    effect.position.x += effect.velocity.x;
                    effect.position.y += effect.velocity.y;
                }
            }
            EnemyDeathEffectKind::FireArray => {
                let frame = effect.first_entry as i64 - 46
                    + (age as f64 / effect.frame_ticks).floor() as i64;
                effect.entry = 46 + frame.rem_euclid(32) as usize;
            }
            EnemyDeathEffectKind::FadeScalePerspective | EnemyDeathEffectKind::FadeScale => {
                effect.scale = fround(fround(effect.scale) * fround(effect.scale_multiplier));
            }
            EnemyDeathEffectKind::BanishBlack
            | EnemyDeathEffectKind::Banish
            | EnemyDeathEffectKind::Fade
            | EnemyDeathEffectKind::FadeAdditive
            | EnemyDeathEffectKind::FadePerspective
            | EnemyDeathEffectKind::FadePerspectiveClipped
            | EnemyDeathEffectKind::LateSplat
            | EnemyDeathEffectKind::Unbind => {}
            EnemyDeathEffectKind::BoulderBit
            | EnemyDeathEffectKind::MoveFadeSin
            | EnemyDeathEffectKind::Bouncer
            | EnemyDeathEffectKind::SmokyBouncer
            | EnemyDeathEffectKind::BlackSmokyBouncer
            | EnemyDeathEffectKind::Scrap => {
                unreachable!("these native programs step before the shared fade")
            }
        }
        effect.age_ticks = age;
        effect.alpha = if effect.kind == EnemyDeathEffectKind::SpriteArray
            && effect.frame_phase.round_ties_even() >= effect.frame_count as f64
        {
            0.0
        } else {
            death_effect_alpha(effect.kind, effect.opacity_timer, effect.alpha_multiplier)
        };
        effect.last_step_tick = Some(tick);
        effect.rotation_deg += effect.angular_velocity_deg;
        if matches!(
            effect.kind,
            EnemyDeathEffectKind::FadeScale | EnemyDeathEffectKind::FadeScalePerspective
        ) {
            effect.scale_y = fround(fround(effect.scale_y) * fround(effect.scale_multiplier));
        }
        true
    }

    fn bouncer_emits_smoke(&mut self, effect: &EnemyDeathEffect) -> bool {
        effect.height < 0.0
            && match effect.kind {
                EnemyDeathEffectKind::SmokyBouncer => self.draw_integer(3) == 1,
                EnemyDeathEffectKind::BlackSmokyBouncer => true,
                _ => false,
            }
    }

    fn smoky_bouncer_birth(
        &mut self,
        owner: &EnemyDeathEffect,
        tick: u64,
        order: &mut WorldPainterOrder,
    ) -> EnemyDeathEffect {
        let radius = self.draw_unit() * 10.0;
        let offset = radial_vector(self.draw_unit() * 360.0, radius);
        let rotation = self.draw_unit() * 360.0;
        let scale = 0.1 + self.draw_unit() * 0.25;
        let black = owner.kind == EnemyDeathEffectKind::BlackSmokyBouncer;
        let opacity = 0.25 + self.draw_unit() * if black { 0.45_f32 as f64 } else { 0.45 };
        let loss = if black { 0.005_f32 as f64 } else { 0.01 };
        let id = self.next_death_effect_id;
        self.next_death_effect_id += 1;
        EnemyDeathEffect {
            age_ticks: 0,
            alpha: opacity,
            alpha_multiplier: 1.0,
            alpha_loss_per_tick: loss,
            angular_velocity_deg: 0.0,
            atlas: EnemyEffectAtlas::BadGuys,
            blend_mode: if black { EnemyEffectBlend::Normal } else { EnemyEffectBlend::Add },
            bounce_retention: 0.0,
            bounce_velocity: 0.0,
            entry: 10,
            first_entry: 10,
            frame_count: 1,
            frame_phase: 0.0,
            frame_velocity: 0.0,
            frame_velocity_damping: 1.0,
            frame_ticks: 1.0,
            height: 0.0,
            id,
            kind: if black {
                EnemyDeathEffectKind::Fade
            } else {
                EnemyDeathEffectKind::FadeAdditive
            },
            last_step_tick: Some(tick),
            lifetime_ticks: (opacity / loss).ceil() as u64,
            opacity_timer: opacity,
            owner_actor_id: owner.owner_actor_id,
            painter_registration: Some(order.register(WorldManagerLane::Transient)),
            presentation_owner: DeathPresentationOwner::WorldSorted,
            position: Vec2::new(owner.position.x + offset.x, owner.position.y + offset.y),
            role: format!("{}:smoke", owner.role),
            rotation_deg: rotation,
            scale,
            scale_y: scale,
            scale_multiplier: 1.0,
            shadow: false,
            spawn_tick: tick,
            tint: if black { 0 } else { 0xbebf8f },
            vertical_velocity: 0.0,
            velocity: Vec2::ZERO,
            velocity_damping: 1.0,
            painter_sort_bias: None,
            scrap_oscillation: None,
        }
    }
}

pub(super) fn radial_vector(heading: f64, magnitude: f64) -> Vec2 {
    let radians = heading * std::f64::consts::PI / 180.0;
    Vec2::new(float::sin(radians) * magnitude, -float::cos(radians) * magnitude)
}

fn death_effect_alpha(kind: EnemyDeathEffectKind, opacity: f64, multiplier: f64) -> f64 {
    if matches!(kind, EnemyDeathEffectKind::Fade | EnemyDeathEffectKind::Unbind) {
        return opacity * multiplier;
    }
    let alpha = if kind == EnemyDeathEffectKind::LateSplat {
        (opacity * 0.25).min(1.0)
    } else {
        opacity.min(1.0)
    };
    alpha * multiplier
}

impl crate::death_animations::NativeBouncerRandom for EnemyWorld {
    fn float(&mut self, maximum: f64) -> f64 {
        self.draw_unit() * maximum
    }
    fn integer(&mut self, maximum: u32) -> u32 {
        self.draw_integer(maximum)
    }
}
