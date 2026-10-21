//! Source `host/run-archive-store.ts`: one ordered worker, atomic non-overwrite
//! gzip/summary publication, and the same 30-day private-artifact retention policy.

use std::collections::BTreeSet;
use std::fs::{self, DirBuilder, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime};

use flate2::{Compression, read::GzDecoder, write::GzEncoder};
use serde::Serialize;

use super::encoding::{RunArchiveFile, archive_players, decode_archive, serialize_archive};
use super::recorder::RunArchive;
use crate::host::runtime_events::{RuntimeEventEntry, RuntimeEventSink};

const RETENTION: Duration = Duration::from_secs(30 * 24 * 60 * 60);
const PRUNE_INTERVAL: Duration = Duration::from_secs(60 * 60);
const TEMPORARY_ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunArchiveReceipt {
    pub file: String,
    pub bytes: usize,
    pub sha256: String,
    pub serialization_ms: f64,
    pub maximum_serialization_slice_ms: f64,
}
enum Job {
    Save { id: String, archive: Box<RunArchive> },
    Prune(SystemTime),
    Close,
}

#[derive(Clone)]
pub struct RunArchiveSink(Sender<Job>);
impl RunArchiveSink {
    pub fn save(&self, archive: RunArchive) -> Result<(), String> {
        self.0
            .send(Job::Save { id: random_uuid()?, archive: Box::new(archive) })
            .map_err(|_| "The run archive worker is closed".into())
    }
}
pub struct RunArchiveStore {
    sink: RunArchiveSink,
    worker: Option<JoinHandle<()>>,
    next_prune: Instant,
}
impl RunArchiveStore {
    pub fn new(directory: PathBuf, events: Option<RuntimeEventSink>) -> Result<Self, String> {
        let (sender, receiver) = mpsc::channel();
        let (ready, started) = mpsc::sync_channel(1);
        let worker = thread::Builder::new().name("run-archives".into()).spawn(move || {
            if let Err(error) = prune_directory(&directory, SystemTime::now()) { log_failure(events.as_ref(), &error); }
            let _ = ready.send(());
            while let Ok(job) = receiver.recv() {
                match job {
                    Job::Save { id, archive } => match write_archive(&directory, &id, &archive) {
                        Ok(receipt) => {
                            let details = serde_json::json!({"file":receipt.file,"bytes":receipt.bytes,"sha256":receipt.sha256,
                                "serializationMs":receipt.serialization_ms,"maximumSerializationSliceMs":receipt.maximum_serialization_slice_ms,
                                "runId":archive.run_id,"sessionId":archive.session_id,"endReason":archive.end_reason});
                            if let Some(events) = &events { log_event(events, "run_archive.saved", "A completed run diagnostic archive was saved.", details); }
                        }
                        Err(error) => log_failure(events.as_ref(), &error),
                    },
                    Job::Prune(now) => if let Err(error) = prune_directory(&directory, now) { log_failure(events.as_ref(), &error); },
                    Job::Close => break,
                }
            }
        }).map_err(|error| format!("Cannot start run archive worker: {error}"))?;
        started.recv().map_err(|_| "The run archive worker failed during startup")?;
        Ok(Self {
            sink: RunArchiveSink(sender),
            worker: Some(worker),
            next_prune: Instant::now() + PRUNE_INTERVAL,
        })
    }
    pub fn sink(&self) -> RunArchiveSink {
        self.sink.clone()
    }
    pub fn prune_if_due(&mut self) {
        if Instant::now() < self.next_prune {
            return;
        }
        self.next_prune = Instant::now() + PRUNE_INTERVAL;
        let _ = self.sink.0.send(Job::Prune(SystemTime::now()));
    }
    pub fn close(&mut self) {
        if let Some(worker) = self.worker.take() {
            let _ = self.sink.0.send(Job::Close);
            worker.join().expect("the archive worker handles serialization and I/O failures");
        }
    }
}
impl Drop for RunArchiveStore {
    fn drop(&mut self) {
        self.close();
    }
}

fn log_failure(events: Option<&RuntimeEventSink>, reason: &str) {
    eprintln!("run_archive.failed: {reason}");
    if let Some(events) = events {
        log_event(
            events,
            "run_archive.failed",
            "A run diagnostic archive operation failed.",
            serde_json::json!({"errorMessage":reason,"errorName":"Error"}),
        );
    }
}
fn log_event(events: &RuntimeEventSink, event: &str, message: &str, details: serde_json::Value) {
    events.publish(RuntimeEventEntry {
        component: "session-supervisor".into(),
        event: event.into(),
        message: message.into(),
        details: serde_json::from_value(details)
            .expect("archive event fields are finite JSON values"),
        occurred_at_utc: Some(super::recorder::utc_now()),
    });
}

