//! The snapshot comparison tree keeps JSON object fields in wire order.
//!
//! Contiguous fields avoid an indexed ordered map for every small game record.
//! This is private to snapshot compaction; other JSON consumers keep their maps.

use serde::de::value::{
    BorrowedStrDeserializer, MapAccessDeserializer, MapDeserializer, SeqDeserializer,
};
use serde::de::{Error as _, IntoDeserializer, MapAccess, SeqAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Error, Number};
use std::collections::HashMap;
use std::fmt;

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) enum Value {
    #[default]
    Null,
    Bool(bool),
    Number(Number),
    String(String),
    Array(Vec<Self>),
    Object(Vec<(String, Self)>),
}

impl Value {
    pub(super) fn as_str(&self) -> Option<&str> {
        if let Self::String(value) = self { Some(value) } else { None }
    }

    pub(super) fn as_u64(&self) -> Option<u64> {
        if let Self::Number(value) = self { value.as_u64() } else { None }
    }

    pub(super) fn as_object(&self) -> Option<&Vec<(String, Self)>> {
        if let Self::Object(value) = self { Some(value) } else { None }
    }

    pub(super) fn as_object_mut(&mut self) -> Option<&mut Vec<(String, Self)>> {
        if let Self::Object(value) = self { Some(value) } else { None }
    }

    pub(super) fn as_array_mut(&mut self) -> Option<&mut Vec<Self>> {
        if let Self::Array(value) = self { Some(value) } else { None }
    }

    pub(super) fn get(&self, key: &str) -> Option<&Self> {
        self.as_object()?.iter().find(|(name, _)| name == key).map(|(_, value)| value)
    }

    pub(super) fn get_mut(&mut self, key: &str) -> Option<&mut Self> {
        self.as_object_mut()?.iter_mut().find(|(name, _)| name == key).map(|(_, value)| value)
    }

    pub(super) fn take(&mut self) -> Self {
        std::mem::take(self)
    }
}

impl Serialize for Value {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Null => serializer.serialize_unit(),
            Self::Bool(value) => serializer.serialize_bool(*value),
            Self::Number(value) => value.serialize(serializer),
            Self::String(value) => serializer.serialize_str(value),
            Self::Array(values) => values.serialize(serializer),
            Self::Object(fields) => {
                let mut object = serializer.serialize_map(Some(fields.len()))?;
                for (key, value) in fields {
                    object.serialize_entry(key, value)?;
                }
                object.end()
            }
        }
    }
}

impl<'de> Deserialize<'de> for Value {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ValueVisitor;
        impl<'de> Visitor<'de> for ValueVisitor {
            type Value = Value;
            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a JSON snapshot value")
            }
            fn visit_unit<E>(self) -> Result<Value, E> {
                Ok(Value::Null)
            }
            fn visit_bool<E>(self, value: bool) -> Result<Value, E> {
                Ok(Value::Bool(value))
            }
            fn visit_i64<E>(self, value: i64) -> Result<Value, E> {
                Ok(Value::Number(value.into()))
            }
            fn visit_u64<E>(self, value: u64) -> Result<Value, E> {
                Ok(Value::Number(value.into()))
            }
            fn visit_f64<E: serde::de::Error>(self, value: f64) -> Result<Value, E> {
                Number::from_f64(value)
                    .map(Value::Number)
                    .ok_or_else(|| E::custom("nonfinite snapshot number"))
            }
            fn visit_str<E>(self, value: &str) -> Result<Value, E> {
                Ok(Value::String(value.to_owned()))
            }
            fn visit_string<E>(self, value: String) -> Result<Value, E> {
                Ok(Value::String(value))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Value, A::Error> {
                let mut values = Vec::new();
                while let Some(value) = sequence.next_element()? {
                    values.push(value);
                }
                Ok(Value::Array(values))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
                let mut fields: Vec<(String, Value)> = Vec::new();
                let mut positions: Option<HashMap<String, usize>> = None;
                while let Some((key, value)) = map.next_entry::<String, Value>()? {
                    // Most game records are small. Index wide user-authored objects
                    // so duplicate-key handling cannot make parsing quadratic.
                    let existing = positions.as_ref().map_or_else(
                        || fields.iter().position(|(name, _)| *name == key),
                        |positions| positions.get(&key).copied(),
                    );
                    if let Some(index) = existing {
                        fields[index].1 = value;
                    } else {
                        if fields.len() == 32 {
                            positions = Some(
                                fields
                                    .iter()
                                    .enumerate()
                                    .map(|(index, (name, _))| (name.clone(), index))
                                    .collect(),
                            );
                        }
                        if let Some(positions) = &mut positions {
                            positions.insert(key.clone(), fields.len());
                        }
                        fields.push((key, value));
                    }
                }
                Ok(Value::Object(fields))
            }
        }
        deserializer.deserialize_any(ValueVisitor)
    }
}

