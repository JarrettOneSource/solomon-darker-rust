//! Faculty spell births, contacts and retained ticks from `enemies/boss-spells.ts`.

use super::boss_spell::{BossSpellKind, BossSpellLight, NativeBossSpell};
use super::config::EnemyFamilyConfig;
use super::effects::{
    BouncerOptions, DeathEffectOverrides, DeathEffectOwner, SimpleDeathEffectSpec,
};
use super::faculty_attachments::FACULTY_HAND_ATTACHMENTS;
use super::faculty_effects::{
    FacultyProjectileSmoke, spawn_blightning_smoke, spawn_faculty_projectile_smoke,
    spawn_tragic_circle_effects,
};
use super::faculty_spells::*;
use super::guided_missile::{create_native_guided_missile, step_native_guided_missile};
use super::model::*;
use super::movement::positive_modulo;
use super::projectile_effects::{ProjectileEffectOwner, spawn_fire_burst};
use super::projectile_motion::first_guided_missile_contact;
use super::targeting::{target_by_id, target_eligible};
use super::world::{EnemyProjectileWorldQuery, EnemyStepContext, EnemyWorld};
use crate::Vec2;
use crate::float::{fround, hypot};
use crate::heading::heading_degrees_from_vector;
use crate::line_obstruction::clip_line_to_bounds;
use crate::painter::WorldPainterOrder;

#[derive(Clone, Copy, Debug, Default)]
struct ContactModifiers {
    mana_damage_maximum_fraction: Option<f64>,
    suppress_hit_response: bool,
    tragic_circle: bool,
    source: Option<EnemyPlayerDamageSource>,
}

