//! Local rankings, native row/scroll geometry and class names from the pinned Website.
use serde_json::Value;
use solomon_sim::Vec2;
use solomon_sim::hall_of_fame::presentation::*;
use solomon_sim::hall_of_fame::*;
use solomon_sim::hud::HudRect;
use solomon_sim::profile::{WizardDiscipline, WizardElement, native_wizard_class_title};

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/website/hall-of-fame-goldens.json")).unwrap()
}
fn number(value: &Value) -> f64 {
    value.as_f64().unwrap()
}
fn same_point(actual: Vec2, expected: &Value) {
    assert_eq!(actual.x, number(&expected["x"]));
    assert_eq!(actual.y, number(&expected["y"]));
}
fn same_rect(actual: HudRect, expected: &Value) {
    assert_eq!(actual.x, number(&expected["left"]));
    assert_eq!(actual.y, number(&expected["top"]));
    assert_eq!(actual.width, number(&expected["width"]));
    assert_eq!(actual.height, number(&expected["height"]));
}
fn same_anchor(actual: HallTextAnchor, expected: &Value) {
    same_point(actual.position, expected);
    assert_eq!(actual.font.as_str(), expected["font"].as_str().unwrap());
    let align = match actual.align {
        HallTextAlign::Center => "center",
        HallTextAlign::Left => "left",
        HallTextAlign::Right => "right",
    };
    assert_eq!(align, expected["align"].as_str().unwrap());
}

#[test]
fn hall_rankings_preserve_source_ties_and_the_complete_class_table() {
    let golden = fixture();
    let entries: Vec<_> = golden["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| HallOfFameEntry {
            account_username: None,
            awesomeness: entry["awesomeness"].as_u64().unwrap(),
            awesomest_kill: None,
            completed_at_utc: String::new(),
            discipline: WizardDiscipline::Arcane,
            elapsed_ticks: entry["elapsedTicks"].as_u64().unwrap(),
            element: WizardElement::Ether,
            heading_index: 0,
            highest_skills: Vec::new(),
            level: 1,
            monsters_killed: entry["monstersKilled"].as_u64().unwrap(),
            perks_used: Vec::new(),
            portrait_scale: 1.0,
            run_id: entry["runId"].as_str().unwrap().to_owned(),
            wave: entry["wave"].as_u64().unwrap(),
            wizard_name: String::new(),
        })
        .collect();
    for case in golden["rankings"].as_array().unwrap() {
        let board = HallOfFameBoard::ALL
            .into_iter()
            .find(|board| board.as_str() == case["board"].as_str().unwrap())
            .unwrap();
        let actual =
            rank_hall_of_fame_entries(&entries, board, case["limit"].as_u64().unwrap() as usize);
        let ids: Vec<_> = actual.iter().map(|entry| entry.run_id.as_str()).collect();
        let expected: Vec<_> =
            case["ids"].as_array().unwrap().iter().map(|id| id.as_str().unwrap()).collect();
        assert_eq!(ids, expected);
    }
    for case in golden["times"].as_array().unwrap() {
        assert_eq!(
            format_hall_of_fame_time(case["ticks"].as_u64().unwrap()),
            case["text"].as_str().unwrap()
        );
    }
    for case in golden["classTitles"].as_array().unwrap() {
        let element = WizardElement::ALL
            .into_iter()
            .find(|element| element.as_str() == case["element"])
            .unwrap();
        let discipline = WizardDiscipline::ALL
            .into_iter()
            .find(|discipline| discipline.as_str() == case["discipline"])
            .unwrap();
        assert_eq!(native_wizard_class_title(element, discipline), case["title"].as_str().unwrap());
    }
}

