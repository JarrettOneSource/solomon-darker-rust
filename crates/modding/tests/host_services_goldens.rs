//! Actual Source services/projections borrow the same prepared host and Session.

mod common;
#[path = "common/inventory.rs"]
mod inventory;

use serde::Deserialize;
use serde_json::{Value, json};
use solomon_modding::engines::scenes::ModPortalActivation;
use solomon_modding::host::{PreparedModDerivedEvent, PreparedModHost, PreparedModUiActionInput};
use solomon_modding::package::{
    compile_web_session_content_definitions, materialize_web_session_content_value,
};
use solomon_sim::boneyard::{BoneyardSource, LoadedBoneyard};
use solomon_sim::economy::{BoastSelection, EquipmentSlot, HubInventoryAction};
use solomon_sim::enemies::SpawnLocationPolicy;
use solomon_sim::modding::{LuaObject, LuaValue};
use solomon_sim::profile::{NewGameSelection, WizardDiscipline, WizardElement};
use solomon_sim::session::{GameSession, GameSessionOptions};

#[derive(Deserialize)]
struct Fixture {
    payload: LuaValue,
    loaded: Value,
    cases: Vec<Case>,
}
#[derive(Deserialize)]
struct Case {
    input: Input,
    output: Value,
    error: Option<String>,
    game: Value,
    checkpoint: Value,
    content: Value,
    runtime: Value,
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", rename_all_fields = "camelCase")]
enum Input {
    Purchase {
        content_id: String,
        row: usize,
    },
    Xp {
        amount: f64,
    },
    Step {
        events: Vec<Event>,
        tick: u64,
    },
    Choose {
        content_id: String,
        mod_sequence: u64,
        #[serde(rename = "nativeSequence")]
        stock_sequence: u64,
    },
    Bind {
        slot: u32,
        content_id: Option<String>,
    },
    Portal {
        input: Portal,
    },
    Room {
        room: usize,
    },
    Return,
    Ui {
        input: Ui,
    },
    Unequip,
    Reforge {
        content_id: String,
        service: usize,
        item_id: u64,
    },
    Enter,
    Activate {
        content_id: Option<String>,
    },
    Tick {
        tick: u64,
    },
    Drain,
}
#[derive(Deserialize)]
struct Event {
    name: String,
    payload: LuaValue,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Portal {
    actor_kind: String,
    confirmed_by_leader: bool,
    owner_id: String,
    player_id: String,
    portal_id: String,
    scene: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Ui {
    action: String,
    arguments: LuaObject,
    content_id: String,
    player_id: String,
    request_id: u64,
}

#[test]
fn admitted_mod_skill_keeps_its_authored_random_boast_on_the_next_queued_offer() {
    let mut fixture: Value = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/mod-host-services-goldens.json"
    ))
    .unwrap();
    let source = fixture["payload"]["mods"][0]["entryScript"].as_str().unwrap();
    let random_boast =
        source.replace("stock_icon = 6,", "stock_icon = 6, random_skill_choices = true,");
    assert_ne!(source, random_boast, "the admitted fixture contains the authored Boast");
    fixture["payload"]["mods"][0]["entryScript"] = json!(random_boast);
    let payload = serde_json::from_value(fixture["payload"].clone()).unwrap();
    let package = compile_web_session_content_definitions(
        materialize_web_session_content_value(&payload).unwrap(),
    )
    .unwrap();
    let mut game = GameSession::new(
        "wizard".into(),
        NewGameSelection {
            name: "Tester".into(),
            element: WizardElement::Ether,
            discipline: WizardDiscipline::Arcane,
        },
        GameSessionOptions::default(),
    );
    let mut host = PreparedModHost::with_clock(&package, &mut game, || 0.0).unwrap();
    let selection = host.runtime().content.boasts().next().unwrap().selection.clone();
    game.participants.get_mut("wizard").unwrap().player.economy.npc.boast.selected =
        Some(BoastSelection::Mod(selection));
    game.grant_experience_with_extensions("wizard", 801.0, Some(host.runtime()));
    let progression = &game.participants.get("wizard").unwrap().player.progression;
    let level = progression.level;
    let offer = progression.pending_offer.as_ref().unwrap();
    let sequence = offer.sequence;
    assert!(offer.automatic_choice_index.is_some());
    let context = serde_json::from_value(json!({ "participant_id": "wizard" })).unwrap();
    host.step(
        &[PreparedModDerivedEvent {
            name: "level.up".into(),
            payload: serde_json::from_value(json!({ "level": level, "player_id": "wizard" }))
                .unwrap(),
        }],
        game.ticks,
        "profile",
        &context,
        &mut game,
    )
    .unwrap();
    let skill = host.runtime().content.skills().next().unwrap().common.content_id.clone();
    let mod_sequence = host.checkpoint().unwrap().skills.offers[0].sequence;
    host.choose_skill("wizard", &skill, mod_sequence, sequence, &mut game).unwrap();
    assert_eq!(host.checkpoint().unwrap().skills.ranks[0].rank, 1);
    let next =
        game.participants.get("wizard").unwrap().player.progression.pending_offer.clone().unwrap();
    assert_ne!(next.sequence, sequence);
    assert!(
        next.automatic_choice_index.is_some(),
        "the next queued offer keeps the admitted resolver"
    );
}

#[test]
fn authored_services_and_complete_client_projections_match_the_source_host() {
    let fixture: Fixture = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/mod-host-services-goldens.json"
    ))
    .unwrap();
    let package = compile_web_session_content_definitions(
        materialize_web_session_content_value(&fixture.payload).unwrap(),
    )
    .unwrap();
    let mut game = GameSession::new(
        "player-1".into(),
        NewGameSelection {
            name: "Tester".into(),
            element: WizardElement::Ether,
            discipline: WizardDiscipline::Arcane,
        },
        GameSessionOptions::default(),
    );
    let mut host = PreparedModHost::with_clock(&package, &mut game, || 0.0).unwrap();
    let context: LuaObject = serde_json::from_value(json!({"participant_id":"player-1"})).unwrap();
    for (index, case) in fixture.cases.into_iter().enumerate() {
        let path = format!("transaction{index}");
        let result: Result<Value, String> = match case.input {
            Input::Purchase { content_id, row } => {
                host.purchase_shop("player-1", &content_id, row, &mut game).map(|()| Value::Null)
            }
            Input::Xp { amount } => {
                game.grant_experience("player-1", amount);
                Ok(Value::Null)
            }
            Input::Step { events, tick } => {
                let events: Vec<_> = events
                    .into_iter()
                    .map(|event| PreparedModDerivedEvent {
                        name: event.name,
                        payload: event.payload,
                    })
                    .collect();
                host.step(&events, tick, "host-game-run", &context, &mut game)
                    .map(|value| json!(value))
            }
            Input::Choose { content_id, mod_sequence, stock_sequence } => host
                .choose_skill("player-1", &content_id, mod_sequence, stock_sequence, &mut game)
                .map(|()| Value::Null),
            Input::Bind { slot, content_id } => host
                .bind_mod_quickbar("player-1", slot, content_id.as_deref())
                .map(|()| Value::Null),
            Input::Portal { input } => host
                .enter_portal(
                    ModPortalActivation {
                        actor_kind: &input.actor_kind,
                        confirmed_by_leader: input.confirmed_by_leader,
                        owner_id: &input.owner_id,
                        portal_id: &input.portal_id,
                        scene: &input.scene,
                    },
                    &input.player_id,
                    &mut game,
                )
                .map(|()| Value::Null),
            Input::Room { room } => {
                host.select_scene_room("host-game-run", room, &mut game).map(|value| json!(value))
            }
            Input::Return => host.return_scene("host-game-run").map(|value| json!(value)),
            Input::Ui { input } => host
                .ui_action(
                    PreparedModUiActionInput {
                        action: &input.action,
                        arguments: input.arguments,
                        content_id: &input.content_id,
                        player_id: &input.player_id,
                        request_id: input.request_id,
                    },
                    &mut game,
                )
                .map(|value| json!(value)),
            Input::Unequip => {
                let result = game.participants.get_mut("player-1").unwrap().player.apply_inventory(
                    &HubInventoryAction::Unequip {
                        slot: EquipmentSlot::Weapon,
                        destination_sack_id: None,
                        destination_slot: None,
                    },
                    &mut game.gameplay_rng,
                );
                assert!(result.transaction.accepted, "canonical fixture unequip is admitted");
                Ok(Value::Null)
            }
            Input::Reforge { content_id, service, item_id } => host
                .reforge_shop("player-1", &content_id, service, item_id, &mut game)
                .map(|()| Value::Null),
            Input::Enter => {
                let loaded = &fixture.loaded;
                game.install_boneyard(LoadedBoneyard {
                    choice_id: "mod-host-game".into(),
                    choice_source: BoneyardSource::Mod,
                    mod_id: Some("example.host-content".into()),
                    mod_name: Some("Host Content".into()),
                    name: "Host Game".into(),
                    run_id: loaded["runId"].as_str().unwrap().into(),
                    seed: loaded["seed"].as_str().unwrap().into(),
                    source_sha256: loaded["sourceSha256"].as_str().unwrap().into(),
                    geometry_sha256: loaded["geometrySha256"].as_str().unwrap().into(),
                    scene: solomon_assets::boneyard::decode_scene(&loaded["scene"].to_string())
                        .unwrap()
                        .into(),
                })
                .map(|()| Value::Null)
            }
            Input::Activate { content_id } => {
                host.activate_boneyard(content_id.as_deref(), false, &game).map(|()| Value::Null)
            }
            Input::Tick { tick } => host.tick(tick, &mut game).map(|value| json!(value)),
            Input::Drain => {
                let spawns = host.drain_enemy_spawns().unwrap();
                let mut rows = Vec::new();
                for spawn in spawns {
                    assert_eq!(
                        spawn.location_policy,
                        SpawnLocationPolicy::Anywhere,
                        "Source host births use anywhere placement"
                    );
                    assert!(spawn.flags.is_empty(), "Source host births carry no native flags");
                    rows.push(json!({"enemyToken":spawn.enemy_token.id(),"flags":[],"id":spawn.id,"locationPolicy":"anywhere","nativeTypeId":spawn.type_id,"position":spawn.position,"spawnTick":spawn.spawn_tick,"waveOrdinal":spawn.wave_ordinal}));
                }
                Ok(json!({"spawns":rows,"presentation":host.drain_presentation().unwrap()}))
            }
        };
        match result {
            Ok(output) => {
                assert_eq!(case.error, None, "{path}");
                common::assert_json_values(&output, &case.output, &format!("{path}.output"));
            }
            Err(error) => assert_eq!(Some(error), case.error, "{path}.error"),
        }
        let player = &game.participants.get("player-1").expect("participant exists").player;
        let state = json!({"health":player.combat.current_health,"mana":player.combat.current_mana,"gold":player.economy.gold,"experience":player.progression.experience,"level":player.progression.level,"backpack":player.economy.backpack.iter().map(inventory::item_value).collect::<Vec<_>>()});
        common::assert_json_values(&state, &case.game, &format!("{path}.game"));
        common::assert_json_values(
            &json!(host.checkpoint().unwrap()),
            &case.checkpoint,
            &format!("{path}.checkpoint"),
        );
        common::assert_json_values(
            &json!(host.project().unwrap()),
            &case.content,
            &format!("{path}.content"),
        );
        common::assert_json_values(
            &json!(host.runtime_projection(Some("player-1"), &game).unwrap()),
            &case.runtime,
            &format!("{path}.runtime"),
        );
    }
}
