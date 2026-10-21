//! Title art and controls from Website `renderer/title-menu-renderer.ts`.

use solomon_sim::float;
use solomon_sim::menu::{TitleState, title_backdrop_offsets_at};
use web_sys::{Document, HtmlElement};

use crate::ui::{StockAtlas, append, element, image, native_button, style};

pub struct TitleView {
    pub root: HtmlElement,
    backdrop: HtmlElement,
    solomon: HtmlElement,
    center: HtmlElement,
    controls: HtmlElement,
    account: HtmlElement,
    cloud_detail: HtmlElement,
    cloud_shadow: HtmlElement,
    grass: HtmlElement,
    horizon: HtmlElement,
    cloak: [HtmlElement; 4],
    eyes: HtmlElement,
    grave_rows: [HtmlElement; 3],
    grave_sprites: [Vec<HtmlElement>; 3],
    state: TitleState,
    grave_frames: [[f64; 4]; 9],
    widths: [f64; 9],
    document: Document,
    party_directory_available: bool,
    dark_cloud_available: bool,
}

impl TitleView {
    pub async fn new(document: &Document) -> Result<Self, String> {
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
        let root = element(document, "section", "menu-screen title-screen")?;
        root.set_attribute("aria-label", "Solomon Darker").map_err(|_| "title label")?;
        let backdrop = element(document, "div", "title-backdrop")?;
        let base = element(document, "div", "title-cloud title-cloud-base")?;
        let cloud_detail = element(document, "div", "title-cloud title-cloud-detail")?;
        let cloud_shadow = element(document, "div", "title-cloud title-cloud-shadow")?;
        let horizon = element(document, "div", "title-horizon")?;
        let grass = element(document, "div", "title-grass")?;
        append(&backdrop, &base)?;
        append(&backdrop, &image(document, "main-menu-moon", "title-moon")?)?;
        append(&backdrop, &cloud_detail)?;
        append(&backdrop, &cloud_shadow)?;
        append(&backdrop, &horizon)?;
        let grave_rows = [
            element(document, "div", "title-grave-row")?,
            element(document, "div", "title-grave-row")?,
            element(document, "div", "title-grave-row")?,
        ];
        for (index, row) in grave_rows.iter().enumerate() {
            row.style()
                .set_property("z-index", &(6 + index * 3).to_string())
                .map_err(|_| "grave depth")?;
            append(&backdrop, row)?;
            let fog = element(document, "div", "title-fog")?;
            fog.style()
                .set_property("z-index", &(7 + index * 3).to_string())
                .map_err(|_| "fog depth")?;
            append(&backdrop, &fog)?;
        }
        append(&backdrop, &element(document, "div", "title-inter-row-fog")?)?;
        append(&backdrop, &element(document, "div", "title-lower-fog")?)?;
        append(&backdrop, &grass)?;
        append(&root, &backdrop)?;
        let solomon = element(document, "div", "title-solomon")?;
        append(&solomon, &image(document, "menu-solomon-body", "title-solomon-body")?)?;
        let eyes = image(document, "menu-solomon-eyes", "title-solomon-eyes")?;
        append(&solomon, &eyes)?;
        let cloak = [
            image(document, "menu-solomon-cloak-0", "title-solomon-cloak")?,
            image(document, "menu-solomon-cloak-0", "title-solomon-cloak")?,
            image(document, "menu-solomon-cloak-1", "title-solomon-cloak")?,
            image(document, "menu-solomon-cloak-1", "title-solomon-cloak")?,
        ];
        for layer in &cloak {
            append(&solomon, layer)?;
        }
        append(&root, &solomon)?;
        let center = element(document, "div", "title-center")?;
        append(&center, &image(document, "logo-solomon-dark", "title-logo")?)?;
        append(&root, &center)?;
        let controls = element(document, "nav", "title-controls")?;
        controls.set_attribute("aria-label", "Main menu").map_err(|_| "title controls label")?;
        append(&controls, &image(document, "main-menu-flourish", "title-flourish-left")?)?;
        append(&controls, &image(document, "main-menu-flourish", "title-flourish-right")?)?;
        append(&root, &controls)?;
        let account = element(document, "span", "game-account-name game-account-name-title")?;
        account.set_text_content(Some("Not logged in"));
        account.set_attribute("aria-label", "Not logged in").map_err(|_| "title account label")?;
        append(&root, &account)?;
        Ok(Self {
            root,
            backdrop,
            solomon,
            center,
            controls,
            account,
            cloud_detail,
            cloud_shadow,
            grass,
            horizon,
            cloak,
            eyes,
            grave_rows,
            grave_sprites: std::array::from_fn(|_| Vec::new()),
            state: TitleState::new(widths),
            grave_frames,
            widths,
            document: document.clone(),
            party_directory_available: false,
            dark_cloud_available: false,
        })
    }

