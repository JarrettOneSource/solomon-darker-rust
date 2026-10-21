//! Differential frames rendered from the unchanged Source React components.

#[cfg(feature = "persistence")]
#[test]
fn source_mod_skill_book_and_picker_rows() {
    use solomon_sim::modding::LuaObject;
    use solomon_sim::modding::ui::{project_mod_skill_book, project_mod_skill_picker};

    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/website/mod-skill-ui-goldens.json")).unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let runtime: Option<LuaObject> = serde_json::from_value(case["runtime"].clone()).unwrap();
        let name = case["name"].as_str().unwrap();
        let book = project_mod_skill_book(runtime.as_ref());
        assert_eq!(book.is_none(), case["book"].is_null(), "{name}: book visibility");
        if let Some(book) = book {
            let learned = case["book"]["learned"].as_array().unwrap();
            assert_eq!(book.learned.len(), learned.len(), "{name}: learned count");
            for (row, expected) in book.learned.iter().zip(learned) {
                assert_eq!(row.name, expected["name"].as_str().unwrap(), "{name}");
                assert_eq!(row.description, expected["description"].as_str().unwrap(), "{name}");
                assert_eq!(row.rank, expected["rank"].as_f64().unwrap(), "{name}");
                assert_eq!(row.maximum_rank, expected["maximum"].as_f64().unwrap(), "{name}");
                assert_eq!(row.subskill, expected["subskill"].as_bool().unwrap(), "{name}");
            }
            let spells = case["book"]["spells"].as_array().unwrap();
            assert_eq!(book.spells.len(), spells.len(), "{name}: spell count");
            for (row, expected) in book.spells.iter().zip(spells) {
                assert_eq!(row.name, expected["name"].as_str().unwrap(), "{name}");
                assert_eq!(row.bound, expected["bound"].as_bool().unwrap(), "{name}");
                assert_eq!(row.slot, expected["slot"].as_f64(), "{name}");
                assert_eq!(row.slot.is_none(), expected["disabled"].as_bool().unwrap(), "{name}");
            }
        }
        let picker = project_mod_skill_picker(runtime.as_ref(), case["player"].as_str().unwrap());
        assert_eq!(picker.is_none(), case["picker"].is_null(), "{name}: picker visibility");
        if let Some(picker) = picker {
            let offered: Vec<_> = picker.skills.iter().map(|row| serde_json::json!({
                "name": row.name,
                "description": if row.description.is_empty() { format!("Rank {}", row.next_rank) } else { row.description.to_owned() },
            })).collect();
            assert_eq!(serde_json::json!(offered), case["picker"]["offered"], "{name}: offered");
        }
    }
}
