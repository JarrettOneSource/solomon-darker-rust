use serde::Deserialize;
use solomon_sim::profile::PlayerSkillOfferOption;
use solomon_sim::skill_picker::{
    skill_picker_card, skill_picker_centers, skill_picker_close, skill_picker_panel,
    skill_picker_reveal,
};

#[derive(Deserialize)]
struct Fixture {
    reveals: Vec<Timing>,
    closes: Vec<Timing>,
    cards: Vec<Card>,
    layouts: Vec<Layout>,
}
#[derive(Deserialize)]
struct Timing {
    elapsed: f64,
    direction: Option<f64>,
    expected: Reveal,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Reveal {
    ambient_alpha: f64,
    curtain_alpha: f64,
    interactive: bool,
    panel_alpha: f64,
    reveal_alpha: f64,
}
#[derive(Deserialize)]
struct Card {
    option: OptionRow,
    expected: CardRow,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OptionRow {
    skill_id: usize,
    target_rank: u32,
    insight: bool,
    weld_build_id: Option<u32>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CardRow {
    family_label: String,
    frame_record: usize,
    glow_tints: Vec<u32>,
    icon_record: usize,
    name: String,
    quick_description: String,
    root: usize,
    root_tint: u32,
}
#[derive(Deserialize)]
struct Layout {
    count: usize,
    panel: Panel,
    centers: Vec<f64>,
}
#[derive(Deserialize)]
struct Panel {
    height: f64,
    left: f64,
    top: f64,
    width: f64,
}

#[test]
fn native_offer_cards_welds_layout_and_modal_timing_match_the_source() {
    let fixture: Fixture =
        serde_json::from_str(include_str!("fixtures/website/skill-picker-goldens.json")).unwrap();
    for timing in fixture.reveals.into_iter().chain(fixture.closes) {
        let actual = timing.direction.map_or_else(
            || skill_picker_reveal(timing.elapsed),
            |direction| skill_picker_close(timing.elapsed, direction),
        );
        let expected = timing.expected;
        assert_eq!(actual.interactive, expected.interactive);
        for (actual, expected) in [
            (actual.ambient_alpha, expected.ambient_alpha),
            (actual.curtain_alpha, expected.curtain_alpha),
            (actual.panel_alpha, expected.panel_alpha),
            (actual.reveal_alpha, expected.reveal_alpha),
        ] {
            assert!((actual - expected).abs() < 1e-12);
        }
    }
    for card in fixture.cards {
        let option = card.option;
        let actual = skill_picker_card(&PlayerSkillOfferOption {
            skill_id: option.skill_id,
            target_rank: option.target_rank,
            insight: option.insight,
            weld_build_id: option.weld_build_id,
        });
        let expected = card.expected;
        assert_eq!(actual.family_label, expected.family_label);
        assert_eq!(actual.frame_record, expected.frame_record);
        assert_eq!(actual.glow_tints, expected.glow_tints);
        assert_eq!(actual.icon_record, expected.icon_record);
        assert_eq!(actual.name, expected.name);
        assert_eq!(actual.quick_description, expected.quick_description);
        assert_eq!(actual.root, expected.root);
        assert_eq!(actual.root_tint, expected.root_tint);
    }
    for layout in fixture.layouts {
        let panel = skill_picker_panel(layout.count);
        assert_eq!(
            [panel.x, panel.y, panel.width, panel.height],
            [layout.panel.left, layout.panel.top, layout.panel.width, layout.panel.height]
        );
        assert_eq!(skill_picker_centers(layout.count), layout.centers);
    }
}