    pub fn show_controls(&mut self, play: bool, can_resume: bool) -> Result<(), String> {
        let existing =
            self.controls.query_selector_all("button").map_err(|_| "title button selector")?;
        for index in 0..existing.length() {
            if let Some(button) = existing.item(index) {
                self.controls.remove_child(&button).map_err(|_| "remove title button")?;
            }
        }
        let rows: &[(&str, &str, &str)] = if play {
            &[
                ("Last game", "continue", "main-menu-text-last-game"),
                ("New game", "new-game", "main-menu-text-new-game"),
                ("Join party", "join-party", ""),
                ("Back", "title", "main-menu-text-back"),
            ]
        } else {
            &[
                ("Play", "play", "main-menu-text-play"),
                ("Explore the Dark Cloud", "explore", "main-menu-text-dark-cloud"),
                ("Settings", "settings", "main-menu-text-settings"),
                ("Hall of Fame", "hall", "main-menu-text-hall"),
            ]
        };
        for &(label, action, label_image) in rows {
            if action == "join-party" && !self.party_directory_available {
                continue;
            }
            if action == "explore" && !self.dark_cloud_available {
                continue;
            }
            let button = native_button(&self.document, label, action)?;
            if action == "continue" && !can_resume {
                button.set_attribute("disabled", "").map_err(|_| "continue availability")?;
            }
            let text = button
                .query_selector(".native-button-label")
                .map_err(|_| "title button label")?
                .ok_or("native button has no label")?;
            text.set_text_content(None);
            if action == "continue" || action == "explore" {
                text.set_class_name("native-button-label title-stacked-label");
                let resume_label = image(
                    &self.document,
                    if action == "continue" {
                        "main-menu-text-resume"
                    } else {
                        "main-menu-text-explore"
                    },
                    "title-button-label",
                )?;
                text.append_child(&resume_label).map_err(|_| "Last game resume label")?;
            }
            let label_art = if action == "join-party" {
                let node = element(&self.document, "span", "title-join-party-label")?;
                node.set_text_content(Some("JOIN PARTY"));
                node
            } else {
                image(&self.document, label_image, "title-button-label")?
            };
            text.append_child(&label_art).map_err(|_| "title label art")?;
            append(&self.controls, &button)?;
        }
        Ok(())
    }

    pub fn set_party_directory_available(&mut self, available: bool) {
        self.party_directory_available = available;
    }

    pub fn set_dark_cloud_available(&mut self, available: bool) {
        self.dark_cloud_available = available;
    }

    pub fn set_account_username(&self, username: Option<&str>) -> Result<(), String> {
        self.account.set_text_content(Some(username.unwrap_or("Not logged in")));
        self.account
            .set_attribute("data-account-username", username.unwrap_or("Not logged in"))
            .map_err(|_| "title account identity")?;
        self.account
            .set_attribute(
                "aria-label",
                &username.map_or_else(
                    || "Not logged in".to_owned(),
                    |name| format!("Signed in as {name}"),
                ),
            )
            .map_err(|_| "title account semantics".to_owned())
    }

    pub fn set_prompt_visible(&self, visible: bool) {
        self.account.set_hidden(visible);
    }

    pub fn restart(&mut self) {
        self.state = TitleState::new(self.widths);
    }

