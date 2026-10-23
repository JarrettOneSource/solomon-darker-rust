//! Responsive camera over the stock panel. Compact screens retain usable slot
//! sizes and scroll the same native design surface, with section shortcuts.
use super::{InventoryPreviewPlacement, PanelEvent, PlayerPanel, PlayerPanels};
use crate::gpu_ui::UiFrame;
use solomon_sim::{Vec2, hud::HudRect};

pub(super) const COMPACT_NAVIGATION_HEIGHT: f32 = 48.0;

pub(super) fn navigation_layer(ctx: &egui::Context, body: egui::LayerId) -> egui::LayerId {
    let navigation =
        egui::LayerId::new(egui::Order::Foreground, egui::Id::unique("panel-sections"));
    // An interacted Area rises within its order. Source's fixed toolbar must
    // remain above the scrolling book after that interaction too.
    ctx.set_sublayer(body, navigation);
    navigation
}
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
    pub(super) fn has_compact_navigation(&self) -> bool {
        self.compact
            && matches!(
                self.panel,
                Some(PlayerPanel::Inventory | PlayerPanel::Skills | PlayerPanel::Service(_))
            )
    }

    pub(super) fn body_viewport(&self) -> egui::Rect {
        egui::Rect::from_min_max(
            egui::pos2(
                0.0,
                if self.has_compact_navigation() { COMPACT_NAVIGATION_HEIGHT } else { 0.0 },
            ),
            egui::pos2(self.viewport_size.0 as f32, self.viewport_size.1 as f32),
        )
    }

    pub fn inventory_preview_underlay(
        &self,
    ) -> Result<Option<super::InventoryPreviewUnderlay>, String> {
        if self.panel != Some(PlayerPanel::Inventory) {
            return Ok(None);
        }
        let r = self.ui.record(62)?;
        let [ox, oy] = r.registered_origin();
        let scale = self.view_scale * 1.25;
        let top = if self.compact { 48.0 } else { 0.0 };
        Ok(Some(super::InventoryPreviewUnderlay {
            texture_path: "assets/stock/UI.png",
            source: HudRect::new(r.x as f64, r.y as f64, r.width as f64, r.height as f64),
            destination: HudRect::new(
                self.stage_origin.x
                    + 800.0 * self.view_scale
                    + (ox as f64 - r.logical_width as f64 / 2.0) * scale,
                self.stage_origin.y
                    + 249.0 * self.view_scale
                    + (oy as f64 - r.logical_height as f64 / 2.0) * scale,
                r.width as f64 * scale,
                r.height as f64 * scale,
            ),
            clip: HudRect::new(
                0.0,
                top,
                self.viewport_size.0 as f64,
                self.viewport_size.1 as f64 - top,
            ),
            tint: 0xffffff,
            alpha: self.alpha * 0.32,
        }))
    }
    pub fn inventory_preview_placement(&self) -> Option<InventoryPreviewPlacement> {
        if self.panel != Some(PlayerPanel::Inventory) || self.notice.is_some() || self.dye.is_some()
        {
            return None;
        }
        let center = Vec2::new(
            self.stage_origin.x + 800.0 * self.view_scale,
            self.stage_origin.y + 249.0 * self.view_scale,
        );
        let top = if self.compact { 48.0 } else { 0.0 };
        let width = self.viewport_size.0 as f64;
        let height = self.viewport_size.1 as f64;
        if center.x + 100.0 * self.view_scale < 0.0
            || center.x - 100.0 * self.view_scale > width
            || center.y + 140.0 * self.view_scale < top
            || center.y - 140.0 * self.view_scale > height
        {
            return None;
        }
        Some(InventoryPreviewPlacement {
            center,
            scale: self.view_scale * 1.25,
            clip: HudRect::new(0.0, top, width, height - top),
            alpha: self.alpha,
        })
    }
    pub(super) fn update_viewport(&mut self, width: i32, height: i32) -> Result<(), String> {
        let opening = self.viewport_panel != self.panel;
        let changed = self.viewport_size != (width, height) || opening;
        self.viewport_size = (width, height);
        self.viewport_panel = self.panel;
        self.compact = width < 1000;
        let dialogue = self.panel == Some(PlayerPanel::Dialogue);
        let selector = matches!(self.panel, Some(PlayerPanel::SkillSelector(_)));
        if changed {
            self.scroll = Vec2::ZERO;
            self.view_scale = if self.compact {
                if dialogue {
                    ((width as f64 - 16.0) / 560.0).min((height as f64 - 16.0) / 500.0)
                } else if selector {
                    1.0
                } else {
                    0.65
                }
            } else {
                (width as f64 / 1600.0).min(height as f64 / 900.0)
            };
            if self.compact && !dialogue && !selector {
                if matches!(self.panel, Some(PlayerPanel::Service(_))) {
                    self.show_section(PanelSection::Shop)?;
                } else if self.panel == Some(PlayerPanel::Skills) {
                    self.scroll.x = (800.0 * self.view_scale - width as f64 / 2.0).max(0.0);
                }
            }
        }
        self.clamp_scroll();
        self.position_stage();
        Ok(())
    }
    fn position_stage(&mut self) {
        let (width, height) = (self.viewport_size.0 as f64, self.viewport_size.1 as f64);
        self.stage_origin = if self.compact {
            if self.panel == Some(PlayerPanel::Dialogue) {
                Vec2::new(
                    width / 2.0 - 800.0 * self.view_scale,
                    height / 2.0 - 300.0 * self.view_scale,
                )
            } else if matches!(self.panel, Some(PlayerPanel::SkillSelector(_))) {
                Vec2::ZERO
            } else {
                Vec2::new(
                    -self.scroll.x,
                    48.0 + ((height - 48.0 - 900.0 * self.view_scale) / 2.0).max(0.0)
                        - self.scroll.y,
                )
            }
        } else {
            Vec2::new(
                (width - 1600.0 * self.view_scale) / 2.0,
                (height - 900.0 * self.view_scale) / 2.0,
            )
        };
    }
    pub(super) fn clamp_scroll(&mut self) {
        self.scroll.x = self
            .scroll
            .x
            .clamp(0.0, (1675.0 * self.view_scale - self.viewport_size.0 as f64).max(0.0));
        self.scroll.y = self
            .scroll
            .y
            .clamp(0.0, (900.0 * self.view_scale - (self.viewport_size.1 as f64 - 48.0)).max(0.0));
        self.position_stage();
    }
    pub(super) fn show_section(&mut self, section: PanelSection) -> Result<(), String> {
        self.view_scale = match section {
            PanelSection::Stats | PanelSection::Equipment => {
                ((self.viewport_size.0 as f64 - 24.0) / 360.0).min(1.0)
            }
            PanelSection::Backpack | PanelSection::Shop => 0.65,
            _ => self.view_scale,
        };
        let width = self.viewport_size.0 as f64;
        let service = matches!(self.panel, Some(PlayerPanel::Service(_)));
        self.scroll = match section {
            PanelSection::Stats => Vec2::new(
                (if service { 263.0 } else { 210.0 }) * self.view_scale - width / 2.0,
                0.0,
            ),
            PanelSection::Backpack => Vec2::new(0.0, 490.0 * self.view_scale),
            PanelSection::Equipment => Vec2::new(
                (if service { 1337.0 } else { 1390.0 }) * self.view_scale - width / 2.0,
                0.0,
            ),
            PanelSection::Shop => Vec2::new(800.0 * self.view_scale - width / 2.0, 0.0),
            PanelSection::Previous => Vec2::new(self.scroll.x - width + 48.0, self.scroll.y),
            PanelSection::Next => Vec2::new(self.scroll.x + width - 48.0, self.scroll.y),
        };
        self.clamp_scroll();
        Ok(())
    }
    pub(super) fn paint_compact_navigation(
        &mut self,
        frame: &mut UiFrame<'_>,
    ) -> Result<(), String> {
        if matches!(self.panel, Some(PlayerPanel::Dialogue | PlayerPanel::SkillSelector(_))) {
            return Ok(());
        }
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
        let layer = navigation_layer(frame.ctx, self.layer_id());
        egui::Area::new(layer.id)
            .order(layer.order)
            .fixed_pos(egui::Pos2::ZERO)
            .constrain_to(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(self.viewport_size.0 as f32, COMPACT_NAVIGATION_HEIGHT),
            ))
            .constrain(false)
            .show(frame.ctx, |ui| {
                ui.set_min_width(self.viewport_size.0 as f32);
                if !(!self.input_suspended
                    && !self.skill_book_result_open()
                    && self.notice.is_none()
                    && self.dye.is_none()
                    && self.rename_sack.is_none())
                {
                    ui.disable();
                }
                frame.record_rect(
                    "panel:navigation",
                    "Panel sections",
                    "navigation",
                    egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(self.viewport_size.0 as f32, COMPACT_NAVIGATION_HEIGHT),
                    ),
                    false,
                );
                ui.horizontal(|ui| {
                    for (action, label) in actions {
                        let response = ui.add_sized(
                            [self.viewport_size.0 as f32 / actions.len() as f32 - 8.0, 40.0],
                            egui::Button::new(*label),
                        );
                        frame.record(&format!("panel:{action}"), label, "button", &response);
                        if response.clicked() {
                            self.events.push(PanelEvent::Activate(
                                super::target_from_action(action).expect("section action"),
                            ));
                        }
                    }
                });
            });
        Ok(())
    }
}
