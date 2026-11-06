//! One cloud document through the load path the hosted game uses, then through the host's
//! checkpoint writer, then through the load path again.
//!
//! The hosted client's title screen parses the stored document (`Game::load_hosted` in
//! `solomon-client`). On Continue, the host's admission (`HostedGameHost::load_admitted_save`)
//! restores the durable profile and then the continuation, and `resume_saved_wizard` loads
//! the restored Boneyard's navigation. A profile-only document starts a New Game:
//! `add_new_wizard` hydrates the profile into a fresh wizard in the College. The host's next
//! checkpoint (`owner_save`) writes a continuation, or the profile alone once the run has
//! reached Game Over or Loadout, and the client stores that text unchanged. This module calls
//! the same public operations in the same order.
//!
//! The host's session policies are not document properties and are not applied: whether a
//! local-only save may enter the Global College, whether the active mods match, and the signed
//! party recovery claim. Each save loads as the first wizard of a private College whose mods
//! match the save.

use std::cell::RefCell;
use std::panic::{self, AssertUnwindSafe};

use solomon_assets::website_save::{
    self, CreateSaveOptions, MAX_SAFE_INTEGER, MAX_SAVE_BYTES, ParsedSaveDocument,
    RestoredGameSave, SCHEMA_VERSION,
};
use solomon_launcher::host::HostContent;
use solomon_sim::game_run::GameRunPhase;
use solomon_sim::profile::{NewGameSelection, WizardDiscipline, WizardElement};
use solomon_sim::session::{GameSession, GameSessionOptions};

use crate::redact::{DocumentText, redact};

/// The player ID of the New Game wizard a profile-only save is hydrated into, the first ID
/// a private College assigns. Its character is fixed below: Create lets the player choose
/// any, and hydrating the profile does not depend on the choice.
const NEW_WIZARD_ID: &str = "player-1";

/// How one stored document fared. Failures carry a redacted message.
pub enum Outcome {
    Ok,
    /// The envelope is not a save document the game accepts.
    ParseError(String),
    /// The envelope parses, but its profile, continuation or Boneyard navigation does not load.
    RestoreError(String),
    /// The save loads, but the game cannot write it again or cannot load what it wrote.
    RoundTripError(String),
    /// The document is larger than the game's save limit, so it was not read.
    Oversize,
    /// The game panicked; the message names the stage and the code location only.
    Panic(String),
}

/// The schema a document declares: a version this build writes or reads, a later version,
/// a missing or malformed version, or none because the text is not read as JSON.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SchemaBucket {
    Version(u64),
    Newer,
    Invalid,
    Unreadable,
}

/// The result for one document.
pub struct Checked {
    pub schema: SchemaBucket,
    pub outcome: Outcome,
}

/// The step a document had reached when it failed.
#[derive(Clone, Copy)]
enum Stage {
    Parse,
    Restore,
    Write,
    Reload,
}

