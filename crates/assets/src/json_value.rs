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

/// The Console uses JSON.stringify(value, null, 2), including source number
/// spelling and Lua object insertion order. It needs no JavaScript evaluator.
pub fn stringify_pretty<T: Serialize + ?Sized>(value: &T) -> Result<String, String> {
    let mut bytes = Vec::new();
    let mut serializer = serde_json::Serializer::with_formatter(
        &mut bytes,
        SourcePrettyFormatter(serde_json::ser::PrettyFormatter::new()),
    );
    value.serialize(&mut serializer).map_err(|error| error.to_string())?;
    Ok(String::from_utf8(bytes).expect("the JSON serializer writes valid UTF-8"))
}

struct SourcePrettyFormatter(serde_json::ser::PrettyFormatter<'static>);

impl Formatter for SourcePrettyFormatter {
    fn write_f32<W: io::Write + ?Sized>(&mut self, writer: &mut W, value: f32) -> io::Result<()> {
        SourceFormatter.write_f32(writer, value)
    }
    fn write_f64<W: io::Write + ?Sized>(&mut self, writer: &mut W, value: f64) -> io::Result<()> {
        SourceFormatter.write_f64(writer, value)
    }
    fn begin_array<W: io::Write + ?Sized>(&mut self, writer: &mut W) -> io::Result<()> {
        self.0.begin_array(writer)
    }
    fn end_array<W: io::Write + ?Sized>(&mut self, writer: &mut W) -> io::Result<()> {
        self.0.end_array(writer)
    }
    fn begin_array_value<W: io::Write + ?Sized>(
        &mut self,
        writer: &mut W,
        first: bool,
    ) -> io::Result<()> {
        self.0.begin_array_value(writer, first)
    }
    fn end_array_value<W: io::Write + ?Sized>(&mut self, writer: &mut W) -> io::Result<()> {
        self.0.end_array_value(writer)
    }
    fn begin_object<W: io::Write + ?Sized>(&mut self, writer: &mut W) -> io::Result<()> {
        self.0.begin_object(writer)
    }
    fn end_object<W: io::Write + ?Sized>(&mut self, writer: &mut W) -> io::Result<()> {
        self.0.end_object(writer)
    }
    fn begin_object_key<W: io::Write + ?Sized>(
        &mut self,
        writer: &mut W,
        first: bool,
    ) -> io::Result<()> {
        self.0.begin_object_key(writer, first)
    }
    fn end_object_key<W: io::Write + ?Sized>(&mut self, writer: &mut W) -> io::Result<()> {
        self.0.end_object_key(writer)
    }
    fn begin_object_value<W: io::Write + ?Sized>(&mut self, writer: &mut W) -> io::Result<()> {
        self.0.begin_object_value(writer)
    }
    fn end_object_value<W: io::Write + ?Sized>(&mut self, writer: &mut W) -> io::Result<()> {
        self.0.end_object_value(writer)
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use solomon_sim::modding::LuaObject;

    #[test]
    fn console_json_preserves_source_numbers_order_and_empty_containers() {
        let mut object = LuaObject::new();
        object.insert("z".into(), LuaValue::Number(-0.0));
        object.insert(
            "a".into(),
            LuaValue::Array(vec![
                LuaValue::Number(1.0),
                LuaValue::Number(1e21),
                LuaValue::Number(1e-7),
                LuaValue::Array(Vec::new()),
                LuaValue::Object(LuaObject::new()),
            ]),
        );
        assert_eq!(
            stringify_pretty(&object).unwrap(),
            "{\n  \"z\": 0,\n  \"a\": [\n    1,\n    1e+21,\n    1e-7,\n    [],\n    {}\n  ]\n}"
        );
    }
}