impl<'de> IntoDeserializer<'de, Error> for &'de Value {
    type Deserializer = Self;
    fn into_deserializer(self) -> Self {
        self
    }
}

impl<'de> Deserializer<'de> for &'de Value {
    type Error = Error;
    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self {
            Value::Null => visitor.visit_unit(),
            Value::Bool(value) => visitor.visit_bool(*value),
            Value::Number(value) => value.deserialize_any(visitor),
            Value::String(value) => visitor.visit_borrowed_str(value),
            Value::Array(values) => SeqDeserializer::new(values.iter()).deserialize_any(visitor),
            Value::Object(fields) => {
                MapDeserializer::new(fields.iter().map(|(key, value)| (Key(key), value)))
                    .deserialize_any(visitor)
            }
        }
    }
    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        if matches!(self, Value::Null) { visitor.visit_none() } else { visitor.visit_some(self) }
    }
    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Error> {
        visitor.visit_newtype_struct(self)
    }
    fn deserialize_enum<V: Visitor<'de>>(
        self,
        name: &'static str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error> {
        match self {
            Value::String(value) => {
                BorrowedStrDeserializer::new(value).deserialize_enum(name, variants, visitor)
            }
            Value::Object(fields) if fields.len() == 1 => MapAccessDeserializer::new(
                MapDeserializer::new(fields.iter().map(|(key, value)| (Key(key), value))),
            )
            .deserialize_enum(name, variants, visitor),
            _ => Err(Error::custom("expected a string or single-field enum object")),
        }
    }
    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf unit unit_struct seq tuple tuple_struct map struct identifier ignored_any
    }
}

// JSON object keys are strings, including integer-keyed game-state maps.
struct Key<'a>(&'a str);
impl<'de> IntoDeserializer<'de, Error> for Key<'de> {
    type Deserializer = Self;
    fn into_deserializer(self) -> Self {
        self
    }
}
macro_rules! numeric_keys {
    ($($method:ident),* $(,)?) => {$ (
        fn $method<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
            if self.0.trim_end() != self.0
                || !matches!(self.0.as_bytes().first(), Some(b'-' | b'0'..=b'9'))
            {
                return Err(Error::custom("expected a numeric JSON object key"));
            }
            let mut decoder = serde_json::Deserializer::from_str(self.0);
            let value = (&mut decoder).$method(visitor)?;
            decoder.end()?;
            Ok(value)
        }
    )*};
}
impl<'de> Deserializer<'de> for Key<'de> {
    type Error = Error;
    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_borrowed_str(self.0)
    }
    numeric_keys!(
        deserialize_i8,
        deserialize_i16,
        deserialize_i32,
        deserialize_i64,
        deserialize_i128,
        deserialize_u8,
        deserialize_u16,
        deserialize_u32,
        deserialize_u64,
        deserialize_u128,
        deserialize_f32,
        deserialize_f64
    );
    fn deserialize_bool<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self.0 {
            "true" => visitor.visit_bool(true),
            "false" => visitor.visit_bool(false),
            _ => Err(Error::custom("expected a boolean JSON object key")),
        }
    }
    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_some(self)
    }
    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Error> {
        visitor.visit_newtype_struct(self)
    }
    fn deserialize_enum<V: Visitor<'de>>(
        self,
        name: &'static str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error> {
        BorrowedStrDeserializer::new(self.0).deserialize_enum(name, variants, visitor)
    }
    serde::forward_to_deserialize_any! {
        char str string bytes byte_buf unit unit_struct seq tuple tuple_struct map struct identifier ignored_any
    }
}
