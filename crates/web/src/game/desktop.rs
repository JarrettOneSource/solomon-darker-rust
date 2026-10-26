//! Save-before-close uses the same source checkpoint and durable revision owner
//! as Save and Leave. The OS loop keeps polling it even when drawing is suspended.

use super::Game;
use crate::menu::MenuAction;

pub(super) enum DesktopClose {
    Saving,
    Complete(Result<(), String>),
}

impl Game {
    pub fn cancel_desktop_close(&mut self) {
        self.desktop_close = None;
    }

    pub fn begin_desktop_close(&mut self) -> Result<bool, String> {
        let Some(native) = &self.native else { return Ok(false) };
        if !native.is_playing() && !native.is_leaving() {
            return Ok(false);
        }
        self.desktop_close = Some(DesktopClose::Saving);
        if !native.is_leaving() {
            self.native_menu_action(&MenuAction::Leave)?;
        }
        Ok(true)
    }

    pub(super) fn finish_desktop_close(&mut self, result: Result<(), String>) {
        if matches!(self.desktop_close, Some(DesktopClose::Saving)) {
            self.desktop_close = Some(DesktopClose::Complete(result));
        }
    }

    pub fn poll_desktop_close(&mut self, now: f64) -> Result<bool, String> {
        if matches!(self.desktop_close, Some(DesktopClose::Saving)) {
            self.poll_native(now)?;
            self.poll_hosted(now)?;
        }
        if matches!(self.desktop_close, Some(DesktopClose::Complete(_))) {
            let Some(DesktopClose::Complete(result)) = self.desktop_close.take() else {
                unreachable!("the completed desktop close owns its result")
            };
            return result.map(|()| true);
        }
        Ok(false)
    }
}
