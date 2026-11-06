//! Equipment effects: how equipped items and completed sets change a wizard's skill ranks and
//! stat modifiers.

mod catalog;
pub mod random;

pub use catalog::EQUIPMENT_SETS;

use crate::economy::{EQUIPMENT_RECIPES, HubEquipmentState, HubInventoryItem};
use crate::float::fround;
use crate::profile::{SKILL_CATALOG, SKILL_ROW_COUNT, skill_root};

/// One equipment effect: its `kind`, the skill, class or stat row it targets, and its
/// `magnitude`. For most kinds, `operator` selects a flat, multiplier or percentage change.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct EquipmentEffect {
    pub kind: u32,
    pub magnitude: f64,
    pub operator: u32,
    pub target: u32,
}

/// An accumulated stat transform, `value * scale + offset` with single-precision rounding.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct EquipmentScalarTransform {
    pub offset: f64,
    pub scale: f64,
}

impl Default for EquipmentScalarTransform {
    fn default() -> Self {
        Self { offset: 0.0, scale: 1.0 }
    }
}

impl EquipmentScalarTransform {
    /// Applies the transform. The identity returns its input bit for bit, without rounding.
    #[must_use]
    pub fn apply(self, value: f64) -> f64 {
        assert!(value.is_finite(), "an equipment transform receives a finite stat");
        if self.scale == 1.0 && self.offset == 0.0 {
            return value;
        }
        fround(fround(value * self.scale) + self.offset)
    }
}

/// Independent additive/multiplicative lanes, in native class and skill row order.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct EquipmentModifiers {
    pub cast_speed_flat: f64,
    pub cast_speed_multiplier: f64,
    pub class_cast_speed_flat: [f64; 8],
    pub class_cast_speed_multiplier: [f64; 8],
    pub class_damage_flat: [f64; 8],
    pub class_damage_multiplier: [f64; 8],
    pub class_mana_cost_flat: [f64; 8],
    pub class_mana_cost_multiplier: [f64; 8],
    pub class_recharge: [EquipmentScalarTransform; 8],
    pub damage_resistance: f64,
    pub feature_bits: u32,
    pub global_damage_flat: f64,
    pub global_damage_multiplier: f64,
    pub global_mana_cost_flat: f64,
    pub global_mana_cost_multiplier: f64,
    pub gold_multiplier: f64,
    pub health_recovery: EquipmentScalarTransform,
    pub magic_resistance: f64,
    pub mana_recovery: EquipmentScalarTransform,
    pub maximum_health: EquipmentScalarTransform,
    pub maximum_mana: EquipmentScalarTransform,
    pub melee_damage_flat: f64,
    pub melee_damage_multiplier: f64,
    pub orb_pull_multiplier: f64,
    pub poison_resistance: f64,
    pub recharge: EquipmentScalarTransform,
    #[cfg_attr(feature = "persistence", serde(with = "crate::persistence::fixed_array"))]
    pub skill_damage_flat: [f64; SKILL_ROW_COUNT],
    #[cfg_attr(feature = "persistence", serde(with = "crate::persistence::fixed_array"))]
    pub skill_damage_multiplier: [f64; SKILL_ROW_COUNT],
    pub walk_speed: EquipmentScalarTransform,
    pub weld_effect: f64,
}

