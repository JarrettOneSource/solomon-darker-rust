//! GPU menu navigation uses egui's retained real widget geometry. Standard
//! gamepad events share these keys through the common browser input adapter.

use egui::{Context, Id};

pub(crate) struct MenuNavigation {
    scope: String,
    needs_focus: bool,
    confirmation: Option<String>,
}
impl MenuNavigation {
    pub fn new() -> Self {
        Self { scope: String::new(), needs_focus: true, confirmation: None }
    }
    pub fn release(&mut self) {
        self.scope.clear();
        self.needs_focus = true;
        self.confirmation = None;
    }
    pub fn update(&mut self, ctx: &Context, scope: &str, preferred: Option<&str>) {
        self.set_focus(ctx, scope, preferred, true);
    }
    /// Call before painting when the actual widget will process initial Enter itself.
    pub fn focus(&mut self, ctx: &Context, scope: &str, preferred: Option<&str>) {
        self.set_focus(ctx, scope, preferred, false);
    }
    fn set_focus(
        &mut self,
        ctx: &Context,
        scope: &str,
        preferred: Option<&str>,
        capture_initial_enter: bool,
    ) {
        if self.scope != scope {
            self.scope = scope.to_owned();
            self.needs_focus = true;
        }
        if self.needs_focus
            && let Some(preferred) = preferred
        {
            if capture_initial_enter
                && ctx.memory(|memory| memory.focused().is_none())
                && ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Enter))
            {
                self.confirmation = Some(preferred.to_owned());
            }
            ctx.memory_mut(|memory| memory.request_focus(Id::unique(preferred)));
            self.needs_focus = false;
        }
    }
    pub fn take_confirmation(&mut self) -> Option<String> {
        self.confirmation.take()
    }
}
