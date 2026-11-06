//! Source JSON field adaptation and exact floating-point fixture comparison.

use serde::de::DeserializeOwned;
use serde_json::Value;

// The fixture preserves Source field names; the domain checkpoint uses snake_case.
pub fn source_value(value: &Value) -> Value {
    match value {
        Value::Array(rows) => Value::Array(rows.iter().map(source_value).collect()),
        Value::Object(fields) => {
            let mut output = serde_json::Map::new();
            for (key, value) in fields {
                let mut name = String::new();
                for letter in key.chars() {
                    if letter.is_ascii_uppercase() {
                        name.push('_');
                        name.push(letter.to_ascii_lowercase());
                    } else {
                        name.push(letter);
                    }
                }
                output.insert(name, source_value(value));
            }
            Value::Object(output)
        }
        other => other.clone(),
    }
}
pub fn input<T: DeserializeOwned>(value: &Value) -> T {
    serde_json::from_value(source_value(value)).unwrap()
}
pub fn compare(actual: &Value, expected: &Value, path: &str) {
    match expected {
        Value::String(bits) if bits.starts_with("f64:") => {
            let expected = u64::from_str_radix(&bits[4..], 16).unwrap();
            assert_eq!(actual.as_f64().unwrap().to_bits(), expected, "{path}");
        }
        Value::Array(rows) => {
            let actual = actual.as_array().unwrap();
            assert_eq!(actual.len(), rows.len(), "{path} length");
            for (index, (actual, expected)) in actual.iter().zip(rows).enumerate() {
                compare(actual, expected, &format!("{path}[{index}]"));
            }
        }
        Value::Object(fields) => {
            let actual = actual.as_object().unwrap();
            assert_eq!(
                actual.keys().collect::<Vec<_>>(),
                fields.keys().collect::<Vec<_>>(),
                "{path} fields"
            );
            for (key, expected) in fields {
                compare(&actual[key], expected, &format!("{path}.{key}"));
            }
        }
        other => assert_eq!(actual, other, "{path}"),
    }
}
