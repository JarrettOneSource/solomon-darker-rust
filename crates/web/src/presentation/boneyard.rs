//! World/encounter/loot projection from `client/boneyard-presentation-timeline.ts`.

use std::collections::BTreeMap;

use solomon_sim::boneyard::BoneyardBounds;
use solomon_sim::boneyard::loot::LootActorRead;
use solomon_sim::boneyard::world_light::{
    BoneyardLightEnvironment, WorldLightPlayer, WorldLightScene, boneyard_world_light_query,
};
use solomon_sim::combat::PuppetHitState;
use solomon_sim::enemies::actor_presentation::{
    EnemyActorAppearanceContext, EnemySpiderOutlineTarget,
};
use solomon_sim::enemies::boss_spell::{BossSpellKind, BossSpellRead, BossSpells};
use solomon_sim::enemies::model::EnemyBrain;
use solomon_sim::enemies::puppet_hits::{EnemyPuppetHit, EnemyPuppetHitKind};
use solomon_sim::float::fround;
use solomon_sim::game_run::GameRunPhase;
use solomon_sim::session::GameSession;

use super::PresentationClocks;
use super::enemies::{self, EnemyPresentationMetadata};
use super::math::{interpolate_members, interpolate_projected_members, lerp, lerp_cycle, lerp_vec};

/// `host/game-snapshot.ts` samples Spider appearance with the host's logical
/// light query and fixed stock viewport, before any client camera projection.
pub(super) fn capture_enemies(session: &GameSession) -> Option<EnemyPresentationMetadata> {
    let world = session.boneyard.as_ref()?;
    let players: Vec<_> = session
        .participant_order
        .iter()
        .map(|id| {
            let participant = &session.participants.get(id).expect("participant exists");
            WorldLightPlayer {
                id,
                position: participant.wizard.position,
                heading_index: participant.wizard.heading_index,
                weapon_pulse: participant.wizard.primary_cast.weapon_pulse,
                overlay_effect_phase: participant.lighting.overlay_effect_phase,
                light_registration: Some(participant.lighting.light_registration),
                viewport: None,
            }
        })
        .collect();
    let light = boneyard_world_light_query(
        WorldLightScene {
            bounds: world.loaded.scene.bounds,
            camera_bounds: world.camera_bounds(),
            lantern_position: world.lantern_position,
            lantern_light_registration: world.lantern_light_registration,
        },
        &world.loaded.run_id,
        &players,
        &world.enemies,
        session.ticks,
        BoneyardLightEnvironment {
            primary_spells: Some(&session.spells),
            secondary_abilities: Some(&session.secondary),
        },
    );
    let actor_context = |actor: &solomon_sim::enemies::model::EnemyActorRef<'_>| {
        let target = actor.target_player_id.as_ref().and_then(|id| session.participants.get(id));
        let pike_target_position = match &actor.brain {
            EnemyBrain::Skeleton(brain) => brain.pike.as_ref().map(|pike| {
                session
                    .participants
                    .get(&pike.player_id)
                    .map_or(pike.position, |player| player.wizard.position)
            }),
            _ => None,
        };
        EnemyActorAppearanceContext {
            tick: session.ticks,
            application_tick: session.ticks,
            complex_lighting: true,
            spider_exposure: light.scalar_at(actor.position),
            spider_target: target.map(|participant| EnemySpiderOutlineTarget {
                position: participant.wizard.position,
                primary_tint: solomon_sim::profile::primary::native_primary_spell_tint(
                    participant.player.book.primary_skill_id,
                    participant.player.book.weld_build_id,
                ),
            }),
            pike_target_position,
        }
    };
    Some(enemies::from_world(
        &world.enemies,
        session.ticks as f64,
        &world.enemy_events,
        &actor_context,
    ))
}

