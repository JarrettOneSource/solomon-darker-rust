//! Package asset preparation from `modding/assets/mod-asset-compiler.ts`.
//! Inputs are accepted package bytes and metadata; this module performs no I/O.

use std::collections::BTreeMap;

use sha2::{Digest, Sha256};
use solomon_sim::modding::assets::*;
use solomon_sim::modding::definition::{
    CompiledWebLuaAsset, CompiledWebLuaMod, WebLuaAssetKind, WebLuaModIdentity,
};
use solomon_sim::modding::{LuaObject, LuaValue};

use crate::json::SourceCollator;

const MAXIMUM_AUDIO_BYTES: usize = 16 * 1_024 * 1_024;
const MAXIMUM_FRAME_COUNT: usize = 4_096;
const MAXIMUM_IMAGE_DIMENSION: u32 = 4_096;
const MAXIMUM_SCENE_NODES: usize = 16_384;
const PNG_SIGNATURE: [u8; 8] = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];

#[derive(Clone, Debug, serde::Deserialize)]
pub struct ModAssetSource {
    pub files: BTreeMap<String, Vec<u8>>,
    pub identity: WebLuaModIdentity,
}

/// Immutable prepared rows. Native callers retain one catalog; the client
/// receives the same plain descriptors through its admitted content projection.
pub struct PreparedModAssetCatalog {
    assets: Vec<PreparedModAsset>,
    by_id: BTreeMap<String, usize>,
    first_animations: BTreeMap<String, String>,
}

impl PreparedModAssetCatalog {
    pub fn new(mut assets: Vec<PreparedModAsset>) -> Result<Self, String> {
        let collator = SourceCollator::default();
        assets.sort_by(|left, right| collator.compare(&left.identity().id, &right.identity().id));
        let mut by_id = BTreeMap::new();
        for (index, asset) in assets.iter().enumerate() {
            if by_id.insert(asset.identity().id.clone(), index).is_some() {
                return Err(format!("prepared mod asset is duplicated: {}", asset.identity().id));
            }
        }
        Ok(Self { assets, by_id, first_animations: BTreeMap::new() })
    }

    pub fn all(&self) -> &[PreparedModAsset] {
        &self.assets
    }

    pub fn get(&self, mod_id: &str, key: &str) -> Option<&PreparedModAsset> {
        let id = format!("{mod_id}:{key}");
        self.by_id.get(&id).map(|&index| &self.assets[index])
    }

    pub fn image(&self, mod_id: &str, key: &str) -> Result<&PreparedModSpriteAsset, String> {
        match self.get(mod_id, key) {
            Some(PreparedModAsset::Image(image)) => Ok(image),
            _ => Err(format!("mod image asset is unavailable: {mod_id}:{key}")),
        }
    }

    /// Source uses the first authored animation when no named animation fits.
    /// Its insertion order comes from the accepted definition, not map sorting.
    pub fn first_animation(&self, image: &PreparedModSpriteAsset) -> Option<&[usize]> {
        let key = self.first_animations.get(&image.identity.id)?;
        self.get(&image.identity.mod_id, &image.identity.key).and_then(|asset| match asset {
            PreparedModAsset::Image(image) => image.animations.get(key).map(Vec::as_slice),
            _ => None,
        })
    }
}

