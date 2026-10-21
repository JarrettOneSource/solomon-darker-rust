use serde::Deserialize;
use solomon_sim::combat::PlayerCombatState;
use solomon_sim::presentation::{
    multiply_native_tints, native_player_material_tint, native_puppet_hit_tint,
};
use solomon_sim::secondary::NativeSecondaryPlayerState;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Case {
    world_tint: u32,
    poison: bool,
    cold: bool,
    poison_before_cold: bool,
    stoneskin: bool,
    tint: u32,
    robe_tint: u32,
    hit_tint: u32,
    flat_hit_tint: u32,
}
#[derive(Deserialize)]
struct Goldens {
    cases: Vec<Case>,
    #[serde(rename = "primaryColors")]
    primary_colors: Vec<PrimaryColor>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PrimaryColor {
    skill_id: usize,
    weld_build_id: Option<u32>,
    tint: u32,
}

#[test]
fn player_status_insertion_order_and_native_dye_hit_colours_replay_the_website() {
    let fixture: Goldens =
        serde_json::from_str(include_str!("fixtures/website/player-material-goldens.json"))
            .unwrap();
    for case in fixture.cases {
        let mut combat = PlayerCombatState::new();
        combat.poison_ticks_remaining = if case.poison { 1_000 } else { 0 };
        combat.cold_slow_ticks_remaining = if case.cold { 250 } else { 0 };
        combat.poison_before_cold = case.poison_before_cold;
        let secondary = NativeSecondaryPlayerState {
            stoneskin_ticks_remaining: u64::from(case.stoneskin),
            ..NativeSecondaryPlayerState::default()
        };
        let tint = native_player_material_tint(case.world_tint, Some(&secondary), &combat);
        assert_eq!(tint, case.tint);
        assert_eq!(multiply_native_tints(0x916491, tint), case.robe_tint);
        assert_eq!(native_puppet_hit_tint(true, tint), case.hit_tint);
        assert_eq!(native_puppet_hit_tint(false, tint), case.flat_hit_tint);
    }
    for color in fixture.primary_colors {
        assert_eq!(
            solomon_sim::profile::primary::native_primary_spell_tint(
                color.skill_id,
                color.weld_build_id
            ),
            color.tint,
        );
    }
}
