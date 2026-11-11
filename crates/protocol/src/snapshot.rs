//! Lossless updates between the complete snapshots actually written to one socket.
//!
//! The replaceable outbox selects a document before encoding. TCP/WebSocket ordering
//! then permits one baseline at each endpoint, independent of render-time ACKs. Game
//! rules, checkpoint fields, floating-point values and flame cursors remain unchanged.

use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;
use std::io::{self, Write};
use std::sync::{Arc, Mutex, OnceLock};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::de::{DeserializeOwned, Error as _, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::{GAME_WEBSOCKET_MAX_PAYLOAD_BYTES, ServerMessage, SnapshotFrame};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SnapshotEncodingError {
    Serialization(String),
    PayloadTooLarge,
}

impl fmt::Display for SnapshotEncodingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Serialization(reason) => formatter.write_str(reason),
            Self::PayloadTooLarge => {
                formatter.write_str("The game snapshot exceeds the transport payload bound")
            }
        }
    }
}

impl std::error::Error for SnapshotEncodingError {}

type SnapshotEncoding = Box<dyn FnOnce() -> Result<Arc<str>, SnapshotEncodingError> + Send>;

/// One frozen publication, shared by peers with the same world/flame projection.
/// Replaced unsent publications drop their state without formatting JSON.
pub struct PreparedSnapshot {
    sequence: u64,
    encode: Mutex<Option<SnapshotEncoding>>,
    frame: OnceLock<Result<Value, String>>,
    full_message: OnceLock<Result<Arc<str>, SnapshotEncodingError>>,
}

impl PreparedSnapshot {
    #[must_use]
    pub fn new<S: Serialize + Send + 'static>(frame: SnapshotFrame<S>) -> Arc<Self> {
        let sequence = frame.sequence;
        let encode = Box::new(move || {
            #[derive(Serialize)]
            struct Message<'a, S> {
                r#type: &'static str,
                frame: &'a SnapshotFrame<S>,
            }
            let message = serde_json::to_string(&Message { r#type: "snapshot", frame: &frame })
                .map_err(|error| SnapshotEncodingError::Serialization(error.to_string()))?;
            if message.len() > GAME_WEBSOCKET_MAX_PAYLOAD_BYTES {
                return Err(SnapshotEncodingError::PayloadTooLarge);
            }
            Ok(Arc::from(message))
        });
        Arc::new(Self {
            sequence,
            encode: Mutex::new(Some(encode)),
            frame: OnceLock::new(),
            full_message: OnceLock::new(),
        })
    }

    /// The socket writer formats once; peers share both the result and failures.
    /// The captured owned state is released as soon as encoding finishes.
    pub fn full_message(&self) -> Result<&Arc<str>, SnapshotEncodingError> {
        self.full_message
            .get_or_init(|| {
                let encode = self
                    .encode
                    .lock()
                    .expect("snapshot ownership does not panic")
                    .take()
                    .expect("the full message initializes once");
                encode()
            })
            .as_ref()
            .map_err(Clone::clone)
    }

    fn frame(&self) -> Result<&Value, String> {
        // The socket writer needs a comparison tree only for publications it sends.
        // Share that tree across peers without allocating it on the authority loop.
        self.frame
            .get_or_init(|| {
                #[derive(Deserialize)]
                struct Message {
                    frame: Value,
                }
                serde_json::from_str::<Message>(
                    self.full_message().map_err(|error| error.to_string())?,
                )
                .map(|message| message.frame)
                .map_err(|error| error.to_string())
            })
            .as_ref()
            .map_err(Clone::clone)
    }
}

/// Owned by the socket writer, never advanced for a replaced unsent publication.
#[derive(Default)]
pub struct SnapshotWriter {
    previous: Option<Arc<PreparedSnapshot>>,
}

