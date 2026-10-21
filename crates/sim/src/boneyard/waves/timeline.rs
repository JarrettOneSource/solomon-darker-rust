//! Seeded retail TimeLine compilation from `core-kernels/boneyard-wave-timeline.ts`.

use crate::NativeRng;
use crate::enemies::{EnemyFlag, EnemyToken, SpawnLocationPolicy, SpawnPositionPolicy};

use super::{
    CompiledOpening, CompiledSpawnBurst, CompiledWaveEntry, CompiledWaveSection, WaveDef,
    WaveGroupEntry,
};

pub const WAVE_LABEL_TO_FIRST_SPAWN_TICKS: u64 = 10;
pub const PAUSE_NODE_GAP_TICKS: u64 = 25;
pub const LULL_RELEASE_TO_NEXT_SPAWN_TICKS: u64 = 85;

#[must_use]
pub fn compile_boneyard_opening(rng: &mut NativeRng) -> CompiledOpening {
    let immediate = 8 + rng.draw_integer(5) as usize;
    let spread = 3 + rng.draw_integer(3) as usize;
    let release_threshold = 1 + rng.draw_integer(4) as usize;
    let entries = vec![CompiledWaveEntry {
        enemy: EnemyToken::Skeleton,
        flags: vec![EnemyFlag::Weak, EnemyFlag::HpDown, EnemyFlag::XpBonus],
    }];
    CompiledOpening {
        bursts: vec![
            CompiledSpawnBurst {
                after_delay_ticks: 0,
                count: immediate,
                entries: entries.clone(),
                group_index: -1,
                location_policy: SpawnLocationPolicy::NearPlayer,
                position_policy: SpawnPositionPolicy::Dark,
                spread_ticks: 0.0,
                start_delay_ticks: 0,
                steady: true,
            },
            CompiledSpawnBurst {
                after_delay_ticks: 0,
                count: spread,
                entries,
                group_index: -1,
                location_policy: SpawnLocationPolicy::NearPlayer,
                position_policy: SpawnPositionPolicy::Dark,
                spread_ticks: 400.0,
                start_delay_ticks: 500,
                steady: true,
            },
        ],
        release_threshold,
    }
}

/// `GenerateTimeline` consumes a group-cost budget and merges consecutive samples of the
/// same GROUP. Even discarded native draws remain in the seeded stream.
#[must_use]
pub fn compile_boneyard_wave_section(
    wave: &WaveDef,
    wave_ordinal: usize,
    rng: &mut NativeRng,
) -> CompiledWaveSection {
    assert!(wave_ordinal > 0, "wave ordinal must be positive");
    // Scalar SPAWN is parsed before the half-budget roll. A zero bound consumes
    // no word, and the two later text samples are separate retained draws.
    let _initial_spawn = rng.draw_integer(1);
    let half_budget = wave.spawn / 2;
    let mut remaining_budget = wave.spawn + half_budget + rng.draw_integer(half_budget);

    // Parsed WAVEDELAY and the singleton selection are retained/dead graph values, but
    // skipping either draw changes every later group and spawn position.
    let _retained_wave_delay = draw_native_range(rng, wave.wave_delay);
    let _retained_singleton = rng.draw_integer(1);

    let mut bursts: Vec<CompiledSpawnBurst> = Vec::new();
    let mut selectable_groups: Vec<usize> = (0..wave.groups.len()).collect();
    while remaining_budget > 0 {
        assert!(
            !selectable_groups.is_empty(),
            "wave {wave_ordinal} exhausted its selectable groups before consuming the spawn budget"
        );
        let resets_only = selectable_groups.iter().all(|index| {
            recurring_group_resets_budget(&wave.groups[*index].entries[0], wave_ordinal)
        });
        assert!(
            remaining_budget >= wave.spawn || !resets_only,
            "wave {wave_ordinal} cannot consume its spawn budget because every remaining group resets it"
        );
        let selected_index = rng.draw_integer(selectable_groups.len() as i32) as usize;
        let group_index = selectable_groups[selected_index];
        let group = &wave.groups[group_index];
        let first = &group.entries[0];
        // The cost belongs to the selected group before family-specific budget
        // changes; those changes must not shorten its delay draws or burst.
        let group_cost = remaining_budget.min(group.entries.len() as i32);
        if wave_ordinal < 28 && first.enemy == EnemyToken::Zombie {
            remaining_budget -= 2;
        }
        let coffin_mode = first.enemy == EnemyToken::Coffin;
        if coffin_mode {
            remaining_budget = wave.spawn;
            selectable_groups.remove(selected_index);
        }
        if first.enemy == EnemyToken::Demon {
            remaining_budget -= 2;
        }
        if recurring_group_resets_budget(first, wave_ordinal) {
            remaining_budget = wave.spawn;
        } else if wave_ordinal < 37 && first.enemy == EnemyToken::Imp {
            remaining_budget -= 2;
        }
        let bonus_bound = (wave_ordinal / 3).clamp(1, 4) as i32;
        let mut bonus = 1;
        // Unlike the retained text samples, a singleton bonus is a constant.
        if bonus_bound > 1 {
            bonus += rng.draw_integer(bonus_bound);
        }
        let mut count = (group_cost + bonus) as usize;
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

fn recurring_group_resets_budget(first: &WaveGroupEntry, wave_ordinal: usize) -> bool {
    (first.enemy == EnemyToken::Skeleton && first.flags.contains(&EnemyFlag::Pike))
        || (wave_ordinal < 37
            && first.enemy == EnemyToken::Imp
            && (first.flags.contains(&EnemyFlag::Split)
                || first.flags.contains(&EnemyFlag::SplitMany)))
}
