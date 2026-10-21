//! Skill descriptors, equipped rows and offer priority from Source `skill-*.ts`.

use crate::profile::{
    NATIVE_SKILL_CATALOG, NATIVE_WELD_BUILDS, NativeSkillBookState, NativeSkillDefinition,
    NativeSkillStatBookValue, PlayerSkillChoice, PlayerStatBookComponent, WizardElement,
    native_skill_category,
};
use crate::session::GameSession;

use super::OPTION_DESCRIPTOR_WIDTH;
use super::spec::{OPTION_DESCRIPTOR_NAMES, scales};

#[derive(Clone, Copy, Debug)]
pub struct MlBotPolicySkillDescriptorInput {
    pub apply_count: f64,
    pub skill_id: usize,
    pub target_rank: f64,
    pub weld_build_id: Option<u32>,
}

#[derive(Clone, Debug)]
pub struct MlBotPolicySkillOfferDescription {
    pub coverage_keys: Vec<String>,
    pub descriptors: Vec<f32>,
    pub generation: u64,
    pub mask: Vec<u8>,
    pub option_ids: Vec<usize>,
}

struct DescriptorValues([f32; OPTION_DESCRIPTOR_WIDTH]);

impl DescriptorValues {
    fn set(&mut self, name: &str, value: f64) {
        let index = OPTION_DESCRIPTOR_NAMES
            .iter()
            .position(|candidate| *candidate == name)
            .expect("descriptor producers use the canonical v7 names");
        self.0[index] = value as f32;
    }

    fn identity(&mut self, prefix: &str, value: usize) -> Result<(), String> {
        if value > u16::MAX as usize {
            return Err(format!("ML bot policy {prefix} {value} is outside uint16"));
        }
        for bit in 0..16 {
            self.set(&format!("{prefix}_bit_{bit}"), ((value >> bit) & 1) as f64);
        }
        Ok(())
    }

    fn mechanical(&mut self, name: &str, value: Option<f64>, scale: f64) {
        self.set(&format!("{name}_present"), f64::from(value.is_some()));
        self.set(
            &format!("{name}_scaled"),
            value.map_or(0.0, |value| (value / scale).clamp(-1.0, 1.0)),
        );
    }
}

