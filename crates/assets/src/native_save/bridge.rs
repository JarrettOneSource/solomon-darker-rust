//! Native profile/wizard disk fields from Website `save/native-save-bridge.ts`.
//! The tree retains every unknown sibling; only named disk members are interpreted.

mod patch;

pub use patch::{patch_native_darkdata, patch_native_gamestate};

use serde::{Deserialize, Serialize};
use solomon_sim::economy::{
    native_hagatha_bundle_state_is_valid, native_hagatha_outcome_state_is_valid,
};

use super::codec::{
    NativeChunkNode, NativeSyncBuffer, decode_native_darkdata, parse_native_sync_buffer, utf8,
};

pub const RETAIL_SOLOMON_DARK_SHA256: &str =
    "03a834566ce70fd8088f4cf9ee6693157130d8aec28c092cb814d6221231f1e3";
pub const NATIVE_WIZARD_SKILL_ROW_COUNT: usize = 83;
pub const NATIVE_HAGATHA_OWNERSHIP_COUNT: usize = 50;
pub const NATIVE_FIRST_MIX_COUNT: usize = 30;
pub const NATIVE_SOURCE_ATTACHMENT_MAX_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeDarkdataProfile {
    #[serde(skip)]
    pub buffer: NativeSyncBuffer,
    pub dowsing_fee: i32,
    pub first_mixed: Vec<bool>,
    pub gold: i32,
    pub hagatha_bundle_selectors: Vec<u32>,
    pub help_pending: Vec<bool>,
    pub librarian_lace_read: bool,
    pub memorial_marker: Vec<bool>,
    pub storage_child_count: usize,
    pub storage_payload_length: usize,
    pub tutorial_pending: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeProgressionRow {
    pub cooldown_cap: f64,
    pub current_cooldown: f64,
    pub effective_rank: u16,
    pub id: usize,
    pub permanent_rank: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeGameBoastState {
    pub failed: bool,
    pub selected: Option<u32>,
    pub succeeded: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeProfilePatch {
    pub dowsing_fee: i32,
    pub first_mixed: Vec<bool>,
    pub gold: i32,
    pub hagatha_bundle_selectors: Vec<u32>,
    pub help_pending: Vec<bool>,
    pub librarian_lace_read: bool,
    pub tutorial_pending: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeWizardProgression {
    pub cheat_death_charges: i32,
    pub cheat_death_enabled: bool,
    pub current_health: f64,
    pub current_mana: f64,
    pub deferred_skill_choices: i32,
    pub discipline_root: i32,
    pub element_root: i32,
    pub experience: f64,
    pub experience_bonus: f64,
    pub experience_enabled: bool,
    pub firewalker_active: bool,
    pub hagatha_ownership: Vec<bool>,
    pub learned_order: Vec<i32>,
    pub level: i32,
    pub mana_cost_reduction: f64,
    pub maximum_health: f64,
    pub maximum_mana: f64,
    pub meditation_idle_delay: i32,
    pub name: String,
    pub next_concentration_slot: i32,
    pub offer_seed: i32,
    pub offensive_damage_flat: f64,
    pub pending_skill_choices: i32,
    pub perk_capacity: i32,
    pub perk_selectors: Vec<u32>,
    pub poison_immunity_ticks: i32,
    pub previous_threshold: f64,
    pub next_threshold: f64,
    pub rows: Vec<NativeProgressionRow>,
    pub random_boast_active: bool,
    pub selected_primary_skill_id: i32,
    pub concentration_skill_ids: [Option<i32>; 2],
    pub skill_quickbar: Vec<Option<i32>>,
    pub starting_primary: i32,
    pub starting_secondary: i32,
    pub weld_effect: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeWizardPatch {
    pub cheat_death_charges: i32,
    pub cheat_death_enabled: bool,
    pub current_health: f64,
    pub current_mana: f64,
    pub deferred_skill_choices: i32,
    pub discipline_root: i32,
    pub element_root: i32,
    pub experience: f64,
    pub experience_bonus: f64,
    pub firewalker_active: bool,
    pub hagatha_ownership: Vec<bool>,
    pub learned_order: Vec<i32>,
    pub level: i32,
    pub mana_cost_reduction: f64,
    pub maximum_health: f64,
    pub maximum_mana: f64,
    pub meditation_idle_delay: i32,
    pub name: String,
    pub next_concentration_slot: i32,
    pub next_threshold: f64,
    pub offer_seed: i32,
    pub offensive_damage_flat: f64,
    pub pending_skill_choices: i32,
    pub perk_capacity: i32,
    pub perk_selectors: Vec<u32>,
    pub permanent_ranks: Vec<u32>,
    pub poison_immunity_ticks: i32,
    pub previous_threshold: f64,
    pub selected_primary_skill_id: i32,
    pub concentration_skill_ids: [Option<i32>; 2],
    pub skill_quickbar: Vec<Option<i32>>,
    pub starting_primary: i32,
    pub starting_secondary: i32,
    pub weld_effect: f64,
}

struct PayloadReader<'a> {
    bytes: &'a [u8],
    claim: &'static str,
    offset: usize,
}

impl<'a> PayloadReader<'a> {
    fn new(bytes: &'a [u8], claim: &'static str) -> Self {
        Self { bytes, claim, offset: 0 }
    }

    fn take(&mut self, size: usize, field: &str) -> Result<&'a [u8], String> {
        if size > self.bytes.len() - self.offset {
            return Err(format!("truncated {} {field} at 0x{:x}", self.claim, self.offset));
        }
        let result = &self.bytes[self.offset..self.offset + size];
        self.offset += size;
        Ok(result)
    }

    fn skip(&mut self, size: usize, field: &str) -> Result<(), String> {
        self.take(size, field)?;
        Ok(())
    }

    fn u8(&mut self, field: &str) -> Result<u8, String> {
        Ok(self.take(1, field)?[0])
    }

    fn boolean(&mut self, field: &str) -> Result<bool, String> {
        let value = self.u8(field)?;
        if value > 1 {
            return Err(format!("{} {field} contains byte {value}", self.claim));
        }
        Ok(value == 1)
    }

    fn u16(&mut self, field: &str) -> Result<u16, String> {
        let bytes = self.take(2, field)?;
        Ok(u16::from_le_bytes(bytes.try_into().expect("the reader checked two bytes")))
    }

    fn u32(&mut self, field: &str) -> Result<u32, String> {
        let bytes = self.take(4, field)?;
        Ok(u32::from_le_bytes(bytes.try_into().expect("the reader checked four bytes")))
    }

    fn i32(&mut self, field: &str) -> Result<i32, String> {
        Ok(self.u32(field)? as i32)
    }

    fn f32(&mut self, field: &str) -> Result<f64, String> {
        Ok(f64::from(f32::from_bits(self.u32(field)?)))
    }

    fn string(&mut self, field: &str) -> Result<String, String> {
        let length = self.u32(&format!("{field} length"))? as usize;
        if length == 0 {
            return Ok(String::new());
        }
        let raw = self.take(length, field)?;
        if raw.last() != Some(&0) {
            return Err(format!("{} {field} is not NUL-terminated", self.claim));
        }
        utf8(&raw[..raw.len() - 1])
            .map(str::to_owned)
            .map_err(|_| format!("{} {field} is not UTF-8", self.claim))
    }

    fn i32s(&mut self, count: usize, field: &str) -> Result<Vec<i32>, String> {
        if count > (self.bytes.len() - self.offset) / 4 {
            return Err(format!("{} {field} count {count} is impossible", self.claim));
        }
        let mut result = Vec::with_capacity(count);
        for index in 0..count {
            result.push(self.i32(&format!("{field}[{index}]"))?);
        }
        Ok(result)
    }

    fn f32s(&mut self, count: usize, field: &str) -> Result<Vec<f64>, String> {
        if count > (self.bytes.len() - self.offset) / 4 {
            return Err(format!("{} {field} count {count} is impossible", self.claim));
        }
        let mut result = Vec::with_capacity(count);
        for index in 0..count {
            result.push(self.f32(&format!("{field}[{index}]"))?);
        }
        Ok(result)
    }

    fn booleans(&mut self, count: usize, field: &str) -> Result<Vec<bool>, String> {
        let mut result = Vec::with_capacity(count);
        for index in 0..count {
            result.push(self.boolean(&format!("{field} {index}"))?);
        }
        Ok(result)
    }

    fn finish(self) -> Result<(), String> {
        if self.offset != self.bytes.len() {
            return Err(format!(
                "{} ended at 0x{:x} with {} unclaimed bytes",
                self.claim,
                self.offset,
                self.bytes.len() - self.offset
            ));
        }
        Ok(())
    }
}

fn require_shape(
    node: &NativeChunkNode,
    length: Option<usize>,
    children: Option<usize>,
    claim: &str,
) -> Result<(), String> {
    if let Some(length) = length
        && node.payload.len() != length
    {
        return Err(format!("{claim} payload is {} bytes; expected {length}", node.payload.len()));
    }
    if let Some(count) = children
        && node.children.len() != count
    {
        return Err(format!("{claim} has {} children; expected {count}", node.children.len()));
    }
    Ok(())
}

fn selectors(values: Vec<i32>, claim: &str) -> Result<Vec<u32>, String> {
    values.into_iter().map(|value| u32::try_from(value).map_err(|_| claim.to_owned())).collect()
}

pub fn decode_native_darkdata_profile(bytes: &[u8]) -> Result<NativeDarkdataProfile, String> {
    let buffer = decode_native_darkdata(bytes)?;
    if !buffer.named_buffers.is_empty() {
        return Err("darkdata unexpectedly has named buffers".into());
    }
    require_shape(&buffer.root, Some(0), Some(6), "darkdata root")?;
    let children = &buffer.root.children;
    require_shape(&children[0], Some(118), None, "darkdata core")?;
    require_shape(&children[3], Some(NATIVE_FIRST_MIX_COUNT), None, "darkdata first-mix state")?;
    require_shape(&children[4], Some(4), None, "darkdata Dowsing fee")?;
    require_shape(&children[5], Some(0), Some(0), "darkdata reserved child")?;
    let mut core = PayloadReader::new(&children[0].payload, "darkdata core");
    let gold = core.i32("profile gold")?;
    let memorial_marker = core.booleans(10, "memorial marker")?;
    let tutorial_pending = core.boolean("Tutorial pending")?;
    let help_pending = core.booleans(10, "Hub help pending")?;
    core.skip(40, "Memoratorium ages")?;
    core.skip(4, "Memoratorium age counter")?;
    core.skip(40, "Memoratorium portrait ids")?;
    core.skip(4, "next portrait id")?;
    core.skip(4, "latest portrait id")?;
    let librarian_lace_read = core.boolean("Lace read")?;
    core.finish()?;
    let mut bulk = PayloadReader::new(&children[2].payload, "darkdata Hagatha bundle");
    let count = bulk.u32("count")? as usize;
    let raw_selectors = bulk.i32s(count, "selectors")?;
    bulk.finish()?;
    let hagatha_bundle_selectors = selectors(raw_selectors, "native Hagatha bundle is invalid")?;
    if !native_hagatha_bundle_state_is_valid(&hagatha_bundle_selectors) {
        return Err("native Hagatha bundle is invalid".into());
    }
    let mut mix = PayloadReader::new(&children[3].payload, "darkdata first-mix state");
    let first_mixed = mix.booleans(NATIVE_FIRST_MIX_COUNT, "selector")?;
    mix.finish()?;
    let mut fee = PayloadReader::new(&children[4].payload, "darkdata Dowsing fee");
    let dowsing_fee = fee.i32("fee")?;
    fee.finish()?;
    Ok(NativeDarkdataProfile {
        dowsing_fee,
        first_mixed,
        gold,
        hagatha_bundle_selectors,
        help_pending,
        librarian_lace_read,
        memorial_marker,
        storage_child_count: children[1].children.len(),
        storage_payload_length: children[1].payload.len(),
        tutorial_pending,
        buffer,
    })
}

struct LocalWizard<'a> {
    node: &'a NativeChunkNode,
    header_a: i32,
    header_b: i32,
    name: String,
    selected_primary_skill_id: i32,
}

fn local_wizard(buffer: &NativeSyncBuffer) -> Result<LocalWizard<'_>, String> {
    if !buffer.named_buffers.is_empty() {
        return Err("native gamestate unexpectedly has named buffers".into());
    }
    require_shape(&buffer.root, None, Some(8), "native gamestate root")?;
    let node = &buffer.root.children[0];
    if node.children.len() < 2 {
        return Err("native local wizard is missing progression children".into());
    }
    let mut reader = PayloadReader::new(&node.payload, "native local-wizard header");
    let header_a = reader.i32("header A")?;
    let header_b = reader.i32("header B")?;
    let length = reader.u32("name length")? as usize;
    if !(2..=256).contains(&length) {
        return Err(format!("native wizard name length {length} is invalid"));
    }
    let raw = reader.take(length, "name")?;
    if raw.last() != Some(&0) {
        return Err("native wizard name is not NUL-terminated".into());
    }
    let name =
        utf8(&raw[..raw.len() - 1]).map_err(|_| "native wizard name is not UTF-8")?.to_owned();
    if name.is_empty() {
        return Err("native wizard name is empty".into());
    }
    let selected_primary_skill_id = reader.i32("selected primary skill")?;
    reader.finish()?;
    Ok(LocalWizard { node, header_a, header_b, name, selected_primary_skill_id })
}

struct BindingState<'a> {
    node: &'a NativeChunkNode,
    integer_offset: usize,
    values: Vec<i32>,
}

