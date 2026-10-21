//! Known disk fields are replaced in the original native tree; opaque siblings survive.

use super::*;
use crate::native_save::codec::{encode_native_darkdata, encode_native_sync_buffer};

fn set_i32(bytes: &mut [u8], offset: usize, value: i32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn set_f32(bytes: &mut [u8], offset: usize, value: f64) {
    bytes[offset..offset + 4].copy_from_slice(&(value as f32).to_le_bytes());
}

fn native_string_bytes(value: &str) -> Vec<u8> {
    if value.is_empty() {
        return 0_u32.to_le_bytes().to_vec();
    }
    let mut output = ((value.len() + 1) as u32).to_le_bytes().to_vec();
    output.extend_from_slice(value.as_bytes());
    output.push(0);
    output
}

pub fn patch_native_darkdata(source: &[u8], patch: &NativeProfilePatch) -> Result<Vec<u8>, String> {
    let decoded = decode_native_darkdata_profile(source)?;
    if patch.help_pending.len() != 10 {
        return Err("native help state must contain 10 booleans".into());
    }
    if patch.first_mixed.len() != NATIVE_FIRST_MIX_COUNT {
        return Err(format!(
            "native first-mix state must contain {NATIVE_FIRST_MIX_COUNT} booleans"
        ));
    }
    if !native_hagatha_bundle_state_is_valid(&patch.hagatha_bundle_selectors) {
        return Err("native Hagatha bundle is invalid".into());
    }
    let mut buffer = decoded.buffer;
    let children = &mut buffer.root.children;
    let core = &mut children[0].payload;
    set_i32(core, 0, patch.gold);
    core[14] = u8::from(patch.tutorial_pending);
    for (target, &value) in core[15..25].iter_mut().zip(&patch.help_pending) {
        *target = u8::from(value);
    }
    core[117] = u8::from(patch.librarian_lace_read);
    let mut selectors = (patch.hagatha_bundle_selectors.len() as u32).to_le_bytes().to_vec();
    for &selector in &patch.hagatha_bundle_selectors {
        selectors.extend_from_slice(&(selector as i32).to_le_bytes());
    }
    children[2].payload = selectors;
    children[3].payload = patch.first_mixed.iter().copied().map(u8::from).collect();
    children[4].payload = patch.dowsing_fee.to_le_bytes().to_vec();
    let output = encode_native_darkdata(&buffer)?;
    decode_native_darkdata_profile(&output)?;
    Ok(output)
}

struct ProgressionOffsets {
    pending_skill_choices: usize,
    level: usize,
    experience: usize,
    previous_threshold: usize,
    next_threshold: usize,
    current_health: usize,
    maximum_health: usize,
    current_mana: usize,
    maximum_mana: usize,
    element_root: usize,
    discipline_root: usize,
    offer_seed: usize,
    deferred_skill_choices: usize,
    starting_secondary: usize,
    offensive_damage_flat: usize,
    mana_cost_reduction: usize,
    experience_bonus: usize,
    starting_primary: usize,
    cheat_death_enabled: usize,
    cheat_death_charges: usize,
    experience_enabled: usize,
    random_boast_active: usize,
    perk_capacity: usize,
    poison_immunity_ticks: usize,
}

impl PayloadReader<'_> {
    fn mark_i32(&mut self, field: &str) -> Result<usize, String> {
        let offset = self.offset;
        self.i32(field)?;
        Ok(offset)
    }

    fn mark_f32(&mut self, field: &str) -> Result<usize, String> {
        let offset = self.offset;
        self.f32(field)?;
        Ok(offset)
    }

    fn mark_boolean(&mut self, field: &str) -> Result<usize, String> {
        let offset = self.offset;
        self.boolean(field)?;
        Ok(offset)
    }

    fn skip_words(&mut self, count: u32, field: &str) -> Result<(), String> {
        // Native counts are u32; check before multiplying on the 32-bit WASM target.
        if count as usize > (self.bytes.len() - self.offset) / 4 {
            return Err(format!("truncated {} {field} at 0x{:x}", self.claim, self.offset));
        }
        self.skip(count as usize * 4, field)
    }
}

