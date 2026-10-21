use serde_json::Value;
use solomon_sim::Vec2;
use solomon_sim::player_panels::skill_hover::{
    NativeSkillHoverLine, NativeSkillHoverLineKind, measure_native_skill_exact_text,
    native_skill_book_tooltip_lines, native_skill_exact_text_runs, native_skill_hover_box_layout,
    skill_picker_detail_lines,
};
use solomon_sim::profile::PlayerSkillOfferOption;

#[test]
fn native_skill_hover_lines_wraps_and_picker_details_match_current_website() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/skill-hover-goldens.json")).unwrap();
    for case in fixture["rows"].as_array().unwrap() {
        let actual = native_skill_book_tooltip_lines(
            case["skillId"].as_u64().unwrap() as usize,
            case["permanentRank"].as_u64().unwrap() as u32,
            case["effectiveRank"].as_u64().unwrap() as u32,
        );
        assert_eq!(lines_json(&actual), case["expected"], "skill {}", case["skillId"]);
    }
    for case in fixture["pickerRows"].as_array().unwrap() {
        let option = &case["option"];
        let actual = skill_picker_detail_lines(&PlayerSkillOfferOption {
            skill_id: option["skillId"].as_u64().unwrap() as usize,
            target_rank: option["targetRank"].as_u64().unwrap() as u32,
            insight: option["insight"].as_bool().unwrap(),
            weld_build_id: option["weldBuildId"].as_u64().map(|id| id as u32),
        });
        assert_eq!(lines_json(&actual), case["expected"], "picker {option}");
    }
    for case in fixture["layouts"].as_array().unwrap() {
        let lines: Vec<_> = case["lines"].as_array().unwrap().iter().map(read_line).collect();
        let actual = native_skill_hover_box_layout(
            &lines,
            Vec2::new(case["source"]["x"].as_f64().unwrap(), case["source"]["y"].as_f64().unwrap()),
        );
        let expected = &case["expected"];
        for (key, value) in [
            ("width", actual.rect.width),
            ("height", actual.rect.height),
            ("x", actual.rect.x),
            ("y", actual.rect.y),
            ("contentWidth", actual.content_width),
            ("lineHeight", actual.line_height),
        ] {
            assert_eq!(value, expected[key].as_f64().unwrap(), "layout {key}: {case}");
        }
        let rendered: Vec<_> = actual
            .rendered
            .iter()
            .map(|line| serde_json::json!({"kind": line.kind.as_str(), "sources": line.sources}))
            .collect();
        assert_eq!(serde_json::json!(rendered), expected["rendered"]);
    }
    for case in fixture["runs"].as_array().unwrap() {
        let source = case["source"].as_str().unwrap();
        let runs = native_skill_exact_text_runs(source);
        let expected = case["expected"].as_array().unwrap();
        assert_eq!(runs.len(), expected.len(), "runs {source}");
        for (run, expected) in runs.iter().zip(expected) {
            assert_eq!(run.text, expected["text"].as_str().unwrap());
            assert_eq!(run.scale.unwrap_or(1.0), expected["scale"].as_f64().unwrap());
            assert_eq!(run.offset_x, expected["offsetX"].as_f64().unwrap());
            assert_eq!(run.offset_y, expected["offsetY"].as_f64().unwrap());
            assert_eq!(run.italic, expected["italic"].as_bool().unwrap());
        }
        assert_eq!(measure_native_skill_exact_text(source), case["width"].as_f64().unwrap());
    }
}

fn lines_json(lines: &[NativeSkillHoverLine]) -> Value {
    let rows: Vec<_> = lines
        .iter()
        .map(|line| serde_json::json!({"kind": line.kind.as_str(), "text": line.text}))
        .collect();
    serde_json::json!(rows)
}

fn read_line(value: &Value) -> NativeSkillHoverLine {
    use NativeSkillHoverLineKind as Kind;
    let kind = match value["kind"].as_str().unwrap() {
        "bonus" => Kind::Bonus,
        "boost" => Kind::Boost,
        "category" => Kind::Category,
        "description" => Kind::Description,
        "level" => Kind::Level,
        "spacer" => Kind::Spacer,
        "stat" => Kind::Stat,
        "title" => Kind::Title,
        _ => panic!("unknown source skill line"),
    };
    NativeSkillHoverLine { kind, text: value["text"].as_str().unwrap().to_owned() }
}