pub fn compile_mod_assets(
    assets: &[GameModAsset],
    mods: &[CompiledWebLuaMod],
    sources: &[ModAssetSource],
) -> Result<PreparedModAssetCatalog, String> {
    let mut by_source = BTreeMap::new();
    for source in sources {
        if by_source.insert(source.identity.id.to_lowercase(), source).is_some() {
            return Err(format!("asset source is duplicated: {}", source.identity.id));
        }
    }
    let mut metadata = BTreeMap::new();
    for asset in assets {
        let id = (asset.mod_id.to_lowercase(), asset.path.to_lowercase());
        if metadata.insert(id, asset).is_some() {
            return Err(format!(
                "mod package asset metadata is duplicated: {}:{}",
                asset.mod_id, asset.path
            ));
        }
    }
    let mut prepared = Vec::new();
    for compiled in mods {
        let source = by_source.get(&compiled.identity.id.to_lowercase()).ok_or_else(|| {
            format!("compiled mod has no package asset source: {}", compiled.identity.id)
        })?;
        for definition in &compiled.assets {
            prepared.push(compile_asset(compiled, definition, source, &metadata)?);
        }
    }
    let mut catalog = PreparedModAssetCatalog::new(prepared)?;
    for compiled in mods {
        for definition in &compiled.assets {
            if let Some(animations) =
                definition.fields.get("animations").and_then(LuaValue::as_object)
                && let Some((name, _)) = animations.iter().next()
            {
                catalog
                    .first_animations
                    .insert(format!("{}:{}", compiled.identity.id, definition.key), name.into());
            }
        }
    }
    Ok(catalog)
}

fn compile_asset(
    compiled: &CompiledWebLuaMod,
    definition: &CompiledWebLuaAsset,
    source: &ModAssetSource,
    metadata: &BTreeMap<(String, String), &GameModAsset>,
) -> Result<PreparedModAsset, String> {
    let path = asset_path(definition)?;
    let field = format!("{}:{}", compiled.identity.id, definition.key);
    let missing = || {
        format!("declared mod asset is absent from its package: {}:{path}", compiled.identity.id)
    };
    let bytes = source.files.get(path).ok_or_else(missing)?;
    let descriptor = metadata
        .get(&(compiled.identity.id.to_lowercase(), path.to_lowercase()))
        .ok_or_else(missing)?;
    if bytes.len() as u64 != descriptor.byte_length {
        return Err(format!("mod asset byte length changed: {}:{path}", compiled.identity.id));
    }
    if sha256(bytes) != descriptor.sha256 {
        return Err(format!("mod asset digest changed: {}:{path}", compiled.identity.id));
    }
    let identity = PreparedModAssetIdentity {
        id: field.clone(),
        key: definition.key.clone(),
        mod_id: compiled.identity.id.clone(),
        path: path.to_owned(),
        sha256: descriptor.sha256.clone(),
    };
    let fields = &definition.fields;
    match definition.asset_kind {
        WebLuaAssetKind::Sprite | WebLuaAssetKind::Sheet => {
            require_file_kind(descriptor, "image", "image/png", &field)?;
            let (width, height) = png_dimensions(bytes, &field)?;
            let (frames, heading_count, animations) =
                if definition.asset_kind == WebLuaAssetKind::Sheet {
                    let frames = sheet_frames(fields, width, height, &field)?;
                    let headings = sheet_heading_count(fields, width, &field)?;
                    let animations =
                        sheet_animations(fields.get("animations"), frames.len(), headings, &field)?;
                    (frames, headings, animations)
                } else {
                    (sprite_frames(fields, width, height, &field)?, None, BTreeMap::new())
                };
            Ok(PreparedModAsset::Image(PreparedModSpriteAsset {
                identity,
                animations,
                asset_kind: definition.asset_kind,
                frames,
                height,
                heading_count,
                width,
            }))
        }
        WebLuaAssetKind::Music | WebLuaAssetKind::Sound => {
            require_audio(descriptor, bytes, &field)?;
            let expected_bus =
                if definition.asset_kind == WebLuaAssetKind::Music { "music" } else { "effects" };
            let bus = match fields.get("bus") {
                None => expected_bus,
                Some(LuaValue::String(value))
                    if !value.is_empty() && value.encode_utf16().count() <= 64 =>
                {
                    value
                }
                _ => return Err(format!("{field}.bus is invalid")),
            };
            if bus != expected_bus {
                return Err(format!("{field}.bus must be {expected_bus}"));
            }
            let loop_audio = if definition.asset_kind == WebLuaAssetKind::Music {
                match fields.get("loop") {
                    None => true,
                    Some(LuaValue::Boolean(value)) => *value,
                    _ => return Err(format!("{field}.loop must be boolean")),
                }
            } else {
                false
            };
            Ok(PreparedModAsset::Audio(PreparedModAudioAsset {
                identity,
                asset_kind: definition.asset_kind,
                bus: bus.to_owned(),
                content_type: descriptor.content_type.clone(),
                loop_audio,
                volume: optional_number(
                    fields.get("volume"),
                    1.0,
                    0.0,
                    1.0,
                    &format!("{field}.volume"),
                )?,
            }))
        }
        WebLuaAssetKind::Scene => {
            require_file_kind(descriptor, "scene", "application/json", &field)?;
            validate_json_document(bytes, &field)?;
            Ok(PreparedModAsset::Document(PreparedModDocumentAsset {
                identity,
                asset_kind: definition.asset_kind,
                content_type: descriptor.content_type.clone(),
            }))
        }
        WebLuaAssetKind::Boneyard => {
            require_file_kind(
                descriptor,
                "boneyard",
                "application/vnd.solomon-dark.boneyard",
                &field,
            )?;
            if bytes.is_empty() {
                return Err(format!("{field} Boneyard asset is empty"));
            }
            Ok(PreparedModAsset::Document(PreparedModDocumentAsset {
                identity,
                asset_kind: definition.asset_kind,
                content_type: descriptor.content_type.clone(),
            }))
        }
    }
}

