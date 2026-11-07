//! Mod packages as the Website stores them, and the session content its content service builds
//! from one.
//!
//! The Website keeps each published version as a ZIP archive, `uploads/mods/SLUG/VERSION.zip`,
//! and identifies it twice: by the SHA-256 of the archive bytes, and by a content digest over
//! every file's path and SHA-256, which does not depend on how the archive was written. When a
//! session starts, `WebModContentService` in the Website backend turns the package into one
//! session content row: the entry script named by `runtime.entryScript`, every `.boneyard`
//! overlay, and the typed asset files under `sprites/`, `art/`, `audio/`, `levels/` and
//! `scenes/`, with its size limits. [`Package::session_content`] builds the same row, so the
//! Rust host receives what production would send it. The Website's publication checks are not
//! repeated: a published package has already passed them.

use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::Path;

use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Map, Value, json};
use solomon_modding::assets::sha256;

const LOCAL_HEADER: u32 = 0x0403_4b50;
const CENTRAL_HEADER: u32 = 0x0201_4b50;
const END_RECORD: u32 = 0x0605_4b50;
const END_RECORD_BYTES: usize = 22;

// The content service's limits; a package over one is disabled before any session starts.
const MAXIMUM_ENTRY_SCRIPT_BYTES: usize = 256 * 1024;
const MAXIMUM_BONEYARD_BYTES: usize = 8 * 1024 * 1024;
const MAXIMUM_TYPED_FILES: usize = 256;
const MAXIMUM_TYPED_FILE_BYTES: usize = 16 * 1024 * 1024;
const MAXIMUM_TYPED_FILES_TOTAL_BYTES: usize = 32 * 1024 * 1024;

/// A package file found under the packages directory.
pub struct PackageFile {
    /// The path below the packages directory, with `/` separators.
    pub relative_path: String,
    pub bytes: Vec<u8>,
    pub sha256: String,
}

/// Every `.zip` or `.sdmod` file below `root`, in path order. Symbolic links are not followed.
pub fn find_packages(root: &Path) -> Result<Vec<PackageFile>, String> {
    let mut packages = Vec::new();
    collect_packages(root, "", &mut packages)?;
    packages.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(packages)
}

fn collect_packages(
    directory: &Path,
    prefix: &str,
    packages: &mut Vec<PackageFile>,
) -> Result<(), String> {
    let entries = fs::read_dir(directory)
        .map_err(|error| format!("Cannot read {}: {error}", directory.display()))?;
    for entry in entries {
        let entry =
            entry.map_err(|error| format!("Cannot read {}: {error}", directory.display()))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let relative_path = format!("{prefix}{name}");
        let kind = entry.file_type().map_err(|error| format!("Cannot read {name}: {error}"))?;
        if kind.is_dir() {
            collect_packages(&entry.path(), &format!("{relative_path}/"), packages)?;
            continue;
        }
        let extension = Path::new(&name).extension().and_then(|extension| extension.to_str());
        let is_package = extension
            .is_some_and(|extension| matches!(extension.to_lowercase().as_str(), "zip" | "sdmod"));
        if kind.is_file() && is_package {
            let bytes = fs::read(entry.path())
                .map_err(|error| format!("Cannot read {relative_path}: {error}"))?;
            packages.push(PackageFile { relative_path, sha256: sha256(&bytes), bytes });
        }
    }
    Ok(())
}

/// A readable package: its manifest and files, with both of the Website's identities.
pub struct Package {
    pub content_sha256: String,
    manifest: Map<String, Value>,
    files: BTreeMap<String, Vec<u8>>,
}

impl Package {
    pub fn read(bytes: &[u8]) -> Result<Self, String> {
        let files = read_zip(bytes)?;
        let manifest = files.get("manifest.json").ok_or("The package has no manifest.json.")?;
        let manifest = match serde_json::from_slice(manifest) {
            Ok(Value::Object(manifest)) => manifest,
            Ok(_) => return Err("manifest.json is not a JSON object.".into()),
            Err(error) => return Err(format!("manifest.json is not valid JSON: {error}")),
        };
        Ok(Self { content_sha256: content_digest(&files), manifest, files })
    }

