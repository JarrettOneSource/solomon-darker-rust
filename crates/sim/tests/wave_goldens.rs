//! Exact native wave compilation, including retained/dead RNG draws and ordered flags.

use serde_json::Value;
use solomon_sim::NativeRng;
use solomon_sim::boneyard::waves::{
    CompiledSpawnBurst, RETAIL_WAVES, WaveDef, WaveGroup, WaveGroupEntry, timeline,
};
use solomon_sim::enemies::{EnemyFlag, EnemyToken};
use solomon_sim::enemies::{SpawnLocationPolicy, SpawnPositionPolicy};
use solomon_sim::rng::seed_from_text;

fn compare_rng(actual: &NativeRng, expected: &Value) {
    assert_eq!(actual.index_a, expected["indexA"].as_u64().unwrap() as usize);
    assert_eq!(actual.index_b, expected["indexB"].as_u64().unwrap() as usize);
    let words: Vec<u32> = serde_json::from_value(expected["words"].clone()).unwrap();
    assert_eq!(actual.words.as_slice(), words);
}

fn compare_bursts(actual: &[CompiledSpawnBurst], expected: &Value) {
    let expected = expected.as_array().unwrap();
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected) {
        assert_eq!(actual.after_delay_ticks, expected["afterDelayTicks"].as_u64().unwrap());
        assert_eq!(actual.count, expected["count"].as_u64().unwrap() as usize);
        assert_eq!(actual.group_index, expected["groupIndex"].as_i64().unwrap() as i32);
        assert_eq!(actual.spread_ticks, expected["spreadTicks"].as_f64().unwrap());
        assert_eq!(actual.start_delay_ticks, expected["startDelayTicks"].as_u64().unwrap());
        assert_eq!(actual.steady, expected["steady"].as_bool().unwrap());
        let location = match actual.location_policy {
            SpawnLocationPolicy::Anywhere => "anywhere",
            SpawnLocationPolicy::NearPlayer => "near-player",
        };
        let position = match actual.position_policy {
            SpawnPositionPolicy::Dark => "dark",
            SpawnPositionPolicy::Direct => "direct",
            SpawnPositionPolicy::Edge => "edge",
            SpawnPositionPolicy::Light => "light",
            SpawnPositionPolicy::Offscreen => "offscreen",
        };
        assert_eq!(location, expected["locationPolicy"]);
        assert_eq!(position, expected["positionPolicy"]);
        let entries = expected["entries"].as_array().unwrap();
        assert_eq!(actual.entries.len(), entries.len());
        for (actual, expected) in actual.entries.iter().zip(entries) {
            assert_eq!(actual.enemy.id(), expected["enemy"]);
            let flags: Vec<&str> = actual.flags.iter().map(|flag| flag.id()).collect();
            let expected: Vec<&str> = expected["flags"]
                .as_array()
                .unwrap()
                .iter()
                .map(|flag| flag.as_str().unwrap())
                .collect();
            assert_eq!(flags, expected);
        }
    }
}

#[test]
fn all_retained_wave_rows_compile_exactly_like_the_website() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/current-wave-compiler-goldens.json"))
            .unwrap();
    assert_eq!(fixture["sourceRevision"], "fa08ae249ff5563570bdb2a653e2ef9358ce7b83");
    assert_eq!(RETAIL_WAVES.len(), 42);
    assert_eq!(RETAIL_WAVES.iter().map(|wave| wave.groups.len()).sum::<usize>(), 205);
    for case in fixture["cases"].as_array().unwrap() {
        let seed = format!("{}:wave-compiler", case["seed"].as_str().unwrap());
        let mut rng = NativeRng::new(i64::from(seed_from_text(&seed)));
        let opening = timeline::compile_boneyard_opening(&mut rng);
        assert_eq!(
            opening.release_threshold,
            case["opening"]["releaseThreshold"].as_u64().unwrap() as usize
        );
        compare_bursts(&opening.bursts, &case["opening"]["bursts"]);
        compare_rng(&rng, &case["opening"]["rngState"]);
        for (index, wave) in RETAIL_WAVES.iter().enumerate() {
            let section = timeline::compile_boneyard_wave_section(wave, index + 1, &mut rng);
            let expected = &case["sections"][index];
            compare_bursts(&section.bursts, &expected["section"]["bursts"]);
            assert_eq!(
                section.lull_threshold,
                expected["section"]["lullThreshold"].as_u64().unwrap() as usize
            );
            assert_eq!(
                section.release_threshold,
                expected["section"]["releaseThreshold"].as_u64().unwrap() as usize
            );
            compare_rng(&rng, &expected["rngState"]);
        }
    }
}

