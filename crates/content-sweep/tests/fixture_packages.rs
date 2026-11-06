//! The sweep plays packages built from this repository's own mod fixtures, reports the step at
//! which the game rejects a package, and matches the Website's published listing to package
//! files by SHA-256 using only anonymous GET requests.

use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use base64::{Engine, engine::general_purpose::STANDARD};
use flate2::write::DeflateEncoder;
use serde_json::{Value, json};
use solomon_modding::assets::sha256;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../sim/tests/fixtures/website");

/// A directory removed when the test ends.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let unique = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("solomon-content-sweep-{name}-{unique}"));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn write(&self, relative: &str, bytes: &[u8]) {
        let path = self.0.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("the scratch directory remains writable");
    }
}

fn fixture(name: &str) -> Value {
    serde_json::from_str(&fs::read_to_string(Path::new(FIXTURES).join(name)).unwrap()).unwrap()
}

/// A ZIP archive with stored or deflated entries, as the authoring CLI and the Website's
/// Boneyard publisher write them.
fn zip(files: &[(&str, Vec<u8>)], deflate: bool) -> Vec<u8> {
    let mut archive = Vec::new();
    let mut directory = Vec::new();
    for (path, bytes) in files {
        let mut checksum = flate2::Crc::new();
        checksum.update(bytes);
        let data = if deflate {
            let mut encoder = DeflateEncoder::new(Vec::new(), flate2::Compression::best());
            encoder.write_all(bytes).unwrap();
            encoder.finish().unwrap()
        } else {
            bytes.clone()
        };
        let method: u16 = if deflate { 8 } else { 0 };
        let mut fields = Vec::new();
        for value in [20_u16, 0x0800, method, 0, 0x0021] {
            fields.extend_from_slice(&value.to_le_bytes());
        }
        fields.extend_from_slice(&checksum.sum().to_le_bytes());
        fields.extend_from_slice(&(data.len() as u32).to_le_bytes());
        fields.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        fields.extend_from_slice(&(path.len() as u16).to_le_bytes());
        fields.extend_from_slice(&0_u16.to_le_bytes());
        directory.extend_from_slice(&0x0201_4b50_u32.to_le_bytes());
        directory.extend_from_slice(&20_u16.to_le_bytes());
        directory.extend_from_slice(&fields);
        // Comment length, disk, internal and external attributes, then the local offset.
        directory.extend_from_slice(&[0; 10]);
        directory.extend_from_slice(&(archive.len() as u32).to_le_bytes());
        directory.extend_from_slice(path.as_bytes());
        archive.extend_from_slice(&0x0403_4b50_u32.to_le_bytes());
        archive.extend_from_slice(&fields);
        archive.extend_from_slice(path.as_bytes());
        archive.extend_from_slice(&data);
    }
    let directory_offset = archive.len() as u32;
    archive.extend_from_slice(&directory);
    archive.extend_from_slice(&0x0605_4b50_u32.to_le_bytes());
    archive.extend_from_slice(&[0; 4]);
    archive.extend_from_slice(&(files.len() as u16).to_le_bytes());
    archive.extend_from_slice(&(files.len() as u16).to_le_bytes());
    archive.extend_from_slice(&(directory.len() as u32).to_le_bytes());
    archive.extend_from_slice(&directory_offset.to_le_bytes());
    archive.extend_from_slice(&0_u16.to_le_bytes());
    archive
}

fn json_bytes(value: &Value) -> Vec<u8> {
    format!("{}\n", serde_json::to_string_pretty(value).unwrap()).into_bytes()
}

/// The Website's content digest, written out here independently of the sweep.
fn content_sha256(files: &[(&str, Vec<u8>)]) -> String {
    let mut records: Vec<String> =
        files.iter().map(|(path, bytes)| format!("{path}\0{}\n", sha256(bytes))).collect();
    records.sort();
    sha256(records.concat().as_bytes())
}

/// The `sdmod new` starter in the authoring goldens: a Lua mod with one item and its icon.
fn starter_files() -> Vec<(&'static str, Vec<u8>)> {
    let authoring = fixture("sdmod-authoring-goldens.json");
    let starter = &authoring["cases"][0];
    assert_eq!(starter["name"], "starter");
    let input = &starter["input"];
    assert_eq!(input["files"][0]["path"], "art/icon.png");
    vec![
        ("manifest.json", json_bytes(&input["manifest"])),
        ("scripts/main.lua", input["entry"].as_str().unwrap().as_bytes().to_vec()),
        ("art/icon.png", STANDARD.decode(input["files"][0]["bytes"].as_str().unwrap()).unwrap()),
    ]
}

