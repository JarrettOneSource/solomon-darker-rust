//! Trusted custom frames from Website `renderer/mod-presentation-assets.ts`.
//! One session owns the verified image URLs and base textures. Derived icon, sprite and
//! wearable records borrow those sources; they never own a second content catalog.

use std::collections::BTreeMap;
use std::rc::Rc;

use solomon_sim::Vec2;
use solomon_sim::modding::assets::{GameModAsset, ModSpriteFrame};
use solomon_sim::modding::inventory::{
    ModConsumableContent, ModItemContent, ModItemIcon, ModWearableSlot,
};
use web_sys::HtmlImageElement;

use crate::assets;
use crate::mod_content::{GameModContentLoadError, ModContentCache, ModContentUrl};
use crate::render::{BlendMode, Renderer, Texture};

const WEARABLE_CELL: f64 = 170.0;
const WEARABLE_HEADINGS: usize = 24;

pub struct PreparedModPresentationAssets {
    images: BTreeMap<String, ModPresentationImage>,
}

/// DOM consumers use the same verified URL and dimensions as the WebGL frames.
pub struct ModPresentationImage {
    url: ModContentUrl,
    image: HtmlImageElement,
}

impl ModPresentationImage {
    #[must_use]
    pub fn url(&self) -> &str {
        self.url.as_str()
    }

    #[must_use]
    pub fn dimensions(&self) -> [u32; 2] {
        [self.image.natural_width(), self.image.natural_height()]
    }
}

impl PreparedModPresentationAssets {
    pub async fn load(
        cache: &ModContentCache,
        assets: &[GameModAsset],
    ) -> Result<Self, GameModContentLoadError> {
        let mut images = BTreeMap::new();
        for asset in assets {
            if asset.kind != "image" {
                continue;
            }
            let url = cache.object_url(asset).await?;
            let image = assets::load_image(url.as_str()).await.map_err(|message| {
                GameModContentLoadError { mod_id: asset.mod_id.clone(), message }
            })?;
            images
                .insert(asset_key(&asset.mod_id, &asset.path), ModPresentationImage { url, image });
        }
        Ok(Self { images })
    }

    /// Install between frames; the next begin_frame restores the ordinary batch binding.
    pub fn upload(self, renderer: &mut Renderer) -> Result<ModPresentationAssets, String> {
        renderer.flush();
        let mut bases = BTreeMap::new();
        for (key, image) in &self.images {
            let texture = match renderer.upload(&image.image) {
                Ok(texture) => texture,
                Err(error) => {
                    for texture in bases.values() {
                        renderer.destroy_texture(texture);
                    }
                    return Err(error);
                }
            };
            bases.insert(key.clone(), texture);
        }
        Ok(ModPresentationAssets {
            images: self.images,
            bases,
            frames: BTreeMap::new(),
            wearables: BTreeMap::new(),
            destroyed: false,
        })
    }
}

/// The session destroys its dependent views before calling destroy on this owner.
pub struct ModPresentationAssets {
    images: BTreeMap<String, ModPresentationImage>,
    bases: BTreeMap<String, Texture>,
    frames: BTreeMap<String, ModSpriteTexture>,
    wearables: BTreeMap<String, Rc<ModWearableTextureFrames>>,
    destroyed: bool,
}

#[derive(Clone)]
pub struct ModSpriteTexture {
    pub texture: Texture,
    pub frame: ModSpriteFrame,
}

pub struct ModSpriteDraw {
    pub center: Vec2,
    pub scale: Vec2,
    pub anchor: Vec2,
    pub rotation_radians: f64,
    pub tint: [f32; 4],
    pub blend: BlendMode,
}

