//! Authored binary geometry (`editor/format/boneyard.ts` → `host/project-boneyard.ts`).

use solomon_sim::Vec2;
use solomon_sim::boneyard::{
    BoneyardBounds, BoneyardFence, BoneyardObject, BoneyardRoad, BoneyardScene, BoneyardSpawn,
    BoneyardSprite, BoneyardTerrain, SolomonDig,
};

use crate::native_save::NativeChunkNode;

const MAX_CHUNKS: usize = 1_000_000;
const MAX_NAMED_BUFFERS: usize = 65_536;

/// Decode the actual authored file before content admission validates the scene.
/// Unknown object/recipe payloads have the same opaque projection as the editor.
pub fn decode_boneyard(bytes: &[u8]) -> Result<BoneyardScene, String> {
    if bytes.len() > 256 * 1024 * 1024 {
        return Err("Boneyard: file exceeds 256 MiB".into());
    }
    let mut reader = BoneyardReader { bytes, offset: 0, chunks: 0, named_buffers: 0 };
    let root = reader.buffer(0)?;
    if reader.offset != bytes.len() {
        return Err(format!("Boneyard: trailing bytes at {}", reader.offset));
    }
    validate_envelope(&root)?;
    let arena = &root.children[0];
    let layout = &arena.children[12].children[0];
    let (name, environment_mode, bounds) = header(&arena.children[0].payload)?;

    let (ids, children) = manager(&layout.children[0])?;
    let mut objects = Vec::new();
    let mut child_index = 0;
    for (index, type_id) in ids.into_iter().enumerate() {
        let count = if is_placeable(type_id) { 3 } else { 1 };
        if count > children.len() - child_index {
            return Err("world object is missing chunks".into());
        }
        objects.push(decode_object(type_id, &children[child_index..child_index + count], index));
        child_index += count;
    }
    if child_index != children.len() {
        return Err("unclaimed world-object chunks".into());
    }

    // The editor reads all three managers before validating their supported IDs.
    let (road_ids, road_children) = manager(&layout.children[5])?;
    let (fence_ids, fence_children) = manager(&layout.children[6])?;
    let (terrain_ids, terrain_children) = manager(&layout.children[12])?;
    validate_manager(&road_ids, road_children, 3004, "unsupported Road manager")?;
    validate_manager(&fence_ids, fence_children, 3005, "unsupported Fence manager")?;
    validate_manager(&terrain_ids, terrain_children, 3009, "unsupported Terrain manager")?;

    let sprite_payload = &layout.children[11].payload;
    let sprite_count = read_u32(sprite_payload, 0)? as usize;
    if sprite_payload.len() as u64 != 4 + sprite_count as u64 * 25 {
        return Err("invalid static-sprite list".into());
    }
    let mut sprites = Vec::new();
    for index in 0..sprite_count {
        let offset = 4 + index * 25;
        let atlas_entry = read_u32(sprite_payload, offset)?;
        sprites.push(BoneyardSprite {
            eid: format!("sprite-{index}"),
            atlas_entry: atlas_entry as usize,
            dead_hawg_entry: Some(u64::from(atlas_entry) + 114),
            position: read_point(sprite_payload, offset + 4)?,
            s0: read_f32(sprite_payload, offset + 12)?,
            s1: read_f32(sprite_payload, offset + 16)?,
            s2: read_f32(sprite_payload, offset + 20)?,
            flags: u32::from(sprite_payload[offset + 24]),
        });
    }
    // Recipe decoders preserve malformed/unknown records as opaque data. Their
    // fields are not part of projectBoneyard; only manager membership can reject.
    for section in [3, 4, 7, 8, 9, 13] {
        let (ids, children) = manager(&layout.children[section])?;
        if ids.len() != children.len() {
            return Err("manager child count mismatch".into());
        }
    }
    let spawn_payload = &layout.children[2].payload;
    if spawn_payload.len() != 12 {
        return Err(format!("{name}: Boneyard has no player spawn"));
    }
    let spawn = BoneyardSpawn {
        position: read_point(spawn_payload, 0)?,
        facing_degrees: read_f32(spawn_payload, 8)?,
    };
    let mut scene = BoneyardScene {
        name,
        environment_mode,
        bounds,
        spawn,
        objects,
        sprites,
        roads: road_children
            .iter()
            .enumerate()
            .map(|(index, chunk)| decode_road(chunk, index))
            .collect(),
        fences: fence_children
            .iter()
            .enumerate()
            .map(|(index, chunk)| decode_fence(chunk, index))
            .collect(),
        terrain: terrain_children
            .iter()
            .enumerate()
            .map(|(index, chunk)| decode_terrain(chunk, index))
            .collect(),
        solomon_dig: None,
    };
    materialize_native_scenery(&mut scene);
    materialize_opening_solomon(&mut scene);
    Ok(scene)
}

