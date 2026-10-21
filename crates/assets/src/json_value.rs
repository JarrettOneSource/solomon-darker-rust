//! Source JSON spelling shared by native profiles, source saves and content digests.

use std::io;

use serde::Serialize;
use serde_json::ser::Formatter;
use solomon_sim::modding::LuaValue;

/// JSON.stringify for the source's finite, JSON-compatible Lua value domain.
pub fn stringify(value: &LuaValue) -> Result<String, String> {
    validate_finite(value)?;
    stringify_serializable(value)
}

/// Preserve a record's field order and use ECMAScript spelling for its numbers.
pub fn stringify_serializable<T: Serialize + ?Sized>(value: &T) -> Result<String, String> {
    let mut bytes = Vec::new();
    let mut serializer = serde_json::Serializer::with_formatter(&mut bytes, SourceFormatter);
    value.serialize(&mut serializer).map_err(|error| error.to_string())?;
    Ok(String::from_utf8(bytes).expect("the JSON serializer writes valid UTF-8"))
}

struct SourceFormatter;

impl Formatter for SourceFormatter {
    fn write_f32<W: io::Write + ?Sized>(&mut self, writer: &mut W, value: f32) -> io::Result<()> {
        // A native f32 becomes a JS Number before JSON.stringify sees it.
        self.write_f64(writer, f64::from(value))
    }

    fn write_f64<W: io::Write + ?Sized>(&mut self, writer: &mut W, value: f64) -> io::Result<()> {
        writer.write_all(ryu_js::Buffer::new().format(value).as_bytes())
    }
}

fn validate_finite(value: &LuaValue) -> Result<(), String> {
    match value {
        LuaValue::Number(value) if !value.is_finite() => {
            return Err("Lua JSON number must be finite".to_owned());
        }
        LuaValue::Array(values) => {
            for value in values {
                validate_finite(value)?;
            }
        }
        LuaValue::Object(values) => {
            for (_, value) in values.iter() {
                validate_finite(value)?;
            }
        }
        _ => {}
    }
    Ok(())
}
