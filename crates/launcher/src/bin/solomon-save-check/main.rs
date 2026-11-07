//! Checks stored cloud saves against the hosted game's load path and prints one JSON summary.
//!
//! Input is JSON lines on stdin, one per stored save row:
//! `{"slot": 1, "revision": 12, "document": "<the stored text>"}`. `solomon-save-rows.py`
//! writes them from the Website database. Each document is loaded as the game loads it,
//! written as the host writes its next checkpoint, and loaded again (see `check`).
//!
//! The summary counts rows by slot, schema version and outcome, lists each distinct error
//! with its count, and names failing rows by slot, revision and the SHA-256 of their stored
//! text. Nothing else from a document is printed or logged: messages are redacted first
//! (see `redact`) and panic reports carry only a code location. The exit status is 0 once
//! the whole input has been read, whatever the saves' outcomes.

mod check;
mod redact;
mod summary;

use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use serde::Deserialize;
use solomon_launcher::host::HostContent;

use crate::summary::Summary;

const USAGE: &str = "Usage: solomon-save-check --assets CLIENT_DIRECTORY < rows.jsonl

Reads one {\"slot\", \"revision\", \"document\"} JSON object per line, loads each document
through the game's load path and prints a JSON summary without document text.
CLIENT_DIRECTORY is the game client the host serves, the directory that contains
assets/navigation: the archive's client/ directory, or SDR_GAME_ASSET_DIRECTORY.";

/// One stored save row. Extra fields, such as a user ID, make the line malformed.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    slot: u64,
    revision: u64,
    document: String,
}

fn main() -> ExitCode {
    let assets = match arguments() {
        Ok(Some(assets)) => assets,
        Ok(None) => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(error) => {
            eprintln!("solomon-save-check: {error}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match run(&assets) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("solomon-save-check: {error}");
            ExitCode::FAILURE
        }
    }
}

/// The client directory, or `None` when help was requested.
fn arguments() -> Result<Option<PathBuf>, String> {
    let mut assets = None;
    let mut arguments = std::env::args_os().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.to_str() {
            Some("--assets") => {
                assets = Some(PathBuf::from(arguments.next().ok_or("--assets needs a directory")?));
            }
            Some("--help" | "-h") => return Ok(None),
            _ => return Err(format!("unknown argument {}", argument.to_string_lossy())),
        }
    }
    let assets = assets.ok_or("--assets is required")?;
    // A wrong directory would fail every Boneyard save, which would read as a game failure.
    if !assets.join("assets/navigation").is_dir() {
        return Err(format!("{} has no assets/navigation directory", assets.display()));
    }
    Ok(Some(assets))
}

fn run(assets: &std::path::Path) -> Result<(), String> {
    let content = HostContent::load(assets)?;
    check::install_quiet_panic_hook();
    let mut summary = Summary::default();
    let mut input = io::stdin().lock();
    let mut line = Vec::new();
    let mut number = 0;
    loop {
        line.clear();
        let read = input
            .read_until(b'\n', &mut line)
            .map_err(|error| format!("cannot read standard input: {error}"))?;
        if read == 0 {
            break;
        }
        number += 1;
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        match serde_json::from_slice::<Row>(&line) {
            Ok(row) => {
                let checked = check::check(&row.document, &content);
                summary.add(row.slot, row.revision, &row.document, checked);
            }
            Err(_) => summary.add_malformed_line(number),
        }
    }
    let mut output = io::stdout().lock();
    serde_json::to_writer_pretty(&mut output, &summary.finish())
        .map_err(|error| format!("cannot write the summary: {error}"))?;
    writeln!(output).map_err(|error| format!("cannot write the summary: {error}"))
}
