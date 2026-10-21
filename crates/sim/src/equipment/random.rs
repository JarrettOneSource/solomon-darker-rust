//! Random item effects and names from Website `core-kernels/native-random-equipment.ts`.
//!
//! Selection retries and discarded draws are observable in the shared loot stream.
//! `loot-goldens.json` replays all equipment types, tiers and advanced-skill pools.

use crate::NativeRng;
use crate::economy::EquipmentType;
use crate::float::fround;
use crate::profile::{NATIVE_SKILL_CATALOG, native_skill_rule};

use super::NativeEquipmentEffect;

const PRIMARY_TARGET_SKILL_IDS: [usize; 18] =
    [8, 11, 15, 16, 21, 22, 23, 27, 29, 32, 40, 50, 52, 55, 65, 72, 73, 74];
const DISCIPLINE_TARGET_SKILL_IDS: [usize; 14] =
    [57, 58, 59, 60, 61, 62, 63, 65, 66, 67, 68, 69, 70, 71];

#[derive(Clone, Debug, PartialEq)]
pub struct NativeRandomEquipmentEffects {
    pub effects: Vec<NativeEquipmentEffect>,
    pub item_level: u32,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeRandomEquipmentSkillPools {
    pub all: Vec<usize>,
    pub primary: Vec<usize>,
    pub primary_or_discipline: Vec<usize>,
}

#[must_use]
pub fn native_random_equipment_skill_pools(
    advanced_unlocks: &[bool; 8],
) -> NativeRandomEquipmentSkillPools {
    let available = |id: usize| id < 72 || advanced_unlocks[id - 72];
    let all = (8..80).filter(|id| available(*id)).collect();
    let primary: Vec<_> =
        PRIMARY_TARGET_SKILL_IDS.into_iter().filter(|id| available(*id)).collect();
    let mut primary_or_discipline = primary.clone();
    primary_or_discipline.extend_from_slice(&DISCIPLINE_TARGET_SKILL_IDS);
    primary_or_discipline.sort_unstable();
    primary_or_discipline.dedup();
    NativeRandomEquipmentSkillPools { all, primary, primary_or_discipline }
}

/// Mutates only the caller's shared native stream; `-1` is the retail starter-item sentinel.
#[must_use]
pub fn generate_native_random_equipment_effects(
    rng: &mut NativeRng,
    equipment_type: EquipmentType,
    level: i32,
    advanced_unlocks: &[bool; 8],
) -> NativeRandomEquipmentEffects {
    assert!(level >= -1, "random equipment level uses -1 or a non-negative level");
    let native_level = if level == -1 { 1 } else { level };
    let minimum = 1.max(native_level / 2);
    let maximum = 1.max(native_level + 5);
    if minimum != maximum {
        rng.draw_integer(maximum - minimum + 1);
    }
    let mut effect_count = 1;
    if native_level > 18 {
        if rng.draw_integer(2) == 1 {
            effect_count = 2;
        }
        if effect_count == 1 && rng.draw_integer(5) == 3 {
            effect_count = 2;
        }
        if effect_count == 1 && rng.draw_integer(10) == 3 {
            effect_count = 2;
        }
    }
    let tier_draw = rng.draw_integer(native_level / 20);
    let tier = if level == -1 { 0 } else { (tier_draw + 1).clamp(1, 3) as usize };
    let name_side = rng.draw_integer(2);
    let mut prefix_next = effect_count != 2 && name_side == 1;
    let pools = native_random_equipment_skill_pools(advanced_unlocks);
    let mut item_level = if effect_count == 2 { 8 } else { 0 };
    let mut prior_selector = None;
    let mut effects = Vec::with_capacity(effect_count);
    let mut prefix = None;
    let mut suffix = None;
    while effects.len() < effect_count {
        prefix_next = !prefix_next;
        let selector = loop {
            let selector = rng.draw_integer(25) as u32;
            if Some(selector) == prior_selector
                || (is_wearable(equipment_type)
                    && [2, 3, 8, 9, 12, 13, 17, 21, 22].contains(&selector))
                || (selector == 6 && effect_count != 1)
                || (level == -1
                    && ![1, 4, 6, 7, 8, 11, 14, 15, 16, 18, 19, 22, 23, 24].contains(&selector))
            {
                continue;
            }
            break selector;
        };
        prior_selector = Some(selector);
        let generated = generate_effect(rng, selector, tier, equipment_type, &pools);
        item_level = item_level.max(generated.item_level);
        if prefix_next {
            prefix = Some(generated.prefix);
        } else {
            suffix = Some(generated.suffix);
        }
        effects.push(generated.effect);
    }
    let base_name = match equipment_type {
        EquipmentType::Amulet => "Amulet",
        EquipmentType::Hat => "Hat",
        EquipmentType::Ring => "Ring",
        EquipmentType::Robe => "Robe",
        EquipmentType::Staff => "Staff",
        EquipmentType::Wand => "Wand",
    };
    let mut name = String::new();
    if let Some(prefix) = prefix {
        name.push_str(&prefix);
        name.push(' ');
    }
    name.push_str(base_name);
    if let Some(suffix) = suffix {
        name.push(' ');
        name.push_str(&suffix);
    }
    NativeRandomEquipmentEffects { effects, item_level, name }
}

struct GeneratedEffect {
    effect: NativeEquipmentEffect,
    item_level: u32,
    prefix: String,
    suffix: String,
}

fn amount(rng: &mut NativeRng, bound: i32, addend: i32) -> f64 {
    f64::from(rng.draw_integer(bound) + addend)
}

fn by_tier(rng: &mut NativeRng, tier: usize, rows: [(i32, i32); 4]) -> f64 {
    let (bound, addend) = rows[tier];
    amount(rng, bound, addend)
}

fn skill_target(rng: &mut NativeRng, pool: &[usize]) -> u32 {
    pool[rng.draw_integer(pool.len() as i32) as usize] as u32
}

fn is_wearable(equipment_type: EquipmentType) -> bool {
    matches!(equipment_type, EquipmentType::Hat | EquipmentType::Robe)
}

fn generate_effect(
    rng: &mut NativeRng,
    selector: u32,
    tier: usize,
    equipment_type: EquipmentType,
    pools: &NativeRandomEquipmentSkillPools,
) -> GeneratedEffect {
    let mut effect =
        NativeEquipmentEffect { kind: selector, magnitude: 0.0, operator: 0, target: 0 };
    let mut item_level = 0;
    effect.magnitude = match selector {
        0 => {
            effect.kind = 9;
            effect.operator = 2;
            by_tier(rng, tier, [(6, 2), (6, 25), (26, 50), (26, 100)])
        }
        1 => {
            effect.kind = 9;
            by_tier(rng, tier, [(2, 1), (3, 1), (5, 5), (5, 10)])
        }
        2 => {
            effect.kind = 1;
            effect.operator = 2;
            by_tier(rng, tier, [(4, 3), (11, 10), (11, 30), (11, 50)])
        }
        3 => {
            effect.kind = 1;
            if tier == 0 { 1.0 } else { by_tier(rng, tier, [(1, 1), (2, 2), (2, 3), (3, 5)]) }
        }
        4 | 5 => {
            effect.kind = 2;
            let magnitude = if selector == 4 {
                effect.operator = 2;
                by_tier(rng, tier, [(6, 4), (11, 15), (11, 35), (11, 60)])
            } else if tier == 0 {
                1.0
            } else {
                by_tier(rng, tier, [(1, 1), (3, 3), (2, 6), (2, 8)])
            };
            effect.target = rng.draw_integer(5) as u32;
            magnitude
        }
        6 => {
            effect.kind = 25;
            let magnitude = by_tier(rng, tier, [(2, 1), (4, 3), (3, 7), (2, 10)]);
            effect.target = skill_target(rng, &pools.primary);
            magnitude
        }
        7 => {
            effect.kind = 3;
            by_tier(rng, tier, [(2, 1), (3, 4), (4, 8), (6, 15)])
        }
        8 => {
            effect.kind = 4;
            effect.target = skill_target(rng, &pools.primary_or_discipline);
            item_level = native_skill_rule(effect.target as usize)
                .expect("generated skill targets have native level rules")
                .minimum_level;
            [1.0, 1.0, 2.0, 4.0][tier]
        }
        9 => {
            effect.kind = 5;
            effect.target = skill_target(rng, &pools.all);
            [1.0, 1.0, 2.0, 3.0][tier]
        }
        10 => {
            let magnitude = if rng.draw_integer(2) == 1 {
                effect.operator = 2;
                by_tier(rng, tier, [(4, 2), (6, 10), (6, 20), (11, 30)])
            } else {
                amount(rng, 2, 1);
                if tier == 0 { 1.0 } else { by_tier(rng, tier, [(1, 1), (3, 2), (3, 5), (3, 8)]) }
            };
            -magnitude
        }
        11 => {
            let magnitude = if rng.draw_integer(2) == 1 {
                effect.operator = 2;
                by_tier(rng, tier, [(3, 5), (6, 15), (6, 25), (11, 35)])
            } else {
                by_tier(rng, tier, [(2, 1), (5, 2), (5, 5), (7, 8)])
            };
            effect.target = rng.draw_integer(5) as u32;
            -magnitude
        }
        12 => {
            effect.operator = 2;
            by_tier(rng, tier, [(5, 2), (6, 10), (11, 30), (26, 50)])
        }
        13 => {
            effect.operator = 2;
            let magnitude = by_tier(rng, tier, [(6, 5), (6, 20), (11, 50), (26, 70)]);
            effect.target = rng.draw_integer(5) as u32;
            magnitude
        }
        14 => {
            effect.operator = 2;
            by_tier(rng, tier, [(6, 5), (11, 10), (26, 50), (26, 100)])
        }
        15 => {
            effect.operator = 1;
            amount(rng, 4, 2);
            if tier == 0 { 2.0 } else { by_tier(rng, tier, [(1, 2), (3, 2), (3, 4), (4, 6)]) }
        }
        16 => {
            effect.operator = 1;
            if tier == 0 { 1.5 } else { by_tier(rng, tier, [(1, 1), (3, 2), (26, 50), (26, 100)]) }
        }
        17 => {
            effect.operator = 2;
            amount(rng, 11, 5);
            by_tier(rng, tier, [(6, 5), (6, 10), (6, 20), (11, 30)])
        }
        18..=20 => {
            effect.operator = 2;
            if selector == 18 {
                amount(rng, 11, 5);
            }
            by_tier(rng, tier, [(6, 2), (6, 5), (11, 20), (16, 40)])
        }
        21 => {
            effect.operator = 2;
            amount(rng, 6, 5);
            by_tier(rng, tier, [(4, 3), (6, 10), (6, 20), (16, 30)])
        }
        22 => {
            effect.operator = 2;
            amount(rng, 6, 5);
            let magnitude = by_tier(rng, tier, [(6, 5), (6, 20), (6, 30), (11, 40)]);
            effect.target = rng.draw_integer(5) as u32;
            magnitude
        }
        23 => by_tier(rng, tier, [(11, 5), (26, 25), (76, 75), (101, 200)]),
        24 => by_tier(rng, tier, [(11, 10), (51, 50), (151, 150), (201, 400)]),
        _ => unreachable!("native random item selector is in0..25"),
    };
    if is_wearable(equipment_type)
        && [0, 1, 2, 3, 4, 5, 6, 10, 11, 12, 13, 17, 18, 19, 20, 21, 22, 23, 24].contains(&selector)
        && effect.magnitude > 2.0
    {
        effect.magnitude = (effect.magnitude * 0.5).round_ties_even();
    }
    effect.magnitude = fround(effect.magnitude);
    let (prefix, suffix) = effect_names(selector);
    if selector == 9 {
        let skill_name = NATIVE_SKILL_CATALOG[effect.target as usize].name;
        return GeneratedEffect {
            effect,
            item_level,
            prefix: skill_name.to_owned(),
            suffix: format!("of {skill_name}"),
        };
    }
    GeneratedEffect { effect, item_level, prefix: prefix.to_owned(), suffix: suffix.to_owned() }
}

fn effect_names(selector: u32) -> (&'static str, &'static str) {
    match selector {
        0 => ("Channeling", "of Channeling"),
        1 => ("Managrinding", "of Managrind"),
        2 | 4 => ("Searing", "of Searing"),
        3 | 5 => ("Arcane", "of Arcane Pain"),
        6 => ("Wielding", "of Wielding"),
        7 => ("Brutal", "of Brutality"),
        8 => ("Ingenious", "of Ingenuity"),
        10 => ("Spellbinding", "of Spellbinding"),
        11 => ("Elemental", "of the Elementalist"),
        12 => ("Nimble", "of Nimble Fingers"),
        13 => ("Cunning", "of Cunning"),
        14 => ("Wealthy", "of Wealth"),
        15 => ("Calling", "of Calling"),
        16 => ("Recovering", "of Recovery"),
        17 => ("Hasty", "of Haste"),
        18 => ("Manly", "of Manliness"),
        19 => ("Spellbreaking", "of Spellbreaking"),
        20 => ("Curing", "of Remedy"),
        21 | 22 => ("Convergent", "of Convergence"),
        23 => ("Fit", "of Fitness"),
        24 => ("Thoughtful", "of Thought"),
        _ => ("Equipment", "of Equipment"),
    }
}
