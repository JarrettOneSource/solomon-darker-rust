//! Lossless updates between the complete snapshots actually written to one socket.
//!
//! The replaceable outbox selects a document before encoding. TCP/WebSocket ordering
//! then permits one baseline at each endpoint, independent of render-time ACKs. Game
//! rules, checkpoint fields, floating-point values and flame cursors remain unchanged.

use std::collections::BTreeMap;
use std::fmt;
use std::io::{self, Write};
use std::sync::Arc;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::de::{DeserializeOwned, Error as _, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::{GAME_WEBSOCKET_MAX_PAYLOAD_BYTES, ServerMessage, SnapshotFrame};

/// One completed publication, shared by peers with the same world/flame projection.
pub struct PreparedSnapshot {
    sequence: u64,
    frame: Value,
    full_message: Arc<str>,
}

impl PreparedSnapshot {
    pub fn new<S: Serialize>(frame: &SnapshotFrame<S>) -> Result<Arc<Self>, String> {
        let value = serde_json::to_value(frame).map_err(|error| error.to_string())?;
        #[derive(Serialize)]
        struct Message<'a> {
            r#type: &'static str,
            frame: &'a Value,
        }
        let full_message = serde_json::to_string(&Message { r#type: "snapshot", frame: &value })
            .map_err(|error| error.to_string())?;
        if full_message.len() > GAME_WEBSOCKET_MAX_PAYLOAD_BYTES {
            return Err("The game snapshot exceeds the transport payload bound".into());
        }
        Ok(Arc::new(Self {
            sequence: frame.sequence,
            frame: value,
            full_message: full_message.into(),
        }))
    }

    #[must_use]
    pub fn full_message(&self) -> &Arc<str> {
        &self.full_message
    }
}

/// Owned by the socket writer, never advanced for a replaced unsent publication.
#[derive(Default)]
pub struct SnapshotWriter {
    previous: Option<Arc<PreparedSnapshot>>,
}

impl SnapshotWriter {
    pub fn encode(&mut self, next: Arc<PreparedSnapshot>) -> Result<Arc<str>, String> {
        let mut message = next.full_message.clone();
        if let Some(previous) = &self.previous {
            if next.sequence <= previous.sequence {
                return Err("Snapshot publications must advance their sequence".into());
            }
            let change = Change::between(&previous.frame, &next.frame)
                .expect("an advancing snapshot changes its sequence");
            let changes = serde_json::to_vec(&change).map_err(|error| error.to_string())?;
            let compressed = miniz_oxide::deflate::compress_to_vec_zlib(&changes, 1);
            let delta = serde_json::to_string(&DeltaMessage::SnapshotDelta {
                sequence: next.sequence,
                base_sequence: previous.sequence,
                changes: STANDARD.encode(compressed),
            })
            .map_err(|error| error.to_string())?;
            if delta.len() < message.len() {
                message = delta.into();
            }
        }
        self.previous = Some(next);
        Ok(message)
    }
}

/// Every received packet advances this cache before rendering can coalesce frames.
#[derive(Default)]
pub struct SnapshotReader {
    previous: Option<ReceivedSnapshot>,
}

struct ReceivedSnapshot {
    sequence: u64,
    frame: Value,
    encoded_bytes: usize,
}

impl SnapshotReader {
    pub fn clear(&mut self) {
        self.previous = None;
    }

    /// Controls and Welcome retain their canonical messages. A malformed packet
    /// invalidates this connection's baseline; the caller closes the failed socket.
    pub fn decode<S: DeserializeOwned>(&mut self, text: &str) -> Result<ServerMessage<S>, String> {
        let result = self.decode_message(text);
        if result.is_err() {
            self.clear();
        }
        result
    }

    fn decode_message<S: DeserializeOwned>(
        &mut self,
        text: &str,
    ) -> Result<ServerMessage<S>, String> {
        if text.len() > GAME_WEBSOCKET_MAX_PAYLOAD_BYTES {
            return Err("The game snapshot exceeds the transport payload bound".into());
        }
        let mut message: Value = serde_json::from_str(text).map_err(|error| error.to_string())?;
        match message.get("type").and_then(Value::as_str) {
            Some("snapshot") => {
                if message.as_object().is_none_or(|object| object.len() != 2) {
                    return Err("The snapshot envelope contains unexpected fields".into());
                }
                let frame = message.get_mut("frame").ok_or("The snapshot has no frame")?.take();
                let decoded =
                    SnapshotFrame::<S>::deserialize(&frame).map_err(|error| error.to_string())?;
                if self
                    .previous
                    .as_ref()
                    .is_some_and(|previous| decoded.sequence <= previous.sequence)
                {
                    return Err("The received snapshot sequence did not advance".into());
                }
                let encoded_bytes = serialized_size(&frame)?;
                self.previous =
                    Some(ReceivedSnapshot { sequence: decoded.sequence, frame, encoded_bytes });
                Ok(ServerMessage::Snapshot { frame: decoded })
            }
            Some("snapshot_delta") => {
                let DeltaMessage::SnapshotDelta { sequence, base_sequence, changes } =
                    serde_json::from_value(message).map_err(|error| error.to_string())?;
                let previous =
                    self.previous.as_mut().ok_or("The snapshot update has no complete baseline")?;
                if base_sequence != previous.sequence || sequence <= base_sequence {
                    return Err("The snapshot update does not follow its received baseline".into());
                }
                let compressed = STANDARD.decode(changes).map_err(|error| error.to_string())?;
                let bytes = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(
                    &compressed,
                    GAME_WEBSOCKET_MAX_PAYLOAD_BYTES,
                )
                .map_err(|_| "The snapshot update is invalid or exceeds its byte limit")?;
                let change: Change =
                    serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
                // Count changed fragments exactly, retaining the unchanged document's
                // byte count instead of serializing all scenery and actors again.
                change.apply(&mut previous.frame, &mut previous.encoded_bytes)?;
                if previous.encoded_bytes > GAME_WEBSOCKET_MAX_PAYLOAD_BYTES {
                    return Err("The reconstructed snapshot exceeds its byte limit".into());
                }
                let decoded = SnapshotFrame::<S>::deserialize(&previous.frame)
                    .map_err(|error| error.to_string())?;
                if decoded.sequence != sequence {
                    return Err("The snapshot update changed its declared sequence".into());
                }
                previous.sequence = sequence;
                Ok(ServerMessage::Snapshot { frame: decoded })
            }
            _ => serde_json::from_value(message).map_err(|error| error.to_string()),
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum DeltaMessage {
    SnapshotDelta { sequence: u64, base_sequence: u64, changes: String },
}

/// Single-element arrays replace a value; two-element arrays edit indexed items;
/// objects edit fields; null removes an object field. Null values use `[null]`.
#[derive(Serialize)]
#[serde(untagged)]
enum Change {
    Replace([Value; 1]),
    Array((usize, Vec<(usize, Change)>)),
    Object(BTreeMap<String, Change>),
    Remove,
}

impl<'de> Deserialize<'de> for Change {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // Recursive untagged derives buffer and clone each subtree at every level.
        // The JSON shape already identifies these four cases, so visit it once.
        struct ChangeVisitor;
        impl<'de> Visitor<'de> for ChangeVisitor {
            type Value = Change;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a snapshot replacement, array edit, field edit, or removal")
            }

            fn visit_unit<E: serde::de::Error>(self) -> Result<Change, E> {
                Ok(Change::Remove)
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Change, A::Error> {
                let mut changes = BTreeMap::new();
                while let Some((key, change)) = map.next_entry()? {
                    changes.insert(key, change);
                }
                Ok(Change::Object(changes))
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Change, A::Error> {
                let first = sequence
                    .next_element::<Value>()?
                    .ok_or_else(|| A::Error::invalid_length(0, &self))?;
                let Some(changes) = sequence.next_element::<Vec<(usize, Change)>>()? else {
                    return Ok(Change::Replace([first]));
                };
                let length = first
                    .as_u64()
                    .and_then(|length| usize::try_from(length).ok())
                    .ok_or_else(|| A::Error::custom("The snapshot array length is invalid"))?;
                if sequence.next_element::<IgnoredAny>()?.is_some() {
                    return Err(A::Error::invalid_length(3, &self));
                }
                Ok(Change::Array((length, changes)))
            }
        }
        deserializer.deserialize_any(ChangeVisitor)
    }
}

impl Change {
    fn between(previous: &Value, next: &Value) -> Option<Self> {
        Some(match (previous, next) {
            (Value::Object(previous), Value::Object(next)) => {
                let mut changes: BTreeMap<_, _> = previous
                    .keys()
                    .filter(|key| !next.contains_key(*key))
                    .map(|key| (key.clone(), Self::Remove))
                    .collect();
                for (key, value) in next {
                    let change = previous.get(key).map_or_else(
                        || Some(Self::Replace([value.clone()])),
                        |previous| Self::between(previous, value),
                    );
                    if let Some(change) = change {
                        changes.insert(key.clone(), change);
                    }
                }
                if changes.is_empty() {
                    return None;
                }
                Self::Object(changes)
            }
            (Value::Array(previous), Value::Array(next)) => {
                let changes: Vec<_> = next
                    .iter()
                    .enumerate()
                    .filter_map(|(index, value)| {
                        previous
                            .get(index)
                            .map_or_else(
                                || Some(Self::Replace([value.clone()])),
                                |previous| Self::between(previous, value),
                            )
                            .map(|change| (index, change))
                    })
                    .collect();
                if changes.is_empty() && previous.len() == next.len() {
                    return None;
                }
                Self::Array((next.len(), changes))
            }
            (Value::Number(previous), Value::Number(next)) => {
                let equal = if previous.is_f64() && next.is_f64() {
                    previous.as_f64().unwrap().to_bits() == next.as_f64().unwrap().to_bits()
                } else {
                    previous == next
                };
                if equal {
                    return None;
                }
                Self::Replace([Value::Number(next.clone())])
            }
            _ if previous == next => return None,
            _ => Self::Replace([next.clone()]),
        })
    }

    fn apply(self, value: &mut Value, encoded_bytes: &mut usize) -> Result<(), String> {
        match self {
            Self::Replace([next]) => {
                change_size(encoded_bytes, serialized_size(value)?, serialized_size(&next)?)?;
                *value = next;
            }
            Self::Object(changes) => {
                let object =
                    value.as_object_mut().ok_or("The snapshot field update has no object")?;
                for (key, change) in changes {
                    match change {
                        Self::Remove => {
                            let removed =
                                object.get(&key).ok_or("The snapshot removes an absent field")?;
                            let bytes = serialized_size(&key)?
                                + 1
                                + serialized_size(removed)?
                                + usize::from(object.len() > 1);
                            change_size(encoded_bytes, bytes, 0)?;
                            object.remove(&key);
                        }
                        Self::Replace([next]) if !object.contains_key(&key) => {
                            let bytes = serialized_size(&key)?
                                + 1
                                + serialized_size(&next)?
                                + usize::from(!object.is_empty());
                            change_size(encoded_bytes, 0, bytes)?;
                            object.insert(key, next);
                        }
                        change => change.apply(
                            object.get_mut(&key).ok_or("The snapshot edits an absent field")?,
                            encoded_bytes,
                        )?,
                    }
                }
            }
            Self::Array((length, changes)) => {
                let array = value.as_array_mut().ok_or("The snapshot item update has no array")?;
                // Each appended item must have its own replacement, so an attacker
                // cannot allocate an enormous array by sending only a new length.
                if length.saturating_sub(array.len()) > changes.len()
                    || changes.iter().any(|(index, _)| *index >= length)
                    || changes.windows(2).any(|pair| pair[0].0 >= pair[1].0)
                {
                    return Err("The snapshot item update is outside its array bounds".into());
                }
                if length < array.len() {
                    let mut removed = array.len().saturating_sub(1) - length.saturating_sub(1);
                    for value in &array[length..] {
                        removed += serialized_size(value)?;
                    }
                    change_size(encoded_bytes, removed, 0)?;
                    array.truncate(length);
                }
                for (index, change) in changes {
                    if index < array.len() {
                        change.apply(&mut array[index], encoded_bytes)?;
                    } else if index == array.len()
                        && let Self::Replace([next]) = change
                    {
                        let added = serialized_size(&next)? + usize::from(!array.is_empty());
                        change_size(encoded_bytes, 0, added)?;
                        array.push(next);
                    } else {
                        return Err("The snapshot item update leaves missing array entries".into());
                    }
                }
                if array.len() != length {
                    return Err("The snapshot item update has an incomplete array".into());
                }
            }
            Self::Remove => return Err("The snapshot removes a value outside an object".into()),
        }
        Ok(())
    }
}

fn change_size(size: &mut usize, removed: usize, added: usize) -> Result<(), String> {
    *size = size
        .checked_sub(removed)
        .and_then(|size| size.checked_add(added))
        .ok_or("The snapshot update has an invalid byte count")?;
    Ok(())
}

fn serialized_size(value: &impl Serialize) -> Result<usize, String> {
    let mut size = SizeLimit(0);
    serde_json::to_writer(&mut size, value).map_err(|error| error.to_string())?;
    Ok(size.0)
}

struct SizeLimit(usize);

impl Write for SizeLimit {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0 = self
            .0
            .checked_add(bytes.len())
            .filter(|size| *size <= GAME_WEBSOCKET_MAX_PAYLOAD_BYTES)
            .ok_or_else(|| io::Error::other("The reconstructed snapshot exceeds its byte limit"))?;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn incremental_byte_counts_match_complete_json_after_nested_insertions_and_removals() {
        let values = [
            json!({}),
            json!({"keep":"unchanged scenery ".repeat(100), "\n🧙":[null, -0.0, {"a":"\\\""}]}),
            json!({"keep":"unchanged scenery ".repeat(100), "\n🧙":[true, 0.0, {"b":u64::MAX}, []], "new":null}),
            json!({"keep":"unchanged scenery ".repeat(100), "\n🧙":[true]}),
            json!({"\n🧙":[]}),
            json!({}),
            json!([[], {}, null, "\u{0}é"]),
            json!([[], {"nested":[[1,2,3], {"x":true}]}]),
            json!([[], {"nested":[[], {}]}, false]),
            json!([]),
            json!(null),
        ];
        let mut value = values[0].clone();
        let mut bytes = serde_json::to_vec(&value).unwrap().len();
        for next in &values[1..] {
            Change::between(&value, next).unwrap().apply(&mut value, &mut bytes).unwrap();
            let encoded = serde_json::to_vec(&value).unwrap();
            assert_eq!(encoded, serde_json::to_vec(next).unwrap());
            assert_eq!(
                bytes,
                encoded.len(),
                "the complete document bound includes JSON punctuation and escaped UTF-8"
            );
        }
    }

    #[test]
    fn reconstructed_size_limit_stops_at_the_existing_message_bound() {
        let mut writer = SizeLimit(GAME_WEBSOCKET_MAX_PAYLOAD_BYTES - 2);
        assert_eq!(writer.write(&[0; 2]).unwrap(), 2);
        assert!(writer.write(&[0]).is_err());
    }
}
