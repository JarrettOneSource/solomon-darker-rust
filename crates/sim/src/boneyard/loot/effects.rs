//! Loot-owned fades and Goodie fragments from `boneyard-loot-store.ts`.
//!
//! These share the enemy effect value model, but retain the loot source's own skip cadence,
//! collision test and shared-stream bounce draws.

use crate::Vec2;
use crate::enemies::model::{
    DeathPresentationOwner, EnemyDeathEffect, EnemyDeathEffectKind, EnemyEffectAtlas,
    EnemyEffectBlend,
};
use crate::float;
use crate::float::fround;
use crate::loot::{NativeLootDropKind, NativeLootPlacement};
use crate::painter::{WorldManagerLane, WorldPainterOrder};

use super::{BoneyardGoodieState, BoneyardLootStore, LootActorRead};

impl BoneyardLootStore {
    pub(super) fn spawn_pickup_effects(
        &mut self,
        actor: &impl LootActorRead,
        tick: u64,
        painter_order: &mut WorldPainterOrder,
    ) {
        if actor.kind == NativeLootDropKind::Gold {
            for (index, loss, tint) in
                [(0, 0.05_f32 as f64, 0xd9ba70), (1, 0.1_f32 as f64, 0xffffff)]
            {
                let mut effect = self.new_loot_effect(
                    actor.id,
                    Vec2::new(actor.position.x, actor.position.y - 10.0),
                    83,
                    tick,
                    painter_order,
                );
                effect.alpha_loss_per_tick = loss;
                effect.blend_mode = EnemyEffectBlend::Add;
                effect.role = format!("gold-pickup-{index}");
                effect.tint = tint;
                self.effects.push(effect);
            }
        } else if actor.kind == NativeLootDropKind::Orb {
            let mut effect =
                self.new_loot_effect(actor.id, actor.position, 15, tick, painter_order);
            effect.alpha_loss_per_tick = 0.05_f32 as f64;
            effect.role = "orb-pickup".to_owned();
            effect.scale = 1.5;
            effect.scale_y = 1.5;
            self.effects.push(effect);
        }
    }

    pub(super) fn spawn_goodie_break_effects(
        &mut self,
        goodie: &BoneyardGoodieState,
        tick: u64,
        painter_order: &mut WorldPainterOrder,
    ) {
        let mut flash = self.create_goodie_bouncer(
            goodie,
            tick,
            52,
            "goodie-break-flash".to_owned(),
            painter_order,
        );
        let flash_angle = self.shared_rng.draw_scaled_float(45.0, true) + 180.0;
        let direction = radial_vector(flash_angle, 1.5);
        let factor = self.shared_rng.draw_float_range(2.0, 1.0);
        let velocity = Vec2::new(fround(direction.x * factor), fround(direction.y * factor));
        flash.position = Vec2::new(
            fround(goodie.position.x + direction.x * 25.0 + velocity.x * 2.0),
            fround(goodie.position.y + direction.y * 25.0),
        );
        flash.velocity = velocity;
        self.effects.push(flash);

        let mut additive = self.new_loot_effect(
            goodie.id,
            Vec2::new(goodie.position.x, goodie.position.y - 20.0),
            15,
            tick,
            painter_order,
        );
        additive.alpha_loss_per_tick = 0.075_f32 as f64;
        additive.blend_mode = EnemyEffectBlend::Add;
        additive.role = "goodie-break-additive".to_owned();
        additive.scale = 4.0;
        additive.scale_y = 4.0;
        self.effects.push(additive);

        for index in 0..20 {
            let mut particle = self.create_goodie_bouncer(
                goodie,
                tick,
                377,
                format!("goodie-break-particle-{index}"),
                painter_order,
            );
            let entry = 377 + self.shared_rng.draw_integer(4) as usize;
            let angle = self.shared_rng.draw_scaled_float(10.0, true)
                + f64::from(self.shared_rng.draw_integer(4)) * 90.0
                + 45.0;
            let direction = radial_vector(angle, 1.5);
            let factor = fround(self.shared_rng.draw_scaled_float(0.5, false) + 0.5);
            let velocity = Vec2::new(fround(direction.x * factor), fround(direction.y * factor));
            particle.entry = entry;
            particle.first_entry = entry;
            particle.position = Vec2::new(
                fround(goodie.position.x + direction.x * 25.0 + velocity.x * 2.0),
                fround(goodie.position.y + direction.y * 25.0),
            );
            particle.velocity = velocity;
            self.effects.push(particle);
        }
    }

    fn create_goodie_bouncer(
        &mut self,
        goodie: &BoneyardGoodieState,
        tick: u64,
        entry: usize,
        role: String,
        painter_order: &mut WorldPainterOrder,
    ) -> EnemyDeathEffect {
        let bounce_velocity = fround(-(self.shared_rng.draw_scaled_float(3.0, false) + 2.0));
        let height = fround(-self.shared_rng.draw_scaled_float(20.0, false));
        let rotation = self.shared_rng.draw_scaled_float(360.0, false);
        let angular_velocity = fround(self.shared_rng.draw_scaled_float(10.0, false) + 1.0);
        let mut effect =
            self.new_loot_effect(goodie.id, goodie.position, entry, tick, painter_order);
        effect.alpha_loss_per_tick = 0.015_f32 as f64;
        effect.angular_velocity_deg = angular_velocity;
        effect.bounce_retention = 0.65_f32 as f64;
        effect.bounce_velocity = bounce_velocity;
        effect.height = height;
        effect.kind = EnemyDeathEffectKind::Bouncer;
        effect.lifetime_ticks = 1000;
        effect.opacity_timer = 2.0;
        effect.role = role;
        effect.rotation_deg = rotation;
        effect.shadow = true;
        effect.vertical_velocity = bounce_velocity;
        effect
    }

