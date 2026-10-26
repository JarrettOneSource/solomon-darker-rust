#![cfg(feature = "persistence")]
//! Actual current Inventory skill commands are accepted transactions, not input ticks.

#[path = "support/secondary.rs"]
#[allow(dead_code)] // This Session fixture uses only the shared secondary row projection.
mod secondary_rows;
mod support;

use serde_json::{Value, json};
use solomon_sim::Vec2;
use solomon_sim::belt::NativeBeltEntry;
use solomon_sim::boneyard::{
    BoneyardBounds, BoneyardScene, BoneyardSource, BoneyardSpawn, LoadedBoneyard,
};
use solomon_sim::enemies::config::EnemyArenaScalars;
use solomon_sim::enemies::path::EnemyNavigation;
use solomon_sim::enemies::world::{
    EnemyGeometry, EnemyMovementRequest, EnemyProjectileWorldQuery, EnemySpawnContext,
};
use solomon_sim::enemies::{EnemySpawnIntent, EnemyToken};
use solomon_sim::profile::{NewGameSelection, WizardDiscipline, WizardElement};
use solomon_sim::session::{GameSession, GameSessionOptions};

struct OpenArena;
impl EnemyNavigation for OpenArena {
    fn direct_path_clear(&self, _: Vec2, _: Vec2, _: f64, _: f64) -> bool {
        true
    }
    fn find_route(&self, _: Vec2, target: Vec2, _: f64, _: f64) -> Option<Vec<Vec2>> {
        Some(vec![target])
    }
}
impl EnemyGeometry for OpenArena {
    fn resolve_movement(&self, request: EnemyMovementRequest) -> Vec2 {
        request.position
    }
    fn line_obstruction(&self, _: Vec2, _: Vec2, _: u32) -> Option<Vec2> {
        None
    }
    fn projectile_world_blocked(&self, _: EnemyProjectileWorldQuery) -> bool {
        false
    }
}

fn loaded() -> LoadedBoneyard {
    LoadedBoneyard {
        choice_id: "mod:combat-fixture".into(),
        choice_source: BoneyardSource::Mod,
        mod_id: Some("combat-fixture".into()),
        mod_name: Some("Combat Fixture".into()),
        name: "Combat Fixture".into(),
        run_id: "modal-belt".into(),
        seed: "modal-belt-seed".into(),
        source_sha256: "c".repeat(64),
        geometry_sha256: "d".repeat(64),
        scene: BoneyardScene {
            name: "Combat fixture".into(),
            environment_mode: 2,
            bounds: BoneyardBounds::new(0.0, 0.0, 500.0, 500.0),
            spawn: BoneyardSpawn { position: Vec2::new(250.0, 250.0), facing_degrees: 180.0 },
            objects: Vec::new(),
            sprites: Vec::new(),
            roads: Vec::new(),
            fences: Vec::new(),
            terrain: Vec::new(),
            solomon_dig: None,
        }
        .into(),
    }
}

fn initial(case: &Value) -> GameSession {
    let spec = &case["spec"];
    let skill = spec["skillId"].as_u64().unwrap() as usize;
    let mut session = GameSession::new(
        "caster".into(),
        NewGameSelection {
            name: "Modal caster".into(),
            discipline: WizardDiscipline::Body,
            element: WizardElement::Fire,
        },
        GameSessionOptions { gameplay_rng_seed: 17, ..GameSessionOptions::default() },
    );
    if spec["hub"] != true {
        session.install_boneyard(loaded()).unwrap();
    }
    let participant = session.participants.get_mut("caster").unwrap();
    let player = &mut participant.player;
    player.book = secondary_rows::read_book(&case["inputs"]["book"]);
    player.skill_runtime.refresh_with_stat_book(
        &mut player.book,
        &player.stat_book,
        &player.economy,
    );
    player.belt.entries.fill(None);
    let slot = spec["slot"].as_u64().unwrap_or(0).min(23) as usize;
    player.belt.entries[slot] = Some(NativeBeltEntry::Skill { skill_id: skill });
    player.combat.current_mana = spec["mana"].as_f64().unwrap_or(10000.0);
    player.combat.maximum_mana = 10000.0;
    player.combat.current_health = 1000.0;
    player.combat.maximum_health = 1000.0;
    session.secondary = secondary_rows::read_state(&case["inputs"]["secondary"]);
    if matches!(skill, 51 | 77) {
        let world = session.boneyard.as_mut().unwrap();
        let tokens = [
            EnemyToken::Skeleton,
            EnemyToken::SkeletonArcher,
            EnemyToken::SkeletonMage,
            EnemyToken::Zombie,
        ];
        let intents: Vec<_> = tokens
            .into_iter()
            .enumerate()
            .map(|(index, token)| {
                EnemySpawnIntent::new(
                    token,
                    Vec2::new(300.0, 160.0 + index as f64 * 60.0),
                    index + 1,
                    session.ticks,
                    1,
                )
            })
            .collect();
        world.enemies.spawn(
            &intents,
            EnemySpawnContext {
                players: &[],
                tick: session.ticks,
                arena_scalars: EnemyArenaScalars::default(),
                roll_loot_seed: None,
            },
            &OpenArena,
            Some(&mut session.painter_order),
        );
        for mut actor in world.enemies.actors.iter_mut() {
            actor.heading_deg = 180.0;
            actor.next_movement_tick = 9_007_199_254_740_991;
        }
    }
    session
}

