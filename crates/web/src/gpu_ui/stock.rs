//! Stock `NativeUiSprite`, `NativeUiStrip`, `NativeUiNineSlice` and bitmap text.
//!
//! Geometry is the existing `ui.rs` Website projection expressed as GPU quads;
//! bundle registration and `sim::native_ui_text` remain their canonical owners.

use egui::{Color32, Painter, Pos2, Rect, Vec2, pos2, vec2};
use solomon_assets::{SpriteRecord, parse_bundle};
use solomon_sim::native_ui_text::{
    NativeUiTextLayout, NativeUiTextSpec, layout_native_ui_text, native_ui_font,
    native_ui_glyph_ink_bounds,
};

use super::UiFrame;
use solomon_graphics::assets;

pub struct StockAtlas {
    name: &'static str,
    path: String,
    records: Vec<SpriteRecord>,
}

impl StockAtlas {
    pub async fn load(name: &'static str) -> Result<Self, String> {
        let bytes = assets::fetch_bytes(&format!("assets/stock/{name}.bundle")).await?;
        let records = parse_bundle(&bytes).map_err(|error| format!("{name}.bundle: {error}"))?;
        Ok(Self { name, path: format!("assets/stock/{name}.png"), records })
    }

    pub fn record(&self, index: usize) -> Result<&SpriteRecord, String> {
        self.records.get(index).ok_or_else(|| format!("{}.bundle has no record {index}", self.name))
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn paint(
        &self,
        frame: &UiFrame<'_>,
        painter: &Painter,
        index: usize,
        origin: Pos2,
        scale: f32,
        tint: Color32,
    ) -> Result<(), String> {
        let record = self.record(index)?;
        let [x, y] = record.registered_origin();
        self.paint_raw(
            frame,
            painter,
            index,
            Rect::from_min_size(
                origin + vec2(x, y) * scale,
                vec2(record.width as f32, record.height as f32) * scale,
            ),
            tint,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn paint_centered(
        &self,
        frame: &UiFrame<'_>,
        painter: &Painter,
        index: usize,
        center: Pos2,
        scale: f32,
        rotation: f32,
        tint: Color32,
    ) -> Result<(), String> {
        let record = self.record(index)?;
        let [x, y] = record.registered_origin();
        let origin = center
            - vec2(record.logical_width as f32, record.logical_height as f32) * (scale * 0.5);
        let destination = Rect::from_min_size(
            origin + vec2(x, y) * scale,
            vec2(record.width as f32, record.height as f32) * scale,
        );
        let size =
            frame.image_size(&self.path).ok_or_else(|| format!("{} is not ready", self.path))?;
        let source = record_rect(record);
        let uv = Rect::from_min_max(
            (source.min.to_vec2() / size).to_pos2(),
            (source.max.to_vec2() / size).to_pos2(),
        );
        frame.image_rotated(painter, &self.path, destination, Some(uv), rotation, center, tint);
        Ok(())
    }

    pub fn paint_raw(
        &self,
        frame: &UiFrame<'_>,
        painter: &Painter,
        index: usize,
        destination: Rect,
        tint: Color32,
    ) -> Result<(), String> {
        let record = self.record(index)?;
        if frame.atlas_image(painter, &self.path, record_rect(record), destination, tint) {
            Ok(())
        } else {
            Err(format!("{} is not ready", self.path))
        }
    }

    pub fn paint_slice(
        &self,
        frame: &UiFrame<'_>,
        painter: &Painter,
        index: usize,
        source: Rect,
        destination: Rect,
        tint: Color32,
    ) -> Result<(), String> {
        let record = self.record(index)?;
        let source = source.translate(vec2(record.x as f32, record.y as f32));
        if frame.atlas_image(painter, &self.path, source, destination, tint) {
            Ok(())
        } else {
            Err(format!("{} is not ready", self.path))
        }
    }

    pub fn paint_tiles(
        &self,
        frame: &UiFrame<'_>,
        painter: &Painter,
        index: usize,
        destination: Rect,
        scale: f32,
        tint: Color32,
    ) -> Result<(), String> {
        if !destination.is_positive() {
            return Ok(());
        }
        let record = self.record(index)?;
        let tile = vec2(record.width as f32, record.height as f32) * scale;
        if tile.x <= 0.0 || tile.y <= 0.0 {
            return Err("stock UI tile scale must be positive".to_owned());
        }
        let painter = painter.with_clip_rect(painter.clip_rect().intersect(destination));
        let mut y = destination.top();
        while y < destination.bottom() {
            let mut x = destination.left();
            while x < destination.right() {
                self.paint_raw(
                    frame,
                    &painter,
                    index,
                    Rect::from_min_size(pos2(x, y), tile),
                    tint,
                )?;
                x += tile.x;
            }
            y += tile.y;
        }
        Ok(())
    }

    pub fn paint_frame(
        &self,
        frame: &UiFrame<'_>,
        painter: &Painter,
        index: usize,
        destination: Rect,
        border: f32,
        tint: Color32,
    ) -> Result<(), String> {
        let record = self.record(index)?;
        let source = vec2(record.width as f32, record.height as f32);
        let scaled = border
            * 1.0_f32
                .min(destination.width() / (2.0 * border))
                .min(destination.height() / (2.0 * border));
        let sx = [0.0, border, source.x - border, source.x];
        let sy = [0.0, border, source.y - border, source.y];
        let dx = [
            destination.left(),
            destination.left() + scaled,
            destination.right() - scaled,
            destination.right(),
        ];
        let dy = [
            destination.top(),
            destination.top() + scaled,
            destination.bottom() - scaled,
            destination.bottom(),
        ];
        for y in 0..3 {
            for x in 0..3 {
                let dest = Rect::from_min_max(pos2(dx[x], dy[y]), pos2(dx[x + 1], dy[y + 1]));
                if dest.is_positive() {
                    self.paint_slice(
                        frame,
                        painter,
                        index,
                        Rect::from_min_max(pos2(sx[x], sy[y]), pos2(sx[x + 1], sy[y + 1])),
                        dest,
                        tint,
                    )?;
                }
            }
        }
        Ok(())
    }

    pub fn paint_native_frame(
        &self,
        frame: &UiFrame<'_>,
        painter: &Painter,
        index: usize,
        destination: Rect,
        fill: bool,
        tint: Color32,
    ) -> Result<(), String> {
        let record = self.record(index)?;
        let size = vec2(record.width as f32, record.height as f32);
        let texture_size =
            frame.image_size(&self.path).ok_or_else(|| format!("{} is not ready", self.path))?;
        let painter = painter.with_clip_rect(painter.clip_rect().intersect(destination));
        let edge = size * 0.05;
        // Recovered +1 overlap prevents seams between half-open mirrored corners.
        let inner = destination.size() - size * 2.0 + Vec2::splat(1.0);
        let right = destination.width() - size.x + 1.0;
        let bottom = destination.height() - size.y + 1.0;
        let pieces = [
            ([0.0, 0.0, size.x, size.y], [0.0, 0.0, size.x, size.y], false, false),
            ([0.0, 0.0, size.x, size.y], [right, 0.0, size.x, size.y], true, false),
            ([0.0, 0.0, size.x, size.y], [0.0, bottom, size.x, size.y], false, true),
            ([0.0, 0.0, size.x, size.y], [right, bottom, size.x, size.y], true, true),
            ([size.x - edge.x, 0.0, edge.x, size.y], [size.x, 0.0, inner.x, size.y], false, false),
            (
                [size.x - edge.x, 0.0, edge.x, size.y],
                [size.x, bottom, inner.x, size.y],
                false,
                true,
            ),
            ([0.0, size.y - edge.y, size.x, edge.y], [0.0, size.y, size.x, inner.y], false, false),
            ([0.0, size.y - edge.y, size.x, edge.y], [right, size.y, size.x, inner.y], true, false),
            (
                [size.x - edge.x, size.y - edge.y, edge.x, edge.y],
                [size.x, size.y, inner.x, inner.y],
                false,
                false,
            ),
        ];
        for (source, dest, mirror_x, mirror_y) in &pieces[..if fill { 9 } else { 8 }] {
            let dest = rect(*dest).translate(destination.min.to_vec2());
            if !dest.is_positive() {
                continue;
            }
            let source = rect(*source).translate(vec2(record.x as f32, record.y as f32));
            let mut uv = Rect::from_min_max(
                (source.min.to_vec2() / texture_size).to_pos2(),
                (source.max.to_vec2() / texture_size).to_pos2(),
            );
            if *mirror_x {
                std::mem::swap(&mut uv.min.x, &mut uv.max.x);
            }
            if *mirror_y {
                std::mem::swap(&mut uv.min.y, &mut uv.max.y);
            }
            frame.image(&painter, &self.path, dest, Some(uv), tint);
        }
        Ok(())
    }

    pub fn paint_strip(
        &self,
        frame: &UiFrame<'_>,
        painter: &Painter,
        index: usize,
        destination: Rect,
        tint: Color32,
    ) -> Result<(), String> {
        if !destination.is_positive() {
            return Ok(());
        }
        let record = self.record(index)?;
        let third = record.width as f32 / 3.0;
        let height = record.height as f32;
        let painter = painter.with_clip_rect(painter.clip_rect().intersect(destination));
        self.paint_slice(
            frame,
            &painter,
            index,
            rect([0.0, 0.0, third, height]),
            rect([destination.left(), destination.top(), third, destination.height()]),
            tint,
        )?;
        let mut x = third;
        while x < destination.width() - third {
            let width = third.min(destination.width() - third - x);
            self.paint_slice(
                frame,
                &painter,
                index,
                rect([third, 0.0, width, height]),
                rect([destination.left() + x, destination.top(), width, destination.height()]),
                tint,
            )?;
            x += width;
        }
        self.paint_slice(
            frame,
            &painter,
            index,
            rect([third * 2.0, 0.0, third, height]),
            rect([destination.right() - third, destination.top(), third, destination.height()]),
            tint,
        )
    }
}

fn rect([x, y, width, height]: [f32; 4]) -> Rect {
    Rect::from_min_size(pos2(x, y), vec2(width, height))
}
fn record_rect(record: &SpriteRecord) -> Rect {
    rect([record.x as f32, record.y as f32, record.width as f32, record.height as f32])
}

pub fn message_frame(
    frame: &UiFrame<'_>,
    painter: &Painter,
    atlas: &StockAtlas,
    destination: Rect,
) -> Result<(), String> {
    let width = destination.width();
    let height = destination.height();
    for (index, x, y, w, h) in [
        (49, 0.0, 0.0, width, height),
        (10, 71.5, -6.5, width - 143.0, 19.0),
        (10, 71.5, height - 12.5, width - 143.0, 19.0),
        (79, -6.5, 76.5, 21.0, height - 153.0),
        (79, width - 14.5, 76.5, 21.0, height - 153.0),
    ] {
        atlas.paint_tiles(
            frame,
            painter,
            index,
            rect([x, y, w, h]).translate(destination.min.to_vec2()),
            1.0,
            Color32::WHITE,
        )?;
    }
    for (index, x, y, scale, rotation) in [
        (107, 29.0, 32.0, 1.0, 0.0),
        (108, width - 29.0, 32.0, 1.0, 0.0),
        (109, 29.0, height - 32.0, 1.0, 0.0),
        (110, width - 29.0, height - 32.0, 1.0, 0.0),
        (18, width / 2.0, -37.0, 1.0, std::f32::consts::FRAC_PI_2),
        (8, width / 2.0, height + 50.0, 1.0, 0.0),
        (8, width / 2.0 - 75.0, height + 37.0, 0.75, 0.0),
        (8, width / 2.0 + 75.0, height + 37.0, 0.75, 0.0),
    ] {
        atlas.paint_centered(
            frame,
            painter,
            index,
            destination.min + vec2(x, y),
            scale,
            rotation,
            Color32::WHITE,
        )?;
    }
    atlas.paint_native_frame(frame, painter, 17, destination.shrink(5.0), true, Color32::WHITE)
}

pub fn native_text(
    frame: &UiFrame<'_>,
    painter: &Painter,
    spec: &NativeUiTextSpec<'_>,
) -> NativeUiTextLayout {
    let layout = layout_native_ui_text(spec);
    paint_text_layout(frame, painter, &layout);
    layout
}

pub fn paint_text_layout(frame: &UiFrame<'_>, painter: &Painter, layout: &NativeUiTextLayout) {
    let atlas = native_ui_font(layout.font).atlas;
    let [width, height] = atlas.dimensions();
    let size = vec2(width as f32, height as f32);
    let path = format!("assets/stock/{}.png", atlas.as_str());
    for glyph in &layout.glyphs {
        let ink = native_ui_glyph_ink_bounds(glyph);
        let destination =
            rect([ink.left as f32, ink.top as f32, ink.width as f32, ink.height as f32]);
        let mut positions = [
            destination.left_top(),
            destination.right_top(),
            destination.right_bottom(),
            destination.left_bottom(),
        ];
        if glyph.italic {
            let shear = (6.0 / glyph.glyph.frame[3]) as f32;
            let origin_y = ((glyph.glyph.logical_size[1] / 2.0 - glyph.glyph.trim_origin[1])
                * glyph.scale) as f32;
            for position in &mut positions {
                position.x += (position.y - destination.top() - origin_y) * shear;
            }
        }
        let source = rect(glyph.glyph.frame.map(|value| value as f32));
        let uv = Rect::from_min_max(
            (source.min.to_vec2() / size).to_pos2(),
            (source.max.to_vec2() / size).to_pos2(),
        );
        let tint = Color32::from_rgba_unmultiplied(
            (glyph.tint >> 16) as u8,
            (glyph.tint >> 8) as u8,
            glyph.tint as u8,
            (glyph.alpha * 255.0).round().clamp(0.0, 255.0) as u8,
        );
        frame.paint_quad(painter, &path, positions, uv, tint, frame.blend, true);
    }
}