    fn new_loot_effect(
        &mut self,
        owner_actor_id: usize,
        position: Vec2,
        entry: usize,
        tick: u64,
        painter_order: &mut WorldPainterOrder,
    ) -> EnemyDeathEffect {
        let id = self.next_effect_id;
        self.next_effect_id += 1;
        EnemyDeathEffect {
            age_ticks: 0,
            alpha: 1.0,
            alpha_multiplier: 1.0,
            alpha_loss_per_tick: 0.0,
            angular_velocity_deg: 0.0,
            atlas: EnemyEffectAtlas::BadGuys,
            blend_mode: EnemyEffectBlend::Normal,
            bounce_retention: 0.0,
            bounce_velocity: 0.0,
            entry,
            first_entry: entry,
            frame_count: 1,
            frame_phase: 0.0,
            frame_velocity: 0.0,
            frame_velocity_damping: 1.0,
            frame_ticks: 1.0,
            height: 0.0,
            id,
            kind: EnemyDeathEffectKind::Fade,
            last_step_tick: Some(tick),
            lifetime_ticks: 100,
            opacity_timer: 1.0,
            owner_actor_id,
            painter_registration: Some(painter_order.register(WorldManagerLane::Transient)),
            presentation_owner: DeathPresentationOwner::WorldSorted,
            position,
            role: String::new(),
            rotation_deg: 0.0,
            scale: 1.0,
            scale_y: 1.0,
            scale_multiplier: 1.0,
            shadow: false,
            spawn_tick: tick,
            tint: 0xffffff,
            vertical_velocity: 0.0,
            velocity: Vec2::ZERO,
            velocity_damping: 1.0,
            painter_sort_bias: None,
            scrap_oscillation: None,
        }
    }

    pub(super) fn step_effects(&mut self, placement: &dyn NativeLootPlacement, tick: u64) {
        let mut effects = std::mem::take(&mut self.effects);
        effects.retain_mut(|effect| self.step_effect(effect, placement, tick));
        self.effects = effects;
    }

    fn step_effect(
        &mut self,
        effect: &mut EnemyDeathEffect,
        placement: &dyn NativeLootPlacement,
        tick: u64,
    ) -> bool {
        effect.age_ticks += 1;
        effect.last_step_tick = Some(tick);
        if effect.kind != EnemyDeathEffectKind::Bouncer {
            effect.opacity_timer = fround(effect.opacity_timer - effect.alpha_loss_per_tick);
            if effect.opacity_timer <= 0.0 {
                return false;
            }
            effect.alpha = effect.opacity_timer.min(1.0);
            return true;
        }
        if effect.height != 0.0 && tick.is_multiple_of(3) {
            return true;
        }
        if effect.height != 0.0 {
            effect.position.x = fround(effect.position.x + effect.velocity.x);
            effect.position.y = fround(effect.position.y + effect.velocity.y);
            effect.height = fround(effect.height + effect.vertical_velocity);
            effect.vertical_velocity = fround(effect.vertical_velocity + (0.4_f32 as f64));
            if effect.height > 0.0 {
                if !placement.can_place(effect.position, 0.01) {
                    return false;
                }
                effect.angular_velocity_deg =
                    fround(self.shared_rng.draw_scaled_float(10.0, false) + 1.0);
                effect.bounce_velocity = fround(effect.bounce_velocity * (0.65_f32 as f64));
                effect.vertical_velocity = effect.bounce_velocity;
                if self.shared_rng.draw_integer(2) == 1 {
                    effect.velocity.x = fround(effect.velocity.x * (0.65_f32 as f64));
                    effect.velocity.y = fround(effect.velocity.y * (0.65_f32 as f64));
                }
                if effect.vertical_velocity > -0.75 {
                    effect.bounce_velocity = 0.0;
                    effect.vertical_velocity = 0.0;
                    effect.angular_velocity_deg = 0.0;
                    effect.velocity = Vec2::ZERO;
                }
                effect.height = effect.vertical_velocity;
            }
            effect.rotation_deg = fround(effect.rotation_deg + effect.angular_velocity_deg);
        }
        effect.opacity_timer = fround(effect.opacity_timer - effect.alpha_loss_per_tick);
        if effect.opacity_timer <= 0.0 {
            return false;
        }
        effect.alpha = effect.opacity_timer.min(1.0);
        true
    }
}

fn radial_vector(angle_deg: f64, speed: f64) -> Vec2 {
    let angle = angle_deg * std::f64::consts::PI / 180.0;
    Vec2::new(fround(float::sin(angle) * speed), fround(-float::cos(angle) * speed))
}
