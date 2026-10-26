//! Source component ownership at `restoreGameSaveDocument`'s live-session boundary.

use serde_json::{Value, json};
use solomon_sim::boneyard::world::BoneyardSceneryTargetRetention;
use solomon_sim::boneyard::{BoneyardSource, BoneyardWorld, LoadedBoneyard};
use solomon_sim::game_run::GameRunPhase;
use solomon_sim::hub::ambient::HubAmbientState;
use solomon_sim::hub::interaction::HubNpcMarkerEpoch;
use solomon_sim::hub::npc::{HubCommonTraderClock, HubNpcState};
use solomon_sim::session::{GameSession, GameSessionOptions};

use super::contract::{
    ParsedSaveDocument, SaveMetadata, SaveWorldKind, integer, only_keys, record,
};
use super::value::decode;

#[derive(Clone, Debug)]
pub struct RestoredGameSave {
    pub session: GameSession,
    pub loaded_boneyard: Option<LoadedBoneyard>,
    pub player_id: String,
    pub metadata: SaveMetadata,
}

pub fn restore(document: &str) -> Result<RestoredGameSave, String> {
    let parsed = super::parse(document)?;
    super::contract::validate_json_tree(
        &serde_json::from_str(document).map_err(|error| format!("{error}"))?,
    )?;
    restore_parsed(parsed)
}

