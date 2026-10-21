//! Serde boundary for source field spelling. Struct fields are translated from their
//! declared Rust names; arbitrary map keys (player IDs and mod state) remain untouched.

use serde::de::{self, DeserializeOwned, DeserializeSeed, IntoDeserializer, Visitor};
use serde_json::{Map, Value};

mod encode;
mod enemies;
mod spells;
pub use encode::encode;

pub fn decode<T: DeserializeOwned>(value: Value) -> Result<T, String> {
    T::deserialize(SourceValue(value)).map_err(|error| error.to_string())
}

struct SourceValue(Value);

impl<'de> de::Deserializer<'de> for SourceValue {
    type Error = serde_json::Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        match self.0 {
            Value::Array(values) => visitor.visit_seq(SourceSequence(values.into_iter())),
            Value::Object(values) => {
                visitor.visit_map(SourceMap { values: values.into_iter(), pending: None })
            }
            Value::Number(number) if number.is_f64() => {
                let value = number.as_f64().expect("JSON numbers are finite");
                // JavaScript has one number type. A normalized native sign or safe tick
                // may be written as 1.0 here while the canonical component is an integer.
                // Preserve negative zero for floating-point geometry and headings.
                if value.fract() == 0.0
                    && value.abs() <= super::contract::MAX_SAFE_INTEGER as f64
                    && !(value == 0.0 && value.is_sign_negative())
                {
                    if value < 0.0 {
                        visitor.visit_i64(value as i64)
                    } else {
                        visitor.visit_u64(value as u64)
                    }
                } else {
                    visitor.visit_f64(value)
                }
            }
            scalar => scalar.into_deserializer().deserialize_any(visitor),
        }
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        if self.0.is_null() { visitor.visit_none() } else { visitor.visit_some(self) }
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_struct<V: Visitor<'de>>(
        self,
        name: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        let mut adapted = adapt_struct(name, self.0)?;
        if fields.contains(&"common") {
            let base = adapted.clone();
            if let Some(fields) = adapted.as_object_mut() {
                fields.insert("common".into(), base);
            }
        }
        let Value::Object(mut source) = adapted else {
            return SourceValue(adapted).deserialize_any(visitor);
        };
        let mut translated = Map::new();
        for field in fields {
            let source_name = source_field(name, field);
            if let Some(value) = source.remove(&source_name) {
                translated.insert((*field).into(), value);
            }
        }
        // Serde owns optional fields and explicit deny_unknown_fields contracts.
        translated.extend(source);
        SourceValue(Value::Object(translated))
            .deserialize_any(visitor)
            .map_err(|error| de::Error::custom(format!("{name}: {error}")))
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        name: &'static str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        let mut raw = self.0;
        if let Some(variant) = spells::route(name, &raw) {
            return visitor.visit_enum(SourceEnum { variant, payload: spells::payload(name, raw) });
        }
        if name == "EnemyBrain"
            && let Value::Object(fields) = &mut raw
            && let Some(family) = fields.remove("family")
        {
            fields.insert("kind".into(), family);
        }
        if name == "EnemyProjectile" && raw["kind"].as_str() == Some("arrow") {
            raw = serde_json::json!({"kind":"arrow", "state":raw, "velocity":raw["velocity"]});
        }
        let (tag, payload) = match raw {
            Value::Number(number)
                if matches!(name, "NativeTutorialStage" | "BoneyardGoodiePhase") =>
            {
                let index = number
                    .as_u64()
                    .and_then(|index| usize::try_from(index).ok())
                    .ok_or_else(|| de::Error::custom(format!("{name} must be an integer")))?;
                let variant = variants
                    .get(index)
                    .ok_or_else(|| de::Error::custom(format!("{name} is invalid")))?;
                ((*variant).to_owned(), Value::Null)
            }
            Value::String(tag) => (tag, Value::Null),
            Value::Object(mut fields) => {
                let tag = fields
                    .remove("kind")
                    .and_then(|value| value.as_str().map(str::to_owned))
                    .ok_or_else(|| de::Error::custom(format!("{name} requires its kind")))?;
                (tag, Value::Object(fields))
            }
            _ => return Err(de::Error::custom(format!("{name} requires an enum value"))),
        };
        let variant = variants
            .iter()
            .find(|variant| source_variant(name, variant) == tag || **variant == tag)
            .ok_or_else(|| de::Error::custom(format!("{name} has unknown value {tag}")))?;
        visitor.visit_enum(SourceEnum { variant, payload })
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf unit unit_struct seq tuple tuple_struct map identifier ignored_any
    }
}

struct SourceSequence(std::vec::IntoIter<Value>);

impl<'de> de::SeqAccess<'de> for SourceSequence {
    type Error = serde_json::Error;

    fn next_element_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, Self::Error> {
        self.0.next().map(|value| seed.deserialize(SourceValue(value))).transpose()
    }
}

