//! Current `hub-inventory-renderer.ts::renderChat` and `hubChatTextRuns`.
//! A click changes only the presentation clock; authored speech and authority stay intact.

use crate::native_ui_text::NativeUiTextRun;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HubChatScrollFrame {
    pub complete: bool,
    pub content_y: f64,
    pub travel: f64,
}

#[must_use]
pub fn hub_chat_scroll_frame(
    started_at_ms: f64,
    accelerated_at_ms: Option<f64>,
    now_ms: f64,
    content_height: f64,
) -> HubChatScrollFrame {
    let normal_elapsed_ms = accelerated_at_ms.unwrap_or(now_ms) - started_at_ms;
    let accelerated_elapsed_ms = accelerated_at_ms.map_or(0.0, |at| now_ms - at);
    let normal_ticks = (normal_elapsed_ms / 10.0).floor().max(0.0);
    let accelerated_ticks = (accelerated_elapsed_ms / 10.0).floor().max(0.0);
    let travel = normal_ticks * 0.125 + accelerated_ticks * 0.8;
    HubChatScrollFrame {
        complete: travel > content_height + 214.0,
        content_y: 214.0 - travel,
        travel,
    }
}

/// Asterisk toggles native italic shear, including an unmatched final delimiter.
#[must_use]
pub fn hub_chat_text_runs(source: &str) -> Vec<NativeUiTextRun<'_>> {
    let mut runs = Vec::new();
    for (index, text) in source.split('*').enumerate() {
        if !text.is_empty() {
            runs.push(NativeUiTextRun { italic: index % 2 == 1, ..NativeUiTextRun::new(text) });
        }
    }
    runs
}
