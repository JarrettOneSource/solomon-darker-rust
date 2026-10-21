//! Authored survival waves from `boneyard-wave-schema.ts` and `boneyard-wave-timeline.ts`.

pub mod director;
mod legacy_timeline;
mod native_programs;
pub mod programs;
mod retail_schedule;
pub mod timeline;

use crate::enemies::{EnemyFlag, EnemyToken, SpawnLocationPolicy, SpawnPositionPolicy};

pub use native_programs::native_survival_programs;
pub use retail_schedule::RETAIL_WAVES;

/// An authored GROUP member; flag order and duplicates are part of its native meaning.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize))]
pub struct WaveGroupEntry {
    pub enemy: EnemyToken,
    pub flags: &'static [EnemyFlag],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize))]
pub struct WaveGroup {
    pub entries: &'static [WaveGroupEntry],
}

/// One parsed retail wave.txt row. The retained schedule is immutable data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize))]
pub struct WaveDef {
    pub spawn: i32,
    pub spawn_delay: [i32; 2],
    pub wave_delay: [i32; 2],
    pub max_enemies: usize,
    pub zombie_wave: bool,
    pub next: &'static [i32],
    pub groups: &'static [WaveGroup],
}

/// The compiler removes the two source flags whose native effects were discarded.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct CompiledWaveEntry {
    pub enemy: EnemyToken,
    pub flags: Vec<EnemyFlag>,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct CompiledSpawnBurst {
    pub after_delay_ticks: u64,
    pub count: usize,
    pub entries: Vec<CompiledWaveEntry>,
    pub group_index: i32,
    pub location_policy: SpawnLocationPolicy,
    pub position_policy: SpawnPositionPolicy,
    pub spread_ticks: f64,
    pub start_delay_ticks: u64,
    pub steady: bool,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct CompiledWaveSection {
    pub bursts: Vec<CompiledSpawnBurst>,
    pub lull_threshold: usize,
    pub release_threshold: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledOpening {
    pub bursts: Vec<CompiledSpawnBurst>,
    pub release_threshold: usize,
}
