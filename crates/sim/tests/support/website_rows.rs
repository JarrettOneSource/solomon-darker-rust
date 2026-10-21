use serde_json::Value;

// Compare complete native rows. Unrounded trig output permits the small platform
// difference between Darwin libm and V8; float32 state and RNG words remain exact.
pub fn compare(actual: &Value, expected: &Value, path: &str) {
    match (actual, expected) {
        (Value::Number(actual), Value::Number(expected)) => {
            let actual = actual.as_f64().unwrap();
            let expected = expected.as_f64().unwrap();
            if path.ends_with("headingDegrees") || path.ends_with("headingDeg") {
                assert!((actual - expected).abs() < 1e-9, "{path}: {actual} != {expected}");
            } else if path.contains(".direction[")
                || path.ends_with(".direction.x")
                || path.ends_with(".direction.y")
            {
                assert!((actual - expected).abs() < 1e-12, "{path}: {actual} != {expected}");
            } else {
                assert_eq!(actual, expected, "{path}");
            }
        }
        (Value::Array(actual), Value::Array(expected)) => {
            assert_eq!(actual.len(), expected.len(), "{path}");
            for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
                compare(actual, expected, &format!("{path}[{index}]"));
            }
        }
        (Value::Object(actual), Value::Object(expected)) => {
            assert_eq!(actual.len(), expected.len(), "{path}: object fields");
            for (key, expected) in expected {
                compare(&actual[key], expected, &format!("{path}.{key}"));
            }
        }
        _ => assert_eq!(actual, expected, "{path}"),
    }
}