pub fn sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write;
        write!(output, "{byte:02x}").expect("writing to a String cannot fail");
    }
    output
}

fn asset_path(definition: &CompiledWebLuaAsset) -> Result<&str, String> {
    let property = if definition.asset_kind == WebLuaAssetKind::Sheet { "image" } else { "path" };
    if let Some(path) = definition.fields.get(property).and_then(LuaValue::as_str)
        && valid_package_asset_path(path)
    {
        return Ok(path);
    }
    Err(format!(
        "{} asset {} has an invalid package path",
        definition.asset_kind.as_str(),
        definition.key
    ))
}

fn valid_package_asset_path(path: &str) -> bool {
    if path.contains(['\0', '\n', '\r', '\u{2028}', '\u{2029}']) {
        return false;
    }
    let Some((directory, remainder)) = path.split_once('/') else { return false };
    matches!(directory, "art" | "audio" | "levels" | "scenes" | "sprites")
        && !remainder.is_empty()
        // The source lookahead starts after the directory slash. Its `^`
        // alternative still means the whole string's start, so it rejects dot
        // segments only after a later slash. A first dot segment reaches the
        // package-map lookup and reports an absent file instead.
        && !remainder.split('/').skip(1).any(|segment| matches!(segment, "." | ".."))
}

fn require_file_kind(
    asset: &GameModAsset,
    kind: &str,
    content_type: &str,
    field: &str,
) -> Result<(), String> {
    if asset.kind != kind || asset.content_type != content_type {
        return Err(format!("{field} requires {kind} {content_type} package data"));
    }
    Ok(())
}

fn require_audio(asset: &GameModAsset, bytes: &[u8], field: &str) -> Result<(), String> {
    if asset.kind != "audio" || bytes.len() > MAXIMUM_AUDIO_BYTES {
        return Err(format!("{field} requires bounded audio package data"));
    }
    let valid = match asset.content_type.as_str() {
        "audio/wav" => bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WAVE"),
        "audio/ogg" => bytes.starts_with(b"OggS"),
        "audio/mpeg" => {
            bytes.starts_with(b"ID3")
                || bytes.first() == Some(&0xff)
                    && bytes.get(1).is_some_and(|byte| byte & 0xe0 == 0xe0)
        }
        _ => false,
    };
    if !valid {
        return Err(format!("{field} audio bytes do not match {}", asset.content_type));
    }
    Ok(())
}

