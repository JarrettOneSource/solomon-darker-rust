//! A custom Pixi-style trimmed frame, borrowing the session's verified image.
//! Source `renderer/mod-presentation-assets.ts` uses this registration for icons.

use solomon_sim::Vec2;
use solomon_sim::economy::HubInventoryItem;
use solomon_sim::modding::assets::ModSpriteFrame;
use solomon_sim::modding::inventory::ModWearableSlot;
use web_sys::{Document, HtmlElement};

use super::{append, element, style, tint};
use crate::mod_presentation_assets::ModPresentationAssets;

/// Resolved presentation only. Keeping the verified URL in the equality key
/// invalidates old belt art when the session revokes/replaces its asset owner.
#[derive(Clone, PartialEq)]
pub(crate) struct ModIconLayer {
    image_url: String,
    dimensions: [u32; 2],
    frame: ModSpriteFrame,
    tint: u32,
}

pub fn mod_icon(
    document: &Document,
    images: &ModPresentationAssets,
    mod_id: &str,
    path: &str,
    frame: ModSpriteFrame,
    center: Vec2,
    mirrored: bool,
) -> Result<HtmlElement, String> {
    let image = images.image(mod_id, path)?;
    frame_icon(
        document,
        &ModIconLayer {
            image_url: image.url().to_owned(),
            dimensions: image.dimensions(),
            frame,
            tint: 0xffffff,
        },
        center,
        mirrored,
    )
}

fn frame_icon(
    document: &Document,
    layer: &ModIconLayer,
    center: Vec2,
    mirrored: bool,
) -> Result<HtmlElement, String> {
    let [width, height] = layer.dimensions;
    let frame = layer.frame;
    let root = element(document, "span", "mod-frame-icon")?;
    style(
        &root,
        &format!(
            "position:absolute;display:block;left:{}px;top:{}px;width:{}px;height:{}px;transform:scaleX({});pointer-events:none",
            center.x - frame.logical_width / 2.0,
            center.y - frame.logical_height / 2.0,
            frame.logical_width,
            frame.logical_height,
            if mirrored { -1 } else { 1 },
        ),
    )?;
    let crop = element(document, "span", "mod-frame-crop")?;
    let left = (frame.logical_width - frame.width) / 2.0 + frame.center_offset_x;
    let top = (frame.logical_height - frame.height) / 2.0 + frame.center_offset_y;
    style(
        &crop,
        &format!(
            "position:absolute;display:block;left:{left}px;top:{top}px;width:{}px;height:{}px;background-image:url('{}');background-size:{width}px {height}px;background-position:{}px {}px;background-repeat:no-repeat;pointer-events:none",
            frame.width, frame.height, layer.image_url, -frame.x, -frame.y,
        ),
    )?;
    append(&root, &crop)?;
    tint(document, &root, layer.tint)?;
    Ok(root)
}

/// Source `hub-inventory/items.ts:addModItemIcon`: custom icons use no stock
/// equipment rotation/offset. Only authored Hat/Robe layers receive clothing tints.
pub fn mod_item_icon(
    document: &Document,
    images: Option<&ModPresentationAssets>,
    item: &HubInventoryItem,
    center: Vec2,
) -> Result<Option<HtmlElement>, String> {
    let Some(layers) = mod_item_icon_layers(images, item)? else { return Ok(None) };
    Ok(Some(paint_mod_item_icon_layers(document, &layers, center)?))
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
            tint,
        });
    }
    Ok(Some(resolved))
}

pub(crate) fn paint_mod_item_icon_layers(
    document: &Document,
    layers: &[ModIconLayer],
    center: Vec2,
) -> Result<HtmlElement, String> {
    let root = element(document, "span", "mod-item-icon")?;
    style(&root, "position:absolute;left:0;top:0;width:0;height:0;pointer-events:none")?;
    for layer in layers {
        append(&root, &frame_icon(document, layer, center, false)?)?;
    }
    Ok(root)
}
