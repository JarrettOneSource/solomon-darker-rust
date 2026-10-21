use serde_json::Value;
use solomon_sim::menu::loading::{
    MATCH_LOADING_PRESENTATION_DELAY_MS, MatchLoadingFlow, MatchLoadingStage, MatchLoadingState,
};

#[test]
fn match_loading_stages_preserve_current_website_progress_copy_and_reveal_delay() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/match-loading-goldens.json")).unwrap();
    assert_eq!(MATCH_LOADING_PRESENTATION_DELAY_MS, fixture["delayMs"].as_f64().unwrap());
    for case in fixture["begins"].as_array().unwrap() {
        let loading = MatchLoadingState::begin(flow(&case["flow"]), stage(&case["stage"]), 1000.0);
        assert_state(&loading, &case["expected"]);
    }
    for sequence in fixture["sequences"].as_array().unwrap() {
        let mut loading =
            MatchLoadingState::begin(flow(&sequence["flow"]), stage(&sequence["start"]), 1000.0);
        for step in sequence["steps"].as_array().unwrap() {
            loading.advance(stage(&step["stage"]));
            assert_state(&loading, &step["expected"]);
        }
        loading.complete();
        assert_state(&loading, &sequence["completed"]);
    }
    let loading = MatchLoadingState::begin(
        MatchLoadingFlow::Hub,
        MatchLoadingStage::ConnectingTransport,
        1000.0,
    );
    for case in fixture["delay"].as_array().unwrap() {
        assert_eq!(
            loading.should_present(case["now"].as_f64().unwrap()),
            case["expected"].as_bool().unwrap()
        );
    }
}

fn flow(value: &Value) -> MatchLoadingFlow {
    match value.as_str().unwrap() {
        "hub" => MatchLoadingFlow::Hub,
        "boneyard" => MatchLoadingFlow::Boneyard,
        other => panic!("unknown loading flow {other}"),
    }
}

fn stage(value: &Value) -> MatchLoadingStage {
    MatchLoadingStage::from_id(value.as_str().unwrap()).expect("fixture loading stage")
}

fn assert_state(loading: &MatchLoadingState, expected: &Value) {
    assert_eq!(loading.flow.id(), expected["flow"].as_str().unwrap());
    assert_eq!(loading.label(), expected["label"].as_str().unwrap());
    assert_eq!(loading.progress, expected["progress"].as_f64().unwrap());
    assert_eq!(loading.stage.definition().id, expected["stage"].as_str().unwrap());
    assert_eq!(loading.started_at_ms, expected["startedAtMs"].as_f64().unwrap());
}
