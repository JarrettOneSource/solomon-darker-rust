//! State transitions from Website `save/game-save-coordinator.ts`.
//! One writer owns the revision. Arrivals during that write share the newest pending
//! document, while each accepted checkpoint keeps its own completion outcome.

use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

use crate::storage::StoredSave;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum SaveOutcome {
    Pending,
    Committed,
    Failed(String),
}

struct Accepted {
    document: String,
    receipt: SaveReceipt,
}

#[derive(Clone, Debug)]
pub(super) struct SaveReceipt(Rc<RefCell<SaveOutcome>>);

impl SaveReceipt {
    fn pending() -> Self {
        Self(Rc::new(RefCell::new(SaveOutcome::Pending)))
    }

    pub fn outcome(&self) -> SaveOutcome {
        self.0.borrow().clone()
    }
}

pub(super) struct SaveBatch {
    pub document: String,
    pub revision: u64,
    pub unchanged: bool,
    receipts: Vec<SaveReceipt>,
}

pub(super) struct HostedSaveCoordinator {
    pub record: Option<StoredSave>,
    stream: Option<u64>,
    sealed: bool,
    last_sequence: u64,
    accepted: BTreeMap<u64, Accepted>,
    active: bool,
    ready: Option<SaveBatch>,
    pending: Option<SaveBatch>,
    last_outcome: SaveReceipt,
}

impl HostedSaveCoordinator {
    pub fn new(record: Option<StoredSave>) -> Self {
        Self {
            record,
            stream: None,
            sealed: false,
            last_sequence: 0,
            accepted: BTreeMap::new(),
            active: false,
            ready: None,
            pending: None,
            last_outcome: SaveReceipt(Rc::new(RefCell::new(SaveOutcome::Committed))),
        }
    }

    pub fn accept(
        &mut self,
        stream: u64,
        sequence: u64,
        document: String,
    ) -> Result<SaveReceipt, String> {
        if self.stream.is_some_and(|current| stream < current) {
            return Err(format!("Game save checkpoint stream {stream} is stale."));
        }
        if self.stream != Some(stream) {
            self.stream = Some(stream);
            self.sealed = false;
            self.last_sequence = 0;
            self.accepted.clear();
        }
        if self.sealed {
            return Err(format!("Game save checkpoint stream {stream} is retired."));
        }
        if let Some(previous) = self.accepted.get(&sequence) {
            if previous.document != document {
                return Err(format!("Game save checkpoint {sequence} changed within one stream."));
            }
            return Ok(previous.receipt.clone());
        }
        if sequence <= self.last_sequence {
            return Err(format!("Game save checkpoint {sequence} arrived out of order."));
        }
        self.last_sequence = sequence;
        self.prune();
        let receipt = SaveReceipt::pending();
        self.accepted
            .insert(sequence, Accepted { document: document.clone(), receipt: receipt.clone() });
        self.enqueue(document, receipt.clone());
        Ok(receipt)
    }

    pub fn replace(&mut self, document: String) -> SaveReceipt {
        self.sealed = self.stream.is_some();
        self.accepted.clear();
        let receipt = SaveReceipt::pending();
        self.enqueue(document, receipt.clone());
        receipt
    }

    fn enqueue(&mut self, document: String, receipt: SaveReceipt) {
        self.last_outcome = receipt.clone();
        // Source accept() begins the first write synchronously. Reserve it before
        // returning to the browser's message drain, even though I/O starts after it.
        let slot =
            if !self.active && self.ready.is_none() { &mut self.ready } else { &mut self.pending };
        let pending = slot.get_or_insert_with(|| SaveBatch {
            document: String::new(),
            revision: 0,
            unchanged: false,
            receipts: Vec::new(),
        });
        pending.document = document;
        pending.receipts.push(receipt);
    }

    pub fn take_write(&mut self) -> Option<SaveBatch> {
        if self.active {
            return None;
        }
        let mut pending = self.ready.take()?;
        pending.revision = self.record.as_ref().map_or(0, |record| record.revision);
        pending.unchanged =
            self.record.as_ref().is_some_and(|record| record.document == pending.document);
        self.active = true;
        Some(pending)
    }

    pub fn complete(&mut self, batch: SaveBatch, result: Result<u64, String>) {
        self.active = false;
        let outcome = match result {
            Ok(revision) => {
                self.record = Some(StoredSave { document: batch.document, revision });
                SaveOutcome::Committed
            }
            Err(error) => SaveOutcome::Failed(error),
        };
        for receipt in batch.receipts {
            *receipt.0.borrow_mut() = outcome.clone();
        }
        self.ready = self.pending.take();
        self.prune();
    }