fn binding_state(root: &NativeChunkNode) -> Result<BindingState<'_>, String> {
    let node = root
        .children
        .get(1)
        .and_then(|owner| owner.children.first())
        .ok_or("native binding state is absent")?;
    let mut reader = PayloadReader::new(&node.payload, "native binding state");
    let count = reader.u32("boolean count")? as usize;
    if count > node.payload.len() - reader.offset {
        return Err("native binding boolean count is impossible".into());
    }
    reader.booleans(count, "boolean")?;
    let count = reader.u32("integer count")? as usize;
    if count < 21 {
        return Err(format!(
            "native binding state has {count} integers; portable fields require at least 21"
        ));
    }
    let integer_offset = reader.offset;
    let values = reader.i32s(count, "integer")?;
    let count = reader.u32("float count")? as usize;
    reader.f32s(count, "float")?;
    let count = reader.u32("String count")? as usize;
    if count > (node.payload.len() - reader.offset) / 4 {
        return Err("native binding String count is impossible".into());
    }
    for index in 0..count {
        reader.string(&format!("String {index}"))?;
    }
    for kind in ["vector2", "range"] {
        let count = reader.u32(&format!("{kind} count"))? as usize;
        if count > (node.payload.len() - reader.offset) / 8 {
            return Err(format!("native binding {kind} count is impossible"));
        }
        reader.skip(count * 8, kind)?;
    }
    reader.finish()?;
    Ok(BindingState { node, integer_offset, values })
}

