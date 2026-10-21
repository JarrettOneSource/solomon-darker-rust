//! Launcher ZIP framing and manifest contract (`save/native-save-archive.ts`).

use std::collections::{BTreeMap, BTreeSet};

use miniz_oxide::inflate::{TINFLStatus, decompress_to_vec_with_limit};
use serde::Serialize;
use serde_json::Value;

use super::codec::utf8;
use super::sha256;

const MAX_ARCHIVE_BYTES: usize = 16 * 1024 * 1024;
const MAX_EXPANDED_BYTES: u64 = 64 * 1024 * 1024;
const MAX_FILES: usize = 256;
const MAX_MANIFEST_BYTES: usize = 128 * 1024;
const ZIP_LOCAL_FILE: u32 = 0x0403_4b50;
const ZIP_CENTRAL_FILE: u32 = 0x0201_4b50;
const ZIP_END: u32 = 0x0605_4b50;
const ZIP_UTF8: u16 = 0x0800;

pub const WEB_GAME_SAVE_SUPPORT_ARCHIVE_PATH: &str = "solomondark/browser-game-save.json";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeArchiveFile {
    pub bytes: Vec<u8>,
    pub path: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeSaveArchive {
    pub darkdata: Vec<u8>,
    pub gamestate: Vec<u8>,
    pub retained_files: Vec<NativeArchiveFile>,
    pub run_name: String,
}

struct ZipEntry {
    compressed_size: usize,
    compression: u16,
    crc32: u32,
    flags: u16,
    local_offset: usize,
    path: String,
    uncompressed_size: usize,
}

fn range<'a>(bytes: &'a [u8], offset: usize, size: usize, claim: &str) -> Result<&'a [u8], String> {
    let end = offset
        .checked_add(size)
        .ok_or_else(|| format!("native save archive has truncated {claim}"))?;
    bytes.get(offset..end).ok_or_else(|| format!("native save archive has truncated {claim}"))
}

fn u16_at(bytes: &[u8], offset: usize, claim: &str) -> Result<u16, String> {
    let raw = range(bytes, offset, 2, claim)?;
    Ok(u16::from_le_bytes(raw.try_into().expect("two ZIP bytes were checked")))
}

fn u32_at(bytes: &[u8], offset: usize, claim: &str) -> Result<u32, String> {
    let raw = range(bytes, offset, 4, claim)?;
    Ok(u32::from_le_bytes(raw.try_into().expect("four ZIP bytes were checked")))
}

fn write_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

const fn crc_table() -> [u32; 256] {
    let mut table = [0; 256];
    let mut index = 0;
    while index < table.len() {
        let mut crc = index as u32;
        let mut bit = 0;
        while bit < 8 {
            crc = if crc & 1 != 0 { 0xedb8_8320 ^ (crc >> 1) } else { crc >> 1 };
            bit += 1;
        }
        table[index] = crc;
        index += 1;
    }
    table
}

#[must_use]
pub fn native_archive_crc32(bytes: &[u8]) -> u32 {
    const TABLE: [u32; 256] = crc_table();
    let mut crc = u32::MAX;
    for &value in bytes {
        crc = TABLE[((crc ^ u32::from(value)) & 0xff) as usize] ^ (crc >> 8);
    }
    crc ^ u32::MAX
}

pub(super) fn validate_path(path: &str) -> Result<(), String> {
    if path.is_empty()
        || path.encode_utf16().count() > 512
        || path.contains(['\\', ':'])
        || path.starts_with('/')
        || path.ends_with('/')
        || path.split('/').any(|part| part.is_empty() || part == "." || part == "..")
        || path.chars().any(|value| value <= '\u{1f}' || ('\u{7f}'..='\u{9f}').contains(&value))
    {
        let quoted = serde_json::to_string(path).expect("a ZIP path is a JSON string");
        return Err(format!("native save archive path is unsafe: {quoted}"));
    }
    Ok(())
}

pub(super) fn retained_path_is_safe(path: &str) -> bool {
    if validate_path(path).is_err() {
        return false;
    }
    let canonical = path.to_lowercase();
    canonical.starts_with("solomondark/")
        && canonical != "solomondark/darkdata.cfg"
        && canonical != "solomondark/settings.txt"
        && !any_gamestate_path(path)
}

fn validate_retained_path(path: &str) -> Result<(), String> {
    validate_path(path)?;
    if !retained_path_is_safe(path) {
        return Err(format!("native retained file path is invalid: {path}"));
    }
    Ok(())
}