// Save files and authored Boneyards share chunk storage, but their readers have
// different source limits and name policies. Boneyard names may be duplicate or
// non-UTF-8, so applying the stricter save decoder would reject accepted content.
struct BoneyardReader<'a> {
    bytes: &'a [u8],
    offset: usize,
    chunks: usize,
    named_buffers: usize,
}

impl BoneyardReader<'_> {
    fn u32(&mut self) -> Result<usize, String> {
        let value = read_u32(self.bytes, self.offset)?;
        self.offset += 4;
        Ok(value as usize)
    }

    fn read(&mut self, size: usize) -> Result<&[u8], String> {
        if size > self.bytes.len() - self.offset {
            return Err(format!("Boneyard: truncated at byte {}", self.offset));
        }
        let value = &self.bytes[self.offset..self.offset + size];
        self.offset += size;
        Ok(value)
    }

    fn chunk(&mut self, depth: usize) -> Result<NativeChunkNode, String> {
        if depth > 512 {
            return Err("Boneyard: chunk nesting exceeds 512".into());
        }
        self.chunks += 1;
        if self.chunks > MAX_CHUNKS {
            return Err("Boneyard: too many chunks".into());
        }
        let offset = self.offset;
        let size = self.u32()?;
        let payload_offset = self.offset;
        let payload = self.read(size)?.to_vec();
        let count = self.u32()?;
        if count > MAX_CHUNKS - self.chunks {
            return Err("Boneyard: too many chunks".into());
        }
        let mut children = Vec::new();
        for _ in 0..count {
            children.push(self.chunk(depth + 1)?);
        }
        Ok(NativeChunkNode { children, offset, payload, payload_offset })
    }

    fn buffer(&mut self, depth: usize) -> Result<NativeChunkNode, String> {
        let root = self.chunk(depth)?;
        let count = self.u32()?;
        if self.named_buffers > MAX_NAMED_BUFFERS || count > MAX_NAMED_BUFFERS - self.named_buffers
        {
            return Err("Boneyard: too many named buffers".into());
        }
        for _ in 0..count {
            self.named_buffers += 1;
            let size = self.u32()?;
            let name = self.read(size)?;
            if name.is_empty() || name.last() != Some(&0) || name[..name.len() - 1].contains(&0) {
                return Err("Boneyard: invalid named buffer".into());
            }
            // Named payloads are parsed for framing/limits, then omitted by the
            // source geometry projection. They do not alter the top-level Arena.
            self.buffer(depth + 1)?;
        }
        Ok(root)
    }
}

