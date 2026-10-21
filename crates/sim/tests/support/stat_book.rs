//! Authored source rows at the differential seam; the sim's default build has no serde.

use serde_json::Value;
use solomon_sim::profile::{
    NativeSkillStatBookEntry, NativeSkillStatBookValue, PlayerStatBookComponent,
};

pub fn read(value: Option<&Value>) -> PlayerStatBookComponent {
    let Some(value) = value else { return PlayerStatBookComponent::default() };
    let mut entries = Vec::new();
    for row in value["entries"].as_array().unwrap() {
        let mut numeric_properties = std::collections::BTreeMap::new();
        for (name, value) in row["numericProperties"].as_object().unwrap() {
            let property = match value.as_array() {
                Some(values) => NativeSkillStatBookValue::Ranks(
                    values.iter().map(|value| value.as_f64().unwrap()).collect(),
                ),
                None => NativeSkillStatBookValue::Scalar(value.as_f64().unwrap()),
            };
            numeric_properties.insert(name.clone(), property);
        }
        entries.push(NativeSkillStatBookEntry {
            cap_level: row["capLevel"].as_u64().unwrap() as u32,
            description: row["description"].as_str().unwrap().to_owned(),
            id: row["id"].as_u64().unwrap() as usize,
            maximum_level: row["maximumLevel"].as_u64().unwrap() as u32,
            numeric_properties,
            quick_description: row["quickDescription"].as_str().unwrap().to_owned(),
        });
    }
    PlayerStatBookComponent { entries }
}
