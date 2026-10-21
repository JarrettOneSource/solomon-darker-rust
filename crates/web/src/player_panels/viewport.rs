//! Compact screens pan the existing stock panel instead of shrinking its controls.
//! The simulation and action targets stay the same; this module owns only the camera
//! over that panel, its section shortcuts and the native browser scroll surface.

use super::{InventoryPreviewPlacement, PlayerPanel, PlayerPanels};
use crate::ui::{append, native_button, style};
use solomon_sim::{Vec2, hud::HudRect};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PanelSection {
    Stats,
    Backpack,
    Equipment,
    Shop,
    Previous,
    Next,
}

impl PlayerPanels {
    #[must_use]
    pub fn inventory_preview_placement(&self) -> Option<InventoryPreviewPlacement> {
        if self.panel != Some(PlayerPanel::Inventory) || self.notice.is_some() || self.dye.is_some()
        {
            return None;
        }
        let viewport = self.viewport.get_bounding_client_rect();
        let stage = self.stage.get_bounding_client_rect();
        let center = Vec2::new(
            stage.left() + 800.0 * self.view_scale,
            stage.top() + 249.0 * self.view_scale,
        );
        let half_width = 100.0 * self.view_scale;
        let half_height = 140.0 * self.view_scale;
        if center.x + half_width < viewport.left()
            || center.x - half_width > viewport.right()
            || center.y + half_height < viewport.top()
            || center.y - half_height > viewport.bottom()
        {
            return None;
        }
        let alpha = self.content.style().get_property_value("opacity").ok()?.parse().ok()?;
        Some(InventoryPreviewPlacement {
            center,
            scale: self.view_scale * 1.25,
            clip: HudRect::new(
                viewport.left(),
                viewport.top(),
                viewport.width(),
                viewport.height(),
            ),
            alpha,
        })
    }

    pub(super) fn update_viewport(&mut self, width: i32, height: i32) -> Result<(), String> {
        let compact = width < 1000;
        let changed = self.viewport_size != (width, height)
            || self.viewport_panel != self.panel
            || self.compact != compact;
        if !changed {
            return Ok(());
        }
        let opening = self.viewport_panel != self.panel;
        self.compact = compact;
        self.viewport_size = (width, height);
        self.viewport_panel = self.panel;
        self.dirty = true;
        self.root
            .set_attribute("data-compact", if compact { "true" } else { "false" })
            .map_err(|_| "panel compact layout")?;
        let dialogue = self.panel == Some(PlayerPanel::Dialogue);
        let selector = matches!(self.panel, Some(PlayerPanel::SkillSelector(_)));
        self.compact_navigation.set_hidden(!compact || dialogue || selector);
        if compact && !dialogue && !selector {
            self.draw_compact_navigation()?;
        }
        if selector && compact {
            self.view_scale = 1.0;
            style(&self.viewport, "top:0;overflow:hidden")?;
            style(&self.surface, &format!("width:{width}px;height:{height}px;margin-top:0"))?;
            style(&self.stage, "left:0;top:0;transform:none")?;
        } else if compact {
            // A 72px stock slot remains at least 46px wide on a phone. Browser scrolling
            // and section shortcuts expose the rest of the original panel at that scale.
            self.view_scale = 0.65;
            if dialogue {
                self.view_scale =
                    ((f64::from(width) - 16.0) / 560.0).min((f64::from(height) - 16.0) / 500.0);
                style(&self.viewport, "top:0;overflow:hidden")?;
                style(&self.surface, &format!("width:{width}px;height:{height}px;margin-top:0"))?;
                style(
                    &self.stage,
                    &format!(
                        "left:{}px;top:{}px;transform:scale({})",
                        f64::from(width) / 2.0 - 800.0 * self.view_scale,
                        f64::from(height) / 2.0 - 300.0 * self.view_scale,
                        self.view_scale,
                    ),
                )?;
            } else {
                self.layout_compact_surface(0.65)?;
            }
        } else {
            self.view_scale = (f64::from(width) / 1600.0).min(f64::from(height) / 900.0);
            style(&self.viewport, "top:0;overflow:hidden")?;
            style(&self.surface, &format!("width:{width}px;height:{height}px;margin-top:0"))?;
            style(
                &self.stage,
                &format!(
                    "left:{}px;top:{}px;transform:scale({})",
                    (f64::from(width) - 1600.0 * self.view_scale) / 2.0,
                    (f64::from(height) - 900.0 * self.view_scale) / 2.0,
                    self.view_scale,
                ),
            )?;
        }
        if opening || !compact {
            self.viewport.set_scroll_left(0);
            self.viewport.set_scroll_top(0);
            if compact && matches!(self.panel, Some(PlayerPanel::Service(_))) {
                self.show_section(PanelSection::Shop)?;
            } else if compact && self.panel == Some(PlayerPanel::Skills) {
                // Native skill pages are centered at x800. Starting at the canvas's
                // left edge shows empty leather while the first cards sit offscreen.
                let left = 800.0 * self.view_scale - f64::from(self.viewport.client_width()) / 2.0;
                self.viewport.set_scroll_left(left.max(0.0) as i32);
            }
        }
        if compact {
            append(&self.root, &self.tooltip)?;
        } else {
            append(&self.stage, &self.tooltip)?;
        }
        Ok(())
    }

