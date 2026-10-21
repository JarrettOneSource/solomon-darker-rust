//! Stock College admission title and cover over the accepted controller state.

use solomon_sim::Vec2;
use solomon_sim::hub::college_intro::{
    NativeCollegeIntroPhase, NativeCollegeIntroState, native_college_title_presentation,
};
use solomon_sim::viewport::game_viewport;
use web_sys::{Document, HtmlElement};

use crate::ui::{StockAtlas, append, element, style};

pub struct CollegeIntroOverlay {
    root: HtmlElement,
    cover: HtmlElement,
    presents: HtmlElement,
    title: HtmlElement,
}

impl CollegeIntroOverlay {
    pub async fn attach(document: &Document) -> Result<Self, String> {
        let atlas = StockAtlas::load("Title").await?;
        let root = element(document, "div", "college-intro-overlay")?;
        root.set_hidden(true);
        root.set_attribute("role", "img").map_err(|_| "College title role")?;
        let cover = element(document, "span", "college-intro-cover")?;
        let presents = atlas.sprite(document, 7, "college-intro-title")?;
        let title = atlas.sprite(document, 9, "college-intro-title")?;
        for child in [&cover, &presents, &title] {
            append(&root, child)?;
        }
        append(&document.body().ok_or("College title document body")?, &root)?;
        Ok(Self { root, cover, presents, title })
    }

    pub fn update(
        &self,
        state: Option<&NativeCollegeIntroState>,
        width: i32,
        height: i32,
    ) -> Result<(), String> {
        let Some(state) =
            state.filter(|state| state.phase == NativeCollegeIntroPhase::CourtyardWalk)
        else {
            self.root.set_hidden(true);
            return Ok(());
        };
        self.root.set_hidden(false);
        let viewport = game_viewport(Vec2::new(f64::from(width), f64::from(height)));
        let view = native_college_title_presentation(state.title_cursor, state.cover_alpha);
        self.root
            .set_attribute(
                "aria-label",
                if view.record == 7 { "Raptisoft Games presents" } else { "Solomon Dark" },
            )
            .map_err(|_| "College title label")?;
        self.root
            .set_attribute("data-college-intro-title-cursor", &state.title_cursor.to_string())
            .map_err(|_| "College title cursor")?;
        self.root
            .set_attribute("data-college-intro-title-record", &view.record.to_string())
            .map_err(|_| "College title record")?;
        style(&self.cover, &format!("opacity:{}", state.cover_alpha))?;
        for (record, title) in [(7, &self.presents), (9, &self.title)] {
            title.set_hidden(view.record != record);
            title
                .style()
                .set_property("top", &format!("{}px", view.y * viewport.display_scale))
                .map_err(|_| "College title baseline")?;
            title
                .style()
                .set_property("opacity", &view.alpha.to_string())
                .map_err(|_| "College title alpha")?;
            title
                .style()
                .set_property(
                    "transform",
                    &format!("translate(-50%, -50%) scale({})", viewport.display_scale),
                )
                .map_err(|_| "College title scale")?;
        }
        Ok(())
    }
}
