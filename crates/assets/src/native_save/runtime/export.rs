//! Native export borrows the canonical restored wizard and durable profile.

use solomon_sim::economy::BoastSelection;

use crate::native_save::{
    NativeArchiveFile, NativeGameBoastState, NativeProfilePatch, NativeWizardPatch,
    PortableGameProfile, WEB_GAME_SAVE_SUPPORT_ARCHIVE_PATH, create_native_game_save_source,
    create_native_save_archive, create_portable_game_profile_from_native,
    load_embedded_native_hub_template, native_source_bytes, parse_portable_game_profile,
    patch_native_darkdata, patch_native_gamestate, validate_native_game_save_source,
};
use crate::native_save_source::NativeSaveSource;
use crate::website_save::{self, CreateSaveOptions, SaveIntegrity};
use serde_json::json;

use super::import::starting_skills;

#[derive(Clone, Debug)]
pub struct PortableExportResult {
    pub archive: Vec<u8>,
    pub portable: PortableGameProfile,
    pub warnings: Vec<String>,
}

fn source_for_export(source: Option<&NativeSaveSource>) -> Result<NativeSaveSource, String> {
    if let Some(source) = source {
        validate_native_game_save_source(source)?;
        return Ok(source.clone());
    }
    create_native_game_save_source(&load_embedded_native_hub_template()?)
}