pub fn spawn_faculty_spell(
    world: &mut EnemyWorld,
    actor: &impl EnemyActorRead,
    action: FacultyActionKind,
    context: &EnemyStepContext<'_>,
    order: &mut WorldPainterOrder,
) {
    let EnemyFamilyConfig::Faculty(family) = &actor.config.family else {
        panic!("Faculty cast requires Faculty config")
    };
    let primary_damage = actor.config.primary_damage.unwrap_or(0.0);
    let secondary_damage = actor.config.secondary_damage;
    let target = target_by_id(actor.target_player_id.as_deref(), context.players);
    let heading = target.map_or(actor.heading_deg, |target| {
        heading_degrees_from_vector(
            target.position.x - actor.position.x,
            target.position.y - actor.position.y,
        )
    });
    if action == FacultyActionKind::Lightning {
        spawn_blightning(world, actor, context, order);
        for player in context.players {
            if player.summoned
                || !target_eligible(player)
                || !native_blightning_contains(actor.position, actor.heading_deg, player.position)
                || !line_clear(context, actor.position, player.position)
            {
                continue;
            }
            contact(
                world,
                context,
                actor.id,
                &player.id,
                0.0,
                primary_damage / 100.0,
                ContactModifiers {
                    mana_damage_maximum_fraction: Some(0.005),
                    suppress_hit_response: true,
                    ..ContactModifiers::default()
                },
            );
        }
        return;
    }
    if action == FacultyActionKind::Throw && family.primary == 0 {
        let direction = heading_vector(heading);
        let position = Vec2::new(
            fround(actor.position.x + direction.x * 40.0),
            fround(actor.position.y + direction.y * 40.0 - 30.0),
        );
        let state = create_native_guided_missile(&mut world.steering_rng, position, heading, 1.5);
        let spell = world.create_boss_spell(
            actor.id,
            context.tick,
            primary_damage,
            state.position,
            BossSpellKind::SkullMissile {
                heading_deg: state.heading_deg,
                minimum_speed: state.minimum_speed,
                phase_deg: state.phase_deg,
                remaining_ticks: state.remaining_ticks,
                speed: state.speed,
                turn_rate: state.turn_rate,
                visual_scale: state.visual_scale,
                target_player_id: actor.target_player_id.clone(),
            },
            order,
        );
        if projectile_line_clear(context, &spell, actor.position, spell.owner.position, 0x380) {
            world.bind_native_query_target(
                &format!("boss-spell:{}", spell.owner.id),
                spell.owner.position,
            );
            world.boss_spells.push(spell);
        } else {
            spell_impact(world, &spell, context, None, order);
            retire_projectile(world, &spell, context.tick);
        }
        emit_faculty_throw(world, actor, context.tick);
        return;
    }
    if (action == FacultyActionKind::Throw && family.primary == 2)
        || (action == FacultyActionKind::TwoHand && family.secondary == 2)
    {
        let ring = action == FacultyActionKind::TwoHand;
        let distance = target.map(|target| {
            hypot(target.position.x - actor.position.x, target.position.y - actor.position.y)
        });
        let states = create_native_dark_fireballs(
            actor.position,
            heading,
            ring,
            distance,
            &mut world.steering_rng,
        );
        for state in states {
            let spell = world.create_boss_spell(
                actor.id,
                context.tick,
                primary_damage,
                state.position,
                BossSpellKind::DarkFireball {
                    arc_phase: state.arc_phase,
                    arc_phase_step: state.arc_phase_step,
                    heading_deg: state.heading_deg,
                    remaining_ticks: state.remaining_ticks,
                    velocity: state.velocity,
                    ground_fire_damage: secondary_damage,
                },
                order,
            );
            if projectile_line_clear(context, &spell, actor.position, spell.owner.position, 0x380) {
                world.bind_native_query_target(
                    &format!("boss-spell:{}", spell.owner.id),
                    spell.owner.position,
                );
                world.boss_spells.push(spell);
            } else {
                impact_dark(world, &spell, context, order);
                retire_projectile(world, &spell, context.tick);
            }
        }
        if ring {
            world.emit_action_sound(
                DeathEffectOwner::from(actor),
                context.tick,
                EnemySound::BansheeDie,
                1.0,
            );
            world.emit_action_sound(
                DeathEffectOwner::from(actor),
                context.tick,
                EnemySound::BigFire,
                1.0,
            );
        } else {
            emit_faculty_throw(world, actor, context.tick);
        }
        return;
    }
    if action != FacultyActionKind::TwoHand {
        return;
    }
    if family.secondary == 0 {
        let point = target.map_or_else(
            || advance(actor.position, heading_vector(actor.heading_deg), 300.0),
            |target| advance(target.position, target.velocity_per_tick, 250.0),
        );
        let state = create_native_rain_of_bones(point, &mut world.steering_rng);
        let mut spell = world.create_boss_spell(
            actor.id,
            context.tick,
            secondary_damage,
            point,
            BossSpellKind::RainOfBones {
                alpha: state.alpha,
                phase: state.phase,
                remaining_ticks: state.remaining_ticks,
                rotation_sign: state.rotation_sign,
                scale: state.scale,
            },
            order,
        );
        let distance = world.steering_rng.draw_scaled_float(50.0, false);
        let angle = world.steering_rng.draw_scaled_float(360.0, false);
        spell.owner.position = advance(point, heading_vector(angle), distance);
        world.boss_spells.push(spell);
        world.emit_action_sound(
            DeathEffectOwner::from(actor),
            context.tick,
            EnemySound::MagicStorm,
            0.8,
        );
        world.emit_action_sound(
            DeathEffectOwner::from(actor),
            context.tick,
            EnemySound::BansheeDie,
            1.0,
        );
        return;
    }
    let Some(target) = target.filter(|_| family.secondary == 1) else { return };
    let lead = world.steering_rng.draw_float_range(130.0, 250.0);
    let ahead = world.steering_rng.draw_scaled_float(75.0, false);
    let point = advance(
        advance(target.position, target.velocity_per_tick, lead),
        heading_vector(target.heading_deg.unwrap_or(0.0)),
        ahead,
    );
    let spell = world.create_boss_spell(
        actor.id,
        context.tick,
        0.0,
        point,
        BossSpellKind::TragicCircle { remaining_ticks: 750 },
        order,
    );
    world.boss_spells.push(spell);
    world.emit_action_sound(
        DeathEffectOwner::from(actor),
        context.tick,
        EnemySound::MagicCircle,
        0.8,
    );
    world.emit_action_sound(
        DeathEffectOwner::from(actor),
        context.tick,
        EnemySound::BansheeDie,
        1.0,
    );
}

fn emit_faculty_throw(world: &mut EnemyWorld, actor: &impl EnemyActorRead, tick: u64) {
    let first_pitch = 1.5 - world.draw_unit() * 0.25;
    world.emit_action_sound(
        DeathEffectOwner::from(actor),
        tick,
        EnemySound::SpinAttack,
        first_pitch,
    );
    let second_pitch = 1.0 + world.draw_unit() * 0.25;
    world.emit_action_sound(
        DeathEffectOwner::from(actor),
        tick,
        EnemySound::ThrowDark,
        second_pitch,
    );
}

