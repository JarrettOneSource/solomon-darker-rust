//! Native menu key admission from the current scene and Source input transitions.

use crate::gpu_ui::BrowserKey;

#[derive(Default)]
pub(super) struct MenuKeyboard {
    discard: bool,
}

impl MenuKeyboard {
    pub(super) fn set_gameplay_enabled(&mut self, enabled: bool) {
        if !enabled {
            self.discard = true;
        }
    }

    pub(super) fn discard_pending(&mut self) {
        self.discard = true;
    }

    pub(super) fn take(&mut self, keys: &[BrowserKey], _gameplay: bool) -> Vec<BrowserKey> {
        if std::mem::take(&mut self.discard) {
            return Vec::new();
        }
        keys.iter()
            .filter(|key| key.pressed && !key.repeat && !key.alt && !key.ctrl && !key.meta)
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: &str) -> BrowserKey {
        BrowserKey {
            code: code.to_owned(),
            key: code.to_owned(),
            pressed: true,
            repeat: false,
            alt: false,
            ctrl: false,
            shift: false,
            meta: false,
        }
    }

    #[test]
    fn gameplay_owner_survives_layout_and_pointer_only_passes_until_the_scene_releases_it() {
        for code in ["Escape", "F2"] {
            let mut input = MenuKeyboard::default();
            input.set_gameplay_enabled(false);
            assert!(input.take(&[], true).is_empty(), "empty layout");
            assert!(input.take(&[], true).is_empty(), "pointer-only dispatch");
            assert!(
                input.take(&[key(code)], true).is_empty(),
                "the actual book/dialogue still owns {code} after empty passes"
            );
            input.set_gameplay_enabled(true);
            let admitted = input.take(&[key(code)], true);
            assert_eq!(admitted.len(), 1, "the next real key may open Pause after release");
            assert_eq!(admitted[0].code, code);
        }
    }

    #[test]
    fn an_open_menu_keeps_its_own_keys_while_gameplay_is_sealed() {
        let mut input = MenuKeyboard::default();
        input.set_gameplay_enabled(false);
        assert!(input.take(&[], true).is_empty());
        let admitted = input.take(&[key("Escape")], false);
        assert_eq!(admitted.len(), 1, "Pause/Settings/Title remain real menu owners");
    }

    #[test]
    fn a_source_input_transition_discards_its_pending_key_only_once() {
        let mut input = MenuKeyboard::default();
        input.discard_pending();
        assert!(input.take(&[key("Escape")], false).is_empty());
        assert_eq!(input.take(&[key("Escape")], false).len(), 1);
    }

    #[test]
    fn physical_key_filter_retains_shift_and_ignores_releases_repeats_and_control_shortcuts() {
        let mut input = MenuKeyboard::default();
        let mut shifted = key("Escape");
        shifted.shift = true;
        let mut released = key("Escape");
        released.pressed = false;
        let mut repeated = key("Escape");
        repeated.repeat = true;
        let mut control = key("Escape");
        control.ctrl = true;
        let mut meta = key("Escape");
        meta.meta = true;
        let mut alt = key("Escape");
        alt.alt = true;
        let admitted = input.take(&[released, repeated, control, meta, alt, shifted], false);
        assert_eq!(admitted.len(), 1);
        assert!(admitted[0].shift);
    }
}
