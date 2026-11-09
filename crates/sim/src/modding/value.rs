//! JSON-compatible Lua values.
//!
//! Objects retain insertion order: state-cell revisions compare values by their JSON text,
//! so sorting an object's fields would change that observable contract.

/// A JSON-compatible Lua value: null, a Boolean, a number, a string, an array or an object.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize))]
#[cfg_attr(feature = "persistence", serde(untagged))]
pub enum LuaValue {
    #[default]
    Null,
    Boolean(bool),
    Number(f64),
    String(String),
    Array(Vec<Self>),
    Object(LuaObject),
}

#[cfg(feature = "persistence")]
impl<'de> serde::Deserialize<'de> for LuaValue {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ValueVisitor;

        impl<'de> serde::de::Visitor<'de> for ValueVisitor {
            type Value = LuaValue;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a JSON-compatible Lua value")
            }

            fn visit_unit<E>(self) -> Result<LuaValue, E> {
                Ok(LuaValue::Null)
            }

            fn visit_bool<E>(self, value: bool) -> Result<LuaValue, E> {
                Ok(LuaValue::Boolean(value))
            }

            fn visit_i64<E>(self, value: i64) -> Result<LuaValue, E> {
                Ok(LuaValue::Number(value as f64))
            }

            fn visit_u64<E>(self, value: u64) -> Result<LuaValue, E> {
                Ok(LuaValue::Number(value as f64))
            }

            fn visit_f64<E>(self, value: f64) -> Result<LuaValue, E> {
                Ok(LuaValue::Number(value))
            }

            fn visit_str<E>(self, value: &str) -> Result<LuaValue, E> {
                Ok(LuaValue::String(value.to_owned()))
            }

            fn visit_string<E>(self, value: String) -> Result<LuaValue, E> {
                Ok(LuaValue::String(value))
            }

            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut sequence: A,
            ) -> Result<LuaValue, A::Error> {
                let mut values = Vec::new();
                while let Some(value) = sequence.next_element()? {
                    values.push(value);
                }
                Ok(LuaValue::Array(values))
            }

            fn visit_map<A: serde::de::MapAccess<'de>>(self, map: A) -> Result<LuaValue, A::Error> {
                <LuaObject as serde::Deserialize>::deserialize(
                    serde::de::value::MapAccessDeserializer::new(map),
                )
                .map(LuaValue::Object)
            }
        }

        // Trying untagged variants first buffers every nested value repeatedly.
        // The incoming JSON kind already selects the one valid Lua representation.
        deserializer.deserialize_any(ValueVisitor)
    }
}

impl LuaValue {
    #[must_use]
    pub fn as_object(&self) -> Option<&LuaObject> {
        match self {
            Self::Object(value) => Some(value),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_array(&self) -> Option<&[Self]> {
        match self {
            Self::Array(value) => Some(value),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value),
            _ => None,
        }
    }

    #[must_use]
    pub const fn as_number(&self) -> Option<f64> {
        match self {
            Self::Number(value) => Some(*value),
            _ => None,
        }
    }

    #[must_use]
    pub const fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Boolean(value) => Some(*value),
            _ => None,
        }
    }

    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Self> {
        self.as_object()?.get(key)
    }
}

/// A Lua/JSON object whose fields follow JavaScript object property order: reassigning a
/// key keeps its position, and array-index keys come first in numeric order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LuaObject {
    fields: Vec<(String, LuaValue)>,
}

impl LuaObject {
    #[must_use]
    pub const fn new() -> Self {
        Self { fields: Vec::new() }
    }