#[allow(clippy::too_many_arguments)] // The world bracket includes its two sampled enemy appearances.
pub(super) fn project(
    output: &mut GameSession,
    older: &GameSession,
    newer: &GameSession,
    older_metadata: &EnemyPresentationMetadata,
    newer_metadata: &EnemyPresentationMetadata,
    blend: f64,
    tick: f64,
    clocks: &mut PresentationClocks,
) -> EnemyPresentationMetadata {
    let world = output.boneyard.as_mut().expect("sampled Boneyard");
    let first = older.boneyard.as_ref().expect("older Boneyard");
    let next = newer.boneyard.as_ref().expect("newer Boneyard");
    let mut metadata = enemies::interpolate(
        &mut world.enemies,
        &first.enemies,
        &next.enemies,
        older_metadata,
        newer_metadata,
        blend,
        tick,
    );
    world.enemies.boss_spells =
        project_boss_spells(&first.enemies.boss_spells, &next.enemies.boss_spells, blend);
    world.enemies.puppet_hits = project_puppet_hits(
        &first.enemies.puppet_hits,
        &next.enemies.puppet_hits,
        blend,
        tick,
        clocks,
    );
    world.gate_leaves = interpolate_members(
        &first.gate_leaves,
        &next.gate_leaves,
        blend,
        |leaf| leaf.id.clone(),
        |first, next, blend| {
            let mut leaf = if blend < 1.0 { first.clone() } else { next.clone() };
            leaf.hinge = lerp_vec(first.hinge, next.hinge, blend);
            leaf.tip = lerp_vec(first.tip, next.tip, blend);
            leaf
        },
    );
    // Rain and other view-only queries must see the same projected gate geometry.
    world.dynamic_collision.set_gate_leaves(&world.gate_leaves);
    if let (Some(first), Some(next), Some(arena)) =
        (&first.arena_transition, &next.arena_transition, &mut world.arena_transition)
    {
        arena.blend_factor = lerp(first.blend_factor, next.blend_factor, blend);
        arena.camera_bounds = BoneyardBounds::new(
            lerp(first.camera_bounds.x, next.camera_bounds.x, blend),
            lerp(first.camera_bounds.y, next.camera_bounds.y, blend),
            lerp(first.camera_bounds.width, next.camera_bounds.width, blend),
            lerp(first.camera_bounds.height, next.camera_bounds.height, blend),
        );
    }
    if let (Some(first), Some(next), Some(encounter)) =
        (&first.encounter, &next.encounter, &mut world.encounter)
    {
        encounter.acceleration = lerp(first.acceleration, next.acceleration, blend);
        encounter.heading_degrees =
            lerp_cycle(first.heading_degrees, next.heading_degrees, blend, 360.0);
        encounter.motion = lerp(first.motion, next.motion, blend);
        encounter.position = lerp_vec(first.position, next.position, blend);
        encounter.transition_offset_y =
            lerp(first.transition_offset_y, next.transition_offset_y, blend);
        encounter.walk_cycle = lerp_cycle(first.walk_cycle, next.walk_cycle, blend, 6.0);
    }
    if let (Some(first), Some(next)) = (first.lantern_position, next.lantern_position) {
        world.lantern_position = Some(lerp_vec(first, next, blend));
    }
    if let (Some(first), Some(next), Some(tutorial)) =
        (&first.tutorial, &next.tutorial, &mut world.tutorial)
        && first.camera_lock_triggered
        && next.camera_lock_triggered
    {
        tutorial.camera_lock_age_ticks =
            lerp(first.camera_lock_age_ticks, next.camera_lock_age_ticks, blend);
    }
    world.loot.actors = interpolate_projected_members(
        first.loot.actors.iter(),
        next.loot.actors.iter(),
        blend,
        |actor| actor.id,
        |actor| actor.snapshot(),
        |first, next, blend| {
            if first.kind != next.kind {
                return first.snapshot();
            }
            let mut actor = if blend < 1.0 { first.snapshot() } else { next.snapshot() };
            actor.alpha = lerp(first.alpha, next.alpha, blend);
            actor.animation_phase =
                lerp_cycle(first.animation_phase, next.animation_phase, blend, 360.0);
            actor.bounce_height = lerp(first.bounce_height, next.bounce_height, blend);
            actor.frame_phase = lerp_cycle(first.frame_phase, next.frame_phase, blend, 18.0);
            actor.position = lerp_vec(first.position, next.position, blend);
            actor.rotation_deg = lerp_cycle(first.rotation_deg, next.rotation_deg, blend, 360.0);
            actor.scatter_progress = lerp(first.scatter_progress, next.scatter_progress, blend);
            actor
        },
    )
    .into_iter()
    .collect();
    let (effects, ages) =
        enemies::interpolate_death_effects(&first.loot.effects, &next.loot.effects, blend);
    world.loot.effects = effects.into_iter().collect();
    metadata.death_effects.extend(ages);
    if older.run.phase == GameRunPhase::GameOver
        && newer.run.phase == GameRunPhase::GameOver
        && older.run.run_id == newer.run.run_id
        && older.run.game_over_event_id == newer.run.game_over_event_id
    {
        output.run.game_over_ticks =
            lerp(older.run.game_over_ticks as f64, newer.run.game_over_ticks as f64, blend).floor()
                as u64;
        if let (Some(first), Some(next)) =
            (older.run.game_over_exit_ticks, newer.run.game_over_exit_ticks)
        {
            output.run.game_over_exit_ticks =
                Some(lerp(first as f64, next as f64, blend).floor() as u64);
        }
    }
    metadata
}