/// The Lua mod of the host services goldens, with its own `sd.kit.boneyard` and assets.
fn host_content_files() -> Vec<(String, Vec<u8>)> {
    let services = fixture("mod-host-services-goldens.json");
    let source = &services["payload"]["mods"][0];
    let manifest = json!({
        "id": source["id"], "name": source["name"], "version": source["version"],
        "runtime": { "apiVersion": "1.0.0", "entryScript": "scripts/main.lua" },
        "requiredMods": [],
    });
    let mut files = vec![
        ("manifest.json".to_owned(), json_bytes(&manifest)),
        (
            "scripts/main.lua".to_owned(),
            source["entryScript"].as_str().unwrap().as_bytes().to_vec(),
        ),
    ];
    for file in source["files"].as_array().unwrap() {
        let bytes = STANDARD.decode(file["bytesBase64"].as_str().unwrap()).unwrap();
        files.push((file["path"].as_str().unwrap().to_owned(), bytes));
    }
    files
}

/// What the Boneyard editor publishes: one Boneyard overlay under the player's levels.
fn boneyard_publication(boneyard: Vec<u8>, name: &str, slug: &str) -> Vec<(String, Vec<u8>)> {
    let source = format!("files/{name}.boneyard");
    let manifest = json!({
        "id": slug, "name": name, "version": "1.0.0", "priority": 100,
        "overlays": [{
            "target": format!("sandbox/DarkCloud/mylevels/{name}.boneyard"),
            "source": source,
            "format": "boneyard",
        }],
        "requiredMods": [],
    });
    vec![("manifest.json".to_owned(), serde_json::to_vec(&manifest).unwrap()), (source, boneyard)]
}

fn borrowed(files: &[(String, Vec<u8>)]) -> Vec<(&str, Vec<u8>)> {
    files.iter().map(|(path, bytes)| (path.as_str(), bytes.clone())).collect()
}

/// A potion written for the draft Web Lua contract, which let a potion name its own
/// `stacking`. The status it applies owns stacking now, so the compiler rejects the field.
fn stale_potion_files() -> Vec<(&'static str, Vec<u8>)> {
    let manifest = json!({
        "id": "example.stale-potion", "name": "Stale Potion", "version": "0.4.0",
        "runtime": { "apiVersion": "1.0.0", "entryScript": "scripts/main.lua" },
        "requiredMods": [],
    });
    let script = r#"local icon = sd.art.sprite("art/icon.png")

return sd.mod({
  api = "1.0.0",
  assets = {icon = icon},
  content = {
    sd.kit.status({key = "warded", duration = "3m", stacking = "refresh", modifiers = {incoming_damage = 0}}),
    sd.kit.potion({
      key = "ward_potion",
      name = "Ward Potion",
      duration = "3m",
      stacking = "refresh",
      status = sd.ref("status", "warded"),
      on_use = sd.effect.status({target = "user", status = sd.ref("status", "warded")}),
      art = {icon = sd.art.ref("icon")},
    }),
  },
})
"#;
    let icon = starter_files().remove(2).1;
    vec![
        ("manifest.json", json_bytes(&manifest)),
        ("scripts/main.lua", script.as_bytes().to_vec()),
        ("art/icon.png", icon),
    ]
}

fn mod_boneyard() -> Vec<u8> {
    let packages = fixture("mod-package-goldens.json");
    STANDARD.decode(packages["boneyardBase64"].as_str().unwrap()).unwrap()
}

fn sweep(arguments: &[&str]) -> (Option<i32>, Value) {
    let assets = concat!(env!("CARGO_MANIFEST_DIR"), "/../../web");
    let output = Command::new(env!("CARGO_BIN_EXE_solomon-content-sweep"))
        .args(["--assets", assets, "--ticks", "120"])
        .args(arguments)
        .output()
        .unwrap();
    let summary = serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!("{error}: {}", String::from_utf8_lossy(&output.stderr));
    });
    (output.status.code(), summary)
}

fn by_id<'a>(rows: &'a Value, id: &str) -> &'a Value {
    rows.as_array().unwrap().iter().find(|row| row["id"] == id).unwrap_or_else(|| {
        panic!("{id} is missing from {rows}");
    })
}