    pub fn resize(&self, width: f64, height: f64) -> Result<(), String> {
        let scale = (width / 1600.0).min(height / 900.0);
        let cover = (width / 1600.0).max(height / 900.0);
        style(
            &self.backdrop,
            &format!(
                "transform:translate({}px, {}px) scale({cover})",
                (width - 1600.0 * cover) / 2.0,
                height - 900.0 * cover
            ),
        )?;
        style(
            &self.solomon,
            &format!("left:0;bottom:{}px;transform:scale({scale})", -60.0 * scale),
        )?;
        style(
            &self.center,
            &format!(
                "left:{}px;top:{}px;transform:scale({scale})",
                (width - 1600.0 * scale) / 2.0,
                (height - 900.0 * scale) / 2.0
            ),
        )?;
        // Keep native desktop placement, with a physical touch target floor on phones.
        let controls_scale = if width < 650.0 { ((width - 100.0) / 353.0).min(1.0) } else { scale };
        let x = width / 2.0 + 51.0 * scale - 176.5 * controls_scale;
        let y = if width < 650.0 {
            height * 0.51
        } else {
            (height - 900.0 * scale) / 2.0 + 421.0 * scale
        };
        style(&self.controls, &format!("left:{x}px;top:{y}px;transform:scale({controls_scale})"))
    }

    pub fn update(&mut self, elapsed_ms: f64) -> Result<(), String> {
        let target = (elapsed_ms.max(0.0) / 1000.0 * 60.0).floor() as u64;
        while self.state.ticks < target {
            self.state.step();
        }
        let [detail, shadow, grass, horizon] = title_backdrop_offsets_at(elapsed_ms / 1000.0);
        for (element, offset) in [
            (&self.cloud_detail, detail),
            (&self.cloud_shadow, shadow),
            (&self.grass, grass),
            (&self.horizon, horizon),
        ] {
            element
                .style()
                .set_property("background-position-x", &format!("{}px", -offset))
                .map_err(|_| "title scroll")?;
        }
        for index in 0..3 {
            let row = &self.state.rows[index];
            while self.grave_sprites[index].len() < row.graves.len() {
                let sprite = element(&self.document, "div", "title-grave")?;
                append(&sprite, &image(&self.document, "main-menu-grave-0", "")?)?;
                append(&self.grave_rows[index], &sprite)?;
                self.grave_sprites[index].push(sprite);
            }
            for (slot, sprite) in self.grave_sprites[index].iter().enumerate() {
                let Some(grave) = row.graves.get(slot) else {
                    sprite.set_hidden(true);
                    continue;
                };
                sprite.set_hidden(false);
                style(
                    sprite,
                    &format!(
                        "left:{}px;top:{}px;filter:brightness({});transform:rotate({}deg) scale({})",
                        grave.x, row.baseline, row.gray, grave.rotation, row.scale
                    ),
                )?;
                let art = sprite.first_element_child().ok_or("grave has no art")?;
                let [width, height, center_x, center_y] = self.grave_frames[grave.image_index];
                art.set_attribute(
                    "src",
                    &format!("assets/stock/menu/main-menu-grave-{}.png", grave.image_index),
                )
                .map_err(|_| "grave art")?;
                art.set_attribute(
                    "style",
                    &format!(
                        "position:absolute;left:{}px;top:{}px;width:{width}px;height:{height}px",
                        -width / 2.0 + center_x,
                        -height / 2.0 + center_y
                    ),
                )
                .map_err(|_| "grave registration")?;
            }
        }
        let current = self.state.cloak_phase.floor() as usize;
        let fraction = self.state.cloak_phase - current as f64;
        for (index, layer) in self.cloak.iter().enumerate() {
            let (frame, alpha) = if index < 2 {
                (current, 1.0 - fraction.powi(3))
            } else {
                ((current + 1) % 5, fraction)
            };
            let edge = frame == 0 || frame == 4;
            layer
                .set_attribute("src", &format!("assets/stock/menu/menu-solomon-cloak-{frame}.png"))
                .map_err(|_| "Solomon cloak art")?;
            style(
                layer,
                &format!(
                    "top:{}px;height:{}px;opacity:{alpha}",
                    if edge { 6 } else { 8 },
                    if edge { 654 } else { 652 }
                ),
            )?;
        }
        let bob = float::sin(self.state.ticks as f64 * std::f64::consts::PI / 180.0);
        self.eyes
            .style()
            .set_property("top", &format!("{}px", 230.0 + bob))
            .map_err(|_| "Solomon eye drift")?;
        Ok(())
    }
}