impl Default for EquipmentModifiers {
    fn default() -> Self {
        Self {
            cast_speed_flat: 0.0,
            cast_speed_multiplier: 1.0,
            class_cast_speed_flat: [0.0; 8],
            class_cast_speed_multiplier: [1.0; 8],
            class_damage_flat: [0.0; 8],
            class_damage_multiplier: [1.0; 8],
            class_mana_cost_flat: [0.0; 8],
            class_mana_cost_multiplier: [1.0; 8],
            class_recharge: [EquipmentScalarTransform::default(); 8],
            damage_resistance: 0.0,
            feature_bits: 0,
            global_damage_flat: 0.0,
            global_damage_multiplier: 1.0,
            global_mana_cost_flat: 0.0,
            global_mana_cost_multiplier: 1.0,
            gold_multiplier: 1.0,
            health_recovery: EquipmentScalarTransform::default(),
            magic_resistance: 0.0,
            mana_recovery: EquipmentScalarTransform::default(),
            maximum_health: EquipmentScalarTransform::default(),
            maximum_mana: EquipmentScalarTransform::default(),
            melee_damage_flat: 0.0,
            melee_damage_multiplier: 1.0,
            orb_pull_multiplier: 1.0,
            poison_resistance: 0.0,
            recharge: EquipmentScalarTransform::default(),
            skill_damage_flat: [0.0; SKILL_ROW_COUNT],
            skill_damage_multiplier: [1.0; SKILL_ROW_COUNT],
            walk_speed: EquipmentScalarTransform::default(),
            weld_effect: 1.0,
        }
    }
}

/// An authored equipment set: its member recipes and the bonus effects granted while all of
/// them are equipped.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EquipmentSet {
    pub name: &'static str,
    pub member_recipe_indices: &'static [usize],
    pub effects: &'static [EquipmentEffect],
}

/// The effects contributed by one equipped item (with its recipe index, if any) or by all
/// completed sets together.
#[derive(Clone, Debug, PartialEq)]
pub struct EquipmentEffectSource {
    pub effects: Vec<EquipmentEffect>,
    pub recipe_index: Option<usize>,
}

/// Resolved equipment: effective skill ranks, the skills equipment newly granted in grant
/// order, and the stat modifiers.
#[derive(Clone, Debug, PartialEq)]
pub struct EquipmentResolution {
    pub effective_ranks: [u32; SKILL_ROW_COUNT],
    pub granted_skill_order: Vec<usize>,
    pub modifiers: EquipmentModifiers,
}

/// The sets whose member recipes are all equipped, in authored catalog order.
#[must_use]
pub fn equipment_completed_sets(equipped_recipe_indices: &[usize]) -> Vec<&'static EquipmentSet> {
    let mut completed = Vec::new();
    for set in &EQUIPMENT_SETS {
        if set.member_recipe_indices.iter().all(|index| equipped_recipe_indices.contains(index)) {
            completed.push(set);
        }
    }
    completed
}

#[must_use]
pub fn equipment_recipe_effects(recipe_index: usize) -> &'static [EquipmentEffect] {
    EQUIPMENT_RECIPES.get(recipe_index).map_or(&[], |recipe| recipe.effects)
}

#[must_use]
pub fn equipment_recipe_description(recipe_index: usize) -> &'static str {
    EQUIPMENT_RECIPES.get(recipe_index).map_or("", |recipe| recipe.description)
}

#[must_use]
pub fn resolve_equipped_effects(
    permanent_ranks: &[u32; SKILL_ROW_COUNT],
    equipment: &HubEquipmentState,
    owned_hagatha_selectors: &[u32],
) -> EquipmentResolution {
    let mut sources = Vec::new();
    // Native equipment iteration is hat, robe, rings, amulet, weapon; f32 stores make this
    // order observable.
    for item in [
        &equipment.hat,
        &equipment.robe,
        &equipment.rings[0],
        &equipment.rings[1],
        &equipment.rings[2],
        &equipment.amulet,
        &equipment.weapon,
    ]
    .into_iter()
    .flatten()
    {
        sources.push(effect_source(item));
    }
    resolve_equipment_effects(permanent_ranks, &sources, owned_hagatha_selectors)
}

