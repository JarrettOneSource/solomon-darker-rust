//! College NPC geometry, dialogue, selectors and markers, from Website
//! `hub-inventory-presentation.ts`, `hub-npc-dialogue.ts`, `native-hub-npc.ts`, and
//! `renderer/hub-npc-marker-presentation.ts`. Stock authored rows live in `catalog.rs`.

mod catalog;
mod presentation;

use std::collections::BTreeSet;

pub use presentation::{
    HubNpcSelectorId, HubNpcSelectorPresentation, hub_boast_failure_text, hub_boast_instruction,
    hub_npc_selector_action_with_mods, hub_npc_selector_response_with_mods,
    hub_npc_selector_rows_with_mods,
};

pub use catalog::{
    HUB_BAD_EULOGIES, HUB_DIALOGUES, HUB_EULOGIES, HUB_INTERACTIONS, HUB_INTERRUPTED_EULOGIES,
    HUB_MARKER_ACTORS, NATIVE_BOAST_INSTRUCTION, NATIVE_BOASTS, NATIVE_LIBRARIAN_BOOKS,
    NATIVE_TEACHER_SPELLS,
};

use crate::economy::{BoastState, HubTraderId, NativeHubNpcState};
use crate::float;
use crate::float::{atan2, fround};
use crate::hub::regions::HubRegion;
use crate::{NativeRng, Vec2};

/// Declaration order is the native interaction tie-break order, with story Polisher last.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum HubInteractionId {
    Hagatha,
    Fomentius,
    Annalist,
    Luthacus,
    Skorcha,
    Teacher,
    Memorator,
    Painting0,
    Painting1,
    Painting100,
    Painting3,
    Painting4,
    Painting5,
    Painting6,
    Painting7,
    Painting8,
    Painting9,
    Librarian,
    Shlorio,
    ArchChancellor,
    Polisher,
}

impl HubInteractionId {
    pub const ALL: [Self; 21] = [
        Self::Hagatha,
        Self::Fomentius,
        Self::Annalist,
        Self::Luthacus,
        Self::Skorcha,
        Self::Teacher,
        Self::Memorator,
        Self::Painting0,
        Self::Painting1,
        Self::Painting100,
        Self::Painting3,
        Self::Painting4,
        Self::Painting5,
        Self::Painting6,
        Self::Painting7,
        Self::Painting8,
        Self::Painting9,
        Self::Librarian,
        Self::Shlorio,
        Self::ArchChancellor,
        Self::Polisher,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Hagatha => "hagatha",
            Self::Fomentius => "fomentius",
            Self::Annalist => "annalist",
            Self::Luthacus => "luthacus",
            Self::Skorcha => "skorcha",
            Self::Teacher => "teacher",
            Self::Memorator => "memorator",
            Self::Painting0 => "painting-0",
            Self::Painting1 => "painting-1",
            Self::Painting100 => "painting-100",
            Self::Painting3 => "painting-3",
            Self::Painting4 => "painting-4",
            Self::Painting5 => "painting-5",
            Self::Painting6 => "painting-6",
            Self::Painting7 => "painting-7",
            Self::Painting8 => "painting-8",
            Self::Painting9 => "painting-9",
            Self::Librarian => "librarian",
            Self::Shlorio => "shlorio",
            Self::ArchChancellor => "arch-chancellor",
            Self::Polisher => "polisher",
        }
    }

    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|candidate| candidate.as_str() == id)
    }

    #[must_use]
    pub fn definition(self, story_office: bool) -> &'static HubInteractionDefinition {
        if story_office && self == Self::ArchChancellor {
            return &catalog::HUB_STORY_ARCH_CHANCELLOR;
        }
        &HUB_INTERACTIONS[self as usize]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HubNpcSelector {
    Boast,
    Books,
    Fomentius,
    Hagatha,
    Luthacus,
    Shlorio,
    TeacherSpells,
}

