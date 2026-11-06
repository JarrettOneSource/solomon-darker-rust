//! `solomon-save-check` reports how the hosted game's load path handles stored cloud saves.
//! These tests feed it recorded documents and crafted corrupt rows, and check that every
//! outcome is counted, that failing rows are named by the SHA-256 of their text, and that no
//! document text reaches the summary or the error stream. The last test runs the database
//! exporter, `scripts/solomon-save-rows.py`, into the checker.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use solomon_assets::website_save::MAX_SAVE_BYTES;

const CHECKER: &str = env!("CARGO_BIN_EXE_solomon-save-check");

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixture(name: &str) -> Value {
    let path = repository().join("crates/sim/tests/fixtures/website").join(name);
    let bytes = std::fs::read(&path).unwrap();
    if name.ends_with(".zlib") {
        let mut json = Vec::new();
        flate2::read::ZlibDecoder::new(bytes.as_slice()).read_to_end(&mut json).unwrap();
        serde_json::from_slice(&json).unwrap()
    } else {
        serde_json::from_slice(&bytes).unwrap()
    }
}

fn document(cases: &Value, name: &str) -> Value {
    cases.as_array().unwrap().iter().find(|case| case["name"] == name).unwrap()["document"].clone()
}

fn row(slot: u64, revision: u64, document: &str) -> String {
    json!({"slot": slot, "revision": revision, "document": document}).to_string() + "\n"
}

