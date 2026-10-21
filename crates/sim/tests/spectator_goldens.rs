use serde_json::{Value, json};
use solomon_sim::Vec2;
use solomon_sim::boneyard::spectator::*;
use solomon_sim::boneyard::waves::director::WavePhase;
use solomon_sim::combat::PlayerLifeState;
use solomon_sim::game_run::GameRunPhase;
#[path = "support/website_rows.rs"]
mod website_rows;

fn state(value: &Value) -> BoneyardSpectatorCameraState {
    BoneyardSpectatorCameraState {
        run_id: value["runId"].as_str().map(str::to_owned),
        target_player_id: value["targetPlayerId"].as_str().map(str::to_owned),
    }
}
fn point(value: &Value) -> Vec2 {
    Vec2::new(value["x"].as_f64().unwrap(), value["y"].as_f64().unwrap())
}
fn p(point: Vec2) -> Value {
    json!({"x":point.x,"y":point.y})
}
fn bounds(bounds: solomon_sim::boneyard::BoneyardBounds) -> Value {
    json!({"x":bounds.x,"y":bounds.y,"width":bounds.width,"height":bounds.height})
}

#[test]
fn spectator_camera_and_status_follow_the_actual_run_and_peer_lifetimes() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/spectator-goldens.json")).unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let source = &case["snapshot"];
        let players: Vec<_> = source["players"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(id, player)| BoneyardSpectatorPlayer {
                id,
                display_name: player["config"]["displayName"].as_str().unwrap(),
                position: point(&player["position"]),
                life_state: match player["progression"]["lifeState"].as_str().unwrap() {
                    "alive" => PlayerLifeState::Alive,
                    "lethal-pending" => PlayerLifeState::LethalPending,
                    "dying" => PlayerLifeState::Dying,
                    "spectating" => PlayerLifeState::Spectating,
                    other => panic!("life {other}"),
                },
            })
            .collect();
        let eligible: Vec<_> = source["run"]["eligiblePlayerIds"]
            .as_array()
            .unwrap()
            .iter()
            .map(|id| id.as_str().unwrap().to_owned())
            .collect();
        let snapshot = BoneyardSpectatorSnapshot {
            players: &players,
            eligible_player_ids: &eligible,
            run_id: source["run"]["runId"].as_str(),
            phase: match source["run"]["phase"].as_str().unwrap() {
                "active" => GameRunPhase::Active,
                "game-over" => GameRunPhase::GameOver,
                other => panic!("phase{other}"),
            },
        };
        let actual = boneyard_spectator_camera_state(
            &snapshot,
            "local",
            &state(&case["current"]),
            case["advance"].as_bool().unwrap(),
        );
        assert_eq!(actual, state(&case["expected"]), "{}", case["name"]);
        let focus = boneyard_camera_focus(&snapshot, "local", &actual, Vec2::new(33.0, 44.0));
        website_rows::compare(
            &json!({"playerId":focus.player_id,"position":p(focus.position)}),
            &case["focus"],
            case["name"].as_str().unwrap(),
        );
        let waves = &source["world"]["waves"];
        let waves = if waves.is_null() {
            None
        } else {
            Some(BoneyardSpectatorWaves {
                pending_spawn_budget: waves["pendingSpawnBudget"].as_u64().unwrap() as usize,
                wave_ordinal: waves["waveOrdinal"].as_u64().unwrap(),
                phase: match waves["phase"].as_str().unwrap() {
                    "opening" => WavePhase::Opening,
                    "spawning" => WavePhase::Spawning,
                    other => panic!("waves {other}"),
                },
            })
        };
        let count = source["world"]["enemies"].as_array().unwrap().len()
            + source["world"]["maggots"].as_array().unwrap().len();
        let status = boneyard_spectator_status(&snapshot, "local", &actual, count, waves);
        let actual=status.map_or(Value::Null,|status|json!({"accessibleLabel":status.accessible_label,"activeEnemyCount":status.active_enemy_count,"displayText":status.display_text,
            "incomingEnemyCount":status.incoming_enemy_count,"instruction":status.instruction,"respawnText":status.respawn_text,"runId":status.run_id,"targetPlayerId":status.target_player_id,
            "title":status.title,"waveOrdinal":status.wave_ordinal,"wavePhase":status.wave_phase.map(|phase|phase.id())}));
        website_rows::compare(&actual, &case["status"], case["name"].as_str().unwrap());
    }
    for case in fixture["layouts"].as_array().unwrap() {
        let layout = native_spectator_hud_layout(Vec2::new(
            case["viewport"]["width"].as_f64().unwrap(),
            case["viewport"]["height"].as_f64().unwrap(),
        ));
        let rail = |rail: NativeSpectatorHudRail| json!({"x":rail.bounds.x,"y":rail.bounds.y,"width":rail.bounds.width,"height":rail.bounds.height,"record":rail.record});
        let actual = json!({"surface":bounds(layout.surface),"text":p(layout.text),"horizontalRails":layout.horizontal_rails.map(rail),"verticalRails":layout.vertical_rails.map(rail),"corners":layout.corners.map(|corner|json!({"centerX":corner.center.x,"centerY":corner.center.y,"record":corner.record}))});
        website_rows::compare(&actual, &case["expected"], "spectator layout");
    }
}