impl SnapshotWriter {
    pub fn encode(&mut self, next: Arc<PreparedSnapshot>) -> Result<Arc<str>, String> {
        let mut message = next.full_message().map_err(|error| error.to_string())?.clone();
        if let Some(previous) = &self.previous {
            if next.sequence <= previous.sequence {
                return Err("Snapshot publications must advance their sequence".into());
            }
            let change = Change::between(previous.frame()?, next.frame()?)
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
    size: SizeCache,
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
                let size = SizeCache::new(&frame)?;
                self.previous = Some(ReceivedSnapshot { sequence: decoded.sequence, frame, size });
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
                // Validate the complete UTF-8 document once. The string reader can
                // then borrow its many short field names without revalidating each.
                let text = std::str::from_utf8(&bytes).map_err(|error| error.to_string())?;
                let change: Change =
                    serde_json::from_str(text).map_err(|error| error.to_string())?;
                // Count changed fragments exactly, retaining the unchanged document's
                // byte count instead of serializing all scenery and actors again.
                change.apply(&mut previous.frame, &mut previous.size)?;
                if previous.size.bytes > GAME_WEBSOCKET_MAX_PAYLOAD_BYTES {
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

// No prior index means a new replacement; no change means an unchanged prior item.
type RetainedItem<'a> = (Option<usize>, Option<Change<'a>>);

/// Single-element arrays replace a value; two-element arrays edit indexed items;
/// `["r", items]` retains/reorders prior items with optional edits. Objects edit
/// fields; null removes an object field. Null values use `[null]`.
#[derive(Serialize)]
#[serde(untagged)]
enum Change<'a> {
    Replace([Value; 1]),
    Array((usize, Vec<(usize, Change<'a>)>)),
    Retained((&'static str, Vec<RetainedItem<'a>>)),
    Object(BTreeMap<Cow<'a, str>, Change<'a>>),
    Remove,
}

impl<'de> Deserialize<'de> for Change<'de> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // Recursive untagged derives buffer and clone each subtree at every level.
        // The JSON shape already identifies these cases, so visit it once.
        struct ChangeVisitor;
        impl<'de> Visitor<'de> for ChangeVisitor {
            type Value = Change<'de>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a snapshot replacement, indexed or retained array edit, field edit, or removal")
            }

            fn visit_unit<E: serde::de::Error>(self) -> Result<Change<'de>, E> {
                Ok(Change::Remove)
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Change<'de>, A::Error> {
                let mut changes = BTreeMap::new();
                #[derive(Deserialize)]
                struct FieldName<'a>(#[serde(borrow)] Cow<'a, str>);

                while let Some(FieldName(key)) = map.next_key()? {
                    changes.insert(key, map.next_value()?);
                }
                Ok(Change::Object(changes))
            }

            fn visit_seq<A: SeqAccess<'de>>(
                self,
                mut sequence: A,
            ) -> Result<Change<'de>, A::Error> {
                let first = sequence
                    .next_element::<Value>()?
                    .ok_or_else(|| A::Error::invalid_length(0, &self))?;
                if first.as_str() == Some("r") {
                    let Some(entries) = sequence.next_element::<Vec<RetainedItem>>()? else {
                        return Ok(Change::Replace([first]));
                    };
                    if sequence.next_element::<IgnoredAny>()?.is_some() {
                        return Err(A::Error::invalid_length(3, &self));
                    }
                    return Ok(Change::Retained(("r", entries)));
                }
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

impl Change<'_> {
    fn between(previous: &Value, next: &Value) -> Option<Self> {
        Some(match (previous, next) {
            (Value::Object(previous), Value::Object(next)) => {
                let mut changes = BTreeMap::new();
                if previous.len() == next.len() && previous.keys().eq(next.keys()) {
                    // Session records keep their field set. Walk matching fields together
                    // instead of searching the same object for every value on every frame.
                    for ((_, previous), (key, value)) in previous.iter().zip(next) {
                        if let Some(change) = Self::between(previous, value) {
                            changes.insert(Cow::Owned(key.clone()), change);
                        }
                    }
                } else {
                    for key in previous.keys().filter(|key| !next.contains_key(*key)) {
                        changes.insert(Cow::Owned(key.clone()), Self::Remove);
                    }
                    for (key, value) in next {
                        let change = previous.get(key).map_or_else(
                            || Some(Self::Replace([value.clone()])),
                            |previous| Self::between(previous, value),
                        );
                        if let Some(change) = change {
                            changes.insert(Cow::Owned(key.clone()), change);
                        }
                    }
                }
                if changes.is_empty() {
                    return None;
                }
                Self::Object(changes)
            }
            (Value::Array(previous), Value::Array(next)) => {
                if let Some(retained) = Self::retained(previous, next) {
                    return Some(retained);
                }
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

    fn retained(previous: &[Value], next: &[Value]) -> Option<Self> {
        // Small or positionally stable arrays do not need an identity index.
        if previous.len().min(next.len()) < 32
            || previous.iter().zip(next).all(|(old, new)| old.get("id") == new.get("id"))
        {
            return None;
        }
        // IDs select an encoding only. The resulting values and their order still
        // come entirely from the next canonical array, including births and edits.
        let mut indices = HashMap::with_capacity(previous.len());
        for (index, value) in previous.iter().enumerate() {
            if indices.insert(value.get("id")?.as_u64()?, index).is_some() {
                return None;
            }
        }
        let mut ids = HashSet::with_capacity(next.len());
        let mut positions = Vec::with_capacity(next.len());
        let mut shifted = false;
        for (index, value) in next.iter().enumerate() {
            let id = value.get("id")?.as_u64()?;
            if !ids.insert(id) {
                return None;
            }
            let old = indices.get(&id).copied();
            shifted |= old.is_some_and(|old| old != index);
            positions.push(old);
        }
        if !shifted {
            return None;
        }
        let entries = positions
            .into_iter()
            .zip(next)
            .map(|(old, value)| {
                let change = old.map_or_else(
                    || Some(Self::Replace([value.clone()])),
                    |old| Self::between(&previous[old], value),
                );
                (old, change)
            })
            .collect();
        Some(Self::Retained(("r", entries)))
    }

    fn apply_child(
        self,
        value: &mut Value,
        size: &mut SizeCache,
        parent_bytes: &mut usize,
    ) -> Result<(), String> {
        let old_bytes = size.bytes;
        self.apply(value, size)?;
        change_size(parent_bytes, old_bytes, size.bytes)
    }

    fn apply(self, value: &mut Value, size: &mut SizeCache) -> Result<(), String> {
        let SizeCache { bytes, children } = size;
        match self {
            Self::Replace([next]) => {
                *size = SizeCache::new(&next)?;
                *value = next;
            }
            Self::Object(changes) => {
                let object =
                    value.as_object_mut().ok_or("The snapshot field update has no object")?;
                let fields = children.fields();
                for (key, change) in changes {
                    let length = object.len();
                    if matches!(change, Self::Remove) {
                        let removed = object
                            .remove(key.as_ref())
                            .ok_or("The snapshot removes an absent field")?;
                        let removed_bytes = match fields.remove(key.as_ref()) {
                            Some(size) => size.bytes,
                            None => serialized_size(&removed)?,
                        };
                        change_size(
                            bytes,
                            serialized_size(&key)? + 1 + removed_bytes + usize::from(length > 1),
                            0,
                        )?;
                    } else if let Some(value) = object.get_mut(key.as_ref()) {
                        if let Some(child) = fields.get_mut(key.as_ref()) {
                            change.apply_child(value, child, bytes)?;
                        } else {
                            let mut child = SizeCache::new(value)?;
                            change.apply_child(value, &mut child, bytes)?;
                            fields.insert(key.into_owned(), child);
                        }
                    } else if let Self::Replace([next]) = change {
                        let child = SizeCache::new(&next)?;
                        change_size(
                            bytes,
                            0,
                            serialized_size(&key)? + 1 + child.bytes + usize::from(length > 0),
                        )?;
                        fields.insert(key.clone().into_owned(), child);
                        object.insert(key.into_owned(), next);
                    } else {
                        return Err("The snapshot edits an absent field".into());
                    }
                }
            }
            Self::Retained((_, entries)) => {
                let array =
                    value.as_array_mut().ok_or("The snapshot retained item update has no array")?;
                // Each prior value may move once; repeated references would allow
                // a tiny packet to amplify the complete baseline without a bound.
                let mut selected = vec![false; array.len()];
                for (old, change) in &entries {
                    if let Some(index) = old {
                        let used = selected
                            .get_mut(*index)
                            .ok_or("The snapshot retained index is outside its array bounds")?;
                        if *used {
                            return Err("The snapshot repeats a retained index".into());
                        }
                        *used = true;
                    } else if !matches!(change, Some(Self::Replace(_))) {
                        return Err("The snapshot new retained item has no replacement".into());
                    }
                }
                let items = children.items(array)?;
                let previous_len = array.len();
                let mut previous = std::mem::take(array);
                let mut previous_sizes = std::mem::take(items);
                for (size, selected) in previous_sizes.iter().zip(&selected) {
                    if !selected {
                        change_size(bytes, size.bytes, 0)?;
                    }
                }
                change_size(
                    bytes,
                    previous_len.saturating_sub(1),
                    entries.len().saturating_sub(1),
                )?;
                array.reserve(entries.len());
                items.reserve(entries.len());
                for (old, change) in entries {
                    if let Some(index) = old {
                        let mut item = previous[index].take();
                        let mut size = std::mem::take(&mut previous_sizes[index]);
                        if let Some(change) = change {
                            change.apply_child(&mut item, &mut size, bytes)?;
                        }
                        array.push(item);
                        items.push(size);
                    } else if let Some(Self::Replace([next])) = change {
                        let size = SizeCache::new(&next)?;
                        change_size(bytes, 0, size.bytes)?;
                        array.push(next);
                        items.push(size);
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
                let items = children.items(array)?;
                if length < array.len() {
                    let removed = array.len().saturating_sub(1) - length.saturating_sub(1)
                        + items[length..].iter().map(|size| size.bytes).sum::<usize>();
                    change_size(bytes, removed, 0)?;
                    array.truncate(length);
                    items.truncate(length);
                }
                for (index, change) in changes {
                    if index < array.len() {
                        change.apply_child(&mut array[index], &mut items[index], bytes)?;
                    } else if index == array.len()
                        && let Self::Replace([next]) = change
                    {
                        let size = SizeCache::new(&next)?;
                        change_size(bytes, 0, size.bytes + usize::from(!array.is_empty()))?;
                        array.push(next);
                        items.push(size);
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

/// Byte counts follow their values through every edit and retained-array move.
/// Children are measured only when an edit first descends into their container;
/// unchanged fields need neither a parallel tree nor another serialization.
#[derive(Default)]
struct SizeCache {
    bytes: usize,
    children: ChildSizes,
}

impl SizeCache {
    fn new(value: &Value) -> Result<Self, String> {
        Ok(Self { bytes: serialized_size(value)?, children: ChildSizes::Unexpanded })
    }
}

#[derive(Default)]
enum ChildSizes {
    #[default]
    Unexpanded,
    Object(BTreeMap<String, SizeCache>),
    Array(Vec<SizeCache>),
}

impl ChildSizes {
    fn fields(&mut self) -> &mut BTreeMap<String, SizeCache> {
        if matches!(self, Self::Unexpanded) {
            *self = Self::Object(BTreeMap::new());
        }
        let Self::Object(fields) = self else {
            unreachable!("a replacement retires the old value's byte counts")
        };
        fields
    }

    fn items(&mut self, values: &[Value]) -> Result<&mut Vec<SizeCache>, String> {
        if matches!(self, Self::Unexpanded) {
            let mut items = Vec::with_capacity(values.len());
            for value in values {
                items.push(SizeCache::new(value)?);
            }
            *self = Self::Array(items);
        }
        let Self::Array(items) = self else {
            unreachable!("a replacement retires the old value's byte counts")
        };
        Ok(items)
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
        let mut size = SizeCache::new(&value).unwrap();
        for next in &values[1..] {
            Change::between(&value, next).unwrap().apply(&mut value, &mut size).unwrap();
            let encoded = serde_json::to_vec(&value).unwrap();
            assert_eq!(encoded, serde_json::to_vec(next).unwrap());
            assert_eq!(
                size.bytes,
                encoded.len(),
                "the complete document bound includes JSON punctuation and escaped UTF-8"
            );
        }
    }

    #[test]
    fn retained_reordering_and_identity_fallback_keep_exact_byte_counts() {
        let records: Vec<_> = (0..64)
            .map(|id| {
                json!({
                    "id":u64::MAX-id,"x":-0.0,"name":"\n🧙","nested":[null,{"value":id}]
                })
            })
            .collect();
        let mut value = json!(records);
        let mut size = SizeCache::new(&value).unwrap();
        let mut next = records[1..].to_vec();
        next.reverse();
        next[0]["x"] = json!(0.0);
        next[1]["new"] = json!("\"\\");
        next.push(json!({"id":7,"new":true}));
        let mut duplicate = next.clone();
        duplicate[0]["id"] = duplicate[1]["id"].clone();
        let mut unidentified = next.clone();
        unidentified[0].as_object_mut().unwrap().remove("id");
        for next in [json!(next), json!(duplicate), json!(unidentified), json!([])] {
            let change = Change::between(&value, &next).unwrap();
            let encoded_change = serde_json::to_string(&change).unwrap();
            let change: Change = serde_json::from_str(&encoded_change).unwrap();
            change.apply(&mut value, &mut size).unwrap();
            assert_eq!(serde_json::to_vec(&value).unwrap(), serde_json::to_vec(&next).unwrap());
            assert_eq!(size.bytes, serialized_size(&next).unwrap());
        }
    }

    #[test]
    fn nested_byte_counts_follow_retained_moves_and_container_replacements() {
        let records: Vec<_> = (0..40)
            .map(|id| json!({"id":id,"nested":[{"text":"🧙\n","x":-0.0},[id]],"keep":true}))
            .collect();
        let mut expected = json!({"records":records});
        let mut actual = expected.clone();
        let mut size = SizeCache::new(&actual).unwrap();
        for stage in 0..6 {
            match stage {
                0 => {
                    for record in expected["records"].as_array_mut().unwrap() {
                        record["nested"][0]["x"] = json!(0.0);
                        record["nested"][1] = json!([u64::MAX, i64::MIN]);
                    }
                }
                1 => {
                    let records = expected["records"].as_array_mut().unwrap();
                    records.remove(0);
                    records.reverse();
                    records[0]["nested"][0]["text"] = json!("\"\\\u{0}");
                    records.push(json!({"id":400,"nested":null}));
                }
                2 => {
                    expected["records"][0]["nested"] = json!({"new":[null,false,1e300]});
                    expected["records"][1].as_object_mut().unwrap().remove("nested");
                }
                3 => {
                    expected["records"][0]["nested"]["new"] = json!([]);
                    expected["records"][1]["nested"] = json!([{"reset":1e-300}]);
                    expected["records"].as_array_mut().unwrap().rotate_left(3);
                }
                4 => expected["records"] = json!([]),
                _ => expected = json!({}),
            }
            let change = Change::between(&actual, &expected).unwrap();
            let packet = serde_json::to_string(&change).unwrap();
            let received: Change = serde_json::from_str(&packet).unwrap();
            received.apply(&mut actual, &mut size).unwrap();
            let canonical = serde_json::to_vec(&expected).unwrap();
            assert_eq!(serde_json::to_vec(&actual).unwrap(), canonical, "stage {stage}");
            assert_eq!(size.bytes, canonical.len(), "stage {stage}");
        }
    }

    #[test]
    fn reconstructed_size_limit_stops_at_the_existing_message_bound() {
        let mut writer = SizeLimit(GAME_WEBSOCKET_MAX_PAYLOAD_BYTES - 2);
        assert_eq!(writer.write(&[0; 2]).unwrap(), 2);
        assert!(writer.write(&[0]).is_err());
    }
}