impl EnemyWorld {
    pub fn step_boss_spells(
        &mut self,
        context: &mut EnemyStepContext<'_>,
        order: &mut WorldPainterOrder,
    ) {
        let population = std::mem::take(&mut self.boss_spells);
        let has_long_ultra_banish=population.iter().any(|spell|matches!(spell.kind,BossSpellKind::UltraBanish {remaining_ticks,..} if remaining_ticks>100));
        for mut spell in population {
            if spell.owner.spawn_tick >= context.tick {
                self.boss_spells.push(spell);
                continue;
            }
            match &mut spell.kind {
                BossSpellKind::UltraBanish { .. } | BossSpellKind::UnholySoul { .. } => {
                    if let Some(spell) = super::ultra_banish::step_ultra_banish_spell(
                        self,
                        spell,
                        context,
                        has_long_ultra_banish,
                        order,
                    ) {
                        self.boss_spells.push(spell);
                    }
                }
                BossSpellKind::EyeLaser { .. }
                | BossSpellKind::UnholySpit { .. }
                | BossSpellKind::GreenFire { .. }
                | BossSpellKind::UnholyBurst { .. }
                | BossSpellKind::MouthBeamSegment { .. } => {
                    if let Some(spell) = super::demon_skull_spells::step_demon_skull_spell(
                        self, spell, context, order,
                    ) {
                        self.boss_spells.push(spell);
                    }
                }
                BossSpellKind::HeartmongerFlicker { phase_deg } => {
                    *phase_deg = fround(*phase_deg + 0.333_333_343_267_440_8);
                    if *phase_deg < 180.0 {
                        spell.owner.age_ticks += 1;
                        self.boss_spells.push(spell);
                    }
                }
                BossSpellKind::HeartmongerSoul { life_phase_deg, bob_phase_deg } => {
                    *life_phase_deg = fround(*life_phase_deg + 0.366_666_674_613_952_64);
                    if *life_phase_deg < 180.0 {
                        *bob_phase_deg = fround(*bob_phase_deg + 2.0);
                        spell.owner.age_ticks += 1;
                        self.boss_spells.push(spell);
                    }
                }
                BossSpellKind::DeathMagic { alpha, alpha_loss_per_tick, light, .. } => {
                    *alpha = fround(*alpha - *alpha_loss_per_tick);
                    if *alpha > 0.0 {
                        if let Some(light) = light {
                            light.intensity =
                                fround(light.intensity - light.loss_per_tick).max(0.0);
                        }
                        spell.owner.age_ticks += 1;
                        self.boss_spells.push(spell);
                    }
                }
                BossSpellKind::Blightning { .. } => {
                    spell.owner.age_ticks += 1;
                    if spell.owner.age_ticks < 2 {
                        self.boss_spells.push(spell);
                    }
                }
                BossSpellKind::FallingBone { .. } => step_falling_bone(self, spell, context, order),
                BossSpellKind::SkullMissile { .. } => step_skull(self, spell, context, order),
                BossSpellKind::DarkFireball { .. } => step_dark(self, spell, context, order),
                BossSpellKind::RainOfBones { .. } => step_rain(self, spell, context, order),
                BossSpellKind::TragicCircle { .. } => step_circle(self, spell, context, order),
                BossSpellKind::DireFire(_) => step_fire(self, spell, context),
            }
        }
    }
}

fn spawn_blightning(
    world: &mut EnemyWorld,
    actor: &impl EnemyActorRead,
    context: &EnemyStepContext<'_>,
    order: &mut WorldPainterOrder,
) {
    let EnemyBrain::Faculty(brain) = actor.brain() else {
        panic!("Blightning requires a Faculty caster")
    };
    let start = native_faculty_lightning_source(
        actor.position,
        actor.body_pose,
        brain.body_heading_deg,
        brain.hand_mask,
    );
    let heading = heading_vector(actor.heading_deg);
    let far = advance(start, heading, 5000.0);
    let line = context
        .native_view_bounds
        .map_or(Some((start, far)), |bounds| clip_line_to_bounds(start, far, bounds));
    let Some((start, end)) = line else { return };
    let terrain_end = context.geometry.clip_spell_segment(start, end);
    let obstructed = distance_squared(terrain_end, end) > 1e-8;
    let endpoint =
        if obstructed { Vec2::new(terrain_end.x, terrain_end.y - 20.0) } else { terrain_end };
    if obstructed {
        spawn_blightning_fade(world, actor.id, context.tick, endpoint, order);
    }
    let half_distance = hypot(endpoint.x - start.x, endpoint.y - start.y) * 0.5;
    let spell = world.create_boss_spell(
        actor.id,
        context.tick,
        0.0,
        start,
        BossSpellKind::Blightning {
            midpoint: advance(start, heading, half_distance),
            endpoint,
            enhanced_effects: context.enhanced_effects,
        },
        order,
    );
    world.boss_spells.push(spell);
    spawn_blightning_fade(world, actor.id, context.tick, start, order);
    spawn_blightning_smoke(
        world,
        DeathEffectOwner::from(actor),
        context.tick,
        start,
        endpoint,
        order,
    );
}