    #[must_use]
    pub fn id(&self) -> Option<&str> {
        self.manifest.get("id").and_then(Value::as_str)
    }

    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.manifest.get("name").and_then(Value::as_str)
    }

    #[must_use]
    pub fn version(&self) -> Option<&str> {
        self.manifest.get("version").and_then(Value::as_str)
    }

    /// Whether this is what the Boneyard editor publishes: Boneyard overlays and no Lua.
    #[must_use]
    pub fn is_boneyard_publication(&self) -> bool {
        self.entry_script_path().is_none()
            && self
                .overlays()
                .iter()
                .any(|overlay| overlay_target(overlay).is_some_and(is_boneyard))
    }

    /// The session content document the Website provisions for a session with only this mod
    /// enabled. `slug` and `name` are the Website's own record of the mod; the rest comes from
    /// the package.
    pub fn session_content(&self, slug: &str, name: &str) -> Result<String, String> {
        let id = self.id().ok_or("manifest.id is missing.")?;
        let version = self.version().ok_or("manifest.version is missing.")?;
        let priority = match self.manifest.get("priority") {
            None => 0,
            Some(value) => value
                .as_i64()
                .filter(|priority| i32::try_from(*priority).is_ok())
                .ok_or("manifest.priority is not a 32-bit integer.")?,
        };
        let manifest_sha256 =
            sha256(format!("{id}\0{version}\0{}\n", self.content_sha256).as_bytes());
        let row = json!({
            "boneyards": self.boneyards()?,
            "contentSha256": self.content_sha256,
            "entryScript": self.entry_script()?,
            "files": self.typed_files()?,
            "id": id,
            "name": name,
            "priority": priority,
            "slug": slug,
            "version": version,
        });
        Ok(json!({ "manifestSha256": manifest_sha256, "mods": [row] }).to_string())
    }

    fn entry_script_path(&self) -> Option<&str> {
        self.manifest.get("runtime")?.get("entryScript")?.as_str()
    }

    fn entry_script(&self) -> Result<Option<String>, String> {
        let Some(path) = self.entry_script_path() else { return Ok(None) };
        let bytes = self
            .files
            .get(path)
            .ok_or_else(|| format!("The mod entry script is missing: {path}"))?;
        if bytes.len() > MAXIMUM_ENTRY_SCRIPT_BYTES {
            return Err(format!("The mod entry script is too large: {path}"));
        }
        String::from_utf8(bytes.clone())
            .map(Some)
            .map_err(|_| format!("The mod entry script is not UTF-8: {path}"))
    }

    fn overlays(&self) -> &[Value] {
        self.manifest.get("overlays").and_then(Value::as_array).map_or(&[], Vec::as_slice)
    }

    fn boneyards(&self) -> Result<Vec<Value>, String> {
        let mut boneyards = Vec::new();
        for overlay in self.overlays() {
            let target = overlay_target(overlay).ok_or("A manifest overlay has no target.")?;
            if !is_boneyard(target) {
                continue;
            }
            let source = overlay
                .get("source")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("The overlay for {target} has no source."))?;
            let bytes = self
                .files
                .get(source)
                .ok_or_else(|| format!("The mod Boneyard is missing: {source}"))?;
            if bytes.len() > MAXIMUM_BONEYARD_BYTES {
                return Err(format!("The mod Boneyard is too large: {source}"));
            }
            boneyards.push(json!({ "bytesBase64": STANDARD.encode(bytes), "target": target }));
        }
        Ok(boneyards)
    }

    fn typed_files(&self) -> Result<Vec<Value>, String> {
        let mut paths: Vec<&String> =
            self.files.keys().filter(|path| typed_asset(path).is_some()).collect();
        paths.sort_by(|left, right| left.encode_utf16().cmp(right.encode_utf16()));
        if paths.len() > MAXIMUM_TYPED_FILES {
            return Err(format!(
                "A web mod may provision at most {MAXIMUM_TYPED_FILES} typed asset files."
            ));
        }
        let mut total_bytes = 0;
        let mut files = Vec::with_capacity(paths.len());
        for path in paths {
            let bytes = &self.files[path];
            if bytes.is_empty() || bytes.len() > MAXIMUM_TYPED_FILE_BYTES {
                return Err(format!("The mod asset file has an invalid size: {path}"));
            }
            total_bytes += bytes.len();
            if total_bytes > MAXIMUM_TYPED_FILES_TOTAL_BYTES {
                return Err("The mod asset files exceed the 32 MiB web-session limit.".into());
            }
            let (kind, content_type) = typed_asset(path).expect("only typed paths were selected");
            files.push(json!({
                "byteLength": bytes.len(),
                "bytesBase64": STANDARD.encode(bytes),
                "contentType": content_type,
                "kind": kind,
                "path": path,
                "sha256": sha256(bytes),
            }));
        }
        Ok(files)
    }
}

