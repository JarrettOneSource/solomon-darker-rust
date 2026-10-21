//! Stock SyncBuffer tree and custom XOR/LZ codec (`save/native-save-codec.ts`).

use std::collections::BTreeSet;

const SYNCBUFFER_MAX_BYTES: usize = 64 * 1024 * 1024;
const SYNCBUFFER_MAX_DEPTH: usize = 128;
const SYNCBUFFER_MAX_NODES: usize = 1_000_000;
const DARKDATA_MAX_DECOMPRESSED_BYTES: usize = 32 * 1024 * 1024;

pub const NATIVE_DARKDATA_KEY: &[u8] = b"MagicEncryptionWord=\"SolomonDarkEncryption\"|there$w#st w&187sfj21\t89n4v 1984x98mn12xc39931c87241@@@@@@";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeChunkNode {
    pub children: Vec<NativeChunkNode>,
    pub offset: usize,
    pub payload: Vec<u8>,
    pub payload_offset: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeNamedBuffer {
    pub buffer: NativeSyncBuffer,
    pub name: String,
    pub name_offset: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeSyncBuffer {
    pub end_offset: usize,
    pub named_buffers: Vec<NativeNamedBuffer>,
    pub offset: usize,
    pub root: NativeChunkNode,
}

struct NativeCursor<'a> {
    bytes: &'a [u8],
    node_count: usize,
    offset: usize,
}

impl NativeCursor<'_> {
    fn u32(&mut self, claim: &str) -> Result<usize, String> {
        if self.offset + 4 > self.bytes.len() {
            return Err(format!("truncated {claim} u32 at 0x{:x}", self.offset));
        }
        let value =
            u32::from_le_bytes(self.bytes[self.offset..self.offset + 4].try_into().unwrap());
        self.offset += 4;
        Ok(value as usize)
    }

    fn take(&mut self, size: usize, claim: &str) -> Result<Vec<u8>, String> {
        if size > self.bytes.len() - self.offset {
            return Err(format!("truncated {claim} at 0x{:x}: need {size} bytes", self.offset));
        }
        let value = self.bytes[self.offset..self.offset + size].to_vec();
        self.offset += size;
        Ok(value)
    }

    fn string(&mut self, claim: &str) -> Result<(String, usize), String> {
        let offset = self.offset;
        let size = self.u32(&format!("{claim} length"))?;
        let raw = self.take(size, claim)?;
        if raw.last() != Some(&0) {
            return Err(format!("{claim} is not NUL-terminated"));
        }
        let name = utf8(&raw[..raw.len() - 1]).map_err(|_| format!("{claim} is not UTF-8"))?;
        Ok((name.to_owned(), offset))
    }

    fn parse_node(&mut self, depth: usize) -> Result<NativeChunkNode, String> {
        if depth > SYNCBUFFER_MAX_DEPTH {
            return Err(format!("SyncBuffer nesting exceeds {SYNCBUFFER_MAX_DEPTH} levels"));
        }
        self.node_count += 1;
        if self.node_count > SYNCBUFFER_MAX_NODES {
            return Err(format!("SyncBuffer node count exceeds {SYNCBUFFER_MAX_NODES}"));
        }
        let offset = self.offset;
        let length = self.u32("node payload length")?;
        let payload_offset = self.offset;
        let payload = self.take(length, "node payload")?;
        let count = self.u32("node child count")?;
        if count > (self.bytes.len() - self.offset) / 8 {
            return Err(format!("node at 0x{offset:x} declares impossible child count {count}"));
        }
        let mut children = Vec::with_capacity(count);
        for _ in 0..count {
            children.push(self.parse_node(depth + 1)?);
        }
        Ok(NativeChunkNode { children, offset, payload, payload_offset })
    }

    fn parse_buffer(&mut self, depth: usize) -> Result<NativeSyncBuffer, String> {
        let offset = self.offset;
        let root = self.parse_node(depth)?;
        let count = self.u32("named-buffer count")?;
        if count > (self.bytes.len() - self.offset) / 13 {
            return Err(format!(
                "buffer at 0x{offset:x} declares impossible named-buffer count {count}"
            ));
        }
        let mut names = BTreeSet::new();
        let mut named_buffers = Vec::with_capacity(count);
        for index in 0..count {
            let (name, name_offset) = self.string(&format!("named buffer {index} name"))?;
            if !names.insert(name.clone()) {
                let quoted = serde_json::to_string(&name).expect("a native name is a JSON string");
                return Err(format!(
                    "buffer at 0x{offset:x} has ambiguous duplicate name {quoted}"
                ));
            }
            let buffer = self.parse_buffer(depth + 1)?;
            named_buffers.push(NativeNamedBuffer { buffer, name, name_offset });
        }
        Ok(NativeSyncBuffer { end_offset: self.offset, named_buffers, offset, root })
    }
}