struct BeltEntry {
    start: usize,
    end: usize,
    kind: i32,
}

fn belt(root: &NativeChunkNode) -> Result<(Vec<BeltEntry>, Vec<Option<i32>>), String> {
    let mut reader = PayloadReader::new(&root.payload, "native BeltButton state");
    let mut entries = Vec::with_capacity(8);
    let mut quickbar = Vec::with_capacity(8);
    for slot in 0..8 {
        let start = reader.offset;
        let kind = reader.i32(&format!("slot {slot} type"))?;
        let id = reader.i32(&format!("slot {slot} id"))?;
        reader.boolean(&format!("slot {slot} flag"))?;
        reader.string(&format!("slot {slot} label"))?;
        reader.i32(&format!("slot {slot} trailing value"))?;
        let end = reader.offset;
        if kind == 7015 && !(8..=79).contains(&id) {
            return Err(format!("native BeltButton slot {slot} has invalid skill {id}"));
        }
        entries.push(BeltEntry { start, end, kind });
        quickbar.push(if kind == 7015 { Some(id) } else { None });
    }
    reader.finish()?;
    Ok((entries, quickbar))
}

fn nullable_binding(value: i32, claim: &str) -> Result<Option<i32>, String> {
    if value == -1 {
        return Ok(None);
    }
    if !(8..=80).contains(&value) {
        return Err(format!("{claim} {value} is invalid"));
    }
    Ok(Some(value))
}

