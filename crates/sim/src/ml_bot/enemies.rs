//! Source `ml-bot-policy/enemies.ts`: ordered visible enemies and observation history.

use std::collections::{BTreeMap, BTreeSet};

use crate::Vec2;
use crate::enemies::EnemyToken;
use crate::enemies::model::EnemyLifeState;
use crate::enemies::targeting::boneyard_enemy_collision_radius;
use crate::enemies::world::EnemyWorld;
use crate::float::hypot;

use super::numbers::{distance_squared, heading_vector, ratio, signed, unsigned, write};
use super::spec::scales;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MlBotPolicyEnemySpecies {
    Spider,
    Cocoon,
    DemonSkull,
    Faculty,
    Heartmonger,
    Archer,
    Coffin,
    Demon,
    Imp,
    Mage,
    Maggot,
    Skeleton,
    Wraith,
    Zombie,
}

impl MlBotPolicyEnemySpecies {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Spider => "spider",
            Self::Cocoon => "cocoon",
            Self::DemonSkull => "demon-skull",
            Self::Faculty => "faculty",
            Self::Heartmonger => "heartmonger",
            Self::Archer => "archer",
            Self::Coffin => "coffin",
            Self::Demon => "demon",
            Self::Imp => "imp",
            Self::Mage => "mage",
            Self::Maggot => "maggot",
            Self::Skeleton => "skeleton",
            Self::Wraith => "wraith",
            Self::Zombie => "zombie",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) enum EnemySource {
    Actor(usize),
    Maggot(usize),
}

#[derive(Clone, Debug)]
pub struct MlBotPolicyEnemyRow {
    pub current_health: f64,
    pub heading_deg: f64,
    pub id: usize,
    pub maximum_health: f64,
    pub position: Vec2,
    pub radius: f64,
    pub species: MlBotPolicyEnemySpecies,
    pub target_player_id: Option<String>,
    pub velocity: Vec2,
    // These indices are used only during the same read-only observation to borrow
    // family clocks. A captured inference frame never clones canonical brains.
    pub(super) source: EnemySource,
}

#[derive(Clone, Debug, Default)]
pub struct MlBotPolicyEnemyMemory {
    previous_positions: BTreeMap<usize, (Vec2, u64)>,
    pub target_id: Option<usize>,
}

pub struct MlBotPolicyEnemyObservationOptions<'a> {
    pub memory: &'a MlBotPolicyEnemyMemory,
    pub own_minion_target_ids: &'a BTreeSet<usize>,
    pub primary_range: f64,
    pub self_position: Vec2,
    pub tick: u64,
}

#[derive(Clone, Debug)]
pub struct MlBotPolicyEnemyObservation {
    pub block_d: Vec<f32>,
    pub block_e: Vec<f32>,
    pub block_l: Vec<f32>,
    pub next_memory: MlBotPolicyEnemyMemory,
    pub rows: Vec<MlBotPolicyEnemyRow>,
}

