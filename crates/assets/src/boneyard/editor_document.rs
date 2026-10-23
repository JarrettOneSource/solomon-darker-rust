//! Complete `editor/format/boneyard.ts::parseBoneyard` documents for the GPU
//! Shelf's native-file import. The existing container reader owns framing/limits;
//! this projection preserves all native state for the Website-owned editor.
use super::{header, is_placeable, manager, read_envelope, read_f32, read_u32, validate_manager};
use crate::native_save::NativeChunkNode;
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};

/// Source `parseBoneyard`, before editor asset hydration. Native bytes and opaque
/// records remain attached; this value never substitutes a playable scene.
pub fn parse_editor_boneyard(bytes: &[u8]) -> Result<Value, String> {
    let root = read_envelope(bytes)?;
    let arena = &root.children[0];
    let layout = &arena.children[12].children[0];
    let payload = &arena.children[0].payload;
    let (name, environment, _) = header(payload)?;
    let end = 4 + read_u32(payload, 0)? as usize;
    let meta = json!({"name":name,"bounds":bounds_value(read_f32(payload,end+519)?,read_f32(payload,end+523)?,read_f32(payload,end+527)?,read_f32(payload,end+531)?),"header":{"flags":&payload[end..end+6],"arenaRuleMode":payload[end+2],"sessionFlag":payload[end+5],"compatibilityFlags":STANDARD.encode(&payload[end+6..end+518]),"environmentMode":environment,"trailing":STANDARD.encode(&payload[end+535..])},"raw":{"file":STANDARD.encode(bytes),"arenaSections":arena.children.iter().map(raw).collect::<Vec<_>>()}});
    let (ids, children) = manager(&layout.children[0])?;
    let mut objects = Vec::new();
    let mut cursor = 0;
    for (index, id) in ids.into_iter().enumerate() {
        let count = if is_placeable(id) { 3 } else { 1 };
        if count > children.len() - cursor {
            return Err("world object is missing chunks".to_owned());
        }
        objects.push(object(id, &children[cursor..cursor + count], index)?);
        cursor += count;
    }
    if cursor != children.len() {
        return Err("unclaimed world-object chunks".to_owned());
    }
    let (roads, road_children) = manager(&layout.children[5])?;
    let (fences, fence_children) = manager(&layout.children[6])?;
    let (terrain, terrain_children) = manager(&layout.children[12])?;
    validate_manager(&roads, road_children, 3004, "unsupported Road manager")?;
    validate_manager(&fences, fence_children, 3005, "unsupported Fence manager")?;
    validate_manager(&terrain, terrain_children, 3009, "unsupported Terrain manager")?;
    let sprite_payload = &layout.children[11].payload;
    let count = read_u32(sprite_payload, 0)? as usize;
    if sprite_payload.len() as u64 != 4 + count as u64 * 25 {
        return Err("invalid static-sprite list".to_owned());
    }
    let sprites =
        (0..count).map(|i| sprite(sprite_payload, 4 + i * 25, i)).collect::<Result<Vec<_>, _>>()?;
    let monsters =
        recipes(
            &layout.children[3],
            |id, c, i| if id == 6001 { monster(c, i) } else { opaque(id, c, i) },
        )?;
    let groups =
        recipes(
            &layout.children[4],
            |id, c, i| if id == 6002 { uid_group(c, i) } else { opaque(id, c, i) },
        )?;
    let items = recipes(&layout.children[7], opaque)?;
    let item_sets = recipes(&layout.children[8], opaque)?;
    let npcs = recipes(&layout.children[9], opaque)?;
    let timelines = recipes(&layout.children[13], timeline)?;
    let mut geometry = json!({"triggerControlRaw":raw(&layout.children[1]),"regionGeometryRaw":raw(&layout.children[2]),"layoutFlagRaw":STANDARD.encode(&layout.children[10].payload),"rawSections":layout.children.iter().map(raw).collect::<Vec<_>>()});
    if layout.children[2].payload.len() == 12 {
        geometry["playerSpawn"] = point(&layout.children[2].payload, 0)?;
        geometry["playerSpawnFacingDeg"] = read_f32(&layout.children[2].payload, 8)?.into();
    }
    if layout.children[10].payload.len() == 1 {
        geometry["layoutFlag"] = layout.children[10].payload[0].into();
    }
    let mut preserved = Vec::new();
    for (kind, rows, label) in [
        ("monsterRecipe", &monsters, true),
        ("itemRecipe", &items, false),
        ("npcRecipe", &npcs, true),
        ("itemSet", &item_sets, false),
        ("uidGroup", &groups, true),
        ("timeline", &timelines, true),
    ] {
        for row in rows {
            let mut entry = json!({"kind":kind,"raw":row["raw"]});
            if label && let Some(name) = row.get("name") {
                entry["label"] = name.clone();
            }
            preserved.push(entry);
        }
    }
    Ok(
        json!({"format":"solomon-dark-boneyard","version":1,"meta":meta,"objects":objects,"roads":road_children.iter().enumerate().map(|(i,c)|road(c,i)).collect::<Result<Vec<_>,_>>()?,"fences":fence_children.iter().enumerate().map(|(i,c)|fence(c,i)).collect::<Result<Vec<_>,_>>()?,"terrain":terrain_children.iter().enumerate().map(|(i,c)|terrain_patch(c,i)).collect::<Vec<_>>(),"sprites":sprites,"recipes":{"monsters":monsters,"items":items,"npcs":npcs,"itemSets":item_sets,"uidGroups":groups},"timeline":{"records":timelines,"defaultTransplantSafe":false},"geometry":geometry,"opaque":preserved,"hasTimeline":!timelines.is_empty()}),
    )
}
fn bounds_value(left: f64, top: f64, right: f64, bottom: f64) -> Value {
    json!({"left":left,"top":top,"right":right,"bottom":bottom,"x":left,"y":top,"w":right-left,"h":bottom-top})
}
fn point(bytes: &[u8], offset: usize) -> Result<Value, String> {
    Ok(json!({"x":read_f32(bytes,offset)?,"y":read_f32(bytes,offset+4)?}))
}
fn encode_chunk(chunk: &NativeChunkNode, out: &mut Vec<u8>) {
    out.extend_from_slice(&(chunk.payload.len() as u32).to_le_bytes());
    out.extend_from_slice(&chunk.payload);
    out.extend_from_slice(&(chunk.children.len() as u32).to_le_bytes());
    for child in &chunk.children {
        encode_chunk(child, out);
    }
}
fn raw(chunk: &NativeChunkNode) -> String {
    raw_chunks(std::slice::from_ref(chunk))
}
fn raw_chunks(chunks: &[NativeChunkNode]) -> String {
    let mut bytes = Vec::new();
    for chunk in chunks {
        encode_chunk(chunk, &mut bytes);
    }
    STANDARD.encode(bytes)
}
fn type_name(id: u32) -> Option<&'static str> {
    Some(match id {
        2001 => "Tree",
        2009 => "Monument",
        2029 => "Gravestone",
        2040 => "Building",
        2061 => "Goodie",
        3004 => "Road",
        3005 => "Fence",
        3006 => "Fencepost",
        3007 => "FenceGrate",
        3009 => "Terrain",
        6001 => "MonsterRecipe",
        6002 => "UIDGroup",
        6003 => "ItemRecipe",
        6004 => "NPCRecipe",
        6005 => "ItemSet",
        6006 => "TimeLine",
        6007 => "TimeLineEvent",
        _ => return None,
    })
}
fn object(id: u32, chunks: &[NativeChunkNode], index: usize) -> Result<Value, String> {
    let mut item = json!({"eid":format!("object-{index}"),"typeId":id,"pos":{"x":0,"y":0},"raw":raw_chunks(chunks)});
    if let Some(name) = type_name(id) {
        item["typeName"] = name.into();
    }
    if !is_placeable(id) || chunks.len() != 3 || chunks[0].payload.len() != 41 {
        return Ok(item);
    }
    item["pos"] = point(&chunks[0].payload, 0)?;
    if chunks[1].payload.len() >= 53 {
        item["sortBias"] = read_f32(&chunks[1].payload, 49)?.into();
    }
    let bytes = &chunks[2].payload;
    let short = |offset| u32::from(u16::from_le_bytes([bytes[offset], bytes[offset + 1]]));
    match (id, bytes.len()) {
        (2001, 5) => {
            let variant = short(0);
            let secondary = short(2);
            item["variant"] = variant.into();
            item["secondaryVariant"] = secondary.into();
            item["secondaryVisible"] = (bytes[4] != 0).into();
            item["secondaryVisibleByte"] = bytes[4].into();
            item["atlasEntry"] = (264 + variant).into();
            item["secondaryAtlasEntry"] = (243 + secondary).into();
        }
        (2009, 2) => {
            let variant = short(0);
            item["variant"] = variant.into();
            item["atlasEntry"] = (156 + variant).into();
        }
        (2029, 20) => {
            let variant = short(0);
            let overlay = short(2);
            item["variant"] = variant.into();
            item["overlayVariant"] = overlay.into();
            item["atlasEntry"] = (97 + variant).into();
            item["overlayAtlasEntry"] = (88 + overlay).into();
            item["tint"] = json!({"r":read_f32(bytes,4)?,"g":read_f32(bytes,8)?,"b":read_f32(bytes,12)?,"a":read_f32(bytes,16)?});
        }
        (2040, 2) => {
            let variant = short(0);
            item["variant"] = variant.into();
            item["atlasEntries"] = json!([148 + variant, 152 + variant]);
        }
        (2061, 12) => {
            let subtype = short(0);
            let phase = u32::from(bytes[2]);
            let variant = subtype * 2 + phase;
            item["subtype"] = subtype.into();
            item["phase"] = phase.into();
            item["variant"] = variant.into();
            item["active"] = (bytes[3] != 0).into();
            item["activeByte"] = bytes[3].into();
            item["timer"] = read_u32(bytes, 4)?.into();
            item["rewardSeed"] = read_u32(bytes, 8)?.into();
            item["atlasEntry"] = (145 + variant).into();
        }
        _ => {}
    }
    Ok(item)
}
fn road(chunk: &NativeChunkNode, index: usize) -> Result<Value, String> {
    let mut item =
        json!({"eid":format!("road-{index}"),"typeId":3004,"points":[],"raw":raw(chunk)});
    let b = &chunk.payload;
    if b.len() != 69 {
        return Ok(item);
    }
    item["points"] = json!([point(b, 0)?, point(b, 8)?]);
    item["uid"] = read_u32(b, 16)?.into();
    item["previousUid"] = read_u32(b, 20)?.into();
    item["nextUid"] = read_u32(b, 24)?.into();
    item["quad"] = Value::Array((0..4).map(|i| point(b, 28 + i * 8)).collect::<Result<_, _>>()?);
    item["style"] = b[60].into();
    item["startWidthScale"] = read_f32(b, 61)?.into();
    item["endWidthScale"] = read_f32(b, 65)?.into();
    Ok(item)
}
fn fence(chunk: &NativeChunkNode, index: usize) -> Result<Value, String> {
    let mut item =
        json!({"eid":format!("fence-{index}"),"typeId":3005,"points":[],"raw":raw(chunk)});
    let b = &chunk.payload;
    if b.len() != 29 {
        return Ok(item);
    }
    item["points"] = json!([point(b, 0)?, point(b, 8)?]);
    item["uid"] = read_u32(b, 16)?.into();
    item["startPostVariant"] = read_u32(b, 20)?.into();
    item["endPostVariant"] = read_u32(b, 24)?.into();
    item["segmentCode"] = b[28].into();
    item["style"] = b[28].into();
    Ok(item)
}
fn terrain_patch(chunk: &NativeChunkNode, index: usize) -> Value {
    let fallback = json!({"eid":format!("terrain-{index}"),"typeId":3009,"pos":{"x":0,"y":0},"raw":raw(chunk)});
    let decode = || -> Result<Value, String> {
        let b = &chunk.payload;
        let style = read_u32(b, 0)?;
        let reserved = read_u32(b, 4)?;
        let count = read_u32(b, 8)? as usize;
        if count as u64 * 8 + 24 > b.len() as u64 {
            return Err("opaque terrain".into());
        }
        let points = (0..count).map(|i| point(b, 12 + i * 8)).collect::<Result<Vec<_>, _>>()?;
        let offset = 12 + count * 8;
        let uid = read_u32(b, offset)?;
        let profile_count = read_u32(b, offset + 4)? as usize;
        if offset as u64 + 12 + profile_count as u64 * 4 != b.len() as u64 {
            return Err("opaque terrain".into());
        }
        let profiles = (0..profile_count)
            .map(|i| read_f32(b, offset + 8 + i * 4).map(Value::from))
            .collect::<Result<Vec<_>, _>>()?;
        let side = read_f32(b, offset + 8 + profile_count * 4)?;
        let mut item = fallback.clone();
        item["pos"] = points.first().cloned().unwrap_or_else(|| json!({"x":0,"y":0}));
        item["points"] = points.into();
        item["style"] = style.into();
        item["entry"] = style.into();
        item["reserved"] = reserved.into();
        item["uid"] = uid.into();
        item["profileSamples"] = profiles.into();
        item["sideSign"] = side.into();
        Ok(item)
    };
    decode().unwrap_or(fallback)
}
fn sprite(bytes: &[u8], offset: usize, index: usize) -> Result<Value, String> {
    let rotation = read_f32(bytes, offset + 12)?;
    let scale = read_f32(bytes, offset + 16)?;
    let alpha = read_f32(bytes, offset + 20)?;
    let atlas = read_u32(bytes, offset)?;
    Ok(
        json!({"eid":format!("sprite-{index}"),"atlasEntry":atlas,"deadHawgEntry":u64::from(atlas)+114,"pos":point(bytes,offset+4)?,"rotationDeg":rotation,"scale":scale,"alpha":alpha,"s0":rotation,"s1":scale,"s2":alpha,"flags":bytes[offset+24],"raw":STANDARD.encode(&bytes[offset..offset+25])}),
    )
}
fn opaque(id: u32, chunk: &NativeChunkNode, index: usize) -> Value {
    let mut item = json!({"typeId":id,"index":index,"raw":raw(chunk)});
    if let Some(name) = type_name(id) {
        item["typeName"] = name.into();
    }
    item
}
struct Payload<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> Payload<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }
    fn byte(&mut self) -> Result<u8, String> {
        let value = *self.bytes.get(self.offset).ok_or("truncated byte")?;
        self.offset += 1;
        Ok(value)
    }
    fn word(&mut self) -> Result<u32, String> {
        let v = read_u32(self.bytes, self.offset)?;
        self.offset += 4;
        Ok(v)
    }
    fn float(&mut self) -> Result<f64, String> {
        let v = read_f32(self.bytes, self.offset)?;
        self.offset += 4;
        Ok(v)
    }
    fn string(&mut self) -> Result<String, String> {
        let size = self.word()? as usize;
        let data = self
            .bytes
            .get(self.offset..self.offset.saturating_add(size))
            .ok_or("invalid native String")?;
        if data.is_empty() || data.last() != Some(&0) {
            return Err("invalid native String".into());
        }
        self.offset += size;
        let text = &data[..data.len() - 1];
        Ok(String::from_utf8_lossy(text.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(text))
            .into_owned())
    }
    fn rectangle(&mut self) -> Result<Value, String> {
        let left = self.float()?;
        let top = self.float()?;
        let right = self.float()?;
        let bottom = self.float()?;
        Ok(bounds_value(left, top, right, bottom))
    }
    fn words(&mut self, count: usize) -> Result<Vec<Value>, String> {
        if count > self.bytes.len().saturating_sub(self.offset) / 4 {
            return Err("truncated u32".into());
        }
        (0..count).map(|_| self.word().map(Value::from)).collect()
    }
}
fn monster(chunk: &NativeChunkNode, index: usize) -> Value {
    let fallback = opaque(6001, chunk, index);
    let decode = || -> Result<Value, String> {
        let mut p = Payload::new(&chunk.payload);
        let mut item = fallback.clone();
        item["enemyType"] = p.word()?.into();
        item["name"] = p.string()?.into();
        item["uid"] = p.word()?.into();
        for key in ["maxHp", "primaryDamage", "chaseSpeed", "moveSpeedScale"] {
            item[key] = p.float()?.into();
        }
        for key in ["variantMode", "projectileMode", "auraMode"] {
            item[key] = p.word()?.into();
        }
        for key in ["headgearMode", "unknown81", "unknown82", "randomVariant"] {
            item[key] = p.byte()?.into();
        }
        item["archetype"] = p.string()?.into();
        let linked = p.byte()?;
        item["hasLinkedUidByte"] = linked.into();
        item["hasLinkedUid"] = (linked != 0).into();
        for key in ["linkedUid", "behaviorCount", "behaviorMin", "behaviorMax"] {
            item[key] = p.word()?.into();
        }
        let flanking = p.byte()?;
        item["flankingByte"] = flanking.into();
        item["flanking"] = (flanking != 0).into();
        for key in [
            "pathfindingMode",
            "dropOrbs",
            "dropPowerups",
            "dropItems",
            "dropSpecificItems",
            "dropGold",
            "dropPotions",
            "specialSpawnMode",
        ] {
            item[key] = p.byte()?.into();
        }
        for key in ["attackSpeed", "xpBonus", "secondaryDamage"] {
            item[key] = p.float()?.into();
        }
        for (key, raw) in [
            ("shield", "shieldByte"),
            ("shieldOthers", "shieldOthersByte"),
            ("unknown96", "unknown96Byte"),
            ("burning", "burningByte"),
        ] {
            let byte = p.byte()?;
            item[raw] = byte.into();
            item[key] = (byte != 0).into();
        }
        for key in ["tertiaryDamage", "extraDamage"] {
            item[key] = p.float()?.into();
        }
        item["behaviorTimer"] = p.word()?.into();
        item["rect98"] = p.rectangle()?;
        item["rectA8"] = p.rectangle()?;
        item["castMode"] = p.byte()?.into();
        if p.offset != chunk.payload.len() {
            return Err("opaque MonsterRecipe".into());
        }
        Ok(item)
    };
    decode().unwrap_or(fallback)
}
fn uid_group(chunk: &NativeChunkNode, index: usize) -> Value {
    let fallback = opaque(6002, chunk, index);
    let decode = || -> Result<Value, String> {
        let mut p = Payload::new(&chunk.payload);
        let mut item = fallback.clone();
        item["name"] = p.string()?.into();
        item["uid"] = p.word()?.into();
        let count = p.word()? as usize;
        item["memberUids"] = p.words(count)?.into();
        item["fields58"] = p.words(3)?.into();
        item["field34"] = p.word()?.into();
        if p.offset != chunk.payload.len() {
            return Err("opaque UIDGroup".into());
        }
        Ok(item)
    };
    decode().unwrap_or(fallback)
}
fn timeline(id: u32, chunk: &NativeChunkNode, index: usize) -> Value {
    let fallback = opaque(id, chunk, index);
    if id != 6006 {
        return fallback;
    }
    let decode = || -> Result<Value, String> {
        let mut p = Payload::new(&chunk.payload);
        let mut item = fallback.clone();
        item["name"] = p.string()?.into();
        let uid = p.word()?;
        item["uid"] = uid.into();
        item["enabled"] = (p.byte()? != 0).into();
        let count = p.word()? as usize;
        item["eventCount"] = count.into();
        item["eventTypeIds"] = p.words(count)?.into();
        let mut reserved = vec![Value::from(uid)];
        for child in chunk.children.iter().take(count).filter(|c| c.payload.len() >= 4) {
            reserved.push(read_u32(&child.payload, 0)?.into());
        }
        item["reservedUids"] = reserved.into();
        Ok(item)
    };
    decode().unwrap_or(fallback)
}
fn recipes(
    section: &NativeChunkNode,
    decode: impl Fn(u32, &NativeChunkNode, usize) -> Value,
) -> Result<Vec<Value>, String> {
    let (ids, children) = manager(section)?;
    if ids.len() != children.len() {
        return Err("manager child count mismatch".into());
    }
    Ok(ids.into_iter().zip(children).enumerate().map(|(i, (id, c))| decode(id, c, i)).collect())
}
