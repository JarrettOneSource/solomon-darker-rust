//! Source controller focus over egui's actual registered widgets.
//!
//! c140 `menu/navigation.rs` searches spatially inside the active root, then
//! wraps in registration order. Physical keyboard traversal stays with egui.

use egui::{Context, FocusDirection, LayerId};
use solomon_sim::input::{MenuDirection, choose_spatial_menu_target};

pub(crate) fn begin(ctx: &Context, controller: bool) -> FocusDirection {
    if controller {
        ctx.memory_mut(|memory| memory.take_focus_direction())
    } else {
        FocusDirection::None
    }
}

pub(crate) fn finish(ctx: &Context, direction: FocusDirection, scope: Option<LayerId>) {
    let Some(scope) = scope else { return };
    let (key, shift) = match direction {
        FocusDirection::Up => (egui::Key::ArrowUp, false),
        FocusDirection::Down => (egui::Key::ArrowDown, false),
        FocusDirection::Left => (egui::Key::ArrowLeft, false),
        FocusDirection::Right => (egui::Key::ArrowRight, false),
        FocusDirection::Previous => (egui::Key::Tab, true),
        FocusDirection::Next => (egui::Key::Tab, false),
        FocusDirection::None => return,
    };
    // Sliders and other native widget owners can consume their actual event.
    // Holding egui's traversal must not turn that event into a second action.
    let pending = ctx.input(|input| {
        input.events.iter().any(|event| {
            matches!(event, egui::Event::Key {
                key: actual,
                pressed: true,
                modifiers,
                ..
            } if *actual == key && if shift { modifiers.shift_only() } else { !modifiers.any() })
        })
    });
    if !pending {
        return;
    }
    let widgets = ctx.focusable_widgets_in_layer(scope);
    let focused = ctx.memory(|memory| memory.focused());
    let Some(active) = widgets.iter().position(|widget| Some(widget.id) == focused) else {
        return;
    };
    let spatial = match direction {
        FocusDirection::Up => Some(MenuDirection::Up),
        FocusDirection::Down => Some(MenuDirection::Down),
        FocusDirection::Left => Some(MenuDirection::Left),
        FocusDirection::Right => Some(MenuDirection::Right),
        _ => None,
    };
    let target = spatial.and_then(|direction| {
        let transform = ctx.layer_transform_to_global(scope).unwrap_or_default();
        let centers: Vec<_> = widgets
            .iter()
            .map(|widget| {
                let center = transform * widget.rect.center();
                solomon_sim::Vec2::new(center.x.into(), center.y.into())
            })
            .collect();
        choose_spatial_menu_target(active, &centers, direction)
    });
    let target = target.unwrap_or_else(|| {
        if matches!(direction, FocusDirection::Up | FocusDirection::Left | FocusDirection::Previous)
        {
            (active + widgets.len() - 1) % widgets.len()
        } else {
            (active + 1) % widgets.len()
        }
    });
    if widgets[target].id != widgets[active].id {
        ctx.memory_mut(|memory| memory.request_focus_next_frame(widgets[target].id));
        ctx.request_repaint();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Area, Id, Order, Pos2, Rect, Sense, pos2, vec2};

    fn pass(
        ctx: &Context,
        events: Vec<egui::Event>,
        controller: bool,
        mut paint: impl FnMut(&Context) -> LayerId,
    ) {
        ctx.run_pass(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(400.0, 130.0))),
                events,
                ..Default::default()
            },
            |ctx| {
                let direction = begin(ctx, controller);
                let scope = paint(ctx);
                finish(ctx, direction, Some(scope));
            },
        )
        .drop_without_applying_deltas();
    }

    fn key(key: egui::Key) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }
    }

    #[test]
    fn source_spatial_search_uses_real_offscreen_rects_and_excludes_other_roots() {
        let ctx = Context::default();
        let scope = LayerId::new(Order::Middle, Id::unique("spatial-menu"));
        let mut focus_initial = true;
        let mut paint = |ctx: &Context| {
            Area::new(scope.id)
                .fixed_pos(Pos2::ZERO)
                .fade_in(false)
                .background_interactable(false)
                .show(ctx, |ui| {
                    for (id, position, enabled) in [
                        ("first", pos2(20.0, 20.0), true),
                        ("second", pos2(20.0, 80.0), true),
                        ("offscreen", pos2(40.0, 180.0), true),
                        ("disabled", pos2(80.0, 20.0), false),
                    ] {
                        ui.add_enabled_ui(enabled, |ui| {
                            let response = ui.interact(
                                Rect::from_min_size(position, vec2(20.0, 20.0)),
                                Id::unique(id),
                                Sense::click(),
                            );
                            if id == "first" && focus_initial && response.enabled() {
                                response.request_focus();
                                focus_initial = false;
                            }
                        });
                    }
                });
            Area::new(Id::unique("spatial-outside"))
                .order(Order::Foreground)
                .fixed_pos(pos2(260.0, 20.0))
                .fade_in(false)
                .background_interactable(false)
                .show(ctx, |ui| {
                    ui.interact(
                        Rect::from_min_size(pos2(260.0, 20.0), vec2(20.0, 20.0)),
                        Id::unique("outside"),
                        Sense::click(),
                    );
                });
            scope
        };
        pass(&ctx, Vec::new(), false, &mut paint);
        pass(&ctx, Vec::new(), false, &mut paint);
        assert_eq!(ctx.memory(|memory| memory.focused()), Some(Id::unique("first")));
        pass(&ctx, vec![key(egui::Key::ArrowRight)], true, &mut paint);
        pass(&ctx, Vec::new(), false, &mut paint);
        pass(&ctx, Vec::new(), false, &mut paint);
        assert_eq!(
            ctx.memory(|memory| memory.focused()),
            Some(Id::unique("offscreen")),
            "Source selects the real positive-primary target beyond egui's cone; disabled and foreign targets are excluded"
        );
    }

    #[test]
    fn focused_native_filters_and_consumed_keys_keep_their_event_ownership() {
        for filtered in [false, true] {
            let ctx = Context::default();
            let scope = LayerId::new(Order::Middle, Id::unique("owned-arrow-menu"));
            let mut focus_initial = true;
            let mut paint = |ctx: &Context| {
                Area::new(scope.id)
                    .fixed_pos(Pos2::ZERO)
                    .fade_in(false)
                    .background_interactable(false)
                    .show(ctx, |ui| {
                        for (id, x) in [("owner", 20.0), ("other", 100.0)] {
                            let response = ui.interact(
                                Rect::from_min_size(pos2(x, 20.0), vec2(40.0, 40.0)),
                                Id::unique(id),
                                Sense::click(),
                            );
                            if id == "owner" {
                                if focus_initial && response.enabled() {
                                    response.request_focus();
                                    focus_initial = false;
                                }
                                if filtered {
                                    ui.memory_mut(|memory| {
                                        memory.set_focus_lock_filter(
                                            response.id,
                                            egui::EventFilter {
                                                horizontal_arrows: true,
                                                ..Default::default()
                                            },
                                        );
                                    });
                                } else if response.has_focus() {
                                    ui.input_mut(|input| {
                                        input.consume_key(
                                            egui::Modifiers::NONE,
                                            egui::Key::ArrowRight,
                                        )
                                    });
                                }
                            }
                        }
                    });
                scope
            };
            pass(&ctx, Vec::new(), false, &mut paint);
            pass(&ctx, Vec::new(), false, &mut paint);
            pass(&ctx, Vec::new(), false, &mut paint);
            pass(&ctx, vec![key(egui::Key::ArrowRight)], true, &mut paint);
            pass(&ctx, Vec::new(), false, &mut paint);
            pass(&ctx, Vec::new(), false, &mut paint);
            assert_eq!(ctx.memory(|memory| memory.focused()), Some(Id::unique("owner")));
        }
    }
}