fn png_dimensions(bytes: &[u8], field: &str) -> Result<(u32, u32), String> {
    if bytes.len() < 24 || !bytes.starts_with(&PNG_SIGNATURE) {
        return Err(format!("{field} is not a PNG image"));
    }
    if bytes[8..12] != 13_u32.to_be_bytes() || &bytes[12..16] != b"IHDR" {
        return Err(format!("{field} PNG has no leading IHDR"));
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into().unwrap());
    let height = u32::from_be_bytes(bytes[20..24].try_into().unwrap());
    if !(1..=MAXIMUM_IMAGE_DIMENSION).contains(&width)
        || !(1..=MAXIMUM_IMAGE_DIMENSION).contains(&height)
    {
        return Err(format!("{field} PNG dimensions must be within 1..{MAXIMUM_IMAGE_DIMENSION}"));
    }
    Ok((width, height))
}

fn sprite_frames(
    fields: &LuaObject,
    width: u32,
    height: u32,
    field: &str,
) -> Result<Vec<ModSpriteFrame>, String> {
    if let Some(frames) = fields.get("frames") {
        let frames = frames.as_array().ok_or_else(|| {
            format!("{field}.frames must contain 1..{MAXIMUM_FRAME_COUNT} frame rectangles")
        })?;
        if frames.is_empty() || frames.len() > MAXIMUM_FRAME_COUNT {
            return Err(format!(
                "{field}.frames must contain 1..{MAXIMUM_FRAME_COUNT} frame rectangles"
            ));
        }
        let mut result = Vec::new();
        for (index, value) in frames.iter().enumerate() {
            result.push(frame(value, width, height, &format!("{field}.frames[{index}]"))?);
        }
        return Ok(result);
    }
    match fields.get("frame") {
        None => Ok(vec![full_frame(width, height, 0, 0)]),
        Some(value) => Ok(vec![frame(value, width, height, &format!("{field}.frame"))?]),
    }
}

fn sheet_frames(
    fields: &LuaObject,
    image_width: u32,
    image_height: u32,
    field: &str,
) -> Result<Vec<ModSpriteFrame>, String> {
    let spec = object(fields.get("frame"), &format!("{field}.frame"))?;
    exact_keys(spec, &["height", "width"], &format!("{field}.frame"))?;
    let width = integer(spec.get("width"), 1, image_width, &format!("{field}.frame.width"))?;
    let height = integer(spec.get("height"), 1, image_height, &format!("{field}.frame.height"))?;
    if !image_width.is_multiple_of(width) || !image_height.is_multiple_of(height) {
        return Err(format!("{field} sheet dimensions must be exact multiples of its frame size"));
    }
    let columns = image_width / width;
    let rows = image_height / height;
    if u64::from(columns) * u64::from(rows) > MAXIMUM_FRAME_COUNT as u64 {
        return Err(format!("{field} sheet exceeds its frame limit"));
    }
    let mut result = Vec::new();
    for index in 0..columns * rows {
        result.push(full_frame(width, height, index % columns * width, index / columns * height));
    }
    Ok(result)
}

fn sheet_heading_count(
    fields: &LuaObject,
    image_width: u32,
    field: &str,
) -> Result<Option<u32>, String> {
    let Some(value) = fields.get("headings") else { return Ok(None) };
    let frame = object(fields.get("frame"), &format!("{field}.frame"))?;
    let width = integer(frame.get("width"), 1, image_width, &format!("{field}.frame.width"))?;
    let headings = integer(Some(value), 1, 24, &format!("{field}.headings"))?;
    if headings != image_width / width {
        return Err(format!("{field}.headings must match its sheet columns"));
    }
    Ok(Some(headings))
}

