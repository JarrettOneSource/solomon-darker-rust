//! Shared GPU presentation helpers and the platform's native Lua value formatting.

pub(crate) mod belt_pull_off;
pub(crate) mod item_hover;
pub(crate) mod mod_icon;
pub(crate) mod portraits;
pub(crate) mod sack_name;
pub(crate) mod skill_hover;
pub(crate) mod trusted_gesture;

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    /// Source authored values use JavaScript String coercion, including arrays and objects.
    #[wasm_bindgen(js_name = String)]
    pub(crate) fn javascript_string(value: &JsValue) -> String;
}
