//! Stock title composition from Website `renderer/title-menu-renderer.ts`.

use super::widgets::{self, Stage};
use crate::gpu_ui::{StockAtlas, UiFrame};
use egui::{Align2, Color32, Pos2, Rect, Ui, Vec2};
use solomon_sim::{
    float,
    menu::{TitleState, title_backdrop_offsets_at},
};

pub(super) struct TitleView {
    atlas: StockAtlas,
    state: TitleState,
    grave_frames: [[f64; 4]; 9],
    widths: [f64; 9],
    party_directory_available: bool,
    dark_cloud_available: bool,
    play: bool,
    can_resume: bool,
    account: Option<String>,
    prompt_visible: bool,
    elapsed: f64,
}
impl TitleView {
    pub async fn new() -> Result<Self, String> {
        let atlas = StockAtlas::load("Title").await?;
        let mut widths = [0.0; 9];
        let mut grave_frames = [[0.0; 4]; 9];
        for index in 0..9 {
            let record = atlas.record(index + 16)?;
            widths[index] = f64::from(record.width);
            grave_frames[index] = [
                f64::from(record.width),
                f64::from(record.height),
                f64::from(record.center_x),
                f64::from(record.center_y),
            ];
        }
        Ok(Self {
            atlas,
            state: TitleState::new(widths),
            grave_frames,
            widths,
            party_directory_available: false,
            dark_cloud_available: false,
            play: false,
            can_resume: false,
            account: None,
            prompt_visible: false,
            elapsed: 0.0,
        })
    }
    pub fn show_controls(&mut self, play: bool, can_resume: bool) -> Result<(), String> {
        self.play = play;
        self.can_resume = can_resume;
        Ok(())
    }
    pub fn set_party_directory_available(&mut self, value: bool) {
        self.party_directory_available = value;
    }
    pub fn set_dark_cloud_available(&mut self, value: bool) {
        self.dark_cloud_available = value;
    }
    pub fn set_account_username(&mut self, value: Option<&str>) -> Result<(), String> {
        self.account = value.map(str::to_owned);
        Ok(())
    }
    pub fn set_prompt_visible(&mut self, value: bool) {
        self.prompt_visible = value;
    }
    pub fn restart(&mut self) {
        self.state = TitleState::new(self.widths);
        self.elapsed = 0.0;
    }
    pub fn update(&mut self, elapsed: f64) -> Result<(), String> {
        self.elapsed = elapsed;
        let target = (elapsed.max(0.0) / 1000.0 * 60.0).floor() as u64;
        while self.state.ticks < target {
            self.state.step();
        }
        Ok(())
    }
    pub fn paint(
        &self,
        frame: &mut UiFrame<'_>,
        ui: &mut Ui,
        view: Rect,
        enabled: bool,
        actions: &mut Vec<String>,
    ) -> Result<(), String> {
        ui.painter().rect_filled(view, 0.0, Color32::BLACK);
        let stage = Stage::fit(view);
        let cover_scale = (view.width() / 1600.0).max(view.height() / 900.0);
        let cover = Stage {
            origin: Pos2::new(
                view.center().x - 800.0 * cover_scale,
                view.bottom() - 900.0 * cover_scale,
            ),
            scale: cover_scale,
        };
        let [detail, shadow, grass, horizon] = title_backdrop_offsets_at(self.elapsed / 1000.0);
        repeat(
            frame,
            ui,
            "main-menu-cloud-base",
            cover,
            [0.0, 0.0, 1600.0, 553.0],
            1298.788990825688,
            0.0,
        );
        widgets::image(
            frame,
            ui,
            "main-menu-moon",
            cover.rect(1304.0, 101.5, 192.0, 192.0),
            Color32::WHITE,
        );
        repeat(
            frame,
            ui,
            "main-menu-cloud-detail",
            cover,
            [0.0, 0.0, 1600.0, 553.0],
            1298.788990825688,
            detail,
        );
        repeat(
            frame,
            ui,
            "main-menu-cloud-shadow",
            cover,
            [0.0, 0.0, 1600.0, 553.0],
            1298.788990825688,
            shadow,
        );
        repeat(frame, ui, "main-menu-horizon", cover, [0.0, 522.0, 1600.0, 31.0], 1024.0, horizon);
        let atlas_size = frame.image_size(self.atlas.path()).ok_or("Title.png is not ready")?;
        for (index, row) in self.state.rows.iter().enumerate() {
            for grave in &row.graves {
                let [w, h, cx, cy] = self.grave_frames[grave.image_index];
                let record = self.atlas.record(grave.image_index + 16)?;
                let uv = Rect::from_min_max(
                    Pos2::new(record.x as f32 / atlas_size.x, record.y as f32 / atlas_size.y),
                    Pos2::new(
                        (record.x + record.width) as f32 / atlas_size.x,
                        (record.y + record.height) as f32 / atlas_size.y,
                    ),
                );
                let center = cover.at(grave.x, row.baseline);
                let rect = cover.rect(
                    grave.x + (-w / 2.0 + cx) * row.scale,
                    row.baseline + (-h / 2.0 + cy) * row.scale,
                    w * row.scale,
                    h * row.scale,
                );
                frame.image_rotated(
                    ui.painter(),
                    self.atlas.path(),
                    rect,
                    Some(uv),
                    grave.rotation.to_radians() as f32,
                    center,
                    widgets::color({
                        let channel = (row.gray.clamp(0.0, 1.0) * 255.0).round() as u32;
                        channel * 0x010101
                    }),
                );
            }
            widgets::gradient(
                ui,
                cover.rect(0.0, 365.0, 1600.0, 535.0),
                Color32::TRANSPARENT,
                widgets::alpha(0x292e40, 0.7),
            );
            if index == 1 {
                widgets::gradient(
                    ui,
                    cover.rect(0.0, 500.0, 1600.0, 212.0),
                    Color32::TRANSPARENT,
                    widgets::color(0x292e40),
                );
                ui.painter().rect_filled(
                    cover.rect(0.0, 712.0, 1600.0, 188.0),
                    0.0,
                    widgets::color(0x282d3f),
                );
            }
        }
        repeat(frame, ui, "main-menu-grass", cover, [0.0, 829.0, 1600.0, 71.0], 1024.0, grass);
        let solomon = Stage {
            origin: Pos2::new(view.left(), view.bottom() - 600.0 * stage.scale),
            scale: stage.scale,
        };
        let solomon_alpha = if view.width() < 650.0 { 0.6 } else { 1.0 };
        widgets::image(
            frame,
            ui,
            "menu-solomon-body",
            solomon.rect(48.0, 106.0, 208.0, 498.0),
            widgets::alpha(0xffffff, solomon_alpha),
        );
        let bob = float::sin(self.state.ticks as f64 * std::f64::consts::PI / 180.0);
        widgets::image(
            frame,
            ui,
            "menu-solomon-eyes",
            solomon.rect(50.0, 230.0 + bob, 171.0, 30.0),
            widgets::alpha(0xffffff, solomon_alpha),
        );
        let current = self.state.cloak_phase.floor() as usize;
        let fraction = self.state.cloak_phase - current as f64;
        for index in 0..4 {
            let (pose, alpha) = if index < 2 {
                (current, 1.0 - fraction.powi(3))
            } else {
                ((current + 1) % 5, fraction)
            };
            let edge = pose == 0 || pose == 4;
            widgets::image(
                frame,
                ui,
                &format!("menu-solomon-cloak-{pose}"),
                solomon.rect(
                    -40.0,
                    if edge { 6.0 } else { 8.0 },
                    366.0,
                    if edge { 654.0 } else { 652.0 },
                ),
                widgets::alpha(0xffffff, alpha * solomon_alpha),
            );
        }
        widgets::image(
            frame,
            ui,
            "logo-solomon-dark",
            stage.rect(435.5, if view.width() < 650.0 { -250.0 } else { 0.0 }, 829.0, 395.0),
            Color32::WHITE,
        );
        let controls_scale = if view.width() < 650.0 {
            ((view.width() - 100.0) / 353.0).min(1.0)
        } else {
            stage.scale
        };
        let x = view.center().x + 51.0 * stage.scale - 176.5 * controls_scale;
        let y = if view.width() < 650.0 {
            view.top() + view.height() * 0.51
        } else {
            stage.origin.y + 421.0 * stage.scale
        };
        let controls = Stage { origin: Pos2::new(x, y), scale: controls_scale };
        widgets::image(
            frame,
            ui,
            "main-menu-flourish",
            controls.rect(-73.5, 19.0, 67.0, 262.0),
            Color32::WHITE,
        );
        widgets::mirrored_image(
            frame,
            ui,
            "main-menu-flourish",
            controls.rect(359.5, 19.0, 67.0, 262.0),
            true,
            false,
            Color32::WHITE,
        );
        let rows = if self.play {
            [
                ("Last game", "continue", "last-game"),
                ("New game", "new-game", "new-game"),
                ("Join party", "join-party", ""),
                ("Back", "title", "back"),
            ]
        } else {
            [
                ("Play", "play", "play"),
                ("Explore the Dark Cloud", "explore", "dark-cloud"),
                ("Settings", "settings", "settings"),
                ("Hall of Fame", "hall", "hall"),
            ]
        };
        let mut row = 0;
        for (label, action, art) in rows {
            if action == "join-party" && !self.party_directory_available
                || action == "explore" && !self.dark_cloud_available
            {
                continue;
            }
            let rect = controls.rect(0.0, f64::from(row) * 76.0, 353.0, 69.0);
            let available = action != "continue" || self.can_resume;
            let admitted = enabled && available;
            let response = widgets::interact(frame, ui, action, label, "button", rect, admitted);
            let paint = frame.with_brightness(if admitted && frame.focus_visible(&response) {
                1.2
            } else {
                1.0
            });
            widgets::paint_stock_button(&paint, ui, &response, rect, controls_scale, available);
            if response.clicked() && admitted {
                actions.push(action.to_owned());
            }
            let tint = widgets::alpha(0xffffff, if available { 1.0 } else { 0.36 });
            let press = if available && widgets::stock_button_pressed(ui, &response) {
                Vec2::splat(6.0 * controls_scale)
            } else {
                Vec2::ZERO
            };
            if action == "join-party" {
                widgets::label(
                    ui,
                    "JOIN PARTY",
                    rect.center() + press,
                    Align2::CENTER_CENTER,
                    "Cinzel Bold",
                    17.28 * controls_scale,
                    paint.filtered_color(widgets::color(0xd8d2c0)),
                );
            } else {
                let names = if action == "continue" {
                    vec!["resume", art]
                } else if action == "explore" {
                    vec!["explore", art]
                } else {
                    vec![art]
                };
                let sizes: Vec<_> = names
                    .iter()
                    .map(|name| {
                        frame
                            .image_size(&format!("assets/stock/menu/main-menu-text-{name}.png"))
                            .unwrap_or(Vec2::ZERO)
                            * controls_scale
                    })
                    .collect();
                let mut top = rect.center().y - sizes.iter().map(|size| size.y).sum::<f32>() / 2.0;
                for (name, size) in names.iter().zip(sizes) {
                    widgets::image(
                        &paint,
                        ui,
                        &format!("main-menu-text-{name}"),
                        Rect::from_min_size(
                            Pos2::new(rect.center().x - size.x / 2.0, top) + press,
                            size,
                        ),
                        tint,
                    );
                    top += size.y;
                }
            }
            row += 1;
        }
        if !self.prompt_visible {
            let account = self.account.as_deref().unwrap_or("Not logged in");
            let rect = widgets::label(
                ui,
                account,
                view.min + Vec2::new(11.0, 12.0),
                Align2::LEFT_TOP,
                "JetBrains Mono",
                14.0,
                widgets::GOLD,
            );
            frame.record_rect("title-account", account, "status", rect, false);
            frame.record_value("title-account", account, None);
        }
        Ok(())
    }
    pub fn default_focus(&self) -> &'static str {
        if self.play { if self.can_resume { "continue" } else { "new-game" } } else { "play" }
    }
}
fn repeat(
    frame: &UiFrame<'_>,
    ui: &Ui,
    name: &str,
    stage: Stage,
    bounds: [f64; 4],
    tile_width: f64,
    offset: f64,
) {
    let [x, y, width, height] = bounds;
    let painter =
        ui.painter().with_clip_rect(stage.rect(x, y, width, height).intersect(ui.clip_rect()));
    let mut left = x - offset.rem_euclid(tile_width);
    while left < x + width {
        frame.image(
            &painter,
            &format!("assets/stock/menu/{name}.png"),
            stage.rect(left, y, tile_width, height),
            None,
            Color32::WHITE,
        );
        left += tile_width;
    }
}
