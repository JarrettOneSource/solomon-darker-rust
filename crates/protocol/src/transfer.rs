//! Chunked transfer of long strings, such as a Welcome or a save checkpoint, through a
//! four-chunk acknowledged window. UTF-16 units keep offsets and split surrogate pairs
//! identical across endpoints.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

pub const GAME_STRING_CHUNK_CHARACTERS: usize = 8 * 1024;
pub const GAME_STRING_CHUNK_WINDOW: usize = 4;
pub const GAME_STRING_STREAM_THRESHOLD: usize = 64 * 1024;
pub const GAME_WELCOME_PROGRESS_TIMEOUT_MS: u64 = 5_000;

/// One chunk of a transferred string: UTF-16 units starting at `offset` of `total_length`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameStringChunk {
    pub data: Vec<u16>,
    pub offset: usize,
    pub total_length: usize,
}

/// What an acknowledgement did to a transfer: nothing (stale), advanced it, or completed it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameStringAcknowledgement {
    Stale,
    Advanced,
    Complete,
}

/// The result of an acknowledgement and the chunks it allows the sender to send next.
pub struct GameStringTransferProgress {
    pub acknowledgement: GameStringAcknowledgement,
    pub chunks: Vec<GameStringChunk>,
}

/// The sending side of a string transfer, with at most `GAME_STRING_CHUNK_WINDOW` chunks
/// awaiting acknowledgement.
pub struct GameStringSender {
    data: Vec<u16>,
    outstanding: VecDeque<usize>,
    acknowledged_offset: usize,
    sent_offset: usize,
    closed: bool,
}

impl GameStringSender {
    pub fn new(data: Vec<u16>) -> Result<Self, String> {
        if data.is_empty() {
            return Err("String transfer cannot be empty".into());
        }
        Ok(Self {
            data,
            outstanding: VecDeque::new(),
            acknowledged_offset: 0,
            sent_offset: 0,
            closed: false,
        })
    }

    pub fn start(&mut self) -> Vec<GameStringChunk> {
        self.pump()
    }

    pub fn acknowledge(
        &mut self,
        next_offset: usize,
    ) -> Result<GameStringTransferProgress, String> {
        if self.closed || next_offset <= self.acknowledged_offset {
            return Ok(GameStringTransferProgress {
                acknowledgement: GameStringAcknowledgement::Stale,
                chunks: Vec::new(),
            });
        }
        if !self.outstanding.contains(&next_offset) {
            return Err("String acknowledgement exceeds a sent chunk".into());
        }
        self.acknowledged_offset = next_offset;
        self.outstanding.retain(|offset| *offset > next_offset);
        if next_offset == self.data.len() {
            self.close();
            return Ok(GameStringTransferProgress {
                acknowledgement: GameStringAcknowledgement::Complete,
                chunks: Vec::new(),
            });
        }
        Ok(GameStringTransferProgress {
            acknowledgement: GameStringAcknowledgement::Advanced,
            chunks: self.pump(),
        })
    }

    pub fn close(&mut self) {
        self.closed = true;
        self.data.clear();
        self.outstanding.clear();
    }

    fn pump(&mut self) -> Vec<GameStringChunk> {
        let mut chunks = Vec::new();
        while !self.closed
            && self.outstanding.len() < GAME_STRING_CHUNK_WINDOW
            && self.sent_offset < self.data.len()
        {
            let offset = self.sent_offset;
            let end = (offset + GAME_STRING_CHUNK_CHARACTERS).min(self.data.len());
            self.sent_offset = end;
            self.outstanding.push_back(end);
            chunks.push(GameStringChunk {
                data: self.data[offset..end].to_vec(),
                offset,
                total_length: self.data.len(),
            });
        }
        chunks
    }
}

struct ReceivingString {
    total_length: usize,
    data: Vec<u16>,
}

/// The receiving side of a string transfer: it assembles chunks in order and enforces a
/// byte limit on the finished string.
pub struct GameStringReceiver {
    current: Option<ReceivingString>,
    maximum_bytes: usize,
    field: String,
}

impl GameStringReceiver {
    #[must_use]
    pub fn new(maximum_bytes: usize, field: String) -> Self {
        Self { current: None, maximum_bytes, field }
    }

    /// No partial value escapes. The byte check replaces an isolated surrogate when
    /// counting UTF-8 bytes, but the returned string keeps its exact units.
    pub fn accept(&mut self, chunk: &GameStringChunk) -> Result<Option<Vec<u16>>, String> {
        if chunk.total_length == 0
            || chunk.total_length > self.maximum_bytes
            || chunk.data.is_empty()
            || chunk.data.len() > GAME_STRING_CHUNK_CHARACTERS
        {
            return Err("String chunk exceeds its transfer bounds".into());
        }
        if self.current.is_none() {
            if chunk.offset != 0 {
                return Err("String transfer is missing its first chunk".into());
            }
            self.current =
                Some(ReceivingString { total_length: chunk.total_length, data: Vec::new() });
        }
        let transfer = self.current.as_mut().expect("the first chunk creates its transfer");
        if chunk.total_length != transfer.total_length
            || chunk.offset != transfer.data.len()
            || chunk
                .offset
                .checked_add(chunk.data.len())
                .is_none_or(|end| end > transfer.total_length)
        {
            return Err("String chunk does not match its pending transfer".into());
        }
        transfer.data.extend(&chunk.data);
        if transfer.data.len() != transfer.total_length {
            return Ok(None);
        }
        if String::from_utf16_lossy(&transfer.data).len() > self.maximum_bytes {
            return Err(format!("{} may contain at most {} bytes", self.field, self.maximum_bytes));
        }
        Ok(self.current.take().map(|transfer| transfer.data))
    }

    pub fn close(&mut self) {
        self.current = None;
    }
}