    pub fn receipt(&self, sequence: u64) -> Option<SaveReceipt> {
        self.accepted.get(&sequence).map(|checkpoint| checkpoint.receipt.clone())
    }

    /// Source `idle()` captures the latest requested write, including a failed one.
    pub fn idle(&self) -> SaveReceipt {
        self.last_outcome.clone()
    }

    pub fn document(&self, sequence: u64) -> Option<&str> {
        self.accepted.get(&sequence).map(|checkpoint| checkpoint.document.as_str())
    }

    fn prune(&mut self) {
        self.accepted.retain(|sequence, checkpoint| {
            *sequence == self.last_sequence || checkpoint.receipt.outcome() == SaveOutcome::Pending
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deployment_without_a_checkpoint_waits_for_the_latest_write_outcome() {
        let mut saves = HostedSaveCoordinator::new(None);
        assert_eq!(saves.idle().outcome(), SaveOutcome::Committed);
        saves.replace("pending".into());
        let idle = saves.idle();
        assert_eq!(idle.outcome(), SaveOutcome::Pending);
        let batch = saves.take_write().unwrap();
        saves.complete(batch, Err("storage unavailable".into()));
        assert_eq!(idle.outcome(), SaveOutcome::Failed("storage unavailable".into()));
        assert_eq!(saves.idle().outcome(), idle.outcome());
        saves.replace("retry".into());
        let batch = saves.take_write().unwrap();
        saves.complete(batch, Ok(1));
        assert_eq!(saves.idle().outcome(), SaveOutcome::Committed);
        assert_eq!(idle.outcome(), SaveOutcome::Failed("storage unavailable".into()));
    }

    #[test]
    fn first_checkpoint_keeps_its_write_when_more_events_arrive_before_browser_io() {
        let mut saves = HostedSaveCoordinator::new(None);
        saves.accept(7, 1, "active-one".into()).unwrap();
        saves.accept(7, 2, "active-two".into()).unwrap();
        saves.replace("profile-only".into());
        let first = saves.take_write().unwrap();
        assert_eq!(first.document, "active-one");
        saves.complete(first, Ok(1));
        let replacement = saves.take_write().unwrap();
        assert_eq!((replacement.document.as_str(), replacement.revision), ("profile-only", 1));
    }

    #[test]
    fn checkpoints_coalesce_behind_the_active_revision_and_settle_together() {
        let mut saves = HostedSaveCoordinator::new(None);
        saves.accept(1, 1, "first".into()).unwrap();
        let first = saves.take_write().unwrap();
        let second = saves.accept(1, 2, "second".into()).unwrap();
        saves.accept(1, 3, "third".into()).unwrap();
        assert!(saves.take_write().is_none());
        saves.complete(first, Ok(1));
        let last = saves.take_write().unwrap();
        assert_eq!((last.document.as_str(), last.revision), ("third", 1));
        assert_eq!(second.outcome(), SaveOutcome::Pending);
        saves.complete(last, Ok(2));
        assert_eq!(second.outcome(), SaveOutcome::Committed);
        assert_eq!(saves.receipt(3).unwrap().outcome(), SaveOutcome::Committed);
    }

    #[test]
    fn failures_do_not_advance_revision_or_acknowledge_a_checkpoint() {
        let mut saves = HostedSaveCoordinator::new(None);
        saves.accept(1, 1, "first".into()).unwrap();
        let first = saves.take_write().unwrap();
        saves.complete(first, Err("revision conflict".into()));
        assert!(saves.record.is_none());
        assert_eq!(
            saves.receipt(1).unwrap().outcome(),
            SaveOutcome::Failed("revision conflict".into())
        );
        saves.accept(1, 1, "first".into()).unwrap();
        assert!(saves.take_write().is_none());
        saves.accept(1, 2, "next".into()).unwrap();
        assert_eq!(saves.take_write().unwrap().revision, 0);
    }

    #[test]
    fn replacement_seals_the_stream_and_duplicate_checkpoints_cannot_change() {
        let mut saves = HostedSaveCoordinator::new(None);
        saves.accept(2, 1, "first".into()).unwrap();
        assert!(saves.accept(2, 1, "changed".into()).unwrap_err().contains("changed"));
        assert!(saves.accept(1, 2, "stale".into()).unwrap_err().contains("stale"));
        let first = saves.take_write().unwrap();
        saves.replace("retired".into());
        assert!(saves.accept(2, 2, "late".into()).unwrap_err().contains("retired"));
        saves.complete(first, Ok(1));
        let replacement = saves.take_write().unwrap();
        assert_eq!((replacement.document.as_str(), replacement.revision), ("retired", 1));
        saves.complete(replacement, Ok(2));
        saves.accept(3, 1, "retired".into()).unwrap();
        assert!(saves.take_write().unwrap().unchanged);
    }
}