fn footer(root: &NativeChunkNode) -> Result<(usize, i32), String> {
    let node = root.children.get(7).ok_or("native Game footer is absent")?;
    let mut reader = PayloadReader::new(&node.payload, "native Game footer");
    reader.u8("global selector byte")?;
    let offset = reader.offset;
    let cursor = reader.i32("concentration replacement cursor")?;
    if cursor != 0 && cursor != 1 {
        return Err(format!("native concentration replacement cursor {cursor} is invalid"));
    }
    reader.string("profile label A")?;
    reader.string("profile label B")?;
    reader.f32("presentation scalar")?;
    reader.finish()?;
    Ok((offset, cursor))
}

struct ProgressionState {
    cheat_death_charges: i32,
    cheat_death_enabled: bool,
    current_health: f64,
    current_mana: f64,
    deferred_skill_choices: i32,
    discipline_root: i32,
    element_root: i32,
    experience: f64,
    experience_bonus: f64,
    experience_enabled: bool,
    hagatha_ownership: Vec<bool>,
    learned_order: Vec<i32>,
    level: i32,
    mana_cost_reduction: f64,
    maximum_health: f64,
    maximum_mana: f64,
    next_threshold: f64,
    offer_seed: i32,
    offensive_damage_flat: f64,
    pending_skill_choices: i32,
    perk_capacity: i32,
    perk_selectors: Vec<i32>,
    poison_immunity_ticks: i32,
    previous_threshold: f64,
    random_boast_active: bool,
    rows: Vec<NativeProgressionRow>,
    starting_primary: i32,
    starting_secondary: i32,
}