// WaveDef borrows the immutable authored bank in production. These finite test rows
// deliberately have the same lifetime, so the fixture reaches the public compiler.
fn authored_wave(value: &Value) -> WaveDef {
    let mut groups = Vec::new();
    for group in value["groups"].as_array().unwrap() {
        let mut entries = Vec::new();
        for entry in group["entries"].as_array().unwrap() {
            let enemy = EnemyToken::from_id(entry["enemy"].as_str().unwrap()).unwrap();
            let flags: Vec<_> = entry["flags"]
                .as_array()
                .unwrap()
                .iter()
                .map(|flag| EnemyFlag::from_id(flag.as_str().unwrap()).unwrap())
                .collect();
            entries.push(WaveGroupEntry { enemy, flags: Box::leak(flags.into_boxed_slice()) });
        }
        groups.push(WaveGroup { entries: Box::leak(entries.into_boxed_slice()) });
    }
    let range = |name: &str| {
        [value[name][0].as_i64().unwrap() as i32, value[name][1].as_i64().unwrap() as i32]
    };
    WaveDef {
        spawn: value["spawn"].as_i64().unwrap() as i32,
        spawn_delay: range("spawnDelay"),
        wave_delay: range("waveDelay"),
        max_enemies: value["maxEnemies"].as_u64().unwrap() as usize,
        zombie_wave: value["zombieWave"].as_bool().unwrap_or(false),
        next: Box::leak(
            value["next"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_i64().unwrap() as i32)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        ),
        groups: Box::leak(groups.into_boxed_slice()),
    }
}

#[test]
fn current_group_budgets_draw_order_and_proven_nontermination_match_source() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/current-wave-compiler-goldens.json"))
            .unwrap();
    assert_eq!(fixture["scenarios"].as_array().unwrap().len(), 49);
    for case in fixture["scenarios"].as_array().unwrap() {
        let wave = authored_wave(&case["wave"]);
        let mut rng = NativeRng::new(1);
        rng.index_a = case["rng"]["indexA"].as_u64().unwrap() as usize;
        rng.index_b = case["rng"]["indexB"].as_u64().unwrap() as usize;
        rng.words = serde_json::from_value::<Vec<u32>>(case["rng"]["words"].clone())
            .unwrap()
            .try_into()
            .unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            timeline::compile_boneyard_wave_section(
                &wave,
                case["ordinal"].as_u64().unwrap() as usize,
                &mut rng,
            )
        }));
        if let Some(expected_error) = case["error"].as_str() {
            let failure = result.expect_err(case["name"].as_str().unwrap());
            let message = failure
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| failure.downcast_ref::<&str>().copied())
                .unwrap();
            assert_eq!(message, expected_error, "{}", case["name"]);
        } else {
            let section = result.unwrap();
            let expected = &case["result"];
            compare_bursts(&section.bursts, &expected["section"]["bursts"]);
            assert_eq!(
                section.lull_threshold,
                expected["section"]["lullThreshold"].as_u64().unwrap() as usize,
                "{}",
                case["name"]
            );
            assert_eq!(
                section.release_threshold,
                expected["section"]["releaseThreshold"].as_u64().unwrap() as usize,
                "{}",
                case["name"]
            );
            compare_rng(&rng, &expected["rngState"]);
        }
    }
}

#[test]
fn native_checkpoints_without_retained_graphs_rebuild_the_original_package_tables() {
    use solomon_sim::boneyard::waves::director::WaveDirector;
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/wave-compiler-goldens.json")).unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let mut checkpoint = WaveDirector::new(case["seed"].as_str().unwrap(), None).checkpoint();
        checkpoint.active_bursts = None;
        checkpoint.compiled_schedule = None;
        checkpoint.opening_bursts = None;
        checkpoint.opening_release_threshold = None;
        let migrated = WaveDirector::restore(checkpoint);
        compare_bursts(&migrated.opening_bursts, &case["opening"]["bursts"]);
        assert_eq!(
            migrated.opening_release_threshold,
            case["opening"]["releaseThreshold"].as_u64().unwrap() as usize
        );
        for (actual, expected) in
            migrated.compiled_schedule.iter().zip(case["sections"].as_array().unwrap())
        {
            compare_bursts(&actual.bursts, &expected["section"]["bursts"]);
            assert_eq!(
                actual.lull_threshold,
                expected["section"]["lullThreshold"].as_u64().unwrap() as usize
            );
            assert_eq!(
                actual.release_threshold,
                expected["section"]["releaseThreshold"].as_u64().unwrap() as usize
            );
        }
        assert_eq!(migrated.compiled_schedule.len(), case["sections"].as_array().unwrap().len());
    }
}