impl HubNpcSelector {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Boast => "boast",
            Self::Books => "books",
            Self::Fomentius => "fomentius",
            Self::Hagatha => "hagatha",
            Self::Luthacus => "luthacus",
            Self::Shlorio => "shlorio",
            Self::TeacherSpells => "teacher-spells",
        }
    }
    #[must_use]
    pub const fn trader(self) -> Option<HubTraderId> {
        match self {
            Self::Fomentius => Some(HubTraderId::Fomentius),
            Self::Hagatha => Some(HubTraderId::Hagatha),
            Self::Luthacus => Some(HubTraderId::Luthacus),
            Self::Shlorio => Some(HubTraderId::Shlorio),
            _ => None,
        }
    }

    #[must_use]
    pub const fn list_selector(self) -> Option<HubNpcListSelector> {
        match self {
            Self::Boast => Some(HubNpcListSelector::Boast),
            Self::Books => Some(HubNpcListSelector::Books),
            Self::TeacherSpells => Some(HubNpcListSelector::TeacherSpells),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HubNpcListSelector {
    Boast,
    Books,
    TeacherSpells,
}

impl HubNpcListSelector {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Boast => "boast",
            Self::Books => "books",
            Self::TeacherSpells => "teacher-spells",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HubNpcCommand {
    pub label: &'static str,
    pub native_command: &'static str,
    pub selector: HubNpcSelector,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HubNpcGeometry {
    pub position: Vec2,
    pub radius: f64,
    pub range_radius: Option<f64>,
    pub region: HubRegion,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HubInteractionDefinition {
    pub id: HubInteractionId,
    pub commands: &'static [HubNpcCommand],
    pub dismissals: &'static [&'static str],
    pub eulogy_index: Option<u32>,
    pub geometry: HubNpcGeometry,
    pub intro: Option<&'static str>,
    pub name: &'static str,
    pub questions: &'static [&'static str],
    pub service_title: Option<&'static str>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HubDialogueRecord {
    pub key: &'static str,
    pub label: &'static str,
    pub lines: &'static [&'static str],
    pub one_shot: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeLibrarianBook {
    pub id: u32,
    pub key: &'static str,
    pub lines: &'static [&'static str],
    pub one_shot: bool,
    pub title: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeTeacherSpell {
    pub explanation_label: &'static str,
    pub explanation_lines: &'static [&'static str],
    pub key: &'static str,
    pub name: &'static str,
    pub price: u64,
    pub quick_description: &'static str,
    pub skill_id: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "kebab-case"))]