pub fn create_portable_game_profile_from_web_save(
    document: &str,
) -> Result<PortableGameProfile, String> {
    let restored = website_save::restore(document)?;
    let profile = website_save::restore_profile(document)?;
    let source = source_for_export(restored.metadata.native_source.as_ref())?;
    let source_bytes = native_source_bytes(&source)?;
    let mut portable = create_portable_game_profile_from_native(&source_bytes, &[])?;
    let owner = restored
        .session
        .participant(&restored.player_id)
        .ok_or("web save has no portable wizard owner")?;
    let player = &owner.player;
    let book = &player.book;
    let progression = &player.progression;
    let combat = &player.combat;
    let runtime = &player.skill_runtime;
    let economy = &profile.economy;
    let (starting_primary, starting_secondary) = starting_skills(player.selection.element);
    let derived = solomon_sim::profile::player_skill_derived_stats_with_stat_book(
        runtime,
        book,
        &player.stat_book,
        progression,
        economy,
    );
    let belt_skills = player.belt.skill_projection();
    let selected_boast = player.economy.npc.boast.selected.as_ref();
    let mod_boast = matches!(selected_boast, Some(BoastSelection::Mod(_)));
    let boast = if mod_boast {
        NativeGameBoastState { failed: false, selected: None, succeeded: false }
    } else {
        NativeGameBoastState {
            failed: player.economy.npc.boast.failed,
            selected: selected_boast.and_then(BoastSelection::native_id),
            succeeded: player.economy.npc.boast.succeeded,
        }
    };
    let mut warnings = portable.warnings;
    if !economy.storage.is_empty() {
        warnings.push("Web Luthacus storage cannot replace the opaque retail item tree; the native attachment keeps its prior storage.".into());
    }
    if economy.tutorial_pending != economy.college_intro_pending {
        warnings.push("Retail has one first-play gate; export combines the browser Tutorial and College obligations.".into());
    }
    if book
        .advanced_unlocks
        .iter()
        .enumerate()
        .any(|(offset, unlocked)| *unlocked && book.permanent_ranks[offset + 72] == 0)
    {
        warnings.push(
            "Purchased-but-unlearned advanced spells cannot persist in an unmodified retail save."
                .into(),
        );
    }
    if profile.hagatha_runtime.serendipity_active || profile.hagatha_runtime.reverie_active {
        warnings.push("Active Serendipity/Reverie until-hurt effects are not retail disk members and reset in stock.".into());
    }
    if economy.unforge_bonuses.maximum_health != 0.0 || economy.unforge_bonuses.maximum_mana != 0.0
    {
        warnings.push("Web Unforge maximum-health/maximum-mana base bonuses have no retail disk fields and reset after stock reload.".into());
    }
    if player.belt.entries[8..].iter().any(Option::is_some) {
        warnings.push("Retail has only eight belt slots; hotbars 2 and 3 remain in the web save and are not included in the stock export.".into());
    }
    if book.primary_skill_id == 52 || belt_skills.contains(&Some(52)) {
        warnings.push("Retail does not serialize the active synthetic Weld build ID; selected or belted Spell Welding resets to the creation-element primary on stock export.".into());
    }
    if mod_boast {
        warnings.push(
            "Retail cannot represent a mod Boast selection; stock export clears the active Boast."
                .into(),
        );
    }
    portable.native_source = source;
    portable.profile.boast = boast;
    portable.profile.dowsing_fee = economy.dowsing_fee as i32;
    portable.profile.first_mixed = (0..super::super::NATIVE_FIRST_MIX_COUNT)
        .map(|selector| economy.first_mixed_selectors.contains(&(selector as u32)))
        .collect();
    portable.profile.gold = economy.gold as i32;
    portable.profile.hagatha_bundle_selectors = economy.hagatha_bundle_selectors.clone();
    portable.profile.help_pending = economy.npc.help_flags.to_vec();
    portable.profile.librarian_lace_read = economy.npc.librarian_lace_read;
    portable.profile.tutorial_pending = economy.tutorial_pending || economy.college_intro_pending;
    portable.warnings = warnings;
    let wizard = &mut portable.wizard;
    wizard.advanced_unlocks = book.advanced_unlocks.to_vec();
    wizard.cheat_death_charges = progression.hagatha_runtime.cheat_death_charges as i32;
    wizard.cheat_death_enabled = economy.owned_perk_selectors.contains(&7);
    wizard.concentration_skill_ids = [
        runtime.concentration_skill_id_a.map(|id| id as i32),
        runtime.concentration_skill_id_b.map(|id| id as i32),
    ];
    wizard.current_health = combat.current_health;
    wizard.current_mana = combat.current_mana;
    wizard.deferred_skill_choices = progression.deferred_skill_choices as i32;
    wizard.discipline_root = book.discipline_root as i32;
    wizard.element_root = book.element_root as i32;
    wizard.experience = progression.experience;
    wizard.experience_bonus = economy.unforge_bonuses.experience;
    wizard.firewalker_active = restored
        .session
        .secondary
        .players
        .get(&restored.player_id)
        .is_some_and(|player| player.firewalker);
    for selector in 0..super::super::NATIVE_HAGATHA_OWNERSHIP_COUNT {
        // Retail-only ownership rows remain in their original native attachment.
        if selector != 8 && selector < 28 {
            wizard.hagatha_ownership[selector] = if selector == 27 {
                economy.tonic_purchases > 0
            } else {
                economy.owned_perk_selectors.contains(&(selector as u32))
            };
        }
    }
    wizard.learned_order = book
        .learned_skill_order
        .iter()
        .filter(|id| book.permanent_ranks[**id] > 0)
        .map(|id| *id as i32)
        .collect();
    wizard.level = progression.level as i32;
    wizard.mana_cost_reduction = economy.unforge_bonuses.mana_cost_reduction;
    wizard.maximum_health = combat.maximum_health;
    wizard.maximum_mana = combat.maximum_mana;
    wizard.meditation_idle_delay = derived.meditation_idle_delay_ticks as i32;
    wizard.name = player.selection.name.clone();
    wizard.next_concentration_slot = if runtime.next_concentration_replacement_slot
        == solomon_sim::profile::PlayerConcentrationSlot::A
    {
        0
    } else {
        1
    };
    wizard.next_threshold = progression.next_threshold;
    wizard.offer_seed = progression.offer_seed as i32;
    wizard.offensive_damage_flat = economy.unforge_bonuses.offensive_damage;
    wizard.pending_skill_choices = progression.pending_levels.len() as i32;
    wizard.perk_capacity = economy.charm_capacity as i32;
    wizard.perk_selectors = economy.owned_perk_selectors.clone();
    wizard.permanent_ranks = book.permanent_ranks.to_vec();
    wizard.poison_immunity_ticks = progression.poison_immunity_ticks_remaining as i32;
    wizard.previous_threshold = progression.previous_threshold;
    wizard.selected_primary_skill_id = if book.primary_skill_id == 52 {
        starting_primary as i32
    } else {
        book.primary_skill_id as i32
    };
    wizard.skill_quickbar =
        belt_skills.iter().map(|id| id.filter(|id| *id != 52).map(|id| id as i32)).collect();
    wizard.starting_primary = starting_primary as i32;
    wizard.starting_secondary = starting_secondary as i32;
    wizard.weld_effect = runtime.equipment_modifiers.weld_effect;
    let darkdata = patch_native_darkdata(
        &source_bytes.darkdata,
        &NativeProfilePatch::from(&portable.profile),
    )?;
    let gamestate = patch_native_gamestate(
        &source_bytes.gamestate,
        &NativeWizardPatch::from(&portable.wizard),
        Some(&portable.profile.boast),
    )?;
    portable.native_source = create_native_game_save_source(&super::super::NativeSaveArchive {
        darkdata,
        gamestate,
        retained_files: source_bytes.retained_files,
        run_name: portable.native_source.run_name.clone(),
    })?;
    // DataView writes wrap integer disk words, but the final source parser still
    // validates the original web values. Retain them so an oversized saved value
    // cannot wrap into an apparently valid portable profile.
    let mut candidate = serde_json::to_value(&portable).map_err(|error| error.to_string())?;
    candidate["profile"]["gold"] = json!(economy.gold);
    candidate["profile"]["dowsingFee"] = json!(economy.dowsing_fee);
    candidate["wizard"]["cheatDeathCharges"] =
        json!(progression.hagatha_runtime.cheat_death_charges);
    candidate["wizard"]["deferredSkillChoices"] = json!(progression.deferred_skill_choices);
    candidate["wizard"]["meditationIdleDelay"] = json!(derived.meditation_idle_delay_ticks);
    candidate["wizard"]["pendingSkillChoices"] = json!(progression.pending_levels.len());
    candidate["wizard"]["poisonImmunityTicks"] = json!(progression.poison_immunity_ticks_remaining);
    parse_portable_game_profile(&crate::json_value::stringify_serializable(&candidate)?)
}

fn support_document(document: &str) -> Result<String, String> {
    let restored = website_save::restore(document)?;
    let mut metadata = restored.metadata;
    metadata.integrity = SaveIntegrity::LocalOnly;
    metadata.native_source = None;
    website_save::create(CreateSaveOptions {
        state: &restored.session,
        player_id: &restored.player_id,
        loaded_boneyard: restored.loaded_boneyard.as_ref(),
        metadata: &metadata,
        party_rejoin_token: None,
    })
}

pub fn export_web_game_save_to_native_archive(
    document: &str,
) -> Result<PortableExportResult, String> {
    let portable = create_portable_game_profile_from_web_save(document)?;
    let mut bytes = native_source_bytes(&portable.native_source)?;
    bytes
        .retained_files
        .retain(|file| file.path.to_lowercase() != WEB_GAME_SAVE_SUPPORT_ARCHIVE_PATH);
    bytes.retained_files.push(NativeArchiveFile {
        path: WEB_GAME_SAVE_SUPPORT_ARCHIVE_PATH.into(),
        bytes: support_document(document)?.into_bytes(),
    });
    let archive = create_native_save_archive(&bytes, 0)?;
    let warnings = portable.warnings.clone();
    Ok(PortableExportResult { archive, portable, warnings })
}
