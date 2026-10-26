//! Enemy-owned hit acceptance from Website `enemies/damage.ts` and `enemies/cocoon.ts`.
//! Shields and armor consume an entire hit; ordinary deaths enter their terminal brain
//! without collapsing the actor's presentation interval or changing its registration.

use super::config::{EnemyClassification, EnemyFamilyConfig};
use super::death::death_brain;
use super::effects::{
    BouncerOptions, DeathEffectOverrides, DeathEffectOwner, SimpleDeathEffectSpec, radial_vector,
};
use super::model::*;
use super::world::EnemyWorld;
use super::{EnemyFlag, EnemyToken};
use crate::Vec2;
use crate::float::fround;
use crate::painter::WorldPainterOrder;

impl EnemyWorld {
    pub fn set_hurricane_contact_cooldown(&mut self, actor_id: usize, cooldown: u64) -> bool {
        assert!(cooldown <= 0xffff, "enemy Hurricane cooldown must fit an unsigned short");
        if let Some(mut actor) = self.actors.iter_mut().find(|actor| actor.id == actor_id) {
            actor.hurricane_contact_cooldown = cooldown as f64;
            return true;
        }
        if let Some(actor) = self.maggots.iter_mut().find(|actor| actor.id == actor_id) {
            actor.hurricane_contact_cooldown = cooldown as f64;
            return true;
        }
        false
    }

    /// Staff disable stacks its native movement/action multipliers only on live targets.
    pub fn apply_staff_disable(&mut self, actor_id: usize) {
        if let Some(mut actor) = self.actors.iter_mut().find(|actor| actor.id == actor_id) {
            if actor.life_state == EnemyLifeState::Alive {
                actor.staff_action_factor = fround(actor.staff_action_factor * 0.5);
                actor.staff_movement_factor = fround(actor.staff_movement_factor * 0.75);
            }
        } else if let Some(actor) = self.maggots.iter_mut().find(|actor| actor.id == actor_id)
            && actor.life_state == EnemyLifeState::Alive
        {
            actor.staff_action_factor = fround(actor.staff_action_factor * 0.5);
            actor.staff_movement_factor = fround(actor.staff_movement_factor * 0.75);
        }
    }

    pub fn apply_staff_heading_perturbation(&mut self, actor_id: usize, delta_degrees: f64) {
        assert!(delta_degrees.is_finite(), "staff heading perturbation must be finite");
        if let Some(mut actor) = self.actors.iter_mut().find(|actor| actor.id == actor_id) {
            if actor.life_state == EnemyLifeState::Alive {
                actor.heading_deg = ((actor.heading_deg + delta_degrees) % 360.0 + 360.0) % 360.0;
            }
        } else if let Some(actor) = self.maggots.iter_mut().find(|actor| actor.id == actor_id)
            && actor.life_state == EnemyLifeState::Alive
        {
            actor.heading_deg = ((actor.heading_deg + delta_degrees) % 360.0 + 360.0) % 360.0;
        }
    }

    pub fn apply_staff_impact_vertical_velocity(&mut self, actor_id: usize, velocity: f64) {
        assert!(velocity.is_finite(), "staff vertical velocity must be finite");
        if let Some(mut actor) = self.actors.iter_mut().find(|actor| actor.id == actor_id)
            && actor.life_state == EnemyLifeState::Alive
            && let EnemyBrain::Imp(brain) = &mut actor.brain
        {
            brain.vertical_velocity = fround(velocity);
        }
    }