pub fn parse_native_sync_buffer(bytes: &[u8]) -> Result<NativeSyncBuffer, String> {
    if bytes.len() > SYNCBUFFER_MAX_BYTES {
        return Err(format!(
            "SyncBuffer input is {} bytes; limit is {SYNCBUFFER_MAX_BYTES}",
            bytes.len()
        ));
    }
    let mut cursor = NativeCursor { bytes, node_count: 0, offset: 0 };
    let result = cursor.parse_buffer(0)?;
    if cursor.offset != bytes.len() {
        return Err(format!(
            "SyncBuffer ended at 0x{:x} with {} unclaimed bytes",
            cursor.offset,
            bytes.len() - cursor.offset
        ));
    }
    Ok(result)
}

/// Recorded offsets describe the input; encoding recomputes their wire positions.
pub fn encode_native_sync_buffer(buffer: &NativeSyncBuffer) -> Result<Vec<u8>, String> {
    let mut output = Vec::new();
    encode_buffer(buffer, &mut output)?;
    Ok(output)
}

fn encode_buffer(buffer: &NativeSyncBuffer, output: &mut Vec<u8>) -> Result<(), String> {
    encode_node(&buffer.root, output)?;
    push_u32(output, buffer.named_buffers.len())?;
    let mut names = BTreeSet::new();
    for item in &buffer.named_buffers {
        if !names.insert(&item.name) {
            let quoted = serde_json::to_string(&item.name).expect("a native name is a JSON string");
            return Err(format!("cannot encode ambiguous duplicate named buffer {quoted}"));
        }
        push_u32(output, item.name.len() + 1)?;
        output.extend_from_slice(item.name.as_bytes());
        output.push(0);
        encode_buffer(&item.buffer, output)?;
    }
    Ok(())
}

fn encode_node(node: &NativeChunkNode, output: &mut Vec<u8>) -> Result<(), String> {
    push_u32(output, node.payload.len())?;
    output.extend_from_slice(&node.payload);
    push_u32(output, node.children.len())?;
    for child in &node.children {
        encode_node(child, output)?;
    }
    Ok(())
}

fn push_u32(output: &mut Vec<u8>, value: usize) -> Result<(), String> {
    let word = u32::try_from(value).map_err(|_| format!("u32 value {value} is invalid"))?;
    output.extend_from_slice(&word.to_le_bytes());
    Ok(())
}

/// TextDecoder's default BOM handling also applies to native names and ZIP paths.
pub(super) fn utf8(bytes: &[u8]) -> Result<&str, std::str::Utf8Error> {
    std::str::from_utf8(bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes))
}

fn xor_darkdata(bytes: &mut [u8]) {
    for (index, value) in bytes.iter_mut().enumerate() {
        *value ^= NATIVE_DARKDATA_KEY[index % NATIVE_DARKDATA_KEY.len()];
    }
}

fn decode_varint(bytes: &[u8], start: usize, claim: &str) -> Result<(usize, usize), String> {
    let mut value: u64 = 0;
    let mut offset = start;
    for _ in 0..5 {
        let Some(&byte) = bytes.get(offset) else {
            return Err(format!("truncated {claim} varint at 0x{start:x}"));
        };
        offset += 1;
        if value > 0x01ff_ffff {
            return Err(format!("overflowing {claim} varint at 0x{start:x}"));
        }
        value = value * 128 + u64::from(byte & 0x7f);
        if byte & 0x80 == 0 {
            return Ok((offset, value as usize));
        }
    }
    Err(format!("overlong {claim} varint at 0x{start:x}"))
}

fn encode_varint(output: &mut Vec<u8>, mut value: usize) {
    let mut groups = [0_u8; 5];
    let mut index = 4;
    groups[index] = (value & 0x7f) as u8;
    value /= 128;
    while value > 0 {
        index -= 1;
        groups[index] = (value & 0x7f) as u8 | 0x80;
        value /= 128;
    }
    output.extend_from_slice(&groups[index..]);
}

