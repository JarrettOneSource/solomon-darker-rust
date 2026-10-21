//! Native tick and kernel constants recovered from the retail executable.
//!
//! Sources: the Website kernels that carry the same values (`core-kernels/player-character.ts`
//! for the tick and the player movement scalars, `hub-collision.ts` for the eight-iteration
//! sweep, `native-enemy-pathfinding.ts` for the enemy path-factor decay and floor) and the
//! retail recordings in `tests/fixtures/native/`: `movement-goldens.json` records every player
//! scalar in its `native_baseline` block (radius at `actor+0x30`, movement scalar at
//! `actor+0x218`) and replays wall contact and knockback against these tables. The enemy
//! tables and the sub-step margin wait for the Boneyard port; no fixture replays them yet.

/// The native fixed simulation rate.
pub const TICK_RATE_HZ: u32 = 100;
/// One fixed tick, in milliseconds.
pub const TICK_INTERVAL_MS: u32 = 10;
/// One fixed tick, in seconds.
pub const TICK_SECONDS: f64 = 0.01;

/// Player movement scalars. Velocities are world units per tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerMovementConstants {
    pub input_divisor: f64,
    pub transient_multiplier: f64,
    pub move_speed_scale: f64,
    pub progression_multiplier: f64,
    pub global_cap_scale: f64,
    pub resulting_cap: f64,
    pub move_step_scale: f64,
    pub move_threshold_squared: f64,
    pub ordinary_damping: f64,
    pub controlled_damping: f64,
}

pub const PLAYER_MOVEMENT: PlayerMovementConstants = PlayerMovementConstants {
    input_divisor: 10.0,
    transient_multiplier: 1.0,
    move_speed_scale: 1.0,
    progression_multiplier: 0.95,
    global_cap_scale: 1.25,
    resulting_cap: 1.1875,
    move_step_scale: 1.0,
    move_threshold_squared: 0.01,
    ordinary_damping: 0.9f32 as f64,
    controlled_damping: 0.95f32 as f64,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CollisionResponseConstants {
    pub common_actor_radius: f64,
    pub movement_fixture_player_radius: f64,
    pub knockback_radius_scale: f64,
    pub enemy_substep_radius_margin: f64,
    pub half_step: f64,
    pub placement_epsilon: f64,
    pub iteration_limit: usize,
}

pub const COLLISION_RESPONSE: CollisionResponseConstants = CollisionResponseConstants {
    common_actor_radius: 15.0,
    movement_fixture_player_radius: 25.0,
    knockback_radius_scale: 0.6,
    enemy_substep_radius_margin: 1.0,
    half_step: 0.5,
    placement_epsilon: 0.01f32 as f64,
    iteration_limit: 8,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EnemyMovementConstants {
    pub direct_step_scale: f64,
    pub local_factor_floor: f64,
    pub local_factor_decay: f64,
}

pub const ENEMY_MOVEMENT: EnemyMovementConstants = EnemyMovementConstants {
    direct_step_scale: 0.25,
    local_factor_floor: 1.0,
    local_factor_decay: 0.995f32 as f64,
};

/// Native object type identifiers observed in the retail actor factory.
pub mod object_type {
    pub const PLAYER: u16 = 0x0001;
    pub const WAVE_ENEMY: u16 = 0x03e9;
    pub const FIRE_PROJECTILE: u16 = 0x07d4;
    pub const KNOCKBACK: u16 = 0x07e9;
}