    pub fn break_skeleton_pike(&mut self, actor_id: usize) -> bool {
        use super::config::SkeletonWeapon;
        let Some(mut actor) = self.actors.iter_mut().find(|actor| actor.id == actor_id) else {
            return false;
        };
        if actor.life_state != EnemyLifeState::Alive
            || actor.config.enemy_token != EnemyToken::Skeleton
        {
            return false;
        }
        let EnemyBrain::Skeleton(brain) = &mut actor.brain else {
            return false;
        };
        let EnemyFamilyConfig::Skeleton { weapon, .. } = &mut actor.body.config.family else {
            return false;
        };
        if *weapon != SkeletonWeapon::Pike {
            return false;
        }
        *weapon = SkeletonWeapon::Claw;
        brain.action = SkeletonAction::Claw;
        brain.pike = None;
        brain.action_progress = 0.0;
        brain.contact_target_player_id = None;
        brain.marker_emitted = false;
        brain.phase = SkeletonPhase::Approach;
        actor.body_pose = 0.0;
        actor.head_facing_offset = 0;
        actor.config.flags.retain(|flag| {
            !matches!(
                flag,
                EnemyFlag::Sword
                    | EnemyFlag::Mace
                    | EnemyFlag::Flail
                    | EnemyFlag::Axe
                    | EnemyFlag::Pike
            )
        });
        true
    }

    /// `releaseBoneyardSkeletonPike`: Hail releases the existing restraint before movement.
    pub fn release_skeleton_pike(&mut self, actor_id: usize) -> bool {
        let Some(actor) = self.actors.iter_mut().find(|actor| actor.id == actor_id) else {
            return false;
        };
        detach_skeleton_pike(actor.body, actor.brain)
    }

    /// `setBoneyardEnemyBlizzardPushState` also addresses independently registered Maggots.
    pub fn set_blizzard_push_state(
        &mut self,
        actor_id: usize,
        accumulator: f64,
        last_tick: u64,
    ) -> bool {
        assert!(
            accumulator.is_finite() && accumulator >= 0.0,
            "enemy Blizzard push accumulator must be finite and nonnegative"
        );
        if let Some(mut actor) = self.actors.iter_mut().find(|actor| actor.id == actor_id) {
            actor.blizzard_push_accumulator = accumulator;
            actor.blizzard_push_last_tick = Some(last_tick);
            return true;
        }
        if let Some(maggot) = self.maggots.iter_mut().find(|actor| actor.id == actor_id) {
            maggot.blizzard_push_accumulator = accumulator;
            maggot.blizzard_push_last_tick = Some(last_tick);
            return true;
        }
        false
    }

