//! Canonical widths, descriptor order and normalization scales from Source `spec.ts`.

mod observation_names;
pub use observation_names::OBSERVATION_NAMES;

pub const OPTION_DESCRIPTOR_NAMES: [&str; 138] = [
    "present",
    "option_id_index_scaled",
    "skill_id_bit_0",
    "skill_id_bit_1",
    "skill_id_bit_2",
    "skill_id_bit_3",
    "skill_id_bit_4",
    "skill_id_bit_5",
    "skill_id_bit_6",
    "skill_id_bit_7",
    "skill_id_bit_8",
    "skill_id_bit_9",
    "skill_id_bit_10",
    "skill_id_bit_11",
    "skill_id_bit_12",
    "skill_id_bit_13",
    "skill_id_bit_14",
    "skill_id_bit_15",
    "catalog_known",
    "apply_count_scaled",
    "learned_rank_scaled",
    "effective_rank_scaled",
    "cap_rank_scaled",
    "max_rank_scaled",
    "band_index_scaled",
    "family_element",
    "family_discipline",
    "family_ether",
    "family_fire",
    "family_air",
    "family_water",
    "family_earth",
    "family_arcane",
    "family_mind",
    "family_body",
    "family_advanced",
    "family_runtime_only",
    "is_primary",
    "is_secondary",
    "is_passive",
    "is_utility",
    "is_weld",
    "is_health_up",
    "is_mana_up",
    "weld_element_ether",
    "weld_element_fire",
    "weld_element_air",
    "weld_element_water",
    "weld_element_earth",
    "weld_build_index_scaled",
    "weld_build_id_bit_0",
    "weld_build_id_bit_1",
    "weld_build_id_bit_2",
    "weld_build_id_bit_3",
    "weld_build_id_bit_4",
    "weld_build_id_bit_5",
    "weld_build_id_bit_6",
    "weld_build_id_bit_7",
    "weld_build_id_bit_8",
    "weld_build_id_bit_9",
    "weld_build_id_bit_10",
    "weld_build_id_bit_11",
    "weld_build_id_bit_12",
    "weld_build_id_bit_13",
    "weld_build_id_bit_14",
    "weld_build_id_bit_15",
    "mana_cost_scaled",
    "damage_min_scaled",
    "damage_max_scaled",
    "range_scaled",
    "cooldown_scaled",
    "radius_scaled",
    "duration_scaled",
    "value_scaled",
    "concentration_scaled",
    "chance_scaled",
    "mana_cost_present",
    "damage_min_present",
    "damage_max_present",
    "range_present",
    "cooldown_present",
    "radius_present",
    "duration_present",
    "value_present",
    "concentration_present",
    "chance_present",
    "quantity_scaled",
    "quantity_present",
    "strength_scaled",
    "strength_present",
    "absorb_scaled",
    "absorb_present",
    "arcs_scaled",
    "arcs_present",
    "armor_plus_scaled",
    "armor_plus_present",
    "charges_scaled",
    "charges_present",
    "flee_scaled",
    "flee_present",
    "fragments_scaled",
    "fragments_present",
    "freeze_scaled",
    "freeze_present",
    "hp_scaled",
    "hp_present",
    "hoard_scaled",
    "hoard_present",
    "loss_scaled",
    "loss_present",
    "max_armor_scaled",
    "max_armor_present",
    "percent_scaled",
    "percent_present",
    "pierces_scaled",
    "pierces_present",
    "pushback_scaled",
    "pushback_present",
    "reflect_scaled",
    "reflect_present",
    "size_scaled",
    "size_present",
    "slow_scaled",
    "slow_present",
    "slowdown_scaled",
    "slowdown_present",
    "speed_scaled",
    "speed_present",
    "speed_up_scaled",
    "speed_up_present",
    "stun_amount_scaled",
    "stun_amount_present",
    "to_hit_scaled",
    "to_hit_present",
    "weaken_scaled",
    "weaken_present",
    "widen_scaled",
    "widen_present",
];