fn progression_offsets(payload: &[u8]) -> Result<ProgressionOffsets, String> {
    let mut reader = PayloadReader::new(payload, "native progression offset walk");
    if reader.u32("row count")? as usize != NATIVE_WIZARD_SKILL_ROW_COUNT {
        return Err("native progression row count drifted".into());
    }
    reader.skip(NATIVE_WIZARD_SKILL_ROW_COUNT * 12, "rows")?;
    let pending_skill_choices = reader.mark_i32("pendingSkillChoices")?;
    let level = reader.mark_i32("level")?;
    let experience = reader.mark_f32("experience")?;
    let previous_threshold = reader.mark_f32("previousThreshold")?;
    let next_threshold = reader.mark_f32("nextThreshold")?;
    reader.skip(8, "global cooldown and unknown scalar")?;
    let current_health = reader.mark_f32("currentHealth")?;
    let maximum_health = reader.mark_f32("maximumHealth")?;
    let current_mana = reader.mark_f32("currentMana")?;
    let maximum_mana = reader.mark_f32("maximumMana")?;
    reader.skip(52, "common scalar prefix remainder")?;
    let count = reader.u32("integer vector count")?;
    reader.skip_words(count, "integer vector")?;
    reader.skip(36, "inline scalar/vector prefix")?;
    reader.skip(24, "unknown/offensive block")?;
    let element_root = reader.mark_i32("elementRoot")?;
    let discipline_root = reader.mark_i32("disciplineRoot")?;
    let offer_seed = reader.mark_i32("offerSeed")?;
    reader.skip(9, "offer marker fields")?;
    let deferred_skill_choices = reader.mark_i32("deferredSkillChoices")?;
    reader.skip(1, "Sorceror action flag")?;
    let starting_secondary = reader.mark_i32("startingSecondary")?;
    let count = reader.u32("forced-offer count")?;
    reader.skip_words(count, "forced offers")?;
    let offensive_damage_flat = reader.mark_f32("offensiveDamageFlat")?;
    let mana_cost_reduction = reader.mark_f32("manaCostReduction")?;
    let experience_bonus = reader.mark_f32("experienceBonus")?;
    reader.skip(20, "late scalar block")?;
    reader.skip(1, "flag +0x814")?;
    let starting_primary = reader.mark_i32("startingPrimary")?;
    let cheat_death_enabled = reader.mark_boolean("cheatDeathEnabled")?;
    let cheat_death_charges = reader.mark_i32("cheatDeathCharges")?;
    reader.skip(4, "hoarded mana")?;
    let experience_enabled = reader.mark_boolean("experienceEnabled")?;
    let random_boast_active = reader.mark_boolean("randomBoastActive")?;
    let perk_capacity = reader.mark_i32("perkCapacity")?;
    let poison_immunity_ticks = reader.mark_i32("poisonImmunityTicks")?;
    reader.finish()?;
    Ok(ProgressionOffsets {
        pending_skill_choices,
        level,
        experience,
        previous_threshold,
        next_threshold,
        current_health,
        maximum_health,
        current_mana,
        maximum_mana,
        element_root,
        discipline_root,
        offer_seed,
        deferred_skill_choices,
        starting_secondary,
        offensive_damage_flat,
        mana_cost_reduction,
        experience_bonus,
        starting_primary,
        cheat_death_enabled,
        cheat_death_charges,
        experience_enabled,
        random_boast_active,
        perk_capacity,
        poison_immunity_ticks,
    })
}