#[must_use]
pub fn resolve_equipment_effects(
    permanent_ranks: &[u32; SKILL_ROW_COUNT],
    sources: &[EquipmentEffectSource],
    owned_hagatha_selectors: &[u32],
) -> EquipmentResolution {
    let mut sources = sources.to_vec();
    let equipped_recipes: Vec<_> =
        sources.iter().filter_map(|source| source.recipe_index).collect();
    let mut set_effects = Vec::new();
    for set in equipment_completed_sets(&equipped_recipes) {
        set_effects.extend_from_slice(set.effects);
    }
    if !set_effects.is_empty() {
        sources.push(EquipmentEffectSource { effects: set_effects, recipe_index: None });
    }
    let mut effective_ranks = *permanent_ranks;
    let mut granted_skill_order = Vec::new();
    // The native grant pass follows every source without kind4 before every source containing it.
    for has_grant in [false, true] {
        for source in &sources {
            if source.effects.iter().any(|effect| effect.kind == 4) != has_grant {
                continue;
            }
            for effect in &source.effects {
                let target = effect.target as usize;
                let previous_rank = effective_ranks.get(target).copied().unwrap_or(0);
                apply_skill_effect(
                    &mut effective_ranks,
                    permanent_ranks,
                    *effect,
                    owned_hagatha_selectors,
                );
                // Native temporary acquisition records follow the actual FX pass,
                // independently of save-facing permanent learning.
                if matches!(effect.kind, 4 | 7)
                    && (8..=79).contains(&target)
                    && previous_rank == 0
                    && effective_ranks[target] > 0
                {
                    granted_skill_order.push(target);
                }
            }
        }
    }
    let mut modifiers = EquipmentModifiers::default();
    for source in &sources {
        for effect in &source.effects {
            apply_stat_effect(&mut modifiers, *effect);
        }
    }
    EquipmentResolution { effective_ranks, granted_skill_order, modifiers }
}

fn effect_source(item: &HubInventoryItem) -> EquipmentEffectSource {
    let effects = match &item.native_effects {
        Some(effects) => effects.clone(),
        None => item
            .recipe_index
            .map_or_else(Vec::new, |index| equipment_recipe_effects(index).to_vec()),
    };
    EquipmentEffectSource { effects, recipe_index: item.recipe_index }
}

fn apply_skill_effect(
    ranks: &mut [u32; SKILL_ROW_COUNT],
    permanent: &[u32; SKILL_ROW_COUNT],
    effect: EquipmentEffect,
    owned: &[u32],
) {
    if !(4..=8).contains(&effect.kind) {
        return;
    }
    let magnitude = (effect.magnitude + 0.5).floor() as i64;
    let target = effect.target as usize;
    match effect.kind {
        4 => set_granted_rank(ranks, target, magnitude, owned),
        5 => add_learned_rank(ranks, target, magnitude),
        6 => {
            for skill_id in 8..=79 {
                if skill_root(skill_id) == Some(target) {
                    add_learned_rank(ranks, skill_id, magnitude);
                }
            }
        }
        7 => {
            if ranks[target] < 1 {
                set_granted_rank(ranks, target, magnitude, owned);
            } else {
                add_learned_rank(ranks, target, magnitude);
            }
        }
        8 => {
            for (skill_id, rank) in permanent.iter().enumerate().take(80).skip(8) {
                if *rank > 0 {
                    add_learned_rank(ranks, skill_id, magnitude);
                }
            }
        }
        _ => unreachable!("the native skill-effect kind is in4..8"),
    }
}

fn set_granted_rank(
    ranks: &mut [u32; SKILL_ROW_COUNT],
    skill_id: usize,
    magnitude: i64,
    owned: &[u32],
) {
    let current = ranks[skill_id];
    let maximum = SKILL_CATALOG[skill_id].maximum_level;
    let granted = i64::from(current).max(magnitude).min(i64::from(maximum));
    assert!(granted >= 0, "an authored skill grant produces a non-negative rank");
    ranks[skill_id] = granted as u32;
    if i64::from(current) <= magnitude && owned.contains(&6) {
        ranks[skill_id] = ranks[skill_id].max(2);
    }
}

fn add_learned_rank(ranks: &mut [u32; SKILL_ROW_COUNT], skill_id: usize, magnitude: i64) {
    let current = ranks[skill_id];
    if current < 1 {
        return;
    }
    let rank = i64::from(current) + magnitude;
    assert!(rank >= 0, "an authored skill modifier produces a non-negative rank");
    ranks[skill_id] = SKILL_CATALOG[skill_id].maximum_level.min(rank as u32);
}

