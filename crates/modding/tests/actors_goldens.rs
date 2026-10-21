//! Actual custom actors, pickups and nested scenes retain the Source lifecycle.

mod common;

use serde::Deserialize;
use serde_json::{Value, json};
use solomon_modding::engines::{
    enemies::ModEnemyEngine,
    powerups::ModPowerupEngine,
    scenes::{ModPortalActivation, ModPortalEngine, ModSceneEngine},
};
use solomon_sim::Vec2;
use solomon_sim::modding::content::{PreparedModContentCatalog, PreparedModContentDefinition};
use solomon_sim::modding::runtime::{
    ModActorPlayer, ModEnemyCheckpoint, ModPowerupCheckpoint, ModPowerupCollectionEvent,
    ModSceneCheckpoint, PreparedModPortal,
};
use std::sync::Arc;

#[derive(Deserialize)]
struct Fixture {
    catalog: Vec<PreparedModContentDefinition>,
    portals: Vec<PreparedModPortal>,
    cases: Vec<Case>,
}
#[derive(Deserialize)]
struct Case {
    input: Input,
    output: Value,
    error: Option<String>,
    enemies: ModEnemyCheckpoint,
    powerups: ModPowerupCheckpoint,
    collections: Vec<ModPowerupCollectionEvent>,
    scenes: ModSceneCheckpoint,
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", rename_all_fields = "camelCase")]
enum Input {
    EnemySpawn {
        content_id: String,
        x: f64,
        y: f64,
        tick: u64,
    },
    EnemyTick {
        tick: u64,
        players: Vec<ModActorPlayer>,
        clamp: bool,
    },
    EnemyDamage {
        id: u64,
        amount: f64,
        tick: u64,
        source_player_id: Option<String>,
    },
    EnemyRestore {
        checkpoint: ModEnemyCheckpoint,
    },
    EnemyClear,
    PowerupSpawn {
        content_id: String,
        x: f64,
        y: f64,
        tick: u64,
    },
    Candidates {
        players: Vec<ModActorPlayer>,
    },
    Collect {
        id: u64,
        player_id: String,
        tick: u64,
    },
    PowerupTick {
        tick: u64,
    },
    PowerupRestore {
        checkpoint: ModPowerupCheckpoint,
    },
    PowerupClear,
    Enter {
        owner_id: String,
        content_id: String,
    },
    Room {
        owner_id: String,
        room_index: usize,
    },
    Return {
        owner_id: String,
    },
    SceneRestore {
        checkpoint: ModSceneCheckpoint,
    },
    Portal {
        actor_kind: String,
        confirmed_by_leader: bool,
        owner_id: String,
        portal_id: String,
        scene: String,
    },
}

#[test]
fn custom_enemy_pickup_scene_and_portal_lifecycles_match_source_transactions() {
    let fixture: Fixture = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/mod-actors-goldens.json"
    ))
    .unwrap();
    let catalog = Arc::new(PreparedModContentCatalog::from_prepared(fixture.catalog).unwrap());
    let mut enemies = ModEnemyEngine::new(Arc::clone(&catalog), 100).unwrap();
    let mut powerups = ModPowerupEngine::new(Arc::clone(&catalog));
    let mut scenes = ModSceneEngine::new(Arc::clone(&catalog));
    let portals = ModPortalEngine::new(&catalog).unwrap();
    assert_eq!(portals.portals(), fixture.portals);
    for (index, case) in fixture.cases.into_iter().enumerate() {
        let result:Result<Value,String>=match case.input {
            Input::EnemySpawn { content_id,x,y,tick }=>enemies.spawn(&content_id,Vec2::new(x,y),tick).map(|value|json!(value)),
            Input::EnemyTick { tick,players,clamp }=>enemies.tick(tick,&players,|_,requested,_|if clamp { Vec2::new(requested.x.clamp(0.0,110.0),requested.y.clamp(0.0,110.0)) }else{ requested }).map(|value|json!(value)),
            Input::EnemyDamage { id,amount,tick,source_player_id }=>enemies.damage(id,amount,tick,source_player_id.as_deref()).map(|value|json!(value)),
            Input::EnemyRestore { checkpoint }=>enemies.restore(checkpoint).map(|()|Value::Null),
            Input::EnemyClear=>{enemies.clear();Ok(Value::Null)},
            Input::PowerupSpawn { content_id,x,y,tick }=>powerups.spawn(&content_id,Vec2::new(x,y),tick).map(|value|json!(value)),
            Input::Candidates { players }=>Ok(json!(powerups.candidates(&players).iter().map(|candidate|json!({"instance":candidate.instance,"playerId":candidate.player_id})).collect::<Vec<_>>())),
            Input::Collect { id,player_id,tick }=>powerups.collect(id,&player_id,tick).map(|value|json!(value)),
            Input::PowerupTick { tick }=>powerups.tick(tick).map(|value|json!(value)),
            Input::PowerupRestore { checkpoint }=>powerups.restore(checkpoint).map(|()|Value::Null),
            Input::PowerupClear=>{powerups.clear();Ok(Value::Null)},
            Input::Enter { owner_id,content_id }=>scenes.enter(&owner_id,&content_id).map(|value|json!(value)),
            Input::Room { owner_id,room_index }=>scenes.select_room(&owner_id,room_index).map(|value|json!(value)),
            Input::Return { owner_id }=>Ok(json!(scenes.return_scene(&owner_id))),
            Input::SceneRestore { checkpoint }=>scenes.restore(checkpoint).map(|()|Value::Null),
            Input::Portal { actor_kind,confirmed_by_leader,owner_id,portal_id,scene }=>portals.activate(&mut scenes,ModPortalActivation { actor_kind:&actor_kind,confirmed_by_leader,owner_id:&owner_id,portal_id:&portal_id,scene:&scene }).map(|value|json!(value)),
        };
        match result {
            Ok(output) => {
                assert!(case.error.is_none(), "transaction{index} unexpectedly accepted");
                common::assert_json_values(&output, &case.output, &format!("transaction{index}"));
            }
            Err(error) => assert_eq!(Some(error), case.error, "transaction{index} error"),
        }
        common::assert_json_values(
            &json!(enemies.checkpoint()),
            &json!(case.enemies),
            &format!("transaction{index}.enemies"),
        );
        common::assert_json_values(
            &json!(powerups.checkpoint()),
            &json!(case.powerups),
            &format!("transaction{index}.powerups"),
        );
        common::assert_json_values(
            &json!(powerups.project_collections()),
            &json!(case.collections),
            &format!("transaction{index}.collections"),
        );
        assert_eq!(scenes.checkpoint(), case.scenes, "transaction{index}.scenes");
    }
}
