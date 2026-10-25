//! Source Console commands retain actual inventory, skill, RNG and cast components.

mod common;
#[path = "common/inventory.rs"]
mod inventory;

use serde::Deserialize;
use serde_json::{Value, json};
use solomon_sim::boneyard::{BoneyardSource, LoadedBoneyard};
use solomon_sim::modding::console::*;
use solomon_sim::profile::{NewGameSelection, PlayerSkillChoice, WizardDiscipline, WizardElement};
use solomon_sim::session::{GameSession, GameSessionOptions};
use solomon_sim::spells::casting::PlayerPrimaryCastState;

#[derive(Deserialize)]
struct Fixture {
    loaded: Value,
    items: Value,
    skills: Value,
    welds: Value,
    cases: Vec<Case>,
    #[serde(rename = "defaultFrame")]
    default_frame: Value,
}
#[derive(Deserialize)]
struct Case {
    input: Input,
    output: Value,
    frame: Value,
    backpack: Value,
    book: Value,
    rng: Value,
    offer: Value,
    primary: Value,
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum Input {
    Commands { commands: Vec<WebLuaCommand> },
    Enter,
    Items { key: String, quantity: f64 },
    Select { choice: Choice },
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Choice {
    choice_index: usize,
    offer_sequence: u64,
    skill_id: usize,
}

#[test]
fn console_catalogs_and_queued_commands_preserve_source_game_components_and_rng() {
    let fixture: Fixture = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/web-lua-commands-goldens.json"
    ))
    .unwrap();
    common::assert_json_values(&json!(web_lua_developer_items()), &fixture.items, "items");
    common::assert_json_values(&json!(web_lua_developer_skills()), &fixture.skills, "skills");
    common::assert_json_values(&json!(web_lua_developer_welds()), &fixture.welds, "welds");
    let mut default_game = GameSession::new(
        "player-1".into(),
        NewGameSelection {
            name: "Tester".into(),
            element: WizardElement::Ether,
            discipline: WizardDiscipline::Arcane,
        },
        GameSessionOptions::default(),
    );
    let loaded = &fixture.loaded;
    default_game
        .install_boneyard(LoadedBoneyard {
            choice_id: "default-random".into(),
            choice_source: BoneyardSource::Default,
            mod_id: None,
            mod_name: None,
            name: "Host Game".into(),
            run_id: loaded["runId"].as_str().unwrap().into(),
            seed: loaded["seed"].as_str().unwrap().into(),
            source_sha256: loaded["sourceSha256"].as_str().unwrap().into(),
            geometry_sha256: loaded["geometrySha256"].as_str().unwrap().into(),
            scene: solomon_assets::boneyard::decode_scene(&loaded["scene"].to_string())
                .unwrap()
                .into(),
        })
        .unwrap();
    common::assert_json_values(
        &json!(create_web_lua_frame_state(&default_game, Some("player-1"))),
        &fixture.default_frame,
        "defaultFrame",
    );
    let mut game = GameSession::new(
        "player-1".into(),
        NewGameSelection {
            name: "Tester".into(),
            element: WizardElement::Ether,
            discipline: WizardDiscipline::Arcane,
        },
        GameSessionOptions::default(),
    );
    for (index, case) in fixture.cases.into_iter().enumerate() {
        let path = format!("transaction{index}");
        let output = match case.input {
            Input::Commands { commands } => {
                let applied = apply_web_lua_commands(&mut game, &commands, None);
                let spawns:Vec<_>=applied.enemy_spawn_intents.iter().map(|spawn|json!({"enemyToken":spawn.enemy_token.id(),"flags":[],"id":spawn.id,"locationPolicy":"anywhere","nativeTypeId":spawn.native_type_id,"position":spawn.position,"spawnTick":spawn.spawn_tick,"waveOrdinal":spawn.wave_ordinal})).collect();
                json!({"enemySpawnIntents":spawns,"nextRunSeed":applied.next_run_seed})
            }
            Input::Items { key, quantity } => json!(
                create_web_lua_developer_grant_items(&key, quantity)
                    .map(|items| items.iter().map(inventory::item_value).collect::<Vec<_>>())
            ),
            Input::Select { choice } => {
                assert!(game.choose_skill(
                    "player-1",
                    PlayerSkillChoice {
                        choice_index: choice.choice_index,
                        offer_sequence: choice.offer_sequence,
                        skill_id: choice.skill_id
                    }
                ));
                Value::Null
            }
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
                .unwrap();
                Value::Null
            }
        };
        common::assert_json_values(&output, &case.output, &format!("{path}.output"));
        common::assert_json_values(
            &json!(create_web_lua_frame_state(&game, Some("player-1"))),
            &case.frame,
            &format!("{path}.frame"),
        );
        let participant = &game.participants["player-1"];
        let player = &participant.player;
        let backpack: Vec<_> = player.economy.backpack.iter().map(inventory::item_value).collect();
        common::assert_json_values(&json!(backpack), &case.backpack, &format!("{path}.backpack"));
        let book = &player.book;
        common::assert_json_values(
            &json!({"permanentRanks":book.permanent_ranks.to_vec(),"effectiveRanks":book.effective_ranks.to_vec(),"learnedSkillOrder":book.learned_skill_order,"advancedUnlocks":book.advanced_unlocks,"primarySkillId":book.primary_skill_id,"weldBuildId":book.weld_build_id,"weldComponentRanks":book.weld_component_ranks}),
            &case.book,
            &format!("{path}.book"),
        );
        common::assert_json_values(
            &json!({"indexA":game.gameplay_rng.index_a,"indexB":game.gameplay_rng.index_b,"words":game.gameplay_rng.words.to_vec()}),
            &case.rng,
            &format!("{path}.rng"),
        );
        let offer = player.progression.pending_offer.as_ref().map(|offer| {
            let options: Vec<_> = offer
                .options
                .iter()
                .map(|option| {
                    let mut value =
                        json!({"skillId":option.skill_id,"targetRank":option.target_rank});
                    if option.insight {
                        value["insight"] = json!(true);
                    }
                    if let Some(id) = option.weld_build_id {
                        value["weldBuildId"] = json!(id);
                    }
                    value
                })
                .collect();
            let mut value =
                json!({"level":offer.level,"options":options,"sequence":offer.sequence});
            if let Some(index) = offer.automatic_choice_index {
                value["automaticChoiceIndex"] = json!(index);
            }
            value
        });
        common::assert_json_values(&json!(offer), &case.offer, &format!("{path}.offer"));
        common::assert_json_values(
            &primary_value(&participant.wizard.primary_cast),
            &case.primary,
            &format!("{path}.primary"),
        );
    }
}

fn primary_value(cast: &PlayerPrimaryCastState) -> Value {
    json!({"actionTick":cast.action_tick.unwrap_or(-1.0),"aimDirection":cast.aim_direction,
        "castSequence":cast.cast_sequence,"channelActive":cast.channel_active,"emissionSequence":cast.emission_sequence,
        "etherBlastCharge":cast.ether_blast_charge,"etherBlastChargeCueSequence":cast.ether_blast_charge_cue_sequence,
        "fizzleSequence":cast.fizzle_sequence,"held":cast.held,"lastWeldPlaybackRate":cast.last_weld_playback_rate,
        "lastWeldSoundVariant":cast.last_weld_sound_variant,"oneShotAttackPoseHeld":cast.one_shot_attack_pose_held,
        "selectedPrimaryAgeTicks":cast.selected_primary_age_ticks,"selectedPrimaryId":cast.selected_primary_id.map_or(-1,i64::from),
        "targetId":cast.target_id,"underpowered":cast.underpowered,"weaponPulse":cast.weapon_pulse})
}
