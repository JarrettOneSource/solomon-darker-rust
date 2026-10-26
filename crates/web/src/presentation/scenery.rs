//! Pure delivered actor roots from `boneyard-presentation-timeline.ts`.
//! This path borrows endpoint metadata; it never samples spell/enemy display owners.

use solomon_sim::Vec2;
use solomon_sim::boneyard::scenery::{
    NativeTreeActorPoses, NativeTreeEnemyPose, NativeTreeMaggotPose, NativeTreePlayerPose,
};
use solomon_sim::boneyard::world::BoneyardWorld;
use solomon_sim::enemies::actor_presentation::{
    EnemyActorAppearanceContext, native_enemy_presentation_input,
};
use solomon_sim::enemies::model::{EnemyBrain, SpiderPhase};
use solomon_sim::session::GameSession;

use super::math::{interpolate_members, lerp_vec};
use super::{EnemyPresentationMetadata, TimedSession};

#[must_use]
pub fn scenery_actor_poses(
    world: &BoneyardWorld,
    players: impl IntoIterator<Item = NativeTreePlayerPose>,
    metadata: Option<&EnemyPresentationMetadata>,
) -> NativeTreeActorPoses {
    let mut poses =
        NativeTreeActorPoses { players: players.into_iter().collect(), ..Default::default() };
    for actor in world.enemies.actors.iter() {
        if matches!(&actor.brain, EnemyBrain::Spider(brain) if brain.phase == SpiderPhase::Captured)
        {
            continue;
        }
        let fallback;
        let sample =
            if let Some(sample) = metadata.and_then(|metadata| metadata.actors.get(&actor.id)) {
                sample
            } else {
                fallback = native_enemy_presentation_input(
                    &actor,
                    EnemyActorAppearanceContext {
                        tick: world.ticks,
                        application_tick: world.ticks,
                        complex_lighting: false,
                        spider_exposure: 1.0,
                        spider_target: None,
                        pike_target_position: None,
                    },
                );
                &fallback
            };
        poses.enemies.push(NativeTreeEnemyPose {
            id: actor.id,
            position: actor.position,
            enemy_token: actor.config.enemy_token,
            native_type_id: actor.config.native_type_id,
            animation_state: sample.state,
            coffin_state: sample.coffin_state,
        });
    }
    for maggot in &world.enemies.maggots {
        poses.maggots.push(NativeTreeMaggotPose {
            id: maggot.id,
            position: maggot.position,
            native_tree_query_member: maggot.native_tree_query_member,
        });
    }
    poses.encounter =
        world.encounter.as_ref().map(|encounter| (encounter.position, encounter.phase));
    poses
}

fn from_endpoint(endpoint: &TimedSession) -> NativeTreeActorPoses {
    let session = &endpoint.session;
    scenery_actor_poses(
        session.boneyard.as_ref().expect("retained Boneyard endpoint"),
        session.participant_order.iter().map(|id| NativeTreePlayerPose {
            id: id.clone(),
            position: session.participants[id].wizard.position,
        }),
        endpoint.enemies.as_ref(),
    )
}

pub(super) fn player_position(
    older: &GameSession,
    newer: &GameSession,
    blend: f64,
    id: &str,
) -> Option<Vec2> {
    if let Some(first) = older.participants.get(id) {
        // Source deliberately holds a departed player's root even at this
        // bracket's endpoint. Enemy list membership has a different rule below.
        return Some(newer.participants.get(id).map_or(first.wizard.position, |next| {
            lerp_vec(first.wizard.position, next.wizard.position, blend)
        }));
    }
    if blend >= 1.0 {
        newer.participants.get(id).map(|player| player.wizard.position)
    } else {
        None
    }
}

pub(super) fn interpolate(
    older: &TimedSession,
    newer: &TimedSession,
    blend: f64,
) -> NativeTreeActorPoses {
    let first = from_endpoint(older);
    let next = from_endpoint(newer);
    interpolate_poses(&first, &next, blend)
}

fn interpolate_poses(
    first: &NativeTreeActorPoses,
    next: &NativeTreeActorPoses,
    blend: f64,
) -> NativeTreeActorPoses {
    let mut ids: Vec<_> = first.players.iter().map(|player| player.id.clone()).collect();
    if blend >= 1.0 {
        for player in &next.players {
            if !ids.contains(&player.id) {
                ids.push(player.id.clone());
            }
        }
    }
    let players = ids
        .into_iter()
        .map(|id| NativeTreePlayerPose {
            position: match (
                first.players.iter().find(|player| player.id == id),
                next.players.iter().find(|player| player.id == id),
            ) {
                (Some(first), Some(next)) => lerp_vec(first.position, next.position, blend),
                (Some(first), None) => first.position,
                (None, Some(next)) => next.position,
                (None, None) => unreachable!("retained player ID has an endpoint"),
            },
            id,
        })
        .collect();
    let enemies = interpolate_members(
        &first.enemies,
        &next.enemies,
        blend,
        |actor| actor.id,
        |first, next, blend| {
            let mut actor = if blend < 1.0 { *first } else { *next };
            actor.position = lerp_vec(first.position, next.position, blend);
            actor
        },
    );
    let maggots = interpolate_members(
        &first.maggots,
        &next.maggots,
        blend,
        |actor| actor.id,
        |first, next, blend| {
            let mut actor = if blend < 1.0 { *first } else { *next };
            actor.position = lerp_vec(first.position, next.position, blend);
            actor
        },
    );
    let mut encounter = if blend < 1.0 { first.encounter } else { next.encounter };
    if let (Some((first, _)), Some((next, _)), Some((position, _))) =
        (first.encounter, next.encounter, &mut encounter)
    {
        *position = lerp_vec(first, next, blend);
    }
    NativeTreeActorPoses { players, enemies, maggots, encounter }
}

#[cfg(test)]
mod tests;
