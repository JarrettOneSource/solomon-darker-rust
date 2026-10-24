//! Commit native input in dispatch order before painting the final GPU frame.

use super::BrowserKey;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct DispatchEnd {
    pub events: usize,
    pub codes: usize,
    pub edits: usize,
    pub click_count: Option<u32>,
    pub controller: bool,
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
    pub controller: bool,
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
        controller: false,
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
            controller: end.controller,
        });
        // Actions commit at the end of a controller's paint. Materialize its new
        // widgets/focus before the next actual dispatch (e.g. open field, type).
        passes.push(InputPass {
            raw: raw.clone(),
            codes: Vec::new(),
            edits: Vec::new(),
            click_count: None,
            controller: false,
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
            controller: false,
        });
    }
    passes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sampled_controller_provenance_belongs_only_to_its_actual_dispatch() {
        let key = |key| egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        let passes = into_passes(
            egui::RawInput {
                events: vec![key(egui::Key::ArrowLeft), key(egui::Key::ArrowUp)],
                ..Default::default()
            },
            Vec::new(),
            Vec::new(),
            vec![
                DispatchEnd { events: 1, ..Default::default() },
                DispatchEnd { events: 2, controller: true, ..Default::default() },
            ],
        );
        assert_eq!(
            passes.iter().map(|pass| pass.controller).collect::<Vec<_>>(),
            [false, false, true, false],
            "native keys with no physical key are not inferred to be controller input"
        );
        assert_eq!(passes[0].raw.events, [key(egui::Key::ArrowLeft)]);
        assert_eq!(passes[2].raw.events, [key(egui::Key::ArrowUp)]);
        assert!(passes[1].raw.events.is_empty() && passes[3].raw.events.is_empty());
    }
}