fn decompress_darkdata(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let Some(&marker) = bytes.first() else {
        return Err("darkdata codec input is empty".into());
    };
    let mut offset = 1;
    let mut output = Vec::new();
    while offset < bytes.len() {
        let value = bytes[offset];
        offset += 1;
        if value != marker {
            output.push(value);
        } else {
            let Some(&command) = bytes.get(offset) else {
                return Err(format!("truncated marker command at 0x{:x}", offset - 1));
            };
            if command == 0 {
                output.push(marker);
                offset += 1;
            } else {
                let (after_length, length) = decode_varint(bytes, offset, "match length")?;
                let (after_distance, distance) =
                    decode_varint(bytes, after_length, "match distance")?;
                offset = after_distance;
                if length == 0 {
                    return Err("back-reference has zero length".into());
                }
                if distance == 0 || distance > output.len() {
                    return Err(format!(
                        "back-reference distance {distance} exceeds decoded prefix {}",
                        output.len()
                    ));
                }
                if length > DARKDATA_MAX_DECOMPRESSED_BYTES - output.len() {
                    return Err("darkdata decompression exceeds the native 32 MiB ceiling".into());
                }
                for _ in 0..length {
                    output.push(output[output.len() - distance]);
                }
            }
        }
        if output.len() > DARKDATA_MAX_DECOMPRESSED_BYTES {
            return Err("darkdata decompression exceeds the native 32 MiB ceiling".into());
        }
    }
    Ok(output)
}

fn compress_darkdata(bytes: &[u8]) -> Vec<u8> {
    if bytes.is_empty() {
        return Vec::new();
    }
    let mut frequencies = [0_u32; 256];
    for &value in bytes {
        frequencies[usize::from(value)] += 1;
    }
    let mut marker = 0;
    for value in 1..frequencies.len() {
        if frequencies[value] < frequencies[marker] {
            marker = value;
        }
    }
    // The source's two-byte chain order decides which equal-length match wins.
    let mut previous = vec![-1_i32; bytes.len()];
    let mut heads = vec![-1_i32; 65_536];
    for index in 0..bytes.len().saturating_sub(1) {
        let key = (usize::from(bytes[index]) << 8) | usize::from(bytes[index + 1]);
        previous[index] = heads[key];
        heads[key] = index as i32;
    }
    let marker = marker as u8;
    let mut output = vec![marker];
    let mut position = 0;
    let mut remaining = bytes.len();
    while remaining > 3 {
        let mut candidate = previous[position];
        let mut best_length = 3;
        let mut best_distance = 0;
        while candidate != -1 {
            let index = candidate as usize;
            let distance = position - index;
            if distance > 99_999 {
                break;
            }
            if bytes.get(index + 3) == bytes.get(position + 3) {
                let limit = distance.min(remaining);
                let mut length = 2;
                while length < limit && bytes[index + length] == bytes[position + length] {
                    length += 1;
                }
                if length > best_length {
                    best_length = length;
                    best_distance = distance;
                }
            }
            candidate = previous[index];
        }
        let accept = match best_length {
            4 => best_distance <= 0x7f,
            5 => best_distance <= 0x3fff,
            6 => best_distance <= 0x1f_ffff,
            7 => best_distance <= 0x0fff_ffff,
            _ => best_length >= 8,
        };
        if accept {
            output.push(marker);
            encode_varint(&mut output, best_length);
            encode_varint(&mut output, best_distance);
            position += best_length;
            remaining -= best_length;
        } else {
            let value = bytes[position];
            output.push(value);
            if value == marker {
                output.push(0);
            }
            position += 1;
            remaining -= 1;
        }
    }
    for &value in &bytes[position..] {
        output.push(value);
        if value == marker {
            output.push(0);
        }
    }
    output
}

pub fn decode_native_darkdata(bytes: &[u8]) -> Result<NativeSyncBuffer, String> {
    let mut decoded = decompress_darkdata(bytes)?;
    xor_darkdata(&mut decoded);
    parse_native_sync_buffer(&decoded)
}

pub fn encode_native_darkdata(buffer: &NativeSyncBuffer) -> Result<Vec<u8>, String> {
    let mut encoded = encode_native_sync_buffer(buffer)?;
    xor_darkdata(&mut encoded);
    Ok(compress_darkdata(&encoded))
}
