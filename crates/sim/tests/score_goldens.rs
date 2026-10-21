mod support;
use serde_json::{Value, json};
use solomon_sim::enemies::config::{EnemyArenaScalars, EnemyConfigRandom, evaluate_enemy_config};
use solomon_sim::enemies::{EnemySpawnIntent, EnemyToken};
use solomon_sim::score::{
    NativeHallOfFamePlayerScoreState, NativeHallOfFameRunState, native_hall_of_fame_award,
    native_hall_of_fame_enemy_name,
};
use solomon_sim::{NativeRng, Vec2};

fn snapshot(score: &NativeHallOfFameRunState) -> Value {
    json!({"awesomeness":score.awesomeness,"awesomestKill":score.awesomest_kill,
        "awesomestKillMaximumHealth":score.awesomest_kill_maximum_health,"elapsedTicks":score.elapsed_ticks,
        "killStreak":score.kill_streak,"monstersKilled":score.monsters_killed,
        "portraitHeadingIndex":score.portrait_heading_index,"portraitScale":score.portrait_scale,
        "startedAtTick":score.started_at_tick})
}
fn player(value: &Value) -> NativeHallOfFamePlayerScoreState {
    NativeHallOfFamePlayerScoreState {
        current_health: value["currentHealth"].as_f64().unwrap(),
        maximum_health: value["maximumHealth"].as_f64().unwrap(),
        level: value["level"].as_u64().unwrap() as u32,
        score_health_multiplier_enabled: value["scoreHealthMultiplierEnabled"].as_bool().unwrap(),
    }
}

#[test]
fn run_score_streak_and_archive_preserve_source_random_draws_and_awards() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/score-goldens.json")).unwrap();
    let enemies: Vec<_> = fixture["tokens"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| {
            let token = EnemyToken::ALL
                .into_iter()
                .find(|token| token.id() == value.as_str().unwrap())
                .unwrap();
            evaluate_enemy_config(
                &EnemySpawnIntent::new(token, Vec2::ZERO, 0, 0, 0),
                EnemyArenaScalars::default(),
                EnemyConfigRandom::default(),
            )
        })
        .collect();
    for (index, enemy) in enemies.iter().enumerate() {
        assert_eq!(
            native_hall_of_fame_enemy_name(enemy),
            fixture["names"][index].as_str().unwrap()
        );
    }
    let mut rng = NativeRng::new(17);
    let mut score = NativeHallOfFameRunState::new(100);
    let mut records = fixture["records"].as_array().unwrap().iter().peekable();
    for tick in 0..450 {
        score.record_enemy_kill(
            &enemies[tick % enemies.len()],
            player(&fixture["player"]),
            if tick % 3 == 0 { 0.49 } else { 1.0 },
            &mut rng,
        );
        if records.peek().is_some_and(|record| record["tick"].as_u64() == Some(tick as u64)) {
            let record = records.next().unwrap();
            support::assert_website_json(&snapshot(&score), &record["state"], "score", &[]);
            support::assert_website_json(
                &json!({"indexA":rng.index_a,"indexB":rng.index_b,"words":&rng.words[..]}),
                &record["rng"],
                "score RNG",
                &[],
            );
        }
    }
    score.reset_kill_streak();
    score.archive(900, 206.5, 0.9125);
    support::assert_website_json(&snapshot(&score), &fixture["archived"], "archive", &[]);
    for case in fixture["awards"].as_array().unwrap() {
        assert_eq!(
            native_hall_of_fame_award(71, 0.5, player(&case["player"]), 999),
            case["expected"].as_u64().unwrap()
        );
    }
}
