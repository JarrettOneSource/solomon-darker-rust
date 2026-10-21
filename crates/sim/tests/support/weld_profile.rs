use serde_json::Value;
use solomon_sim::profile::primary::{
    NativePrimarySkillProfile, NativePrimarySkillProfileBase, NativeWeldPrimarySkillProfile,
};
use solomon_sim::profile::weld::{NativeWeldCastKind, NativeWeldPrimaryVector};

pub fn profile(value: &Value) -> NativePrimarySkillProfile {
    let number = |key: &str| value[key].as_f64().unwrap();
    let build_id = value["buildId"].as_u64().unwrap() as u32;
    let cast_kind = match value["castKind"].as_str().unwrap() {
        "channel" => NativeWeldCastKind::Channel,
        "persistent" => NativeWeldCastKind::Persistent,
        _ => NativeWeldCastKind::OneShot,
    };
    NativePrimarySkillProfile::Weld(NativeWeldPrimarySkillProfile {
        common: NativePrimarySkillProfileBase {
            damage_minimum: number("damageMinimum"),
            damage_maximum: number("damageMaximum"),
            damage_roll_count: number("damageRollCount"),
            mana_cost: number("manaCost"),
            rank: value["rank"].as_u64().unwrap() as u32,
            skill_id: value["skillId"].as_u64().unwrap() as u32,
        },
        build_id,
        cast_kind,
        damage_factor: number("damageFactor"),
        vector: NativeWeldPrimaryVector {
            build_id,
            cast_kind,
            values: value["vector"]["values"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_f64().unwrap())
                .collect(),
        },
    })
}
