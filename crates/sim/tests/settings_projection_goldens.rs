use serde_json::Value;
use solomon_sim::settings::{GameSettings, camera_zoom_for_fov, game_light_quality};

#[test]
fn settings_project_into_native_camera_and_light_target_scales() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/settings-projection-goldens.json"))
            .unwrap();
    for case in fixture["cameras"].as_array().unwrap() {
        assert_eq!(
            camera_zoom_for_fov(
                case["nativeZoom"].as_f64().unwrap(),
                case["percent"].as_u64().unwrap() as u16
            ),
            case["expected"].as_f64().unwrap()
        );
    }
    for case in fixture["lights"].as_array().unwrap() {
        let settings = GameSettings {
            light_quality_percent: case["percent"].as_u64().unwrap() as u16,
            ..GameSettings::default()
        };
        assert_eq!(game_light_quality(&settings), case["expected"].as_f64().unwrap());
    }
}

#[test]
fn settings_controls_online_admission_and_legacy_bindings_match_current_source() {
    use serde_json::json;
    use solomon_sim::settings::{
        GameControlAction, GameControlBindings, game_online_preferences, game_shared_hub_enabled,
    };

    fn controls_record(controls: &GameControlBindings) -> Value {
        let mut result = json!({});
        for action in GameControlAction::ALL {
            result[action.as_str()] = json!(controls.code(action));
        }
        result
    }

    fn control_action(name: &str) -> GameControlAction {
        GameControlAction::ALL.into_iter().find(|action| action.as_str() == name).unwrap()
    }

    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/settings-projection-goldens.json"))
            .unwrap();
    let defaults = GameSettings::default();
    assert_eq!(controls_record(&defaults.controls), fixture["defaults"]["controls"]);
    assert_eq!(defaults.enable_activity_messages, fixture["defaults"]["enableActivityMessages"]);
    assert_eq!(defaults.enable_cheats, fixture["defaults"]["enableCheats"]);
    assert_eq!(defaults.enable_global_chat, fixture["defaults"]["enableGlobalChat"]);
    assert_eq!(defaults.enable_online_features, fixture["defaults"]["enableOnlineFeatures"]);
    assert_eq!(defaults.enable_shared_hub, fixture["defaults"]["enableSharedHub"]);
    assert_eq!(defaults.submit_runs_to_server, fixture["defaults"]["submitRunsToServer"]);
    assert_eq!(defaults.enhanced_effects, fixture["defaults"]["enhancedEffects"]);
    assert_eq!(defaults.reverse_hotbar_cycle, fixture["defaults"]["reverseHotbarCycle"]);
    for case in fixture["controls"].as_array().unwrap() {
        let mut controls = defaults.controls.clone();
        controls.rebind(
            control_action(case["action"].as_str().unwrap()),
            case["code"].as_str().unwrap(),
        );
        assert_eq!(
            controls_record(&controls),
            case["expected"],
            "{} {}",
            case["action"],
            case["code"]
        );
    }
    for case in fixture["online"].as_array().unwrap() {
        let bits = case["bits"].as_u64().unwrap();
        let settings = GameSettings {
            enable_online_features: bits & 1 != 0,
            enable_global_chat: bits & 2 != 0,
            enable_activity_messages: bits & 4 != 0,
            submit_runs_to_server: bits & 8 != 0,
            enable_shared_hub: bits & 16 != 0,
            ..GameSettings::default()
        };
        let preferences = game_online_preferences(&settings);
        assert_eq!(
            json!({
                "activityMessages": preferences.activity_messages,
                "globalChat": preferences.global_chat,
                "submitRuns": preferences.submit_runs,
            }),
            case["preferences"],
            "online bits {bits}"
        );
        assert_eq!(game_shared_hub_enabled(&settings), case["sharedHub"].as_bool().unwrap());
    }
    for case in fixture["migrations"].as_array().unwrap() {
        let source = &case["controls"];
        let mut controls = GameControlBindings {
            move_up: source["moveUp"].as_str().unwrap().into(),
            move_down: source["moveDown"].as_str().unwrap().into(),
            move_left: source["moveLeft"].as_str().unwrap().into(),
            move_right: source["moveRight"].as_str().unwrap().into(),
            open_menu: source["openMenu"].as_str().unwrap().into(),
            open_inventory: source["openInventory"].as_str().unwrap().into(),
            open_skills: source["openSkills"].as_str().unwrap().into(),
            open_chat: source["openChat"].as_str().unwrap().into(),
            open_cheats: source["openCheats"].as_str().unwrap_or("").into(),
            cycle_hotbar: source["cycleHotbar"].as_str().unwrap_or("").into(),
            belt: std::array::from_fn(|slot| {
                source[format!("belt{}", slot + 1)].as_str().unwrap().into()
            }),
        };
        if !controls.migrate_missing_bindings() {
            controls = GameControlBindings::default();
        }
        assert_eq!(controls_record(&controls), case["expected"]);
    }
}