fn restore_parsed(parsed: ParsedSaveDocument) -> Result<RestoredGameSave, String> {
    let continuation = parsed.continuation.ok_or("game save has no resumable continuation")?;
    let summary = continuation.summary;
    for (id, _) in parsed.metadata.mod_state.iter() {
        if !parsed.metadata.mods.iter().any(|content| content.id.eq_ignore_ascii_case(id)) {
            return Err("game save state belongs to an inactive mod".into());
        }
    }
    let normalized = super::normalize::simulation(
        &continuation.simulation,
        &continuation.loaded_boneyard,
        &summary.player_id,
        parsed.source_schema_version,
    )?;
    let source = record(&normalized, "game save simulation")?;
    only_keys(
        source,
        "game save simulation",
        &[
            "accumulatorSeconds",
            "combatRng",
            "enhancedEffects",
            "gameRng",
            "hallOfFameClockStartedAtTick",
            "levelUpBarrier",
            "worldManagerOrder",
            "modEffects",
            "nextLevelUpBarrierId",
            "nextModConsumableUseId",
            "playerEntities",
            "primarySpells",
            "run",
            "screenFlashes",
            "secondaryAbilities",
            "tick",
            "world",
        ],
    )?;
    let tick = integer(&source["tick"], "game save simulation tick")?;
    let hall_tick = integer(&source["hallOfFameClockStartedAtTick"], "game save Hall clock")?;
    if hall_tick > tick {
        return Err("game save Hall clock is invalid".into());
    }
    if tick != summary.saved_at_tick {
        return Err("game save tick summary drifted".into());
    }
    if source["modEffects"].as_array().is_none_or(|effects| !effects.is_empty()) {
        return Err("game save may not persist active mod effects".into());
    }
    if source["nextModConsumableUseId"].as_f64() != Some(1.0) {
        return Err("game save mod consumable sequence is invalid".into());
    }
    let players = record(&source["playerEntities"], "game save players")?;
    const COMPONENTS: &[&str] = &[
        "belts",
        "configs",
        "economies",
        "entityIds",
        "identities",
        "lightings",
        "locomotions",
        "primaryCasts",
        "progressions",
        "skillBooks",
        "skillRuntimes",
        "statBooks",
    ];
    for field in COMPONENTS {
        if players.get(*field).and_then(Value::as_array).is_none_or(|rows| rows.len() != 1) {
            return Err("game save must contain exactly one owner component row".into());
        }
    }
    if players["identities"][0]["playerId"].as_str() != Some(&summary.player_id) {
        return Err("game save owner identity drifted".into());
    }
    let next_entity_id = integer(
        players.get("nextEntityId").ok_or("game save player entity sequence is invalid")?,
        "game save player entity sequence",
    )?;
    if next_entity_id < 1 {
        return Err("game save player entity sequence is invalid".into());
    }
    let selection = decode(players["configs"][0].clone())?;
    if selection != summary.character {
        return Err("game save owner character summary drifted".into());
    }
    let mut session =
        GameSession::new(summary.player_id.clone(), selection, GameSessionOptions::default());
    session.next_entity_id = next_entity_id;
    session.gameplay_rng = decode(source["gameRng"].clone())?;
    session.combat_rng = decode(source["combatRng"].clone())?;
    session.enhanced_effects = decode(source["enhancedEffects"].clone())?;
    session.screen_flashes = decode(source["screenFlashes"].clone())?;
    session.secondary = decode(source["secondaryAbilities"].clone())?;
    session.spells = decode(source["primarySpells"].clone())?;
    session.run = decode(source["run"].clone())?;
    session.painter_order = decode(source["worldManagerOrder"].clone())?;
    session.level_up_barrier = decode(source["levelUpBarrier"].clone())?;
    session.next_level_up_barrier_id =
        integer(&source["nextLevelUpBarrierId"], "game save level-up barrier sequence")?;
    session.hall_of_fame_clock_started_at_tick = hall_tick;
    session.ticks = tick;
    if session.run.phase != summary.phase {
        return Err("game save phase summary drifted".into());
    }
    let world = record(&source["world"], "game save world")?;
    let kind = match summary.world_kind {
        SaveWorldKind::Hub => "hub",
        SaveWorldKind::Boneyard => "boneyard",
    };
    if world.get("kind").and_then(Value::as_str) != Some(kind) {
        return Err("game save world summary drifted".into());
    }
    if summary.active_run
        != (summary.world_kind == SaveWorldKind::Boneyard
            && session.run.phase == GameRunPhase::Active)
    {
        return Err("game save active run summary drifted".into());
    }
    let participant = session.participants.get_mut(&summary.player_id).unwrap();
    *participant.entity_id = integer(&players["entityIds"][0], "game save player entity ID")?;
    participant.player.economy = decode(players["economies"][0].clone())?;
    if !participant.player.economy.inventory_is_valid() {
        return Err("game save player economy 0 inventory is invalid".into());
    }
    participant.player.book = decode(players["skillBooks"][0].clone())?;
    let expected_book = solomon_sim::profile::NativeSkillBookState::new(
        participant.player.selection.element,
        participant.player.selection.discipline,
    );
    if participant.player.book.element_root != expected_book.element_root
        || participant.player.book.discipline_root != expected_book.discipline_root
    {
        return Err("game save player skill book 0 roots disagree with its character".into());
    }
    participant.player.stat_book = decode(players["statBooks"][0].clone())?;
    participant.player.belt = decode(players["belts"][0].clone())?;
    participant.player.progression = decode(players["progressions"][0].clone())?;
    participant.player.combat = decode(players["progressions"][0].clone())?;
    participant.player.skill_runtime = decode(players["skillRuntimes"][0].clone())?;
    *participant.lighting = decode(players["lightings"][0].clone())?;
    let mut locomotion = record(&players["locomotions"][0], "game save player locomotion")?.clone();
    locomotion.insert("primaryCast".into(), players["primaryCasts"][0].clone());
    locomotion.insert("region".into(), Value::from("courtyard"));
    locomotion.insert("transition".into(), Value::Null);
    *participant.wizard = decode(Value::Object(locomotion))?;
    let loaded_boneyard = match summary.world_kind {
        SaveWorldKind::Hub => {
            if !continuation.loaded_boneyard.is_null()
                && !completed_tutorial_attachment(
                    &continuation.loaded_boneyard,
                    &participant.player.economy,
                    session.run.phase,
                )
            {
                return Err("Hub game save carries a Boneyard".into());
            }
            let (hub, legacy_intro) = super::normalize::hub::participant(
                &source["world"],
                &summary.player_id,
                parsed.source_schema_version,
            )?;
            if legacy_intro {
                *participant.wizard =
                    solomon_sim::HubWizard::at(solomon_sim::Vec2::new(972.0, 1044.0));
            }
            participant.wizard.region = decode(hub["region"].clone())?;
            participant.wizard.transition = decode(hub["transition"].clone())?;
            *participant.college_intro = decode(hub["collegeIntro"].clone())?;
            let seed = session.gameplay_rng.draw_integer(0x4000_0000) as u32;
            session.npcs =
                HubNpcState::with_live_student_painters(seed, &mut session.painter_order);
            session.ambient = HubAmbientState::new();
            session.hub_trader_animation_seed = seed;
            session.dowser_clock = HubCommonTraderClock::new(seed ^ 5016);
            *participant.marker_epoch =
                HubNpcMarkerEpoch::new(seed, tick, &participant.player.economy.npc.help_flags);
            None
        }
        SaveWorldKind::Boneyard => {
            let loaded = loaded_boneyard(&continuation.loaded_boneyard)?;
            if world["runId"].as_str() != Some(&loaded.run_id)
                || session.run.run_id.as_ref() != Some(&loaded.run_id)
                || session.run.phase != GameRunPhase::Active
            {
                return Err("game save Boneyard run ownership is inconsistent".into());
            }
            let mut checkpoint = BoneyardWorld::new(loaded.clone()).checkpoint();
            checkpoint.ticks = tick;
            checkpoint.death_weapons = decode(world["deathWeapons"].clone())?;
            checkpoint.next_death_weapon_id =
                integer(&world["nextDeathWeaponId"], "game save nextDeathWeaponId")?;
            checkpoint.gate_leaves = decode(world["gateLeaves"].clone())?;
            checkpoint.arena_transition = decode(world["arenaTransition"].clone())?;
            checkpoint.encounter = decode(world["encounter"].clone())?;
            checkpoint.tutorial = decode(world["tutorial"].clone())?;
            checkpoint.scenery_target_retention = Some(scenery_target_retention(world)?.into());
            checkpoint.enemies = decode(world["enemies"].clone())?;
            checkpoint.loot = decode(world["loot"].clone())?;
            checkpoint.enemy_world_feedback = decode(world["enemyWorldFeedback"].clone())?;
            checkpoint.enemy_events = decode(world["enemyEvents"].clone())?;
            checkpoint.loot_events = decode(world["lootEvents"].clone())?;
            checkpoint.player_ouch_deadline_tick =
                integer(&world["playerOuchDeadlineTick"], "game save player ouch deadline")?;
            checkpoint.lantern_position = decode(world["lanternPosition"].clone())?;
            checkpoint.lantern_light_registration =
                decode(world["lanternLightRegistration"].clone())?;
            checkpoint.solomon_painter_registration =
                decode(world["solomonPainterRegistration"].clone())?;
            checkpoint.waves = if world["waves"].is_null() {
                None
            } else {
                Some(decode(json!({
                    "seed": loaded.seed,
                    "sourceSha256": loaded.source_sha256,
                    "state": world["waves"],
                    "activeBursts": world["waves"]["activeBursts"],
                    "compiledSchedule": world["waves"]["compiledSchedule"],
                    "openingBursts": world["waves"]["openingBursts"],
                    "openingReleaseThreshold": world["waves"]["openingReleaseThreshold"],
                }))?)
            };
            *participant.score = decode(world["hallOfFameRuns"][&summary.player_id].clone())?;
            *participant.tutorial_profile_economy =
                decode(world["tutorialProfileEconomy"].clone())?;
            session.boneyard = Some(BoneyardWorld::restore(loaded.clone(), checkpoint)?);
            Some(loaded)
        }
    };
    session.migrate_participant_starter_appearance(&summary.player_id)?;
    let participant = session.participants.get_mut(&summary.player_id).unwrap();
    let player = participant.player;
    if player.autofill_skill_selections(&mut session.gameplay_rng) {
        let selected = if player.book.primary_skill_id == 52 {
            player.book.weld_build_id.expect("normalized Weld book retains its build")
        } else {
            player.book.primary_skill_id as u32
        };
        participant.wizard.primary_cast.reset_for_selection(selected);
    }
    if let Some(barrier) = &session.level_up_barrier {
        for id in &barrier.pending_player_ids {
            let pending = session
                .participants
                .get_mut(id)
                .ok_or("game save level-up barrier pending player is absent")?;
            let player = pending.player;
            if player.progression.pending_offer.is_some() {
                continue;
            }
            if parsed.source_schema_version >= 28 || player.progression.pending_levels.is_empty() {
                return Err("game save level-up barrier pending player has no skill offer".into());
            }
            player.progression.pending_offer =
                Some(solomon_sim::profile::build_player_skill_offer(
                    &player.progression,
                    &player.book,
                    player.combat.maximum_mana,
                    player.progression.revision,
                    &mut session.gameplay_rng,
                ));
        }
    }
    if parsed.source_schema_version < 48
        && let Some(world) = &mut session.boneyard
    {
        use solomon_sim::combat::PlayerLifeState;
        use solomon_sim::death_animations::NativeDeathWeaponActor;
        use solomon_sim::player_presentation::{
            PlayerDeathWeaponTrigger, legacy_player_death_weapon_motion,
        };
        for id in &session.participant_order {
            let participant = &session.participants.get(id).expect("participant exists");
            let combat = &participant.player.combat;
            let Some(registration) = participant.lighting.death_weapon_painter_registration else {
                continue;
            };
            if combat.death_epoch < 1
                || !matches!(
                    combat.life_state,
                    PlayerLifeState::Dying | PlayerLifeState::Spectating
                )
            {
                continue;
            }
            let weapon = solomon_sim::appearance::equipment::player_death_equipment_appearance(
                participant.player.selection.element,
                &participant.player.economy.equipment,
            )
            .weapon;
            let motion = legacy_player_death_weapon_motion(
                &PlayerDeathWeaponTrigger {
                    death_epoch: combat.death_epoch,
                    heading_index: participant.wizard.heading_index,
                    player_id: id.clone(),
                    run_id: world.loaded.run_id.clone(),
                    weapon,
                },
                combat.death_tick,
                participant.wizard.position,
            );
            world.death_weapons.push(NativeDeathWeaponActor {
                age_ticks: 0,
                birth_tick: tick,
                death_epoch: combat.death_epoch,
                id: world.next_death_weapon_id,
                life: 99999.0,
                motion,
                owner_id: id.clone(),
                painter_registration: registration,
                weapon,
            });
            world.next_death_weapon_id += 1;
        }
    }
    crate::save_document::validate_participant(
        &session.participants.get(&summary.player_id).unwrap(),
    )?;
    session.checkpoint().validate()?;
    Ok(RestoredGameSave {
        session,
        loaded_boneyard,
        player_id: summary.player_id,
        metadata: parsed.metadata,
    })
}