#[must_use]
pub fn native_faculty_lightning_source(root: Vec2, pose: f64, heading: f64, hand_mask: u8) -> Vec2 {
    let facing = positive_modulo(((heading + 10.0) / 20.0).trunc(), 18.0) as usize;
    let points = &FACULTY_HAND_ATTACHMENTS[pose.trunc() as usize][facing];
    let mut position = Vec2::new(root.x, root.y - 15.0);
    for (hand, point) in points.iter().enumerate() {
        if hand_mask & (1 << hand) != 0 {
            position.x = fround(position.x + point.x);
            position.y = fround(position.y + point.y);
        }
    }
    position
}

fn spawn_blightning_fade(
    world: &mut EnemyWorld,
    owner: usize,
    tick: u64,
    position: Vec2,
    order: &mut WorldPainterOrder,
) {
    let scale = world.steering_rng.draw_float_range(1.5, 2.0);
    let spell = world.create_boss_spell(
        owner,
        tick,
        0.0,
        position,
        BossSpellKind::DeathMagic {
            scale,
            alpha: 2.0,
            alpha_loss_per_tick: 1.0,
            light: None,
            painter_sort_bias: 25.0,
        },
        order,
    );
    world.boss_spells.push(spell);
}

fn step_skull(
    world: &mut EnemyWorld,
    mut spell: NativeBossSpell,
    context: &EnemyStepContext<'_>,
    order: &mut WorldPainterOrder,
) {
    let BossSpellKind::SkullMissile { target_player_id, .. } = &spell.kind else { unreachable!() };
    let target = target_by_id(target_player_id.as_deref(), context.players)
        .filter(|target| target_eligible(target));
    let motion = step_native_guided_missile(
        spell.guided_missile_state(),
        target.map(|target| target.position),
        1.0,
    );
    spell.set_guided_missile_state(motion.state);
    spell.owner.age_ticks += 1;
    if target.is_none()
        && let BossSpellKind::SkullMissile { target_player_id, .. } = &mut spell.kind
    {
        *target_player_id = None;
    }
    world.bind_native_query_target(&format!("boss-spell:{}", spell.owner.id), spell.owner.position);
    let outside = context.geometry.projectile_world_blocked(EnemyProjectileWorldQuery::Bounds {
        projectile_id: spell.owner.id,
        position: spell.owner.position,
        margin: 500.0,
    });
    let mut impacted = false;
    if !outside {
        let BossSpellKind::SkullMissile { target_player_id, .. } = &spell.kind else {
            unreachable!()
        };
        if let Some(player) = first_guided_missile_contact(
            world,
            spell.owner.position,
            target_player_id.as_deref(),
            context.players,
        ) {
            spell_impact(world, &spell, context, Some(&player.id), order);
            contact(
                world,
                context,
                spell.owner.owner_actor_id,
                &player.id,
                0.0,
                spell.owner.damage,
                ContactModifiers {
                    mana_damage_maximum_fraction: Some(0.15),
                    source: Some(contact_source(&spell, 0.0)),
                    ..ContactModifiers::default()
                },
            );
            let mut event = EnemyEvent::new(
                EnemyEventKind::PlayerStatusSound,
                spell.owner.owner_actor_id,
                context.tick,
            );
            event.target_player_id = Some(Some(player.id.clone()));
            event.source_position = Some(spell.owner.position);
            event.sound = Some(EnemySound::MagicMissileHit);
            event.pitch = Some(0.85);
            event.gain_scale = Some(1.0);
            world.emit_event(event);
            impacted = true;
        }
        if spell.owner.age_ticks.is_multiple_of(5)
            && !projectile_line_clear(
                context,
                &spell,
                spell.owner.position,
                motion.terrain_probe,
                0x700,
            )
        {
            spell_impact(world, &spell, context, None, order);
            impacted = true;
        }
    }
    spawn_faculty_projectile_smoke(
        world,
        effect_owner(&spell),
        context.tick,
        FacultyProjectileSmoke::SkullTrail,
        0.0,
        order,
    );
    if outside || impacted || motion.state.remaining_ticks <= 0.0 {
        retire_projectile(world, &spell, context.tick);
    } else {
        world.boss_spells.push(spell);
    }
}

