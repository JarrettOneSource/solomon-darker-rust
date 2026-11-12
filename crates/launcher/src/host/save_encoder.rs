//! One owned hosted capture at a time. Later requests remain coalesced player IDs on
//! their authorities; this thread owns only plain values and the canonical save codec.

use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError};
use std::thread::{self, JoinHandle};

use super::hosted::{SaveCompletion, SaveJob};

pub(super) struct SaveEncoder {
    requests: Option<SyncSender<SaveJob>>,
    completions: Receiver<SaveCompletion>,
    worker: Option<JoinHandle<()>>,
    in_flight: bool,
}

impl SaveEncoder {
    pub fn start() -> Result<Self, String> {
        let (requests, receiver) = mpsc::sync_channel::<SaveJob>(1);
        let (completed, completions) = mpsc::channel();
        let worker = thread::Builder::new()
            .name("solomon-hosted-save".into())
            .spawn(move || {
                while let Ok(request) = receiver.recv() {
                    if completed.send(request.encode()).is_err() {
                        break;
                    }
                }
            })
            .map_err(|error| format!("Cannot start the hosted save encoder: {error}"))?;
        Ok(Self { requests: Some(requests), completions, worker: Some(worker), in_flight: false })
    }

    pub fn is_available(&self) -> bool {
        !self.in_flight
    }

    pub fn submit(&mut self, request: SaveJob) -> Result<(), String> {
        if self.in_flight {
            return Err("The hosted save encoder already owns a capture".into());
        }
        self.requests
            .as_ref()
            .ok_or("The hosted save encoder is closed")?
            .try_send(request)
            .map_err(|_| "The hosted save encoder stopped unexpectedly")?;
        self.in_flight = true;
        Ok(())
    }

    pub fn poll(&mut self) -> Result<Option<SaveCompletion>, String> {
        match self.completions.try_recv() {
            Ok(completion) => {
                self.in_flight = false;
                Ok(Some(completion))
            }
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => {
                Err("The hosted save encoder stopped unexpectedly".into())
            }
        }
    }

    pub fn finish(&mut self) -> Result<(), String> {
        self.requests.take();
        if let Some(worker) = self.worker.take() {
            worker.join().map_err(|_| "The hosted save encoder failed")?;
        }
        Ok(())
    }
}

impl Drop for SaveEncoder {
    fn drop(&mut self) {
        self.requests.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