pub fn describe_ml_bot_policy_skill(
    book: &NativeSkillBookState,
    stat_book: &PlayerStatBookComponent,
    input: MlBotPolicySkillDescriptorInput,
) -> Result<Vec<f32>, String> {
    let id = input.skill_id;
    let catalog = policy_skill_catalog_row(id);
    let stats = stat_book.entries.get(id);
    let family = catalog.map_or("unknown", |row| row.family);
    let category = native_skill_category(id);
    let mut values = DescriptorValues([0.0; OPTION_DESCRIPTOR_WIDTH]);
    values.set("present", 1.0);
    values.set("option_id_index_scaled", scaled(id as f64, scales::SKILL_ID));
    values.identity("skill_id", id)?;
    values.set("catalog_known", f64::from(catalog.is_some() && stats.is_some()));
    values.set("apply_count_scaled", scaled(input.apply_count, scales::SKILL_RANK));
    values.set(
        "learned_rank_scaled",
        scaled(book.permanent_ranks.get(id).copied().unwrap_or(0) as f64, scales::SKILL_RANK),
    );
    values.set(
        "effective_rank_scaled",
        scaled(book.effective_ranks.get(id).copied().unwrap_or(0) as f64, scales::SKILL_RANK),
    );
    values.set(
        "cap_rank_scaled",
        scaled(stats.map_or(0, |row| row.cap_level) as f64, scales::SKILL_RANK),
    );
    values.set(
        "max_rank_scaled",
        scaled(stats.map_or(0, |row| row.maximum_level) as f64, scales::SKILL_RANK),
    );
    values.set(
        "band_index_scaled",
        if (8..=47).contains(&id) { (id % 8) as f64 / scales::SKILL_BAND } else { 0.0 },
    );
    values.set(
        "family_element",
        f64::from(matches!(family, "air" | "earth" | "ether" | "fire" | "water")),
    );
    values.set("family_discipline", f64::from(matches!(family, "arcane" | "body" | "mind")));
    for candidate in [
        "ether",
        "fire",
        "air",
        "water",
        "earth",
        "arcane",
        "mind",
        "body",
        "advanced",
        "runtime_only",
    ] {
        values.set(&format!("family_{candidate}"), f64::from(family == candidate));
    }
    values.set("is_primary", f64::from(category == Some(1) && id != 52));
    values.set("is_secondary", f64::from(category == Some(2)));
    values.set("is_passive", f64::from(category == Some(0)));
    values.set("is_utility", f64::from(category == Some(3) || id == 52));
    values.set("is_weld", f64::from(id == 52));
    values.set("is_health_up", f64::from(id == 64));
    values.set("is_mana_up", f64::from(id == 56));
    if let Some(build_id) = input.weld_build_id {
        let weld = NATIVE_WELD_BUILDS
            .iter()
            .find(|weld| weld.id == build_id)
            .ok_or_else(|| format!("ML bot policy weld build {build_id} is not native"))?;
        for primary in weld.primary_skill_ids {
            let element = match primary {
                8 => "ether",
                16 => "fire",
                24 => "air",
                32 => "water",
                40 => "earth",
                _ => return Err(format!("ML bot policy weld primary {primary} has no element")),
            };
            values.set(&format!("weld_element_{element}"), 1.0);
        }
        values.set("weld_build_index_scaled", scaled((weld.id - 1000) as f64, 10.0));
        values.identity("weld_build_id", weld.id as usize)?;
    }
    let property = |name: &str| stats.and_then(|row| row.numeric_properties.get(name));
    let ranked_property = |name: &str| ranked(property(name), input.target_rank);
    for (name, value, scale) in [
        ("mana_cost", ranked_property("mManaCost"), scales::MANA),
        (
            "damage_min",
            ranked(property("mDamage1").or_else(|| property("mDamage")), input.target_rank),
            scales::SKILL_DAMAGE,
        ),
        (
            "damage_max",
            ranked(property("mDamage2").or_else(|| property("mDamage")), input.target_rank),
            scales::SKILL_DAMAGE,
        ),
        ("range", ranked_property("mRange"), scales::RANGE),
        ("cooldown", ranked_property("mCooldown"), scales::COOLDOWN_SECONDS),
        ("radius", ranked_property("mRadius"), scales::SKILL_RADIUS),
        (
            "duration",
            ranked(property("mDuration").or_else(|| property("mSeconds")), input.target_rank),
            scales::SKILL_DURATION_SECONDS,
        ),
        ("value", ranked_property("mValue"), scales::SKILL_VALUE),
        ("concentration", ranked_property("mConcentration"), scales::SKILL_CONCENTRATION),
        ("chance", ranked_property("mChance"), scales::SKILL_CHANCE),
        ("quantity", ranked_property("mQuantity"), scales::SKILL_QUANTITY),
        ("strength", ranked_property("mStrength"), scales::SKILL_STRENGTH),
        ("absorb", ranked_property("mAbsorb"), scales::SKILL_ABSORB),
        ("arcs", ranked_property("mArcs"), scales::SKILL_ARCS),
        ("armor_plus", ranked_property("mArmorPlus"), scales::SKILL_ARMOR_PLUS),
        ("charges", ranked_property("mCharges"), scales::SKILL_CHARGES),
        ("flee", ranked_property("mFlee"), scales::SKILL_FLEE),
        ("fragments", ranked_property("mFragments"), scales::SKILL_FRAGMENTS),
        ("freeze", ranked_property("mFreeze"), scales::SKILL_FREEZE),
        ("hp", ranked_property("mHP"), scales::SKILL_HP),
        ("hoard", ranked_property("mHoard"), scales::SKILL_HOARD),
        ("loss", ranked_property("mLoss"), scales::SKILL_LOSS),
        ("max_armor", ranked_property("mMaxArmor"), scales::SKILL_MAX_ARMOR),
        ("percent", ranked_property("mPercent"), scales::SKILL_PERCENT),
        ("pierces", ranked_property("mPierces"), scales::SKILL_PIERCES),
        ("pushback", ranked_property("mPushback"), scales::SKILL_PUSHBACK),
        ("reflect", ranked_property("mReflect"), scales::SKILL_REFLECT),
        ("size", ranked_property("mSize"), scales::SKILL_SIZE),
        ("slow", ranked_property("mSlow"), scales::SKILL_SLOW),
        ("slowdown", ranked_property("mSlowdown"), scales::SKILL_SLOWDOWN),
        ("speed", ranked_property("mSpeed"), scales::SKILL_SPEED),
        ("speed_up", ranked_property("mSpeedUp"), scales::SKILL_SPEED_UP),
        ("stun_amount", ranked_property("mStunAmount"), scales::SKILL_STUN_AMOUNT),
        ("to_hit", ranked_property("mToHit"), scales::SKILL_TO_HIT),
        ("weaken", ranked_property("mWeaken"), scales::SKILL_WEAKEN),
        ("widen", ranked_property("mWiden"), scales::SKILL_WIDEN),
    ] {
        values.mechanical(name, value, scale);
    }
    Ok(values.0.to_vec())
}