fn sheet_animations(
    value: Option<&LuaValue>,
    frame_count: usize,
    headings: Option<u32>,
    field: &str,
) -> Result<BTreeMap<String, Vec<usize>>, String> {
    let source = object(value, &format!("{field}.animations"))?;
    let mut result = BTreeMap::new();
    for (name, frames) in source.iter() {
        let valid_name = name.len() <= 64
            && name.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
            && name.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
            });
        let frames = frames.as_array().filter(|frames| !frames.is_empty());
        if !valid_name || frames.is_none() {
            return Err(format!("{field}.animations.{name} is invalid"));
        }
        let maximum = frame_count as u32 / headings.unwrap_or(1);
        let mut indices = Vec::new();
        for (index, candidate) in frames.unwrap().iter().enumerate() {
            let frame = integer(
                Some(candidate),
                1,
                maximum,
                &format!("{field}.animations.{name}[{index}]"),
            )?;
            indices.push(((frame - 1) * headings.unwrap_or(1)) as usize);
        }
        result.insert(name.to_owned(), indices);
    }
    if result.is_empty() {
        return Err(format!("{field} sheet requires at least one animation"));
    }
    Ok(result)
}

fn frame(
    value: &LuaValue,
    image_width: u32,
    image_height: u32,
    field: &str,
) -> Result<ModSpriteFrame, String> {
    let spec = object(Some(value), field)?;
    exact_keys(
        spec,
        &[
            "center_x",
            "center_y",
            "content_height",
            "content_width",
            "height",
            "logical_height",
            "logical_width",
            "width",
            "x",
            "y",
        ],
        field,
    )?;
    let zero = LuaValue::Number(0.0);
    let x = integer(
        spec.get("x").filter(|value| **value != LuaValue::Null).or(Some(&zero)),
        0,
        image_width - 1,
        &format!("{field}.x"),
    )?;
    let y = integer(
        spec.get("y").filter(|value| **value != LuaValue::Null).or(Some(&zero)),
        0,
        image_height - 1,
        &format!("{field}.y"),
    )?;
    let width = integer(spec.get("width"), 1, image_width, &format!("{field}.width"))?;
    let height = integer(spec.get("height"), 1, image_height, &format!("{field}.height"))?;
    if x + width > image_width || y + height > image_height {
        return Err(format!("{field} extends beyond its image"));
    }
    let default_width = LuaValue::Number(f64::from(width));
    let default_height = LuaValue::Number(f64::from(height));
    let logical_width = integer(
        spec.get("logical_width")
            .filter(|value| **value != LuaValue::Null)
            .or(Some(&default_width)),
        width,
        MAXIMUM_IMAGE_DIMENSION,
        &format!("{field}.logical_width"),
    )?;
    let logical_height = integer(
        spec.get("logical_height")
            .filter(|value| **value != LuaValue::Null)
            .or(Some(&default_height)),
        height,
        MAXIMUM_IMAGE_DIMENSION,
        &format!("{field}.logical_height"),
    )?;
    Ok(ModSpriteFrame {
        center_offset_x: optional_number(
            spec.get("center_x"),
            0.0,
            -f64::from(MAXIMUM_IMAGE_DIMENSION),
            f64::from(MAXIMUM_IMAGE_DIMENSION),
            &format!("{field}.center_x"),
        )?,
        center_offset_y: optional_number(
            spec.get("center_y"),
            0.0,
            -f64::from(MAXIMUM_IMAGE_DIMENSION),
            f64::from(MAXIMUM_IMAGE_DIMENSION),
            &format!("{field}.center_y"),
        )?,
        content_height: optional_number(
            spec.get("content_height"),
            f64::from(height),
            0.0,
            f64::from(logical_height),
            &format!("{field}.content_height"),
        )?,
        content_width: optional_number(
            spec.get("content_width"),
            f64::from(width),
            0.0,
            f64::from(logical_width),
            &format!("{field}.content_width"),
        )?,
        height: f64::from(height),
        logical_height: f64::from(logical_height),
        logical_width: f64::from(logical_width),
        width: f64::from(width),
        x: f64::from(x),
        y: f64::from(y),
    })
}