struct SourceMap {
    values: serde_json::map::IntoIter,
    pending: Option<Value>,
}

impl<'de> de::MapAccess<'de> for SourceMap {
    type Error = serde_json::Error;

    fn next_key_seed<K: DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, Self::Error> {
        let Some((key, value)) = self.values.next() else {
            return Ok(None);
        };
        self.pending = Some(value);
        seed.deserialize(key.into_deserializer()).map(Some)
    }

    fn next_value_seed<V: DeserializeSeed<'de>>(
        &mut self,
        seed: V,
    ) -> Result<V::Value, Self::Error> {
        seed.deserialize(SourceValue(self.pending.take().expect("map value follows its key")))
    }
}

struct SourceEnum {
    variant: &'static str,
    payload: Value,
}

impl<'de> de::EnumAccess<'de> for SourceEnum {
    type Error = serde_json::Error;
    type Variant = Self;

    fn variant_seed<V: DeserializeSeed<'de>>(
        self,
        seed: V,
    ) -> Result<(V::Value, Self), Self::Error> {
        let variant = seed.deserialize(self.variant.into_deserializer())?;
        Ok((variant, self))
    }
}

impl<'de> de::VariantAccess<'de> for SourceEnum {
    type Error = serde_json::Error;

    fn unit_variant(self) -> Result<(), Self::Error> {
        if self.payload.is_null() || self.payload.as_object().is_some_and(Map::is_empty) {
            Ok(())
        } else {
            Err(de::Error::custom("unit enum carries unexpected fields"))
        }
    }

    fn newtype_variant_seed<T: DeserializeSeed<'de>>(
        self,
        seed: T,
    ) -> Result<T::Value, Self::Error> {
        seed.deserialize(SourceValue(self.payload))
    }

    fn tuple_variant<V: Visitor<'de>>(
        self,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        de::Deserializer::deserialize_any(SourceValue(self.payload), visitor)
    }

    fn struct_variant<V: Visitor<'de>>(
        self,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        de::Deserializer::deserialize_struct(
            SourceValue(self.payload),
            self.variant,
            fields,
            visitor,
        )
    }
}

fn source_field(name: &str, field: &str) -> String {
    match (name, field) {
        ("NewGameSelection", "name") => return "displayName".into(),
        ("SolomonEncounter", "heading_degrees") => return "headingDeg".into(),
        ("SolomonEncounter", "rng") => return "rngState".into(),
        ("NativeTutorialState", "rng") => return "rngState".into(),
        ("EnemyWorld", "head_facing_rng") => return "headFacingRngState".into(),
        ("EnemyWorld", "locomotion_rng") => return "locomotionRngState".into(),
        ("EnemyWorld", "steering_rng") => return "steeringRngState".into(),
        ("EnemyEvent", "kind") => return "type".into(),
        ("WaveState", "rng") => return "rngState".into(),
        ("PrimarySpellProjectileBase", "registration") => return "lightRegistration".into(),
        ("BoneyardBounds", "width") => return "w".into(),
        ("BoneyardBounds", "height") => return "h".into(),
        ("BoneyardObject" | "BoneyardSprite" | "BoneyardTerrain", "position") => {
            return "pos".into();
        }
        ("BoneyardObject", "rotation_degrees") => return "rot".into(),
        ("BoneyardSpawn", "facing_degrees") => return "facingDeg".into(),
        _ => {}
    }
    let mut upper = false;
    field
        .chars()
        .filter_map(|letter| {
            if letter == '_' {
                upper = true;
                None
            } else if upper {
                upper = false;
                Some(letter.to_ascii_uppercase())
            } else {
                Some(letter)
            }
        })
        .collect()
}

