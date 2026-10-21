//! Versioned standalone save documents. The simulation owns the checkpoint; this I/O
//! boundary rejects incompatible or incomplete data before a live game is replaced.

use serde::{Deserialize, Serialize};
use solomon_sim::boneyard::{BoneyardWorld, LoadedBoneyard};
use solomon_sim::heading::HEADING_COUNT;
use solomon_sim::profile::{NATIVE_SKILL_ROW_COUNT, native_weld_build};
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
    let document: SaveDocument<GameCheckpoint> = serde_json::from_str(text)
        .map_err(|error| format!("Could not read the saved game: {error}"))?;
    let checkpoint = document.checkpoint;
    checkpoint.validate()?;
    for participant in checkpoint.participants.values() {
        validate_participant(participant)?;
    }
    Ok(checkpoint)
}

/// Host files also retain detached records outside the live map. Apply the same
/// constructor/index preconditions before either kind of participant is admitted.
pub fn validate_participant(participant: &SessionParticipant) -> Result<(), String> {
    let book = &participant.player.book;
    if book.primary_skill_id >= NATIVE_SKILL_ROW_COUNT
        || book.element_root >= 8
        || book.discipline_root >= 8
        || book.learned_skill_order.iter().any(|id| *id >= NATIVE_SKILL_ROW_COUNT)
        || book.weld_build_id.is_some_and(|id| native_weld_build(id).is_none())
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
            .insert("local".into(), solomon_sim::secondary::NativeSecondaryPlayerState::default());
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
}
