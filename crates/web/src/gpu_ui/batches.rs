//! Commit native input in dispatch order before painting the final GPU frame.

use super::BrowserKey;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct DispatchEnd {
    pub events: usize,
    pub codes: usize,
}

pub(super) struct InputPass {
    pub raw: egui::RawInput,
    pub codes: Vec<BrowserKey>,
}

pub(super) fn into_passes(
    mut raw: egui::RawInput,
    codes: Vec<BrowserKey>,
    boundaries: Vec<DispatchEnd>,
) -> Vec<InputPass> {
    let mut events = std::mem::take(&mut raw.events).into_iter();
    let total = DispatchEnd { events: events.len(), codes: codes.len() };
    let mut codes = codes.into_iter();
    let mut previous = DispatchEnd::default();
    let mut passes = Vec::new();
    for end in boundaries {
        let mut input = raw.clone();
        input.events = events.by_ref().take(end.events - previous.events).collect();
        passes.push(InputPass {
            raw: input,
            codes: codes.by_ref().take(end.codes - previous.codes).collect(),
        });
        // Actions commit at the end of a controller's paint. Materialize its new
        // widgets/focus before the next actual dispatch (e.g. open field, type).
        passes.push(InputPass { raw: raw.clone(), codes: Vec::new() });
        previous = end;
    }
    if previous != total || passes.is_empty() {
        raw.events = events.collect();
        passes.push(InputPass { raw, codes: codes.collect() });
    }
    passes
}