fn any_gamestate_path(path: &str) -> bool {
    path.to_ascii_lowercase()
        .strip_prefix("solomondark/savegames/")
        .and_then(|suffix| suffix.strip_suffix("/gamestate.sav"))
        .is_some_and(|middle| !middle.is_empty())
}

pub(super) fn run_name_is_valid(name: &str) -> bool {
    (1..=64).contains(&name.len())
        && name.bytes().all(|value| value.is_ascii_alphanumeric() || b"._-".contains(&value))
}

pub fn create_stored_zip(files: &[NativeArchiveFile]) -> Result<Vec<u8>, String> {
    if files.is_empty() || files.len() > MAX_FILES {
        return Err("native save archive file count is invalid".into());
    }
    let mut seen = BTreeSet::new();
    let mut local_parts = Vec::new();
    let mut central_parts = Vec::new();
    for file in files {
        validate_path(&file.path)?;
        if !seen.insert(file.path.to_lowercase()) {
            return Err("native save archive has duplicate paths".into());
        }
        let name = file.path.as_bytes();
        let crc = native_archive_crc32(&file.bytes);
        let mut local = vec![0; 30 + name.len()];
        write_u32(&mut local, 0, ZIP_LOCAL_FILE);
        write_u16(&mut local, 4, 20);
        write_u16(&mut local, 6, ZIP_UTF8);
        // Stable source DOS date: 2000-01-01 at midnight.
        write_u16(&mut local, 12, 10_273);
        write_u32(&mut local, 14, crc);
        write_u32(&mut local, 18, file.bytes.len() as u32);
        write_u32(&mut local, 22, file.bytes.len() as u32);
        write_u16(&mut local, 26, name.len() as u16);
        local[30..].copy_from_slice(name);
        let mut central = vec![0; 46 + name.len()];
        write_u32(&mut central, 0, ZIP_CENTRAL_FILE);
        write_u16(&mut central, 4, 20);
        write_u16(&mut central, 6, 20);
        write_u16(&mut central, 8, ZIP_UTF8);
        write_u16(&mut central, 14, 10_273);
        write_u32(&mut central, 16, crc);
        write_u32(&mut central, 20, file.bytes.len() as u32);
        write_u32(&mut central, 24, file.bytes.len() as u32);
        write_u16(&mut central, 28, name.len() as u16);
        write_u32(&mut central, 42, local_parts.len() as u32);
        central[46..].copy_from_slice(name);
        local_parts.extend_from_slice(&local);
        local_parts.extend_from_slice(&file.bytes);
        central_parts.extend_from_slice(&central);
    }
    let mut end = [0; 22];
    write_u32(&mut end, 0, ZIP_END);
    write_u16(&mut end, 8, files.len() as u16);
    write_u16(&mut end, 10, files.len() as u16);
    write_u32(&mut end, 12, central_parts.len() as u32);
    write_u32(&mut end, 16, local_parts.len() as u32);
    local_parts.extend_from_slice(&central_parts);
    local_parts.extend_from_slice(&end);
    if local_parts.len() > MAX_ARCHIVE_BYTES {
        return Err("native save archive exceeds 16 MiB".into());
    }
    Ok(local_parts)
}

fn find_end(bytes: &[u8]) -> Result<usize, String> {
    if bytes.len() >= 22 {
        let start = bytes.len().saturating_sub(65_557);
        for offset in (start..=bytes.len() - 22).rev() {
            if u32_at(bytes, offset, "ZIP end signature")? == ZIP_END {
                let comment = usize::from(u16_at(bytes, offset + 20, "ZIP comment length")?);
                if offset + 22 + comment == bytes.len() {
                    return Ok(offset);
                }
            }
        }
    }
    Err("native save archive has no ZIP end record".into())
}