fn overlay_target(overlay: &Value) -> Option<&str> {
    overlay.get("target").and_then(Value::as_str)
}

fn is_boneyard(target: &str) -> bool {
    target.to_lowercase().ends_with(".boneyard")
}

/// The kind and content type the Website gives a package file it provisions as an asset. The
/// extension is everything after the last dot of the final path segment, matched exactly.
fn typed_asset(path: &str) -> Option<(&'static str, &'static str)> {
    let file_name = path.rsplit('/').next().unwrap_or(path);
    let (_, extension) = file_name.rsplit_once('.')?;
    if path.starts_with("sprites/") || path.starts_with("art/") {
        return match extension {
            "png" => Some(("image", "image/png")),
            "bundle" => Some(("sprite-bundle", "application/vnd.solomon-dark.sprite-bundle")),
            "json" => Some(("art-metadata", "application/json")),
            _ => None,
        };
    }
    if path.starts_with("audio/") {
        return match extension {
            "ogg" => Some(("audio", "audio/ogg")),
            "wav" => Some(("audio", "audio/wav")),
            "mp3" => Some(("audio", "audio/mpeg")),
            _ => None,
        };
    }
    if path.starts_with("levels/") {
        return match extension {
            "boneyard" => Some(("boneyard", "application/vnd.solomon-dark.boneyard")),
            "json" => Some(("level-metadata", "application/json")),
            _ => None,
        };
    }
    (path.starts_with("scenes/") && extension == "json").then_some(("scene", "application/json"))
}

/// The Website's content digest: SHA-256 over one `PATH\0SHA256\n` record per file, in ordinal
/// (UTF-16 code unit) path order.
fn content_digest(files: &BTreeMap<String, Vec<u8>>) -> String {
    let mut paths: Vec<&String> = files.keys().collect();
    paths.sort_by(|left, right| left.encode_utf16().cmp(right.encode_utf16()));
    let mut records = String::new();
    for path in paths {
        records.push_str(&format!("{path}\0{}\n", sha256(&files[path])));
    }
    sha256(records.as_bytes())
}

