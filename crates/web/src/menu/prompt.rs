//! The two title confirmations share the pinned `title-menu-prompt.ts` stock message.

use web_sys::{Document, HtmlElement};

use crate::ui::{StockAtlas, append, element, message_frame, native_button, style};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TitlePromptKind {
    Tutorial,
    KillWizard,
}

pub(super) struct TitlePrompt {
    pub root: HtmlElement,
    stage: HtmlElement,
    heading: HtmlElement,
    body: HtmlElement,
    pub kind: Option<TitlePromptKind>,
}

impl TitlePrompt {
    pub async fn new(document: &Document) -> Result<Self, String> {
        let atlas = StockAtlas::load("UI").await?;
        let root = element(document, "section", "title-prompt")?;
        root.set_hidden(true);
        root.set_attribute("role", "dialog").map_err(|_| "title prompt role")?;
        root.set_attribute("aria-modal", "true").map_err(|_| "title prompt modal")?;
        root.set_attribute("aria-describedby", "title-prompt-body")
            .map_err(|_| "title prompt description")?;
        let stage = element(document, "div", "title-prompt-stage")?;
        append(&stage, &message_frame(document, &atlas, 500.0, 362.0)?)?;
        let heading = element(document, "h2", "title-prompt-heading")?;
        let body = element(document, "p", "title-prompt-body")?;
        body.set_id("title-prompt-body");
        append(&stage, &heading)?;
        append(&stage, &body)?;
        for (label, action, left) in
            [("YES", "prompt-primary", 45.0), ("NO", "prompt-secondary", 261.0)]
        {
            let button = native_button(document, label, action)?;
            style(&button, &format!("position:absolute;left:{left}px;top:216px;width:200px"))?;
            if action == "prompt-primary" {
                button
                    .set_attribute("data-game-default-focus", "true")
                    .map_err(|_| "title prompt focus")?;
            }
            append(&stage, &button)?;
        }
        append(&root, &stage)?;
        Ok(Self { root, stage, heading, body, kind: None })
    }

    pub fn show(&mut self, kind: TitlePromptKind) -> Result<(), String> {
        let (title, body) = match kind {
            TitlePromptKind::Tutorial => (
                "PLAY THE TUTORIAL?",
                "Learn the controls and confront\nSolomon Dark before beginning your\nfirst game.",
            ),
            TitlePromptKind::KillWizard => (
                "Kill character?",
                "Starting a new game will kill off your\ncurrent game and character (Lucritius\nwill scavenge his equipment)!\nAre you sure you want to do this?",
            ),
        };
        self.heading.set_text_content(Some(title));
        self.body.set_text_content(Some(body));
        self.root.set_attribute("aria-label", title).map_err(|_| "title prompt name")?;
        self.root.set_hidden(false);
        self.kind = Some(kind);
        Ok(())
    }

    pub fn hide(&mut self) {
        self.root.set_hidden(true);
        self.kind = None;
    }

    pub fn resize(&self, width: i32, height: i32) -> Result<(), String> {
        let native = (f64::from(width) / 1600.0).min(f64::from(height) / 900.0);
        let scale = if native < 44.0 / 69.0 {
            ((f64::from(width) - 24.0) / 500.0).min((f64::from(height) - 24.0) / 500.0)
        } else {
            native
        };
        style(
            &self.stage,
            &format!(
                "left:{}px;top:{}px;transform:scale({scale})",
                f64::from(width) / 2.0 - 250.0 * scale,
                f64::from(height) / 2.0 - 182.0 * scale,
            ),
        )
    }
}
