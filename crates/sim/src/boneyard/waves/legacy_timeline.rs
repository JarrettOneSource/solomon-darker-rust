//! Compiler used by native checkpoints written before retained wave tables existed.
//!
//! The old standalone package reconstructed its tables with the ed0a2d59 Website
//! compiler. Only missing-table checkpoint migration uses this implementation;
//! all new directors use the current compiler in `timeline`.

use super::timeline::WAVE_LABEL_TO_FIRST_SPAWN_TICKS;
use super::{CompiledSpawnBurst, CompiledWaveEntry, CompiledWaveSection, WaveDef};
use crate::NativeRng;
use crate::enemies::{EnemyFlag, EnemyToken, SpawnLocationPolicy, SpawnPositionPolicy};

/// `GenerateTimeline` consumes a group-cost budget and merges consecutive samples of the
/// same GROUP. Even discarded native draws remain in the seeded stream.
#[must_use]
pub(super) fn compile_legacy_wave_section(
    wave: &WaveDef,
    wave_ordinal: usize,
    rng: &mut NativeRng,
) -> CompiledWaveSection {
    assert!(wave_ordinal > 0, "wave ordinal must be positive");
    let half_budget = wave.spawn / 2;
    let mut remaining_budget = wave.spawn + half_budget + rng.draw_integer(half_budget.max(1));

    // Parsed WAVEDELAY and the singleton selection are retained/dead graph values, but
    // skipping either draw changes every later group and spawn position.
    let _retained_wave_delay = draw_native_range(rng, wave.wave_delay);
    let _retained_singleton = rng.draw_integer(1);

    let mut bursts: Vec<CompiledSpawnBurst> = Vec::new();
    while remaining_budget > 0 {
        let group_index = rng.draw_integer(wave.groups.len() as i32) as usize;
        let group = &wave.groups[group_index];
        let first = &group.entries[0];
        if wave_ordinal < 28 && first.enemy == EnemyToken::Zombie {
            remaining_budget -= 2;
        }
        let coffin_mode = first.enemy == EnemyToken::Coffin;
        if coffin_mode {
            remaining_budget = wave.spawn;
        }
        if first.enemy == EnemyToken::Demon {
            remaining_budget -= 2;
        }
        if first.enemy == EnemyToken::Skeleton
            && group.entries.iter().any(|entry| entry.flags.contains(&EnemyFlag::Pike))
        {
            remaining_budget = wave.spawn;
        }
        if wave_ordinal < 37 && first.enemy == EnemyToken::Imp {
            let splits = group.entries.iter().any(|entry| {
                entry.flags.contains(&EnemyFlag::Split)
                    || entry.flags.contains(&EnemyFlag::SplitMany)
            });
            if splits {
                remaining_budget = wave.spawn;
            } else {
                remaining_budget -= 2;
            }
        }
        let group_cost = remaining_budget.min(group.entries.len() as i32);
        if group_cost <= 0 {
            break;
        }
        let bonus_bound = (wave_ordinal / 3).clamp(1, 4) as i32;
        let mut count = group_cost as usize + rng.draw_integer(bonus_bound) as usize + 1;
        if wave_ordinal >= 4 {
            count += count / 3;
        }
        if wave_ordinal >= 9 {
            count += count / 3;
        }
        if coffin_mode {
            count = count.min(wave_ordinal / 5);
        }

        let mut spread_ticks = 0.0;
        for _ in 0..group_cost {
            let delay = draw_native_range(rng, wave.spawn_delay);
            spread_ticks += if coffin_mode { 25.0 } else { f64::from(delay) * 0.5 };
        }
        let location_policy = if coffin_mode {
            SpawnLocationPolicy::NearPlayer
        } else {
            SpawnLocationPolicy::Anywhere
        };
        let position_policy =
            if coffin_mode { SpawnPositionPolicy::Light } else { SpawnPositionPolicy::Dark };
        if let Some(prior) = bursts.last_mut().filter(|prior| {
            prior.group_index == group_index as i32
                && prior.location_policy == location_policy
                && prior.position_policy == position_policy
        }) {
            prior.count += count;
            prior.spread_ticks += spread_ticks;
        } else {
            let mut entries = Vec::new();
            for entry in group.entries {
                let flags = entry
                    .flags
                    .iter()
                    .copied()
                    .filter(|flag| !matches!(flag, EnemyFlag::Ignite | EnemyFlag::Immortalize))
                    .collect();
                entries.push(CompiledWaveEntry { enemy: entry.enemy, flags });
            }
            bursts.push(CompiledSpawnBurst {
                after_delay_ticks: if coffin_mode { 100 } else { 0 },
                count,
                entries,
                group_index: group_index as i32,
                location_policy,
                position_policy,
                spread_ticks,
                start_delay_ticks: if bursts.is_empty() {
                    WAVE_LABEL_TO_FIRST_SPAWN_TICKS
                } else {
                    0
                },
                steady: true,
            });
        }
        remaining_budget -= group_cost;
    }
    let release_upper = 10.max(wave_ordinal / 2);
    let release_threshold = 10 + rng.draw_integer((release_upper - 10 + 1) as i32) as usize;
    let lull_upper = 4.max(wave_ordinal / 2);
    let lull_threshold = 2 + rng.draw_integer((lull_upper - 2 + 1) as i32) as usize + 1;
    CompiledWaveSection { bursts, lull_threshold, release_threshold }
}

fn draw_native_range(rng: &mut NativeRng, range: [i32; 2]) -> i32 {
    range[0] + rng.draw_integer(range[1] - range[0] + 1)
}
