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
        let scope_changed = self.scope != scope;
        if scope_changed {
            self.scope = scope.to_owned();
            self.needs_focus = true;
        }
        if self.needs_focus
            && let Some(preferred) = preferred
        {
            let preferred_id = Id::unique(preferred);
            let response = ctx.read_response(preferred_id);
            let ready = response.as_ref().is_some_and(|response| {
                response.enabled() && response.visible() && response.sense.is_focusable()
            });
            // A new Area first registers invisible, disabled sizing widgets. Keep
            // requesting its default until a real widget can retain the focus.
            // Tab/click focus acquired in that layer meanwhile belongs to the user.
            if !scope_changed
                && let Some(layer) = response.as_ref().map(|response| response.layer_id)
                && let Some(focused) = ctx.memory(|memory| memory.focused())
                && let Some(focused) = ctx.read_response(focused)
                && focused.layer_id == layer
                && focused.enabled()
                && focused.visible()
                && focused.sense.is_focusable()
            {
                self.needs_focus = !ready;
                return;
            }
            if capture_initial_enter
                && ready
                && ctx.memory(|memory| memory.focused().is_none())
                && ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Enter))
            {
                self.confirmation = Some(preferred.to_owned());
            }
            ctx.memory_mut(|memory| memory.request_focus(preferred_id));
            self.needs_focus = scope_changed || !ready;
        }
    }
    pub fn take_confirmation(&mut self) -> Option<String> {
        self.confirmation.take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pass(
        ctx: &Context,
        navigation: &mut MenuNavigation,
        scope: &str,
        events: Vec<egui::Event>,
    ) -> (egui::Response, egui::Response) {
        let mut responses = None;
        ctx.run_pass(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(400.0, 300.0),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                navigation.focus(ctx, scope, Some("menu-first"));
                egui::Area::new(Id::unique("menu-navigation-test"))
                    .background_interactable(false)
                    .fade_in(false)
                    .fixed_pos(egui::Pos2::ZERO)
                    .show(ctx, |ui| {
                        let first = ui.interact(
                            egui::Rect::from_min_size(
                                egui::pos2(20.0, 20.0),
                                egui::vec2(120.0, 50.0),
                            ),
                            Id::unique("menu-first"),
                            egui::Sense::click(),
                        );
                        let second = ui.interact(
                            egui::Rect::from_min_size(
                                egui::pos2(20.0, 90.0),
                                egui::vec2(120.0, 50.0),
                            ),
                            Id::unique("menu-second"),
                            egui::Sense::click(),
                        );
                        responses = Some((first, second));
                    });
                // Like the fullscreen control: a real focusable widget on a
                // different root, outside this Source menu's controller scope.
                egui::Area::new(Id::unique("outside-menu"))
                    .background_interactable(false)
                    .fade_in(false)
                    .order(egui::Order::Foreground)
                    .fixed_pos(egui::pos2(260.0, 240.0))
                    .show(ctx, |ui| {
                        ui.interact(
                            egui::Rect::from_min_size(
                                egui::pos2(260.0, 240.0),
                                egui::vec2(100.0, 30.0),
                            ),
                            Id::unique("outside-menu-control"),
                            egui::Sense::click(),
                        );
                    });
            },
        )
        .drop_without_applying_deltas();
        responses.expect("Area painted both real widgets")
    }

    fn key(key: egui::Key) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: Some(key),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }
    }

    #[test]
    fn first_visible_widget_receives_focus_and_real_enter_after_sizing() {
        let ctx = Context::default();
        let mut navigation = MenuNavigation::new();
        let (first, _) = pass(&ctx, &mut navigation, "dialogue", Vec::new());
        assert!(!first.enabled());
        assert!(!first.visible());
        assert_eq!(ctx.memory(|memory| memory.focused()), None);

        let (first, _) = pass(&ctx, &mut navigation, "dialogue", vec![key(egui::Key::Enter)]);
        assert!(first.enabled() && first.visible());
        assert!(first.has_focus());
        assert!(first.clicked());
    }

    #[test]
    fn pending_default_preserves_real_tab_focus_but_new_scope_claims_its_default() {
        let ctx = Context::default();
        let mut navigation = MenuNavigation::new();
        pass(&ctx, &mut navigation, "dialogue-speech", Vec::new());
        let (first, _) = pass(&ctx, &mut navigation, "dialogue-speech", Vec::new());
        assert!(first.has_focus());

        let (first, second) =
            pass(&ctx, &mut navigation, "dialogue-speech", vec![key(egui::Key::Tab)]);
        assert!(!first.has_focus());
        assert!(second.has_focus());

        let (_, second) = pass(&ctx, &mut navigation, "dialogue-speech", Vec::new());
        assert!(second.has_focus());

        let (first, second) = pass(&ctx, &mut navigation, "dialogue-choices", Vec::new());
        assert!(first.has_focus());
        assert!(!second.has_focus());
    }

    #[test]
    fn controller_edges_wrap_native_menu_order_without_leaving_its_scope() {
        let ctx = Context::default();
        let mut navigation = MenuNavigation::new();
        pass(&ctx, &mut navigation, "menu", Vec::new());
        let (first, _) = pass(&ctx, &mut navigation, "menu", Vec::new());
        assert!(first.has_focus());
        let mut dispatch = |key, pressed| {
            pass(
                &ctx,
                &mut navigation,
                "menu",
                vec![egui::Event::Key {
                    key,
                    physical_key: None,
                    pressed,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
            pass(&ctx, &mut navigation, "menu", Vec::new());
            pass(&ctx, &mut navigation, "menu", Vec::new())
        };
        let (first, second) = dispatch(egui::Key::ArrowUp, true);
        assert!(!first.has_focus());
        assert!(second.has_focus(), "Source previous fallback wraps to the menu's last widget");
        dispatch(egui::Key::ArrowUp, false);
        let (first, second) = dispatch(egui::Key::ArrowDown, true);
        assert!(first.has_focus(), "Source next fallback wraps to the menu's first widget");
        assert!(!second.has_focus());
        dispatch(egui::Key::ArrowDown, false);

        // Ordinary keyboard Tab retains its native access to the separate root.
        pass(&ctx, &mut navigation, "menu", vec![key(egui::Key::Tab)]);
        pass(&ctx, &mut navigation, "menu", Vec::new());
        pass(&ctx, &mut navigation, "menu", vec![key(egui::Key::Tab)]);
        assert_eq!(ctx.memory(|memory| memory.focused()), Some(Id::unique("outside-menu-control")));
    }
}