fn decode_progression(node: &NativeChunkNode) -> Result<ProgressionState, String> {
    require_shape(node, None, Some(2), "native progression")?;
    let mut reader = PayloadReader::new(&node.payload, "native progression");
    let count = reader.u32("row count")? as usize;
    if count != NATIVE_WIZARD_SKILL_ROW_COUNT {
        return Err(format!(
            "native progression has {count} rows; expected {NATIVE_WIZARD_SKILL_ROW_COUNT}"
        ));
    }
    let mut rows = Vec::with_capacity(count);
    for id in (0..count).rev() {
        let permanent_rank = reader.u16(&format!("row {id} permanent rank"))?;
        let effective_rank = reader.u16(&format!("row {id} effective rank"))?;
        let current_cooldown = reader.f32(&format!("row {id} current cooldown"))?;
        let cooldown_cap = reader.f32(&format!("row {id} cooldown cap"))?;
        rows.push(NativeProgressionRow {
            cooldown_cap,
            current_cooldown,
            effective_rank,
            id,
            permanent_rank,
        });
    }
    rows.reverse();
    let pending_skill_choices = reader.i32("pending choices")?;
    let level = reader.i32("level")?;
    let experience = reader.f32("experience")?;
    let previous_threshold = reader.f32("previous threshold")?;
    let next_threshold = reader.f32("next threshold")?;
    reader.skip(8, "global cooldown and unknown scalar")?;
    let current_health = reader.f32("current health")?;
    let maximum_health = reader.f32("maximum health")?;
    let current_mana = reader.f32("current mana")?;
    let maximum_mana = reader.f32("maximum mana")?;
    reader.skip(52, "common scalar prefix remainder")?;
    let count = reader.u32("integer vector count")? as usize;
    reader.i32s(count, "integer vector")?;
    reader.skip(36, "inline scalar/vector prefix")?;
    reader.skip(24, "unknown/offensive scalar block")?;
    let element_root = reader.i32("element root")?;
    let discipline_root = reader.i32("discipline root")?;
    let offer_seed = reader.i32("offer seed")?;
    reader.skip(9, "offer marker state")?;
    let deferred_skill_choices = reader.i32("deferred choices")?;
    reader.boolean("Sorceror action flag")?;
    let starting_secondary = reader.i32("starting secondary")?;
    let count = reader.u32("forced-offer count")? as usize;
    reader.i32s(count, "forced offers")?;
    let offensive_damage_flat = reader.f32("offensive damage flat")?;
    let mana_cost_reduction = reader.f32("mana cost reduction")?;
    let experience_bonus = reader.f32("experience bonus")?;
    reader.skip(20, "late scalar block")?;
    reader.boolean("flag +0x814")?;
    let starting_primary = reader.i32("starting primary")?;
    let cheat_death_enabled = reader.boolean("cheat-death enabled")?;
    let cheat_death_charges = reader.i32("cheat-death charges")?;
    reader.skip(4, "hoarded mana")?;
    let experience_enabled = reader.boolean("local experience admission")?;
    let random_boast_active = reader.boolean("random-choice Boast")?;
    let perk_capacity = reader.i32("perk capacity")?;
    let poison_immunity_ticks = reader.i32("poison immunity ticks")?;
    reader.finish()?;
    let mut primary = PayloadReader::new(&node.children[0].payload, "primary-stat vector");
    let count = primary.u32("count")? as usize;
    primary.f32s(count, "values")?;
    primary.finish()?;
    let mut collections =
        PayloadReader::new(&node.children[1].payload, "native progression collections");
    let count = collections.u32("perk count")? as usize;
    let perk_selectors = collections.i32s(count, "perk selectors")?;
    let hagatha_ownership =
        collections.booleans(NATIVE_HAGATHA_OWNERSHIP_COUNT, "Hagatha ownership")?;
    let count = collections.u32("learned count")? as usize;
    let learned_order = collections.i32s(count, "learned order")?;
    collections.finish()?;
    Ok(ProgressionState {
        cheat_death_charges,
        cheat_death_enabled,
        current_health,
        current_mana,
        deferred_skill_choices,
        discipline_root,
        element_root,
        experience,
        experience_bonus,
        experience_enabled,
        hagatha_ownership,
        learned_order,
        level,
        mana_cost_reduction,
        maximum_health,
        maximum_mana,
        next_threshold,
        offer_seed,
        offensive_damage_flat,
        pending_skill_choices,
        perk_capacity,
        perk_selectors,
        poison_immunity_ticks,
        previous_threshold,
        random_boast_active,
        rows,
        starting_primary,
        starting_secondary,
    })
}

