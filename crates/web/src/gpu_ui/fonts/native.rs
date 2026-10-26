//! Stock WOFF2 and system fallback faces use the native Unicode/font stack.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use egui::epaint::text::{GlyphBitmap, GlyphRasterizerRequest, RasterizedGlyph};
use egui::{ColorImage, Context, vec2};
use solomon_graphics::text::{FontSystem, TextStyle};

thread_local! {
    static FONTS: RefCell<Weak<RefCell<FontSystem>>> = const { RefCell::new(Weak::new()) };
}

pub(crate) struct PlatformFonts {
    _owner: Rc<RefCell<FontSystem>>,
}

pub(crate) fn with_fonts<T>(
    run: impl FnOnce(&mut FontSystem) -> Result<T, String>,
) -> Result<T, String> {
    let fonts = FONTS
        .with(|fonts| fonts.borrow().upgrade())
        .ok_or("Native font resources are not loaded")?;
    run(&mut fonts.borrow_mut())
}

pub(crate) async fn load(
    _surface: &crate::platform::Surface,
    ctx: &Context,
) -> Result<PlatformFonts, String> {
    crate::runtime_ui::enter_phase("fonts")?;
    let mut fonts = FontSystem::default();
    for (_, file, _, _) in super::FACES {
        let path = format!("assets/stock/fonts/{file}");
        let bytes = solomon_graphics::assets::fetch_bytes(&path).await?;
        fonts.register_woff2(&bytes).map_err(|error| format!("{file}: {error}"))?;
        crate::runtime_ui::completed(&path)?;
    }
    Ok(install(fonts, ctx))
}

fn install(fonts: FontSystem, ctx: &Context) -> PlatformFonts {
    // Launcher and game windows overlap during loading. Both must retain the
    // same font owner, including when the new window fails and is discarded.
    let owner = FONTS
        .with(|fonts| fonts.borrow().upgrade())
        .unwrap_or_else(|| Rc::new(RefCell::new(fonts)));
    FONTS.with(|fonts| *fonts.borrow_mut() = Rc::downgrade(&owner));
    super::configure(ctx);
    PlatformFonts { _owner: owner }
}

#[cfg(test)]
pub(crate) fn test_fonts() -> PlatformFonts {
    let mut fonts = FontSystem::default();
    fonts
        .register_woff2(include_bytes!(
            "../../../../../web/assets/stock/fonts/alegreya-latin-400-normal.woff2"
        ))
        .unwrap();
    install(fonts, &Context::default())
}

pub(super) fn rasterize(request: &GlyphRasterizerRequest<'_>) -> Result<RasterizedGlyph, String> {
    let (weight, style, family) = super::font_spec(request.family);
    let family = family.replace("SDGPU ", "");
    with_fonts(|fonts| {
        let text = fonts.shape(
            request.cluster,
            TextStyle {
                families: &family,
                size: request.font_size_px,
                weight: weight.parse().expect("font weight"),
                italic: style == "italic",
                letter_spacing: 0.0,
                line_height: None,
            },
        )?;
        let ink = text.ink;
        let left = ink.x0.floor() - 1.0;
        let top = ink.y0.floor() - 1.0;
        let width = (ink.x1.ceil() - left + 1.0).max(1.0);
        let height = (ink.y1.ceil() - top + 1.0).max(1.0);
        if width > f64::from(u16::MAX) || height > f64::from(u16::MAX) {
            return Err("Native glyph exceeds raster dimensions".into());
        }
        let baseline = text.layout.lines().next().map_or(0.0, |line| line.metrics().baseline);
        let advance = text.layout.full_width();
        let bitmap =
            fonts.rasterize(&text, [width as u16, height as u16], [-left, -top], [255; 4], None);
        let colored = bitmap
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .any(|pixel| pixel[0] != pixel[1] || pixel[1] != pixel[2]);
        Ok(RasterizedGlyph {
            bitmap: GlyphBitmap {
                image: ColorImage::from_rgba_premultiplied(
                    [bitmap.width as usize, bitmap.height as usize],
                    &bitmap.rgba,
                ),
                offset_px: vec2(left as f32, top as f32 - baseline),
                is_color: colored,
            },
            advance_px: advance,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_fonts_survive_either_overlapping_window_retiring() {
        for retire_newest in [true, false] {
            let first = install(FontSystem::default(), &Context::default());
            let second = install(FontSystem::default(), &Context::default());
            let retained = if retire_newest {
                drop(second);
                first
            } else {
                drop(first);
                second
            };
            assert!(with_fonts(|_| Ok(())).is_ok(), "a live window still owns native fonts");
            drop(retained);
            assert!(with_fonts(|_| Ok(())).is_err(), "fonts retire with their last window");
        }
    }
}
