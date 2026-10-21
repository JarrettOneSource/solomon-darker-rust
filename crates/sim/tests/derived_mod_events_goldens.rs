#![cfg(feature = "persistence")]

#[path = "support/mod.rs"]
mod support;

use serde::Deserialize;
use serde_json::Value;
use solomon_sim::modding::events::{GameSimulationEventSnapshot, derive_web_lua_events};

#[derive(Deserialize)]
struct Fixture {
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    previous: GameSimulationEventSnapshot,
    current: GameSimulationEventSnapshot,
    includes: Option<Vec<String>>,
    expected: Value,
}

#[test]
fn derived_events_follow_actual_source_transitions_and_retained_actor_order() {
    let fixture: Fixture =
        serde_json::from_str(include_str!("fixtures/website/derived-mod-events-goldens.json"))
            .unwrap();
    for case in fixture.cases {
        let events = derive_web_lua_events(&case.previous, &case.current, |name| {
            case.includes.as_ref().is_none_or(|names| names.iter().any(|entry| entry == name))
        });
        support::assert_website_json(
            &serde_json::to_value(events).unwrap(),
            &case.expected,
            &case.name,
            &[],
        );
    }
}
