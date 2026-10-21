//! Settled-Hub conversion from `save/game-save-portability.ts`.

use solomon_sim::belt::PlayerBeltComponent;
use solomon_sim::economy::native_hagatha_outcome_state_is_valid;
use solomon_sim::float::fround;
use solomon_sim::modding::LuaObject;
use solomon_sim::profile::{
    NativeHagathaRuntimeState, NativeSkillBookState, NewGameSelection, PlayerConcentrationSlot,
    PlayerProgressionState, PlayerSkillRuntimeState, PlayerStatBookComponent, WizardDiscipline,
    WizardElement, native_skill_category,
};
use solomon_sim::secondary::NativeSecondaryPlayerState;
use solomon_sim::session::{GameSession, GameSessionOptions, PlayerLevelUpBarrier};

use crate::native_save::PortableGameProfile;
use crate::website_save::{self, CreateSaveOptions, SaveIntegrity, SaveMetadata};

#[derive(Clone, Debug)]
pub struct PortableImportResult {
    pub character: NewGameSelection,
    pub document: String,
    pub player_id: String,
    pub warnings: Vec<String>,
}

pub(super) fn starting_skills(element: WizardElement) -> (usize, usize) {
    match element {
        WizardElement::Ether => (8, 11),
        WizardElement::Fire => (16, 21),
        WizardElement::Air => (24, 27),
        WizardElement::Water => (32, 35),
        WizardElement::Earth => (40, 45),
    }
}

fn character(profile: &PortableGameProfile) -> Result<NewGameSelection, String> {
    let element = match profile.wizard.element_root {
        0 => WizardElement::Ether,
        1 => WizardElement::Fire,
        2 => WizardElement::Air,
        3 => WizardElement::Water,
        4 => WizardElement::Earth,
        _ => return Err("portable class roots are invalid".into()),
    };
    let discipline = match profile.wizard.discipline_root {
        0 => WizardDiscipline::Ether,
        1 => WizardDiscipline::Fire,
        2 => WizardDiscipline::Air,
        3 => WizardDiscipline::Water,
        4 => WizardDiscipline::Earth,
        5 => WizardDiscipline::Body,
        6 => WizardDiscipline::Mind,
        7 => WizardDiscipline::Arcane,
        _ => return Err("portable class roots are invalid".into()),
    };
    let (primary, secondary) = starting_skills(element);
    if profile.wizard.starting_primary != primary as i32
        || profile.wizard.starting_secondary != secondary as i32
    {
        return Err("portable class roots and starting spells disagree".into());
    }
    Ok(NewGameSelection { name: profile.wizard.name.clone(), element, discipline })
}

fn skill_book(profile: &PortableGameProfile, character: &NewGameSelection) -> NativeSkillBookState {
    let mut book = NativeSkillBookState::new(character.element, character.discipline);
    book.permanent_ranks = profile
        .wizard
        .permanent_ranks
        .as_slice()
        .try_into()
        .expect("a parsed portable profile has 83 skill rows");
    book.permanent_ranks[..8].fill(1);
    book.effective_ranks = book.permanent_ranks;
    book.learned_skill_order = profile.wizard.learned_order.iter().map(|id| *id as usize).collect();
    for skill_id in 8..=79 {
        if book.permanent_ranks[skill_id] > 0 && !book.learned_skill_order.contains(&skill_id) {
            book.learned_skill_order.push(skill_id);
        }
    }
    for (index, unlocked) in profile.wizard.advanced_unlocks.iter().enumerate() {
        book.advanced_unlocks[index] = *unlocked || book.permanent_ranks[index + 72] > 0;
    }
    book.discipline_root = profile.wizard.discipline_root as usize;
    book.element_root = profile.wizard.element_root as usize;
    let selected = profile.wizard.selected_primary_skill_id as usize;
    if [8, 16, 24, 32, 40].contains(&selected) && book.permanent_ranks[selected] > 0 {
        book.primary_skill_id = selected;
    }
    book
}

fn belt(profile: &PortableGameProfile, book: &NativeSkillBookState) -> PlayerBeltComponent {
    let mut skills = [None; 8];
    for (slot, source) in profile.wizard.skill_quickbar.iter().enumerate() {
        if let Some(skill_id) = source {
            let id = *skill_id as usize;
            if id != 52
                && book.permanent_ranks[id] > 0
                && matches!(native_skill_category(id), Some(1 | 2))
            {
                skills[slot] = Some(id);
            }
        }
    }
    PlayerBeltComponent::from_skill_quickbar(skills)
        .expect("the portable belt contains only native primary and secondary skills")
}

