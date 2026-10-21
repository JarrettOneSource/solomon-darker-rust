//! Wizard-name admission and stock heading ink from Website `create-wizard-name.ts`.
//! The editable browser input is semantic; its visible text is this native glyph layout.

use crate::native_ui_text::{
    NativeUiFontName, NativeUiGlyphLayout, NativeUiTextAlign, NativeUiTextSpec,
    layout_native_ui_text, measure_native_ui_text, native_ui_font, native_ui_glyph_ink_bounds,
};

pub const CREATE_WIZARD_NAME_MAX_LENGTH: usize = 64;
pub const CREATE_WIZARD_NAME_MAX_WIDTH: f64 = 372.0;
/// CreateWizardMenu::Update, 0058A8FF: an empty final TextBox uses this literal.
pub const NATIVE_EMPTY_WIZARD_NAME: &str = "Genericus";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CreateWizardNameRejection {
    Empty,
    TooLong,
    Unsupported,
    TooWide,
}

impl CreateWizardNameRejection {
    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::Empty => "Enter a wizard name.",
            Self::TooLong => "Wizard names may contain at most 64 characters.",
            Self::Unsupported => "Use letters, numbers, or ! , . / : ? only.",
            Self::TooWide => "Wizard name is too wide.",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CreateWizardNameLayout {
    pub glyphs: Vec<NativeUiGlyphLayout>,
    pub height: f64,
    pub left: f64,
    pub right: f64,
    pub top: f64,
    pub value: String,
    pub width: f64,
}

/// Remove unsupported account-name characters and stop before the first width overflow.
#[must_use]
pub fn initial_create_wizard_name(value: &str) -> String {
    let mut supported = String::new();
    for character in value.chars() {
        if !supported_character(character) {
            continue;
        }
        let mut next = supported.clone();
        next.push(character);
        if next.encode_utf16().count() > CREATE_WIZARD_NAME_MAX_LENGTH
            || measure_create_wizard_name(&next) > CREATE_WIZARD_NAME_MAX_WIDTH
        {
            break;
        }
        supported = next;
    }
    if supported.is_empty() { "Helvidius".to_owned() } else { supported }
}

/// `None` means the original value is accepted; admission does not trim or normalize it.
#[must_use]
pub fn validate_create_wizard_name(value: &str) -> Option<CreateWizardNameRejection> {
    if value.is_empty() {
        return Some(CreateWizardNameRejection::Empty);
    }
    if value.encode_utf16().count() > CREATE_WIZARD_NAME_MAX_LENGTH {
        return Some(CreateWizardNameRejection::TooLong);
    }
    if !value.chars().all(supported_character) {
        return Some(CreateWizardNameRejection::Unsupported);
    }
    if measure_create_wizard_name(value) > CREATE_WIZARD_NAME_MAX_WIDTH {
        return Some(CreateWizardNameRejection::TooWide);
    }
    None
}

/// Current Source allows an empty editable draft and substitutes its native name
/// only when validating the final manual or random loadout.
pub fn finalize_create_wizard_name(value: &str) -> Result<&str, CreateWizardNameRejection> {
    let name = if value.is_empty() { NATIVE_EMPTY_WIZARD_NAME } else { value };
    if let Some(reason) = validate_create_wizard_name(name) {
        return Err(reason);
    }
    Ok(name)
}

#[must_use]
pub fn measure_create_wizard_name(value: &str) -> f64 {
    let text = value.to_uppercase();
    let font = native_ui_font(NativeUiFontName::Heading);
    assert!(
        text.chars().all(|character| font.glyph(character as u32).is_some()),
        "cannot measure an unsupported wizard-name character"
    );
    measure_native_ui_text(&text, NativeUiFontName::Heading, 1.0)
}

#[must_use]
pub fn layout_create_wizard_name(value: &str) -> CreateWizardNameLayout {
    assert!(validate_create_wizard_name(value).is_none(), "wizard name is admitted before layout");
    let value = value.to_uppercase();
    let mut layout = layout_native_ui_text(&NativeUiTextSpec {
        align: NativeUiTextAlign::Left,
        ..NativeUiTextSpec::new(NativeUiFontName::Heading, &value, 0.0, 0.0)
    });
    let mut minimum_left = f64::INFINITY;
    let mut minimum_top = f64::INFINITY;
    let mut maximum_right = f64::NEG_INFINITY;
    let mut maximum_bottom = f64::NEG_INFINITY;
    for glyph in &layout.glyphs {
        let ink = native_ui_glyph_ink_bounds(glyph);
        minimum_left = minimum_left.min(ink.left);
        minimum_top = minimum_top.min(ink.top);
        maximum_right = maximum_right.max(ink.left + ink.width);
        maximum_bottom = maximum_bottom.max(ink.top + ink.height);
    }
    minimum_left = minimum_left.floor();
    minimum_top = minimum_top.floor();
    maximum_right = maximum_right.ceil();
    maximum_bottom = maximum_bottom.ceil();
    let width = maximum_right - minimum_left;
    let height = maximum_bottom - minimum_top;
    let left = 50.0 + (384.0 - width) / 2.0;
    for glyph in &mut layout.glyphs {
        glyph.center_x += left - minimum_left;
        glyph.center_y += 19.0 - minimum_top;
    }
    CreateWizardNameLayout {
        glyphs: layout.glyphs,
        height,
        left,
        right: left + width,
        top: 19.0,
        value,
        width,
    }
}

fn supported_character(character: char) -> bool {
    let upper = character.to_uppercase().to_string();
    // Source tests JavaScript upper.length === 1, so only one BMP code unit qualifies.
    upper.encode_utf16().count() == 1
        && native_ui_font(NativeUiFontName::Heading)
            .glyph(upper.chars().next().expect("uppercase character exists") as u32)
            .is_some()
}