    pub fn damage(
        &mut self,
        request: EnemyDamageRequest,
        painter_order: Option<&mut WorldPainterOrder>,
    ) -> EnemyDamageResult {
        assert!(
            self.last_step_tick.is_none_or(|tick| request.tick >= tick),
            "enemy damage tick must not precede the store clock"
        );
        assert!(
            request.amount.is_finite() && request.amount > 0.0,
            "enemy damage must be finite and positive"
        );
        assert!(
            request.hit_strength.is_finite() && request.hit_strength >= 0.0,
            "enemy hit strength must be finite and nonnegative"
        );
        let Some(index) = self.actors.iter().position(|actor| actor.id == request.actor_id) else {
            return self.damage_maggot(request);
        };
        let mut actor = self.actors.get(index).expect("enemy index is live").snapshot();
        if actor.life_state != EnemyLifeState::Alive {
            return EnemyDamageResult::REJECTED;
        }
        let mut standalone;
        let order = if let Some(order) = painter_order {
            order
        } else {
            standalone = self.standalone_painter_order();
            &mut standalone
        };
        if matches!(actor.brain, EnemyBrain::Cocoon(_)) {
            return self.damage_cocoon(&actor, request, order);
        }
        if actor.shield_health > 0.0 {
            if actor.shield_sound_cooldown_ticks == 0 {
                let pitch = 0.8 + self.draw_unit() * 0.05;
                self.emit_damage_sound(&actor, request.tick, EnemySound::HitShield, pitch);
                actor.shield_pulse = 2.0;
                actor.shield_sound_cooldown_ticks = 10;
            }
            actor.shield_health = (actor.shield_health - request.amount).max(0.0);
            if actor.shield_health == 0.0 {
                actor.shield_maximum_health = 0.0;
                self.emit_damage_sound(&actor, request.tick, EnemySound::PopShield, 0.8);
                self.spawn_shield_break_particles(&actor, request.tick, order);
            }
            self.actors.replace(index, actor);
            return EnemyDamageResult {
                accepted: true,
                health_damage: 0.0,
                killed: false,
                special_death_accepted: false,
            };
        }
        let hurt_ready = match &actor.brain {
            EnemyBrain::Portal(brain) => brain.hurt_ticks_remaining == 0,
            _ => actor.last_damage_tick.is_none_or(|last| request.tick.saturating_sub(last) >= 20),
        };
        if let Some(sound) = enemy_hurt_sound(actor.config.enemy_token)
            && !request.suppress_hurt_sound
            && hurt_ready
        {
            let pitch = 0.9 + self.draw_unit() * 0.2;
            self.emit_damage_sound(&actor, request.tick, sound, pitch);
        }
        if let EnemyBrain::Portal(brain) = &mut actor.brain
            && hurt_ready
        {
            brain.hurt_ticks_remaining = 10 + u64::from(self.draw_integer(15));
        }
        if let EnemyBrain::Spider(brain) = &mut actor.brain
            && !request.magic
            && !(brain.strafe_ticks_remaining > 0 && brain.action_state == 3)
        {
            brain.strafe_sign = self.steering_rng.draw_sign(1.0);
            brain.strafe_ticks_remaining = self.steering_rng.draw_integer(21) as i64 * 2 + 40;
            brain.action_state = 2;
        }
        let prior_health = actor.current_health;
        let mut current_health = prior_health - request.amount;
        let health_damage = prior_health.max(0.0).min(request.amount);
        if current_health <= 0.0
            && matches!(actor.config.family, EnemyFamilyConfig::Skeleton { armor: true, .. })
        {
            let base = 0.9_f32 as f64;
            let pitch = fround(base + self.draw_unit() * (1.25 - base));
            let sound = match self.draw_integer(3) {
                0 => EnemySound::ArmorCrash1,
                1 => EnemySound::ArmorCrash2,
                _ => EnemySound::ArmorCrash3,
            };
            self.emit_damage_sound(&actor, request.tick, sound, pitch);
            self.spawn_skeleton_armor_break(
                &actor,
                request.tick,
                request.enhanced_effects.unwrap_or(true),
                order,
            );
            let EnemyFamilyConfig::Skeleton { armor, headgear, .. } = &mut actor.body.config.family
            else {
                unreachable!()
            };
            let bonus = match headgear {
                1 => 6.0,
                2 | 5 => 10.0,
                4 => 15.0,
                _ => 0.0,
            };
            current_health = fround(fround(4.0 + self.draw_unit() * 2.0) + bonus);
            *armor = false;
            detach_skeleton_pike(&mut actor.body, &mut actor.brain);
            actor.config.flags.retain(|flag| *flag != EnemyFlag::Armor);
            actor.config.base_speed = fround(actor.config.base_speed * 1.75);
            actor.config.attack_speed = fround(actor.config.attack_speed * 1.5);
        }
        let killed = current_health <= 0.0;
        if actor.config.classification != EnemyClassification::Normal {
            self.featured_boss_id = if killed { None } else { Some(actor.id) };
            self.queue_demon_skull_health_triggers(
                prior_health / actor.config.maximum_health,
                current_health / actor.config.maximum_health,
            );
        }
        actor.current_health = current_health;
        actor.last_damaged_by_player_id = request.source_player_id;
        actor.last_damage_tick = Some(request.tick);
        actor.hit_feedback.receive(request.tick, request.hit_strength);
        actor.hit_reaction_timer = if request.suppress_hit_reaction { 0.0 } else { 1.0 };
        if killed {
            actor.ether_drain_captured = request.ether_drain_capture
                && matches!(
                    actor.config.enemy_token,
                    EnemyToken::Skeleton
                        | EnemyToken::SkeletonArcher
                        | EnemyToken::SkeletonMage
                        | EnemyToken::Zombie
                        | EnemyToken::Demon
                        | EnemyToken::Spider
                        | EnemyToken::Imp
                        | EnemyToken::Wraith
                );
            if let EnemyBrain::Spider(brain) = &mut actor.brain
                && request.ether_drain_capture
            {
                brain.phase = SpiderPhase::Captured;
            } else {
                death_brain(&mut actor.brain);
            }
            actor.death_epoch = Some(self.next_death_epoch);
            self.next_death_epoch += 1;
            actor.death_started_tick = Some(request.tick);
            actor.death_tick = 0;
            actor.head_facing_offset = 0;
            actor.lethal_magic_damage = request.has_magic_damage;
            actor.life_state = EnemyLifeState::Dying;
            if actor.config.enemy_token == EnemyToken::SkeletonArcher {
                actor.lighting.charge = 0.0;
                actor.lighting.provider_copies = 0;
            }
            actor.shield_health = 0.0;
            actor.shield_maximum_health = 0.0;
            actor.shield_pulse = 0.0;
            actor.shield_sound_cooldown_ticks = 0;
            if actor.config.enemy_token == EnemyToken::DireFaculty {
                self.boss_narration.cancel();
                self.emit_event(EnemyEvent::new(
                    EnemyEventKind::EnemyDialogueStop,
                    actor.id,
                    request.tick,
                ));
            }
        }
        self.actors.replace(index, actor);
        EnemyDamageResult { accepted: true, health_damage, killed, special_death_accepted: killed }
    }