fn scenery_target_retention(
    world: &serde_json::Map<String, Value>,
) -> Result<BoneyardSceneryTargetRetention, String> {
    let targets = |field: &str| {
        world
            .get(field)
            .and_then(Value::as_array)
            .ok_or_else(|| format!("game save {field} must be an array"))
    };
    let mut earthquake_ids = std::collections::BTreeSet::new();
    let mut earthquake_variants = std::collections::BTreeMap::new();
    for target in targets("earthquakeSceneryTargets")? {
        let id = integer(&target["id"], "game save earthquake scenery target ID")?;
        let id = usize::try_from(id).map_err(|_| {
            "game save earthquake scenery target ID is outside the native range".to_owned()
        })?;
        earthquake_ids.insert(id);
        if let Some(variant) = target.get("secondaryVariant") {
            earthquake_variants.insert(id, decode(variant.clone())?);
        }
    }
    let string_ids = |field| -> Result<_, String> {
        let mut ids = std::collections::BTreeSet::new();
        for target in targets(field)? {
            let id = target["id"]
                .as_str()
                .ok_or_else(|| format!("game save {field} requires string target IDs"))?;
            ids.insert(id.to_owned());
        }
        Ok(ids)
    };
    Ok(BoneyardSceneryTargetRetention {
        earthquake_ids,
        earthquake_variants,
        primary_ids: string_ids("primarySceneryTargets")?,
        spell_ids: string_ids("scenerySpellTargets")?,
    })
}

