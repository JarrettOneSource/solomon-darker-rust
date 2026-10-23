//! GPU menu navigation uses egui's retained real widget geometry. Standard
//! gamepad events share these keys through the common browser input adapter.

use egui::{Context, Id};

pub(crate) struct MenuNavigation {
    scope: String,
    needs_focus: bool,
}
impl MenuNavigation {
    pub fn new() -> Self {
        Self { scope: String::new(), needs_focus: true }
    }
    pub fn release(&mut self) {
        self.scope.clear();
        self.needs_focus = true;
    }
    pub fn update(&mut self, ctx: &Context, scope: &str, preferred: Option<&str>) {
        if self.scope != scope {
            self.scope = scope.to_owned();
            self.needs_focus = true;
        }
        if self.needs_focus
            && let Some(preferred) = preferred
        {
            ctx.memory_mut(|memory| memory.request_focus(Id::unique(preferred)));
            self.needs_focus = false;
        }
    }
}