/// Every file of a stored or deflated ZIP archive. Directory entries hold no content and are
/// skipped, as the content digest skips them.
fn read_zip(bytes: &[u8]) -> Result<BTreeMap<String, Vec<u8>>, String> {
    let end = find_end_record(bytes)?;
    let count = u16_at(bytes, end + 10)?;
    let directory_offset = u32_at(bytes, end + 16)?;
    if count == u16::MAX || directory_offset == u32::MAX {
        return Err("ZIP64 packages are not supported.".into());
    }
    let mut files = BTreeMap::new();
    let mut cursor = directory_offset as usize;
    for _ in 0..count {
        if u32_at(bytes, cursor)? != CENTRAL_HEADER {
            return Err("The ZIP central directory is invalid.".into());
        }
        let flags = u16_at(bytes, cursor + 8)?;
        let method = u16_at(bytes, cursor + 10)?;
        let crc = u32_at(bytes, cursor + 16)?;
        let compressed_size = u32_at(bytes, cursor + 20)? as usize;
        let size = u32_at(bytes, cursor + 24)? as usize;
        let name_length = usize::from(u16_at(bytes, cursor + 28)?);
        let extra_length = usize::from(u16_at(bytes, cursor + 30)?);
        let comment_length = usize::from(u16_at(bytes, cursor + 32)?);
        let local_offset = u32_at(bytes, cursor + 42)? as usize;
        let path = String::from_utf8(slice(bytes, cursor + 46, name_length)?.to_vec())
            .map_err(|_| "A ZIP entry name is not UTF-8.")?;
        cursor += 46 + name_length + extra_length + comment_length;
        if path.ends_with('/') {
            continue;
        }
        if flags & 1 != 0 {
            return Err(format!("The ZIP entry is encrypted: {path}"));
        }
        if u32_at(bytes, local_offset)? != LOCAL_HEADER {
            return Err(format!("The ZIP entry has no local header: {path}"));
        }
        let data_offset = local_offset
            + 30
            + usize::from(u16_at(bytes, local_offset + 26)?)
            + usize::from(u16_at(bytes, local_offset + 28)?);
        let data = slice(bytes, data_offset, compressed_size)?;
        let content = match method {
            0 => data.to_vec(),
            8 => inflate(data, size).map_err(|error| format!("{path}: {error}"))?,
            _ => return Err(format!("The ZIP entry uses compression method {method}: {path}")),
        };
        let mut checksum = flate2::Crc::new();
        checksum.update(&content);
        if content.len() != size || checksum.sum() != crc {
            return Err(format!("The ZIP entry failed its checksum: {path}"));
        }
        if files.insert(path.clone(), content).is_some() {
            return Err(format!("The ZIP archive repeats {path}"));
        }
    }
    Ok(files)
}

fn inflate(data: &[u8], size: usize) -> Result<Vec<u8>, String> {
    let mut content = Vec::with_capacity(size);
    // One byte past the declared size is enough to detect an entry that inflates larger.
    flate2::read::DeflateDecoder::new(data)
        .take(size as u64 + 1)
        .read_to_end(&mut content)
        .map_err(|error| format!("invalid deflate data: {error}"))?;
    Ok(content)
}

fn find_end_record(bytes: &[u8]) -> Result<usize, String> {
    let not_zip = || "The file is not a ZIP archive.".to_owned();
    let last = bytes.len().checked_sub(END_RECORD_BYTES).ok_or_else(not_zip)?;
    // The record ends with a comment of at most 65,535 bytes.
    let first = last.saturating_sub(usize::from(u16::MAX));
    (first..=last)
        .rev()
        .find(|&offset| bytes[offset..offset + 4] == END_RECORD.to_le_bytes())
        .ok_or_else(not_zip)
}

fn slice(bytes: &[u8], offset: usize, length: usize) -> Result<&[u8], String> {
    offset
        .checked_add(length)
        .and_then(|end| bytes.get(offset..end))
        .ok_or_else(|| "The ZIP archive is truncated.".into())
}

fn u16_at(bytes: &[u8], offset: usize) -> Result<u16, String> {
    let field = slice(bytes, offset, 2)?;
    Ok(u16::from_le_bytes([field[0], field[1]]))
}

