//! Counts of checked rows and the JSON summary printed at the end of a run. Rows are named
//! only by slot, revision and the SHA-256 of their stored text.

use std::collections::BTreeMap;

use serde::{Serialize, Serializer};
use sha2::{Digest, Sha256};

use crate::check::{Checked, Outcome, SchemaBucket};

/// Rows per outcome. Every outcome is always present, including zero counts.
#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct OutcomeCounts {
    ok: u64,
    parse_error: u64,
    restore_error: u64,
    round_trip_error: u64,
    oversize: u64,
    panic: u64,
}

/// One distinct redacted message and the rows that produced it.
#[derive(Serialize)]
struct ErrorGroup {
    outcome: &'static str,
    message: String,
    count: usize,
    rows: Vec<FailedRow>,
}

#[derive(Serialize)]
struct FailedRow {
    slot: u64,
    revision: u64,
    sha256: String,
}

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    rows: u64,
    /// Input lines that were not `{"slot", "revision", "document"}` objects, by line number.
    malformed_lines: Vec<u64>,
    totals: OutcomeCounts,
    by_slot: BTreeMap<u64, OutcomeCounts>,
    by_slot_and_schema_version: BTreeMap<u64, BTreeMap<SchemaBucket, OutcomeCounts>>,
    errors: Vec<ErrorGroup>,
    #[serde(skip)]
    failures: BTreeMap<(&'static str, String), Vec<FailedRow>>,
}

impl Summary {
    pub fn add_malformed_line(&mut self, line: u64) {
        self.malformed_lines.push(line);
    }

    pub fn add(&mut self, slot: u64, revision: u64, document: &str, checked: Checked) {
        self.rows += 1;
        let by_schema = self.by_slot_and_schema_version.entry(slot).or_default();
        for counts in [
            &mut self.totals,
            self.by_slot.entry(slot).or_default(),
            by_schema.entry(checked.schema).or_default(),
        ] {
            counts.add(&checked.outcome);
        }
        let (outcome, message) = match checked.outcome {
            Outcome::Ok => return,
            Outcome::ParseError(message) => ("parseError", message),
            Outcome::RestoreError(message) => ("restoreError", message),
            Outcome::RoundTripError(message) => ("roundTripError", message),
            Outcome::Oversize => ("oversize", "game save exceeds its size limit".into()),
            Outcome::Panic(message) => ("panic", message),
        };
        let sha256 =
            Sha256::digest(document.as_bytes()).iter().map(|byte| format!("{byte:02x}")).collect();
        self.failures.entry((outcome, message)).or_default().push(FailedRow {
            slot,
            revision,
            sha256,
        });
    }

    /// Orders the distinct messages by how many rows they affect.
    pub fn finish(mut self) -> Self {
        self.errors = std::mem::take(&mut self.failures)
            .into_iter()
            .map(|((outcome, message), rows)| ErrorGroup {
                outcome,
                message,
                count: rows.len(),
                rows,
            })
            .collect();
        self.errors.sort_by_key(|group| std::cmp::Reverse(group.count));
        self
    }
}

impl OutcomeCounts {
    fn add(&mut self, outcome: &Outcome) {
        let count = match outcome {
            Outcome::Ok => &mut self.ok,
            Outcome::ParseError(_) => &mut self.parse_error,
            Outcome::RestoreError(_) => &mut self.restore_error,
            Outcome::RoundTripError(_) => &mut self.round_trip_error,
            Outcome::Oversize => &mut self.oversize,
            Outcome::Panic(_) => &mut self.panic,
        };
        *count += 1;
    }
}

/// Schema versions are summary keys: the number itself, or `newer`, `invalid` or
/// `unreadable`.
impl Serialize for SchemaBucket {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Version(version) => serializer.collect_str(version),
            Self::Newer => serializer.serialize_str("newer"),
            Self::Invalid => serializer.serialize_str("invalid"),
            Self::Unreadable => serializer.serialize_str("unreadable"),
        }
    }
}
