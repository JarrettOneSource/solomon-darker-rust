//! The matching writer preserves source spellings without changing native checkpoints.

use serde::ser::{self, Serialize, SerializeMap, SerializeSeq, SerializeStruct};
use serde_json::{Map, Value};

use super::{kebab, source_field, source_variant};

pub fn encode<T: Serialize + ?Sized>(value: &T) -> Result<Value, String> {
    value.serialize(SourceSerializer).map_err(|error| error.to_string())
}

struct SourceSerializer;

macro_rules! scalar {
    ($($method:ident($kind:ty)),* $(,)?) => {
        $(fn $method(self, value: $kind) -> Result<Value, Self::Error> {
            serde_json::to_value(value)
        })*
    };
}

impl ser::Serializer for SourceSerializer {
    type Ok = Value;
    type Error = serde_json::Error;
    type SerializeSeq = SourceSequence;
    type SerializeTuple = SourceSequence;
    type SerializeTupleStruct = SourceSequence;
    type SerializeTupleVariant = SourceSequence;
    type SerializeMap = SourceMap;
    type SerializeStruct = SourceStruct;
    type SerializeStructVariant = SourceStruct;

    scalar! {
        serialize_bool(bool), serialize_i8(i8), serialize_i16(i16), serialize_i32(i32),
        serialize_i64(i64), serialize_i128(i128), serialize_u8(u8), serialize_u16(u16),
        serialize_u32(u32), serialize_u64(u64), serialize_u128(u128),
        serialize_f32(f32), serialize_f64(f64), serialize_char(char), serialize_str(&str)
    }

    fn serialize_bytes(self, value: &[u8]) -> Result<Value, Self::Error> {
        Ok(Value::Array(value.iter().map(|byte| Value::from(*byte)).collect()))
    }

    fn serialize_none(self) -> Result<Value, Self::Error> {
        Ok(Value::Null)
    }
    fn serialize_unit(self) -> Result<Value, Self::Error> {
        Ok(Value::Null)
    }
    fn serialize_unit_struct(self, _name: &'static str) -> Result<Value, Self::Error> {
        Ok(Value::Null)
    }

    fn serialize_some<T: Serialize + ?Sized>(self, value: &T) -> Result<Value, Self::Error> {
        value.serialize(self)
    }

    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Value, Self::Error> {
        value.serialize(self)
    }

    fn serialize_unit_variant(
        self,
        name: &'static str,
        index: u32,
        variant: &'static str,
    ) -> Result<Value, Self::Error> {
        if matches!(name, "NativeTutorialStage" | "BoneyardGoodiePhase") {
            return Ok(Value::from(index));
        }
        let tag = source_variant(name, variant);
        if name == "EnemyFamilyConfig" {
            return Ok(match variant {
                "Wraith" => serde_json::json!({"dazzle":true}),
                "Cocoon" => serde_json::json!({"kind":"cocoon"}),
                _ => unreachable!("other enemy families carry their payload"),
            });
        }
        Ok(
            if matches!(
                name,
                "NativeBeltEntry" | "NativeSpentEmber" | "SpiderCommand" | "NativeSkillBookOutcome"
            ) {
                serde_json::json!({"kind": tag})
            } else {
                Value::String(tag)
            },
        )
    }

    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        name: &'static str,
        _index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Value, Self::Error> {
        let mut payload = value.serialize(self)?;
        if super::spells::transparent_wrapper(name, variant) {
            return Ok(payload);
        }
        let fields = payload.as_object_mut().ok_or_else(|| {
            ser::Error::custom(format!("{name}::{variant} requires an object payload"))
        })?;
        if name == "EnemyFamilyConfig" {
            return Ok(payload);
        }
        let tag_field = if name == "EnemyBrain" { "family" } else { "kind" };
        fields.insert(tag_field.into(), Value::String(source_variant(name, variant)));
        Ok(payload)
    }

    fn serialize_seq(self, length: Option<usize>) -> Result<SourceSequence, Self::Error> {
        Ok(SourceSequence { values: Vec::with_capacity(length.unwrap_or(0)), variant: None })
    }
    fn serialize_tuple(self, length: usize) -> Result<SourceSequence, Self::Error> {
        self.serialize_seq(Some(length))
    }
    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        length: usize,
    ) -> Result<SourceSequence, Self::Error> {
        self.serialize_seq(Some(length))
    }
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
        length: usize,
    ) -> Result<SourceSequence, Self::Error> {
        Ok(SourceSequence { values: Vec::with_capacity(length), variant: Some(variant) })
    }
    fn serialize_map(self, _length: Option<usize>) -> Result<SourceMap, Self::Error> {
        Ok(SourceMap { values: Map::new(), key: None })
    }
    fn serialize_struct(
        self,
        name: &'static str,
        _length: usize,
    ) -> Result<SourceStruct, Self::Error> {
        Ok(SourceStruct { name, values: Map::new(), variant: None })
    }
    fn serialize_struct_variant(
        self,
        name: &'static str,
        _index: u32,
        variant: &'static str,
        _length: usize,
    ) -> Result<SourceStruct, Self::Error> {
        Ok(SourceStruct { name, values: Map::new(), variant: Some(variant) })
    }
}

struct SourceSequence {
    values: Vec<Value>,
    variant: Option<&'static str>,
}

impl SerializeSeq for SourceSequence {
    type Ok = Value;
    type Error = serde_json::Error;
    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Self::Error> {
        self.values.push(value.serialize(SourceSerializer)?);
        Ok(())
    }
    fn end(self) -> Result<Value, Self::Error> {
        Ok(match self.variant {
            None => Value::Array(self.values),
            Some(variant) => serde_json::json!({"kind": kebab(variant), "values": self.values}),
        })
    }
}

