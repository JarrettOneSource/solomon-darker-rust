//! Source controller focus over egui's actual registered widgets.
//!
//! c140 `menu/navigation.rs` searches spatially inside the active root, then
//! wraps in registration order. Physical keyboard traversal stays with egui.

use egui::{Context, FocusDirection, Id, LayerId};
use solomon_sim::input::{MenuDirection, choose_spatial_menu_target};

#[derive(Clone, Copy)]
pub(crate) struct ControllerInput {
    direction: FocusDirection,
    confirm: bool,
    focused: Option<Id>,
}

pub(crate) fn begin(ctx: &Context, controller: bool) -> ControllerInput {
    let mut input =
        ControllerInput { direction: FocusDirection::None, confirm: false, focused: None };
    if controller {
        input.focused = ctx.memory(|memory| memory.focused());
        input.direction = ctx.memory_mut(|memory| memory.take_focus_direction());
        input.confirm =
            ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
        if input.confirm {
            let pass = ctx.cumulative_pass_nr();
            ctx.data_mut(|data| data.insert_temp(Id::unique("source-controller-confirm"), pass));
        }
    }
    input
}

pub(crate) fn confirmation_pending(ctx: &Context) -> bool {
    ctx.data(|data| data.get_temp::<u64>(Id::unique("source-controller-confirm")))
        == Some(ctx.cumulative_pass_nr())
}

#[derive(Clone, Copy)]
struct PreferredFocus {
    pass: u64,
    widget: Option<Id>,
}

/// Existing navigation owners supply their Source default without moving focus.
/// Only the final highest scope can use it, and only during this actual paint.
pub(crate) fn preferred(ctx: &Context, scope: &str, widget: Option<Id>) {
    let key = Id::unique("source-controller-preferred").with(scope);
    let preferred = PreferredFocus { pass: ctx.cumulative_pass_nr(), widget };
    ctx.data_mut(|data| data.insert_temp(key, preferred));
}