impl ModSpriteTexture {
    /// Pixi uses orig/trim to place the content inside an authored logical frame.
    /// Custom textures use plain frame UV edges, unlike native stock record registration.
    pub fn draw(&self, renderer: &mut Renderer, draw: &ModSpriteDraw) {
        let corners = mod_sprite_corners(
            self.frame,
            draw.center,
            draw.scale,
            draw.anchor,
            draw.rotation_radians,
        )
        .map(|point| [point.x as f32, point.y as f32]);
        renderer.draw_quad(
            &self.texture,
            [
                self.frame.x as f32,
                self.frame.y as f32,
                self.frame.width as f32,
                self.frame.height as f32,
            ],
            corners,
            [draw.tint; 4],
            draw.blend,
        );
    }
}

#[derive(Clone)]
pub struct ModWearableTextureFrames {
    pub primary: Vec<Vec<ModSpriteTexture>>,
    pub secondary: Option<Vec<Vec<ModSpriteTexture>>>,
    pub slot: ModWearableSlot,
}

#[derive(Clone, Copy)]
pub enum ModWearableLayer {
    Primary,
    Secondary,
}

impl ModPresentationAssets {
    pub fn destroy(&mut self, renderer: &mut Renderer) {
        if self.destroyed {
            return;
        }
        renderer.flush();
        for texture in self.bases.values() {
            renderer.destroy_texture(texture);
        }
        self.frames.clear();
        self.wearables.clear();
        self.bases.clear();
        self.images.clear();
        self.destroyed = true;
    }

    pub fn image(&self, mod_id: &str, path: &str) -> Result<&ModPresentationImage, String> {
        self.require_live()?;
        self.images
            .get(&asset_key(mod_id, path))
            .ok_or_else(|| format!("mod image asset is unavailable: {mod_id}:{path}"))
    }

    pub fn icon(&mut self, content: &ModItemContent) -> Result<ModSpriteTexture, String> {
        self.icon_frame(
            &content.content_id,
            &content.mod_id,
            &content.icon,
            "icon",
            &content.icon.image_path,
        )
    }

    pub fn consumable_icon(
        &mut self,
        content: &ModConsumableContent,
    ) -> Result<ModSpriteTexture, String> {
        self.icon_frame(
            &content.content_id,
            &content.mod_id,
            &content.icon,
            "icon",
            &content.icon.image_path,
        )
    }

    pub fn icon_trim(
        &mut self,
        content: &ModItemContent,
    ) -> Result<Option<ModSpriteTexture>, String> {
        self.require_live()?;
        let Some(path) = &content.icon_trim_image_path else { return Ok(None) };
        self.icon_frame(&content.content_id, &content.mod_id, &content.icon, "icon-trim", path)
            .map(Some)
    }

    pub fn sprite_frame(
        &mut self,
        cache_key: &str,
        mod_id: &str,
        image_path: &str,
        frame: ModSpriteFrame,
    ) -> Result<ModSpriteTexture, String> {
        self.cached_frame(
            format!("sprite:{cache_key}"),
            mod_id,
            image_path,
            frame,
            "mod sprite asset",
        )
    }

    pub fn wearable(
        &mut self,
        renderer: &mut Renderer,
        content: &ModItemContent,
    ) -> Result<Option<Rc<ModWearableTextureFrames>>, String> {
        self.require_live()?;
        let Some(wearable) = &content.wearable else { return Ok(None) };
        if !self.wearables.contains_key(&content.content_id) {
            let primary = self.actor_sheet(
                renderer,
                &content.mod_id,
                &wearable.worn_image_path,
                wearable.slot,
            )?;
            let secondary = if let Some(path) = &wearable.worn_trim_image_path {
                Some(self.actor_sheet(renderer, &content.mod_id, path, wearable.slot)?)
            } else {
                None
            };
            if secondary.as_ref().is_some_and(|secondary| secondary[0].len() != primary[0].len()) {
                return Err(format!(
                    "mod wearable layers have different pose counts: {}:{}",
                    content.mod_id, content.key
                ));
            }
            self.wearables.insert(
                content.content_id.clone(),
                Rc::new(ModWearableTextureFrames { primary, secondary, slot: wearable.slot }),
            );
        }
        Ok(self.wearables.get(&content.content_id).cloned())
    }