fn central_directory(bytes: &[u8]) -> Result<Vec<ZipEntry>, String> {
    if bytes.is_empty() || bytes.len() > MAX_ARCHIVE_BYTES {
        return Err("native save archive size is invalid".into());
    }
    let end = find_end(bytes)?;
    if u16_at(bytes, end + 4, "ZIP disk")? != 0 || u16_at(bytes, end + 6, "ZIP central disk")? != 0
    {
        return Err("multi-disk native save archives are unsupported".into());
    }
    let disk_count = usize::from(u16_at(bytes, end + 8, "ZIP disk entry count")?);
    let count = usize::from(u16_at(bytes, end + 10, "ZIP entry count")?);
    let size = u32_at(bytes, end + 12, "ZIP central size")? as usize;
    let start = u32_at(bytes, end + 16, "ZIP central offset")? as usize;
    if count == 0
        || count > MAX_FILES
        || disk_count != count
        || start.checked_add(size) != Some(end)
    {
        return Err("native save archive central directory is invalid".into());
    }
    let mut entries = Vec::with_capacity(count);
    let mut seen = BTreeSet::new();
    let mut offset = start;
    let mut expanded = 0_u64;
    for index in 0..count {
        if u32_at(bytes, offset, &format!("central entry {index}"))? != ZIP_CENTRAL_FILE {
            return Err(format!("native save archive central entry {index} is invalid"));
        }
        let flags = u16_at(bytes, offset + 8, "ZIP flags")?;
        let compression = u16_at(bytes, offset + 10, "ZIP compression")?;
        let crc32 = u32_at(bytes, offset + 16, "ZIP CRC")?;
        let compressed_size = u32_at(bytes, offset + 20, "ZIP compressed size")? as usize;
        let uncompressed_size = u32_at(bytes, offset + 24, "ZIP uncompressed size")? as usize;
        let name_length = usize::from(u16_at(bytes, offset + 28, "ZIP name length")?);
        let extra_length = usize::from(u16_at(bytes, offset + 30, "ZIP extra length")?);
        let comment_length = usize::from(u16_at(bytes, offset + 32, "ZIP entry comment length")?);
        let disk = u16_at(bytes, offset + 34, "ZIP entry disk")?;
        let local_offset = u32_at(bytes, offset + 42, "ZIP local offset")? as usize;
        range(bytes, offset + 46, name_length + extra_length + comment_length, "ZIP entry")?;
        let path = utf8(&bytes[offset + 46..offset + 46 + name_length])
            .map_err(|_| "native save archive path is not UTF-8")?
            .to_owned();
        validate_path(&path)?;
        if !seen.insert(path.to_lowercase()) {
            return Err("native save archive has duplicate paths".into());
        }
        if flags & 1 != 0 || flags & !ZIP_UTF8 & !0x0008 != 0 || disk != 0 {
            return Err("native save archive uses unsupported ZIP features".into());
        }
        if compression != 0 && compression != 8 {
            return Err(format!("native save archive compression {compression} is unsupported"));
        }
        expanded += uncompressed_size as u64;
        if expanded > MAX_EXPANDED_BYTES {
            return Err("native save archive expands beyond 64 MiB".into());
        }
        entries.push(ZipEntry {
            compressed_size,
            compression,
            crc32,
            flags,
            local_offset,
            path,
            uncompressed_size,
        });
        offset += 46 + name_length + extra_length + comment_length;
    }
    if offset != end {
        return Err("native save archive central size drifted".into());
    }
    Ok(entries)
}

pub fn read_zip(bytes: &[u8]) -> Result<Vec<NativeArchiveFile>, String> {
    let entries = central_directory(bytes)?;
    let mut result = Vec::with_capacity(entries.len());
    for entry in entries {
        let offset = entry.local_offset;
        if u32_at(bytes, offset, "ZIP local signature")? != ZIP_LOCAL_FILE {
            return Err(format!("native save archive local record for {} is invalid", entry.path));
        }
        let flags = u16_at(bytes, offset + 6, "ZIP local flags")?;
        let compression = u16_at(bytes, offset + 8, "ZIP local compression")?;
        let local_crc = u32_at(bytes, offset + 14, "ZIP local CRC")?;
        let local_compressed = u32_at(bytes, offset + 18, "ZIP local compressed size")? as usize;
        let local_uncompressed =
            u32_at(bytes, offset + 22, "ZIP local uncompressed size")? as usize;
        let name_length = usize::from(u16_at(bytes, offset + 26, "ZIP local name length")?);
        let extra_length = usize::from(u16_at(bytes, offset + 28, "ZIP local extra length")?);
        let data_offset = offset + 30 + name_length + extra_length;
        // Central sizes are u32. Avoid wrapping their sum on WASM before the range check.
        range(bytes, offset + 30, name_length + extra_length, "ZIP local data")?;
        range(bytes, data_offset, entry.compressed_size, "ZIP local data")?;
        let path = utf8(&bytes[offset + 30..offset + 30 + name_length])
            .map_err(|_| "native save archive local path is not UTF-8")?;
        if path != entry.path || flags != entry.flags || compression != entry.compression {
            return Err(format!("native save archive local metadata drifted for {}", entry.path));
        }
        let descriptor = flags & 0x0008 != 0;
        if (!descriptor
            && (local_crc != entry.crc32
                || local_compressed != entry.compressed_size
                || local_uncompressed != entry.uncompressed_size))
            || (descriptor
                && ((local_crc != 0 && local_crc != entry.crc32)
                    || (local_compressed != 0 && local_compressed != entry.compressed_size)
                    || (local_uncompressed != 0 && local_uncompressed != entry.uncompressed_size)))
        {
            return Err(format!("native save archive local sizes drifted for {}", entry.path));
        }
        let compressed = range(bytes, data_offset, entry.compressed_size, "ZIP local data")?;
        let expanded = if compression == 0 {
            compressed.to_vec()
        } else {
            decompress_to_vec_with_limit(compressed, entry.uncompressed_size).map_err(|error| {
                if error.status == TINFLStatus::HasMoreOutput {
                    "native save archive entry exceeds its declared size".to_owned()
                } else {
                    "native save archive deflate stream is invalid".to_owned()
                }
            })?
        };
        if expanded.len() != entry.uncompressed_size
            || native_archive_crc32(&expanded) != entry.crc32
        {
            return Err(format!("native save archive integrity failed for {}", entry.path));
        }
        result.push(NativeArchiveFile { bytes: expanded, path: entry.path });
    }
    Ok(result)
}