/// Import one validated portable profile into the actual shared simulation owner.
/// The native attachment stays opaque; no in-flight native Arena is reconstructed.
pub fn create_web_game_save_from_portable_profile(
    profile: &PortableGameProfile,
) -> Result<PortableImportResult, String> {
    let character = character(profile)?;
    let player_id = format!("native-{}", &profile.native_source.gamestate_sha256[..24]);
    let mut state = GameSession::new(
        player_id.clone(),
        character.clone(),
        GameSessionOptions {
            gameplay_rng_seed: i64::from(profile.wizard.offer_seed),
            ..GameSessionOptions::default()
        },
    );
    let player = &mut state
        .participants
        .get_mut(&player_id)
        .expect("the imported participant was just created")
        .player;
    let economy = &mut player.economy;
    economy.owned_perk_selectors = profile.wizard.perk_selectors.clone();
    economy.tonic_purchases =
        economy.owned_perk_selectors.iter().filter(|selector| **selector == 27).count() as u32;
    economy.charm_capacity = profile.wizard.perk_capacity as u32;
    if !native_hagatha_outcome_state_is_valid(
        &economy.owned_perk_selectors,
        economy.tonic_purchases,
        economy.charm_capacity,
    ) {
        return Err("portable Hagatha outcome state is invalid".into());
    }
    economy.npc.boast.failed = profile.profile.boast.failed;
    economy.npc.boast.failure_sequence = u64::from(profile.profile.boast.failed);
    economy.npc.boast.selected = profile.profile.boast.selected.map(Into::into);
    economy.npc.boast.succeeded = profile.profile.boast.succeeded;
    economy.npc.help_flags = profile
        .profile
        .help_pending
        .as_slice()
        .try_into()
        .expect("the portable profile has ten help flags");
    economy.npc.librarian_lace_read = profile.profile.librarian_lace_read;
    economy.college_intro_pending = profile.profile.tutorial_pending;
    economy.dowsing_fee = profile.profile.dowsing_fee as u64;
    economy.first_mixed_selectors = profile
        .profile
        .first_mixed
        .iter()
        .enumerate()
        .filter_map(|(selector, mixed)| mixed.then_some(selector as u32))
        .collect();
    economy.gold = profile.profile.gold as u64;
    economy.hagatha_bundle_selectors = profile.profile.hagatha_bundle_selectors.clone();
    economy.revision += 1;
    economy.tutorial_pending = profile.profile.tutorial_pending;
    economy.unforge_bonuses.experience = profile.wizard.experience_bonus;
    economy.unforge_bonuses.mana_cost_reduction = profile.wizard.mana_cost_reduction;
    economy.unforge_bonuses.offensive_damage = profile.wizard.offensive_damage_flat;

    let mut book = skill_book(profile, &character);
    let stat_book = PlayerStatBookComponent::default();
    let hagatha = NativeHagathaRuntimeState {
        cheat_death_charges: profile.wizard.cheat_death_charges as u32,
        ..NativeHagathaRuntimeState::default()
    };
    let mut runtime = PlayerSkillRuntimeState::new_with_stat_book(&mut book, &stat_book, economy);
    for (index, selected) in profile.wizard.concentration_skill_ids.iter().enumerate() {
        if let Some(skill_id) = selected {
            let id = *skill_id as usize;
            if book.permanent_ranks[id] > 0
                && native_skill_category(id) == Some(3)
                && (index == 0 || economy.owned_perk_selectors.contains(&21))
            {
                let slot = if index == 0 {
                    PlayerConcentrationSlot::A
                } else {
                    PlayerConcentrationSlot::B
                };
                runtime.set_concentration_slot_with_stat_book(
                    &mut book, &stat_book, economy, id, slot,
                );
            }
        }
    }
    runtime.next_concentration_replacement_slot = if profile.wizard.next_concentration_slot == 0 {
        PlayerConcentrationSlot::A
    } else {
        PlayerConcentrationSlot::B
    };
    let mut progression = PlayerProgressionState::new(profile.wizard.offer_seed as u32);
    progression.hagatha_runtime = hagatha;
    let derived = solomon_sim::profile::player_skill_derived_stats_with_stat_book(
        &runtime,
        &book,
        &stat_book,
        &progression,
        economy,
    );
    // Native saves omit the web-only HP/MP bases. Match the source's two fround
    // operations while transferring only the saved current/maximum ratios.
    player.combat.current_health = fround(
        derived.maximum_health
            * fround(profile.wizard.current_health / profile.wizard.maximum_health),
    );
    player.combat.current_mana = fround(
        derived.maximum_mana * fround(profile.wizard.current_mana / profile.wizard.maximum_mana),
    );
    player.combat.maximum_health = derived.maximum_health;
    player.combat.maximum_mana = derived.maximum_mana;
    progression.deferred_skill_choices = profile.wizard.deferred_skill_choices as usize;
    progression.experience = profile.wizard.experience;
    progression.level = profile.wizard.level as u32;
    progression.next_threshold = profile.wizard.next_threshold;
    progression.offer_seed = profile.wizard.offer_seed as u32;
    progression.pending_levels =
        vec![progression.level; profile.wizard.pending_skill_choices as usize];
    progression.poison_immunity_ticks_remaining = profile.wizard.poison_immunity_ticks as u64;
    progression.previous_threshold = profile.wizard.previous_threshold;
    progression.revision += 1;
    player.belt = belt(profile, &book);
    player.book = book;
    player.skill_runtime = runtime;
    player.stat_book = stat_book;
    player.progression = progression;
    let imported_primary = player.book.primary_skill_id;
    let imported_belt = player.belt.skill_projection();
    let imported_concentrations = [
        player.skill_runtime.concentration_skill_id_a,
        player.skill_runtime.concentration_skill_id_b,
    ];
    let pending_count = profile.wizard.pending_skill_choices;
    if pending_count > 0 {
        state.level_up_barrier = Some(PlayerLevelUpBarrier {
            barrier_id: 1,
            milestone_experience: profile.wizard.experience,
            milestone_level: profile.wizard.level as u32,
            participant_ids: vec![player_id.clone()],
            pending_player_ids: vec![player_id.clone()],
            run_id: None,
            source_player_id: player_id.clone(),
        });
        state.next_level_up_barrier_id = 2;
    }
    state.secondary.players.insert(
        player_id.clone(),
        NativeSecondaryPlayerState {
            firewalker: profile.wizard.firewalker_active,
            reserved_mana: if profile.wizard.firewalker_active { 50.0 } else { 0.0 },
            ..NativeSecondaryPlayerState::default()
        },
    );
    let mut warnings = profile.warnings.clone();
    if profile.profile.native_storage.payload_length != 4
        || profile.profile.native_storage.child_count != 0
    {
        warnings.push("Native Luthacus storage remains in the source attachment and is not available during web play.".into());
    }
    if profile
        .wizard
        .advanced_unlocks
        .iter()
        .enumerate()
        .any(|(index, unlocked)| *unlocked && profile.wizard.permanent_ranks[index + 72] == 0)
    {
        warnings.push("Purchased-but-unlearned advanced spells are a web retention extension and cannot be reconstructed from retail disk bytes.".into());
    }
    if imported_primary != profile.wizard.selected_primary_skill_id as usize {
        warnings.push("Stock selected a transient Plane Orb or a Weld whose build identity is not on disk; web play starts on the creation-element primary.".into());
    }
    if profile
        .wizard
        .skill_quickbar
        .iter()
        .enumerate()
        .any(|(slot, skill)| skill.map(|id| id as usize) != imported_belt[slot])
    {
        warnings.push("Native Belt entries that depend on an unpersisted Weld build or non-durable effective rank remain in the native attachment but are unavailable in web play.".into());
    }
    if profile
        .wizard
        .concentration_skill_ids
        .iter()
        .enumerate()
        .any(|(slot, skill)| skill.map(|id| id as usize) != imported_concentrations[slot])
    {
        warnings.push("Native concentrations backed only by non-materialized equipment state reset during web import.".into());
    }
    let metadata = SaveMetadata {
        integrity: SaveIntegrity::LocalOnly,
        mods: Vec::new(),
        mod_state: LuaObject::new(),
        native_source: Some(profile.native_source.clone()),
    };
    let document = website_save::create(CreateSaveOptions {
        state: &state,
        player_id: &player_id,
        loaded_boneyard: None,
        metadata: &metadata,
        party_rejoin_token: None,
    })?;
    Ok(PortableImportResult { character, document, player_id, warnings })
}
