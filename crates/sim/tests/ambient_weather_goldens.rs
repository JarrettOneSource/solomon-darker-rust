//! Actual Arena rain, independent from ability rain and gameplay RNG.
use serde_json::{Value, json};
use solomon_sim::boneyard::BoneyardBounds;
use solomon_sim::boneyard::weather::*;
use std::cell::Cell;

#[path = "support/website_rows.rs"]
mod website_rows;

#[test]
fn arena_rain_collision_lifetimes_birth_lighting_and_ramp_match_source() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/ambient-weather-goldens.json"))
            .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let mode = case["mode"].as_u64().unwrap() as u32;
        let enhanced = case["enhanced"].as_bool().unwrap();
        let mut weather = NativeBoneyardWeather::new(17, mode, enhanced);
        for frame in case["frames"].as_array().unwrap() {
            let tick = frame["tick"].as_u64().unwrap();
            let bounds = &frame["bounds"];
            let bounds = BoneyardBounds::new(
                bounds["x"].as_f64().unwrap(),
                bounds["y"].as_f64().unwrap(),
                bounds["w"].as_f64().unwrap(),
                bounds["h"].as_f64().unwrap(),
            );
            let collision_checks = Cell::new(0);
            weather.advance_to(
                tick,
                bounds,
                frame["viewportHeight"].as_f64().unwrap(),
                &|position, radius| {
                    assert_eq!(radius, 4.0);
                    collision_checks.set(collision_checks.get() + 1);
                    position.x < 0.0 && position.y < 0.0
                },
            );
            assert_eq!(collision_checks.get(), frame["collisionChecks"].as_u64().unwrap());
            let plan = weather.plan(&|position| {
                if tick < 80 {
                    0.25
                } else if position.x < 200.0 {
                    f64::NAN
                } else {
                    0.8
                }
            });
            let drops: Vec<_> = plan
                .drops
                .iter()
                .map(|drop| {
                    json!([
                        drop.id,
                        drop.position.x,
                        drop.position.y,
                        drop.height,
                        drop.length,
                        drop.start.x,
                        drop.start.y,
                        drop.end.x,
                        drop.end.y,
                        drop.start_color,
                        drop.end_color,
                        drop.start_alpha,
                        drop.end_alpha,
                        drop.width,
                    ])
                })
                .collect();
            let splashes: Vec<_> = plan
                .splashes
                .iter()
                .map(|splash| {
                    json!([
                        splash.id,
                        splash.position.x,
                        splash.position.y,
                        splash.age_ticks,
                        splash.alpha,
                        splash.scale,
                    ])
                })
                .collect();
            assert_eq!(weather.counts(), [drops.len(), splashes.len()]);
            website_rows::compare(
                &json!({"tick":plan.tick,"mode":plan.mode,"spawnCount":plan.spawn_count,
                "drops":drops,"splashes":splashes}),
                &frame["plan"],
                &format!("mode{mode}.{enhanced}.{tick}"),
            );
        }
    }
    for seed in fixture["seeds"].as_array().unwrap() {
        assert_eq!(
            u64::from(native_boneyard_weather_tick_seed(seed[0].as_u64().unwrap())),
            seed[1].as_u64().unwrap()
        );
    }
    assert_eq!(json!(native_boneyard_weather_streak_ramp().as_slice()), fixture["ramp"]);
}