#[derive(Serialize)]
struct ManifestFile {
    path: String,
    sha256: String,
    size: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    files: Vec<ManifestFile>,
    name: Option<String>,
    schema_version: u64,
    slot: u64,
}

fn parse_manifest(bytes: &[u8]) -> Result<Manifest, String> {
    if bytes.is_empty() || bytes.len() > MAX_MANIFEST_BYTES {
        return Err("native save manifest size is invalid".into());
    }
    let text = utf8(bytes).map_err(|_| "native save manifest is not valid UTF-8 JSON")?;
    let value: Value =
        serde_json::from_str(text).map_err(|_| "native save manifest is not valid UTF-8 JSON")?;
    let root = value.as_object().ok_or("native save manifest is invalid")?;
    let slot = root.get("slot").and_then(super::safe_unsigned);
    let name = root.get("name");
    let files = root.get("files").and_then(Value::as_array);
    if root.keys().any(|key| !["files", "name", "schemaVersion", "slot"].contains(&key.as_str()))
        || root.get("schemaVersion").and_then(Value::as_f64) != Some(1.0)
        || slot.is_none_or(|slot| slot > 7)
        || name.is_none_or(|name| {
            !name.is_null()
                && name.as_str().is_none_or(|name| {
                    name.encode_utf16().count() > 40
                        || name.chars().any(|value| {
                            value <= '\u{1f}' || ('\u{7f}'..='\u{9f}').contains(&value)
                        })
                })
        })
        || files.is_none_or(|files| files.is_empty() || files.len() > MAX_FILES - 1)
    {
        return Err("native save manifest contract is invalid".into());
    }
    let mut paths = BTreeSet::new();
    let mut result = Vec::new();
    for (index, file) in files.expect("manifest file membership was checked").iter().enumerate() {
        let root = file
            .as_object()
            .ok_or_else(|| format!("native save manifest file {index} is invalid"))?;
        let path = root.get("path").and_then(Value::as_str);
        let digest = root.get("sha256").and_then(Value::as_str);
        let size = root.get("size").and_then(super::safe_unsigned);
        if root.keys().any(|key| !["path", "sha256", "size"].contains(&key.as_str()))
            || path.is_none_or(|path| !path.to_lowercase().starts_with("solomondark/"))
            || digest.is_none_or(|digest| !super::sha256_is_valid(digest))
            || size.is_none_or(|size| size > 9_007_199_254_740_991)
        {
            return Err(format!("native save manifest file {index} is invalid"));
        }
        let path = path.expect("manifest path was checked");
        validate_path(path)?;
        if !paths.insert(path.to_lowercase()) {
            return Err("native save manifest has duplicate paths".into());
        }
        result.push(ManifestFile {
            path: path.to_owned(),
            sha256: digest.expect("manifest hash was checked").to_owned(),
            size: size.expect("manifest size was checked"),
        });
    }
    Ok(Manifest {
        files: result,
        name: name.and_then(Value::as_str).map(str::to_owned),
        schema_version: 1,
        slot: slot.expect("manifest slot was checked"),
    })
}

