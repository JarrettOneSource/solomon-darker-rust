use serde_json::Value;

const TRIG_TOLERANCE: f64 = 1e-9;

/// JSON carries integers and floating values without a declared numeric type. Compare their
/// values while retaining exact arithmetic; only named trigonometric paths get a tolerance.
pub fn assert_website_json(actual: &Value, expected: &Value, path: &str, trig_paths: &[&str]) {
    match (actual, expected) {
        (Value::Object(actual), Value::Object(expected)) => {
            assert_eq!(
                actual.len(),
                expected.len(),
                "{path}: extra {:?}; missing {:?}",
                actual.keys().filter(|key| !expected.contains_key(*key)).collect::<Vec<_>>(),
                expected.keys().filter(|key| !actual.contains_key(*key)).collect::<Vec<_>>()
            );
            for (key, expected) in expected {
                assert_website_json(&actual[key], expected, &format!("{path}.{key}"), trig_paths);
            }
        }
        (Value::Array(actual), Value::Array(expected)) => {
            assert_eq!(actual.len(), expected.len(), "{path}");
            for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
                assert_website_json(actual, expected, &format!("{path}[{index}]"), trig_paths);
            }
        }
        (Value::Number(actual), Value::Number(expected)) => {
            let actual = actual.as_f64().unwrap();
            let expected = expected.as_f64().unwrap();
            if trig_paths.iter().any(|field| path.ends_with(field)) {
                assert!(
                    (actual - expected).abs() <= TRIG_TOLERANCE,
                    "{path}: {actual} != {expected}"
                );
            } else {
                assert_eq!(actual, expected, "{path}");
            }
        }
        _ => assert_eq!(actual, expected, "{path}"),
    }
}