fn wizard_extension(node: &NativeChunkNode) -> Result<(i32, bool, f64), String> {
    require_shape(node, Some(9), Some(0), "native wizard disk extension")?;
    let mut reader = PayloadReader::new(&node.payload, "native wizard disk extension");
    let meditation_idle_delay = reader.i32("Meditation idle delay")?;
    let firewalker_active = reader.boolean("Firewalker active")?;
    let weld_effect = reader.f32("weld effect")?;
    reader.finish()?;
    Ok((meditation_idle_delay, firewalker_active, weld_effect))
}

pub fn decode_native_gamestate_wizard(bytes: &[u8]) -> Result<NativeWizardProgression, String> {
    let buffer = parse_native_sync_buffer(bytes)?;
    let local = local_wizard(&buffer)?;
    let progression = decode_progression(&local.node.children[0])?;
    let (meditation_idle_delay, firewalker_active, weld_effect) =
        wizard_extension(&local.node.children[1])?;
    let bindings = binding_state(&buffer.root)?;
    let selected_primary_skill_id =
        nullable_binding(bindings.values[12], "native selected primary")?;
    if selected_primary_skill_id != Some(local.selected_primary_skill_id) {
        return Err("native local-wizard selected primary bindings disagree".into());
    }
    let selected_primary_skill_id = local.selected_primary_skill_id;
    let concentration_skill_ids = [
        nullable_binding(bindings.values[16], "native concentration A")?,
        nullable_binding(bindings.values[20], "native concentration B")?,
    ];
    let (_, skill_quickbar) = belt(&buffer.root)?;
    let (_, next_concentration_slot) = footer(&buffer.root)?;
    let perk_selectors =
        selectors(progression.perk_selectors, "native Hagatha outcomes are invalid")?;
    let tonics = perk_selectors.iter().filter(|value| **value == 27).count() as u32;
    let capacity = u32::try_from(progression.perk_capacity).ok();
    if capacity.is_none_or(|capacity| {
        !native_hagatha_outcome_state_is_valid(&perk_selectors, tonics, capacity)
    }) {
        return Err("native Hagatha outcomes are invalid".into());
    }
    Ok(NativeWizardProgression {
        cheat_death_charges: progression.cheat_death_charges,
        cheat_death_enabled: progression.cheat_death_enabled,
        current_health: progression.current_health,
        current_mana: progression.current_mana,
        deferred_skill_choices: progression.deferred_skill_choices,
        discipline_root: progression.discipline_root,
        element_root: progression.element_root,
        experience: progression.experience,
        experience_bonus: progression.experience_bonus,
        experience_enabled: progression.experience_enabled,
        firewalker_active,
        hagatha_ownership: progression.hagatha_ownership,
        learned_order: progression.learned_order,
        level: progression.level,
        mana_cost_reduction: progression.mana_cost_reduction,
        maximum_health: progression.maximum_health,
        maximum_mana: progression.maximum_mana,
        meditation_idle_delay,
        name: local.name,
        next_concentration_slot,
        offer_seed: progression.offer_seed,
        offensive_damage_flat: progression.offensive_damage_flat,
        pending_skill_choices: progression.pending_skill_choices,
        perk_capacity: progression.perk_capacity,
        perk_selectors,
        poison_immunity_ticks: progression.poison_immunity_ticks,
        previous_threshold: progression.previous_threshold,
        next_threshold: progression.next_threshold,
        rows: progression.rows,
        random_boast_active: progression.random_boast_active,
        selected_primary_skill_id,
        concentration_skill_ids,
        skill_quickbar,
        starting_primary: progression.starting_primary,
        starting_secondary: progression.starting_secondary,
        weld_effect,
    })
}