thread_local! {
    static PANIC_LOCATION: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Replaces the default panic report, which would print the panic message to stderr. A
/// message can contain document values; the code location cannot.
pub fn install_quiet_panic_hook() {
    panic::set_hook(Box::new(|info| {
        let location = info.location().map(|location| {
            format!("{}:{}:{}", location.file(), location.line(), location.column())
        });
        PANIC_LOCATION.with(|slot| *slot.borrow_mut() = location);
    }));
}

pub fn check(document: &str, content: &HostContent) -> Checked {
    if document.len() > MAX_SAVE_BYTES {
        return Checked { schema: SchemaBucket::Unreadable, outcome: Outcome::Oversize };
    }
    let mut stage = Stage::Parse;
    let mut version = None;
    let mut rewritten = None;
    let result = guarded(|| -> Result<(), String> {
        let parsed = website_save::parse(document)?;
        version = Some(parsed.source_schema_version);
        stage = Stage::Restore;
        let loaded = restore(document, &parsed, content)?;
        stage = Stage::Write;
        let text: &str = rewritten.insert(write(&loaded)?);
        stage = Stage::Reload;
        restore(text, &website_save::parse(text)?, content)?;
        Ok(())
    });
    let schema = match version {
        Some(version) if version <= SCHEMA_VERSION => SchemaBucket::Version(version),
        Some(_) => SchemaBucket::Newer,
        None => declared_schema(document),
    };
    let message = match result {
        Ok(Ok(())) => return Checked { schema, outcome: Outcome::Ok },
        Ok(Err(message)) => message,
        Err(location) => {
            let outcome = Outcome::Panic(format!("panicked during {} at {location}", stage.name()));
            return Checked { schema, outcome };
        }
    };
    let mut documents = vec![document];
    documents.extend(rewritten.as_deref());
    let message = redact(&message, &DocumentText::collect(&documents));
    let outcome = match stage {
        Stage::Parse => Outcome::ParseError(message),
        Stage::Restore => Outcome::RestoreError(message),
        Stage::Write => Outcome::RoundTripError(format!("write: {message}")),
        Stage::Reload => Outcome::RoundTripError(format!("reload: {message}")),
    };
    Checked { schema, outcome }
}

/// Runs `work`, turning a panic into the code location where it happened.
fn guarded<T>(work: impl FnOnce() -> T) -> Result<T, String> {
    panic::catch_unwind(AssertUnwindSafe(work)).map_err(|_| {
        PANIC_LOCATION
            .with(|slot| slot.borrow_mut().take())
            .unwrap_or_else(|| "an unknown location".into())
    })
}

/// Loads a parsed document into the session the game would play, as the host admits it:
/// Continue restores a continuation, and New Game hydrates a profile-only save into a fresh
/// wizard. Both restore the durable profile first.
fn restore(
    document: &str,
    parsed: &ParsedSaveDocument,
    content: &HostContent,
) -> Result<RestoredGameSave, String> {
    let profile = website_save::restore_profile(document)?;
    if parsed.continuation.is_none() {
        let mut session = GameSession::new(
            NEW_WIZARD_ID.into(),
            NewGameSelection {
                name: "Wizard".into(),
                element: WizardElement::Ether,
                discipline: WizardDiscipline::Arcane,
            },
            GameSessionOptions::default(),
        );
        website_save::hydrate_profile(&mut session, NEW_WIZARD_ID, &profile)?;
        return Ok(RestoredGameSave {
            session,
            loaded_boneyard: None,
            player_id: NEW_WIZARD_ID.into(),
            metadata: profile.metadata,
        });
    }
    let restored = website_save::restore(document)?;
    if let Some(loaded) = &restored.loaded_boneyard {
        content.load_navigation(loaded)?;
    }
    Ok(restored)
}

/// Writes the document the host's next checkpoint would send the client. Inside a party
/// recovery lineage the host also signs a party rejoin token with its server secret; the
/// checker writes the unsigned document the host writes everywhere else.
fn write(loaded: &RestoredGameSave) -> Result<String, String> {
    let state = &loaded.session;
    if matches!(state.run.phase, GameRunPhase::GameOver | GameRunPhase::Loadout) {
        return website_save::create_profile(state, &loaded.player_id, &loaded.metadata);
    }
    website_save::create(CreateSaveOptions {
        state,
        player_id: &loaded.player_id,
        loaded_boneyard: loaded.loaded_boneyard.as_ref(),
        metadata: &loaded.metadata,
        party_rejoin_token: None,
    })
}

/// The schema of a document the codec rejected before reading its version, read by the
/// codec's rule: a positive safe integer. Anything else is `Invalid`, so the summary never
/// repeats another value.
fn declared_schema(document: &str) -> SchemaBucket {
    let Ok(root) = serde_json::from_str::<serde_json::Value>(document) else {
        return SchemaBucket::Unreadable;
    };
    let Some(version) = root.get("schemaVersion").and_then(serde_json::Value::as_f64) else {
        return SchemaBucket::Invalid;
    };
    if version < 1.0 || version.fract() != 0.0 || version > MAX_SAFE_INTEGER as f64 {
        SchemaBucket::Invalid
    } else if version as u64 <= SCHEMA_VERSION {
        SchemaBucket::Version(version as u64)
    } else {
        SchemaBucket::Newer
    }
}

impl Stage {
    fn name(self) -> &'static str {
        match self {
            Self::Parse => "parse",
            Self::Restore => "restore",
            Self::Write => "write",
            Self::Reload => "reload",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_panic_reports_only_where_it_happened() {
        // The hook is process-wide; the other tests in this binary do not panic.
        let previous = panic::take_hook();
        install_quiet_panic_hook();
        let name = "CANARY Bob";
        let result = guarded(|| -> u32 { panic!("player {name} is not saved") });
        panic::set_hook(previous);
        let location = result.unwrap_err();
        assert!(location.contains("check.rs:"), "{location}");
        assert!(!location.contains("CANARY"), "{location}");
    }
}