fn validate_envelope(root: &NativeChunkNode) -> Result<(), String> {
    if !root.payload.is_empty() || root.children.len() != 1 {
        return Err("Boneyard: invalid root".into());
    }
    let arena = &root.children[0];
    if !arena.payload.is_empty() || arena.children.len() != 13 {
        return Err("Boneyard: invalid Arena".into());
    }
    let region = &arena.children[12];
    if !region.payload.is_empty() || region.children.len() != 1 {
        return Err("Boneyard: invalid Region".into());
    }
    let layout = &region.children[0];
    if !layout.payload.is_empty() || layout.children.len() != 14 {
        return Err("Boneyard: invalid RegionLayout".into());
    }
    Ok(())
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, String> {
    let value = bytes.get(offset..offset.saturating_add(4)).ok_or("truncated u32")?;
    Ok(u32::from_le_bytes(value.try_into().expect("the word slice has four bytes")))
}

fn read_f32(bytes: &[u8], offset: usize) -> Result<f64, String> {
    let value = bytes.get(offset..offset.saturating_add(4)).ok_or("truncated f32")?;
    Ok(f64::from(f32::from_le_bytes(value.try_into().expect("the float slice has four bytes"))))
}

fn read_point(bytes: &[u8], offset: usize) -> Result<Vec2, String> {
    Ok(Vec2::new(read_f32(bytes, offset)?, read_f32(bytes, offset + 4)?))
}

fn header(bytes: &[u8]) -> Result<(String, u32, BoneyardBounds), String> {
    let size = read_u32(bytes, 0)? as usize;
    if size == 0 || size > bytes.len().saturating_sub(4) || bytes[4 + size - 1] != 0 {
        return Err("invalid native String".into());
    }
    let end = 4 + size;
    let text = &bytes[4..end - 1];
    // The editor's nonfatal TextDecoder replaces malformed UTF-8 and strips BOM.
    let text = text.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(text);
    let name = String::from_utf8_lossy(text).into_owned();
    if bytes.len() - end < 535 {
        return Err("Arena header is too short".into());
    }
    let left = read_f32(bytes, end + 519)?;
    let top = read_f32(bytes, end + 523)?;
    let right = read_f32(bytes, end + 527)?;
    let bottom = read_f32(bytes, end + 531)?;
    Ok((
        name,
        u32::from(bytes[end + 518]),
        BoneyardBounds::new(left, top, right - left, bottom - top),
    ))
}

fn manager(section: &NativeChunkNode) -> Result<(Vec<u32>, &[NativeChunkNode]), String> {
    if section.payload.len() < 4 {
        return Err("truncated polymorphic manager".into());
    }
    let count = read_u32(&section.payload, 0)?;
    if section.payload.len() as u64 != 4 + u64::from(count) * 4 {
        return Err("invalid polymorphic manager".into());
    }
    let mut ids = Vec::new();
    for index in 0..count as usize {
        ids.push(read_u32(&section.payload, 4 + index * 4)?);
    }
    Ok((ids, &section.children))
}

fn validate_manager(
    ids: &[u32],
    children: &[NativeChunkNode],
    type_id: u32,
    claim: &str,
) -> Result<(), String> {
    if ids.len() != children.len() || ids.iter().any(|id| *id != type_id) {
        return Err(claim.into());
    }
    Ok(())
}

fn is_placeable(type_id: u32) -> bool {
    matches!(type_id, 2001 | 2009 | 2029 | 2040 | 2061)
}

fn decode_object(type_id: u32, chunks: &[NativeChunkNode], index: usize) -> BoneyardObject {
    let mut item = BoneyardObject {
        eid: format!("object-{index}"),
        type_id,
        position: Vec2::default(),
        variant: None,
        rotation_degrees: None,
        scale: None,
        sort_bias: None,
        atlas_entry: None,
        secondary_atlas_entry: None,
        secondary_variant: None,
        secondary_visible: None,
        overlay_atlas_entry: None,
        overlay_variant: None,
        atlas_entries: None,
    };
    if !is_placeable(type_id) || chunks.len() != 3 || chunks[0].payload.len() != 41 {
        return item;
    }
    item.position =
        read_point(&chunks[0].payload, 0).expect("the placeable position has eight bytes");
    if chunks[1].payload.len() >= 53 {
        item.sort_bias =
            Some(read_f32(&chunks[1].payload, 49).expect("the sort bias has four bytes"));
    }
    let subtype = &chunks[2].payload;
    let short = |offset| u32::from(u16::from_le_bytes([subtype[offset], subtype[offset + 1]]));
    match (type_id, subtype.len()) {
        (2001, 5) => {
            let variant = short(0);
            let secondary = short(2);
            item.variant = Some(variant);
            item.secondary_variant = Some(secondary);
            item.secondary_visible = Some(subtype[4] != 0);
            item.atlas_entry = Some(264 + variant as usize);
            item.secondary_atlas_entry = Some(243 + secondary as usize);
        }
        (2009, 2) => {
            let variant = short(0);
            item.variant = Some(variant);
            item.atlas_entry = Some(156 + variant as usize);
        }
        (2029, 20) => {
            let variant = short(0);
            let overlay = short(2);
            item.variant = Some(variant);
            item.overlay_variant = Some(overlay);
            item.atlas_entry = Some(97 + variant as usize);
            item.overlay_atlas_entry = Some(88 + overlay as usize);
        }
        (2040, 2) => {
            let variant = short(0);
            item.variant = Some(variant);
            item.atlas_entries = Some(vec![148 + variant as usize, 152 + variant as usize]);
        }
        (2061, 12) => {
            let variant = short(0) * 2 + u32::from(subtype[2]);
            item.variant = Some(variant);
            item.atlas_entry = Some(145 + variant as usize);
        }
        _ => {}
    }
    item
}

// Fixed-size records have already admitted every field offset before reading.
fn decode_road(chunk: &NativeChunkNode, index: usize) -> BoneyardRoad {
    let mut item = BoneyardRoad {
        eid: format!("road-{index}"),
        type_id: 3004,
        link_mask: 0,
        points: Vec::new(),
        style: None,
        start_width_scale: None,
        end_width_scale: None,
        quad: None,
    };
    if chunk.payload.len() != 69 {
        return item;
    }
    let bytes = &chunk.payload;
    item.points = vec![read_point(bytes, 0).unwrap(), read_point(bytes, 8).unwrap()];
    item.link_mask = u8::from(read_u32(bytes, 20).unwrap() != u32::MAX)
        | (u8::from(read_u32(bytes, 24).unwrap() != u32::MAX) << 1);
    item.quad = Some((0..4).map(|index| read_point(bytes, 28 + index * 8).unwrap()).collect());
    item.style = Some(u32::from(bytes[60]));
    item.start_width_scale = Some(read_f32(bytes, 61).unwrap());
    item.end_width_scale = Some(read_f32(bytes, 65).unwrap());
    item
}

fn decode_fence(chunk: &NativeChunkNode, index: usize) -> BoneyardFence {
    let mut item = BoneyardFence {
        eid: format!("fence-{index}"),
        type_id: 3005,
        points: Vec::new(),
        style: None,
        segment_code: None,
        start_post_variant: None,
        end_post_variant: None,
    };
    if chunk.payload.len() != 29 {
        return item;
    }
    let bytes = &chunk.payload;
    item.points = vec![read_point(bytes, 0).unwrap(), read_point(bytes, 8).unwrap()];
    let start = read_u32(bytes, 20).unwrap();
    let end = read_u32(bytes, 24).unwrap();
    item.start_post_variant = (start != u32::MAX).then_some(start);
    item.end_post_variant = (end != u32::MAX).then_some(end);
    item.style = Some(u32::from(bytes[28]));
    item.segment_code = item.style;
    item
}

fn decode_terrain(chunk: &NativeChunkNode, index: usize) -> BoneyardTerrain {
    let mut item = BoneyardTerrain {
        eid: format!("terrain-{index}"),
        position: Vec2::default(),
        points: None,
        style: None,
        entry: None,
        uid: None,
        profile_samples: None,
        side_sign: None,
    };
    let bytes = &chunk.payload;
    // Malformed terrain remains an opaque row in the editor. Check lengths before
    // allocating its counted arrays, retaining the same empty geometry on failure.
    let decode = || -> Result<_, String> {
        let style = read_u32(bytes, 0)?;
        let count = read_u32(bytes, 8)? as usize;
        if count as u64 * 8 + 24 > bytes.len() as u64 {
            return Err("opaque terrain".into());
        }
        let mut points = Vec::new();
        for index in 0..count {
            points.push(read_point(bytes, 12 + index * 8)?);
        }
        let offset = 12 + count * 8;
        let uid = f64::from(read_u32(bytes, offset)?);
        let profile_count = read_u32(bytes, offset + 4)?;
        if offset as u64 + 12 + u64::from(profile_count) * 4 != bytes.len() as u64 {
            return Err("opaque terrain".into());
        }
        let mut profile = Vec::with_capacity(profile_count as usize);
        for index in 0..profile_count as usize {
            profile.push(read_f32(bytes, offset + 8 + index * 4)?);
        }
        let side_sign = read_f32(bytes, offset + 8 + profile_count as usize * 4)?;
        Ok((style, points, uid, profile, side_sign))
    };
    if let Ok((style, points, uid, profile, side_sign)) = decode() {
        item.position = points.first().copied().unwrap_or_default();
        item.style = Some(style);
        item.entry = Some(style as usize);
        item.points = Some(points);
        item.uid = Some(uid);
        item.profile_samples = Some(profile);
        item.side_sign = Some(side_sign);
    }
    item
}

/// Current `project-boneyard.ts::materializeNativeBoneyardScenery` retires
/// stored Tree15..18, then appends freshly constructed Scrubs in source order.
fn materialize_native_scenery(scene: &mut BoneyardScene) {
    let mut replacements = Vec::new();
    scene.objects.retain(|object| {
        let variant = object.variant.unwrap_or(0);
        if object.type_id != 2001 || !(15..=18).contains(&variant) {
            return true;
        }
        replacements.push(BoneyardObject {
            eid: object.eid.clone(),
            type_id: 2062,
            position: object.position,
            variant: Some(variant),
            atlas_entry: Some(264 + variant as usize),
            rotation_degrees: None,
            scale: None,
            sort_bias: None,
            secondary_atlas_entry: None,
            secondary_variant: None,
            secondary_visible: None,
            overlay_atlas_entry: None,
            overlay_variant: None,
            atlas_entries: None,
        });
        false
    });
    scene.objects.extend(replacements);
}

fn materialize_opening_solomon(scene: &mut BoneyardScene) {
    let mut selected = None;
    let mut selected_distance = f64::INFINITY;
    for object in &scene.objects {
        if object.type_id != 2029 || object.overlay_variant != Some(8) {
            continue;
        }
        let dx = object.position.x - scene.spawn.position.x;
        let dy = object.position.y - scene.spawn.position.y;
        let distance = dx * dx + dy * dy;
        if distance < selected_distance {
            selected = Some(object.position);
            selected_distance = distance;
        }
    }
    let Some(grave) = selected else {
        return;
    };
    let position = Vec2::new(grave.x + 10.0, grave.y + 113.0);
    scene.solomon_dig = Some(SolomonDig {
        grave_position: grave,
        lantern_position: Vec2::new(grave.x - 55.0, grave.y + 73.0),
        position,
        frame_program: vec![
            0, 0, 0, 0, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 15, 17, 17, 17, 17, 16, 15, 13, 11, 9,
            7, 5, 3, 1,
        ],
        ticks_per_frame: 5,
    });
    scene.sprites.retain(|sprite| {
        let entry = sprite.dead_hawg_entry.map_or(sprite.atlas_entry as u64, |entry| entry - 114);
        !clutter_contains(sprite, entry, position)
            && (!(21..=24).contains(&entry) || !clutter_contains(sprite, entry, grave))
    });
}

fn clutter_contains(sprite: &BoneyardSprite, entry: u64, point: Vec2) -> bool {
    let (width, height) = match entry {
        6 => (260.0, 178.0),
        7 => (89.0, 89.0),
        8 => (62.0, 62.0),
        21 => (64.0, 56.0),
        22 => (70.0, 58.0),
        23 => (80.0, 62.0),
        24 => (69.0, 59.0),
        _ => return false,
    };
    let scale_y = if sprite.s1.is_finite() { sprite.s1.max(0.0) } else { 1.0 };
    let scale_x = scale_y * if sprite.flags & 1 != 0 { 0.8 } else { 1.0 };
    let rotation =
        if sprite.s0.is_finite() { sprite.s0 } else { 0.0 } * std::f64::consts::PI / 180.0;
    let dx = point.x - sprite.position.x;
    let dy = point.y - sprite.position.y;
    let local_x = dx * rotation.cos() + dy * rotation.sin();
    let local_y = -dx * rotation.sin() + dy * rotation.cos();
    local_x.abs() <= width * scale_x / 2.0 && local_y.abs() <= height * scale_y / 2.0
}
