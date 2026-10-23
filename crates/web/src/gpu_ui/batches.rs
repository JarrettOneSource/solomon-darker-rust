//! Commit native input in dispatch order before painting the final GPU frame.

use super::BrowserKey;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct DispatchEnd {
    pub events: usize,
    pub codes: usize,
    pub edits: usize,
    pub click_count: Option<u32>,
}

impl DispatchEnd {
    pub fn same_endpoint(self, other: Self) -> bool {
        self.events == other.events && self.codes == other.codes && self.edits == other.edits
    }
}

pub(super) struct InputPass {
    pub raw: egui::RawInput,
    pub codes: Vec<BrowserKey>,
    pub edits: Vec<super::edit::model::Receipt>,
    pub click_count: Option<u32>,
}

pub(super) fn into_passes(
    mut raw: egui::RawInput,
    codes: Vec<BrowserKey>,
    edits: Vec<super::edit::model::Receipt>,
    boundaries: Vec<DispatchEnd>,
) -> Vec<InputPass> {
    let mut events = std::mem::take(&mut raw.events).into_iter();
    let total = DispatchEnd {
        events: events.len(),
        codes: codes.len(),
        edits: edits.len(),
        click_count: None,
    };
    let mut edits = edits.into_iter();
    let mut codes = codes.into_iter();
    let mut previous = DispatchEnd::default();
    let mut passes = Vec::new();
    for end in boundaries {
        let mut input = raw.clone();
        input.events = events.by_ref().take(end.events - previous.events).collect();
        passes.push(InputPass {
            raw: input,
            codes: codes.by_ref().take(end.codes - previous.codes).collect(),
            edits: edits.by_ref().take(end.edits - previous.edits).collect(),
            click_count: end.click_count,
        });
        // Actions commit at the end of a controller's paint. Materialize its new
        // widgets/focus before the next actual dispatch (e.g. open field, type).
        passes.push(InputPass {
            raw: raw.clone(),
            codes: Vec::new(),
            edits: Vec::new(),
            click_count: None,
        });
        previous = end;
    }
    if !previous.same_endpoint(total) || passes.is_empty() {
        raw.events = events.collect();
        passes.push(InputPass {
            raw,
            codes: codes.collect(),
            edits: edits.collect(),
            click_count: None,
        });
    }
    passes
}
