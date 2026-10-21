//! Native spectator status panels from `NativeSpectatorStatus.tsx`.
//! Bitmap pens and authored UI rails use the shared retained stock atlases.

use solomon_sim::boneyard::spectator::{
    BoneyardSpectatorStatusPresentation, NativeSpectatorHudLayout, NativeSpectatorHudRail,
    native_spectator_hud_layout,
};
use solomon_sim::native_ui_text::{NativeUiFontName, NativeUiTextAlign, NativeUiTextSpec};
use solomon_sim::viewport::GameViewport;
use web_sys::{Document, HtmlElement};

use crate::ui::{StockAtlas, append, element, style};

pub struct SpectatorStatusView {
    document: Document,
    root: HtmlElement,
    atlas: StockAtlas,
    previous: Option<(BoneyardSpectatorStatusPresentation, GameViewport)>,
}

impl SpectatorStatusView {
    pub async fn attach(document: &Document) -> Result<Self, String> {
        let root = element(document, "div", "boneyard-spectator-status")?;
        for (name, value) in [
            ("role", "status"),
            ("aria-atomic", "true"),
            ("aria-live", "polite"),
            ("data-native-font", "Fonts.93-184"),
            ("data-native-ui-records", "10,79,107,108,109,110"),
        ] {
            root.set_attribute(name, value).map_err(|_| "spectator status attribute")?;
        }
        root.set_hidden(true);
        document
            .body()
            .ok_or("no body for spectator status")?
            .append_child(&root)
            .map_err(|_| "spectator status mount")?;
        Ok(Self {
            document: document.clone(),
            root,
            atlas: StockAtlas::load("UI").await?,
            previous: None,
        })
    }

    pub fn hide(&mut self) {
        self.root.set_hidden(true);
        self.previous = None;
    }

    pub fn update(
        &mut self,
        status: Option<&BoneyardSpectatorStatusPresentation>,
        viewport: GameViewport,
    ) -> Result<(), String> {
        let Some(status) = status else {
            self.hide();
            return Ok(());
        };
        if self.previous.as_ref().is_some_and(|(old, size)| old == status && *size == viewport) {
            return Ok(());
        }
        self.root.set_hidden(false);
        self.root.set_text_content(None);
        let layout = native_spectator_hud_layout(viewport.logical_size);
        let area = layout.surface;
        style(
            &self.root,
            &format!(
                "position:absolute;z-index:10001;overflow:visible;pointer-events:none;transform-origin:0 0;transform:scale({});left:{}px;top:{}px;width:{}px;height:{}px",
                viewport.display_scale,
                area.x * viewport.display_scale,
                area.y * viewport.display_scale,
                area.width,
                area.height
            ),
        )?;
        for (name, value) in [
            ("data-display-text", status.display_text.as_str()),
            ("data-run-id", status.run_id.as_str()),
            ("data-target-player-id", status.target_player_id.as_deref().unwrap_or("")),
            ("aria-label", status.accessible_label.as_str()),
        ] {
            self.root.set_attribute(name, value).map_err(|_| "spectator accepted status")?;
        }
        self.panel(&self.root, &layout, &status.display_text)?;
        let respawn = element(&self.document, "div", "boneyard-spectator-respawn-status")?;
        respawn
            .set_attribute("aria-hidden", "true")
            .map_err(|_| "spectator respawn decorative state")?;
        style(
            &respawn,
            &format!(
                "position:absolute;overflow:visible;pointer-events:none;left:0;top:{}px;width:{}px;height:{}px",
                area.height + 8.0,
                area.width,
                area.height
            ),
        )?;
        for (name, value) in [
            ("data-active-enemy-count", status.active_enemy_count.to_string()),
            ("data-incoming-enemy-count", status.incoming_enemy_count.to_string()),
            ("data-respawn-text", status.respawn_text.clone()),
            ("data-wave-ordinal", status.wave_ordinal.map_or(String::new(), |n| n.to_string())),
            ("data-wave-phase", status.wave_phase.map_or("", |phase| phase.id()).into()),
        ] {
            respawn.set_attribute(name, &value).map_err(|_| "spectator respawn status")?;
        }
        self.panel(&respawn, &layout, &status.respawn_text)?;
        append(&self.root, &respawn)?;
        self.previous = Some((status.clone(), viewport));
        Ok(())
    }

    fn panel(
        &self,
        parent: &HtmlElement,
        layout: &NativeSpectatorHudLayout,
        text: &str,
    ) -> Result<(), String> {
        for rail in &layout.horizontal_rails {
            self.rail(parent, *rail, true)?;
        }
        for rail in &layout.vertical_rails {
            self.rail(parent, *rail, false)?;
        }
        for corner in &layout.corners {
            let record = self.atlas.record(corner.record)?;
            let sprite = self.atlas.raw_sprite(&self.document, corner.record)?;
            sprite
                .style()
                .set_property(
                    "left",
                    &format!("{}px", corner.center.x - f64::from(record.logical_width) / 2.0),
                )
                .map_err(|_| "spectator corner x")?;
            sprite
                .style()
                .set_property(
                    "top",
                    &format!("{}px", corner.center.y - f64::from(record.logical_height) / 2.0),
                )
                .map_err(|_| "spectator corner y")?;
            append(parent, &sprite)?;
        }
        let mut spec =
            NativeUiTextSpec::new(NativeUiFontName::Medium, text, layout.text.x, layout.text.y);
        spec.align = NativeUiTextAlign::Left;
        spec.tint = 0xffe68c;
        let text =
            crate::ui::text::native_text(&self.document, &spec, "boneyard-spectator-status-text")?;
        text.style().set_property("z-index", "1").map_err(|_| "spectator text depth")?;
        append(parent, &text)
    }

    fn rail(
        &self,
        parent: &HtmlElement,
        rail: NativeSpectatorHudRail,
        horizontal: bool,
    ) -> Result<(), String> {
        let record = self.atlas.record(rail.record)?;
        let bounds = rail.bounds;
        let container = element(&self.document, "span", "boneyard-spectator-status-rail")?;
        style(
            &container,
            &format!(
                "position:absolute;display:block;overflow:hidden;left:{}px;top:{}px;width:{}px;height:{}px",
                bounds.x, bounds.y, bounds.width, bounds.height
            ),
        )?;
        let (extent, step) = if horizontal {
            (bounds.width, f64::from(record.logical_width))
        } else {
            (bounds.height, f64::from(record.logical_height))
        };
        let mut offset = 0.0;
        while offset < extent {
            let clipped = (extent - offset).min(step);
            let segment =
                element(&self.document, "span", "boneyard-spectator-status-rail-segment")?;
            let (x, y, width, height) = if horizontal {
                (offset, 0.0, clipped, f64::from(record.logical_height))
            } else {
                (0.0, offset, f64::from(record.logical_width), clipped)
            };
            style(
                &segment,
                &format!(
                    "position:absolute;display:block;overflow:hidden;left:{x}px;top:{y}px;width:{width}px;height:{height}px"
                ),
            )?;
            append(&segment, &self.atlas.raw_sprite(&self.document, rail.record)?)?;
            append(&container, &segment)?;
            offset += step;
        }
        append(parent, &container)
    }
}
