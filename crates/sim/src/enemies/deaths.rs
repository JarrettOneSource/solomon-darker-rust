//! Real terminal epochs, child births and rewards from `enemies/deaths.ts` and
//! `construction.ts::spawnTerminalChildren`.

use super::config::EnemyFamilyConfig;
use super::demon_skull_death::step_dying_demon_skull;
use super::effects::*;
use super::faculty_death::{spawn_faculty_dying_effects, spawn_faculty_finale};
use super::heartmonger::step_heartmonger;
use super::model::*;
use super::projectile_emission::{SpawnEnemyProjectileOptions, spawn_projectile};
use super::projectiles::EnemyProjectileKind;
use super::world::{EnemyStepContext, EnemyWorld};
use super::{EnemySpawnIntent, EnemyToken};
use crate::Vec2;
use crate::boneyard::loot::BoneyardLootDeathProgram;
use crate::float;
use crate::float::fround;
use crate::painter::WorldPainterOrder;

impl EnemyWorld {
    /// Returns whether the actor remains during its authored terminal presentation.
    pub fn step_dying_actor(
        &mut self,
        actor: &mut EnemyActor,
        context: &mut EnemyStepContext<'_>,
        order: &mut WorldPainterOrder,
    ) -> bool {
        if actor.config.enemy_token == EnemyToken::Cocoon {
            return false;
        }
        let tick = context.tick;
        let enhanced_effects = context.enhanced_effects;
        let captured = actor.ether_drain_captured
            || matches!(&actor.brain, EnemyBrain::Spider(brain) if brain.phase == SpiderPhase::Captured);
        if matches!(&actor.brain,EnemyBrain::DemonSkull(brain) if brain.death_countdown>0) {
            step_dying_demon_skull(self, actor, tick, order);
            return true;
        }
        if matches!(actor.brain, EnemyBrain::Heartmonger(_)) {
            step_heartmonger(self, actor, context, order);
        }
        if let EnemyFamilyConfig::Faculty(family) = &actor.config.family {
            let death_tick = tick - actor.death_started_tick.unwrap_or(tick);
            if actor.death_tick == 0 && death_tick > 0 {
                emit_stream(self, actor, tick, "faculty-die");
                let mut event = EnemyEvent::new(EnemyEventKind::EnemyScreenFlash, actor.id, tick);
                event.source_position = Some(actor.position);
                event.screen_flash = Some(EnemyScreenFlash {
                    red: 0.0,
                    green: 0.0,
                    blue: 0.0,
                    alpha: 1.0,
                    decay_per_tick: 0.01,
                    point_attenuated: true,
                });
                self.emit_event(event);
            }
            if actor.death_tick < 101 && death_tick >= 101 {
                emit_stream(
                    self,
                    actor,
                    tick,
                    if family.female { "faculty-no-female" } else { "faculty-no" },
                );
            }
            let mut heading = actor.heading_deg;
            for step in actor.death_tick + 1..=death_tick {
                spawn_faculty_dying_effects(
                    self,
                    actor,
                    tick - death_tick + step,
                    step,
                    enhanced_effects,
                    order,
                );
                heading = fround(heading + (step / 20) as f64);
                if heading >= 360.0 {
                    heading = fround(heading - 360.0);
                }
            }
            actor.body_pose = 3.0;
            actor.death_tick = death_tick;
            actor.heading_deg = heading;
            if let EnemyBrain::Faculty(brain) = &mut actor.brain {
                brain.body_pose = 3.0;
                brain.body_heading_deg = heading;
            }
            if death_tick < 250 {
                return true;
            }
        }
        if actor.config.enemy_token == EnemyToken::Demon && !captured {
            let start = actor.death_started_tick.unwrap_or(tick);
            let death_tick = tick - start;
            if !actor.death_presentation_started {
                self.spawn_demon_death_fires(actor, start, order);
                actor.death_presentation_started = true;
            }
            if actor.death_tick < 95 && death_tick >= 95 {
                emit_death_sound(self, actor, tick, EnemySound::Flash, 1.0);
                emit_death_sound(self, actor, tick, EnemySound::DemonDie, 1.0);
                self.spawn_demon_death_fire_burst(actor, tick, order);
            }
            actor.death_tick = death_tick;
            if death_tick < 100 {
                return true;
            }
        }
        self.detach_heartmonger_crows(actor, tick);
        self.emit_event(EnemyEvent::new(EnemyEventKind::EnemyDeath, actor.id, tick));
        let output = terminal_output(actor.config.enemy_token);
        let count = if captured { Some(0) } else { self.terminal_output_count(actor) };
        let mut event = EnemyEvent::new(EnemyEventKind::EnemyTerminalOutput, actor.id, tick);
        event.count = count;
        event.output = Some(output);
        self.emit_event(event);
        // Feedback is synchronous here. Delaying this callback until queue collection
        // changes its order relative to explosions and terminal child constructors.
        context.geometry.on_terminal_output(output, count);
        if !captured {
            self.emit_enemy_death_sounds(actor, tick, count);
        }
        if let EnemyFamilyConfig::Zombie { rotten: true, poison_pool_damage, .. } =
            actor.config.family
            && poison_pool_damage > 0.0
            && !captured
        {
            spawn_projectile(
                self,
                actor,
                tick,
                EnemyProjectileKind::PoisonPool,
                poison_pool_damage,
                SpawnEnemyProjectileOptions::default(),
                enhanced_effects,
                order,
            );
        }
        if !captured {
            self.spawn_terminal_effects(actor, tick, count, enhanced_effects, order);
        }
        if !captured {
            self.spawn_terminal_children(actor, context, order);
        }
        let mut event = EnemyEvent::new(EnemyEventKind::Reward, actor.id, tick);
        event.target_player_id = Some(actor.last_damaged_by_player_id.clone());
        let event_id = self.emit_event(event);
        self.rewards.push(EnemyReward {
            actor_id: actor.id,
            event_id,
            experience: actor.config.experience,
            loot_source: EnemyLootSource {
                actor_seed: actor.loot_seed,
                enemy_token: actor.config.enemy_token,
                on_death_program: actor
                    .config
                    .miniboss_die_program
                    .then_some(BoneyardLootDeathProgram::MinibossDie),
                policies: actor.config.recipe_uid.map(|_| actor.config.loot_policies),
                participant_slot: 0,
                position: actor.position,
                recipe_uid: actor.config.recipe_uid,
            },
            player_id: actor.last_damaged_by_player_id.clone(),
        });
        let event_id =
            self.emit_event(EnemyEvent::new(EnemyEventKind::EnemyRetired, actor.id, tick));
        self.retired.push(EnemyRetirement { actor_id: actor.id, event_id });
        false
    }

