//! Platform services used by the shared client. Native builds call the operating
//! system; browser builds use their existing Web APIs.

#[cfg(target_arch = "wasm32")]
mod browser;
pub mod files;
pub(crate) mod format;
pub(crate) mod http;
mod keyboard;
#[cfg(not(target_arch = "wasm32"))]
mod native;
mod surface;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use keyboard::dispatch_keyboard;
pub use keyboard::{KeyStroke, KeyboardEffect, KeyboardEvent, KeyboardListener};

pub use surface::Surface;

#[cfg(target_arch = "wasm32")]
pub use browser::*;
#[cfg(not(target_arch = "wasm32"))]
pub use native::*;

pub fn menu_gamepad() -> Result<solomon_sim::input::MenuGamepadState, String> {
    use solomon_sim::input::{MenuGamepadButtons, MenuGamepadState, read_menu_gamepad};
    for pad in gamepads()?.into_iter().filter(|pad| pad.connected && pad.standard) {
        let buttons = pad.buttons;
        let state = read_menu_gamepad(
            pad.left,
            MenuGamepadButtons {
                up: buttons[12],
                down: buttons[13],
                left: buttons[14],
                right: buttons[15],
                confirm: buttons[0],
                back: buttons[1],
                previous: buttons[4],
                next: buttons[5],
            },
        );
        if state.active() {
            return Ok(state);
        }
    }
    Ok(MenuGamepadState::default())
}

/// UUID v4 identity for local UI transactions, using the platform's secure entropy.
pub fn random_uuid() -> Result<String, String> {
    let mut bytes = [0_u8; 16];
    random_bytes(&mut bytes)?;
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    Ok(format!("{}-{}-{}-{}-{}", &hex[..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..]))
}

/// Consume an explicit return link without retaining it in navigation history.
pub fn take_query_parameter(key: &str, expected: &str) -> Result<bool, String> {
    let mut url = location()?;
    if url.query_pairs().find(|(name, _)| name == key).is_none_or(|(_, value)| value != expected) {
        return Ok(false);
    }
    let pairs: Vec<_> = url
        .query_pairs()
        .filter(|(name, _)| name != key)
        .map(|(name, value)| (name.into_owned(), value.into_owned()))
        .collect();
    url.set_query(None);
    if !pairs.is_empty() {
        url.query_pairs_mut().extend_pairs(pairs);
    }
    replace_location(&url)?;
    Ok(true)
}