fn step_dark(
    world: &mut EnemyWorld,
    mut spell: NativeBossSpell,
    context: &EnemyStepContext<'_>,
    order: &mut WorldPainterOrder,
) {
    let source = spell.dark_fireball_state();
    let step = step_native_dark_fireball(source, 1.0);
    let obstruction = step.spell.age_ticks.is_multiple_of(10)
        && !projectile_line_clear(
            context,
            &spell,
            source.position,
            advance(source.position, source.velocity, 10.0),
            0x700,
        );
    spell.set_dark_fireball_state(step.spell);
    if obstruction {
        spell.owner.position = source.position;
    }
    let mut impacted = false;
    let mut outside = false;
    if obstruction {
        impact_dark(world, &spell, context, order);
        impacted = true;
    } else {
        outside = context.geometry.projectile_world_blocked(EnemyProjectileWorldQuery::View {
            projectile_id: spell.owner.id,
            position: spell.owner.position,
            margin: 50.0,
        });
        if context.tick.is_multiple_of(2) {
            spawn_faculty_projectile_smoke(
                world,
                effect_owner(&spell),
                context.tick,
                FacultyProjectileSmoke::DarkTrail,
                source.arc_phase,
                order,
            );
        }
        let hit = context.players.iter().any(|player| {
            !player.summoned
                && target_eligible(player)
                && distance_squared(spell.owner.position, player.position) < 900.0
        });
        if hit {
            impact_dark(world, &spell, context, order);
            impacted = true;
        }
    }
    // The subclass impact still runs after an inherited player or terrain impact.
    if step.impact {
        impact_dark(world, &spell, context, order);
        impacted = true;
    }
    if impacted || outside || step.spell.remaining_ticks <= 0.0 {
        retire_projectile(world, &spell, context.tick);
    } else {
        world.boss_spells.push(spell);
    }
}

fn impact_dark(
    world: &mut EnemyWorld,
    spell: &NativeBossSpell,
    context: &EnemyStepContext<'_>,
    order: &mut WorldPainterOrder,
) {
    let state = spell.dark_fireball_state();
    if state.arc_phase_step > 0.0 {
        spawn_faculty_projectile_smoke(
            world,
            effect_owner(spell),
            context.tick,
            FacultyProjectileSmoke::DarkRingImpact,
            0.0,
            order,
        );
        spawn_dire_fires(world, spell, context.tick, order);
    }
    for player in context.players {
        if target_eligible(player)
            && distance_squared(spell.owner.position, player.position)
                < 150.0 * 150.0 + player.collision_radius * player.collision_radius
        {
            contact(
                world,
                context,
                spell.owner.owner_actor_id,
                &player.id,
                spell.owner.damage * 0.5,
                spell.owner.damage * 0.5,
                ContactModifiers {
                    source: Some(contact_source(spell, 20.0)),
                    ..ContactModifiers::default()
                },
            );
        }
    }
    if state.arc_phase_step == 0.0 {
        spawn_fire_burst(
            world,
            ProjectileEffectOwner {
                id: spell.owner.id,
                owner_actor_id: spell.owner.owner_actor_id,
                age_ticks: spell.owner.age_ticks,
            },
            context.tick,
            spell.owner.position,
            1.75,
            0.25,
            order,
        );
    } else {
        world.emit_action_sound(effect_owner(spell), context.tick, EnemySound::FireballHit, 2.0);
    }
    spell_impact(world, spell, context, None, order);
}

fn retire_projectile(world: &mut EnemyWorld, spell: &NativeBossSpell, tick: u64) {
    let mut event =
        EnemyEvent::new(EnemyEventKind::ProjectileRetired, spell.owner.owner_actor_id, tick);
    event.projectile_id = Some(spell.owner.id);
    event.target_player_id =
        Some(if let BossSpellKind::SkullMissile { target_player_id, .. } = &spell.kind {
            target_player_id.clone()
        } else {
            None
        });
    world.emit_event(event);
}