    fn terminal_output_count(&self, actor: &EnemyActor) -> Option<usize> {
        let imps =
            self.actors.iter().filter(|actor| actor.config.enemy_token == EnemyToken::Imp).count();
        match actor.config.family {
            EnemyFamilyConfig::Imp { split_depth, .. } => {
                Some(if split_depth > 0 && imps <= 68 { 2 } else { 0 })
            }
            EnemyFamilyConfig::Demon { split_count, .. } => {
                Some(split_count.min(70_usize.saturating_sub(imps) as i64) as usize)
            }
            _ => None,
        }
    }

    fn spawn_terminal_children(
        &mut self,
        actor: &EnemyActor,
        context: &mut EnemyStepContext<'_>,
        order: &mut WorldPainterOrder,
    ) {
        let (count, split_depth, imp) = match actor.config.family {
            EnemyFamilyConfig::Imp { split_depth, .. } => {
                let imps = self
                    .actors
                    .iter()
                    .filter(|actor| actor.config.enemy_token == EnemyToken::Imp)
                    .count();
                if split_depth == 0 || imps > 68 {
                    return;
                }
                (2, split_depth - 1, true)
            }
            EnemyFamilyConfig::Demon { split_count, .. } => (split_count, 0, false),
            _ => return,
        };
        let radius = actor.config.collision_radius.max(10.0);
        let mut intents = Vec::new();
        for index in 0..count {
            let position = if imp {
                actor.position
            } else {
                let angle = index as f64 / count as f64 * std::f64::consts::PI * 2.0;
                Vec2::new(
                    actor.position.x + float::cos(angle) * radius,
                    actor.position.y + float::sin(angle) * radius,
                )
            };
            intents.push(EnemySpawnIntent::new(
                EnemyToken::Imp,
                position,
                self.next_synthetic_spawn_intent_id,
                context.tick,
                actor.wave_ordinal,
            ));
            self.next_synthetic_spawn_intent_id += 1;
        }
        let spawned = self.materialize_step_intents(&intents, context, Some(split_depth), order);
        if imp {
            for (index, id) in spawned.into_iter().enumerate() {
                let child = self
                    .actors
                    .iter_mut()
                    .find(|child| child.id == id)
                    .expect("materialized Imp child retains its actor row");
                child.heading_deg =
                    ((actor.heading_deg + [-90.0, 90.0][index]) % 360.0 + 360.0) % 360.0;
            }
        }
    }