fn ranked(value: Option<&NativeSkillStatBookValue>, rank: f64) -> Option<f64> {
    let value = match value? {
        NativeSkillStatBookValue::Scalar(value) => *value,
        NativeSkillStatBookValue::Ranks(values) => {
            if values.is_empty() || rank.is_nan() {
                return None;
            }
            let index = rank.max(0.0).min((values.len() - 1) as f64);
            // JavaScript array lookup at a fractional property is undefined. Only
            // the clamping endpoints can turn an out-of-range fraction into a row.
            if index.fract() != 0.0 {
                return None;
            }
            values[index as usize]
        }
    };
    value.is_finite().then_some(value)
}

pub fn ml_bot_policy_skill_coverage_key(id: usize, weld_build_id: Option<u32>) -> String {
    if let Some(weld) =
        weld_build_id.and_then(|id| NATIVE_WELD_BUILDS.iter().find(|weld| weld.id == id))
    {
        format!("weld:{}", weld.id)
    } else {
        format!("family:{}", policy_skill_catalog_row(id).map_or("unknown", |row| row.family))
    }
}

fn policy_skill_catalog_row(id: usize) -> Option<&'static NativeSkillDefinition> {
    // Source appends Nullskill (82) only to playerStatBook, after the 82 catalog
    // skills. The Rust table also retains that sentinel, but it is not a known
    // catalog skill for descriptor membership or coverage-family selection.
    NATIVE_SKILL_CATALOG.get(id).filter(|_| id < 82)
}

pub fn describe_ml_bot_policy_skill_offer(
    session: &GameSession,
    player_id: &str,
) -> Result<Option<MlBotPolicySkillOfferDescription>, String> {
    let participant = session
        .participant(player_id)
        .ok_or_else(|| format!("ML bot policy has no player {player_id}"))?;
    let player = &participant.player;
    let Some(offer) = &player.progression.pending_offer else {
        return Ok(None);
    };
    if offer.options.is_empty() {
        return Ok(None);
    }
    let mut descriptors = Vec::with_capacity(offer.options.len() * OPTION_DESCRIPTOR_WIDTH);
    let mut option_ids = Vec::new();
    let mut coverage_keys = Vec::new();
    for option in &offer.options {
        let permanent = player.book.permanent_ranks.get(option.skill_id).copied().unwrap_or(0);
        descriptors.extend(describe_ml_bot_policy_skill(
            &player.book,
            &player.stat_book,
            MlBotPolicySkillDescriptorInput {
                apply_count: f64::from(option.target_rank.saturating_sub(permanent)),
                skill_id: option.skill_id,
                target_rank: f64::from(option.target_rank),
                weld_build_id: option.weld_build_id,
            },
        )?);
        option_ids.push(option.skill_id);
        coverage_keys.push(ml_bot_policy_skill_coverage_key(option.skill_id, option.weld_build_id));
    }
    Ok(Some(MlBotPolicySkillOfferDescription {
        coverage_keys,
        descriptors,
        generation: offer.sequence,
        mask: vec![1; option_ids.len()],
        option_ids,
    }))
}

