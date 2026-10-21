//! JSON-compatible Lua values from `protocol/codecs/lua.ts`.
//!
//! Objects retain insertion order: the source uses JSON equality for state-cell
//! revisions, so sorting an object's fields would change that observable contract.

#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
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

/// A source Lua/JSON object. Reassigning a key preserves its original position,
/// just as assigning a property of an existing JavaScript object does.
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
            // JavaScript enumerates array-index properties first, in numeric
            // order, even when Object.fromEntries inserted them in another order.
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