    fn actor_sheet(
        &self,
        renderer: &mut Renderer,
        mod_id: &str,
        path: &str,
        slot: ModWearableSlot,
    ) -> Result<Vec<Vec<ModSpriteTexture>>, String> {
        let source = self.base(mod_id, path, "mod wearable asset")?;
        let rows = mod_wearable_rows(f64::from(source.width), f64::from(source.height), slot)
            .ok_or_else(|| format!("mod wearable asset has invalid geometry: {mod_id}:{path}"))?;
        renderer.set_texture_nearest(source);
        let mut headings = Vec::with_capacity(WEARABLE_HEADINGS);
        for heading in 0..WEARABLE_HEADINGS {
            let mut poses = Vec::with_capacity(rows);
            for pose in 0..rows {
                poses.push(ModSpriteTexture {
                    texture: source.clone(),
                    frame: mod_wearable_cell(heading, pose),
                });
            }
            headings.push(poses);
        }
        Ok(headings)
    }

    fn icon_frame(
        &mut self,
        content_id: &str,
        mod_id: &str,
        icon: &ModItemIcon,
        suffix: &str,
        image_path: &str,
    ) -> Result<ModSpriteTexture, String> {
        self.cached_frame(
            format!("{content_id}:{suffix}"),
            mod_id,
            image_path,
            icon.frame,
            "mod icon asset",
        )
    }

    fn cached_frame(
        &mut self,
        key: String,
        mod_id: &str,
        path: &str,
        frame: ModSpriteFrame,
        field: &str,
    ) -> Result<ModSpriteTexture, String> {
        self.require_live()?;
        if let Some(cached) = self.frames.get(&key) {
            return Ok(cached.clone());
        }
        let texture = self.base(mod_id, path, field)?.clone();
        let result = ModSpriteTexture { texture, frame };
        self.frames.insert(key, result.clone());
        Ok(result)
    }

    fn base(&self, mod_id: &str, path: &str, field: &str) -> Result<&Texture, String> {
        self.require_live()?;
        self.bases
            .get(&asset_key(mod_id, path))
            .ok_or_else(|| format!("{field} is unavailable: {mod_id}:{path}"))
    }

    fn require_live(&self) -> Result<(), String> {
        if self.destroyed { Err("mod presentation textures are destroyed".into()) } else { Ok(()) }
    }
}

pub fn mod_wearable_frame(
    textures: &ModWearableTextureFrames,
    layer: ModWearableLayer,
    heading: usize,
    pose: usize,
) -> Result<&ModSpriteTexture, String> {
    let (name, bank) = match layer {
        ModWearableLayer::Primary => ("primary", Some(&textures.primary)),
        ModWearableLayer::Secondary => ("secondary", textures.secondary.as_ref()),
    };
    let poses = bank
        .and_then(|headings| headings.get(heading))
        .filter(|poses| !poses.is_empty())
        .ok_or_else(|| format!("Missing mod wearable {name} frame"))?;
    Ok(&poses[pose.min(poses.len() - 1)])
}

fn asset_key(mod_id: &str, path: &str) -> String {
    format!("{}\0{}", mod_id.to_lowercase(), path.to_lowercase())
}

fn mod_wearable_rows(width: f64, height: f64, slot: ModWearableSlot) -> Option<usize> {
    let maximum = match slot {
        ModWearableSlot::Hat => 1.0,
        ModWearableSlot::Robe => 5.0,
        ModWearableSlot::Staff => 10.0,
    };
    let rows = height / WEARABLE_CELL;
    if width / WEARABLE_CELL != WEARABLE_HEADINGS as f64
        || rows.fract() != 0.0
        || !(1.0..=maximum).contains(&rows)
    {
        return None;
    }
    Some(rows as usize)
}