    fn emit_enemy_death_sounds(&mut self, actor: &EnemyActor, tick: u64, count: Option<usize>) {
        let cue = |world: &mut Self, sound: EnemySound, base: f64, range: f64| {
            let pitch = if range == 0.0 { base } else { base + world.draw_unit() * range };
            emit_death_sound(world, actor, tick, sound, pitch);
        };
        match actor.config.enemy_token {
            EnemyToken::Spider => cue(self, EnemySound::SpiderDie, 0.95, 0.15),
            EnemyToken::Cocoon | EnemyToken::DemonSkull => {}
            EnemyToken::Heartmonger
            | EnemyToken::DireFaculty
            | EnemyToken::Skeleton
            | EnemyToken::SkeletonArcher
            | EnemyToken::SkeletonMage => cue(self, EnemySound::SkeletonDie, 0.8, 0.2),
            EnemyToken::Imp => {
                if count == Some(2) {
                    cue(self, EnemySound::ImpSplit, 0.9, 0.2)
                } else {
                    cue(self, EnemySound::FireyDeath, 0.8, 0.2)
                }
            }
            EnemyToken::Portal => cue(self, EnemySound::PortalDie, 1.0, 0.0),
            EnemyToken::Zombie => {
                if matches!(actor.config.family, EnemyFamilyConfig::Zombie { rotten: true, .. }) {
                    for _ in 0..3 {
                        cue(self, EnemySound::ZombiePoisonSplat, 0.9, 0.15);
                    }
                }
                cue(self, EnemySound::ZombieDie, 0.8, 0.2);
                cue(self, EnemySound::ZombieDieGroan, 0.8, 0.2);
            }
            EnemyToken::Wraith => {
                cue(self, EnemySound::Flash, 1.0, 0.0);
                cue(self, EnemySound::BansheeDie, 0.9, 0.2);
                cue(self, EnemySound::BansheeDie, 0.9, 0.2);
                cue(self, EnemySound::BansheeDie, 0.8, 0.4);
            }
            EnemyToken::Demon => cue(self, EnemySound::FireyDeath, 0.8, 0.2),
            EnemyToken::Coffin => cue(self, EnemySound::CoffinBreak, 1.0, 0.1),
        }
    }