fn step_rain(
    world: &mut EnemyWorld,
    mut spell: NativeBossSpell,
    context: &EnemyStepContext<'_>,
    order: &mut WorldPainterOrder,
) {
    let step = step_native_rain_of_bones(spell.rain_state(), &mut world.steering_rng);
    if let Some(bone) = step.bone {
        let falling = world.create_boss_spell(
            spell.owner.owner_actor_id,
            context.tick,
            0.0,
            bone.position,
            BossSpellKind::FallingBone {
                entry: bone.entry,
                height: -175.0,
                color_ramp: 0.0,
                rotation_deg: bone.rotation_deg,
            },
            order,
        );
        world.boss_spells.push(falling);
        for player in context.players {
            if !player.summoned
                && target_eligible(player)
                && distance_squared(bone.position, player.position) < 3600.0
            {
                contact(
                    world,
                    context,
                    spell.owner.owner_actor_id,
                    &player.id,
                    spell.owner.damage,
                    0.0,
                    ContactModifiers {
                        mana_damage_maximum_fraction: Some(0.15),
                        source: Some(contact_source(&spell, 20.0)),
                        ..ContactModifiers::default()
                    },
                );
            }
        }
    }
    if step.state.alpha > 0.0 {
        let state = step.state;
        spell.owner.age_ticks += 1;
        spell.kind = BossSpellKind::RainOfBones {
            alpha: state.alpha,
            phase: state.phase,
            remaining_ticks: state.remaining_ticks,
            rotation_sign: state.rotation_sign,
            scale: state.scale,
        };
        world.boss_spells.push(spell);
    }
}

fn step_falling_bone(
    world: &mut EnemyWorld,
    mut spell: NativeBossSpell,
    context: &EnemyStepContext<'_>,
    order: &mut WorldPainterOrder,
) {
    let BossSpellKind::FallingBone { entry, height, color_ramp, .. } = &mut spell.kind else {
        unreachable!()
    };
    let fall = world.steering_rng.draw_scaled_float(2.0, false);
    *height = fround(fround(fall + 5.0) + *height);
    if *height < 0.0 {
        *color_ramp = fround(*color_ramp + 0.100_000_001_490_116_12).min(1.0);
        spell.owner.age_ticks += 1;
        world.boss_spells.push(spell);
        return;
    }
    let entry = *entry;
    let pitch = 0.8 + world.draw_unit() * 0.3;
    let gain = world.draw_unit();
    let mut event =
        EnemyEvent::new(EnemyEventKind::EnemyActionSound, spell.owner.owner_actor_id, context.tick);
    event.source_position = Some(spell.owner.position);
    event.sound = Some(EnemySound::Knock);
    event.pitch = Some(pitch);
    event.gain_scale = Some(gain);
    world.emit_event(event);
    let angle = world.steering_rng.draw_scaled_float(360.0, false);
    let distance = world.steering_rng.draw_scaled_float(10.0, false);
    let direction = heading_vector(angle);
    let velocity = Vec2::new(fround(direction.x * 1.5), direction.y);
    let position = Vec2::new(
        fround(spell.owner.position.x + (15.0 + distance) * velocity.x + velocity.x * 2.0),
        fround(spell.owner.position.y + (15.0 + distance) * velocity.y),
    );
    world.spawn_bouncer(
        effect_owner(&spell),
        context.tick,
        |_| entry,
        "acid-pain-bone-landed",
        |_| BouncerOptions {
            opacity_timer: Some(2.0),
            scale: Some(1.200_000_047_683_715_8),
            velocity: Some(velocity),
            position: Some(position),
            ..BouncerOptions::default()
        },
        context.enhanced_effects,
        order,
    );
}