fn full_frame(width: u32, height: u32, x: u32, y: u32) -> ModSpriteFrame {
    ModSpriteFrame {
        center_offset_x: 0.0,
        center_offset_y: 0.0,
        content_height: f64::from(height),
        content_width: f64::from(width),
        height: f64::from(height),
        logical_height: f64::from(height),
        logical_width: f64::from(width),
        width: f64::from(width),
        x: f64::from(x),
        y: f64::from(y),
    }
}

fn validate_json_document(bytes: &[u8], field: &str) -> Result<(), String> {
    let value: LuaValue =
        serde_json::from_slice(bytes).map_err(|_| format!("{field} is not valid UTF-8 JSON"))?;
    let document =
        value.as_object().ok_or_else(|| format!("{field} scene document must be an object"))?;
    exact_keys(document, &["name", "rooms", "schemaVersion"], field)?;
    let rooms = document.get("rooms").and_then(LuaValue::as_array);
    if document.get("schemaVersion").and_then(LuaValue::as_number) != Some(1.0)
        || !document.get("name").and_then(LuaValue::as_str).is_some_and(|name| !name.is_empty())
        || !rooms.is_some_and(|rooms| {
            !rooms.is_empty()
                && rooms.iter().all(|room| room.as_str().is_some_and(|name| !name.is_empty()))
        })
    {
        return Err(format!(
            "{field} scene document requires schemaVersion 1, name, and room keys"
        ));
    }
    let mut nodes = 0;
    validate_scene_nodes(&value, 0, &mut nodes, field)
}

fn validate_scene_nodes(
    value: &LuaValue,
    depth: usize,
    nodes: &mut usize,
    field: &str,
) -> Result<(), String> {
    *nodes += 1;
    if *nodes > MAXIMUM_SCENE_NODES || depth > 32 {
        return Err(format!("{field} scene document exceeds its graph budget"));
    }
    match value {
        LuaValue::Array(values) => {
            for value in values {
                validate_scene_nodes(value, depth + 1, nodes, field)?;
            }
        }
        LuaValue::Object(values) => {
            for (_, value) in values.iter() {
                validate_scene_nodes(value, depth + 1, nodes, field)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn exact_keys(source: &LuaObject, allowed: &[&str], field: &str) -> Result<(), String> {
    let unknown: Vec<_> =
        source.iter().map(|(key, _)| key).filter(|key| !allowed.contains(key)).collect();
    if !unknown.is_empty() {
        return Err(format!("{field} contains unknown fields: {}", unknown.join(", ")));
    }
    Ok(())
}

fn object<'a>(value: Option<&'a LuaValue>, field: &str) -> Result<&'a LuaObject, String> {
    value.and_then(LuaValue::as_object).ok_or_else(|| format!("{field} must be an object"))
}

fn integer(
    value: Option<&LuaValue>,
    minimum: u32,
    maximum: u32,
    field: &str,
) -> Result<u32, String> {
    if let Some(value) = value.and_then(LuaValue::as_number)
        && value.is_finite()
        && value.fract() == 0.0
        && value >= f64::from(minimum)
        && value <= f64::from(maximum)
    {
        return Ok(value as u32);
    }
    Err(format!("{field} must be an integer within {minimum}..{maximum}"))
}

fn optional_number(
    value: Option<&LuaValue>,
    fallback: f64,
    minimum: f64,
    maximum: f64,
    field: &str,
) -> Result<f64, String> {
    let Some(value) = value else { return Ok(fallback) };
    if let Some(value) = value.as_number()
        && value.is_finite()
        && value >= minimum
        && value <= maximum
    {
        return Ok(value);
    }
    Err(format!("{field} must be finite within {minimum}..{maximum}"))
}