    fn spawn_terminal_effects(
        &mut self,
        actor: &EnemyActor,
        tick: u64,
        count: Option<usize>,
        enhanced_effects: bool,
        order: &mut WorldPainterOrder,
    ) {
        let owner = DeathEffectOwner::from(actor);
        match actor.config.enemy_token {
            EnemyToken::Spider => self.spawn_spider_remains(actor, tick),
            EnemyToken::Cocoon | EnemyToken::DemonSkull => {}
            EnemyToken::DireFaculty => {
                spawn_faculty_finale(self, actor, tick, enhanced_effects, order)
            }
            EnemyToken::Heartmonger => {
                self.spawn_heartmonger_shatter(actor, tick, enhanced_effects, order)
            }
            EnemyToken::Skeleton | EnemyToken::SkeletonArcher | EnemyToken::SkeletonMage => {
                self.spawn_skeleton_shatter(actor, tick, enhanced_effects, order)
            }
            EnemyToken::Portal => {
                self.spawn_portal_terminal_effects(actor, tick, enhanced_effects, order)
            }
            EnemyToken::Zombie => {
                self.spawn_zombie_terminal_effects(actor, tick, enhanced_effects, order)
            }
            EnemyToken::Wraith => {
                self.spawn_wraith_terminal_effects(actor, tick, enhanced_effects, order)
            }
            EnemyToken::Coffin => {
                self.spawn_coffin_terminal_effects(actor, tick, enhanced_effects, order)
            }
            EnemyToken::Imp => {
                self.spawn_unbind(actor, tick, order);
                let scale = if count == Some(2) { 0.25 } else { 1.0 };
                self.spawn_simple_death_effect(
                    owner,
                    tick,
                    SimpleDeathEffectSpec {
                        alpha: 1.0,
                        alpha_loss_per_tick: 0.02 / scale,
                        atlas: EnemyEffectAtlas::BadGuys,
                        blend_mode: EnemyEffectBlend::Add,
                        entry: 15,
                        kind: EnemyDeathEffectKind::Banish,
                        lifetime_ticks: (2.0 / (0.02 / scale)).ceil() as u64,
                        role: "imp-banish",
                        scale,
                    },
                    DeathEffectOverrides { opacity_timer: Some(2.0), ..Default::default() },
                    order,
                );
                self.spawn_sprite_array(
                    owner,
                    tick,
                    "imp-sprite-array",
                    401,
                    19,
                    1.0,
                    2.0 * scale,
                    DeathEffectOverrides {
                        frame_velocity: Some(0.5 / scale),
                        frame_velocity_damping: Some(0.98),
                        presentation_owner: Some(DeathPresentationOwner::PreWorldQueue),
                        ..Default::default()
                    },
                    order,
                );
            }
            EnemyToken::Demon => {
                self.spawn_simple_death_effect(
                    owner,
                    tick,
                    SimpleDeathEffectSpec {
                        alpha: 1.0,
                        alpha_loss_per_tick: 0.01,
                        atlas: EnemyEffectAtlas::BadGuys,
                        blend_mode: EnemyEffectBlend::Add,
                        entry: 15,
                        kind: EnemyDeathEffectKind::Banish,
                        lifetime_ticks: 200,
                        role: "demon-banish",
                        scale: 2.0,
                    },
                    DeathEffectOverrides { opacity_timer: Some(2.0), ..Default::default() },
                    order,
                );
                self.spawn_sprite_array(
                    owner,
                    tick,
                    "demon-sprite-array",
                    401,
                    19,
                    1.0,
                    4.0,
                    DeathEffectOverrides {
                        frame_velocity: Some(0.25),
                        frame_velocity_damping: Some(0.995),
                        presentation_owner: Some(DeathPresentationOwner::PreWorldQueue),
                        ..Default::default()
                    },
                    order,
                );
            }
        }
    }

    fn spawn_demon_death_fires(
        &mut self,
        actor: &EnemyActor,
        start: u64,
        order: &mut WorldPainterOrder,
    ) {
        let owner = DeathEffectOwner::from(actor);
        for delay in [0, 20, 40, 60, 80] {
            let phase = self.draw_integer(32) as usize;
            let radius = self.draw_unit() * (100 - delay) as f64 / 20.0;
            let displacement = radial_vector(self.draw_unit() * 360.0, radius);
            self.spawn_simple_death_effect(
                owner,
                start,
                SimpleDeathEffectSpec {
                    alpha: 1.0,
                    alpha_loss_per_tick: 0.0,
                    atlas: EnemyEffectAtlas::DeadHawg,
                    blend_mode: EnemyEffectBlend::Add,
                    entry: 46 + phase,
                    kind: EnemyDeathEffectKind::FireArray,
                    lifetime_ticks: 500,
                    role: &format!("demon-death-fire:{delay}"),
                    scale: 1.0,
                },
                DeathEffectOverrides {
                    first_entry: Some(46 + phase),
                    frame_count: Some(32),
                    frame_ticks: Some(4.0),
                    frame_velocity: Some(0.25),
                    presentation_owner: Some(DeathPresentationOwner::PreWorldQueue),
                    position: Some(Vec2::new(
                        owner.position.x + displacement.x,
                        owner.position.y + displacement.y - 20.0,
                    )),
                    spawn_delay_ticks: Some(delay),
                    ..Default::default()
                },
                order,
            );
        }
    }

