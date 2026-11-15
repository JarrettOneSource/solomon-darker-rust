//! Continuations written before skeleton boss progress was persisted still run both bosses.

use serde_json::{Value, json};
use solomon_assets::boneyard::BoneyardCatalog;
use solomon_assets::website_save::{self, CreateSaveOptions, SaveIntegrity, SaveMetadata};
use solomon_sim::boneyard::waves::director::{WavePhase, WaveTickContext};
use solomon_sim::boneyard::waves::programs::{FoulshaftPhase, SkeletonBossState};
use solomon_sim::boneyard::{BoneyardSource, LoadedBoneyard};
use solomon_sim::modding::LuaObject;
use solomon_sim::profile::{NewGameSelection, WizardDiscipline, WizardElement};
use solomon_sim::session::{GameSession, GameSessionOptions};

fn document(loaded: &LoadedBoneyard) -> Value {
    let mut session = GameSession::new(
        "local".into(),
        NewGameSelection {
            name: "Boss continuation".into(),
            element: WizardElement::Ether,
            discipline: WizardDiscipline::Arcane,
        },
        GameSessionOptions::default(),
    );
    session.install_boneyard(loaded.clone()).unwrap();
    let metadata = SaveMetadata {
        integrity: SaveIntegrity::LocalOnly,
        mods: Vec::new(),
        mod_state: LuaObject::default(),
        native_source: None,
    };
    serde_json::from_str(
        &website_save::create(CreateSaveOptions {
            state: &session,
            player_id: "local",
            loaded_boneyard: Some(loaded),
            metadata: &metadata,
            party_rejoin_token: None,
        })
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn missing_skeleton_programs_resume_both_authored_bosses_in_all_twelve_sources() {
    let catalog = BoneyardCatalog::load_embedded().unwrap();
    assert_eq!(catalog.templates.len(), 12);
    for index in 0..12_u32 {
        let mut seed = [0; 16];
        seed[..4].copy_from_slice(&index.to_be_bytes());
        let loaded = catalog.materialize(seed);
        for version in [33, website_save::SCHEMA_VERSION] {
            let mut save = document(&loaded);
            save["schemaVersion"] = json!(version);
            if version == 33 {
                save["profile"].as_object_mut().unwrap().remove("advancedUnlocks");
                let simulation = save["continuation"]["simulation"].as_object_mut().unwrap();
                simulation.remove("enhancedEffects");
                simulation.remove("screenFlashes");
            }
            save["continuation"]["simulation"]["world"]["waves"]
                .as_object_mut()
                .unwrap()
                .remove("skeletonBosses");
            let restored = website_save::restore(&save.to_string()).unwrap();
            let original = restored.session.boneyard.as_ref().unwrap().waves.as_ref().unwrap();
            let programs = original.programs.as_ref().unwrap();
            for boss in [&programs.ironmaw, &programs.foulshaft] {
                let mut director = original.clone();
                director.state.phase = WavePhase::WaveThreshold;
                director.state.wave_ordinal = boss.wave_ordinal;
                let births = director.step(WaveTickContext {
                    bounds: loaded.scene.bounds,
                    live_boss_count: 0,
                    live_enemy_count: 0,
                    live_zombie_count: 0,
                    players: &[],
                    tick: 1,
                });
                assert!(
                    births.iter().any(|birth| birth.authored_recipe.as_ref() == Some(&boss.recipe)),
                    "source {} schema {version} boss {}",
                    loaded.source_sha256,
                    boss.recipe.name
                );
                let state = director.state.skeleton_bosses.as_ref().unwrap();
                assert_eq!(state.ironmaw_spawned, boss.recipe.name == "Ironmaw");
                assert_eq!(
                    state.foulshaft_phase,
                    if boss.recipe.name == "Foulshaft" {
                        FoulshaftPhase::Reinforcements
                    } else {
                        FoulshaftPhase::Eligible
                    }
                );
                let mut missed = original.clone();
                missed.state.phase = WavePhase::WaveThreshold;
                missed.state.wave_ordinal = boss.wave_ordinal + 1;
                let births = missed.step(WaveTickContext {
                    bounds: loaded.scene.bounds,
                    live_boss_count: 0,
                    live_enemy_count: 0,
                    live_zombie_count: 0,
                    players: &[],
                    tick: 1,
                });
                assert!(
                    !births
                        .iter()
                        .any(|birth| birth.authored_recipe.as_ref() == Some(&boss.recipe))
                );
            }
        }
    }
}

#[test]
fn saved_skeleton_progress_and_explicit_null_are_preserved() {
    let loaded = BoneyardCatalog::load_embedded().unwrap().materialize([0; 16]);
    for phase in [
        FoulshaftPhase::Eligible,
        FoulshaftPhase::Reinforcements,
        FoulshaftPhase::BossWait,
        FoulshaftPhase::Retired,
    ] {
        let mut save = document(&loaded);
        let expected = SkeletonBossState {
            foulshaft_archers_remaining: 3,
            foulshaft_phase: phase,
            foulshaft_ticks_remaining: 25,
            ironmaw_spawned: true,
        };
        save["continuation"]["simulation"]["world"]["waves"]["skeletonBosses"] = json!({
            "foulshaftArchersRemaining":3, "foulshaftPhase":phase.id(),
            "foulshaftTicksRemaining":25, "ironmawSpawned":true, "sourceSha256":loaded.source_sha256,
        });
        let mut restored = website_save::restore(&save.to_string()).unwrap();
        let director = restored.session.boneyard.as_mut().unwrap().waves.as_mut().unwrap();
        assert_eq!(director.state.skeleton_bosses.as_ref(), Some(&expected));
        director.state.phase = WavePhase::WaveThreshold;
        director.state.wave_ordinal = director.programs.as_ref().unwrap().ironmaw.wave_ordinal;
        let births = director.step(WaveTickContext {
            bounds: loaded.scene.bounds,
            live_boss_count: 1,
            live_enemy_count: 1,
            live_zombie_count: 0,
            players: &[],
            tick: 1,
        });
        assert!(!births.iter().any(|birth| {
            birth.authored_recipe.as_ref().is_some_and(|recipe| recipe.name == "Ironmaw")
        }));
        let state = director.state.skeleton_bosses.as_ref().unwrap();
        assert!(state.ironmaw_spawned);
        assert_eq!(
            state.foulshaft_ticks_remaining,
            if matches!(phase, FoulshaftPhase::Reinforcements | FoulshaftPhase::BossWait) {
                24
            } else {
                25
            }
        );
    }
    let mut save = document(&loaded);
    save["continuation"]["simulation"]["world"]["waves"]["skeletonBosses"] = Value::Null;
    let restored = website_save::restore(&save.to_string()).unwrap();
    assert!(restored.session.boneyard.unwrap().waves.unwrap().state.skeleton_bosses.is_none());
}

#[test]
fn tutorial_and_custom_saves_do_not_acquire_generated_waves() {
    let catalog = BoneyardCatalog::load_embedded().unwrap();
    let mut custom = catalog.materialize([0; 16]);
    custom.choice_id = "custom".into();
    custom.choice_source = BoneyardSource::Mod;
    for loaded in [catalog.materialize_tutorial([0; 16]), custom] {
        let restored = website_save::restore(&document(&loaded).to_string()).unwrap();
        assert!(restored.session.boneyard.unwrap().waves.is_none());
    }
}