#[test]
fn fixture_packages_load_and_rejected_packages_name_their_failing_step() {
    let packages = Scratch::new("fixtures");
    let starter = zip(&starter_files(), false);
    packages.write("fixture-base/1.0.0.zip", &starter);
    packages.write("host-content.zip", &zip(&borrowed(&host_content_files()), false));
    let boneyard = mod_boneyard();
    let publication = boneyard_publication(boneyard.clone(), "Crypt Walk", "crypt-walk");
    packages.write("crypt-walk/1.0.0.zip", &zip(&borrowed(&publication), true));
    let truncated = boneyard_publication(boneyard[..64].to_vec(), "Short Walk", "short-walk");
    packages.write("short-walk.sdmod", &zip(&borrowed(&truncated), true));
    packages.write("stale-potion.zip", &zip(&stale_potion_files(), false));
    packages.write("notes/readme.zip", b"not an archive");
    packages.write("notes/readme.txt", b"not a package");

    let (status, summary) = sweep(&["--packages", packages.0.to_str().unwrap()]);

    assert_eq!(status, Some(1), "a failed package fails the sweep: {summary:#}");
    assert_eq!(
        summary["counts"],
        json!({
            "boneyard": { "ok": 1, "failed": 1, "unavailable": 0 },
            "mod": { "ok": 2, "failed": 2, "unavailable": 0 },
        })
    );

    let base = by_id(&summary["ok"], "fixture-base");
    assert_eq!(base["kind"], "mod");
    assert_eq!(base["slug"], "fixture-base", "the Website's SLUG/VERSION.zip names the slug");
    assert_eq!(base["sha256"], sha256(&starter));
    assert_eq!(base["contentSha256"], content_sha256(&starter_files()));
    assert_eq!(
        base["boneyards"],
        json!([{
            "choiceId": "default-random", "name": "Random Boneyard", "source": "stock",
            "sourceSha256": base["boneyards"][0]["sourceSha256"], "ticks": 120,
        }]),
        "a Lua mod plays the stock Random Boneyard"
    );

    let host = by_id(&summary["ok"], "example.host-content");
    let plays = host["boneyards"].as_array().unwrap();
    assert_eq!(plays.len(), 2, "the stock Boneyard and the mod's own: {host:#}");
    assert_eq!(plays[1]["source"], "mod");
    assert_eq!(plays[1]["sourceSha256"], sha256(&host_content_boneyard()));
    assert!(plays.iter().all(|play| play["ticks"] == 120), "{host:#}");

    let crypt = by_id(&summary["ok"], "crypt-walk");
    assert_eq!(crypt["kind"], "boneyard");
    assert_eq!(crypt["boneyards"][0]["name"], "Crypt Walk");
    assert_eq!(crypt["boneyards"][0]["sourceSha256"], sha256(&boneyard));
    assert_eq!(crypt["boneyards"][0]["ticks"], 120);

    let stale = by_id(&summary["failed"], "example.stale-potion");
    assert_eq!(stale["stage"], "definitions");
    assert_eq!(stale["version"], "0.4.0");
    let error = stale["error"].as_str().unwrap();
    assert!(error.starts_with("E_UNKNOWN_FIELD content[1].fields.stacking"), "{error}");

    let short = by_id(&summary["failed"], "short-walk");
    assert_eq!((&short["kind"], &short["stage"]), (&json!("boneyard"), &json!("admission")));

    let unreadable = summary["failed"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["slug"] == "readme")
        .expect("the file that is not an archive is reported");
    assert_eq!(unreadable["stage"], "package");
    assert_eq!(unreadable["error"], "The file is not a ZIP archive.");
    assert_eq!(unreadable["sha256"], sha256(b"not an archive"));
    assert!(unreadable["id"].is_null());
}

fn host_content_boneyard() -> Vec<u8> {
    host_content_files().into_iter().find(|(path, _)| path.ends_with(".boneyard")).unwrap().1
}

/// Serves `routes` by request target until the test ends, recording each request head.
fn serve(routes: BTreeMap<String, Value>) -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&requests);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = stream.unwrap();
            let mut head = Vec::new();
            let mut byte = [0];
            while !head.ends_with(b"\r\n\r\n") {
                stream.read_exact(&mut byte).unwrap();
                head.push(byte[0]);
            }
            let head = String::from_utf8(head).unwrap();
            let target = head.split_whitespace().nth(1).unwrap().to_owned();
            recorded.lock().unwrap().push(head);
            let (status, body) = match routes.get(&target) {
                Some(body) => ("200 OK", body.to_string()),
                None => ("404 Not Found", json!({ "error": "missing" }).to_string()),
            };
            write!(
                stream,
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\
                 Connection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        }
    });
    (format!("http://{address}/game-rust"), requests)
}

