//! Replay the actual stock Skills names, descriptions and special native wrapping.

use serde::Deserialize;
use solomon_sim::player_panels::skill_text::{
    native_skill_page_display_name, native_skill_page_text_height, native_skill_page_tint,
    native_skill_page_wrapped_lines,
};

#[derive(Deserialize)]
struct Name {
    name: String,
    rank: u32,
    expected: String,
    lines: Vec<String>,
    height: f64,
}

#[derive(Deserialize)]
struct Wrap {
    text: String,
    lines: Vec<String>,
    height: f64,
}

#[derive(Deserialize)]
struct Tint {
    root: usize,
    expected: u32,
}

#[derive(Deserialize)]
struct Fixture {
    names: Vec<Name>,
    wraps: Vec<Wrap>,
    tints: Vec<Tint>,
}

#[test]
fn source_skill_page_text() {
    let fixture: Fixture =
        serde_json::from_str(include_str!("fixtures/website/skill-text-goldens.json")).unwrap();
    for case in fixture.names {
        let displayed = native_skill_page_display_name(&case.name, case.rank);
        assert_eq!(displayed, case.expected, "name {} rank {}", case.name, case.rank);
        let lines = native_skill_page_wrapped_lines(&displayed);
        assert_eq!(lines, case.lines, "name wrap {}", case.name);
        assert_eq!(native_skill_page_text_height(&lines), case.height);
    }
    for case in fixture.wraps {
        let lines = native_skill_page_wrapped_lines(&case.text);
        assert_eq!(lines, case.lines, "description {:?}", case.text);
        assert_eq!(native_skill_page_text_height(&lines), case.height);
    }
    for case in fixture.tints {
        assert_eq!(native_skill_page_tint(case.root), case.expected, "root {}", case.root);
    }
}
