//! Weighted offers, learned ranks and eight quickbar slots from `mod-skill-engine.ts`.

use std::collections::HashSet;
use std::sync::Arc;

use solomon_sim::modding::content::{PreparedModContentCatalog, PreparedModSkillDefinition};
use solomon_sim::modding::definition::WebLuaContentKind;
use solomon_sim::modding::runtime::{
    ModSkillBinding, ModSkillCheckpoint, ModSkillOffer, ModSkillRank,
};
use solomon_sim::modding::{LuaObject, LuaValue};

use super::{MAXIMUM_SAFE_INTEGER, validate_player};
use crate::json::SourceCollator;

pub struct ModSkillEngine {
    catalog: Arc<PreparedModContentCatalog>,
    bindings: Vec<(String, String)>,
    ranks: Vec<(String, u32)>,
    offers: Vec<ModSkillOffer>,
    revision: u64,
    collator: SourceCollator,
}

impl ModSkillEngine {
    pub fn new(catalog: Arc<PreparedModContentCatalog>) -> Self {
        Self {
            catalog,
            bindings: Vec::new(),
            ranks: Vec::new(),
            offers: Vec::new(),
            revision: 0,
            collator: SourceCollator::default(),
        }
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn checkpoint(&self) -> ModSkillCheckpoint {
        ModSkillCheckpoint {
            bindings: self.project_bindings(None),
            offers: self.offers.clone(),
            ranks: self.project(None),
            revision: self.revision,
        }
    }

    pub fn choose(
        &mut self,
        player_id: &str,
        content_id: &str,
        offer_sequence: u64,
    ) -> Result<ModSkillRank, String> {
        let definition = self
            .catalog
            .skill(content_id)
            .ok_or_else(|| format!("mod skill is unavailable: {content_id}"))?;
        let offer = self
            .offers
            .iter()
            .position(|offer| {
                offer.player_id == player_id
                    && offer.sequence == offer_sequence
                    && offer.content_ids.iter().any(|id| id == content_id)
            })
            .ok_or_else(|| format!("mod skill was not offered: {content_id}"))?;
        if !self.eligible(player_id, definition)? {
            return Err(format!("mod skill is not eligible: {content_id}"));
        }
        let key = cell_key(player_id, content_id)?;
        let index = self.ranks.iter().position(|(name, _)| *name == key);
        let rank = index.map_or(0, |index| self.ranks[index].1) + 1;
        if index.is_none() && self.ranks.len() >= 4_096 {
            return Err("mod skill rank limit reached".into());
        }
        if let Some(index) = index {
            self.ranks[index].1 = rank
        } else {
            self.ranks.push((key, rank))
        }
        self.offers.remove(offer);
        self.revision += 1;
        Ok(ModSkillRank {
            content_id: content_id.into(),
            player_id: player_id.into(),
            rank: u64::from(rank),
        })
    }

    pub fn offer(
        &mut self,
        player_id: &str,
        level: u64,
        seed: u64,
        count: u32,
    ) -> Result<Vec<&PreparedModSkillDefinition>, String> {
        if level == 0
            || level > MAXIMUM_SAFE_INTEGER
            || seed > MAXIMUM_SAFE_INTEGER
            || !(1..=8).contains(&count)
        {
            return Err("mod skill offer input is invalid".into());
        }
        let mut pool = Vec::new();
        for skill in self.catalog.skills() {
            if level >= u64::from(skill.minimum_level) && self.eligible(player_id, skill)? {
                pool.push(skill)
            }
        }
        let mut selected = Vec::new();
        while !pool.is_empty() && selected.len() < count as usize {
            let total: f64 = pool.iter().map(|skill| skill.offer_weight).sum();
            let mut cursor = unit_roll(seed, selected.len() as u32) * total;
            let mut chosen = pool.len() - 1;
            for (index, skill) in pool.iter().enumerate() {
                cursor -= skill.offer_weight;
                if cursor < 0.0 {
                    chosen = index;
                    break;
                }
            }
            selected.push(pool.remove(chosen));
        }
        let offer = ModSkillOffer {
            content_ids: selected.iter().map(|skill| skill.common.content_id.clone()).collect(),
            player_id: player_id.into(),
            sequence: self.revision + 1,
        };
        if let Some(index) = self.offers.iter().position(|offer| offer.player_id == player_id) {
            self.offers[index] = offer
        } else {
            self.offers.push(offer)
        }
        self.revision += 1;
        Ok(selected)
    }

    pub fn project(&self, player_id: Option<&str>) -> Vec<ModSkillRank> {
        let mut rows = Vec::new();
        for (key, rank) in &self.ranks {
            let (owner, content_id) =
                key.split_once('\0').expect("skill cell key retains its separator");
            if player_id.is_none_or(|id| owner == id) {
                rows.push(ModSkillRank {
                    content_id: content_id.into(),
                    player_id: owner.into(),
                    rank: u64::from(*rank),
                })
            }
        }
        rows.sort_by(|left, right| {
            self.collator.compare(&left.player_id, &right.player_id).then_with(|| {
                let left = left.content_id.parse::<u64>().expect("compiled skill content identity");
                let right =
                    right.content_id.parse::<u64>().expect("compiled skill content identity");
                left.cmp(&right)
            })
        });
        rows
    }

    pub fn bind(
        &mut self,
        player_id: &str,
        slot: u32,
        content_id: Option<&str>,
    ) -> Result<(), String> {
        if slot >= 8 {
            return Err("mod quickbar slot is invalid".into());
        }
        let key = binding_key(player_id, u64::from(slot))?;
        let index = self.bindings.iter().position(|(name, _)| *name == key);
        let Some(content_id) = content_id else {
            if let Some(index) = index {
                self.bindings.remove(index);
                self.revision += 1
            }
            return Ok(());
        };
        if self.catalog.spell(content_id).is_none()
            || !self.spell_unlocked(player_id, content_id)?
        {
            return Err("mod quickbar spell is unavailable".into());
        }
        if index.is_some_and(|index| self.bindings[index].1 == content_id) {
            return Ok(());
        }
        if let Some(index) = index {
            self.bindings[index].1 = content_id.into()
        } else {
            self.bindings.push((key, content_id.into()))
        }
        self.revision += 1;
        Ok(())
    }

    pub fn filter(&self, player_id: &str, key: &str, source: f64) -> Result<f64, String> {
        let mut value = source;
        for skill in self.catalog.skills() {
            let rank = self.rank(player_id, &skill.common.content_id)?;
            for row in skill.ranks.iter().take(rank as usize) {
                let modifiers = row
                    .get("modify")
                    .filter(|value| !matches!(value, LuaValue::Null))
                    .or_else(|| row.get("modifiers"))
                    .and_then(LuaValue::as_object);
                let modifier = modifiers.and_then(|fields| fields.get(key));
                if let Some(LuaValue::Number(number)) = modifier
                    && number.is_finite()
                {
                    value = *number
                } else if let Some(operations) = modifier.and_then(LuaValue::as_object) {
                    value = apply_operations(value, operations)
                }
            }
        }
        Ok(if value.is_finite() { value.max(0.0) } else { source })
    }

    pub fn project_bindings(&self, player_id: Option<&str>) -> Vec<ModSkillBinding> {
        let mut rows = Vec::new();
        for (key, content_id) in &self.bindings {
            let (owner, slot) =
                key.rsplit_once('\0').expect("quickbar binding key retains its separator");
            if player_id.is_none_or(|id| owner == id) {
                rows.push(ModSkillBinding {
                    content_id: content_id.clone(),
                    player_id: owner.into(),
                    slot: slot.parse().expect("binding key stores a numeric slot"),
                })
            }
        }
        rows.sort_by(|left, right| {
            self.collator
                .compare(&left.player_id, &right.player_id)
                .then(left.slot.cmp(&right.slot))
        });
        rows
    }

    pub fn spell_unlocked(&self, player_id: &str, content_id: &str) -> Result<bool, String> {
        let mut found_owner = false;
        for skill in self.catalog.skills() {
            let owns_spell =
                skill.grants.iter().chain(skill.rank_grants.iter().flatten()).any(|grant| {
                    grant.target_kind == WebLuaContentKind::Spell && grant.content_id == content_id
                });
            if !owns_spell {
                continue;
            }
            found_owner = true;
            let rank = self.rank(player_id, &skill.common.content_id)?;
            if rank > 0 && skill.grants.iter().any(|grant| grant.content_id == content_id)
                || skill
                    .rank_grants
                    .iter()
                    .take(rank as usize)
                    .flatten()
                    .any(|grant| grant.content_id == content_id)
            {
                return Ok(true);
            }
        }
        Ok(!found_owner)
    }

    pub fn offers(&self, player_id: Option<&str>) -> Vec<ModSkillOffer> {
        self.offers
            .iter()
            .filter(|offer| player_id.is_none_or(|id| offer.player_id == id))
            .cloned()
            .collect()
    }

    pub fn rank(&self, player_id: &str, content_id: &str) -> Result<u32, String> {
        let key = cell_key(player_id, content_id)?;
        Ok(self.ranks.iter().find(|(name, _)| *name == key).map_or(0, |(_, rank)| *rank))
    }

    pub fn restore(&mut self, checkpoint: ModSkillCheckpoint) -> Result<(), String> {
        if checkpoint.revision > MAXIMUM_SAFE_INTEGER || checkpoint.ranks.len() > 4_096 {
            return Err("mod skill checkpoint is invalid".into());
        }
        let mut candidate = Vec::new();
        let mut keys = HashSet::new();
        for row in &checkpoint.ranks {
            let definition = self.catalog.skill(&row.content_id);
            let key = cell_key(&row.player_id, &row.content_id)?;
            if definition.is_none_or(|definition| {
                row.rank == 0 || row.rank > u64::from(definition.maximum_rank)
            }) || !keys.insert(key.clone())
            {
                return Err("mod skill checkpoint contains an invalid rank".into());
            }
            candidate.push((
                key,
                u32::try_from(row.rank).expect("accepted definition bounds the stored skill rank"),
            ));
        }
        self.ranks = candidate;
        self.offers.clear();
        self.bindings.clear();
        for offer in checkpoint.offers {
            if self.offers.iter().any(|row| row.player_id == offer.player_id)
                || offer.sequence == 0
                || offer.sequence > MAXIMUM_SAFE_INTEGER
                || offer.content_ids.len() > 8
                || offer.content_ids.iter().any(|id| self.catalog.skill(id).is_none())
            {
                return Err("mod skill checkpoint contains an invalid offer".into());
            }
            self.offers.push(offer);
        }
        for binding in checkpoint.bindings {
            let key = binding_key(&binding.player_id, binding.slot)?;
            if self.bindings.iter().any(|(name, _)| *name == key)
                || self.catalog.spell(&binding.content_id).is_none()
                || !self.spell_unlocked(&binding.player_id, &binding.content_id)?
            {
                return Err("mod skill checkpoint contains an invalid quickbar binding".into());
            }
            self.bindings.push((key, binding.content_id));
        }
        self.revision = checkpoint.revision;
        Ok(())
    }

    fn eligible(
        &self,
        player_id: &str,
        skill: &PreparedModSkillDefinition,
    ) -> Result<bool, String> {
        if self.rank(player_id, &skill.common.content_id)? >= skill.maximum_rank {
            return Ok(false);
        }
        if let Some(parent) = &skill.parent
            && self.rank(player_id, &parent.content_id)? == 0
        {
            return Ok(false);
        }
        for required in &skill.prerequisites {
            if self.rank(player_id, &required.content_id)? == 0 {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

fn cell_key(player_id: &str, content_id: &str) -> Result<String, String> {
    validate_player(player_id, "skill")?;
    Ok(format!("{player_id}\0{content_id}"))
}

fn binding_key(player_id: &str, slot: u64) -> Result<String, String> {
    validate_player(player_id, "skill")?;
    Ok(format!("{player_id}\0{slot}"))
}

fn apply_operations(mut value: f64, fields: &LuaObject) -> f64 {
    if let Some(add) = fields.get("add").and_then(LuaValue::as_number) {
        value += add
    }
    if let Some(multiply) = fields.get("multiply").and_then(LuaValue::as_number) {
        value *= multiply
    }
    if let Some(set) = fields.get("set").and_then(LuaValue::as_number) {
        value = set
    }
    value
}

fn unit_roll(seed: u64, ordinal: u32) -> f64 {
    let mut value = seed as u32 ^ ordinal.wrapping_add(1).wrapping_mul(0x9e37_79b9);
    value ^= value >> 16;
    value = value.wrapping_mul(0x7feb_352d);
    value ^= value >> 15;
    // The final Source ^= has no trailing >>> 0: its bitwise result is signed,
    // so this deliberately retains negative rolls rather than normalizing them.
    f64::from(value as i32) / 4_294_967_296.0
}