#[test]
fn the_published_listing_selects_each_mods_latest_package_by_its_published_digests() {
    let packages = Scratch::new("listing");
    let starter = zip(&starter_files(), false);
    packages.write("fixture-base/1.0.0.zip", &starter);
    let publication = boneyard_publication(mod_boneyard(), "Crypt Walk", "crypt-walk");
    let crypt = zip(&borrowed(&publication), true);
    packages.write("crypt-walk/1.0.0.zip", &crypt);
    let stale = zip(&stale_potion_files(), false);
    packages.write("stale-potion/0.4.0.zip", &stale);

    let version = |label: &str, package: String, content: String| {
        json!({ "id": 1, "version": label, "manifestVersion": label, "packageSha256": package,
            "contentSha256": content, "fileSize": 1 })
    };
    let detail = |name: &str, package_id: &str, tags: Value, versions: Value| json!({ "name": name, "packageId": package_id, "tags": tags, "versions": versions });
    let routes = BTreeMap::from([
        (
            "/game-rust/api/mods?sort=newest&pageSize=50&page=1".to_owned(),
            json!({
                "items": [{ "slug": "fixture-base" }, { "slug": "crypt-walk" }, { "slug": "unpackaged" }],
                "total": 3, "page": 1, "pageSize": 50,
            }),
        ),
        (
            "/game-rust/api/mods/fixture-base".to_owned(),
            detail(
                "Fixture Base",
                "fixture-base",
                json!(["items"]),
                json!([
                    version("1.0.0", sha256(&starter), content_sha256(&starter_files())),
                    version("0.9.0", "a".repeat(64), "b".repeat(64)),
                ]),
            ),
        ),
        (
            "/game-rust/api/mods/crypt-walk".to_owned(),
            detail(
                "Crypt Walk",
                "crypt-walk",
                json!(["boneyard"]),
                json!([version("1.0.0", sha256(&crypt).to_uppercase(), "c".repeat(64))]),
            ),
        ),
        (
            "/game-rust/api/mods/unpackaged".to_owned(),
            detail(
                "Unpackaged",
                "example.unpackaged",
                json!(["boneyard", "waves"]),
                json!([
                    { "version": "2.0.0", "manifestVersion": null, "packageSha256": null, "contentSha256": null },
                    version("1.0.0", "d".repeat(64), "e".repeat(64)),
                ]),
            ),
        ),
    ]);
    let (base, requests) = serve(routes);

    let (status, summary) =
        sweep(&["--api", &base, "--packages", packages.0.to_str().unwrap(), "--delay-ms", "0"]);

    assert_eq!(status, Some(1), "{summary:#}");
    assert_eq!(summary["api"], format!("{base}/"));
    assert_eq!(summary["published"], 3);
    assert_eq!(
        summary["counts"],
        json!({
            "boneyard": { "ok": 0, "failed": 1, "unavailable": 1 },
            "mod": { "ok": 1, "failed": 0, "unavailable": 0 },
        })
    );
    let base_mod = by_id(&summary["ok"], "fixture-base");
    assert_eq!(
        (&base_mod["slug"], &base_mod["version"]),
        (&json!("fixture-base"), &json!("1.0.0"))
    );

    let mismatch = by_id(&summary["failed"], "crypt-walk");
    assert_eq!(mismatch["stage"], "identity");
    assert_eq!(mismatch["sha256"], sha256(&crypt), "upper-case published digests still match");

    let missing = by_id(&summary["unavailable"], "example.unpackaged");
    assert_eq!(missing["kind"], "boneyard", "the editor's boneyard tag names the kind");
    assert_eq!(missing["version"], "1.0.0", "a version without a package is skipped");
    assert_eq!(missing["sha256"], "d".repeat(64));
    assert_eq!(missing["reason"], "No package under --packages has the published SHA-256.");

    assert_eq!(
        summary["ignoredPackages"],
        json!([{ "path": "stale-potion/0.4.0.zip", "sha256": sha256(&stale) }]),
        "a package no listed mod loads is not swept"
    );

    let requests = requests.lock().unwrap();
    let targets: Vec<&str> =
        requests.iter().map(|head| head.split_whitespace().nth(1).unwrap()).collect();
    assert_eq!(
        targets,
        [
            "/game-rust/api/mods?sort=newest&pageSize=50&page=1",
            "/game-rust/api/mods/fixture-base",
            "/game-rust/api/mods/crypt-walk",
            "/game-rust/api/mods/unpackaged",
        ]
    );
    for head in requests.iter() {
        let lower = head.to_ascii_lowercase();
        assert!(lower.starts_with("get "), "{head}");
        assert!(lower.contains("user-agent: solomon-darker-content-sweep\r\n"), "{head}");
        assert!(!lower.contains("authorization:") && !lower.contains("cookie:"), "{head}");
    }
}