pub enum BoastFailureProducer {
    MagicalEquipment,
    ManaUnderflow,
    PotionUse,
    SecondaryCast,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeBoastDefinition {
    pub failure_producer: Option<BoastFailureProducer>,
    pub icon_record: usize,
    pub id: u32,
    pub label: &'static str,
    pub response: &'static str,
    pub statement: &'static str,
}

/// A borrowed stock or authored definition. The prepared content owner resolves
/// it from the state's selection; the state never retains a second definition.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoastRules<'a> {
    pub failure_producers: &'a [BoastFailureProducer],
    pub label: &'a str,
    pub random_skill_choices: bool,
    pub score_multiplier: f64,
    pub success_wave: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HubInteractionAvailability {
    pub skorcha_position: Option<Vec2>,
    pub skorcha_variant: Option<usize>,
    pub story_office: bool,
}

#[must_use]
pub fn hub_interaction_within_range(
    id: HubInteractionId,
    region: HubRegion,
    point: Vec2,
    availability: HubInteractionAvailability,
) -> bool {
    if id == HubInteractionId::Skorcha && availability.skorcha_position.is_none() {
        return false;
    }
    let geometry = interaction_geometry(id, availability);
    if geometry.region != region {
        return false;
    }
    let radius = geometry.range_radius.unwrap_or(geometry.radius);
    squared_distance(point, geometry.position) <= 5.0 * radius * radius + 1500.0
}

#[must_use]
pub fn nearest_hub_interaction(
    region: HubRegion,
    point: Vec2,
    availability: HubInteractionAvailability,
) -> Option<HubInteractionId> {
    nearest_interaction(region, point, availability, true)
}

#[must_use]
pub fn hub_interaction_at_point(
    region: HubRegion,
    point: Vec2,
    availability: HubInteractionAvailability,
) -> Option<HubInteractionId> {
    nearest_interaction(region, point, availability, false)
}

fn nearest_interaction(
    region: HubRegion,
    point: Vec2,
    availability: HubInteractionAvailability,
    service_range: bool,
) -> Option<HubInteractionId> {
    let mut nearest: Option<(f64, HubInteractionId)> = None;
    for id in HubInteractionId::ALL {
        if (id == HubInteractionId::Skorcha && availability.skorcha_position.is_none())
            || (id == HubInteractionId::Polisher && !availability.story_office)
        {
            continue;
        }
        let geometry = interaction_geometry(id, availability);
        if geometry.region != region {
            continue;
        }
        let radius = if service_range {
            geometry.range_radius.unwrap_or(geometry.radius)
        } else {
            geometry.radius
        };
        let maximum = if service_range { 5.0 * radius * radius + 1500.0 } else { radius * radius };
        let distance = squared_distance(point, geometry.position);
        if distance > maximum {
            continue;
        }
        if nearest.is_none_or(|(previous, _)| distance < previous) {
            nearest = Some((distance, id));
        }
    }
    nearest.map(|(_, id)| id)
}

fn interaction_geometry(
    id: HubInteractionId,
    availability: HubInteractionAvailability,
) -> HubNpcGeometry {
    let mut geometry = id.definition(false).geometry;
    if id != HubInteractionId::Skorcha {
        return geometry;
    }
    if let Some(position) = availability.skorcha_position {
        geometry.position = position;
    }
    geometry
}

fn squared_distance(left: Vec2, right: Vec2) -> f64 {
    let dx = left.x - right.x;
    let dy = left.y - right.y;
    dx * dx + dy * dy
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HubNpcChatNext {
    Choices,
    Close,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HubNpcChatContent {
    Choices,
    Selector(HubNpcListSelector),
    Speech { key: String, lines: Vec<String>, next: HubNpcChatNext },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HubNpcChatChoice {
    Command { label: &'static str, selector: HubNpcSelector },
    Question { key: &'static str, label: &'static str },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HubNpcSelectorRow {
    pub id: u32,
    pub label: &'static str,
    pub detail: String,
    pub price: Option<u64>,
    pub boast_icon_record: Option<usize>,
}

#[must_use]
pub fn hub_dialogue_record(key: &str) -> Option<&'static HubDialogueRecord> {
    HUB_DIALOGUES.iter().find(|record| record.key == key)
}

#[must_use]
pub fn create_hub_npc_chat_content(
    id: HubInteractionId,
    npc: &NativeHubNpcState,
    random_index: i64,
    eulogy_index_override: Option<u32>,
    story_office: bool,
    consumed: &BTreeSet<String>,
) -> HubNpcChatContent {
    let interaction = id.definition(story_office);
    if let Some(index) = eulogy_index_override.or(interaction.eulogy_index) {
        let mut lines = Vec::new();
        if let Some(line) =
            HUB_EULOGIES.iter().find(|(id, _)| *id == index).and_then(|(_, line)| *line)
        {
            lines.push(line.to_owned());
        }
        if !npc.boast.succeeded {
            lines.push(pick(HUB_BAD_EULOGIES, random_index).to_owned());
        }
        return HubNpcChatContent::Speech {
            key: format!("SAY_EULOGY_{index}"),
            lines,
            next: HubNpcChatNext::Close,
        };
    }
    let intro = interaction.intro.and_then(hub_dialogue_record);
    if intro.is_none_or(|record| record.one_shot && consumed.contains(record.key)) {
        if !hub_npc_chat_choices(id, story_office, consumed).is_empty() {
            return HubNpcChatContent::Choices;
        }
        return hub_npc_dismissal(id, random_index, story_office)
            .unwrap_or(HubNpcChatContent::Choices);
    }
    let next = if !hub_npc_chat_choices(id, story_office, consumed).is_empty() {
        HubNpcChatNext::Choices
    } else {
        HubNpcChatNext::Close
    };
    record_speech(intro.expect("available authored NPC intro"), next)
}

#[must_use]
pub fn hub_npc_chat_choices(
    id: HubInteractionId,
    story_office: bool,
    consumed: &BTreeSet<String>,
) -> Vec<HubNpcChatChoice> {
    let interaction = id.definition(story_office);
    let mut choices = Vec::new();
    for command in interaction.commands {
        choices
            .push(HubNpcChatChoice::Command { label: command.label, selector: command.selector });
    }
    for key in interaction.questions {
        let question = hub_dialogue_record(key).expect("the authored NPC question");
        if question.one_shot && consumed.contains(question.key) {
            continue;
        }
        choices.push(HubNpcChatChoice::Question { key: question.key, label: question.label });
    }
    choices
}

#[must_use]
pub fn hub_npc_question(
    id: HubInteractionId,
    key: &str,
    story_office: bool,
    consumed: &BTreeSet<String>,
) -> Option<HubNpcChatContent> {
    if !id.definition(story_office).questions.contains(&key) {
        return None;
    }
    let question = hub_dialogue_record(key).expect("the authored NPC question");
    if question.one_shot && consumed.contains(question.key) {
        return None;
    }
    let mut remaining = consumed.clone();
    if question.one_shot {
        remaining.insert(question.key.to_owned());
    }
    let next = if hub_npc_chat_choices(id, story_office, &remaining).is_empty() {
        HubNpcChatNext::Close
    } else {
        HubNpcChatNext::Choices
    };
    Some(record_speech(question, next))
}

/// Native Chat::Say retires one-shot rows as speech starts, including a skipped intro.
pub fn consume_hub_npc_speech(
    consumed: &mut BTreeSet<String>,
    id: HubInteractionId,
    key: &str,
    story_office: bool,
) {
    let interaction = id.definition(story_office);
    if interaction.intro != Some(key) && !interaction.questions.contains(&key) {
        return;
    }
    if hub_dialogue_record(key).is_some_and(|record| record.one_shot) {
        consumed.insert(key.to_owned());
    }
}

#[must_use]
pub fn hub_npc_dismissal(
    id: HubInteractionId,
    random_index: i64,
    story_office: bool,
) -> Option<HubNpcChatContent> {
    let dismissals = id.definition(story_office).dismissals;
    if dismissals.is_empty() {
        return None;
    }
    Some(record_speech(
        hub_dialogue_record(pick(dismissals, random_index)).expect("the authored NPC dismissal"),
        HubNpcChatNext::Close,
    ))
}

#[must_use]
pub fn hub_npc_selector_rows(
    selector: HubNpcListSelector,
    npc: &NativeHubNpcState,
    advanced_unlocks: &[bool; 8],
) -> Vec<HubNpcSelectorRow> {
    match selector {
        HubNpcListSelector::Boast => NATIVE_BOASTS
            .iter()
            .map(|boast| HubNpcSelectorRow {
                id: boast.id,
                label: boast.label,
                detail: boast.statement.to_owned(),
                price: None,
                boast_icon_record: Some(boast.icon_record),
            })
            .collect(),
        HubNpcListSelector::Books => NATIVE_LIBRARIAN_BOOKS
            .iter()
            .filter(|book| !book.one_shot || !npc.librarian_lace_read)
            .map(|book| HubNpcSelectorRow {
                id: book.id,
                label: book.title,
                detail: book.lines.join(" "),
                price: None,
                boast_icon_record: None,
            })
            .collect(),
        HubNpcListSelector::TeacherSpells => NATIVE_TEACHER_SPELLS
            .iter()
            .filter(|spell| !advanced_unlocks[spell.skill_id - 72])
            .map(|spell| HubNpcSelectorRow {
                id: spell.skill_id as u32,
                label: spell.name,
                detail: spell.quick_description.to_owned(),
                price: Some(spell.price),
                boast_icon_record: None,
            })
            .collect(),
    }
}

#[must_use]
pub fn hub_npc_selector_title(selector: HubNpcListSelector) -> &'static str {
    match selector {
        HubNpcListSelector::Boast => "SELECT A BOAST",
        HubNpcListSelector::Books => "SELECT A BOOK",
        HubNpcListSelector::TeacherSpells => "SELECT A SPELL",
    }
}

#[must_use]
pub fn hub_npc_selector_response(
    selector: HubNpcListSelector,
    id: u32,
) -> Option<HubNpcChatContent> {
    match selector {
        HubNpcListSelector::Boast => {
            let boast = NATIVE_BOASTS.iter().find(|boast| boast.id == id)?;
            Some(record_speech(
                hub_dialogue_record(boast.response).expect("the native boast response"),
                HubNpcChatNext::Close,
            ))
        }
        HubNpcListSelector::Books => {
            let book = NATIVE_LIBRARIAN_BOOKS.iter().find(|book| book.id == id)?;
            Some(speech(book.key, book.lines, HubNpcChatNext::Choices))
        }
        HubNpcListSelector::TeacherSpells => {
            let spell = NATIVE_TEACHER_SPELLS.iter().find(|spell| spell.skill_id == id as usize)?;
            Some(speech(spell.key, spell.explanation_lines, HubNpcChatNext::Choices))
        }
    }
}

#[must_use]
pub fn hub_npc_selector_action(
    selector: HubNpcListSelector,
    id: u32,
) -> crate::economy::HubInventoryAction {
    use crate::economy::HubInventoryAction;
    match selector {
        HubNpcListSelector::Boast => {
            assert!(id <= 4, "the selected stock boast is in 0..4");
            HubInventoryAction::SelectBoast { boast_id: id.into() }
        }
        HubNpcListSelector::Books => HubInventoryAction::ReadLibrarianBook { book_id: id },
        HubNpcListSelector::TeacherSpells => {
            HubInventoryAction::BuyTeacherSpell { skill_id: id as usize }
        }
    }
}

fn record_speech(record: &HubDialogueRecord, next: HubNpcChatNext) -> HubNpcChatContent {
    speech(record.key, record.lines, next)
}
fn speech(key: &str, lines: &[&str], next: HubNpcChatNext) -> HubNpcChatContent {
    HubNpcChatContent::Speech {
        key: key.to_owned(),
        lines: lines.iter().map(|line| (*line).to_owned()).collect(),
        next,
    }
}
fn pick<T: Copy>(values: &[T], random_index: i64) -> T {
    assert!(!values.is_empty(), "native NPC selection has an authored row");
    values[random_index.unsigned_abs() as usize % values.len()]
}

impl BoastState {
    #[must_use]
    pub fn selected_native(&self) -> Option<u32> {
        self.selected.as_ref()?.native_id()
    }

    pub fn select(&mut self, selection: crate::economy::BoastSelection) {
        self.selected = Some(selection);
        self.succeeded = false;
    }

    pub fn fail(&mut self, producer: BoastFailureProducer) {
        self.fail_with_rules(producer, native_boast_rules(self));
    }

    pub fn fail_with_rules(
        &mut self,
        producer: BoastFailureProducer,
        definition: Option<BoastRules<'_>>,
    ) {
        if self.failed
            || definition.is_none_or(|definition| !definition.failure_producers.contains(&producer))
        {
            return;
        }
        self.failed = true;
        self.failure_sequence += 1;
        self.succeeded = false;
    }

    pub fn complete_wave(&mut self, completed_wave: u32) {
        self.complete_wave_with_rules(completed_wave, native_boast_rules(self));
    }

    pub fn complete_wave_with_rules(
        &mut self,
        completed_wave: u32,
        definition: Option<BoastRules<'_>>,
    ) {
        if definition.is_some_and(|definition| completed_wave >= definition.success_wave)
            && !self.failed
            && !self.succeeded
        {
            self.succeeded = true;
        }
    }

    #[must_use]
    pub fn score(&self, score: u64) -> u64 {
        self.score_with_rules(score, native_boast_rules(self))
    }

    #[must_use]
    pub fn score_with_rules(&self, score: u64, definition: Option<BoastRules<'_>>) -> u64 {
        assert!(
            score <= 9_007_199_254_740_991,
            "a native boast score fits the safe integer contract"
        );
        let Some(definition) = definition.filter(|_| self.succeeded) else { return score };
        (fround(score as f64) * definition.score_multiplier).trunc() as u64
    }

    #[must_use]
    pub fn uses_random_skill_choices(&self) -> bool {
        self.uses_random_skill_choices_with_rules(native_boast_rules(self))
    }

    #[must_use]
    pub fn uses_random_skill_choices_with_rules(&self, definition: Option<BoastRules<'_>>) -> bool {
        self.selected.is_some()
            && !self.failed
            && definition.is_some_and(|definition| definition.random_skill_choices)
    }

    #[must_use]
    pub fn failure_text(&self) -> Option<String> {
        self.failure_text_with_rules(native_boast_rules(self))
    }

    #[must_use]
    pub fn failure_text_with_rules(&self, definition: Option<BoastRules<'_>>) -> Option<String> {
        let definition = definition?;
        self.failed.then(|| format!("FAILED \"{}\"", definition.label))
    }
}

fn native_boast_rules(state: &BoastState) -> Option<BoastRules<'static>> {
    native_boast_rules_for_selection(state.selected.as_ref()?)
}

#[must_use]
pub fn native_boast_rules_for_selection(
    selection: &crate::economy::BoastSelection,
) -> Option<BoastRules<'static>> {
    let id = selection.native_id()?;
    let definition = NATIVE_BOASTS.get(id as usize)?;
    Some(BoastRules {
        failure_producers: definition.failure_producer.as_slice(),
        label: definition.label,
        random_skill_choices: id == 3,
        score_multiplier: 1.1_f32 as f64,
        success_wave: 30,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HubNpcMarkerSide {
    Left,
    Right,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HubNpcMarkerStyle {
    Help,
    Talk,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HubNpcMarkerSurface {
    Dialogue,
    Inventory,
    Modal,
    Service,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HubNpcMarkerDefinition {
    pub interaction_id: HubInteractionId,
    pub phase_advances: bool,
    pub profile_hint_index: Option<usize>,
    pub record: usize,
    pub region: HubRegion,
    pub side: HubNpcMarkerSide,
    pub style: HubNpcMarkerStyle,
    pub type_id: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HubNpcMarkerFrame {
    pub alpha: f64,
    pub interaction_id: HubInteractionId,
    pub phase_degrees: f64,
    pub position: Vec2,
    pub record: usize,
    pub region: HubRegion,
    pub side: HubNpcMarkerSide,
    pub style: HubNpcMarkerStyle,
    pub visible: bool,
}

/// Suppression and phase seed are captured on region entry; acknowledgement changes the
/// live tutorial flags without re-seeding the existing scene's markers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct HubNpcMarkerEpoch {
    pub seed: u32,
    pub started_at_tick: u64,
    pub suppression: [bool; 3],
}

impl HubNpcMarkerEpoch {
    #[must_use]
    pub fn new(trader_seed: u32, tick: u64, help_flags: &[bool; 10]) -> Self {
        Self {
            seed: trader_seed ^ tick as u32,
            started_at_tick: tick,
            suppression: [help_flags[0], help_flags[1], help_flags[2]],
        }
    }

    #[must_use]
    pub fn frame(
        &self,
        id: HubInteractionId,
        absolute_tick: u64,
        availability: HubInteractionAvailability,
        surface: Option<HubNpcMarkerSurface>,
    ) -> HubNpcMarkerFrame {
        hub_npc_marker_frame(
            id,
            absolute_tick.saturating_sub(self.started_at_tick),
            self.seed,
            self.suppression,
            availability,
            surface,
        )
    }
}

#[must_use]
pub fn hub_npc_marker_frame(
    id: HubInteractionId,
    tick: u64,
    seed: u32,
    suppression: [bool; 3],
    availability: HubInteractionAvailability,
    surface: Option<HubNpcMarkerSurface>,
) -> HubNpcMarkerFrame {
    let actor = HUB_MARKER_ACTORS
        .iter()
        .find(|actor| actor.interaction_id == id)
        .expect("an interaction with an authored actor marker");
    let mirrored = id == HubInteractionId::Skorcha && availability.skorcha_variant == Some(1);
    let side = if mirrored { HubNpcMarkerSide::Left } else { actor.side };
    let position = interaction_geometry(id, availability).position;
    let actor_seed = seed ^ actor.type_id.wrapping_mul(0x9e37_79b1);
    let initial_phase = NativeRng::new(i64::from(actor_seed)).draw_integer(5000);
    let phase_degrees =
        f64::from(initial_phase) + if actor.phase_advances { tick as f64 } else { 0.0 };
    let suppressed = actor.profile_hint_index.is_some_and(|index| suppression[index]);
    HubNpcMarkerFrame {
        alpha: float::sin(phase_degrees * std::f64::consts::PI / 180.0) * 0.25 + 0.75,
        interaction_id: id,
        phase_degrees,
        position: Vec2::new(
            position.x + if side == HubNpcMarkerSide::Right { 30.0 } else { -30.0 },
            position.y - 60.0,
        ),
        record: actor.record + usize::from(mirrored),
        region: actor.region,
        side,
        style: actor.style,
        visible: (id != HubInteractionId::Skorcha || availability.skorcha_position.is_some())
            && !suppressed
            && matches!(surface, None | Some(HubNpcMarkerSurface::Dialogue)),
    }
}

impl NativeHubNpcState {
    pub fn acknowledge_hint(&mut self, id: HubInteractionId) -> bool {
        let index = HUB_MARKER_ACTORS
            .iter()
            .find(|actor| actor.interaction_id == id)
            .and_then(|actor| actor.profile_hint_index);
        let Some(index) = index else {
            return false;
        };
        if !self.help_flags[index] {
            return false;
        }
        self.help_flags[index] = false;
        true
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HubNpcOnboardingPlan {
    WalkToTalk {
        target: HubInteractionId,
        arrow_offset: Vec2,
        arrow_record: usize,
        arrow_rotation_degrees: f64,
        text: &'static str,
        text_offset: Vec2,
    },
    Directional {
        target: HubInteractionId,
        offset: Vec2,
        record: usize,
    },
}

#[must_use]
pub fn hub_npc_onboarding_plan(
    help_flags: &[bool; 10],
    tick: u64,
    surface: Option<HubNpcMarkerSurface>,
) -> Vec<HubNpcOnboardingPlan> {
    if surface.is_some() {
        return Vec::new();
    }
    if help_flags[0] {
        return vec![HubNpcOnboardingPlan::WalkToTalk {
            target: HubInteractionId::Annalist,
            arrow_offset: Vec2::new(15.0, -65.0),
            arrow_record: 28,
            arrow_rotation_degrees: 200.0,
            text: "WALK INTO WIZARDS\nTO TALK TO THEM",
            text_offset: Vec2::new(15.0, -115.0),
        }];
    }
    if tick % 80 <= 40 {
        return Vec::new();
    }
    let mut plans = Vec::new();
    for (index, target, offset) in [
        (1, HubInteractionId::Fomentius, Vec2::new(32.0, -62.0)),
        (2, HubInteractionId::Luthacus, Vec2::new(-32.0, -62.0)),
    ] {
        if help_flags[index] {
            plans.push(HubNpcOnboardingPlan::Directional { target, offset, record: 88 });
        }
    }
    plans
}

#[must_use]
pub fn hub_npc_directional_hint_frame(target: Vec2, viewport: Vec2) -> (Vec2, f64) {
    let position = Vec2::new(
        (viewport.x - 58.0).min(58.0_f64.max(target.x)),
        (viewport.y - 64.0).min(64.0_f64.max(target.y)),
    );
    (position, atan2(target.y - position.y, target.x - position.x))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fifty_unit_hagatha_approach_is_in_range_and_fifty_two_is_outside() {
        let availability = HubInteractionAvailability::default();
        // A radius of 15 gives 5 * 15 * 15 + 1500 = 2625 squared world units.
        assert!(hub_interaction_within_range(
            HubInteractionId::Hagatha,
            HubRegion::Courtyard,
            Vec2::new(1390.0, 280.0),
            availability
        ));
        assert!(!hub_interaction_within_range(
            HubInteractionId::Hagatha,
            HubRegion::Courtyard,
            Vec2::new(1392.0, 280.0),
            availability
        ));
    }

    #[test]
    fn a_boast_pays_ten_percent_at_wave_thirty_until_its_actual_failure_occurs() {
        let mut boast = BoastState { selected: Some(2.into()), ..BoastState::default() };
        boast.fail(BoastFailureProducer::PotionUse);
        boast.complete_wave(29);
        assert!(!boast.succeeded);
        boast.complete_wave(30);
        assert_eq!(boast.score(100), 110);
        boast.fail(BoastFailureProducer::SecondaryCast);
        boast.fail(BoastFailureProducer::SecondaryCast);
        assert_eq!(boast.failure_sequence, 1);
        assert_eq!(boast.score(100), 100);
        assert!(!boast.succeeded);
    }
}