fn mod_wearable_cell(heading: usize, pose: usize) -> ModSpriteFrame {
    ModSpriteFrame {
        x: heading as f64 * WEARABLE_CELL,
        y: pose as f64 * WEARABLE_CELL,
        width: WEARABLE_CELL,
        height: WEARABLE_CELL,
        logical_width: WEARABLE_CELL,
        logical_height: WEARABLE_CELL,
        content_width: WEARABLE_CELL,
        content_height: WEARABLE_CELL,
        center_offset_x: 0.0,
        center_offset_y: 0.0,
    }
}

fn mod_sprite_corners(
    frame: ModSpriteFrame,
    center: Vec2,
    scale: Vec2,
    anchor: Vec2,
    rotation: f64,
) -> [Vec2; 4] {
    let x = ((frame.logical_width - frame.width) / 2.0 + frame.center_offset_x
        - anchor.x * frame.logical_width)
        * scale.x;
    let y = ((frame.logical_height - frame.height) / 2.0 + frame.center_offset_y
        - anchor.y * frame.logical_height)
        * scale.y;
    let width = frame.width * scale.x;
    let height = frame.height * scale.y;
    let (sine, cosine) = rotation.sin_cos();
    [[x, y], [x + width, y], [x + width, y + height], [x, y + height]]
        .map(|[x, y]| Vec2::new(cosine * x - sine * y + center.x, sine * x + cosine * y + center.y))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authored_custom_frames_and_wearable_cells_match_source_textures() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../sim/tests/fixtures/website/mod-presentation-assets-goldens.json"
        ))
        .unwrap();
        for case in fixture["sprites"].as_array().unwrap() {
            let frame: ModSpriteFrame = serde_json::from_value(case["frame"].clone()).unwrap();
            let input = &case["input"];
            let point = |field: &str| {
                Vec2::new(input[field][0].as_f64().unwrap(), input[field][1].as_f64().unwrap())
            };
            let corners = mod_sprite_corners(
                frame,
                point("center"),
                point("scale"),
                point("anchor"),
                input["rotation"].as_f64().unwrap(),
            );
            for (actual, expected) in corners.iter().zip(case["corners"].as_array().unwrap()) {
                // Pixi's transform uses platform trigonometry; frame/trim arithmetic is exact.
                assert!((actual.x - expected["x"].as_f64().unwrap()).abs() < 1.0e-12);
                assert!((actual.y - expected["y"].as_f64().unwrap()).abs() < 1.0e-12);
            }
        }
        for case in fixture["wearables"].as_array().unwrap() {
            let slot = match case["slot"].as_str().unwrap() {
                "hat" => ModWearableSlot::Hat,
                "robe" => ModWearableSlot::Robe,
                "staff" => ModWearableSlot::Staff,
                _ => unreachable!(),
            };
            let rows = mod_wearable_rows(
                case["width"].as_f64().unwrap(),
                case["height"].as_f64().unwrap(),
                slot,
            )
            .unwrap();
            assert_eq!(rows, case["rows"].as_u64().unwrap() as usize);
            let heading = case["heading"].as_u64().unwrap() as usize;
            let pose = (case["pose"].as_u64().unwrap() as usize).min(rows - 1);
            let frame = mod_wearable_cell(heading, pose);
            let cell: Vec<f64> = case["cell"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_f64().unwrap())
                .collect();
            assert_eq!([frame.x, frame.y, frame.width, frame.height], cell.as_slice());
            assert_eq!(case["scaleMode"], "nearest");
        }
        assert!(mod_wearable_rows(3910.0, 170.0, ModWearableSlot::Hat).is_none());
        assert!(mod_wearable_rows(4080.0, 340.0, ModWearableSlot::Hat).is_none());
        assert_ne!(
            mod_wearable_rows(4080.0, 850.0, ModWearableSlot::Robe),
            mod_wearable_rows(4080.0, 340.0, ModWearableSlot::Robe)
        );
        assert_eq!(asset_key("EXAMPLE.ART", "Images/ICON.PNG"), "example.art\0images/icon.png");
    }
}