fn completed_tutorial_attachment(
    value: &Value,
    economy: &solomon_sim::economy::HubEconomyState,
    phase: GameRunPhase,
) -> bool {
    if phase != GameRunPhase::Hub || economy.tutorial_pending {
        return false;
    }
    let Ok(loaded) = record(value, "game save loaded Boneyard") else {
        return false;
    };
    if only_keys(
        loaded,
        "game save loaded Boneyard",
        &["choice", "geometrySha256", "runId", "scene", "seed", "sourceSha256"],
    )
    .is_err()
    {
        return false;
    }
    for field in ["runId", "seed", "geometrySha256", "sourceSha256"] {
        if !value[field].is_string() || (field == "runId" && value[field].as_str() == Some("")) {
            return false;
        }
    }
    value["choice"]["id"].as_str() == Some("stock-tutorial")
        && value["choice"]["name"].as_str() == Some("Tutorial")
        && value["choice"]["source"].as_str() == Some("default")
        && value["scene"]["name"].as_str() == Some("Tutorial")
}

pub(super) fn loaded_boneyard(value: &Value) -> Result<LoadedBoneyard, String> {
    let loaded = record(value, "game save loaded Boneyard")?;
    only_keys(
        loaded,
        "game save loaded Boneyard",
        &["choice", "geometrySha256", "runId", "scene", "seed", "sourceSha256"],
    )?;
    let choice = record(&loaded["choice"], "game save Boneyard choice")?;
    let string = |value: &Value| {
        value
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| "game save loaded Boneyard identity is invalid".to_owned())
    };
    let run_id = string(&loaded["runId"])?;
    if run_id.is_empty() {
        return Err("game save loaded Boneyard identity is invalid".into());
    }
    let scene = crate::boneyard::decode_scene(&loaded["scene"].to_string())?;
    Ok(LoadedBoneyard {
        choice_id: string(choice.get("id").ok_or("game save Boneyard choice is invalid")?)?,
        choice_source: decode::<BoneyardSource>(
            choice.get("source").ok_or("game save Boneyard choice is invalid")?.clone(),
        )?,
        name: string(choice.get("name").ok_or("game save Boneyard choice is invalid")?)?,
        mod_id: choice.get("modId").and_then(Value::as_str).map(str::to_owned),
        mod_name: choice.get("modName").and_then(Value::as_str).map(str::to_owned),
        run_id,
        seed: string(&loaded["seed"])?,
        source_sha256: string(&loaded["sourceSha256"])?,
        geometry_sha256: string(&loaded["geometrySha256"])?,
        scene: scene.into(),
    })
}
