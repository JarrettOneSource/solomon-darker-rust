//! Versioned standalone save documents. The simulation owns the checkpoint; this I/O
//! boundary rejects incompatible or incomplete data before a live game is replaced.

use serde::{Deserialize, Serialize};
use solomon_sim::boneyard::{BoneyardWorld, LoadedBoneyard};
use solomon_sim::heading::HEADING_COUNT;
use solomon_sim::profile::{SKILL_ROW_COUNT, weld_build};
use solomon_sim::session::{GameCheckpoint, SessionParticipant};

// Schema 2 replaces the single local player and world-owned wizard with ordered
// participants. Earlier candidate saves cannot identify that ownership reliably.
const SAVE_SCHEMA_VERSION: u32 = 2;
// Same complete-run ceiling as the Website save contract, including nested sacks.
const MAX_SAVE_BYTES: usize = 96 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SaveDocument<T> {
    schema_version: u32,
    checkpoint: T,
}

#[derive(Deserialize)]
struct SaveVersion {
    schema_version: u32,
}

pub fn encode(checkpoint: &GameCheckpoint) -> Result<String, String> {
    let text =
        serde_json::to_string(&SaveDocument { schema_version: SAVE_SCHEMA_VERSION, checkpoint })
            .map_err(|error| format!("Could not encode the game: {error}"))?;
    // JSON turns non-finite floats into null. Verify the complete document before a write
    // can replace the last good save; a failed encode must never leave an unreadable slot.
    decode(&text)?;
    Ok(text)
}

pub fn decode(text: &str) -> Result<GameCheckpoint, String> {
    if text.len() > MAX_SAVE_BYTES {
        return Err("The saved game exceeds the supported size".into());
    }
    // Read the version before decoding the checkpoint. An old candidate should report
    // its incompatible schema, rather than a misleading missing-participants error.
    let version: SaveVersion = serde_json::from_str(text)
        .map_err(|error| format!("Could not read the saved game: {error}"))?;
    if version.schema_version != SAVE_SCHEMA_VERSION {
        return Err(format!(
            "Save format {} is not supported by this build",
            version.schema_version
        ));
    }
    let mut value: serde_json::Value = serde_json::from_str(text)
        .map_err(|error| format!("Could not read the saved game: {error}"))?;
    if let Some(checkpoint) = value.get_mut("checkpoint") {
        migrate_checkpoint(checkpoint)?;
    }
    let document: SaveDocument<GameCheckpoint> = serde_json::from_value(value)
        .map_err(|error| format!("Could not read the saved game: {error}"))?;
    let checkpoint = document.checkpoint;
    checkpoint.validate()?;
    for participant in checkpoint.participants.values() {
        validate_participant(participant)?;
    }
    Ok(checkpoint)
}

// Earlier checkpoints retained the trajectory but no acquired handle or turn
// history. Native null-target defaults preserve that trajectory without inventing
// a past query. Fresh releases use the recovered constructor/release contract.
/// Normalize historical fields in one native GameCheckpoint before typed decoding.
/// Save and private archive envelopes retain their own version/metadata checks.
/// Only the persisted spell queue is visited; authored JSON is never searched.
pub fn migrate_checkpoint(checkpoint: &mut serde_json::Value) -> Result<(), String> {
    if let Some(actors) =
        checkpoint.pointer_mut("/spells/transients").and_then(serde_json::Value::as_array_mut)
    {
        for actor in actors {
            if let Some(fields) = actor
                .pointer_mut("/Weld/Persistent/EtherealBoulder")
                .and_then(serde_json::Value::as_object_mut)
            {
                migrate_boulder_guidance_fields(fields, true)?;
            }
        }
    }
    Ok(())
}

pub(crate) fn migrate_website_boulder_guidance(
    actor: &mut serde_json::Map<String, serde_json::Value>,
) -> Result<(), String> {
    migrate_boulder_guidance_fields(actor, false)
}