fn sha256(text: &str) -> String {
    Sha256::digest(text.as_bytes()).iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Runs the checker on `input` with the repository's client assets. It must exit 0 whatever
/// the saves' outcomes.
fn check(input: Vec<u8>) -> (Value, String) {
    let mut child = Command::new(CHECKER)
        .arg("--assets")
        .arg(repository().join("web"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let writer = std::thread::spawn(move || stdin.write_all(&input));
    let output = child.wait_with_output().unwrap();
    writer.join().unwrap().unwrap();
    assert!(output.status.success(), "checker exit {:?}", output.status);
    let stdout = String::from_utf8(output.stdout).unwrap();
    (serde_json::from_str(&stdout).unwrap(), String::from_utf8(output.stderr).unwrap())
}

#[test]
fn recorded_saves_load_and_each_failure_is_counted_without_document_text() {
    let continuations = fixture("current-save-continuation-goldens.json.zlib")["cases"].clone();
    let profiles = fixture("current-save-profile-goldens.json")["profiles"].clone();
    let hub = document(&continuations, "current-hub-effects-true");
    let mut documents = vec![
        (1, hub.to_string()),
        (1, document(&continuations, "current-boneyard-effects-true").to_string()),
        (1, document(&continuations, "current-tutorial-effects-true").to_string()),
        (
            1,
            document(&continuations, "historical-save-migration-legacy-hub-painter-and-belt-1")
                .to_string(),
        ),
        (0, document(&profiles, "current").to_string()),
        (0, document(&profiles, "compatible-future").to_string()),
    ];
    // Each crafted value is a canary that must not reach the output.
    documents.push((0, r#"{"schemaVersion":52,"profile":"CANARY-TRUNCATED"#.into()));
    let mut unsupported = hub.clone();
    unsupported["schemaVersion"] = json!(0);
    unsupported["continuation"]["summary"]["character"]["displayName"] = json!("CANARY Name");
    documents.push((1, unsupported.to_string()));
    let mut extra = hub.clone();
    extra["CANARY_ROOT_FIELD"] = json!(true);
    documents.push((1, extra.to_string()));
    let mut corrupted = hub.clone();
    corrupted["continuation"]["simulation"]["gameRng"]["indexA"] = json!("CANARY-NOT-A-NUMBER");
    documents.push((1, corrupted.to_string()));
    let mut unknown_phase = hub.clone();
    unknown_phase["continuation"]["simulation"]["run"]["phase"] = json!("CANARY-PHASE");
    documents.push((1, unknown_phase.to_string()));
    let mut flat = document(&continuations, "current-boneyard-effects-true");
    let scene = &mut flat["continuation"]["loadedBoneyard"]["scene"];
    scene["name"] = json!("CANARY Yard");
    scene["bounds"]["w"] = json!(0);
    documents.push((1, flat.to_string()));
    // A recorded test arena, not a shipped Boneyard: the host has no navigation for it.
    let arena = document(&continuations, "historical-save-spell-migration-secondary-11");
    documents.push((1, arena.to_string()));
    let rejected = document(&continuations, "current-bad-heading-index-effects-true");
    documents.push((1, rejected.to_string()));
    documents.push((0, "x".repeat(MAX_SAVE_BYTES + 1)));

    let mut input = Vec::new();
    for (revision, (slot, text)) in documents.iter().enumerate() {
        input.extend(row(*slot, revision as u64 + 1, text).into_bytes());
    }
    let malformed = [
        json!({"slot": 1, "revision": 99, "document": "{}", "userId": 7}).to_string(),
        "CANARY not a row".to_owned(),
    ];
    for line in &malformed {
        input.extend(format!("{line}\n").into_bytes());
    }
    let (summary, stderr) = check(input);

    assert_eq!(summary["rows"], 15);
    assert_eq!(summary["malformedLines"], json!([16, 17]));
    assert_eq!(
        summary["totals"],
        json!({"ok": 6, "parseError": 3, "restoreError": 5, "roundTripError": 0,
            "oversize": 1, "panic": 0})
    );
    let counts = |ok, parse, restore, oversize| {
        json!({"ok": ok, "parseError": parse, "restoreError": restore, "roundTripError": 0,
            "oversize": oversize, "panic": 0})
    };
    assert_eq!(
        summary["bySlotAndSchemaVersion"],
        json!({
            "0": {"51": counts(1, 0, 0, 0), "newer": counts(1, 0, 0, 0),
                "unreadable": counts(0, 1, 0, 1)},
            "1": {"1": counts(1, 0, 0, 0), "37": counts(0, 0, 1, 0), "51": counts(3, 1, 4, 0),
                "invalid": counts(0, 1, 0, 0)},
        })
    );
    assert_eq!(summary["bySlot"]["0"], counts(2, 1, 0, 1));
    assert_eq!(summary["bySlot"]["1"], counts(4, 2, 5, 0));

    let mut messages = Vec::new();
    for error in summary["errors"].as_array().unwrap() {
        for failed in error["rows"].as_array().unwrap() {
            let revision = failed["revision"].as_u64().unwrap() as usize;
            let (slot, text) = &documents[revision - 1];
            assert_eq!(failed["slot"], *slot, "row {revision}");
            assert_eq!(failed["sha256"], sha256(text), "row {revision}");
            let message = error["message"].as_str().unwrap();
            messages.push((revision, error["outcome"].as_str().unwrap(), message));
        }
        assert_eq!(error["count"], error["rows"].as_array().unwrap().len());
    }
    messages.sort_by_key(|(revision, ..)| *revision);
    let navigation = messages.iter().find(|(revision, ..)| *revision == 13).unwrap().2;
    // The operating system's error text follows the redacted path.
    assert!(navigation.starts_with("Cannot load native navigation <file>: "), "{navigation}");
    assert_eq!(
        messages,
        [
            (7, "parseError", "game save is not valid JSON"),
            (8, "parseError", "game save schema version is invalid"),
            (9, "parseError", "game save has unexpected field <field>"),
            (10, "restoreError", "RngDocument: invalid type: string, expected usize"),
            (11, "restoreError", "GameRunLifecycle: GameRunPhase has unknown value <value>"),
            (12, "restoreError", "<scene>: Boneyard bounds must have positive size"),
            (13, "restoreError", navigation),
            (14, "restoreError", "game save player locomotion # heading invariant is invalid"),
            (15, "oversize", "game save exceeds its size limit"),
        ]
    );

    let printed = summary.to_string();
    for text in [
        "CANARY",
        "current-save-owner",
        "Current Save",
        "sourceOwner_With-Dashes",
        "Save Oracle",
        "Current Profile",
        "spell-continuation-owner",
        "Spell Continuation",
        "3376a0b6710d937122479fd0c672b51d0f44b75a2acb096bd71a03c5ddce5af2",
    ] {
        assert!(!printed.contains(text), "the summary repeats document text {text}");
    }
    assert_eq!(stderr, "");
}

#[test]
fn the_row_exporter_streams_a_read_only_database_into_the_checker() {
    let python = if cfg!(windows) { "python" } else { "python3" };
    let directory = std::env::temp_dir().join(format!("solomon-save-rows-{}", std::process::id()));
    if directory.exists() {
        std::fs::remove_dir_all(&directory).unwrap();
    }
    std::fs::create_dir(&directory).unwrap();
    let database = directory.join("website.db");
    let profile = document(&fixture("current-save-profile-goldens.json")["profiles"], "current");
    let continuation = document(
        &fixture("current-save-continuation-goldens.json.zlib")["cases"],
        "current-hub-effects-true",
    );
    let stored = [
        (41, 0, 3, profile.to_string()),
        (41, 1, 7, continuation.to_string()),
        (42, 1, 2, r#"{"schemaVersion": 52, "profile": "CANARY"#.to_owned()),
    ];
    // The Website's table, created and filled by Python's own SQLite module.
    let created = Command::new(python)
        .arg("-c")
        .arg(
            "import json, sqlite3, sys\n\
             db = sqlite3.connect(sys.argv[1])\n\
             db.execute('CREATE TABLE WebGameSaves (Id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT, \
             UserId INTEGER NOT NULL, Slot INTEGER NOT NULL, Revision INTEGER NOT NULL, \
             Document TEXT NOT NULL)')\n\
             db.executemany('INSERT INTO WebGameSaves (UserId, Slot, Revision, Document) \
             VALUES (?, ?, ?, ?)', json.load(sys.stdin))\n\
             db.commit()",
        )
        .arg(&database)
        .stdin(Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            child.stdin.take().unwrap().write_all(json!(stored).to_string().as_bytes())?;
            child.wait()
        })
        .unwrap();
    assert!(created.success());
    let before = std::fs::read(&database).unwrap();
    let exporter = repository().join("scripts/solomon-save-rows.py");

    let exported = Command::new(python).arg(&exporter).arg(&database).output().unwrap();
    assert!(exported.status.success());
    assert!(exported.stderr.is_empty());
    let lines: Vec<Value> = String::from_utf8(exported.stdout.clone())
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let rows: Vec<_> = stored
        .iter()
        .map(|(_, slot, revision, text)| json!({"slot": slot, "revision": revision, "document": text}))
        .collect();
    assert_eq!(lines, rows, "rows carry exactly slot, revision and document, in Id order");

    let (summary, stderr) = check(exported.stdout);
    assert_eq!(summary["totals"]["ok"], 2);
    assert_eq!(summary["totals"]["parseError"], 1);
    assert!(!summary.to_string().contains("CANARY"));
    assert_eq!(stderr, "");

    let found = Command::new(python)
        .arg(&exporter)
        .arg(&database)
        .args(["--find-sha256", &sha256(&stored[2].3)])
        .output()
        .unwrap();
    assert!(found.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&found.stdout).unwrap(),
        json!({"id": 3, "slot": 1, "revision": 2})
    );
    assert_eq!(std::fs::read(&database).unwrap(), before, "the exporter never writes");
    std::fs::remove_dir_all(&directory).unwrap();
}