fn step_circle(
    world: &mut EnemyWorld,
    mut spell: NativeBossSpell,
    context: &EnemyStepContext<'_>,
    order: &mut WorldPainterOrder,
) {
    let BossSpellKind::TragicCircle { remaining_ticks } = &mut spell.kind else { unreachable!() };
    *remaining_ticks -= 1;
    let remaining = *remaining_ticks;
    let source_age = spell.owner.age_ticks;
    spell.owner.age_ticks += 1;
    spawn_tragic_circle_effects(world, effect_owner(&spell), context.tick, remaining, order);
    if source_age.is_multiple_of(10) {
        for player in context.players {
            if !target_eligible(player)
                || !native_tragic_circle_contains(spell.owner.position, player.position)
            {
                continue;
            }
            let rotation = world.steering_rng.draw_scaled_float(360.0, false);
            let scale =
                fround(1.0 + world.steering_rng.draw_scaled_float(1.0, false) * fround(0.65));
            let alpha = fround(0.5 + world.steering_rng.draw_scaled_float(0.25, false));
            world.spawn_simple_death_effect(
                DeathEffectOwner { id: spell.owner.owner_actor_id, position: player.position },
                context.tick,
                SimpleDeathEffectSpec {
                    alpha,
                    alpha_loss_per_tick: super::effects::NATIVE_TRAGIC_CONTACT_ALPHA_LOSS,
                    atlas: EnemyEffectAtlas::BadGuys,
                    entry: 7,
                    blend_mode: EnemyEffectBlend::Add,
                    kind: EnemyDeathEffectKind::FadeScale,
                    lifetime_ticks: 1000,
                    role: "tragic-circle-contact",
                    scale,
                },
                DeathEffectOverrides {
                    painter_sort_bias: Some(50.0),
                    position: Some(Vec2::new(
                        fround(player.position.x),
                        fround(player.position.y - 15.0),
                    )),
                    rotation_deg: Some(rotation),
                    scale_multiplier: Some(fround(1.1)),
                    tint: Some(0x80_00_00),
                    ..DeathEffectOverrides::default()
                },
                order,
            );
            contact(
                world,
                context,
                spell.owner.owner_actor_id,
                &player.id,
                0.0,
                0.0,
                ContactModifiers { tragic_circle: true, ..ContactModifiers::default() },
            );
        }
    }
    if remaining > 0 {
        world.boss_spells.push(spell);
    }
}

fn step_fire(world: &mut EnemyWorld, mut spell: NativeBossSpell, context: &EnemyStepContext<'_>) {
    let BossSpellKind::DireFire(source) = spell.kind else { unreachable!() };
    let state = step_native_dire_fire(source, &mut world.steering_rng);
    if context.tick.is_multiple_of(3) {
        for player in context.players {
            if !target_eligible(player)
                || distance_squared(spell.owner.position, player.position)
                    >= (32.0 * source.scale).powi(2)
            {
                continue;
            }
            contact(
                world,
                context,
                spell.owner.owner_actor_id,
                &player.id,
                spell.owner.damage * 0.015,
                spell.owner.damage * 0.015,
                ContactModifiers {
                    suppress_hit_response: true,
                    source: Some(contact_source(&spell, 20.0)),
                    ..ContactModifiers::default()
                },
            );
        }
    }
    if state.alpha > 0.0 {
        spell.kind = BossSpellKind::DireFire(state);
        spell.owner.age_ticks += 1;
        world.boss_spells.push(spell);
    }
}

fn spawn_dire_fires(
    world: &mut EnemyWorld,
    source: &NativeBossSpell,
    tick: u64,
    order: &mut WorldPainterOrder,
) {
    let BossSpellKind::DarkFireball { ground_fire_damage, .. } = source.kind else {
        unreachable!()
    };
    let mut first = create_native_dire_fire(&mut world.steering_rng);
    let scale = world.steering_rng.draw_scaled_float(0.75, false);
    let alpha = world.steering_rng.draw_scaled_float(5.0, false);
    first.alpha = fround(5.0 + alpha);
    first.scale = fround(0.75 + scale);
    let spell = world.create_boss_spell(
        source.owner.owner_actor_id,
        tick,
        ground_fire_damage,
        source.owner.position,
        BossSpellKind::DireFire(first),
        order,
    );
    world.boss_spells.push(spell);
    let mut second = create_native_dire_fire(&mut world.steering_rng);
    let offset = world.steering_rng.draw_scaled_float(10.0, false);
    let sign = world.steering_rng.draw_sign(fround(10.0 + offset));
    let size = world.steering_rng.draw_scaled_float(0.200_000_002_980_232_24, false);
    second.alpha = 5.0;
    second.glow = false;
    second.scale = fround(0.600_000_023_841_857_9 + size);
    let position =
        Vec2::new(fround(source.owner.position.x + sign), fround(source.owner.position.y + 5.0));
    let spell = world.create_boss_spell(
        source.owner.owner_actor_id,
        tick,
        source.owner.damage,
        position,
        BossSpellKind::DireFire(second),
        order,
    );
    world.boss_spells.push(spell);
}