fn project_boss_spells(older: &BossSpells, newer: &BossSpells, blend: f64) -> BossSpells {
    if blend >= 1.0 {
        return newer.clone();
    }
    let by_id: BTreeMap<_, _> = newer.iter().map(|spell| (spell.owner.id, spell)).collect();
    older
        .iter()
        .map(|first| {
            let mut spell = first.snapshot();
            let Some(next) = by_id.get(&first.owner.id) else { return spell };
            if std::mem::discriminant(first.kind) != std::mem::discriminant(next.kind)
                || matches!(first.kind, BossSpellKind::MouthBeamSegment { .. })
            {
                return spell;
            }
            spell.owner.position = lerp_vec(first.owner.position, next.owner.position, blend);
            match (&mut spell.kind, &first.kind, &next.kind) {
                (
                    BossSpellKind::UnholySpit { progress, .. },
                    BossSpellKind::UnholySpit { progress: first, .. },
                    BossSpellKind::UnholySpit { progress: next, .. },
                ) => *progress = lerp(*first, *next, blend),
                (
                    BossSpellKind::EyeLaser { phase_deg, .. },
                    BossSpellKind::EyeLaser { phase_deg: first, .. },
                    BossSpellKind::EyeLaser { phase_deg: next, .. },
                ) => *phase_deg = lerp_cycle(*first, *next, blend, 360.0),
                (
                    BossSpellKind::SkullMissile { phase_deg, heading_deg, .. },
                    BossSpellKind::SkullMissile {
                        phase_deg: first_phase,
                        heading_deg: first_heading,
                        ..
                    },
                    BossSpellKind::SkullMissile {
                        phase_deg: next_phase,
                        heading_deg: next_heading,
                        ..
                    },
                ) => {
                    *phase_deg = lerp_cycle(*first_phase, *next_phase, blend, 360.0);
                    *heading_deg = lerp_cycle(*first_heading, *next_heading, blend, 360.0);
                }
                (
                    BossSpellKind::DarkFireball { arc_phase, .. },
                    BossSpellKind::DarkFireball { arc_phase: first, .. },
                    BossSpellKind::DarkFireball { arc_phase: next, .. },
                ) => *arc_phase = lerp(*first, *next, blend),
                (BossSpellKind::GreenFire { fire, .. }, _, _) => {
                    fire.position = spell.owner.position
                }
                (
                    BossSpellKind::UnholyBurst { offset_y, .. },
                    BossSpellKind::UnholyBurst { offset_y: first, .. },
                    BossSpellKind::UnholyBurst { offset_y: next, .. },
                ) => *offset_y = lerp(*first, *next, blend),
                (
                    BossSpellKind::UnholySoul { height, .. },
                    BossSpellKind::UnholySoul { height: first, .. },
                    BossSpellKind::UnholySoul { height: next, .. },
                ) => *height = lerp(*first, *next, blend),
                _ => {}
            }
            spell
        })
        .collect()
}

fn project_puppet_hits(
    older: &solomon_sim::ComponentEntities<EnemyPuppetHit>,
    newer: &solomon_sim::ComponentEntities<EnemyPuppetHit>,
    blend: f64,
    tick: f64,
    clocks: &mut PresentationClocks,
) -> solomon_sim::ComponentEntities<EnemyPuppetHit> {
    clocks.puppet_hits.clear();
    clocks.puppet_hit_timers.clear();
    let mut previous: BTreeMap<_, _> =
        older.iter().map(|hit| (hit.target_id.as_str(), hit)).collect();
    let mut output = solomon_sim::ComponentEntities::default();
    for next in newer {
        let before = previous.remove(next.target_id.as_str());
        if next.hit_tick as f64 > tick {
            if let Some(before) = before {
                clocks.puppet_hits.insert(before.target_id.clone(), before.feedback.alpha(tick));
                clocks
                    .puppet_hit_timers
                    .insert(before.target_id.clone(), before.feedback.timer_at(tick));
                output.push(before.clone());
            }
            continue;
        }
        let mut hit = next.clone();
        if let Some(before) = before.filter(|before| before.hit_tick == next.hit_tick) {
            hit.feedback.timer = lerp(before.feedback.timer, next.feedback.timer, blend);
            hit.feedback.tick = tick.floor() as u64;
            // Source samples this timer at the fractional display tick itself.
            clocks.puppet_hits.insert(
                hit.target_id.clone(),
                fround(hit.feedback.strength * hit.feedback.timer).min(1.0),
            );
            clocks.puppet_hit_timers.insert(hit.target_id.clone(), hit.feedback.timer);
        } else {
            let mut feedback = PuppetHitState::default();
            feedback.receive(next.hit_tick, next.feedback.strength);
            if !matches!(next.kind, EnemyPuppetHitKind::Meteor | EnemyPuppetHitKind::Leviathan) {
                feedback.step(next.hit_tick, 1);
            }
            hit.feedback = feedback;
            clocks.puppet_hits.insert(hit.target_id.clone(), feedback.alpha(tick));
            clocks.puppet_hit_timers.insert(hit.target_id.clone(), feedback.timer_at(tick));
        }
        output.push(hit);
    }
    if blend < 1.0 {
        // The source Map preserves older insertion order, not sorted target IDs.
        for before in older {
            if previous.contains_key(before.target_id.as_str()) {
                clocks.puppet_hits.insert(before.target_id.clone(), before.feedback.alpha(tick));
                clocks
                    .puppet_hit_timers
                    .insert(before.target_id.clone(), before.feedback.timer_at(tick));
                output.push(before.clone());
            }
        }
    }
    output
}