pub(crate) fn finish(ctx: &Context, input: ControllerInput, scope: Option<(LayerId, &str)>) {
    if !input.confirm && input.direction == FocusDirection::None {
        return;
    }
    let Some((layer, scope)) = scope else { return };
    let widgets = ctx.focusable_widgets_in_layer(layer);
    // Discovery can request a new default while widgets paint. Source movement
    // still uses the active element captured before confirm/default admission.
    let active = widgets.iter().position(|widget| Some(widget.id) == input.focused);
    let initial = active.map(|index| widgets[index].id).or_else(|| {
        let key = Id::unique("source-controller-preferred").with(scope);
        let preferred = ctx.data(|data| data.get_temp::<PreferredFocus>(key));
        let preferred = preferred.filter(|preferred| preferred.pass == ctx.cumulative_pass_nr());
        if let Some(preferred) = preferred {
            // A declared but unavailable default owns initial admission until it appears.
            widgets.iter().find(|widget| Some(widget.id) == preferred.widget)
        } else {
            widgets.first()
        }
        .map(|widget| widget.id)
    });
    if input.confirm
        && let Some(initial) = initial
    {
        // Source A clicks the pre-move target. The existing empty commit delivers
        // its ordinary response, independently of focus and without replaying Enter.
        if ctx.request_activation_next_frame(initial) {
            ctx.request_repaint();
        }
    }
    let direction = input.direction;
    let pending = match direction {
        FocusDirection::Up => Some((egui::Key::ArrowUp, false)),
        FocusDirection::Down => Some((egui::Key::ArrowDown, false)),
        FocusDirection::Left => Some((egui::Key::ArrowLeft, false)),
        FocusDirection::Right => Some((egui::Key::ArrowRight, false)),
        FocusDirection::Previous => Some((egui::Key::Tab, true)),
        FocusDirection::Next => Some((egui::Key::Tab, false)),
        FocusDirection::None => None,
    }
    .is_some_and(|(key, shift)| {
        // Native widget owners can consume their actual arrow or bumper event.
        ctx.input(|input| {
            input.events.iter().any(|event| {
                matches!(event, egui::Event::Key {
                    key: actual,
                    pressed: true,
                    modifiers,
                    ..
                } if *actual == key && if shift { modifiers.shift_only() } else { !modifiers.any() })
            })
        })
    });
    let movement = if !pending {
        None
    } else if let Some(active) = active {
        let spatial = match direction {
            FocusDirection::Up => Some(MenuDirection::Up),
            FocusDirection::Down => Some(MenuDirection::Down),
            FocusDirection::Left => Some(MenuDirection::Left),
            FocusDirection::Right => Some(MenuDirection::Right),
            _ => None,
        };
        let target = spatial.and_then(|direction| {
            let transform = ctx.layer_transform_to_global(layer).unwrap_or_default();
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
            if matches!(
                direction,
                FocusDirection::Up | FocusDirection::Left | FocusDirection::Previous
            ) {
                (active + widgets.len() - 1) % widgets.len()
            } else {
                (active + 1) % widgets.len()
            }
        });
        Some(widgets[target].id)
    } else {
        initial
    };
    if let Some(target) = movement.or(if input.confirm { initial } else { None })
        && ctx.memory(|memory| memory.focused()) != Some(target)
    {
        ctx.memory_mut(|memory| memory.request_focus_next_frame(target));
        ctx.request_repaint();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Area, Order, Pos2, Rect, Sense, pos2, vec2};

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
                finish(ctx, direction, Some((scope, "test")));
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

    #[test]
    fn reentry_uses_its_active_scope_preference_and_keeps_hidden_defaults_pending() {
        for default in [Some(Id::unique("second")), Some(Id::unique("hidden")), None] {
            let ctx = Context::default();
            let scope = LayerId::new(Order::Middle, Id::unique("preferred-menu"));
            let mut focus_outside = true;
            let mut paint = |ctx: &Context| {
                preferred(ctx, "test", default);
                Area::new(scope.id)
                    .fixed_pos(Pos2::ZERO)
                    .fade_in(false)
                    .background_interactable(false)
                    .show(ctx, |ui| {
                        for (id, x) in [("first", 20.0), ("second", 100.0)] {
                            ui.interact(
                                Rect::from_min_size(pos2(x, 20.0), vec2(40.0, 40.0)),
                                Id::unique(id),
                                Sense::click(),
                            );
                        }
                    });
                Area::new(Id::unique("preferred-outside"))
                    .order(Order::Foreground)
                    .fixed_pos(pos2(260.0, 20.0))
                    .fade_in(false)
                    .background_interactable(false)
                    .show(ctx, |ui| {
                        let response = ui.interact(
                            Rect::from_min_size(pos2(260.0, 20.0), vec2(40.0, 40.0)),
                            Id::unique("outside"),
                            Sense::click(),
                        );
                        if focus_outside && response.enabled() {
                            response.request_focus();
                            focus_outside = false;
                        }
                    });
                preferred(ctx, "other-scope", Some(Id::unique("first")));
                scope
            };
            pass(&ctx, Vec::new(), false, &mut paint);
            pass(&ctx, Vec::new(), false, &mut paint);
            assert_eq!(ctx.memory(|memory| memory.focused()), Some(Id::unique("outside")));
            pass(&ctx, vec![key(egui::Key::ArrowRight)], true, &mut paint);
            pass(&ctx, Vec::new(), false, &mut paint);
            pass(&ctx, Vec::new(), false, &mut paint);
            let expected = if default == Some(Id::unique("second")) { "second" } else { "outside" };
            assert_eq!(ctx.memory(|memory| memory.focused()), Some(Id::unique(expected)));
        }
    }

    #[test]
    fn controller_a_activates_the_real_pre_move_target_without_a_foreign_enter() {
        for outside in [false, true] {
            let ctx = Context::default();
            let scope = LayerId::new(Order::Middle, Id::unique("confirm-menu"));
            let mut focus_initial = true;
            let mut clicked = Vec::new();
            let mut paint = |ctx: &Context| {
                assert!(!ctx.input(|input| input.key_pressed(egui::Key::Enter)));
                let initial = if outside { "outside" } else { "first" };
                preferred(ctx, "test", Some(Id::unique("second")));
                for (layer, controls) in [
                    (scope, vec![("first", 20.0), ("second", 80.0)]),
                    (
                        LayerId::new(Order::Foreground, Id::unique("confirm-outside")),
                        vec![("outside", 20.0)],
                    ),
                ] {
                    Area::new(layer.id)
                        .order(layer.order)
                        .fixed_pos(Pos2::ZERO)
                        .fade_in(false)
                        .background_interactable(false)
                        .show(ctx, |ui| {
                            for (id, y) in &controls {
                                let response = ui.interact(
                                    Rect::from_min_size(pos2(20.0, *y), vec2(40.0, 40.0)),
                                    Id::unique(id),
                                    Sense::click(),
                                );
                                if *id == initial && focus_initial && response.enabled() {
                                    response.request_focus();
                                    focus_initial = false;
                                }
                                if response.clicked() {
                                    clicked.push((*id).to_owned());
                                }
                            }
                        });
                }
                scope
            };
            pass(&ctx, Vec::new(), false, &mut paint);
            pass(&ctx, Vec::new(), false, &mut paint);
            let mut events = vec![key(egui::Key::Enter)];
            if !outside {
                events.push(key(egui::Key::ArrowDown));
            }
            pass(&ctx, events, true, &mut paint);
            pass(&ctx, Vec::new(), false, &mut paint);
            pass(&ctx, Vec::new(), false, &mut paint);
            assert_eq!(ctx.memory(|memory| memory.focused()), Some(Id::unique("second")));
            assert_eq!(clicked, [if outside { "second" } else { "first" }]);
        }
    }

    #[test]
    fn queued_activation_requires_the_same_actual_eligible_registration_and_expires() {
        use std::cell::Cell;

        for unavailable in 1..=3 {
            let ctx = Context::default();
            let scope = LayerId::new(Order::Middle, Id::unique("activation-menu"));
            let mode = Cell::new(0);
            let mut focus_initial = true;
            let mut clicked = 0;
            let mut paint = |ctx: &Context| {
                assert!(
                    ctx.read_response(Id::unique("target"))
                        .is_none_or(|response| !response.clicked())
                );
                assert!(
                    !ctx.request_activation_next_frame(Id::unique("target")),
                    "a previous-pass response is not a current widget"
                );
                preferred(ctx, "test", Some(Id::unique("target")));
                let layer = if mode.get() == 3 {
                    LayerId::new(Order::Foreground, Id::unique("activation-foreign"))
                } else {
                    scope
                };
                Area::new(layer.id)
                    .order(layer.order)
                    .fixed_pos(Pos2::ZERO)
                    .fade_in(false)
                    .background_interactable(false)
                    .show(ctx, |ui| {
                        if mode.get() != 2 {
                            ui.add_enabled_ui(mode.get() != 1, |ui| {
                                let response = ui.interact(
                                    Rect::from_min_size(pos2(20.0, 20.0), vec2(40.0, 40.0)),
                                    Id::unique("target"),
                                    Sense::click(),
                                );
                                if focus_initial && response.enabled() {
                                    response.request_focus();
                                    focus_initial = false;
                                }
                                clicked += usize::from(response.clicked());
                            });
                        }
                    });
                scope
            };
            pass(&ctx, Vec::new(), false, &mut paint);
            pass(&ctx, Vec::new(), false, &mut paint);
            pass(&ctx, vec![key(egui::Key::Enter)], true, &mut paint);
            mode.set(unavailable);
            pass(&ctx, Vec::new(), false, &mut paint);
            mode.set(0);
            pass(&ctx, Vec::new(), false, &mut paint);
            assert_eq!(
                clicked, 0,
                "activation cannot survive disabled, removed or foreign-layer delivery"
            );
        }
    }

    #[test]
    fn default_discovery_does_not_change_the_original_source_active_element() {
        let ctx = Context::default();
        let scope = LayerId::new(Order::Middle, Id::unique("discovery-menu"));
        let mut clicked = Vec::new();
        let mut paint = |ctx: &Context| {
            let discovering = confirmation_pending(ctx);
            preferred(ctx, "test", discovering.then(|| Id::unique("second")));
            Area::new(scope.id)
                .fixed_pos(Pos2::ZERO)
                .fade_in(false)
                .background_interactable(false)
                .show(ctx, |ui| {
                    for (id, y) in [("first", 20.0), ("second", 80.0)] {
                        let response = ui.interact(
                            Rect::from_min_size(pos2(20.0, y), vec2(40.0, 40.0)),
                            Id::unique(id),
                            Sense::click(),
                        );
                        if discovering && id == "second" && response.enabled() {
                            response.request_focus();
                        }
                        if response.clicked() {
                            clicked.push(id);
                        }
                    }
                });
            scope
        };
        pass(&ctx, Vec::new(), false, &mut paint);
        pass(&ctx, Vec::new(), false, &mut paint);
        assert_eq!(ctx.memory(|memory| memory.focused()), None);
        pass(&ctx, vec![key(egui::Key::Enter), key(egui::Key::ArrowDown)], true, &mut paint);
        pass(&ctx, Vec::new(), false, &mut paint);
        pass(&ctx, Vec::new(), false, &mut paint);
        assert_eq!(ctx.memory(|memory| memory.focused()), Some(Id::unique("second")));
        assert_eq!(
            clicked,
            ["second"],
            "initial A plus direction only enters the preferred control"
        );
    }

    #[test]
    fn a_single_widget_winning_scope_recovers_focus_from_a_lower_default_write() {
        let ctx = Context::default();
        let scope = LayerId::new(Order::Foreground, Id::unique("single-winning-menu"));
        let lower = LayerId::new(Order::Middle, Id::unique("lower-menu"));
        let mut focus_initial = true;
        let mut paint = |ctx: &Context| {
            for (layer, id) in [(scope, "winner"), (lower, "lower")] {
                Area::new(layer.id)
                    .order(layer.order)
                    .fixed_pos(Pos2::ZERO)
                    .fade_in(false)
                    .background_interactable(false)
                    .show(ctx, |ui| {
                        let response = ui.interact(
                            Rect::from_min_size(pos2(20.0, 20.0), vec2(40.0, 40.0)),
                            Id::unique(id),
                            Sense::click(),
                        );
                        if id == "winner" && focus_initial && response.enabled() {
                            response.request_focus();
                            focus_initial = false;
                        } else if id == "lower"
                            && ctx.input(|input| input.key_pressed(egui::Key::ArrowUp))
                        {
                            response.request_focus();
                        }
                    });
            }
            scope
        };
        pass(&ctx, Vec::new(), false, &mut paint);
        pass(&ctx, Vec::new(), false, &mut paint);
        pass(&ctx, vec![key(egui::Key::ArrowUp)], true, &mut paint);
        pass(&ctx, Vec::new(), false, &mut paint);
        pass(&ctx, Vec::new(), false, &mut paint);
        assert_eq!(ctx.memory(|memory| memory.focused()), Some(Id::unique("winner")));
    }
}