fn spell_impact(
    world: &mut EnemyWorld,
    spell: &NativeBossSpell,
    context: &EnemyStepContext<'_>,
    target: Option<&str>,
    order: &mut WorldPainterOrder,
) {
    let mut event =
        EnemyEvent::new(EnemyEventKind::ProjectileImpact, spell.owner.owner_actor_id, context.tick);
    event.projectile_id = Some(spell.owner.id);
    event.target_player_id = Some(target.map(str::to_owned));
    world.emit_event(event);
    let skull = matches!(spell.kind, BossSpellKind::SkullMissile { .. });
    if skull {
        let pitch = fround(1.0 + world.steering_rng.draw_scaled_float(0.1, false));
        world.emit_action_sound(
            effect_owner(spell),
            context.tick,
            EnemySound::MagicMissileHit,
            pitch,
        );
        let fade = world.create_boss_spell(
            spell.owner.owner_actor_id,
            context.tick,
            0.0,
            spell.owner.position,
            BossSpellKind::DeathMagic {
                scale: 2.0,
                alpha: 2.0,
                alpha_loss_per_tick: fround(0.02),
                painter_sort_bias: 100.0,
                light: Some(BossSpellLight {
                    radius: 0.75,
                    intensity: 1.0,
                    loss_per_tick: fround(0.05),
                }),
            },
            order,
        );
        world.boss_spells.push(fade);
    }
    world.emit_action_sound(effect_owner(spell), context.tick, EnemySound::ThrowDark, 1.0);
    spawn_faculty_projectile_smoke(
        world,
        effect_owner(spell),
        context.tick,
        if skull {
            FacultyProjectileSmoke::SkullImpact
        } else {
            FacultyProjectileSmoke::DarkImpact
        },
        0.0,
        order,
    );
    let mut event =
        EnemyEvent::new(EnemyEventKind::EnemyScreenFlash, spell.owner.owner_actor_id, context.tick);
    event.source_position = Some(spell.owner.position);
    event.screen_flash = Some(EnemyScreenFlash {
        alpha: 1.0,
        red: 0.0,
        green: 0.0,
        blue: 0.0,
        decay_per_tick: fround(0.05),
        point_attenuated: true,
    });
    world.emit_event(event);
}

fn contact(
    world: &mut EnemyWorld,
    context: &EnemyStepContext<'_>,
    actor_id: usize,
    player_id: &str,
    physical: f64,
    magic: f64,
    modifiers: ContactModifiers,
) {
    let mut event = EnemyEvent::new(EnemyEventKind::AttackMarker, actor_id, context.tick);
    event.target_player_id = Some(Some(player_id.to_owned()));
    let event_id = world.emit_event(event);
    let mut damage = EnemyPlayerDamage::new(actor_id, event_id, player_id.to_owned());
    damage.physical_damage = fround(physical);
    damage.magic_damage = fround(magic);
    damage.mana_damage_maximum_fraction = modifiers.mana_damage_maximum_fraction;
    damage.suppress_hit_response = modifiers.suppress_hit_response;
    damage.tragic_circle = modifiers.tragic_circle;
    damage.source = modifiers.source.map(Some);
    world.player_damage.push(damage);
}

fn effect_owner(spell: &NativeBossSpell) -> DeathEffectOwner {
    DeathEffectOwner { id: spell.owner.owner_actor_id, position: spell.owner.position }
}

fn contact_source(spell: &NativeBossSpell, collision_radius: f64) -> EnemyPlayerDamageSource {
    EnemyPlayerDamageSource {
        position: spell.owner.position,
        collision_radius,
        reflectable_actor_id: None,
    }
}

fn advance(point: Vec2, direction: Vec2, distance: f64) -> Vec2 {
    Vec2::new(fround(point.x + direction.x * distance), fround(point.y + direction.y * distance))
}

fn distance_squared(first: Vec2, second: Vec2) -> f64 {
    let dx = first.x - second.x;
    let dy = first.y - second.y;
    dx * dx + dy * dy
}

fn line_clear(context: &EnemyStepContext<'_>, start: Vec2, end: Vec2) -> bool {
    distance_squared(context.geometry.clip_spell_segment(start, end), end) < 1e-8
}

fn projectile_line_clear(
    context: &EnemyStepContext<'_>,
    spell: &NativeBossSpell,
    start: Vec2,
    end: Vec2,
    mask: u32,
) -> bool {
    !context.geometry.projectile_world_blocked(EnemyProjectileWorldQuery::Line {
        projectile_id: spell.owner.id,
        start,
        end,
        native_exclusion_mask: mask,
    })
}