fn random_uuid() -> Result<String, String> {
    let mut bytes = crate::host::authority::random_bytes::<16>()?;
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let mut id = String::with_capacity(36);
    use std::fmt::Write;
    for (index, byte) in bytes.into_iter().enumerate() {
        if matches!(index, 4 | 6 | 8 | 10) {
            id.push('-');
        }
        write!(id, "{byte:02x}").expect("writing into a String succeeds");
    }
    Ok(id)
}
fn create_directory(path: &Path) -> Result<(), String> {
    let mut builder = DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path).map_err(|error| format!("Cannot create {}: {error}", path.display()))
}
fn write_private(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file =
        options.open(path).map_err(|error| format!("Cannot create {}: {error}", path.display()))?;
    file.write_all(bytes).map_err(|error| format!("Cannot write {}: {error}", path.display()))
}
fn temporary_directory(directory: &Path) -> Result<PathBuf, String> {
    loop {
        let bytes = crate::host::authority::random_bytes::<6>()?;
        let suffix: String = bytes
            .into_iter()
            .map(|byte| {
                char::from(TEMPORARY_ALPHABET[usize::from(byte) % TEMPORARY_ALPHABET.len()])
            })
            .collect();
        let path = directory.join(format!(".writing-{suffix}"));
        let mut builder = DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        match builder.create(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(format!("Cannot create {}: {error}", path.display())),
        }
    }
}
fn archive_file(name: &str) -> bool {
    let Some(id) = name.strip_suffix(".sdrrun.gz") else {
        return false;
    };
    id.len() == 36
        && id
            .bytes()
            .all(|byte| byte == b'-' || byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
fn temporary_file(name: &str) -> bool {
    name.strip_prefix(".writing-").is_some_and(|suffix| {
        suffix.len() == 6 && suffix.bytes().all(|byte| byte.is_ascii_alphanumeric())
    })
}

pub fn write_archive(
    directory: &Path,
    id: &str,
    archive: &RunArchive,
) -> Result<RunArchiveReceipt, String> {
    let file = format!("{id}.sdrrun.gz");
    if !archive_file(&file) {
        return Err("Run archive id is invalid".into());
    }
    let (bytes, metrics) = serialize_archive(archive, id)?;
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&bytes).map_err(|error| error.to_string())?;
    let compressed = encoder.finish().map_err(|error| error.to_string())?;
    let receipt = RunArchiveReceipt {
        file: file.clone(),
        bytes: compressed.len(),
        sha256: solomon_modding::assets::sha256(&compressed),
        serialization_ms: metrics.total_ms,
        maximum_serialization_slice_ms: metrics.maximum_slice_ms,
    };
    let summary = serde_json::to_vec(&serde_json::json!({"file":receipt.file,"bytes":receipt.bytes,"sha256":receipt.sha256,
        "serializationMs":receipt.serialization_ms,"maximumSerializationSliceMs":receipt.maximum_serialization_slice_ms,
        "id":id,"runId":archive.run_id,"sessionId":archive.session_id,"revision":archive.revision,"protocolVersion":solomon_protocol::PROTOCOL_VERSION,
        "endedAtUtc":archive.ended_at_utc,"endReason":archive.end_reason,"boneyard":archive.loaded_boneyard.name,
        "players":archive_players(archive),"lastAliveTick":archive.last_alive.as_ref().map(|row| row.state.checkpoint.ticks),
        "worstTick":archive.worst_tick.state.checkpoint.ticks,"worstTickMs":archive.worst_tick.tick_ms,"performance":archive.performance})).map_err(|error| error.to_string())?;
    create_directory(directory)?;
    let temporary = temporary_directory(directory)?;
    let result = (|| {
        write_private(&temporary.join("capture"), &compressed)?;
        write_private(&temporary.join("summary"), &summary)?;
        fs::hard_link(temporary.join("capture"), directory.join(&file))
            .map_err(|error| error.to_string())?;
        fs::hard_link(temporary.join("summary"), directory.join(format!("{file}.json")))
            .map_err(|error| error.to_string())?;
        Ok(receipt)
    })();
    let cleanup = fs::remove_dir_all(&temporary).map_err(|error| error.to_string());
    match result {
        Err(error) => Err(error),
        Ok(receipt) => cleanup.map(|()| receipt),
    }
}

pub fn read_run_archive(path: &Path) -> Result<RunArchiveFile, String> {
    // As in Source, only private operator-owned captures enter this reader; it is
    // not an upload/import route and cannot become a client continuation.
    let compressed = fs::File::open(path).map_err(|error| error.to_string())?;
    let mut bytes = Vec::new();
    GzDecoder::new(compressed).read_to_end(&mut bytes).map_err(|error| error.to_string())?;
    decode_archive(&bytes)
}

pub fn prune_directory(directory: &Path, now: SystemTime) -> Result<usize, String> {
    create_directory(directory)?;
    let names: BTreeSet<_> = fs::read_dir(directory)
        .map_err(|error| error.to_string())?
        .map(|entry| entry.map(|entry| entry.file_name().to_string_lossy().into_owned()))
        .collect::<Result<_, _>>()
        .map_err(|error| error.to_string())?;
    let cutoff = now.checked_sub(RETENTION).unwrap_or(SystemTime::UNIX_EPOCH);
    let mut removed = 0;
    for name in &names {
        let temporary = temporary_file(name);
        if (!temporary && !archive_file(name)) || names.contains(&format!("{name}.keep")) {
            continue;
        }
        let path = directory.join(name);
        if fs::metadata(&path)
            .and_then(|value| value.modified())
            .map_err(|error| error.to_string())?
            >= cutoff
        {
            continue;
        }
        if temporary {
            let metadata = fs::symlink_metadata(&path).map_err(|error| error.to_string())?;
            if metadata.is_dir() { fs::remove_dir_all(&path) } else { fs::remove_file(&path) }
                .map_err(|error| error.to_string())?;
        } else {
            fs::remove_file(&path).map_err(|error| error.to_string())?;
            match fs::remove_file(directory.join(format!("{name}.json"))) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.to_string()),
            }
        }
        removed += 1;
    }
    Ok(removed)
}