    fn draw_compact_navigation(&self) -> Result<(), String> {
        self.compact_navigation.set_text_content(None);
        let actions: &[(&str, &str)] = match self.panel {
            Some(PlayerPanel::Skills) => &[
                ("view-previous", "◀"),
                ("view-next", "▶"),
                ("inventory", "Inventory"),
                ("close", "Done"),
            ],
            Some(PlayerPanel::Service(_)) => &[
                ("view-shop", "Shop"),
                ("view-backpack", "Backpack"),
                ("view-equipment", "Equip"),
                ("view-stats", "Stats"),
                ("skills", "Skills"),
                ("close", "Done"),
            ],
            _ => &[
                ("view-stats", "Stats"),
                ("view-backpack", "Backpack"),
                ("view-equipment", "Equip"),
                ("skills", "Skills"),
                ("close", "Done"),
            ],
        };
        for (action, label) in actions {
            let button = native_button(&self.document, label, action)?;
            button
                .set_attribute("data-panel-action", action)
                .map_err(|_| "panel section action")?;
            if *action == "view-previous" || *action == "view-next" {
                button
                    .set_attribute(
                        "aria-label",
                        if *action == "view-previous" {
                            "Scroll panel left"
                        } else {
                            "Scroll panel right"
                        },
                    )
                    .map_err(|_| "panel scroll label")?;
            }
            append(&self.compact_navigation, &button)?;
        }
        Ok(())
    }

    fn layout_compact_surface(&mut self, scale: f64) -> Result<(), String> {
        self.view_scale = scale;
        style(&self.viewport, "top:48px;overflow:auto")?;
        let margin = ((f64::from(self.viewport_size.1) - 48.0 - 900.0 * scale) / 2.0).max(0.0);
        // The native final backpack column extends beyond the 1600px canvas.
        // Scrolling keeps that addressed slot reachable without moving its contents.
        style(
            &self.surface,
            &format!("width:{}px;height:{}px;margin-top:{margin}px", 1675.0 * scale, 900.0 * scale),
        )?;
        style(&self.stage, &format!("left:0;top:0;transform:scale({scale})"))
    }

    pub(super) fn show_section(&mut self, section: PanelSection) -> Result<(), String> {
        let scale = match section {
            PanelSection::Stats | PanelSection::Equipment => {
                ((f64::from(self.viewport_size.0) - 24.0) / 360.0).min(1.0)
            }
            PanelSection::Backpack | PanelSection::Shop => 0.65,
            PanelSection::Previous | PanelSection::Next => self.view_scale,
        };
        self.layout_compact_surface(scale)?;
        let width = f64::from(self.viewport.client_width());
        let current = f64::from(self.viewport.scroll_left());
        let (left, top) = match section {
            PanelSection::Stats => {
                let center =
                    if matches!(self.panel, Some(PlayerPanel::Service(_))) { 263.0 } else { 210.0 };
                (center * scale - width / 2.0, 0.0)
            }
            PanelSection::Backpack => (0.0, 490.0 * self.view_scale),
            PanelSection::Equipment => {
                let center = if matches!(self.panel, Some(PlayerPanel::Service(_))) {
                    1337.0
                } else {
                    1390.0
                };
                (center * scale - width / 2.0, 0.0)
            }
            PanelSection::Shop => (800.0 * self.view_scale - width / 2.0, 0.0),
            PanelSection::Previous => {
                (current - width + 48.0, f64::from(self.viewport.scroll_top()))
            }
            PanelSection::Next => (current + width - 48.0, f64::from(self.viewport.scroll_top())),
        };
        self.viewport.set_scroll_left(left.max(0.0) as i32);
        self.viewport.set_scroll_top(top.max(0.0) as i32);
        Ok(())
    }
}