fn migrate_boulder_guidance_fields(
    actor: &mut serde_json::Map<String, serde_json::Value>,
    native: bool,
) -> Result<(), String> {
    use serde_json::{Value, json};
    let key = |snake: &'static str, camel: &'static str| if native { snake } else { camel };
    let old_clock = key("lifetime_ticks_remaining", "lifetimeTicksRemaining");
    let old_turn = key("speed_factor", "speedFactor");
    if !actor.contains_key(old_clock) && !actor.contains_key(old_turn) {
        return Ok(());
    }
    if [
        key("flight_speed", "flightSpeed"),
        key("heading_degrees", "headingDegrees"),
        key("reacquires_target", "reacquiresTarget"),
        key("target_id", "targetId"),
        key("tracking_ticks_remaining", "trackingTicksRemaining"),
        key("turn_accumulator", "turnAccumulator"),
        key("turn_input", "turnInput"),
    ]
    .iter()
    .any(|field| actor.contains_key(*field))
    {
        return Err("saved Boulder mixes legacy and guidance fields".into());
    }
    let turn = actor
        .get(old_turn)
        .and_then(Value::as_f64)
        .filter(|v| v.is_finite() && *v > 0.0)
        .ok_or("saved Boulder turn input is invalid")?;
    let clock = actor
        .get(old_clock)
        .and_then(Value::as_u64)
        .ok_or("saved Boulder tracking clock is invalid")?;
    let direction = if native { &actor["common"]["direction"] } else { &actor["direction"] };
    let component = |value: &Value, axis: &str| {
        value[axis]
            .as_f64()
            .filter(|v| v.is_finite())
            .ok_or_else(|| format!("saved Boulder {axis} is invalid"))
    };
    let heading = solomon_sim::float::fround(solomon_sim::heading::heading_degrees_from_vector(
        component(direction, "x")?,
        component(direction, "y")?,
    ));
    let held = actor["phase"] == if native { "Held" } else { "held" };
    let speed = if held {
        2.25
    } else {
        solomon_sim::float::hypot(
            component(&actor["velocity"], "x")?,
            component(&actor["velocity"], "y")?,
        )
    };
    actor.remove(old_clock);
    actor.remove(old_turn);
    for (field, value) in [
        (key("flight_speed", "flightSpeed"), json!(speed)),
        (key("heading_degrees", "headingDegrees"), json!(heading)),
        (key("reacquires_target", "reacquiresTarget"), json!(held && turn > 1.0)),
        (key("target_id", "targetId"), Value::Null),
        (
            key("tracking_ticks_remaining", "trackingTicksRemaining"),
            json!(if held {
                if turn > 1.0 { (turn * 1000.0 + 250.0).trunc() as u64 } else { 250 }
            } else {
                clock
            }),
        ),
        (key("turn_accumulator", "turnAccumulator"), json!(0.01_f32 as f64)),
        (key("turn_input", "turnInput"), json!(turn)),
    ] {
        actor.insert(field.into(), value);
    }
    Ok(())
}

/// Host files also retain detached records outside the live map. Apply the same
/// constructor/index preconditions before either kind of participant is admitted.
pub fn validate_participant(participant: &SessionParticipant) -> Result<(), String> {
    let book = &participant.player.book;
    if book.primary_skill_id >= SKILL_ROW_COUNT
        || book.element_root >= 8
        || book.discipline_root >= 8
        || book.learned_skill_order.iter().any(|id| *id >= SKILL_ROW_COUNT)
        || book.weld_build_id.is_some_and(|id| weld_build(id).is_none())
        || (book.primary_skill_id == 52 && book.weld_build_id.is_none())
    {
        return Err("The saved skill book contains an invalid skill".into());
    }
    if participant.wizard.heading_index >= HEADING_COUNT {
        return Err("The saved wizard has an invalid heading".into());
    }
    Ok(())
}