fn patch_selections(root: &mut NativeChunkNode, patch: &NativeWizardPatch) -> Result<(), String> {
    if !(8..=79).contains(&patch.selected_primary_skill_id)
        || patch.concentration_skill_ids.iter().flatten().any(|id| !(8..=79).contains(id))
        || patch.skill_quickbar.len() != 8
        || patch.next_concentration_slot < 0
        || patch.next_concentration_slot > 1
        || patch.skill_quickbar.iter().flatten().any(|id| !(8..=79).contains(id))
    {
        return Err("native selection patch is invalid".into());
    }
    let bindings = binding_state(root)?;
    let offset = bindings.integer_offset;
    let mut payload = bindings.node.payload.clone();
    set_i32(&mut payload, offset + 12 * 4, patch.selected_primary_skill_id);
    set_i32(&mut payload, offset + 16 * 4, patch.concentration_skill_ids[0].unwrap_or(-1));
    set_i32(&mut payload, offset + 20 * 4, patch.concentration_skill_ids[1].unwrap_or(-1));
    root.children[1].children[0].payload = payload;
    let (entries, _) = belt(root)?;
    let mut payload = Vec::new();
    for (slot, entry) in entries.iter().enumerate() {
        if let Some(id) = patch.skill_quickbar[slot] {
            canonical_belt_entry(&mut payload, 7015, id);
        } else if entry.kind == 7015 {
            canonical_belt_entry(&mut payload, 7000, 0);
        } else {
            payload.extend_from_slice(&root.payload[entry.start..entry.end]);
        }
    }
    root.payload = payload;
    let (offset, _) = footer(root)?;
    set_i32(&mut root.children[7].payload, offset, patch.next_concentration_slot);
    Ok(())
}

fn canonical_belt_entry(output: &mut Vec<u8>, kind: i32, id: i32) {
    output.extend_from_slice(&kind.to_le_bytes());
    output.extend_from_slice(&id.to_le_bytes());
    output.push(0);
    output.extend_from_slice(&0_u32.to_le_bytes());
    output.extend_from_slice(&0_i32.to_le_bytes());
}

fn patch_game_state(
    node: &mut NativeChunkNode,
    boast: &NativeGameBoastState,
) -> Result<(), String> {
    if (boast.selected.is_none() && (boast.failed || boast.succeeded))
        || (boast.selected == Some(3) && boast.failed)
        || (boast.failed && boast.succeeded)
    {
        return Err("native Boast patch is inconsistent".into());
    }
    let layout = game_state_layout(node)?;
    let mut tail = node.payload[layout.path.end..].to_vec();
    tail[layout.failure_offset - layout.path.end] = u8::from(boast.failed);
    tail[layout.success_offset - layout.path.end] = u8::from(boast.succeeded);
    let mut payload = node.payload[..layout.selected_offset].to_vec();
    payload.push(boast.selected.map_or(0xff, |id| id as u8));
    payload.extend_from_slice(&native_string_bytes(boast_statement(boast.selected)?));
    payload.extend_from_slice(&node.payload[layout.boast_text.end..layout.bridge_text.end]);
    payload.extend_from_slice(&native_string_bytes("data\\levels\\survival.boneyard"));
    payload.extend_from_slice(&tail);
    node.payload = payload;
    Ok(())
}