fn snapshot(session: &GameSession) -> Value {
    let participant = &session.participants["caster"];
    let flashes = &session.screen_flashes;
    let writes: Vec<_> = flashes.writes.iter().map(|row| json!({ "order":row.order,
        "tick":row.tick,"worldKey":row.world_key,"position":secondary_rows::point_snapshot(row.position),
        "flash":secondary_rows::flash_snapshot(row.flash),"onlyIfClear":row.only_if_clear })).collect();
    let enemies: Vec<_> = session.boneyard.iter().flat_map(|world| world.enemies.actors.iter()).map(|actor| json!({
        "id":actor.id,"position":secondary_rows::point_snapshot(actor.position),"heading":actor.heading_deg,
        "wanderHeading":actor.path.wander_heading_deg })).collect();
    let enemy_events: Vec<_> = session.boneyard.iter().flat_map(|world| &world.enemy_events).map(|event| json!({
        "actorId":event.actor_id,"eventId":event.event_id,"type":event.kind.id(),"tick":event.tick })).collect();
    json!({"tick":session.ticks,"position":secondary_rows::point_snapshot(participant.wizard.position),
        "headingIndex":participant.wizard.heading_index,"mana":participant.player.combat.current_mana,
        "primarySkillId":participant.player.book.primary_skill_id,
        "concentrations":[participant.player.skill_runtime.concentration_skill_id_a,participant.player.skill_runtime.concentration_skill_id_b],
        "secondary":secondary_rows::state_snapshot(&session.secondary),
        "flashes":{"epoch":flashes.epoch,"nextOrder":flashes.next_order,"writes":writes},
        "painters":{"nextRegistrationOrdinal":{"actor":session.painter_order.next_actor_ordinal,"transient":session.painter_order.next_transient_ordinal}},
        "enemies":enemies,"enemyEvents":enemy_events})
}

#[test]
fn public_modal_skill_commands_commit_aim_contacts_and_flashes_without_advancing_a_tick() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/modal-belt-goldens.json")).unwrap();
    assert_eq!(fixture["sourceRevision"], "fa08ae249ff5563570bdb2a653e2ef9358ce7b83");
    for (index, case) in fixture["cases"].as_array().unwrap().iter().enumerate() {
        let mut session = initial(case);
        support::assert_website_json(
            &snapshot(&session),
            &case["initial"],
            &format!("case{index}.initial"),
            &[],
        );
        let spec = &case["spec"];
        let slot = spec["slot"].as_u64().unwrap_or(0) as usize;
        let aim = spec["aim"].as_object().map(|_| secondary_rows::point(&spec["aim"]));
        for (step, row) in case["rows"].as_array().unwrap().iter().enumerate() {
            let result = session.activate_belt_slot("caster", slot, aim);
            assert_eq!(json!(result.transaction.accepted), row["accepted"], "case{index}.{step}");
            assert_eq!(
                json!(result.transaction.reason.map(|reason| reason.as_str())),
                row["reason"],
                "case{index}.{step}"
            );
            support::assert_website_json(
                &snapshot(&session),
                &row["expected"],
                &format!("case{index}.{step}"),
                &[],
            );
        }
    }
}