pub fn load_boneyard(checkpoint: &GameCheckpoint) -> Result<Option<LoadedBoneyard>, String> {
    let Some(saved) = &checkpoint.boneyard else {
        return Ok(None);
    };
    if saved.choice_source != solomon_sim::boneyard::BoneyardSource::Default {
        return Err("The saved Boneyard is not in this build".into());
    }
    if saved.seed.len() != 32 || !saved.seed.is_ascii() {
        return Err("The saved Boneyard seed is invalid".into());
    }
    let mut seed = [0; 16];
    for (index, byte) in seed.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&saved.seed[index * 2..index * 2 + 2], 16)
            .map_err(|_| "The saved Boneyard seed is invalid")?;
    }
    let mut loaded = match saved.choice_id.as_str() {
        "default-random" => crate::boneyard::load_boneyard(seed),
        "stock-tutorial" => crate::boneyard::load_tutorial(seed),
        _ => Err("The saved Boneyard is not in this build".into()),
    }?;
    // Hosted materializeBoneyard draws the run identity separately from its scene seed.
    loaded.run_id.clone_from(&saved.run_id);
    // Reject changed assets before spending time uploading a scene for an incompatible run.
    BoneyardWorld::restore(loaded.clone(), *saved.clone())?;
    Ok(Some(loaded))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use solomon_sim::Vec2;
    use solomon_sim::profile::{NewGameSelection, WizardDiscipline, WizardElement};
    use solomon_sim::session::{
        GameSession, GameSessionInput, GameSessionOptions, GameSessionTickContext,
    };

    fn fresh_session() -> GameSession {
        let selection = NewGameSelection {
            name: "Felix".into(),
            element: WizardElement::Fire,
            discipline: WizardDiscipline::Mind,
        };
        let mut session = GameSession::new_participants(
            vec![("local".into(), selection.clone()), ("ally".into(), selection)],
            GameSessionOptions { gameplay_rng_seed: 71, ..GameSessionOptions::default() },
        )
        .unwrap();
        session
            .secondary
            .players
            .insert("local".into(), solomon_sim::secondary::SecondaryPlayerState::default());
        session.participants.get_mut("local").unwrap().world_aim = Some(Vec2::new(230.5, 510.75));
        session
    }

    fn step_hub(session: &mut GameSession) {
        let inputs = BTreeMap::from([
            (
                "local".into(),
                GameSessionInput { movement: Vec2::new(0.0, 1.0), ..GameSessionInput::default() },
            ),
            (
                "ally".into(),
                GameSessionInput { movement: Vec2::new(1.0, 0.0), ..GameSessionInput::default() },
            ),
        ]);
        session.step(&inputs, GameSessionTickContext::default());
    }

    fn step_boneyard_movement(session: &mut GameSession, movement: Vec2) {
        let wizard = &mut session.participants.get_mut("local").unwrap().wizard;
        session.boneyard.as_mut().unwrap().step_movement_only(wizard, movement);
    }

    #[test]
    fn resume_preserves_native_random_rings_skill_arrays_and_world_continuation() {
        let mut uninterrupted = fresh_session();
        uninterrupted.participants.get_mut("ally").unwrap().player.book.permanent_ranks[82] = 3;
        uninterrupted.participants.get_mut("local").unwrap().player.book.permanent_ranks[21] = 1;
        uninterrupted.secondary.players.get_mut("local").unwrap().cooldown_ticks_by_skill[21] =
            242.5;
        for _ in 0..100 {
            step_hub(&mut uninterrupted);
        }
        let checkpoint = decode(&encode(&uninterrupted.checkpoint()).unwrap()).unwrap();
        assert_eq!(checkpoint.participant_order, ["local", "ally"]);
        let mut resumed = GameSession::restore(checkpoint, None).unwrap();
        for _ in 0..100 {
            step_hub(&mut uninterrupted);
            step_hub(&mut resumed);
        }
        assert_eq!(
            encode(&resumed.checkpoint()).unwrap(),
            encode(&uninterrupted.checkpoint()).unwrap()
        );

        let mut seed = [0; 16];
        seed[3] = 7;
        uninterrupted.install_boneyard(crate::boneyard::load_boneyard(seed).unwrap()).unwrap();
        for _ in 0..115 {
            step_boneyard_movement(&mut uninterrupted, Vec2::new(0.0, 1.0));
        }
        let checkpoint = decode(&encode(&uninterrupted.checkpoint()).unwrap()).unwrap();
        let loaded = load_boneyard(&checkpoint).unwrap();
        let mut resumed = GameSession::restore(checkpoint, loaded).unwrap();
        for _ in 0..150 {
            step_boneyard_movement(&mut uninterrupted, Vec2::new(1.0, 0.0));
            step_boneyard_movement(&mut resumed, Vec2::new(1.0, 0.0));
        }
        assert_eq!(
            encode(&resumed.checkpoint()).unwrap(),
            encode(&uninterrupted.checkpoint()).unwrap()
        );
    }

    #[test]
    fn stock_scene_reloads_preserve_the_host_run_identity_and_validate_geometry() {
        let catalog = crate::boneyard::BoneyardCatalog::load_embedded().unwrap();
        let loaded =
            catalog.materialize_choice("default-random", [7; 16], "host-owned-run").unwrap();
        let mut session = fresh_session();
        session.install_boneyard(loaded.clone()).unwrap();
        let checkpoint = session.checkpoint();
        let restored = load_boneyard(&checkpoint).unwrap().unwrap();
        assert_eq!(restored, loaded);
        let mut changed_geometry = checkpoint;
        changed_geometry.boneyard.as_mut().unwrap().geometry_sha256 = "changed".into();
        assert!(load_boneyard(&changed_geometry).is_err());
    }

    #[test]
    fn invalid_checkpoints_are_rejected_before_they_can_replace_a_live_game() {
        let checkpoint = fresh_session().checkpoint();
        let value: serde_json::Value = serde_json::from_str(&encode(&checkpoint).unwrap()).unwrap();
        for (pointer, invalid) in [
            ("/schema_version", serde_json::json!(99)),
            ("/checkpoint/gameplay_rng/index_a", serde_json::json!(55)),
            ("/checkpoint/secondary/rng/divisor", serde_json::json!(0)),
            ("/checkpoint/participants/ally/player/book/primary_skill_id", serde_json::json!(83)),
            ("/checkpoint/participants/ally/player/book/primary_skill_id", serde_json::json!(52)),
            (
                "/checkpoint/participants/ally/player/book/permanent_ranks",
                serde_json::json!([1, 2]),
            ),
            ("/checkpoint/participants/ally/wizard/heading_index", serde_json::json!(24)),
            ("/checkpoint/participants/ally/id", serde_json::json!("other")),
            ("/checkpoint/participant_order", serde_json::json!(["local", "local"])),
            ("/checkpoint/run/eligible_player_ids", serde_json::json!(["missing"])),
        ] {
            let mut broken = value.clone();
            *broken.pointer_mut(pointer).unwrap() = invalid;
            assert!(decode(&broken.to_string()).is_err(), "accepted {pointer}");
        }
        let legacy = r#"{"schema_version":1,"checkpoint":{"player":{}}}"#;
        assert_eq!(decode(legacy).unwrap_err(), "Save format 1 is not supported by this build");
        let mut nonfinite = checkpoint;
        nonfinite.participants.get_mut("ally").unwrap().wizard.position.x = f64::NAN;
        assert!(encode(&nonfinite).is_err());
    }

    #[test]
    fn boulder_guidance_checkpoint_and_website_save_preserve_current_state_and_migrate_old_absence()
    {
        use solomon_sim::spells::model::PrimarySpellTransient;
        use solomon_sim::spells::weld::*;
        let mut session = fresh_session();
        let held = create_weld_persistent_actor(
            WeldActorBirth {
                build_id: 1006,
                direction: Vec2::new(1.0, 0.0),
                id: 1,
                origin: Vec2::new(120.0, 200.0),
                owner_id: "local",
                tick: 0,
                vector: &[12.0, 3.0, 1.0, 1.5, 2.5, 1.0],
                world_key: "hub:courtyard",
            },
            Some(&mut session.painter_order),
        );
        let mut actor = release_weld_persistent_actor(
            held.clone(),
            2,
            1,
            &mut session.combat_rng,
            Some(&mut session.painter_order),
            &[],
        )
        .actors
        .remove(0);
        let WeldPersistentActorState::EtherealBoulder(rock) = &mut actor else { panic!() };
        rock.common.owner.painter_registrations =
            vec![rock.common.owner.light_registration.unwrap()];
        rock.common.owner.age_ticks = 30;
        rock.flight_ticks = 12;
        rock.heading_degrees = 87.25;
        rock.target_id = Some("enemy:99".into());
        rock.turn_accumulator = 0.61_f32 as f64;
        rock.tracking_ticks_remaining = 1738;
        rock.hit_target_ids = vec!["enemy:2".into()];
        session.spells.next_id = 2;
        session
            .spells
            .transients
            .push(PrimarySpellTransient::Weld(WeldWorldActor::Persistent(actor)));
        let checkpoint = session.checkpoint();
        let current = encode(&checkpoint).unwrap();
        assert_eq!(encode(&decode(&current).unwrap()).unwrap(), current);
        for field in [
            "flight_speed",
            "heading_degrees",
            "reacquires_target",
            "target_id",
            "tracking_ticks_remaining",
            "turn_accumulator",
            "turn_input",
        ] {
            let mut missing: serde_json::Value = serde_json::from_str(&current).unwrap();
            missing
                .pointer_mut("/checkpoint/spells/transients/0/Weld/Persistent/EtherealBoulder")
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove(field);
            assert!(
                decode(&missing.to_string()).is_err(),
                "required wire/checkpoint field {field}"
            );
        }
        for legacy in ["lifetime_ticks_remaining", "speed_factor"] {
            let mut mixed: serde_json::Value = serde_json::from_str(&current).unwrap();
            mixed
                .pointer_mut("/checkpoint/spells/transients/0/Weld/Persistent/EtherealBoulder")
                .unwrap()[legacy] = serde_json::json!(1200);
            assert!(decode(&mixed.to_string()).unwrap_err().contains("mixes legacy"), "{legacy}");
        }
        let mut old: serde_json::Value = serde_json::from_str(&current).unwrap();
        let path = "/checkpoint/spells/transients/0/Weld/Persistent/EtherealBoulder";
        let fields = old.pointer_mut(path).unwrap().as_object_mut().unwrap();
        for key in [
            "flight_speed",
            "heading_degrees",
            "reacquires_target",
            "target_id",
            "tracking_ticks_remaining",
            "turn_accumulator",
            "turn_input",
        ] {
            fields.remove(key);
        }
        fields.insert("lifetime_ticks_remaining".into(), serde_json::json!(1200));
        fields.insert("speed_factor".into(), serde_json::json!(1.5));
        let mut old_held = old.clone();
        let held = old_held.pointer_mut(path).unwrap();
        held["phase"] = serde_json::json!("Held");
        held["flight_ticks"] = serde_json::json!(0);
        held["quantity"] = serde_json::json!(1);
        held["velocity"] = serde_json::json!({"x":0.0,"y":0.0});
        let migrated_held = serde_json::to_value(decode(&old_held.to_string()).unwrap()).unwrap();
        let held =
            &migrated_held["spells"]["transients"][0]["Weld"]["Persistent"]["EtherealBoulder"];
        assert_eq!(held["flight_speed"], 2.25);
        assert_eq!(held["tracking_ticks_remaining"], 1750);
        assert_eq!(held["reacquires_target"], true);
        assert_eq!(held["target_id"], serde_json::Value::Null);
        let migrated = decode(&old.to_string()).unwrap();
        let migrated = serde_json::to_value(&migrated).unwrap();
        let rock = &migrated["spells"]["transients"][0]["Weld"]["Persistent"]["EtherealBoulder"];
        assert_eq!(rock["target_id"], serde_json::Value::Null);
        assert_eq!(rock["turn_accumulator"], serde_json::json!(0.01_f32 as f64));
        assert_eq!(rock["tracking_ticks_remaining"], 1200);
        assert_eq!(rock["heading_degrees"], 90.0);
        assert_eq!(rock["common"]["origin"], serde_json::json!({"x":120.0,"y":200.0}));
        for invalid in [serde_json::json!(0), serde_json::json!(10.1)] {
            let mut bad: serde_json::Value = serde_json::from_str(&current).unwrap();
            bad.pointer_mut(path).unwrap()["turn_accumulator"] = invalid;
            assert!(decode(&bad.to_string()).is_err());
        }
        let metadata = crate::website_save::SaveMetadata {
            integrity: crate::website_save::SaveIntegrity::LocalOnly,
            mods: vec![],
            mod_state: Default::default(),
            native_source: None,
        };
        let website = crate::website_save::create(crate::website_save::CreateSaveOptions {
            state: &session,
            player_id: "local",
            loaded_boneyard: None,
            metadata: &metadata,
            party_rejoin_token: None,
        })
        .unwrap();
        let restored = crate::website_save::restore(&website).unwrap();
        let projected: serde_json::Value = serde_json::from_str(&website).unwrap();
        let saved = &projected["continuation"]["simulation"]["primarySpells"]["transients"][0];
        assert_eq!(saved["targetId"], "enemy:99");
        assert_eq!(saved["turnAccumulator"], serde_json::json!(0.61_f32 as f64));
        assert_eq!(saved["flightSpeed"], 2.25);
        assert_eq!(
            restored.session.spells.transients.iter().next().unwrap(),
            session.spells.transients.iter().next().unwrap()
        );
        for legacy in ["lifetimeTicksRemaining", "speedFactor"] {
            let mut mixed = projected.clone();
            mixed["schemaVersion"] = serde_json::json!(51);
            mixed["continuation"]["simulation"]["primarySpells"]["transients"][0][legacy] =
                serde_json::json!(1200);
            let error = crate::website_save::restore(&mixed.to_string())
                .expect_err("legacy Website rows cannot mix old and current guidance");
            assert!(error.contains("mixes legacy"), "{legacy}: {error}");
        }
        let mut old = projected;
        old["schemaVersion"] = serde_json::json!(51);
        let saved = old["continuation"]["simulation"]["primarySpells"]["transients"][0]
            .as_object_mut()
            .unwrap();
        for key in [
            "flightSpeed",
            "headingDegrees",
            "reacquiresTarget",
            "targetId",
            "trackingTicksRemaining",
            "turnAccumulator",
            "turnInput",
        ] {
            saved.remove(key);
        }
        saved.insert("lifetimeTicksRemaining".into(), serde_json::json!(1200));
        saved.insert("speedFactor".into(), serde_json::json!(1.5));
        let migrated = crate::website_save::restore(&old.to_string()).unwrap();
        let reexport = crate::website_save::create(crate::website_save::CreateSaveOptions {
            state: &migrated.session,
            player_id: &migrated.player_id,
            loaded_boneyard: None,
            metadata: &migrated.metadata,
            party_rejoin_token: None,
        })
        .unwrap();
        let reexport: serde_json::Value = serde_json::from_str(&reexport).unwrap();
        let saved = &reexport["continuation"]["simulation"]["primarySpells"]["transients"][0];
        assert_eq!(saved["targetId"], serde_json::Value::Null);
        assert_eq!(saved["turnAccumulator"], serde_json::json!(0.01_f32 as f64));
        assert_eq!(saved["trackingTicksRemaining"], 1200);
        assert_eq!(migrated.session.combat_rng, session.combat_rng);
    }

    #[test]
    fn boulder_disk_migration_preserves_unrelated_authored_objects() {
        let custom = serde_json::json!({"EtherealBoulder":{"lifetime_ticks_remaining":42,"speed_factor":"custom", "direction":"authored"}});
        let mut document = serde_json::json!({"checkpoint":{"spells":{"transients":[
            {"Weld":{"Persistent":{"EtherealBoulder":{"common":{"direction":{"x":1,"y":0}},"velocity":{"x":3,"y":0},"phase":"Flight","lifetime_ticks_remaining":90,"speed_factor":1.5}}}},
            {"custom":custom}
        ]},"mod_state":custom},"authored":custom});
        migrate_checkpoint(&mut document["checkpoint"]).unwrap();
        assert_eq!(document["authored"], custom);
        assert_eq!(document["checkpoint"]["mod_state"], custom);
        assert_eq!(document["checkpoint"]["spells"]["transients"][1]["custom"], custom);
        assert_eq!(
            document["checkpoint"]["spells"]["transients"][0]["Weld"]["Persistent"]["EtherealBoulder"]
                ["tracking_ticks_remaining"],
            90
        );
    }
}