#[test]
fn native_rows_expansion_clipping_and_current_run_scroll_replay_the_source() {
    let golden = fixture();
    for case in golden["layouts"].as_array().unwrap() {
        let actual = hall_row_layout(
            number(&case["top"]),
            case["expanded"].as_bool().unwrap(),
            number(&case["rankWidth"]),
            number(&case["awesomenessWidth"]),
        );
        let expected = &case["layout"];
        for (anchor, key) in [
            (actual.account, "account"),
            (actual.awesomeness, "awesomeness"),
            (actual.level, "level"),
            (actual.name, "name"),
            (actual.rank, "rank"),
        ] {
            same_anchor(anchor, &expected[key]);
        }
        same_point(actual.chevron.position, &expected["chevron"]);
        assert_eq!(actual.chevron.rotation, number(&expected["chevron"]["rotation"]));
        assert_eq!(actual.height, number(&expected["height"]));
        same_rect(actual.highlight, &expected["highlight"]);
        same_point(actual.ornament, &expected["ornament"]);
        assert_eq!(actual.separator_y, number(&expected["separatorY"]));
        same_point(actual.wizard, &expected["wizard"]);
        if let Some(actual) = actual.expanded {
            let expected = &expected["expanded"];
            assert_eq!(actual.top, number(&expected["top"]));
            same_rect(actual.kills_frame, &expected["killsFrame"]);
            for (anchor, key) in [
                (actual.awesomest_kill, "awesomestKill"),
                (actual.awesomest_label, "awesomestLabel"),
                (actual.highest_skills, "highestSkills"),
                (actual.monsters_killed, "monstersKilled"),
                (actual.perks_used, "perksUsed"),
                (actual.survival, "survival"),
                (actual.time_label, "timeLabel"),
                (actual.time_value, "timeValue"),
                (actual.wave_label, "waveLabel"),
                (actual.wave_value, "waveValue"),
            ] {
                same_anchor(anchor, &expected[key]);
            }
            for (index, center) in actual.perk_centers.iter().enumerate() {
                same_point(*center, &expected["perkCenters"][index]);
            }
            for (index, cell) in actual.skill_cells.iter().enumerate() {
                let expected = &expected["skillCells"][index];
                same_point(cell.center, &expected["center"]);
                same_anchor(cell.numeral, &expected["numeral"]);
                same_rect(cell.badge(12.0), &expected["badge"]);
            }
        } else {
            assert!(expected["expanded"].is_null());
        }
        for (index, rect) in hall_separator_halves(actual.separator_y).iter().enumerate() {
            same_rect(*rect, &case["separators"][index]);
        }
    }
    for case in golden["scroll"].as_array().unwrap() {
        let expanded: Vec<_> = case["expanded"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_bool().unwrap())
            .collect();
        let (tops, height) = hall_row_tops(&expanded);
        let expected: Vec<_> = case["tops"].as_array().unwrap().iter().map(number).collect();
        assert_eq!(tops, expected);
        assert_eq!(height, number(&case["contentHeight"]));
        assert_eq!(hall_scroll_extent(height), number(&case["extent"]));
        assert_eq!(
            hall_current_row_scroll_target(tops.last().copied().unwrap_or(0.0), height),
            number(&case["target"])
        );
        assert_eq!(
            hall_visible_row_range(&tops, &expanded, number(&case["offset"])),
            (
                case["range"]["start"].as_u64().unwrap() as usize,
                case["range"]["end"].as_u64().unwrap() as usize
            )
        );
        assert_eq!(hall_tile_offset(number(&case["offset"])), number(&case["tile"]));
    }
    for case in golden["motion"].as_array().unwrap() {
        let tick = number(&case["tick"]);
        // These three functions contain trigonometry; only libm's last bit may differ.
        for (actual, key) in [
            (hall_scroll_ease(tick), "ease"),
            (hall_highlight_fill_alpha(tick), "fill"),
            (hall_current_frame_alpha(tick), "frame"),
        ] {
            assert!((actual - number(&case[key])).abs() < 1.0e-12, "{key} tick{tick}");
        }
    }
}