struct NativeStringSpan {
    end: usize,
    length_offset: usize,
    value: String,
}

struct GameStateLayout {
    boast_text: NativeStringSpan,
    bridge_text: NativeStringSpan,
    failure_offset: usize,
    path: NativeStringSpan,
    selected_offset: usize,
    success_offset: usize,
}

fn native_string_span(payload: &[u8], length_offset: usize) -> Option<NativeStringSpan> {
    let length_bytes = payload.get(length_offset..length_offset.checked_add(4)?)?;
    let length =
        u32::from_le_bytes(length_bytes.try_into().expect("four string length bytes")) as usize;
    let start = length_offset + 4;
    let raw = payload.get(start..start.checked_add(length)?)?;
    let value = if length == 0 {
        String::new()
    } else {
        if raw.last() != Some(&0) {
            return None;
        }
        utf8(&raw[..raw.len() - 1]).ok()?.to_owned()
    };
    Some(NativeStringSpan { end: start + length, length_offset, value })
}

fn boast_statement(selected: Option<u32>) -> Result<&'static str, String> {
    let Some(selected) = selected else {
        return Ok("");
    };
    solomon_sim::hub::interaction::NATIVE_BOASTS
        .iter()
        .find(|boast| boast.id == selected)
        .map(|boast| boast.statement)
        .ok_or_else(|| format!("native Boast {selected} is invalid"))
}

