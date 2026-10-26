use super::{ControlMode, FullscreenRequest};
use crate::platform::Surface;

pub(super) fn mode(_: &Surface) -> Result<ControlMode, String> {
    Ok(ControlMode::Fullscreen)
}
pub(super) fn active(surface: &Surface) -> bool {
    surface.window().fullscreen().is_some()
}
pub(super) fn toggle(surface: &Surface) -> Result<FullscreenRequest, String> {
    let fullscreen = (!active(surface)).then_some(winit::window::Fullscreen::Borderless(None));
    surface.window().set_fullscreen(fullscreen);
    Ok(Box::pin(async { Ok(()) }))
}
