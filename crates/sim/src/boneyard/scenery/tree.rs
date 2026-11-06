//! Which actors make Tree canopies fade, the canopy scan clock, and the position of a
//! Tree's secondary foliage. These are view inputs; physical collision, health and
//! visibility do not replace native membership.

use std::collections::BTreeMap;

use crate::Vec2;
use crate::boneyard::presentation::TreeOcclusion;
use crate::boneyard::{BoneyardBounds, encounter::SolomonPhase};
use crate::enemies::EnemyToken;
use crate::enemies::actor_presentation::{EnemyAnimationState, EnemyCoffinState};
use crate::float::fround as f;

/// Actors that may fade a Tree canopy: players, enemies, Maggots and Solomon's position and
/// phase during the encounter.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TreeActorPoses {
    pub players: Vec<TreePlayerPose>,
    pub enemies: Vec<TreeEnemyPose>,
    pub maggots: Vec<TreeMaggotPose>,
    pub encounter: Option<(Vec2, SolomonPhase)>,
}

/// A player's ID and position for Tree canopy scans.
#[derive(Clone, Debug, PartialEq)]
pub struct TreePlayerPose {
    pub id: String,
    pub position: Vec2,
}

/// An enemy's position and the type, animation and coffin state that decide whether it
/// counts in Tree canopy scans.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TreeEnemyPose {
    pub id: usize,
    pub position: Vec2,
    pub enemy_token: EnemyToken,
    pub type_id: u16,
    pub animation_state: EnemyAnimationState,
    pub coffin_state: EnemyCoffinState,
}

/// A Maggot's position and whether it currently counts in Tree canopy scans.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TreeMaggotPose {
    pub id: usize,
    pub position: Vec2,
    pub tree_query_member: bool,
}

/// One tick's Tree scan inputs: the positions of counted actors and the camera bounds.
#[derive(Clone, Debug, PartialEq)]
pub struct TreeOcclusionFrame {
    pub actor_positions: Vec<Vec2>,
    pub camera_bounds: BoneyardBounds,
}

/// One view-owned clock for the loaded Trees' canopy fades. A scan on a tick that has no
/// delivered frame keeps the previous target rather than guessing an occupant.
pub struct TreeOcclusionPresentation {
    trees: BTreeMap<String, TreeOcclusion>,
    last_tick: u64,
}

impl TreeOcclusionPresentation {
    #[must_use]
    pub fn new(trees: BTreeMap<String, TreeOcclusion>, start_tick: u64) -> Self {
        Self { trees, last_tick: start_tick }
    }

    #[must_use]
    pub fn get(&self, id: &str) -> Option<&TreeOcclusion> {
        self.trees.get(id)
    }

    pub fn update(
        &mut self,
        display_tick: f64,
        current: &TreeOcclusionFrame,
        retained_from: Option<u64>,
        mut sample: impl FnMut(u64) -> Option<TreeOcclusionFrame>,
    ) {
        if !display_tick.is_finite() || display_tick < 0.0 {
            return;
        }
        let target_tick = display_tick.floor() as u64;
        if target_tick <= self.last_tick {
            return;
        }
        let from_tick =
            (self.last_tick + 1).max(retained_from.unwrap_or(target_tick).min(target_tick));
        let unknown_ticks = from_tick - self.last_tick - 1;
        let mut frames = BTreeMap::new();
        for tree in self.trees.values_mut() {
            tree.advance_unobserved(unknown_ticks);
            for at in from_tick..=target_tick {
                let mut inside = None;
                if tree.countdown <= 1 {
                    let frame = frames.entry(at).or_insert_with(|| {
                        if at == target_tick { Some(current.clone()) } else { sample(at) }
                    });
                    if let Some(frame) = frame
                        && tree.overlaps_camera(frame.camera_bounds)
                    {
                        inside =
                            Some(frame.actor_positions.iter().any(|point| tree.contains(*point)));
                    }
                }
                tree.step(inside);
            }
        }
        self.last_tick = target_tick;
    }
}

#[must_use]
pub fn tree_actor_positions(poses: &TreeActorPoses, local_player_id: &str) -> Vec<Vec2> {
    let mut positions = Vec::new();
    // A dying local wizard retains its category/bank/cell until actual removal.
    if let Some(player) = poses.players.iter().find(|player| player.id == local_player_id) {
        positions.push(player.position);
    }
    for enemy in &poses.enemies {
        if enemy.enemy_token.type_id() != enemy.type_id
            || enemy.animation_state == EnemyAnimationState::Death
        {
            continue;
        }
        if enemy.enemy_token == EnemyToken::Coffin && enemy.coffin_state == EnemyCoffinState::Hidden
        {
            continue;
        }
        positions.push(enemy.position);
    }
    for maggot in poses.maggots.iter() {
        if maggot.tree_query_member {
            positions.push(maggot.position);
        }
    }
    if let Some((position, phase)) = poses.encounter
        && phase != SolomonPhase::Gone
    {
        positions.push(position);
    }
    positions
}

#[must_use]
pub fn tree_secondary_position(root: Vec2, camera: BoneyardBounds) -> Vec2 {
    let x = f(f(camera.x) + f(f(camera.width) * 0.5));
    let y = f(f(camera.y) + f(f(camera.height) * 0.5));
    let scale = 1.025_f32 as f64;
    Vec2::new(f(x + f(f(f(root.x) - x) * scale)), f(y + f(f(f(root.y) - y) * scale)))
}
