//! Heartmonger activation, summons and flock ownership from `native-heartmonger.ts`
//! and `enemies/heartmonger.ts`.

use super::config::EnemyFamilyConfig;
use super::crow::{NativeCrowTarget, create_native_crow, step_native_crow};
use super::effects::DeathEffectOwner;
use super::model::*;
use super::movement::{move_toward_target, staff_movement_speed};
use super::path::{RadialDirection, TangentDirection};
use super::targeting::{target_by_id, target_eligible};
use super::world::{EnemyStepContext, EnemyWorld};
use super::{EnemyFlag, EnemySpawnIntent, EnemyToken, SpawnPositionPolicy};
use crate::float::{fround, hypot};
use crate::heading::heading_degrees_from_vector;
use crate::painter::WorldPainterOrder;
use crate::{NativeRng, Vec2};

pub const NATIVE_HEARTMONGER_SUMMON_INTERVALS: [u64; 5] = [0, 350, 250, 150, 75];
pub const NATIVE_HEARTMONGER_BLIND_CHANCES: [f64; 5] = [0.0, 5.0, 10.0, 30.0, 100.0];

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct DetachedCrow {
    pub flight: CrowState,
    pub owner_actor_id: usize,
    pub spawn_tick: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeHeartmongerSummon {
    pub count: usize,
    pub enemy_token: EnemyToken,
    pub flags: Vec<EnemyFlag>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeHeartmongerSound {
    pub pitch: f64,
    pub position: Vec2,
    pub sound: EnemySound,
}

pub struct NativeHeartmongerContext<'a> {
    pub admitted: bool,
    pub is_visible: &'a dyn Fn(Vec2) -> bool,
    pub light_intensity: f64,
    pub position: Vec2,
    pub targets: &'a [NativeCrowTarget<'a>],
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct NativeHeartmongerStep {
    pub sounds: Vec<NativeHeartmongerSound>,
    pub struck_target_ids: Vec<String>,
    pub summons: Vec<NativeHeartmongerSummon>,
}

#[must_use]
pub fn step_native_heartmonger(
    brain: &mut HeartmongerBrain,
    rng: &mut NativeRng,
    summon_mode: u8,
    context: NativeHeartmongerContext<'_>,
) -> NativeHeartmongerStep {
    let mut step = NativeHeartmongerStep::default();
    let interval = NATIVE_HEARTMONGER_SUMMON_INTERVALS[summon_mode as usize];
    if brain.activated && interval > 0 {
        // Source decrements to -1 before summoning. The retained state never stores
        // that intermediate negative counter, so zero is the birth boundary here.
        if brain.summon_ticks_remaining == 0 {
            summon_friends(rng, &mut step.summons);
            brain.summon_ticks_remaining = interval;
        } else {
            brain.summon_ticks_remaining -= 1;
        }
    }
    if rng.draw_integer(200) == 3 {
        let pitch = rng.draw_float_range(0.899_999_976_158_142_1, 1.0);
        let sound = if rng.draw_integer(2) == 0 { EnemySound::Crow1 } else { EnemySound::Crow2 };
        step.sounds.push(NativeHeartmongerSound { pitch, position: context.position, sound });
    }
    if rng.draw_integer(200) == 5 {
        brain.head_variant = rng.draw_integer(2) as u8;
    }
    let mut cooldown =
        if context.admitted { brain.crow_cooldown_ticks.saturating_sub(1) } else { 100 };
    let mut crows = Vec::with_capacity(brain.crows.len());
    for crow in std::mem::take(&mut brain.crows) {
        let flight = step_native_crow(
            crow,
            rng,
            Some(context.position),
            cooldown == 0,
            context.targets,
            context.is_visible,
        );
        if let Some(next_cooldown) = flight.parent_cooldown_ticks {
            cooldown = next_cooldown;
        }
        if let Some(target_id) = flight.struck_target_id {
            step.struck_target_ids.push(target_id);
            step.sounds.push(NativeHeartmongerSound {
                pitch: 1.0,
                position: flight
                    .crow
                    .as_ref()
                    .and_then(|crow| crow.position)
                    .expect("a crow completing its dive has a root"),
                sound: EnemySound::Crow1,
            });
        }
        if let Some(pitch) = flight.return_pitch {
            step.sounds.push(NativeHeartmongerSound {
                pitch,
                position: flight
                    .crow
                    .as_ref()
                    .and_then(|crow| crow.position)
                    .expect("a returning crow has a root"),
                sound: EnemySound::Wings,
            });
        }
        if let Some(crow) = flight.crow {
            crows.push(crow);
        }
    }
    brain.activated |= context.admitted && context.light_intensity > 0.400_000_005_960_464_5;
    brain.crows = crows;
    brain.crow_cooldown_ticks = cooldown;
    step
}

#[must_use]
pub fn detach_native_heartmonger_crows(
    brain: &HeartmongerBrain,
    rng: &mut NativeRng,
    position: Vec2,
    extra_crow_id: usize,
) -> Vec<CrowState> {
    let mut extra = create_native_crow(extra_crow_id, rng);
    let heading = rng.draw_scaled_float(360.0, false);
    let mut crows = brain.crows.clone();
    for crow in &mut crows {
        if crow.position.is_none() {
            crow.position = Some(position);
        }
    }
    extra.heading_deg = heading;
    extra.height = 60.0;
    extra.orbit_phase_deg = 0.0;
    extra.orbit_radius = 0.0;
    extra.position = Some(position);
    extra.speed = -1.0;
    crows.push(extra);
    crows
}

fn summon_friends(rng: &mut NativeRng, summons: &mut Vec<NativeHeartmongerSummon>) {
    if rng.draw_integer(5) == 1 {
        let flags = if rng.draw_integer(5) == 1 {
            vec![EnemyFlag::Hooded, EnemyFlag::Leading]
        } else {
            Vec::new()
        };
        summons.push(NativeHeartmongerSummon {
            count: 1,
            enemy_token: EnemyToken::SkeletonArcher,
            flags,
        });
        return;
    }
    let flags = if rng.draw_integer(5) == 1 { vec![EnemyFlag::Helm] } else { Vec::new() };
    summons.push(NativeHeartmongerSummon { count: 1, enemy_token: EnemyToken::Skeleton, flags });
    if rng.draw_integer(10) != 1 {
        return;
    }
    summons.push(NativeHeartmongerSummon {
        count: rng.draw_integer(2) as usize + 1,
        enemy_token: EnemyToken::Imp,
        flags: Vec::new(),
    });
    if rng.draw_integer(10) == 1 {
        summons.push(NativeHeartmongerSummon {
            count: rng.draw_integer(2) as usize + 1,
            enemy_token: EnemyToken::Imp,
            flags: Vec::new(),
        });
    }
}

pub fn step_heartmonger(
    world: &mut EnemyWorld,
    actor: &mut EnemyActor,
    context: &EnemyStepContext<'_>,
    order: &mut WorldPainterOrder,
) {
    let EnemyBrain::Heartmonger(mut brain) = actor.brain.clone() else {
        panic!("Heartmonger brain required");
    };
    let EnemyFamilyConfig::Heartmonger { summon_mode, .. } = actor.config.family else {
        panic!("Heartmonger config required");
    };
    let visibility =
        context.native_visibility.map_or_else(Default::default, |query| query(actor.position));
    let mut targets = Vec::new();
    for target in context.players {
        if target_eligible(target) {
            targets.push(NativeCrowTarget { id: &target.id, position: target.position });
        }
    }
    let step = step_native_heartmonger(
        &mut brain,
        &mut world.steering_rng,
        summon_mode,
        NativeHeartmongerContext {
            admitted: visibility.admitted,
            is_visible: context.projected_point_visible.unwrap_or(&|_| true),
            light_intensity: visibility.intensity,
            position: actor.position,
            targets: &targets,
        },
    );
    for sound in step.sounds {
        world.emit_action_sound(
            DeathEffectOwner { id: actor.id, position: sound.position },
            context.tick,
            sound.sound,
            sound.pitch,
        );
    }
    for target_id in step.struck_target_ids {
        let mut event = EnemyEvent::new(EnemyEventKind::EnemyScreenFlash, actor.id, context.tick);
        event.source_position = Some(actor.position);
        event.screen_flash = Some(EnemyScreenFlash {
            alpha: 1.0,
            red: 0.0,
            green: 0.0,
            blue: 0.0,
            decay_per_tick: fround(0.1),
            point_attenuated: false,
        });
        world.emit_event(event);
        let event = super::combat::attack_marker(
            world,
            actor,
            context.tick,
            Some(target_id.clone()),
            order,
        );
        super::combat::direct_player_damage(world, actor, Some(&target_id), event);
    }
    for summon in step.summons {
        for _ in 0..summon.count {
            let mut intent = EnemySpawnIntent::new(
                summon.enemy_token,
                actor.position,
                world.next_synthetic_spawn_intent_id,
                context.tick,
                actor.wave_ordinal,
            );
            world.next_synthetic_spawn_intent_id += 1;
            intent.flags = summon.flags.clone();
            intent.position_policy = SpawnPositionPolicy::Dark;
            world.pending_spawn_intents.push(intent);
        }
    }
    actor.path.flank_ticks_remaining = 100;
    if context.tick > actor.spawn_tick && (context.tick - actor.spawn_tick - 1).is_multiple_of(1000)
    {
        let target = target_by_id(actor.target_player_id.as_deref(), context.players);
        actor.path.flank_angle_deg = if let Some(target) = target {
            let sign = if world.steering_rng.draw_integer(2) == 1 { -1.0 } else { 1.0 };
            heading_degrees_from_vector(
                (target.position.y - actor.position.y) * sign,
                -(target.position.x - actor.position.x) * sign,
            )
        } else {
            let magnitude = world.steering_rng.draw_float_range(45.0, 135.0);
            world.steering_rng.draw_sign(magnitude)
        };
        actor.path.flank_radius = world.steering_rng.draw_float_range(150.0, 250.0);
    }
    let prior_position = actor.position;
    let effect = context.ability_effect(actor.id);
    if actor.life_state == EnemyLifeState::Alive
        && effect.time_scale >= 9.999_999_747_378_752e-5
        && effect.disrupted_ticks == 0
    {
        if actor.path.reorientation_ticks_remaining > 0 {
            let target = target_by_id(actor.target_player_id.as_deref(), context.players)
                .map(|target| target.position);
            actor.heading_deg =
                actor.path.step_reorientation(actor.heading_deg, actor.position, target);
        } else {
            let radial = if effect.flee_ticks > 0 {
                RadialDirection::Retreat
            } else {
                RadialDirection::Approach
            };
            move_toward_target(world, actor, context, radial, TangentDirection::Hold);
        }
    }
    let distance = hypot(actor.position.x - prior_position.x, actor.position.y - prior_position.y);
    let movement_scale = actor.config.chase_speed * staff_movement_speed(actor);
    let phase_step =
        if movement_scale == 0.0 { 0.0 } else { fround(distance / movement_scale * 0.25) };
    brain.leg_phase = fround(brain.leg_phase + phase_step);
    if brain.leg_phase >= 10.0 {
        brain.leg_phase = fround(brain.leg_phase - 10.0);
    }
    let torso_step = fround(phase_step * 0.850_000_023_841_857_9 * 0.25);
    let variation = world.steering_rng.draw_scaled_float(fround(torso_step * 0.25), true);
    brain.torso_phase = fround(fround(brain.torso_phase + torso_step) + variation);
    if brain.torso_phase >= 3.0 {
        let sound = if world.steering_rng.draw_integer(2) == 0 {
            EnemySound::ChainClank1
        } else {
            EnemySound::ChainClank2
        };
        world.emit_action_sound(DeathEffectOwner::from(&*actor), context.tick, sound, 1.0);
        brain.torso_phase = fround(brain.torso_phase - 3.0);
    }
    for crow in &mut brain.crows {
        crow.light_intensity = crow.position.map_or(0.0, |position| {
            context.native_visibility.map_or(1.0, |query| query(position).intensity)
        });
    }
    actor.body_pose = brain.torso_phase.trunc();
    actor.brain = EnemyBrain::Heartmonger(brain);
}

impl EnemyWorld {
    pub(super) fn spawn_heartmonger_departure(
        &mut self,
        actor: &EnemyActor,
        tick: u64,
        order: &mut WorldPainterOrder,
    ) {
        use super::boss_spell::BossSpellKind;
        self.emit_action_sound(DeathEffectOwner::from(actor), tick, EnemySound::ChainClank1, 1.0);
        self.emit_action_sound(DeathEffectOwner::from(actor), tick, EnemySound::ChainClank2, 1.0);
        let mut event = EnemyEvent::new(EnemyEventKind::EnemyStream, actor.id, tick);
        event.stream = Some("heart-break".to_owned());
        event.source_position = Some(actor.position);
        self.emit_event(event);
        let flicker = self.create_boss_spell(
            actor.id,
            tick,
            0.0,
            actor.position,
            BossSpellKind::HeartmongerFlicker { phase_deg: 0.0 },
            order,
        );
        self.boss_spells.push(flicker);
        let bob_phase_deg = self.steering_rng.draw_scaled_float(360.0, false);
        let soul = self.create_boss_spell(
            actor.id,
            tick,
            0.0,
            actor.position,
            BossSpellKind::HeartmongerSoul { life_phase_deg: 0.0, bob_phase_deg },
            order,
        );
        self.boss_spells.push(soul);
    }
    pub fn detach_heartmonger_crows(&mut self, actor: &EnemyActor, tick: u64) {
        let EnemyBrain::Heartmonger(brain) = &actor.brain else { return };
        let crows = detach_native_heartmonger_crows(
            brain,
            &mut self.steering_rng,
            actor.position,
            self.next_death_effect_id,
        );
        self.next_death_effect_id += 1;
        for flight in crows {
            self.detached_crows.push(DetachedCrow {
                flight,
                owner_actor_id: actor.id,
                spawn_tick: tick,
            });
        }
        self.emit_action_sound(DeathEffectOwner::from(actor), tick, EnemySound::Wings, 1.0);
    }

    pub fn step_detached_crows(&mut self, context: &EnemyStepContext<'_>) {
        for source in std::mem::take(&mut self.detached_crows) {
            let step = step_native_crow(
                source.flight,
                &mut self.steering_rng,
                None,
                false,
                &[],
                context.projected_point_visible.unwrap_or(&|_| true),
            );
            if let Some(mut flight) = step.crow {
                let position = flight.position.expect("a surviving detached crow has a root");
                flight.light_intensity =
                    context.native_visibility.map_or(1.0, |query| query(position).intensity);
                self.detached_crows.push(DetachedCrow { flight, ..source });
            }
        }
    }
}