pub fn observe_ml_bot_policy_skill_loadout(
    session: &GameSession,
    player_id: &str,
) -> Result<Vec<f32>, String> {
    let participant = session
        .participant(player_id)
        .ok_or_else(|| format!("ML bot policy has no player {player_id}"))?;
    let player = &participant.player;
    let mut values = vec![0.0; 9 * OPTION_DESCRIPTOR_WIDTH];
    let quickbar = player.belt.skill_projection();
    for (row, id) in std::iter::once(Some(player.book.primary_skill_id)).chain(quickbar).enumerate()
    {
        let Some(id) = id else {
            continue;
        };
        let descriptor = describe_ml_bot_policy_skill(
            &player.book,
            &player.stat_book,
            MlBotPolicySkillDescriptorInput {
                apply_count: 0.0,
                skill_id: id,
                target_rank: f64::from(player.book.effective_ranks.get(id).copied().unwrap_or(0)),
                weld_build_id: if id == 52 { player.book.weld_build_id } else { None },
            },
        )?;
        values[row * OPTION_DESCRIPTOR_WIDTH..(row + 1) * OPTION_DESCRIPTOR_WIDTH]
            .copy_from_slice(&descriptor);
    }
    Ok(values)
}

/// The host controller dispatches this selection through the canonical command path.
/// Source's scripted chooser computes its temporary applied state before dispatch; no
/// world/RNG ownership is transferred to this read-only priority calculation.
pub fn choose_ml_bot_policy_scripted_skill(
    session: &GameSession,
    player_id: &str,
) -> Result<Option<PlayerSkillChoice>, String> {
    let participant = session
        .participant(player_id)
        .ok_or_else(|| format!("ML bot policy has no player {player_id}"))?;
    let player = &participant.player;
    let Some(offer) = &player.progression.pending_offer else {
        return Ok(None);
    };
    if offer.options.is_empty() {
        return Ok(None);
    }
    let band_start = match player.selection.element {
        WizardElement::Air => 24,
        WizardElement::Earth => 40,
        WizardElement::Ether => 8,
        WizardElement::Fire => 16,
        WizardElement::Water => 32,
    };
    let mut selected = None;
    let mut best_priority = usize::MAX;
    for (index, option) in offer.options.iter().enumerate() {
        let id = option.skill_id;
        let priority = if id == 52 && option.weld_build_id.is_some() {
            let eligible = NATIVE_WELD_BUILDS
                .iter()
                .find(|weld| Some(weld.id) == option.weld_build_id)
                .is_some_and(|weld| {
                    weld.primary_skill_ids.iter().all(|id| player.book.effective_ranks[*id] > 0)
                });
            if eligible { 0 } else { usize::MAX }
        } else if (band_start..=band_start + 7).contains(&id) {
            if id == band_start { 1 } else { 10 + id - band_start }
        } else if id == 64 {
            30
        } else if [8, 16, 24, 32, 40].contains(&id) {
            if player.book.effective_ranks[id] == 0 { 50 + index } else { usize::MAX }
        } else {
            100 + index
        };
        if priority < best_priority {
            selected = Some(index);
            best_priority = priority;
        }
    }
    let index =
        selected.ok_or_else(|| format!("ML bot policy found no eligible skill for {player_id}"))?;
    Ok(Some(PlayerSkillChoice {
        choice_index: index,
        offer_sequence: offer.sequence,
        skill_id: offer.options[index].skill_id,
    }))
}

fn scaled(value: f64, scale: f64) -> f64 {
    (value / scale).clamp(0.0, 1.0)
}