    fn damage_maggot(&mut self, request: EnemyDamageRequest) -> EnemyDamageResult {
        let Some(maggot) = self.maggots.iter_mut().find(|maggot| maggot.id == request.actor_id)
        else {
            return EnemyDamageResult::REJECTED;
        };
        if maggot.life_state != EnemyLifeState::Alive
            || !maggot.combat_active && !request.ether_drain_capture
        {
            return EnemyDamageResult::REJECTED;
        }
        let health_damage = maggot.current_health.max(0.0).min(request.amount);
        maggot.current_health -= request.amount;
        let killed = maggot.current_health <= 0.0;
        if killed {
            maggot.native_tree_query_member = false;
            maggot.ether_drain_captured = request.ether_drain_capture;
            maggot.death_epoch = Some(self.next_death_epoch);
            self.next_death_epoch += 1;
            maggot.death_started_tick = Some(request.tick);
            maggot.life_state = EnemyLifeState::Dying;
        }
        maggot.last_damaged_by_player_id = request.source_player_id;
        maggot.last_damage_tick = Some(request.tick);
        maggot.hit_feedback.receive(request.tick, request.hit_strength);
        EnemyDamageResult { accepted: true, health_damage, killed, special_death_accepted: killed }
    }

    fn damage_cocoon(
        &mut self,
        actor: &impl EnemyActorRead,
        request: EnemyDamageRequest,
        order: &mut WorldPainterOrder,
    ) -> EnemyDamageResult {
        let EnemyBrain::Cocoon(brain) = actor.brain() else { unreachable!() };
        let prior =
            brain.owner_player_id.as_ref().and_then(|id| self.webbed_players.get(id).copied());
        let hit_pulse =
            if request.magic { self.steering_rng.draw_scaled_float(0.5, false) } else { 1.0 };
        let next = prior.and_then(|state| {
            super::webbed::damage_native_cocoon(state, request.amount, hit_pulse)
        });
        if let Some(id) = &brain.owner_player_id {
            if let Some(state) = next {
                self.webbed_players.insert(id.clone(), state);
            } else {
                self.webbed_players.remove(id);
            }
        }
        if next.is_none() {
            for mut candidate in self.actors.iter_mut() {
                if let EnemyBrain::Cocoon(candidate_brain) = &mut candidate.brain
                    && candidate_brain.owner_player_id == brain.owner_player_id
                {
                    candidate.body.life_state = EnemyLifeState::Dying;
                    candidate_brain.phase = CocoonPhase::Death;
                }
            }
            if prior.is_some() {
                self.release_cocoon(
                    DeathEffectOwner { id: actor.id, position: brain.owner_position },
                    request.tick,
                    request.enhanced_effects.unwrap_or(true),
                    order,
                );
            }
        }
        if let Some(tick) = self.last_step_tick {
            self.step_pre_world_effect_births(tick);
        }
        if self.featured_boss_id.is_some_and(|id| !self.actors.iter().any(|actor| actor.id == id)) {
            self.featured_boss_id = None;
        }
        EnemyDamageResult {
            accepted: true,
            health_damage: 0.0,
            killed: next.is_none(),
            special_death_accepted: false,
        }
    }

