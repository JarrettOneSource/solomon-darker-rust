//! Current Website hall-of-fame-store.ts browser documents. The browser owns
//! storage I/O; these functions preserve independent v3/v2 admission, read-only
//! legacy merging, run identity and the stable native top-100 ranking.

use std::collections::BTreeSet;

use serde::Serialize;
use serde::ser::SerializeStruct;
use serde_json::Value;
use solomon_sim::hall_of_fame::{
    HallOfFameBoard, HallOfFameEntry, HallOfFameSkill, rank_hall_of_fame_entries,
};
use solomon_sim::profile::{WizardDiscipline, WizardElement};

pub const STORAGE_KEY: &str = "sdr.game.hall-of-fame.v3";
pub const LEGACY_STORAGE_KEY: &str = "sdr.game.hall-of-fame.v2";
const MAXIMUM_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;

/// A malformed row invalidates only its own key. Modern rows win duplicate
/// run IDs before ranking, even when the legacy duplicate has a higher score.
#[must_use]
pub fn read(current: Option<&str>, legacy: Option<&str>) -> Vec<HallOfFameEntry> {
    let mut entries = Vec::new();
    let mut seen = BTreeSet::new();
    for entry in read_key(current).into_iter().chain(read_key(legacy)) {
        if seen.insert(entry.run_id.clone()) {
            entries.push(entry);
        }
    }
    ranked(&entries)
}

/// Source recordLocalHallOfFame replaces all rows with the same run ID and
/// writes the returned list to v3. The caller leaves the legacy key untouched.
#[must_use]
pub fn record(
    entry: HallOfFameEntry,
    current: Option<&str>,
    legacy: Option<&str>,
) -> Vec<HallOfFameEntry> {
    let mut entries = vec![entry];
    for candidate in read(current, legacy) {
        if candidate.run_id != entries[0].run_id {
            entries.push(candidate);
        }
    }
    ranked(&entries)
}

/// Source writes camelCase rows, without modifying or rewriting v2.
pub fn serialize(entries: &[HallOfFameEntry]) -> Result<String, String> {
    let rows: Vec<_> = entries.iter().map(StoredEntry).collect();
    serde_json::to_string(&rows).map_err(|error| format!("Hall of Fame document: {error}"))
}

fn ranked(entries: &[HallOfFameEntry]) -> Vec<HallOfFameEntry> {
    rank_hall_of_fame_entries(entries, HallOfFameBoard::Awesomeness, 100)
        .into_iter()
        .cloned()
        .collect()
}

fn read_key(serialized: Option<&str>) -> Vec<HallOfFameEntry> {
    let Some(value) = serialized.and_then(|text| serde_json::from_str::<Value>(text).ok()) else {
        return Vec::new();
    };
    let Some(rows) = value.as_array() else {
        return Vec::new();
    };
    let mut entries = Vec::new();
    for row in rows {
        let Some(entry) = read_entry(row) else {
            return Vec::new();
        };
        entries.push(entry);
    }
    entries
}

fn read_entry(value: &Value) -> Option<HallOfFameEntry> {
    let source = value.as_object()?;
    let discipline = source.get("discipline")?.as_str()?;
    let element = source.get("element")?.as_str()?;
    let mut highest_skills = Vec::new();
    for skill in source.get("highestSkills")?.as_array()? {
        highest_skills.push(HallOfFameSkill {
            rank: integer(skill.get("rank")?)?,
            skill_id: integer(skill.get("skillId")?)?,
        });
    }
    let mut perks_used = Vec::new();
    for selector in source.get("perksUsed")?.as_array()? {
        perks_used.push(integer(selector)?);
    }
    let portrait_scale = source.get("portraitScale")?.as_f64()?;
    if highest_skills.len() > 3 || perks_used.len() > 9 || !(0.85..=1.0).contains(&portrait_scale) {
        return None;
    }
    Some(HallOfFameEntry {
        account_username: nullable_string(source.get("accountUsername")?)?,
        awesomeness: integer(source.get("awesomeness")?)?,
        awesomest_kill: nullable_string(source.get("awesomestKill")?)?,
        completed_at_utc: source.get("completedAtUtc")?.as_str()?.to_owned(),
        discipline: WizardDiscipline::ALL.into_iter().find(|root| root.as_str() == discipline)?,
        elapsed_ticks: integer(source.get("elapsedTicks")?)?,
        element: WizardElement::ALL.into_iter().find(|root| root.as_str() == element)?,
        heading_index: integer(source.get("headingIndex")?)?,
        highest_skills,
        level: integer(source.get("level")?)?,
        monsters_killed: integer(source.get("monstersKilled")?)?,
        perks_used,
        portrait_scale,
        run_id: source.get("runId")?.as_str()?.to_owned(),
        wave: integer(source.get("wave")?)?,
        wizard_name: source.get("wizardName")?.as_str()?.to_owned(),
    })
}

fn integer(value: &Value) -> Option<u64> {
    let number = value.as_f64()?;
    if number.fract() != 0.0 || !(0.0..=MAXIMUM_SAFE_INTEGER).contains(&number) {
        return None;
    }
    Some(number as u64)
}

fn nullable_string(value: &Value) -> Option<Option<String>> {
    if value.is_null() { Some(None) } else { value.as_str().map(|text| Some(text.to_owned())) }
}

struct StoredEntry<'a>(&'a HallOfFameEntry);

impl Serialize for StoredEntry<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let entry = self.0;
        let skills: Vec<_> = entry.highest_skills.iter().map(StoredSkill).collect();
        let mut row = serializer.serialize_struct("HallOfFameEntry", 16)?;
        row.serialize_field("accountUsername", &entry.account_username)?;
        row.serialize_field("awesomeness", &entry.awesomeness)?;
        row.serialize_field("awesomestKill", &entry.awesomest_kill)?;
        row.serialize_field("completedAtUtc", &entry.completed_at_utc)?;
        row.serialize_field("discipline", entry.discipline.as_str())?;
        row.serialize_field("elapsedTicks", &entry.elapsed_ticks)?;
        row.serialize_field("element", entry.element.as_str())?;
        row.serialize_field("headingIndex", &entry.heading_index)?;
        row.serialize_field("highestSkills", &skills)?;
        row.serialize_field("level", &entry.level)?;
        row.serialize_field("monstersKilled", &entry.monsters_killed)?;
        row.serialize_field("perksUsed", &entry.perks_used)?;
        // JSON.stringify spells the upper portrait limit as 1, not 1.0.
        if entry.portrait_scale == 1.0 {
            row.serialize_field("portraitScale", &1_u8)?;
        } else {
            row.serialize_field("portraitScale", &entry.portrait_scale)?;
        }
        row.serialize_field("runId", &entry.run_id)?;
        row.serialize_field("wave", &entry.wave)?;
        row.serialize_field("wizardName", &entry.wizard_name)?;
        row.end()
    }
}

struct StoredSkill<'a>(&'a HallOfFameSkill);

impl Serialize for StoredSkill<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut row = serializer.serialize_struct("HallOfFameSkill", 2)?;
        row.serialize_field("rank", &self.0.rank)?;
        row.serialize_field("skillId", &self.0.skill_id)?;
        row.end()
    }
}
