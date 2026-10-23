//! GPU custom Pixi-style trimmed icons from `renderer/mod-presentation-assets.ts`.
//! The session owns verified image URLs, dimensions and authored tint layers.

use egui::{Color32, Painter, Pos2, Rect, pos2, vec2};
use solomon_sim::economy::HubInventoryItem;
use solomon_sim::modding::assets::ModSpriteFrame;
use solomon_sim::modding::inventory::ModWearableSlot;

use crate::gpu_ui::UiFrame;
use crate::mod_presentation_assets::ModPresentationAssets;

#[derive(Clone, PartialEq)]
pub(crate) struct ModIconLayer {
    image_url: String,
    dimensions: [u32; 2],
    frame: ModSpriteFrame,
    tint: Color32,
}

#[derive(Clone, Copy)]
pub struct ModIconDraw {
    pub center: Pos2,
    pub scale: f32,
    pub mirrored: bool,
    pub tint: Color32,
}

pub fn mod_icon(
    gpu: &UiFrame<'_>,
    painter: &Painter,
    images: &ModPresentationAssets,
    mod_id: &str,
    path: &str,
    frame: ModSpriteFrame,
    draw: ModIconDraw,
) -> Result<(), String> {
    let image = images.image(mod_id, path)?;
    frame_icon(
        gpu,
        painter,
        &ModIconLayer {
            image_url: image.url().to_owned(),
            dimensions: image.dimensions(),
            frame,
            tint: draw.tint,
        },
        draw.center,
        draw.scale,
        draw.mirrored,
    );
    Ok(())
}

pub(crate) struct ModIconGeometry {
    pub destination: Rect,
    pub uv: Rect,
}
impl ModIconLayer {
    pub(crate) fn url(&self) -> &str {
        &self.image_url
    }
    pub(crate) fn frame(&self) -> ModSpriteFrame {
        self.frame
    }
    pub(crate) fn dimensions(&self) -> [u32; 2] {
        self.dimensions
    }
    pub(crate) fn geometry(&self, center: Pos2, scale: f32, mirrored: bool) -> ModIconGeometry {
        let frame = self.frame;
        let left = (-frame.width / 2.0 + frame.center_offset_x) as f32 * scale;
        let top = (-frame.height / 2.0 + frame.center_offset_y) as f32 * scale;
        let width = frame.width as f32 * scale;
        let height = frame.height as f32 * scale;
        let destination = Rect::from_min_size(
            center + vec2(if mirrored { -left - width } else { left }, top),
            vec2(width, height),
        );
        let [image_width, image_height] = self.dimensions.map(|value| value as f32);
        let mut uv = Rect::from_min_max(
            pos2(frame.x as f32 / image_width, frame.y as f32 / image_height),
            pos2(
                (frame.x + frame.width) as f32 / image_width,
                (frame.y + frame.height) as f32 / image_height,
            ),
        );
        if mirrored {
            std::mem::swap(&mut uv.min.x, &mut uv.max.x);
        }
        ModIconGeometry { destination, uv }
    }
}
fn frame_icon(
    gpu: &UiFrame<'_>,
    painter: &Painter,
    layer: &ModIconLayer,
    center: Pos2,
    scale: f32,
    mirrored: bool,
) {
    let geometry = layer.geometry(center, scale, mirrored);
    gpu.image(painter, &layer.image_url, geometry.destination, Some(geometry.uv), layer.tint);
}

pub fn mod_item_icon(
    gpu: &UiFrame<'_>,
    painter: &Painter,
    images: Option<&ModPresentationAssets>,
    item: &HubInventoryItem,
    center: Pos2,
    scale: f32,
) -> Result<bool, String> {
    let Some(layers) = mod_item_icon_layers(images, item)? else { return Ok(false) };
    paint_mod_item_icon_layers(gpu, painter, &layers, center, scale);
    Ok(true)
}

pub(crate) fn mod_item_icon_layers(
    images: Option<&ModPresentationAssets>,
    item: &HubInventoryItem,
) -> Result<Option<Vec<ModIconLayer>>, String> {
    let identity = if let Some(content) = &item.mod_content {
        Some((&content.mod_id, &content.icon.image_path, content.icon.frame))
    } else {
        item.mod_item_content
            .as_ref()
            .map(|content| (&content.mod_id, &content.icon.image_path, content.icon.frame))
    };
    let Some((mod_id, path, frame)) = identity else { return Ok(None) };
    let images = images.ok_or("Mod inventory images are not installed")?;
    let tinted =
        item.mod_item_content.as_ref().and_then(|content| content.wearable.as_ref()).is_some_and(
            |wearable| matches!(wearable.slot, ModWearableSlot::Hat | ModWearableSlot::Robe),
        );
    let mut layers = vec![path.as_str()];
    if let Some(path) =
        item.mod_item_content.as_ref().and_then(|content| content.icon_trim_image_path.as_deref())
    {
        layers.push(path);
    }
    let mut resolved = Vec::new();
    for (index, path) in layers.into_iter().enumerate() {
        let image = images.image(mod_id, path)?;
        let tint = if tinted {
            item.icon_tints.and_then(|colors| colors[index]).unwrap_or(0xffffff)
        } else {
            0xffffff
        };
        resolved.push(ModIconLayer {
            image_url: image.url().to_owned(),
            dimensions: image.dimensions(),
            frame,
            tint: Color32::from_rgb((tint >> 16) as u8, (tint >> 8) as u8, tint as u8),
        });
    }
    Ok(Some(resolved))
}

pub(crate) fn paint_mod_item_icon_layers(
    gpu: &UiFrame<'_>,
    painter: &Painter,
    layers: &[ModIconLayer],
    center: Pos2,
    scale: f32,
) {
    for layer in layers {
        frame_icon(gpu, painter, layer, center, scale, false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn trimmed_mod_frames_keep_their_center_offset_and_mirror_the_registered_crop() {
        let layer = ModIconLayer {
            image_url: "verified.png".into(),
            dimensions: [200, 100],
            tint: Color32::WHITE,
            frame: ModSpriteFrame {
                x: 20.0,
                y: 10.0,
                width: 40.0,
                height: 20.0,
                logical_width: 100.0,
                logical_height: 80.0,
                content_width: 40.0,
                content_height: 20.0,
                center_offset_x: 7.0,
                center_offset_y: -5.0,
            },
        };
        let ordinary = layer.geometry(pos2(50.0, 60.0), 2.0, false);
        assert_eq!(ordinary.destination, Rect::from_min_size(pos2(24.0, 30.0), vec2(80.0, 40.0)));
        assert_eq!(ordinary.uv, Rect::from_min_max(pos2(0.1, 0.1), pos2(0.3, 0.3)));
        let mirrored = layer.geometry(pos2(50.0, 60.0), 2.0, true);
        assert_eq!(mirrored.destination, Rect::from_min_size(pos2(-4.0, 30.0), vec2(80.0, 40.0)));
        assert_eq!(mirrored.uv, Rect::from_min_max(pos2(0.3, 0.1), pos2(0.1, 0.3)));
    }
}