fn source_variant(name: &str, variant: &str) -> String {
    if let Some(kind) = spells::kind(name, variant) {
        return kind;
    }
    if matches!(name, "EnemyEffectAtlas" | "NativeEtherDrainAtlas") {
        return variant.into();
    }
    if name == "EnemySound" {
        let sound: solomon_sim::enemies::model::EnemySound =
            serde_json::from_value(Value::String(variant.into()))
                .expect("Serde supplies a declared enemy sound");
        return sound.id().into();
    }
    if name == "EnemyToken" {
        let token: solomon_sim::enemies::EnemyToken =
            serde_json::from_value(Value::String(variant.into()))
                .expect("Serde supplies a declared enemy token");
        return token.id().into();
    }
    if name == "EnemyFlag" {
        let flag: solomon_sim::enemies::EnemyFlag =
            serde_json::from_value(Value::String(variant.into()))
                .expect("Serde supplies a declared enemy flag");
        return flag.id().into();
    }
    if name == "NativeTutorialCue" {
        let cue: solomon_sim::boneyard::tutorial::NativeTutorialCue =
            serde_json::from_value(Value::String(variant.into()))
                .expect("Serde supplies a declared Tutorial cue variant");
        return cue.id().into();
    }
    match (name, variant) {
        ("SolomonDigCue", "Shovel1") => "shovel-1".into(),
        ("SolomonDigCue", "Shovel2") => "shovel-2".into(),
        ("SolomonDigCue", "ThrowDirt1") => "throw-dirt-1".into(),
        ("SolomonDigCue", "ThrowDirt2") => "throw-dirt-2".into(),
        ("SolomonVoiceCue", "Hello1") => "solomon-hello-1".into(),
        ("SolomonVoiceCue", "Hello2") => "solomon-hello-2".into(),
        ("SolomonVoiceCue", "Hello3") => "solomon-hello-3".into(),
        ("SolomonVoiceCue", "Hello4") => "solomon-hello-4".into(),
        ("SolomonVoiceCue", "Laugh1") => "solomon-laugh-1".into(),
        ("SolomonVoiceCue", "GetHimBoys") => "solomon-get-him-boys".into(),
        ("PrimarySpellTransient", "FireParticle") => "fire".into(),
        _ => kebab(variant),
    }
}

fn kebab(value: &str) -> String {
    let mut result = String::new();
    for letter in value.chars() {
        if letter.is_ascii_uppercase() && !result.is_empty() {
            result.push('-');
        }
        result.push(letter.to_ascii_lowercase());
    }
    result
}

fn adapt_struct(name: &str, mut value: Value) -> Result<Value, serde_json::Error> {
    enemies::prepare(name, &mut value).map_err(de::Error::custom)?;
    spells::prepare(name, &mut value).map_err(de::Error::custom)?;
    match name {
        "PlayerPrimaryCastState" => {
            for field in ["actionTick", "selectedPrimaryId"] {
                if value.get(field).and_then(Value::as_f64) == Some(-1.0) {
                    value[field] = Value::Null;
                }
            }
        }
        "PrimarySpellProjectileBase" => {
            let light = value.get("lightRegistration").ok_or_else(|| {
                de::Error::custom("primary projectile requires its light registration")
            })?;
            if value.get("painterRegistrations") != Some(&serde_json::json!([light])) {
                return Err(de::Error::custom(
                    "primary projectile painter and light registration drifted",
                ));
            }
        }
        "NativeRng" | "NativeRngDocument" => {
            value
                .as_object_mut()
                .ok_or_else(|| de::Error::custom("RNG requires an object"))?
                .insert("divisor".into(), Value::from(100_000));
        }
        "HubShopItem" => {
            let price = value
                .as_object_mut()
                .and_then(|object| object.remove("price"))
                .ok_or_else(|| de::Error::custom("shop item requires its price"))?;
            value = serde_json::json!({"item": value, "price": price});
        }
        "PlayerBeltComponent" => value = serde_json::json!({"entries": value}),
        "WorldPainterOrder" => {
            let lanes = value
                .get("nextRegistrationOrdinal")
                .ok_or_else(|| de::Error::custom("world painter order requires both lanes"))?;
            value = serde_json::json!({"nextActorOrdinal": lanes["actor"], "nextTransientOrdinal": lanes["transient"]});
        }
        "EnemyWorld" => {
            let fields = value
                .as_object_mut()
                .ok_or_else(|| de::Error::custom("enemy world requires an object"))?;
            // These are per-step output buffers. The Website stores their accepted events
            // on the Boneyard world and rebuilds these buffers on the next tick.
            for field in [
                "spawnedActorIds",
                "events",
                "playerDamage",
                "playerKnockbacks",
                "pendingSpawnIntents",
                "retired",
                "rewards",
            ] {
                fields.insert(field.into(), Value::Array(Vec::new()));
            }
            fields.insert("playerPositions".into(), serde_json::json!({}));
        }
        "WaveState" => {
            let fields = value
                .as_object_mut()
                .ok_or_else(|| de::Error::custom("wave state requires an object"))?;
            let active = fields
                .get("activeBursts")
                .ok_or_else(|| de::Error::custom("wave state requires active bursts"))?;
            let section = if active.as_array().is_some_and(Vec::is_empty) {
                Value::Null
            } else if fields.get("openingBursts") == Some(active) {
                Value::String("opening".into())
            } else {
                let index = fields
                    .get("compiledSchedule")
                    .and_then(Value::as_array)
                    .and_then(|sections| {
                        sections.iter().position(|section| section.get("bursts") == Some(active))
                    })
                    .ok_or_else(|| {
                        de::Error::custom("wave state active bursts have no compiled section")
                    })?;
                serde_json::json!({"kind":"schedule", "index":index})
            };
            fields.insert("activeSection".into(), section);
        }
        _ => {}
    }
    Ok(value)
}