fn apply_stat_effect(target: &mut EquipmentModifiers, effect: EquipmentEffect) {
    let magnitude = effect.magnitude;
    let index = effect.target as usize;
    match effect.kind {
        1 => split_effect(
            &mut target.global_damage_multiplier,
            &mut target.global_damage_flat,
            effect,
        ),
        2 => split_effect(
            &mut target.class_damage_multiplier[index],
            &mut target.class_damage_flat[index],
            effect,
        ),
        3 => {
            split_effect(&mut target.melee_damage_multiplier, &mut target.melee_damage_flat, effect)
        }
        9 => append_transform(&mut target.mana_recovery, effect),
        10 => split_effect(
            &mut target.global_mana_cost_multiplier,
            &mut target.global_mana_cost_flat,
            effect,
        ),
        11 => split_effect(
            &mut target.class_mana_cost_multiplier[index],
            &mut target.class_mana_cost_flat[index],
            effect,
        ),
        12 => split_effect(&mut target.cast_speed_multiplier, &mut target.cast_speed_flat, effect),
        13 => split_effect(
            &mut target.class_cast_speed_multiplier[index],
            &mut target.class_cast_speed_flat[index],
            effect,
        ),
        14 => {
            target.gold_multiplier = fround(target.gold_multiplier * percent_multiplier(magnitude))
        }
        15 => {
            target.orb_pull_multiplier =
                fround(target.orb_pull_multiplier * operator_multiplier(effect))
        }
        16 => append_transform(&mut target.health_recovery, effect),
        17 => {
            if effect.operator == 0 {
                target.walk_speed.offset = fround(target.walk_speed.offset + magnitude / 10.0);
            } else {
                append_transform(&mut target.walk_speed, effect);
            }
        }
        18 => target.damage_resistance = fround(target.damage_resistance + magnitude / 100.0),
        19 => target.magic_resistance = fround(target.magic_resistance + magnitude / 100.0),
        20 => target.poison_resistance = fround(target.poison_resistance + magnitude / 100.0),
        21 => append_transform(&mut target.recharge, effect),
        22 => append_transform(&mut target.class_recharge[index], effect),
        23 => append_transform(&mut target.maximum_health, effect),
        24 => append_transform(&mut target.maximum_mana, effect),
        25 => split_effect(
            &mut target.skill_damage_multiplier[index],
            &mut target.skill_damage_flat[index],
            effect,
        ),
        26..=37 => target.feature_bits |= 1 << (effect.kind - 26),
        38 => {
            target.weld_effect = match effect.operator {
                0 => fround(target.weld_effect + magnitude),
                1 => fround(target.weld_effect * magnitude),
                _ => fround(target.weld_effect + magnitude / 100.0),
            };
        }
        39 => target.feature_bits |= 0x1000,
        _ => {}
    }
}

fn split_effect(multiplier: &mut f64, flat: &mut f64, effect: EquipmentEffect) {
    if effect.operator == 0 {
        *flat = fround(*flat + effect.magnitude);
    } else {
        *multiplier = fround(*multiplier * operator_multiplier(effect));
    }
}

fn append_transform(transform: &mut EquipmentScalarTransform, effect: EquipmentEffect) {
    if effect.operator == 0 {
        transform.offset = fround(transform.offset + effect.magnitude);
        return;
    }
    let multiplier = operator_multiplier(effect);
    transform.offset = fround(transform.offset * multiplier);
    transform.scale = fround(transform.scale * multiplier);
}

fn operator_multiplier(effect: EquipmentEffect) -> f64 {
    if effect.operator == 1 { effect.magnitude } else { percent_multiplier(effect.magnitude) }
}

fn percent_multiplier(magnitude: f64) -> f64 {
    fround(1.0 + magnitude / 100.0)
}