macro_rules! sequence {
    ($trait:ident, $method:ident) => {
        impl ser::$trait for SourceSequence {
            type Ok = Value;
            type Error = serde_json::Error;
            fn $method<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Self::Error> {
                SerializeSeq::serialize_element(self, value)
            }
            fn end(self) -> Result<Value, Self::Error> {
                SerializeSeq::end(self)
            }
        }
    };
}
sequence!(SerializeTuple, serialize_element);
sequence!(SerializeTupleStruct, serialize_field);
sequence!(SerializeTupleVariant, serialize_field);

struct SourceMap {
    values: Map<String, Value>,
    key: Option<String>,
}

impl SerializeMap for SourceMap {
    type Ok = Value;
    type Error = serde_json::Error;
    fn serialize_key<T: Serialize + ?Sized>(&mut self, key: &T) -> Result<(), Self::Error> {
        self.key = Some(match serde_json::to_value(key)? {
            Value::String(key) => key,
            Value::Number(key) => key.to_string(),
            _ => return Err(ser::Error::custom("Website object keys must be strings or integers")),
        });
        Ok(())
    }
    fn serialize_value<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Self::Error> {
        self.values.insert(
            self.key.take().expect("map value follows its key"),
            value.serialize(SourceSerializer)?,
        );
        Ok(())
    }
    fn end(self) -> Result<Value, Self::Error> {
        Ok(Value::Object(self.values))
    }
}

struct SourceStruct {
    name: &'static str,
    values: Map<String, Value>,
    variant: Option<&'static str>,
}

impl SerializeStruct for SourceStruct {
    type Ok = Value;
    type Error = serde_json::Error;
    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error> {
        self.values.insert(source_field(self.name, key), value.serialize(SourceSerializer)?);
        Ok(())
    }
    fn end(mut self) -> Result<Value, Self::Error> {
        if let Some(variant) = self.variant {
            self.values.insert("kind".into(), Value::String(source_variant(self.name, variant)));
        }
        if let Some(common) = self.values.remove("common") {
            let Value::Object(fields) = common else {
                return Err(ser::Error::custom("actor common fields must be an object"));
            };
            self.values.extend(fields);
        }
        match self.name {
            "PlayerPrimaryCastState" => {
                for field in ["actionTick", "selectedPrimaryId"] {
                    if self.values.get(field).is_some_and(Value::is_null) {
                        self.values.insert(field.into(), Value::from(-1));
                    }
                }
            }
            "PrimarySpellProjectileBase" => {
                self.values.insert(
                    "painterRegistrations".into(),
                    serde_json::json!([self.values["lightRegistration"]]),
                );
            }
            "BoneyardSpawn" => {
                let position = self.values.remove("position").expect("spawn owns its position");
                self.values.extend(position.as_object().expect("position has x and y").clone());
            }
            "BoneyardObject" | "BoneyardSprite" | "BoneyardRoad" | "BoneyardFence"
            | "BoneyardTerrain" => {
                self.values.retain(|_, value| !value.is_null());
            }
            "WaveDef" => {
                if self.values.get("zombieWave") == Some(&Value::Bool(false)) {
                    self.values.remove("zombieWave");
                }
            }
            "HubStudentState" => {
                let ordinal =
                    self.values.remove("registrationOrdinal").expect("student owns its painter");
                self.values.insert(
                    "painterRegistration".into(),
                    serde_json::json!({"managerLane":"actor", "registrationOrdinal":ordinal}),
                );
            }
            "EnemyWorld" => {
                for field in [
                    "spawnedActorIds",
                    "events",
                    "playerDamage",
                    "playerKnockbacks",
                    "pendingSpawnIntents",
                    "retired",
                    "rewards",
                    "playerPositions",
                ] {
                    self.values.remove(field);
                }
            }
            "NativeRng" => {
                self.values.remove("divisor");
            }
            "HubActionFeedback" => {
                if self.values.get("dyeSessionId").is_some_and(Value::is_null) {
                    self.values.remove("dyeSessionId");
                }
            }
            "HubShopItem" => {
                let Value::Object(mut item) =
                    self.values.remove("item").expect("shop item owns its item")
                else {
                    return Err(ser::Error::custom("shop item requires an object"));
                };
                item.extend(self.values);
                self.values = item;
            }
            "PlayerBeltComponent" => {
                return Ok(self.values.remove("entries").expect("belt owns its entries"));
            }
            "WorldPainterOrder" => {
                return Ok(serde_json::json!({"nextRegistrationOrdinal": {
                    "actor": self.values["nextActorOrdinal"], "transient": self.values["nextTransientOrdinal"]
                }}));
            }
            "HubInventoryItem" => {
                // These source fields are optional rather than explicitly nullable.
                for field in [
                    "contents",
                    "generatedLevel",
                    "iconTints",
                    "inventorySlot",
                    "nativeSelector",
                    "nativeEffects",
                ] {
                    if self.values.get(field).is_some_and(Value::is_null) {
                        self.values.remove(field);
                    }
                }
            }
            _ => {}
        }
        let mut result = Value::Object(self.values);
        super::enemies::finish(self.name, &mut result);
        super::spells::finish(self.name, &mut result);
        Ok(result)
    }
}

impl ser::SerializeStructVariant for SourceStruct {
    type Ok = Value;
    type Error = serde_json::Error;
    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error> {
        SerializeStruct::serialize_field(self, key, value)
    }
    fn end(self) -> Result<Value, Self::Error> {
        SerializeStruct::end(self)
    }
}