    fn release_cocoon(
        &mut self,
        owner: DeathEffectOwner,
        tick: u64,
        enhanced_effects: bool,
        order: &mut WorldPainterOrder,
    ) {
        let rotation = self.draw_unit() * 360.0;
        self.spawn_simple_death_effect(
            owner,
            tick,
            SimpleDeathEffectSpec {
                alpha: 1.0,
                alpha_loss_per_tick: fround((0.1_f32 as f64) * (0.15_f32 as f64)),
                atlas: EnemyEffectAtlas::DeadHawg,
                blend_mode: EnemyEffectBlend::Add,
                entry: 14,
                kind: EnemyDeathEffectKind::FadePerspective,
                lifetime_ticks: 267,
                role: "cocoon-flash",
                scale: 1.0,
            },
            DeathEffectOverrides {
                opacity_timer: Some(4.0),
                rotation_deg: Some(rotation),
                ..DeathEffectOverrides::default()
            },
            order,
        );
        let pitch = 1.0 + (self.draw_unit() * 2.0 - 1.0) * 0.05;
        self.emit_action_sound(owner, tick, EnemySound::Disintegrate, pitch);
        let pitch = 0.8 + self.draw_unit() * 0.1;
        let sound =
            if self.draw_integer(2) == 0 { EnemySound::Webbed1 } else { EnemySound::Webbed2 };
        self.emit_action_sound(owner, tick, sound, pitch);
        let mut heading = self.draw_unit() * 360.0;
        for _ in 0..12 {
            let entry = if self.draw_integer(2) == 0 { 11 } else { 10 };
            let y = owner.position.y - self.draw_unit() * 25.0;
            let speed = 2.0 + self.draw_unit() * 5.0;
            let velocity = radial_vector(heading + (self.draw_unit() * 2.0 - 1.0) * 20.0, speed);
            let scale = 1.5 + self.draw_unit() * 0.5;
            let mut loss = 0.025;
            if self.draw_integer(5) == 1 {
                loss *= 0.5;
            }
            if self.draw_integer(5) == 1 {
                loss *= 0.5;
            }
            self.spawn_simple_death_effect(
                owner,
                tick,
                SimpleDeathEffectSpec {
                    alpha: 1.0,
                    alpha_loss_per_tick: loss,
                    atlas: EnemyEffectAtlas::BadGuys,
                    blend_mode: EnemyEffectBlend::Normal,
                    entry,
                    kind: EnemyDeathEffectKind::MoveFade,
                    lifetime_ticks: (1.0_f64 / loss).ceil() as u64,
                    role: "cocoon-fragment",
                    scale,
                },
                DeathEffectOverrides {
                    rotation_deg: Some(heading),
                    velocity: Some(velocity),
                    velocity_damping: Some(0.8),
                    position: Some(Vec2::new(
                        owner.position.x + velocity.x * 10.0,
                        y + velocity.y * 10.0,
                    )),
                    ..DeathEffectOverrides::default()
                },
                order,
            );
            heading += 30.0;
        }
        for _ in 0..7 {
            self.spawn_bouncer(
                owner,
                tick,
                |_| 27,
                "cocoon-bouncer",
                |world| {
                    let vector = radial_vector(heading, 1.0);
                    let velocity = Vec2::new(vector.x * 1.5, vector.y);
                    let radius = 15.0 + world.draw_unit() * 10.0;
                    heading += 72.0 + (world.draw_unit() * 2.0 - 1.0) * 10.0;
                    BouncerOptions {
                        opacity_timer: Some(1.5),
                        scale_y: Some(0.75),
                        velocity: Some(velocity),
                        position: Some(Vec2::new(
                            owner.position.x + velocity.x * (radius + 2.0),
                            owner.position.y + velocity.y * radius,
                        )),
                        ..BouncerOptions::default()
                    }
                },
                enhanced_effects,
                order,
            );
        }
        self.emit_event(EnemyEvent::new(EnemyEventKind::CocoonReleased, owner.id, tick));
    }

