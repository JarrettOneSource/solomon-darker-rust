//! Pure file-selection routing from `save/game-save-files.ts`; platform I/O stays outside.

use solomon_sim::profile::{WizardDiscipline, WizardElement};

use crate::native_save::{
    NativeSaveArchive, PortableGameProfile, WEB_GAME_SAVE_SUPPORT_ARCHIVE_PATH,
    create_native_game_save_source, create_portable_game_profile_from_native,
    load_embedded_native_hub_template, read_native_save_archive, read_zip,
};
use crate::native_save_source::NativeSaveSource;
use crate::website_save::{self, CreateSaveOptions, SaveIntegrity};

use super::import::create_web_game_save_from_portable_profile;

pub struct GameSaveInputFile<'a> {
    pub name: &'a str,
    pub relative_path: &'a str,
    pub bytes: &'a [u8],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameSaveImportSource {
    Game,
    Stock,
}

#[derive(Clone, Debug)]
pub struct GameSaveImportPreview {
    pub discipline: WizardDiscipline,
    pub display_name: String,
    pub document: String,
    pub element: WizardElement,
    pub gold: u64,
    pub hagatha_perks: usize,
    pub learned_rows: usize,
    pub level: u32,
    pub source: GameSaveImportSource,
    pub warnings: Vec<String>,
}

fn run_name(path: &str) -> String {
    let parts: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
    let candidate = if parts.len() >= 2 { parts[parts.len() - 2] } else { "" };
    if super::super::archive::run_name_is_valid(candidate) {
        candidate.into()
    } else {
        "_survival".into()
    }
}

fn utf8(bytes: &[u8]) -> Result<&str, String> {
    super::super::codec::utf8(bytes)
        .map_err(|_| "The encoded data was not valid for encoding utf-8".into())
}

fn standalone_gamestate(bytes: &[u8], name: String) -> Result<PortableGameProfile, String> {
    let template = load_embedded_native_hub_template()?;
    let save = NativeSaveArchive {
        darkdata: template.darkdata,
        gamestate: bytes.to_vec(),
        retained_files: Vec::new(),
        run_name: name,
    };
    create_portable_game_profile_from_native(&save, &["Only gamestate.sav was supplied. Wizard progression was imported from it; missing darkdata.cfg profile fields start from stock defaults (500 gold, fresh NPC/profile state, and empty Luthacus storage).".into()])
}

fn preview_save(
    document: &str,
    source: GameSaveImportSource,
    warnings: Vec<String>,
    native_source: Option<NativeSaveSource>,
) -> Result<GameSaveImportPreview, String> {
    let restored = website_save::restore(document)?;
    let player =
        &restored.session.participants.get(&restored.player_id).expect("participant exists").player;
    let mut metadata = restored.metadata;
    metadata.integrity = SaveIntegrity::LocalOnly;
    // A raw game JSON import starts a new local lineage. An embedded support
    // document instead receives the ZIP's current, byte-verified native files.
    if source == GameSaveImportSource::Game {
        metadata.native_source = native_source;
    }
    Ok(GameSaveImportPreview {
        discipline: player.selection.discipline,
        display_name: player.selection.name.clone(),
        document: website_save::create(CreateSaveOptions {
            state: &restored.session,
            player_id: &restored.player_id,
            loaded_boneyard: restored.loaded_boneyard.as_ref(),
            metadata: &metadata,
            party_rejoin_token: None,
        })?,
        element: player.selection.element,
        gold: player.economy.gold,
        hagatha_perks: player.economy.owned_perk_selectors.len(),
        learned_rows: player.book.permanent_ranks.iter().filter(|rank| **rank > 0).count(),
        level: player.progression.level,
        source,
        warnings,
    })
}

fn preview_stock(profile: &PortableGameProfile) -> Result<GameSaveImportPreview, String> {
    let imported = create_web_game_save_from_portable_profile(profile)?;
    preview_save(&imported.document, GameSaveImportSource::Stock, imported.warnings, None)
}

pub fn read_game_save_file_selection(
    files: &[GameSaveInputFile<'_>],
) -> Result<GameSaveImportPreview, String> {
    if files.is_empty() {
        return Err("Choose a save first.".into());
    }
    if files.len() == 1 && files[0].name.to_lowercase().ends_with(".json") {
        if files[0].bytes.len() > website_save::MAX_SAVE_BYTES {
            return Err("The save is too large.".into());
        }
        return preview_save(utf8(files[0].bytes)?, GameSaveImportSource::Game, Vec::new(), None);
    }
    if files.len() == 1 && files[0].name.to_lowercase().ends_with(".zip") {
        let entries = read_zip(files[0].bytes)?;
        if entries.iter().any(|file| file.path.to_lowercase() == "manifest.json") {
            let mut save = read_native_save_archive(files[0].bytes)?;
            let support = save
                .retained_files
                .iter()
                .find(|file| file.path.to_lowercase() == WEB_GAME_SAVE_SUPPORT_ARCHIVE_PATH)
                .map(|file| file.bytes.clone());
            save.retained_files
                .retain(|file| file.path.to_lowercase() != WEB_GAME_SAVE_SUPPORT_ARCHIVE_PATH);
            if let Some(support) = support {
                let source = create_native_game_save_source(&save)?;
                return preview_save(
                    utf8(&support)?,
                    GameSaveImportSource::Game,
                    Vec::new(),
                    Some(source),
                );
            }
            return preview_stock(&create_portable_game_profile_from_native(&save, &[])?);
        }
        if entries.len() != 1
            || entries[0]
                .path
                .split('/')
                .next_back()
                .is_none_or(|name| name.to_lowercase() != "gamestate.sav")
        {
            return Err("A ZIP without manifest.json must contain only one gamestate.sav.".into());
        }
        return preview_stock(&standalone_gamestate(
            &entries[0].bytes,
            run_name(&entries[0].path),
        )?);
    }
    let darkdata = files.iter().find(|file| file.name.to_lowercase() == "darkdata.cfg");
    let gamestate = files.iter().find(|file| file.name.to_lowercase() == "gamestate.sav");
    if files.len() == 1
        && let Some(gamestate) = gamestate
    {
        return preview_stock(&standalone_gamestate(
            gamestate.bytes,
            run_name(gamestate.relative_path),
        )?);
    }
    if files.len() != 2 || darkdata.is_none() || gamestate.is_none() {
        return Err("Choose a game save JSON, a save ZIP, one gamestate.sav, or darkdata.cfg and gamestate.sav together.".into());
    }
    let darkdata = darkdata.expect("the native pair contains darkdata.cfg");
    let gamestate = gamestate.expect("the native pair contains gamestate.sav");
    preview_stock(&create_portable_game_profile_from_native(
        &NativeSaveArchive {
            darkdata: darkdata.bytes.to_vec(),
            gamestate: gamestate.bytes.to_vec(),
            retained_files: Vec::new(),
            run_name: run_name(gamestate.relative_path),
        },
        &[],
    )?)
}