    #[must_use]
    pub fn get(&self, key: &str) -> Option<&LuaValue> {
        for (name, value) in &self.fields {
            if name == key {
                return Some(value);
            }
        }
        None
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut LuaValue> {
        for (name, value) in &mut self.fields {
            if name == key {
                return Some(value);
            }
        }
        None
    }

    pub fn insert(&mut self, key: String, value: LuaValue) -> Option<LuaValue> {
        for (name, previous) in &mut self.fields {
            if *name == key {
                return Some(std::mem::replace(previous, value));
            }
        }
        if let Some(index) = javascript_array_index(&key) {
            // Array-index keys come first, in numeric order, whatever order
            // they were inserted in.
            let position = self.fields.iter().position(|(name, _)| {
                javascript_array_index(name).is_none_or(|existing| existing > index)
            });
            if let Some(position) = position {
                self.fields.insert(position, (key, value));
                return None;
            }
        }
        self.fields.push((key, value));
        None
    }

    pub fn remove(&mut self, key: &str) -> Option<LuaValue> {
        let index = self.fields.iter().position(|(name, _)| name == key)?;
        Some(self.fields.remove(index).1)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.fields.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &LuaValue)> {
        self.fields.iter().map(|(key, value)| (key.as_str(), value))
    }

    pub fn into_fields(self) -> Vec<(String, LuaValue)> {
        self.fields
    }
}

fn javascript_array_index(key: &str) -> Option<u32> {
    let index = key.parse::<u32>().ok()?;
    if index == u32::MAX || index.to_string() != key {
        return None;
    }
    Some(index)
}

impl FromIterator<(String, LuaValue)> for LuaObject {
    fn from_iter<T: IntoIterator<Item = (String, LuaValue)>>(fields: T) -> Self {
        let mut object = Self::new();
        for (key, value) in fields {
            object.insert(key, value);
        }
        object
    }
}

#[cfg(feature = "persistence")]
impl serde::Serialize for LuaObject {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;
        let mut object = serializer.serialize_map(Some(self.fields.len()))?;
        for (key, value) in &self.fields {
            object.serialize_entry(key, value)?;
        }
        object.end()
    }
}

#[cfg(feature = "persistence")]
impl<'de> serde::Deserialize<'de> for LuaObject {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct ObjectVisitor;

        impl<'de> serde::de::Visitor<'de> for ObjectVisitor {
            type Value = LuaObject;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a Lua object")
            }

            fn visit_map<A>(self, mut map: A) -> Result<LuaObject, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let mut object = LuaObject::new();
                while let Some((key, value)) = map.next_entry::<String, LuaValue>()? {
                    object.insert(key, value);
                }
                Ok(object)
            }
        }

        deserializer.deserialize_map(ObjectVisitor)
    }
}

#[cfg(all(test, feature = "persistence"))]
mod tests {
    use super::LuaValue;

    #[test]
    fn json_values_keep_number_bits_and_javascript_property_order() {
        let value: LuaValue = serde_json::from_str(
            r#"{"tail":[null,true,false,-0.0,9007199254740991,18446744073709551615,0.1,"🧙\n"],"10":"ten","2":"two","name":"first","name":"last"}"#,
        )
        .unwrap();
        let object = value.as_object().unwrap();
        assert_eq!(
            object.iter().map(|(key, _)| key).collect::<Vec<_>>(),
            ["2", "10", "tail", "name"]
        );
        assert_eq!(object.get("name").unwrap().as_str(), Some("last"));
        let array = object.get("tail").unwrap().as_array().unwrap();
        assert_eq!(array[0], LuaValue::Null);
        assert_eq!(array[1].as_bool(), Some(true));
        assert_eq!(array[2].as_bool(), Some(false));
        for (index, expected) in
            [(3, -0.0_f64), (4, 9_007_199_254_740_991.0), (5, u64::MAX as f64), (6, 0.1)]
        {
            assert_eq!(array[index].as_number().unwrap().to_bits(), expected.to_bits());
        }
        assert_eq!(array[7].as_str(), Some("🧙\n"));
        let encoded = serde_json::to_string(&value).unwrap();
        assert_eq!(serde_json::from_str::<LuaValue>(&encoded).unwrap(), value);
        let from_value =
            serde_json::from_value::<LuaValue>(serde_json::to_value(&value).unwrap()).unwrap();
        assert_eq!(from_value.get("tail"), value.get("tail"));
        assert!(serde_json::from_str::<LuaValue>("[true,]").is_err());
    }
}