    pub(super) fn emit_damage_sound(
        &mut self,
        actor: &impl EnemyActorRead,
        tick: u64,
        sound: EnemySound,
        pitch: f64,
    ) {
        let mut event = EnemyEvent::new(EnemyEventKind::EnemyDamageSound, actor.id, tick);
        event.gain_scale = Some(1.0);
        event.pitch = Some(pitch);
        event.sound = Some(sound);
        event.source_position = Some(actor.position);
        self.emit_event(event);
    }

    pub(super) fn emit_action_sound(
        &mut self,
        owner: DeathEffectOwner,
        tick: u64,
        sound: EnemySound,
        pitch: f64,
    ) {
        let mut event = EnemyEvent::new(EnemyEventKind::EnemyActionSound, owner.id, tick);
        event.gain_scale = Some(1.0);
        event.pitch = Some(pitch);
        event.sound = Some(sound);
        event.source_position = Some(owner.position);
        self.emit_event(event);
    }

    fn spawn_shield_break_particles(
        &mut self,
        actor: &impl EnemyActorRead,
        tick: u64,
        order: &mut WorldPainterOrder,
    ) {
        for _ in 0..20 {
            let rotation = self.draw_unit() * 360.0;
            let alpha = 0.5 + self.draw_unit() * 0.75;
            let scale = 1.5 + self.draw_unit() * 0.25;
            self.spawn_simple_death_effect(
                DeathEffectOwner::from(actor),
                tick,
                SimpleDeathEffectSpec {
                    alpha,
                    alpha_loss_per_tick: 0.05,
                    atlas: EnemyEffectAtlas::BadGuys,
                    blend_mode: EnemyEffectBlend::Add,
                    entry: 69,
                    kind: EnemyDeathEffectKind::Fade,
                    lifetime_ticks: (alpha / 0.05).ceil() as u64,
                    role: "shield-break-particle",
                    scale,
                },
                DeathEffectOverrides {
                    rotation_deg: Some(rotation),
                    position: Some(Vec2::new(actor.position.x, actor.position.y - 30.0)),
                    ..DeathEffectOverrides::default()
                },
                order,
            );
        }
    }

    fn queue_demon_skull_health_triggers(&mut self, before: f64, after: f64) {
        let encounter = &mut self.demon_skull_encounter;
        if !encounter.health_triggers_enabled {
            return;
        }
        let mut mask = 0;
        for (index, threshold) in [0.8_f32, 0.6_f32, 0.4_f32, 0.2_f32].into_iter().enumerate() {
            let bit = 1 << index;
            if encounter.health_triggers_fired_mask & bit == 0
                && before > f64::from(threshold)
                && after < f64::from(threshold)
            {
                mask |= bit;
            }
        }
        encounter.health_triggers_fired_mask |= mask;
        encounter.pending_capabilities |= mask;
    }
}

pub(super) fn detach_skeleton_pike(body: &mut EnemyBody, brain: &mut EnemyBrain) -> bool {
    let EnemyBrain::Skeleton(brain) = brain else {
        return false;
    };
    if brain.pike.is_none() {
        return false;
    }
    body.body_pose = 0.0;
    body.head_facing_offset = 0;
    brain.pike = None;
    brain.action_progress = 0.0;
    brain.contact_target_player_id = None;
    brain.marker_emitted = false;
    brain.phase = SkeletonPhase::Approach;
    true
}

fn enemy_hurt_sound(token: EnemyToken) -> Option<EnemySound> {
    match token {
        EnemyToken::DireFaculty
        | EnemyToken::Heartmonger
        | EnemyToken::Skeleton
        | EnemyToken::SkeletonArcher
        | EnemyToken::SkeletonMage => Some(EnemySound::BoneCrack),
        EnemyToken::Zombie => Some(EnemySound::ZombieOuch),
        EnemyToken::Portal => Some(EnemySound::PortalHurt),
        EnemyToken::Coffin
        | EnemyToken::Spider
        | EnemyToken::Cocoon
        | EnemyToken::DemonSkull
        | EnemyToken::Demon
        | EnemyToken::Imp
        | EnemyToken::Wraith => None,
    }
}