pub fn create_native_save_archive(save: &NativeSaveArchive, slot: u32) -> Result<Vec<u8>, String> {
    if slot > 7 {
        return Err("native save archive slot is invalid".into());
    }
    if !run_name_is_valid(&save.run_name) {
        return Err("native save archive run name is invalid".into());
    }
    for file in &save.retained_files {
        validate_retained_path(&file.path)?;
    }
    let mut files = vec![
        NativeArchiveFile { bytes: save.darkdata.clone(), path: "solomondark/darkdata.cfg".into() },
        NativeArchiveFile {
            bytes: save.gamestate.clone(),
            path: format!("solomondark/savegames/{}/gamestate.sav", save.run_name),
        },
    ];
    files.extend(save.retained_files.iter().cloned());
    let manifest = Manifest {
        files: files
            .iter()
            .map(|file| ManifestFile {
                path: file.path.clone(),
                sha256: sha256(&file.bytes),
                size: file.bytes.len() as u64,
            })
            .collect(),
        name: Some("Browser Game".into()),
        schema_version: 1,
        slot: u64::from(slot),
    };
    let bytes = serde_json::to_vec(&manifest).map_err(|error| error.to_string())?;
    let mut zip = vec![NativeArchiveFile { bytes, path: "manifest.json".into() }];
    zip.extend(files.into_iter().map(|file| NativeArchiveFile {
        bytes: file.bytes,
        path: format!("savegames/{}", file.path),
    }));
    create_stored_zip(&zip)
}

pub fn read_native_save_archive(bytes: &[u8]) -> Result<NativeSaveArchive, String> {
    let files = read_zip(bytes)?;
    let canonical: BTreeMap<String, &NativeArchiveFile> =
        files.iter().map(|file| (file.path.to_lowercase(), file)).collect();
    let manifest_bytes =
        canonical.get("manifest.json").ok_or("native save archive has no manifest.json")?;
    let manifest = parse_manifest(&manifest_bytes.bytes)?;
    if files.len() != manifest.files.len() + 1 {
        return Err("native save archive file count does not match its manifest".into());
    }
    let paths: BTreeSet<String> = manifest
        .files
        .iter()
        .map(|file| format!("savegames/{}", file.path.to_lowercase()))
        .collect();
    if canonical.keys().any(|path| path != "manifest.json" && !paths.contains(path)) {
        return Err("native save archive contains a file absent from its manifest".into());
    }
    for file in &manifest.files {
        let actual = canonical.get(&format!("savegames/{}", file.path.to_lowercase()));
        if actual.is_none_or(|actual| {
            actual.bytes.len() as u64 != file.size || sha256(&actual.bytes) != file.sha256
        }) {
            return Err(format!("native save archive manifest failed for {}", file.path));
        }
    }
    let darkdata = canonical.get("savegames/solomondark/darkdata.cfg");
    let game_rows: Vec<_> = manifest
        .files
        .iter()
        .filter(|file| {
            let parts: Vec<_> = file.path.split('/').collect();
            parts.len() == 4
                && parts[0].eq_ignore_ascii_case("solomondark")
                && parts[1].eq_ignore_ascii_case("savegames")
                && run_name_is_valid(parts[2])
                && parts[3].eq_ignore_ascii_case("gamestate.sav")
        })
        .collect();
    if darkdata.is_none() || game_rows.len() != 1 {
        return Err("native save archive does not contain one current wizard".into());
    }
    let game = game_rows[0];
    let gamestate = canonical[&format!("savegames/{}", game.path.to_lowercase())];
    let semantic_game = game.path.to_lowercase();
    let mut retained_files = Vec::new();
    for file in &manifest.files {
        let path = file.path.to_lowercase();
        if path == "solomondark/darkdata.cfg" || path == semantic_game {
            continue;
        }
        validate_retained_path(&file.path)?;
        retained_files.push(NativeArchiveFile {
            bytes: canonical[&format!("savegames/{path}")].bytes.clone(),
            path: file.path.clone(),
        });
    }
    Ok(NativeSaveArchive {
        darkdata: darkdata.expect("native darkdata membership was checked").bytes.clone(),
        gamestate: gamestate.bytes.clone(),
        retained_files,
        run_name: game
            .path
            .split('/')
            .nth(2)
            .expect("native gamestate path was checked")
            .to_owned(),
    })
}