pub fn observe_ml_bot_policy_enemies(
    enemies: &EnemyWorld,
    options: MlBotPolicyEnemyObservationOptions<'_>,
) -> MlBotPolicyEnemyObservation {
    let mut rows = Vec::new();
    for (index, actor) in enemies.actors.iter().enumerate() {
        if actor.life_state != EnemyLifeState::Alive {
            continue;
        }
        rows.push(MlBotPolicyEnemyRow {
            current_health: actor.current_health,
            heading_deg: actor.heading_deg,
            id: actor.id,
            maximum_health: actor.config.maximum_health,
            position: actor.position,
            radius: boneyard_enemy_collision_radius(&actor),
            source: EnemySource::Actor(index),
            species: species(actor.config.enemy_token),
            target_player_id: actor.target_player_id.clone(),
            velocity: Vec2::ZERO,
        });
    }
    for (index, actor) in enemies.maggots.iter().enumerate() {
        if actor.life_state != EnemyLifeState::Alive || !actor.combat_active {
            continue;
        }
        rows.push(MlBotPolicyEnemyRow {
            current_health: actor.current_health,
            heading_deg: actor.heading_deg,
            id: actor.id,
            maximum_health: actor.maximum_health,
            position: actor.position,
            radius: actor.collision_radius,
            source: EnemySource::Maggot(index),
            species: MlBotPolicyEnemySpecies::Maggot,
            target_player_id: actor.controller.target_player_id.clone(),
            velocity: Vec2::ZERO,
        });
    }
    let mut previous_positions = BTreeMap::new();
    for row in &mut rows {
        if let Some((previous, tick)) = options.memory.previous_positions.get(&row.id) {
            let elapsed_ticks = options.tick.saturating_sub(*tick).max(1) as f64;
            row.velocity = Vec2::new(
                (row.position.x - previous.x) * scales::TICK_RATE / elapsed_ticks,
                (row.position.y - previous.y) * scales::TICK_RATE / elapsed_ticks,
            );
        }
        previous_positions.insert(row.id, (row.position, options.tick));
    }
    rows.sort_by(|left, right| {
        distance_squared(left.position, options.self_position)
            .total_cmp(&distance_squared(right.position, options.self_position))
            .then(left.id.cmp(&right.id))
    });
    let target_id = options.memory.target_id.filter(|id| rows.iter().any(|row| row.id == *id));
    let mut block_d = vec![0.0; 8 * 11];
    for (slot, row) in rows.iter().take(8).enumerate() {
        let dx = row.position.x - options.self_position.x;
        let dy = row.position.y - options.self_position.y;
        let distance = hypot(dx, dy);
        write(
            &mut block_d,
            slot * 11,
            &[
                1.0,
                signed(dx, scales::RANGE),
                signed(dy, scales::RANGE),
                unsigned(distance, scales::RANGE),
                ratio(row.current_health, row.maximum_health),
                unsigned(row.radius, scales::RADIUS),
                signed(row.velocity.x, scales::VELOCITY),
                signed(row.velocity.y, scales::VELOCITY),
                f64::from((distance - row.radius).max(0.0) <= options.primary_range),
                f64::from(Some(row.id) == target_id),
                f64::from(options.own_minion_target_ids.contains(&row.id)),
            ],
        );
    }
    let mut block_e = vec![0.0; 9];
    let mut block_l = vec![0.0; 4];
    if let Some(target) = target_id.and_then(|id| rows.iter().find(|row| row.id == id)) {
        let dx = target.position.x - options.self_position.x;
        let dy = target.position.y - options.self_position.y;
        let distance = hypot(dx, dy);
        let edge_distance = (distance - target.radius).max(0.0);
        write(
            &mut block_e,
            0,
            &[
                1.0,
                signed(dx, scales::RANGE),
                signed(dy, scales::RANGE),
                unsigned(distance, scales::RANGE),
                unsigned(edge_distance, scales::RANGE),
                ratio(target.current_health, target.maximum_health),
                unsigned(target.radius, scales::RADIUS),
                f64::from(edge_distance <= options.primary_range),
                unsigned(options.primary_range, scales::RANGE),
            ],
        );
        let facing = heading_vector(target.heading_deg);
        write(
            &mut block_l,
            0,
            &[
                signed(target.velocity.x, scales::VELOCITY),
                signed(target.velocity.y, scales::VELOCITY),
                facing.x,
                facing.y,
            ],
        );
    }
    MlBotPolicyEnemyObservation {
        block_d,
        block_e,
        block_l,
        next_memory: MlBotPolicyEnemyMemory { previous_positions, target_id },
        rows,
    }
}

fn species(token: EnemyToken) -> MlBotPolicyEnemySpecies {
    use MlBotPolicyEnemySpecies as Species;
    match token {
        EnemyToken::Coffin | EnemyToken::Portal => Species::Coffin,
        EnemyToken::Cocoon => Species::Cocoon,
        EnemyToken::Demon => Species::Demon,
        EnemyToken::DemonSkull => Species::DemonSkull,
        EnemyToken::DireFaculty => Species::Faculty,
        EnemyToken::Heartmonger => Species::Heartmonger,
        EnemyToken::Imp => Species::Imp,
        EnemyToken::Skeleton => Species::Skeleton,
        EnemyToken::SkeletonArcher => Species::Archer,
        EnemyToken::SkeletonMage => Species::Mage,
        EnemyToken::Spider => Species::Spider,
        EnemyToken::Wraith => Species::Wraith,
        EnemyToken::Zombie => Species::Zombie,
    }
}