fn game_state_layout(node: &NativeChunkNode) -> Result<GameStateLayout, String> {
    let mut paths = Vec::new();
    for offset in 0..node.payload.len().saturating_sub(4) {
        if let Some(span) = native_string_span(&node.payload, offset)
            && span.value.to_lowercase().ends_with(".boneyard")
        {
            paths.push(span);
        }
    }
    if paths.len() != 1 {
        return Err(format!(
            "native gamestate has {} selected Boneyard path candidates",
            paths.len()
        ));
    }
    let path = paths.pop().expect("the unique path was checked");
    let mut candidates = Vec::new();
    for selected_offset in path.length_offset.saturating_sub(1024)..path.length_offset {
        let raw = node.payload[selected_offset];
        if raw != 0xff && raw > 4 {
            continue;
        }
        let selected = if raw == 0xff { None } else { Some(u32::from(raw)) };
        let Some(boast_text) = native_string_span(&node.payload, selected_offset + 1) else {
            continue;
        };
        let matches = if selected.is_none() {
            boast_text.value.is_empty() || boast_text.value == "\u{1}"
        } else {
            boast_text.value == boast_statement(selected)?
        };
        if !matches {
            continue;
        }
        let Some(bridge_text) = native_string_span(&node.payload, boast_text.end) else {
            continue;
        };
        if bridge_text.end == path.length_offset {
            candidates.push((boast_text, bridge_text, selected_offset));
        }
    }
    if candidates.len() != 1 {
        return Err(format!("native gamestate has {} Boast layout candidates", candidates.len()));
    }
    let (boast_text, bridge_text, selected_offset) =
        candidates.pop().expect("the unique layout was checked");
    let post_path = native_string_span(&node.payload, path.end + 8)
        .filter(|span| span.end + 6 <= node.payload.len())
        .ok_or("native gamestate Boast tail is truncated")?;
    let failure_offset = post_path.end + 4;
    let success_offset = failure_offset + 1;
    if node.payload[failure_offset] > 1 || node.payload[success_offset] > 1 {
        return Err("native gamestate Boast state is not boolean".into());
    }
    Ok(GameStateLayout {
        boast_text,
        bridge_text,
        failure_offset,
        path,
        selected_offset,
        success_offset,
    })
}

fn game_boast(node: &NativeChunkNode) -> Result<NativeGameBoastState, String> {
    let layout = game_state_layout(node)?;
    let raw = node.payload[layout.selected_offset];
    let selected = if raw == 0xff { None } else { Some(u32::from(raw)) };
    let failed = node.payload[layout.failure_offset] == 1;
    let succeeded = node.payload[layout.success_offset] == 1;
    if (selected.is_none() && (failed || succeeded))
        || (selected == Some(3) && failed)
        || (failed && succeeded)
    {
        return Err("native gamestate Boast lifecycle is inconsistent".into());
    }
    Ok(NativeGameBoastState { failed, selected, succeeded })
}

pub fn decode_native_gamestate_boast(bytes: &[u8]) -> Result<NativeGameBoastState, String> {
    let buffer = parse_native_sync_buffer(bytes)?;
    require_shape(&buffer.root, None, Some(8), "native gamestate root")?;
    game_boast(&buffer.root.children[5])
}
