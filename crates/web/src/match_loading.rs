//! Website `MatchLoadingScreen.tsx`: source art and lifecycle copy over the existing scene.
//! The coordinator owns progress and completion; visibility never delays admission.

use solomon_sim::menu::loading::MatchLoadingState;
use wasm_bindgen::JsCast;
use web_sys::{Document, HtmlElement, HtmlImageElement};

use crate::ui::{append, element, image};

pub struct MatchLoadingView {
    root: HtmlElement,
    art: HtmlImageElement,
    label: HtmlElement,
    track: HtmlElement,
    fill: HtmlElement,
    presented: Option<MatchLoadingState>,
}

impl MatchLoadingView {
    pub fn attach(document: &Document) -> Result<Self, String> {
        let root = element(document, "div", "match-loading-screen")?;
        root.set_hidden(true);
        root.set_attribute("aria-busy", "true").map_err(|_| "match loading status")?;
        let art = image(document, "match-loading-background", "match-loading-art")?;
        let scrim = element(document, "div", "match-loading-scrim")?;
        scrim.set_attribute("aria-hidden", "true").map_err(|_| "match loading scrim")?;
        let label = element(document, "div", "match-loading-label")?;
        label.set_attribute("role", "status").map_err(|_| "match loading label")?;
        label.set_attribute("aria-live", "polite").map_err(|_| "match loading live label")?;
        let track = element(document, "div", "match-loading-progress")?;
        for (name, value) in
            [("role", "progressbar"), ("aria-valuemin", "0"), ("aria-valuemax", "100")]
        {
            track.set_attribute(name, value).map_err(|_| "match loading progress semantics")?;
        }
        let fill = element(document, "div", "match-loading-progress-fill")?;
        append(&track, &fill)?;
        for child in [&art, &scrim, &label, &track] {
            append(&root, child)?;
        }
        append(&document.body().ok_or("match loading body")?, &root)?;
        Ok(Self {
            root,
            art: art.dyn_into().map_err(|_| "match loading image")?,
            label,
            track,
            fill,
            presented: None,
        })
    }

    pub fn update(
        &mut self,
        loading: Option<&MatchLoadingState>,
        now_ms: f64,
    ) -> Result<(), String> {
        self.root.set_hidden(loading.is_none());
        let Some(loading) = loading else {
            self.presented = None;
            return Ok(());
        };
        let art_ready = self.art.complete() && self.art.natural_width() > 0;
        let delay_elapsed = loading.should_present(now_ms);
        for (name, value) in [
            ("data-art-ready", art_ready),
            ("data-delay-elapsed", delay_elapsed),
            ("data-visible", art_ready && delay_elapsed),
        ] {
            self.root
                .set_attribute(name, if value { "true" } else { "false" })
                .map_err(|_| "match loading visibility")?;
        }
        if self.presented == Some(*loading) {
            return Ok(());
        }
        self.label.set_text_content(Some(loading.label()));
        self.root
            .set_attribute("data-flow", loading.flow.id())
            .map_err(|_| "match loading flow")?;
        self.root
            .set_attribute("data-stage", loading.stage.definition().id)
            .map_err(|_| "match loading stage")?;
        self.root
            .set_attribute("data-progress", &loading.progress.to_string())
            .map_err(|_| "match loading progress")?;
        self.track
            .set_attribute("aria-label", loading.label())
            .map_err(|_| "match loading progress label")?;
        self.track
            .set_attribute("aria-valuenow", &(loading.progress * 100.0).to_string())
            .map_err(|_| "match loading progress value")?;
        self.fill
            .style()
            .set_property("width", &format!("{}%", loading.progress * 100.0))
            .map_err(|_| "match loading fill")?;
        self.presented = Some(*loading);
        Ok(())
    }
}

impl Drop for MatchLoadingView {
    fn drop(&mut self) {
        self.root.remove();
    }
}