pub mod scales {
    pub const AIM_OFFSET_WORLD: f64 = 60.0;
    pub const ALLY_COUNT: f64 = 50.0;
    pub const COOLDOWN_SECONDS: f64 = 60.0;
    pub const EDGE_PRESSURE_RANGE: f64 = 480.0;
    pub const EFFECT_LIFETIME_SECONDS: f64 = 60.0;
    pub const ENEMY_ACTION_SECONDS: f64 = 2.0;
    pub const ENEMY_COUNT: f64 = 16.0;
    pub const ENEMY_PHASE_SECONDS: f64 = 5.0;
    pub const EQUIPMENT_EFFECT: f64 = 4.0;
    pub const EQUIPMENT_RARITY: f64 = 2.0;
    pub const EQUIPMENT_TARGET_KIND: f64 = 8.0;
    pub const GLOBAL_COOLDOWN_TICKS: f64 = 150.0;
    pub const HAZARD_CONTACT_SECONDS: f64 = 10.0;
    pub const HAZARD_LIFETIME_SECONDS: f64 = 60.0;
    pub const HISTORY_SECONDS: f64 = 5.0;
    pub const HP: f64 = 1000.0;
    pub const INVENTORY_COUNT_SATURATION: f64 = 99.0;
    pub const LEVEL: f64 = 75.0;
    pub const MANA: f64 = 2000.0;
    pub const MINION_AGE_SECONDS: f64 = 60.0;
    pub const MINION_COUNT: f64 = 4.0;
    pub const MULTIPLIER: f64 = 4.0;
    pub const OWN_EFFECT_COUNT: f64 = 16.0;
    pub const PATCH_RADIUS: f64 = 3.0;
    pub const PATCH_SPACING: f64 = 60.0;
    pub const PICKUP_COUNT: f64 = 8.0;
    pub const POTION_SLOTS: f64 = 12.0;
    pub const RADIUS: f64 = 100.0;
    pub const RANGE: f64 = 1000.0;
    pub const RAY_RANGE: f64 = 480.0;
    pub const RAY_STEP: f64 = 60.0;
    pub const SKILL_DAMAGE: f64 = 500.0;
    pub const SKILL_BAND: f64 = 8.0;
    pub const SKILL_ABSORB: f64 = 600.0;
    pub const SKILL_ARCS: f64 = 12.0;
    pub const SKILL_ARMOR_PLUS: f64 = 60.0;
    pub const SKILL_CHANCE: f64 = 100.0;
    pub const SKILL_CHARGES: f64 = 6.0;
    pub const SKILL_CONCENTRATION: f64 = 25.0;
    pub const SKILL_DURATION_SECONDS: f64 = 30.0;
    pub const SKILL_FLEE: f64 = 6.0;
    pub const SKILL_FRAGMENTS: f64 = 10.0;
    pub const SKILL_FREEZE: f64 = 100.0;
    pub const SKILL_HP: f64 = 700.0;
    pub const SKILL_HOARD: f64 = 60.0;
    pub const SKILL_ID: f64 = 81.0;
    pub const SKILL_LOSS: f64 = 80.0;
    pub const SKILL_MAX_ARMOR: f64 = 300.0;
    pub const SKILL_PERCENT: f64 = 90.0;
    pub const SKILL_PIERCES: f64 = 8.0;
    pub const SKILL_PUSHBACK: f64 = 100.0;
    pub const SKILL_QUANTITY: f64 = 14.0;
    pub const SKILL_REFLECT: f64 = 250.0;
    pub const SKILL_RADIUS: f64 = 20.0;
    pub const SKILL_RANK: f64 = 20.0;
    pub const SKILL_SIZE: f64 = 210.0;
    pub const SKILL_SLOW: f64 = 95.0;
    pub const SKILL_SLOWDOWN: f64 = 50.0;
    pub const SKILL_SPEED: f64 = 350.0;
    pub const SKILL_SPEED_UP: f64 = 350.0;
    pub const SKILL_STRENGTH: f64 = 2100.0;
    pub const SKILL_STUN_AMOUNT: f64 = 100.0;
    pub const SKILL_TO_HIT: f64 = 25.0;
    pub const SKILL_VALUE: f64 = 1250.0;
    pub const SKILL_WEAKEN: f64 = 85.0;
    pub const SKILL_WIDEN: f64 = 150.0;
    pub const STATUS_DURATION_SECONDS: f64 = 60.0;
    pub const TARGET_ACTION: f64 = 8.0;
    pub const THREAT_COUNT: f64 = 8.0;
    pub const THREAT_RADIUS_WORLD: f64 = 340.0;
    pub const TICK_RATE: f64 = 100.0;
    pub const VELOCITY: f64 = 1000.0;
    pub const WAVE: f64 = 20.0;
}

#[derive(Clone, Copy, Debug)]
pub struct MlBotPolicyBlock {
    pub key: char,
    pub start: usize,
    pub length: usize,
}

pub const BLOCKS: [MlBotPolicyBlock; 20] = [
    MlBotPolicyBlock { key: 'A', start: 0, length: 32 },
    MlBotPolicyBlock { key: 'B', start: 32, length: 11 },
    MlBotPolicyBlock { key: 'C', start: 43, length: 120 },
    MlBotPolicyBlock { key: 'D', start: 163, length: 88 },
    MlBotPolicyBlock { key: 'E', start: 251, length: 9 },
    MlBotPolicyBlock { key: 'F', start: 260, length: 56 },
    MlBotPolicyBlock { key: 'G', start: 316, length: 85 },
    MlBotPolicyBlock { key: 'I', start: 401, length: 41 },
    MlBotPolicyBlock { key: 'H', start: 442, length: 43 },
    MlBotPolicyBlock { key: 'J', start: 485, length: 3 },
    MlBotPolicyBlock { key: 'K', start: 488, length: 352 },
    MlBotPolicyBlock { key: 'L', start: 840, length: 4 },
    MlBotPolicyBlock { key: 'M', start: 844, length: 104 },
    MlBotPolicyBlock { key: 'N', start: 948, length: 289 },
    MlBotPolicyBlock { key: 'O', start: 1237, length: 230 },
    MlBotPolicyBlock { key: 'P', start: 1467, length: 105 },
    MlBotPolicyBlock { key: 'Q', start: 1572, length: 9 },
    MlBotPolicyBlock { key: 'R', start: 1581, length: 141 },
    MlBotPolicyBlock { key: 'S', start: 1722, length: 62 },
    MlBotPolicyBlock { key: 'T', start: 1784, length: 1242 },
];