    fn spawn_demon_death_fire_burst(
        &mut self,
        actor: &EnemyActor,
        tick: u64,
        order: &mut WorldPainterOrder,
    ) {
        let owner = DeathEffectOwner::from(actor);
        let rotation = self.draw_unit() * 360.0;
        let magnitude = 0.5 + self.draw_unit();
        let angular = if self.draw_integer(2) == 0 { -magnitude } else { magnitude };
        let step = 0.25 * 0.75;
        let lifetime = (4.0_f64 / step).ceil() as u64;
        self.spawn_simple_death_effect(
            owner,
            tick,
            SimpleDeathEffectSpec {
                alpha: 0.5,
                alpha_loss_per_tick: 0.5 * step / 4.0,
                atlas: EnemyEffectAtlas::BadGuys,
                blend_mode: EnemyEffectBlend::Normal,
                entry: 110,
                kind: EnemyDeathEffectKind::Fade,
                lifetime_ticks: lifetime,
                role: "demon-death-fire-burst-glow",
                scale: 10.0,
            },
            DeathEffectOverrides {
                position: Some(Vec2::new(owner.position.x, owner.position.y - 20.0)),
                presentation_owner: Some(DeathPresentationOwner::DirectPostWorld),
                tint: Some(0xff_80_00),
                velocity: Some(Vec2::new(0.0, -1.0)),
                ..Default::default()
            },
            order,
        );
        self.spawn_simple_death_effect(
            owner,
            tick,
            SimpleDeathEffectSpec {
                alpha: 1.0,
                alpha_loss_per_tick: 0.0,
                atlas: EnemyEffectAtlas::BadGuys,
                blend_mode: EnemyEffectBlend::Add,
                entry: 251,
                kind: EnemyDeathEffectKind::SpriteArray,
                lifetime_ticks: lifetime,
                role: "demon-death-fire-burst-frame",
                scale: 2.0,
            },
            DeathEffectOverrides {
                first_entry: Some(251),
                frame_count: Some(4),
                frame_ticks: Some(1.0 / step),
                frame_velocity: Some(step),
                angular_velocity_deg: Some(angular),
                position: Some(Vec2::new(owner.position.x, owner.position.y - 20.0)),
                presentation_owner: Some(DeathPresentationOwner::DirectPostWorld),
                rotation_deg: Some(rotation),
                tint: Some(0xff_ff_bf),
                velocity: Some(Vec2::new(0.0, -1.0)),
                ..Default::default()
            },
            order,
        );
    }
}

fn emit_stream(world: &mut EnemyWorld, actor: &EnemyActor, tick: u64, stream: &str) {
    let mut event = EnemyEvent::new(EnemyEventKind::EnemyStream, actor.id, tick);
    event.stream = Some(stream.to_owned());
    event.source_position = Some(actor.position);
    world.emit_event(event);
}
fn emit_death_sound(
    world: &mut EnemyWorld,
    actor: &EnemyActor,
    tick: u64,
    sound: EnemySound,
    pitch: f64,
) {
    let mut event = EnemyEvent::new(EnemyEventKind::EnemyDeathSound, actor.id, tick);
    event.gain_scale = Some(1.0);
    event.pitch = Some(pitch);
    event.sound = Some(sound);
    event.source_position = Some(actor.position);
    world.emit_event(event);
}
fn terminal_output(token: EnemyToken) -> EnemyTerminalOutput {
    match token {
        EnemyToken::DemonSkull => EnemyTerminalOutput::DiscorporealBanish,
        EnemyToken::DireFaculty => EnemyTerminalOutput::FacultyBreak,
        EnemyToken::Heartmonger => EnemyTerminalOutput::HeartmongerShatter,
        EnemyToken::Skeleton => EnemyTerminalOutput::SkeletonShatter,
        EnemyToken::SkeletonArcher => EnemyTerminalOutput::ArcherShatter,
        EnemyToken::SkeletonMage => EnemyTerminalOutput::MageShatter,
        EnemyToken::Imp => EnemyTerminalOutput::ImpSplit,
        EnemyToken::Portal => EnemyTerminalOutput::PortalBreak,
        EnemyToken::Zombie => EnemyTerminalOutput::ZombieCollapse,
        EnemyToken::Wraith => EnemyTerminalOutput::WraithFragments,
        EnemyToken::Demon => EnemyTerminalOutput::DemonSplit,
        EnemyToken::Coffin => EnemyTerminalOutput::CoffinBreak,
        EnemyToken::Spider => EnemyTerminalOutput::SpiderCollapse,
        EnemyToken::Cocoon => panic!("Cocoon retires through its player owner"),
    }
}
