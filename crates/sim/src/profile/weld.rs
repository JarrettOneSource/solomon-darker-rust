//! The ten native welded-primary vectors from `native-weld-primary-profile.ts`.
//!
//! `PlayerWizardSkills::RebuildWeldedSpell` (`0x00666020`) narrows every catalog read
//! and vector write. Intermediate narrowing is retained in the formula order below.

use crate::float::fround;

use super::{
    NATIVE_SKILL_ROW_COUNT, NativeSkillBookState, PlayerStatBookComponent, native_weld_build,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeWeldCastKind {
    Channel,
    OneShot,
    Persistent,
}

impl NativeWeldCastKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Channel => "channel",
            Self::OneShot => "one-shot",
            Self::Persistent => "persistent",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeWeldPrimaryVector {
    pub build_id: u32,
    pub cast_kind: NativeWeldCastKind,
    pub values: Vec<f64>,
}

#[must_use]
pub fn native_weld_primary_cast_mode(build_id: u32) -> NativeWeldCastKind {
    match build_id {
        1000..=1002 | 1009 => NativeWeldCastKind::OneShot,
        1003..=1005 => NativeWeldCastKind::Channel,
        1006..=1008 => NativeWeldCastKind::Persistent,
        _ => panic!("weld build {build_id} is not native"),
    }
}

// These are the reference's already-rounded native constants, widened to JavaScript doubles.
const F32_001: f64 = 0.01_f32 as f64;
const F32_002: f64 = 0.02_f32 as f64;
const F32_08: f64 = 0.8_f32 as f64;
const F32_086: f64 = 0.86_f32 as f64;
const F32_09: f64 = 0.9_f32 as f64;
const F32_0932: f64 = 0.932_f32 as f64;
const F32_099: f64 = 0.99_f32 as f64;
const F32_105: f64 = 1.05_f32 as f64;
const F32_11: f64 = 1.1_f32 as f64;
const F32_115: f64 = 1.15_f32 as f64;
const F32_12: f64 = 1.2_f32 as f64;
const F32_125: f64 = 1.25;
const F32_13: f64 = 1.3_f32 as f64;
const F32_29: f64 = 2.9_f32 as f64;

/// Freezes component ranks at the same seam as `nativeWeldPrimaryVector`.
#[must_use]
pub fn native_weld_primary_vector(
    book: &NativeSkillBookState,
    build_id: u32,
    weld_effect: f64,
) -> NativeWeldPrimaryVector {
    native_weld_primary_vector_with_stat_book(
        book,
        &PlayerStatBookComponent::default(),
        build_id,
        weld_effect,
    )
}

#[must_use]
pub fn native_weld_primary_vector_with_stat_book(
    book: &NativeSkillBookState,
    stat_book: &PlayerStatBookComponent,
    build_id: u32,
    weld_effect: f64,
) -> NativeWeldPrimaryVector {
    assert!(
        weld_effect.is_finite() && weld_effect > 0.0,
        "weld effect must be finite and positive"
    );
    let build = native_weld_build(build_id).expect("requested weld build is native");
    let frozen =
        if book.weld_build_id == Some(build_id) { book.weld_component_ranks } else { None };
    let mut ranks = [0; NATIVE_SKILL_ROW_COUNT];
    for (index, &skill_id) in build.component_skill_ids.iter().enumerate() {
        ranks[skill_id] = match frozen {
            Some(frozen) => frozen[index],
            None => book.effective_ranks[skill_id],
        };
    }
    for skill_id in build.primary_skill_ids {
        assert!(ranks[skill_id] > 0, "weld primary skill {skill_id} must be learned");
    }
    let input = WeldInputs { ranks, stat_book, weld_effect };
    let values = match build_id {
        1000 => {
            let ether_cost = input.cost(&[8, 10, 9]);
            let fire_cost = input.cost(&[16, 18, 17]);
            let quantity = input.value_or(10, "mQuantity", 1.0).round();
            let explode_damage = input.value(18, "mDamage");
            let fragments = input.value(17, "mFragments");
            let mut mana_cost = input.divide_weld(fround(
                (ether_cost.min(fire_cost) + 2.0 * ether_cost.max(fire_cost)) / 3.0,
            ));
            if quantity > 1.0 && explode_damage > 0.0 {
                let first = fround(fround(quantity * F32_002) + F32_125);
                mana_cost = fround(mana_cost * fround(first + quantity * F32_001));
            }
            if fragments > 0.0 {
                mana_cost = fround(mana_cost * quantity * F32_105 * 0.5);
            }
            vec![
                fround(
                    fround((2.0 * input.value(8, "mDamage1") + input.value(16, "mDamage")) / 3.0)
                        * weld_effect,
                ),
                fround(
                    fround((2.0 * input.value(8, "mDamage2") + input.value(16, "mDamage")) / 3.0)
                        * weld_effect,
                ),
                mana_cost,
                quantity,
                fround((1.0 + input.value(9, "mSpeed") / 100.0) * F32_125),
                fround(explode_damage * 0.75),
                fround(input.value(18, "mRadius") * F32_08),
                fround(input.value(17, "mDamage") * 0.75),
                fround(fragments * 0.75),
            ]
        }
        1001 => {
            let ether_cost = input.cost(&[8, 10, 9]);
            let water_cost = input.cost(&[32, 34, 33]);
            let water_damage = input.value(32, "mDamage");
            vec![
                fround(
                    fround((input.value(8, "mDamage1") + 2.0 * water_damage) / 5.0) * weld_effect,
                ),
                fround(
                    fround((input.value(8, "mDamage2") + 2.0 * water_damage) / 4.0) * weld_effect,
                ),
                input.divide_weld(fround(
                    fround(
                        (2.0 * ether_cost.min(water_cost) + 3.0 * ether_cost.max(water_cost)) / 3.0,
                    ) * F32_11,
                )),
                input.value_or(10, "mQuantity", 1.0).round(),
                fround(1.0 + input.value(9, "mSpeed") / 100.0),
                fround(input.value(33, "mPushback") * F32_002),
                fround(input.value(34, "mWiden") / 150.0),
            ]
        }
        1002 => {
            let ether_cost = input.cost(&[8, 10, 9]);
            let air_cost = input.cost(&[24, 25, 26]);
            let air_damage = input.value(24, "mDamage");
            vec![
                fround(
                    fround(input.value(8, "mDamage1") / 3.0 + air_damage / 100.0)
                        * F32_29
                        * weld_effect,
                ),
                fround(
                    fround(input.value(8, "mDamage2") / 3.0 + air_damage / 100.0)
                        * F32_29
                        * weld_effect,
                ),
                input.divide_weld(fround(fround(ether_cost + air_cost / 100.0) * F32_099)),
                input.value_or(10, "mQuantity", 1.0).round(),
                fround(1.0 + input.value(9, "mSpeed") / 100.0),
                input.value(25, "mArcs").round(),
                fround(1.0 - input.value(26, "mStunAmount") * F32_09 / 100.0),
            ]
        }
        1003 => {
            let explode_rank = input.rank(18);
            let arc_rank = input.rank(25);
            let explode_damage = input.value(18, "mDamage");
            let arc_count = input.value(25, "mArcs").round();
            let mut mana_cost = input.divide_weld(fround(
                fround(2.0 * input.cost(&[16, 18, 17]) + input.cost(&[24, 25, 26]) / 50.0)
                    * F32_086,
            ));
            if arc_count > 0.0 && explode_damage > 0.0 {
                let mut multiplier = F32_115;
                if arc_rank > 1 {
                    multiplier = fround(F32_115 + f64::from(arc_rank) * F32_002);
                }
                if explode_rank > 1 {
                    multiplier = fround(multiplier + f64::from(explode_rank) * F32_001);
                }
                mana_cost = fround(mana_cost * multiplier);
            }
            if input.rank(17) > 0 && input.rank(26) > 0 {
                mana_cost = fround(mana_cost * F32_105);
            }
            vec![
                fround(
                    fround(
                        (2.0 * input.value(16, "mDamage") + input.value(24, "mDamage") / 100.0)
                            / F32_13,
                    ) * weld_effect,
                ),
                mana_cost,
                arc_count,
                fround(1.0 - input.value(26, "mStunAmount") * F32_09 / 100.0),
                fround(explode_damage * 0.75),
                input.value(18, "mRadius"),
                input.value(17, "mDamage"),
                fround(input.value(17, "mFragments") * 0.75),
            ]
        }
        1004 => {
            let water_cost = input.cost(&[32, 34, 33]);
            let air_cost = input.cost(&[24, 25, 26]);
            vec![
                fround(
                    fround(
                        input.value(32, "mDamage") * F32_12 + input.value(24, "mDamage") * 0.004,
                    ) * weld_effect,
                ),
                input.divide_weld(fround(
                    water_cost.max(air_cost) * F32_12 + water_cost.min(air_cost) * 0.6,
                )),
                input.value(25, "mArcs").round(),
                fround(1.0 - input.value(26, "mStunAmount") * F32_09 / 100.0),
                0.0,
                fround(input.value(33, "mPushback") / 100.0),
                fround(input.value(34, "mWiden") / 250.0),
            ]
        }
        1005 => {
            let explode_rank = input.rank(18);
            let widen_rank = input.rank(34);
            let explode_damage = input.value(18, "mDamage");
            let widen = input.value(34, "mWiden");
            let mut mana_cost = input.divide_weld(fround(
                fround(input.cost(&[16, 18, 17]) + 2.0 * input.cost(&[32, 34, 33]) / 100.0)
                    * F32_0932,
            ));
            if widen > 0.0 && explode_damage > 0.0 {
                let mut multiplier = F32_115;
                if widen_rank > 1 {
                    multiplier = fround(F32_115 + f64::from(widen_rank) * F32_002);
                }
                if explode_rank > 1 {
                    multiplier = fround(multiplier + f64::from(explode_rank) * F32_001);
                }
                mana_cost = fround(mana_cost * multiplier);
            }
            if input.rank(17) > 0 && input.rank(33) > 0 {
                mana_cost = fround(mana_cost * F32_105);
            }
            vec![
                fround(
                    fround((input.value(16, "mDamage") + 2.0 * input.value(32, "mDamage")) / 1.125)
                        * weld_effect,
                ),
                mana_cost,
                fround(widen * 0.5),
                fround(input.value(33, "mPushback") * 0.0075),
                fround(explode_damage * 0.75),
                input.value(18, "mRadius"),
                input.value(17, "mDamage"),
                fround(input.value(17, "mFragments") * 0.75),
            ]
        }
        1006 => {
            let ether_cost = input.cost(&[8, 10, 9]);
            let earth_cost = input.cost(&[40, 43, 42]);
            vec![
                fround(
                    fround((input.value(8, "mDamage1") + input.value(40, "mDamage")) * 0.5)
                        * weld_effect,
                ),
                input.divide_weld(fround(
                    (ether_cost.min(earth_cost) + 2.0 * ether_cost.max(earth_cost)) * 0.5,
                )),
                input.value_or(10, "mQuantity", 1.0).round().clamp(1.0, 4.0),
                fround(1.0 + input.value(9, "mSpeed") / 100.0),
                fround(input.value(43, "mStrength") / 100.0),
                fround(1.0 + input.value(42, "mSpeedUp") / 100.0),
            ]
        }
        1007 => {
            let explode_damage = fround(input.value(18, "mDamage") * 0.75);
            let fragments = input.value(17, "mFragments");
            let toughness = fround(input.value(43, "mStrength") / 100.0);
            let growth = fround(1.0 + input.value(42, "mSpeedUp") / 100.0);
            let mut mana_cost = input.divide_weld(fround(
                (input.cost(&[16, 18, 17]) + input.cost(&[40, 43, 42])) * F32_125,
            ));
            if toughness > 1.0 || explode_damage > 0.0 || growth > 1.0 || fragments > 0.0 {
                mana_cost = fround(mana_cost * 4.5);
            }
            if toughness > 1.0 && explode_damage > 0.0 {
                mana_cost = fround(fround(mana_cost + toughness * 5.0) * 1.5);
            }
            if fragments > 0.0 {
                mana_cost = fround(mana_cost + fragments);
            }
            let first_damage = fround(input.value(16, "mDamage") * 2.0);
            let second_damage = fround(input.value(40, "mDamage") * 2.0);
            vec![
                fround(first_damage.min(second_damage) * weld_effect),
                fround(first_damage.max(second_damage) * weld_effect),
                mana_cost,
                growth,
                toughness,
                explode_damage,
                input.value(18, "mRadius"),
                input.value(17, "mDamage"),
                fround(fragments * 0.6),
            ]
        }
        1008 => {
            let damage = fround(
                ((3.0 * input.value(32, "mDamage") + input.value(40, "mDamage")) / 20.0 * 1.5)
                    .max(1.0),
            );
            let growth = fround(1.0 + input.value(42, "mSpeedUp") / 100.0);
            let widen = fround(input.value(34, "mWiden") / 60.0);
            let mut mana_cost = fround(
                input.divide_weld(fround(
                    input.cost(&[40, 43, 42]) + input.cost(&[32, 34, 33]) / 100.0,
                )) + damage * 2.0,
            );
            if widen > 0.0 && growth > 1.0 {
                mana_cost = fround(mana_cost * F32_125);
            }
            vec![
                fround(damage * weld_effect),
                mana_cost,
                growth,
                fround(input.value(43, "mStrength") / 100.0),
                fround(input.value(33, "mPushback") / 100.0),
                widen,
            ]
        }
        1009 => {
            let arcs = input.value(25, "mArcs").round();
            let mana_cost = fround(
                input.divide_weld(fround(
                    (3.0 * input.cost(&[40, 43, 42]) + 2.0 * input.cost(&[24, 25, 26]) / 100.0)
                        * 0.25,
                )) + 2.0 * arcs,
            );
            vec![
                fround(
                    fround((input.value(40, "mDamage") + input.value(24, "mDamage") / 25.0) * 0.2)
                        * weld_effect,
                ),
                mana_cost,
                arcs,
                fround(1.0 - input.value(26, "mStunAmount") * F32_09 / 100.0),
                f64::from(input.rank(43)),
                fround(1.0 + input.value(42, "mSpeedUp") / 100.0),
            ]
        }
        _ => unreachable!("weld build was checked above"),
    };
    NativeWeldPrimaryVector {
        build_id,
        cast_kind: native_weld_primary_cast_mode(build_id),
        values: values.into_iter().map(fround).collect(),
    }
}

struct WeldInputs<'a> {
    ranks: [u32; NATIVE_SKILL_ROW_COUNT],
    weld_effect: f64,
    stat_book: &'a PlayerStatBookComponent,
}

impl WeldInputs<'_> {
    fn rank(&self, skill_id: usize) -> u32 {
        self.ranks[skill_id]
    }

    fn value(&self, skill_id: usize, property: &str) -> f64 {
        self.value_or(skill_id, property, 0.0)
    }

    fn value_or(&self, skill_id: usize, property: &str, fallback: f64) -> f64 {
        let rank = self.rank(skill_id);
        if rank == 0 {
            return fround(fallback);
        }
        let value = self
            .stat_book
            .stat(skill_id, property, rank)
            .unwrap_or_else(|| panic!("weld component {skill_id} lacks {property} at rank {rank}"));
        fround(value)
    }

    fn cost(&self, skill_ids: &[usize]) -> f64 {
        let mut total = 0.0;
        for &skill_id in skill_ids {
            if self.rank(skill_id) > 0 {
                total = fround(total + self.value(skill_id, "mManaCost"));
            }
        }
        total
    }

    fn divide_weld(&self, value: f64) -> f64 {
        fround(value / fround(self.weld_effect))
    }
}
