//! Shared font names, readiness and GPU glyph registration. Platform adapters
//! load stock faces and use their native text rasterizer.

use egui::epaint::text::{FontPriority, GlyphRasterizer};
use egui::{Context, FontDefinitions, FontFamily};
use std::cell::RefCell;

#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
use browser as platform;
#[cfg(not(target_arch = "wasm32"))]
use native as platform;
#[cfg(all(test, not(target_arch = "wasm32")))]
pub(crate) use native::test_fonts;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use native::with_fonts;
pub(super) use platform::{PlatformFonts, load};

thread_local! {
    static GLYPH_ERROR: RefCell<Option<String>> = const { RefCell::new(None) };
}

const FACES: [(&str, &str, &str, &str); 9] = [
    ("SDGPU Cinzel", "cinzel-latin-500-normal.woff2", "500", "normal"),
    ("SDGPU Cinzel", "cinzel-latin-700-normal.woff2", "700", "normal"),
    ("SDGPU Cinzel", "cinzel-latin-800-normal.woff2", "800", "normal"),
    ("SDGPU Alegreya", "alegreya-latin-400-normal.woff2", "400", "normal"),
    ("SDGPU Alegreya", "alegreya-latin-500-normal.woff2", "500", "normal"),
    ("SDGPU Alegreya", "alegreya-latin-700-normal.woff2", "700", "normal"),
    ("SDGPU Alegreya", "alegreya-latin-400-italic.woff2", "400", "italic"),
    ("SDGPU JetBrains Mono", "jetbrains-mono-latin-400-normal.woff2", "400", "normal"),
    ("SDGPU JetBrains Mono", "jetbrains-mono-latin-600-normal.woff2", "600", "normal"),
];

pub(super) fn startup_resources() -> Vec<crate::runtime_ui::StartupResource> {
    FACES
        .into_iter()
        .map(|(_, file, _, _)| {
            crate::runtime_ui::StartupResource::new("fonts", format!("assets/stock/fonts/{file}"))
        })
        .collect()
}

pub(super) fn font_spec(family: &FontFamily) -> (&'static str, &'static str, &'static str) {
    let name = match family {
        FontFamily::Name(name) => name.as_ref(),
        FontFamily::Monospace => "JetBrains Mono",
        FontFamily::Proportional => "Georgia",
    };
    match name {
        "Cinzel" => ("500", "normal", "'SDGPU Cinzel','Times New Roman',serif"),
        "Cinzel Semibold" => ("600", "normal", "'SDGPU Cinzel','Times New Roman',serif"),
        "Cinzel Bold" => ("700", "normal", "'SDGPU Cinzel','Times New Roman',serif"),
        "Cinzel ExtraBold" => ("800", "normal", "'SDGPU Cinzel','Times New Roman',serif"),
        "Alegreya" => ("400", "normal", "'SDGPU Alegreya',Georgia,serif"),
        "Alegreya Medium" => ("500", "normal", "'SDGPU Alegreya',Georgia,serif"),
        // Source requests600 with only its400/500/700 faces imported; keep native matching.
        "Alegreya Semibold" => ("600", "normal", "'SDGPU Alegreya',Georgia,serif"),
        "Alegreya Bold" => ("700", "normal", "'SDGPU Alegreya',Georgia,serif"),
        "Alegreya Italic" => ("400", "italic", "'SDGPU Alegreya',Georgia,serif"),
        "Alegreya Bold Italic" => ("700", "italic", "'SDGPU Alegreya',Georgia,serif"),
        "JetBrains Mono" => ("400", "normal", "'SDGPU JetBrains Mono',monospace"),
        "JetBrains Mono Semibold" => ("600", "normal", "'SDGPU JetBrains Mono',monospace"),
        "JetBrains Mono Bold" => ("700", "normal", "'SDGPU JetBrains Mono',monospace"),
        "Palatino" => ("400", "normal", "Palatino,'Palatino Linotype',Georgia,serif"),
        "Arial" => ("400", "normal", "Arial,Helvetica,sans-serif"),
        "IM FELL English" => ("400", "normal", "'IM FELL English',Georgia,serif"),
        "IM FELL English Bold" => ("700", "normal", "'IM FELL English',Georgia,serif"),
        "Georgia Bold" => ("700", "normal", "Georgia,'Times New Roman',serif"),
        _ => ("400", "normal", "Georgia,'Times New Roman',serif"),
    }
}

pub(super) fn font_css(family: &FontFamily, size: f32) -> String {
    let (weight, style, family) = font_spec(family);
    format!("{style} {weight} {size}px {family}")
}

fn configure(ctx: &Context) {
    let mut definitions = FontDefinitions::default();
    let fallback = definitions.families[&FontFamily::Proportional].clone();
    for family in [
        "Georgia",
        "Georgia Bold",
        "Arial",
        "Palatino",
        "Cinzel",
        "Cinzel Semibold",
        "Cinzel Bold",
        "Cinzel ExtraBold",
        "Alegreya",
        "Alegreya Medium",
        "Alegreya Semibold",
        "Alegreya Bold",
        "Alegreya Italic",
        "Alegreya Bold Italic",
        "JetBrains Mono",
        "JetBrains Mono Semibold",
        "JetBrains Mono Bold",
        "IM FELL English",
        "IM FELL English Bold",
    ] {
        definitions.families.insert(FontFamily::Name(family.into()), fallback.clone());
    }
    ctx.set_fonts(definitions);
    ctx.set_glyph_rasterizers(vec![
        GlyphRasterizer::new("solomon::platform_fonts", |request| {
            match platform::rasterize(request) {
                Ok(glyph) => Some(glyph),
                Err(error) => {
                    GLYPH_ERROR.with(|slot| *slot.borrow_mut() = Some(error));
                    None
                }
            }
        })
        .with_priority(FontPriority::Highest),
    ]);
}

pub(super) fn take_error() -> Option<String> {
    GLYPH_ERROR.with(|slot| slot.borrow_mut().take())
}
