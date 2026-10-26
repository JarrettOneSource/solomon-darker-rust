use std::future::Future;

pub fn epoch_ms() -> f64 {
    js_sys::Date::now()
}

pub fn monotonic_ms() -> f64 {
    web_sys::window().and_then(|window| window.performance()).expect("client clock").now()
}

pub fn iso_now() -> String {
    js_sys::Date::new_0().to_iso_string().into()
}

pub fn random_unit() -> f64 {
    js_sys::Math::random()
}

pub fn random_bytes(bytes: &mut [u8]) -> Result<(), String> {
    web_sys::window()
        .ok_or("no window")?
        .crypto()
        .map_err(|_| "random source")?
        .get_random_values_with_u8_array(bytes)
        .map_err(|_| "random generation")?;
    Ok(())
}

pub fn spawn_local(future: impl Future<Output = ()> + 'static) {
    wasm_bindgen_futures::spawn_local(future);
}

pub async fn next_frame() -> Result<(), String> {
    let window = web_sys::window().ok_or("No animation frame owner")?;
    let promise = js_sys::Promise::new(&mut |resolve, reject| {
        if let Err(value) = window.request_animation_frame(&resolve) {
            let _ = reject.call1(&wasm_bindgen::JsValue::UNDEFINED, &value);
        }
    });
    wasm_bindgen_futures::JsFuture::from(promise)
        .await
        .map_err(|error| format!("Animation frame: {error:?}"))?;
    Ok(())
}

pub fn error(message: &str) {
    web_sys::console::error_1(&message.into());
}

pub fn warn(message: &str) {
    web_sys::console::warn_1(&message.into());
}

pub fn coarse_pointer() -> bool {
    media_matches("(hover:none) and (pointer:coarse)")
}

pub fn reduced_motion() -> bool {
    media_matches("(prefers-reduced-motion: reduce)")
}

fn media_matches(query: &str) -> bool {
    web_sys::window()
        .and_then(|window| window.match_media(query).ok().flatten())
        .is_some_and(|query| query.matches())
}

pub fn gamepads() -> Result<Vec<solomon_sim::input::StandardGamepadSnapshot>, String> {
    use solomon_sim::Vec2;
    use wasm_bindgen::{JsCast, JsValue};
    use web_sys::{Gamepad, GamepadButton, GamepadMappingType};
    let window = web_sys::window().ok_or("No gamepad window")?;
    let navigator = window.navigator();
    let function = js_sys::Reflect::get(navigator.as_ref(), &JsValue::from_str("getGamepads"))
        .map_err(|_| "cannot inspect gamepad support")?;
    if !function.is_function() {
        return Ok(Vec::new());
    }
    let values = navigator.get_gamepads().map_err(|_| "browser denied gamepad access")?;
    let mut pads = Vec::new();
    for value in values {
        if value.is_null() || value.is_undefined() {
            continue;
        }
        let pad: Gamepad = value.dyn_into().map_err(|_| "browser returned an invalid gamepad")?;
        let axes = pad.axes();
        let buttons = pad.buttons();
        let mut pressed = [false; 16];
        for (index, pressed) in pressed.iter_mut().enumerate() {
            *pressed = buttons
                .get(index as u32)
                .dyn_into::<GamepadButton>()
                .is_ok_and(|button| button.pressed() || button.value() > 0.5);
        }
        pads.push(solomon_sim::input::StandardGamepadSnapshot {
            index: pad.index(),
            connected: pad.connected(),
            standard: pad.mapping() == GamepadMappingType::Standard,
            left: Vec2::new(
                axes.get(0).as_f64().unwrap_or(0.0),
                axes.get(1).as_f64().unwrap_or(0.0),
            ),
            right: Vec2::new(
                axes.get(2).as_f64().unwrap_or(0.0),
                axes.get(3).as_f64().unwrap_or(0.0),
            ),
            buttons: pressed,
        });
    }
    Ok(pads)
}

/// Start while the caller still owns its trusted click; await only the receipt.
pub fn copy_text(
    text: &str,
) -> Result<futures_util::future::LocalBoxFuture<'static, Result<(), String>>, String> {
    use wasm_bindgen::JsCast;
    let navigator = web_sys::window().ok_or("Clipboard window unavailable")?.navigator();
    let clipboard = js_sys::Reflect::get(&navigator, &"clipboard".into())
        .map_err(|_| "Clipboard unavailable")?;
    let write: js_sys::Function = js_sys::Reflect::get(&clipboard, &"writeText".into())
        .map_err(|_| "Clipboard unavailable")?
        .dyn_into()
        .map_err(|_| "Clipboard unavailable")?;
    let promise: js_sys::Promise = write
        .call1(&clipboard, &text.into())
        .map_err(|_| "The text could not be copied")?
        .dyn_into()
        .map_err(|_| "Invalid clipboard receipt")?;
    Ok(Box::pin(async move {
        wasm_bindgen_futures::JsFuture::from(promise)
            .await
            .map(|_| ())
            .map_err(|_| "The text could not be copied".into())
    }))
}

pub fn location() -> Result<url::Url, String> {
    let href = web_sys::window()
        .ok_or("no window")?
        .location()
        .href()
        .map_err(|_| "Could not read the game URL")?;
    url::Url::parse(&href).map_err(|error| format!("Invalid game URL: {error}"))
}

pub fn website_mount() -> Result<Option<crate::hosted::WebsiteMount>, String> {
    crate::hosted::WebsiteMount::configured(&web_sys::window().ok_or("no window")?)
}

pub(super) fn replace_location(url: &url::Url) -> Result<(), String> {
    web_sys::window()
        .ok_or("no window")?
        .history()
        .map_err(|_| "Game navigation history")?
        .replace_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(url.as_str()))
        .map_err(|_| "Could not update the game location".into())
}

pub fn navigate(url: &str) -> Result<(), String> {
    web_sys::window()
        .ok_or("no window")?
        .location()
        .assign(url)
        .map_err(|_| "Could not open the requested page".into())
}

pub fn reload() -> Result<(), String> {
    web_sys::window()
        .ok_or("no window")?
        .location()
        .reload()
        .map_err(|_| "Could not reload the game".into())
}

pub fn network_origin() -> Result<String, String> {
    Ok(location()?.origin().ascii_serialization())
}