pub fn patch_native_gamestate(
    source: &[u8],
    patch: &NativeWizardPatch,
    boast: Option<&NativeGameBoastState>,
) -> Result<Vec<u8>, String> {
    if patch.permanent_ranks.len() != NATIVE_WIZARD_SKILL_ROW_COUNT
        || patch.permanent_ranks.iter().any(|rank| *rank > u16::MAX as u32)
    {
        return Err("native permanent ranks is invalid".into());
    }
    if patch.learned_order.iter().any(|id| !(8..=79).contains(id)) {
        return Err("native learned order is invalid".into());
    }
    let tonics = patch.perk_selectors.iter().filter(|value| **value == 27).count() as u32;
    if u32::try_from(patch.perk_capacity).ok().is_none_or(|capacity| {
        !native_hagatha_outcome_state_is_valid(&patch.perk_selectors, tonics, capacity)
    }) {
        return Err("native Hagatha outcomes are invalid".into());
    }
    if patch.hagatha_ownership.len() != NATIVE_HAGATHA_OWNERSHIP_COUNT {
        return Err(format!(
            "native Hagatha ownership must contain {NATIVE_HAGATHA_OWNERSHIP_COUNT} booleans"
        ));
    }
    let mut name = patch.name.as_bytes().to_vec();
    name.push(0);
    if !(2..=256).contains(&name.len()) {
        return Err("native wizard name is invalid".into());
    }
    let mut buffer = parse_native_sync_buffer(source)?;
    let local = local_wizard(&buffer)?;
    let mut game_node = buffer.root.children[5].clone();
    let retained_boast = if boast.is_none() { Some(game_boast(&game_node)?) } else { None };
    let boast =
        boast.or(retained_boast.as_ref()).expect("one source or requested Boast is retained");
    let progression = &local.node.children[0];
    decode_progression(progression)?;
    let mut payload = progression.payload.clone();
    let offsets = progression_offsets(&payload)?;
    for (id, &rank) in patch.permanent_ranks.iter().enumerate() {
        let offset = 4 + (NATIVE_WIZARD_SKILL_ROW_COUNT - 1 - id) * 12;
        payload[offset..offset + 2].copy_from_slice(&(rank as u16).to_le_bytes());
        payload[offset + 2..offset + 4].copy_from_slice(&(rank as u16).to_le_bytes());
    }
    for (offset, value) in [
        (offsets.pending_skill_choices, patch.pending_skill_choices),
        (offsets.level, patch.level),
        (offsets.element_root, patch.element_root),
        (offsets.discipline_root, patch.discipline_root),
        (offsets.offer_seed, patch.offer_seed),
        (offsets.deferred_skill_choices, patch.deferred_skill_choices),
        (offsets.starting_secondary, patch.starting_secondary),
        (offsets.starting_primary, patch.starting_primary),
        (offsets.cheat_death_charges, patch.cheat_death_charges),
        (offsets.perk_capacity, patch.perk_capacity),
        (offsets.poison_immunity_ticks, patch.poison_immunity_ticks),
    ] {
        set_i32(&mut payload, offset, value);
    }
    for (offset, value) in [
        (offsets.experience, patch.experience),
        (offsets.previous_threshold, patch.previous_threshold),
        (offsets.next_threshold, patch.next_threshold),
        (offsets.current_health, patch.current_health),
        (offsets.maximum_health, patch.maximum_health),
        (offsets.current_mana, patch.current_mana),
        (offsets.maximum_mana, patch.maximum_mana),
        (offsets.offensive_damage_flat, patch.offensive_damage_flat),
        (offsets.mana_cost_reduction, patch.mana_cost_reduction),
        (offsets.experience_bonus, patch.experience_bonus),
    ] {
        set_f32(&mut payload, offset, value);
    }
    payload[offsets.cheat_death_enabled] = u8::from(patch.cheat_death_enabled);
    payload[offsets.experience_enabled] = 1;
    payload[offsets.random_boast_active] = u8::from(boast.selected == Some(3));
    let mut next_progression = progression.clone();
    next_progression.payload = payload;
    let mut collections = (patch.perk_selectors.len() as u32).to_le_bytes().to_vec();
    for &selector in &patch.perk_selectors {
        collections.extend_from_slice(&(selector as i32).to_le_bytes());
    }
    collections.extend(patch.hagatha_ownership.iter().copied().map(u8::from));
    collections.extend_from_slice(&(patch.learned_order.len() as u32).to_le_bytes());
    for &id in &patch.learned_order {
        collections.extend_from_slice(&id.to_le_bytes());
    }
    next_progression.children[1].payload = collections;
    let mut next_wizard = local.node.clone();
    wizard_extension(&next_wizard.children[1])?;
    let extension = &mut next_wizard.children[1].payload;
    set_i32(extension, 0, patch.meditation_idle_delay);
    extension[4] = u8::from(patch.firewalker_active);
    set_f32(extension, 5, patch.weld_effect);
    let mut header = local.header_a.to_le_bytes().to_vec();
    header.extend_from_slice(&local.header_b.to_le_bytes());
    header.extend_from_slice(&(name.len() as u32).to_le_bytes());
    header.extend_from_slice(&name);
    header.extend_from_slice(&patch.selected_primary_skill_id.to_le_bytes());
    next_wizard.payload = header;
    next_wizard.children[0] = next_progression;
    buffer.root.children[0] = next_wizard;
    patch_selections(&mut buffer.root, patch)?;
    patch_game_state(&mut game_node, boast)?;
    buffer.root.children[5] = game_node;
    let output = encode_native_sync_buffer(&buffer)?;
    decode_native_gamestate_wizard(&output)?;
    decode_native_gamestate_boast(&output)?;
    Ok(output)
}