fn u32_at(bytes: &[u8], offset: usize) -> Result<u32, String> {
    let field = slice(bytes, offset, 4)?;
    Ok(u32::from_le_bytes([field[0], field[1], field[2], field[3]]))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stored ZIP archive with the given entries, written the way the authoring CLI does.
    fn stored_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut archive = Vec::new();
        let mut directory = Vec::new();
        for (path, bytes) in entries {
            let mut checksum = flate2::Crc::new();
            checksum.update(bytes);
            let offset = archive.len() as u32;
            let header = |signature: u32, central: bool| {
                let mut header = signature.to_le_bytes().to_vec();
                if central {
                    header.extend_from_slice(&20_u16.to_le_bytes());
                }
                for field in [20_u16, 0x0800, 0, 0, 0x0021] {
                    header.extend_from_slice(&field.to_le_bytes());
                }
                header.extend_from_slice(&checksum.sum().to_le_bytes());
                header.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
                header.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
                header.extend_from_slice(&(path.len() as u16).to_le_bytes());
                header.extend_from_slice(&0_u16.to_le_bytes());
                if central {
                    // Comment length, disk, internal and external attributes, then the offset.
                    header.extend_from_slice(&[0; 10]);
                    header.extend_from_slice(&offset.to_le_bytes());
                }
                header.extend_from_slice(path.as_bytes());
                header
            };
            archive.extend(header(LOCAL_HEADER, false));
            archive.extend_from_slice(bytes);
            directory.extend(header(CENTRAL_HEADER, true));
        }
        let directory_offset = archive.len() as u32;
        archive.extend_from_slice(&directory);
        archive.extend_from_slice(&END_RECORD.to_le_bytes());
        archive.extend_from_slice(&[0; 4]);
        archive.extend_from_slice(&(entries.len() as u16).to_le_bytes());
        archive.extend_from_slice(&(entries.len() as u16).to_le_bytes());
        archive.extend_from_slice(&(directory.len() as u32).to_le_bytes());
        archive.extend_from_slice(&directory_offset.to_le_bytes());
        archive.extend_from_slice(&0_u16.to_le_bytes());
        archive
    }

    #[test]
    fn the_session_row_keeps_typed_assets_the_entry_script_and_boneyard_overlays() {
        let manifest = br#"{"id":"example.row","name":"Row","version":"1.2.0","priority":7,
            "runtime":{"apiVersion":"1.0.0","entryScript":"scripts/main.lua"},
            "overlays":[{"target":"data/levels/Row.BONEYARD","source":"files/row.boneyard","format":"boneyard"}],
            "requiredMods":[]}"#;
        let archive = stored_zip(&[
            ("manifest.json", manifest),
            ("scripts/main.lua", b"return sd.mod({})"),
            ("files/row.boneyard", b"scene"),
            ("art/b.png", b"png"),
            ("art/a.bundle", b"bundle"),
            ("art/notes.txt", b"not provisioned"),
            ("audio/hit.ogg", b"ogg"),
            ("README.md", b"not provisioned"),
        ]);
        let package = Package::read(&archive).unwrap();
        assert!(!package.is_boneyard_publication(), "a package with Lua is a mod");
        let document: Value =
            serde_json::from_str(&package.session_content("row", "Row Mod").unwrap()).unwrap();
        let row = &document["mods"][0];
        assert_eq!(row["entryScript"], "return sd.mod({})");
        assert_eq!(row["priority"], 7);
        assert_eq!(row["name"], "Row Mod", "the Website's own name wins over the manifest's");
        assert_eq!(row["boneyards"][0]["target"], "data/levels/Row.BONEYARD");
        assert_eq!(row["boneyards"][0]["bytesBase64"], STANDARD.encode(b"scene"));
        let files: Vec<(&str, &str)> = row["files"]
            .as_array()
            .unwrap()
            .iter()
            .map(|file| (file["path"].as_str().unwrap(), file["kind"].as_str().unwrap()))
            .collect();
        assert_eq!(
            files,
            [("art/a.bundle", "sprite-bundle"), ("art/b.png", "image"), ("audio/hit.ogg", "audio")]
        );
        assert_eq!(
            document["manifestSha256"],
            sha256(format!("example.row\x001.2.0\x00{}\n", package.content_sha256).as_bytes())
        );
    }

    #[test]
    fn a_corrupted_entry_fails_its_checksum_and_a_non_archive_is_rejected() {
        let mut archive = stored_zip(&[("manifest.json", b"{\"id\":\"example.crc\"}")]);
        // The entry's bytes follow its 30-byte local header and name.
        archive[30 + "manifest.json".len()] ^= 1;
        assert_eq!(
            Package::read(&archive).err().unwrap(),
            "The ZIP entry failed its checksum: manifest.json"
        );
        assert_eq!(Package::read(b"not a zip").err().unwrap(), "The file is not a ZIP archive.");
    }
}
