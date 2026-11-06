//! Real egui layout/input/drain contracts for the Source composition policy.

use egui::{
    Area, Color32, Context, Event, Id, LayerId, Modifiers, Order, Pos2, RawInput, Rect, Sense,
    pos2, vec2,
};

fn pointer(down: bool, pos: Pos2) -> Vec<Event> {
    vec![
        Event::PointerMoved(pos),
        Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: down,
            modifiers: Modifiers::NONE,
        },
    ]
}

fn layers() -> [LayerId; 5] {
    [
        LayerId::new(Order::Middle, Id::unique("source-hud")),
        LayerId::new(Order::Foreground, Id::unique("source-book")),
        LayerId::new(Order::Foreground, Id::unique("source-menu")),
        LayerId::new(Order::Middle, Id::unique("source-social")),
        LayerId::new(Order::Foreground, Id::unique("source-observer")),
    ]
}

fn ordered_pass(
    ctx: &Context,
    visible: &[usize],
    modal: &[usize],
    events: Vec<Event>,
) -> (Vec<usize>, Vec<usize>) {
    let ids = layers();
    let mut clicked = Vec::new();
    let output = ctx.run_pass(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(400.0, 300.0))),
            events,
            ..Default::default()
        },
        |ctx| {
            for &index in visible {
                let layer = ids[index];
                Area::new(layer.id).order(layer.order).fixed_pos(Pos2::ZERO).fade_in(false).show(
                    ctx,
                    |ui| {
                        ui.set_min_size(vec2(400.0, 300.0));
                        if modal.contains(&index) {
                            ui.memory_mut(|memory| memory.set_modal_layer(layer));
                        }
                        ui.painter().rect_filled(
                            Rect::from_min_size(Pos2::ZERO, vec2(400.0, 300.0)),
                            0.0,
                            Color32::from_rgb(index as u8 + 10, 0, 0),
                        );
                        let response = ui.interact(
                            Rect::from_min_size(pos2(10.0, 10.0), vec2(80.0, 60.0)),
                            layer.id.with("control"),
                            Sense::click(),
                        );
                        if response.clicked() {
                            clicked.push(index);
                        }
                    },
                );
            }
        },
    );
    let painted = output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Rect(rect)
                if rect.fill.g() == 0 && rect.fill.b() == 0 && rect.fill.r() >= 10 =>
            {
                Some(usize::from(rect.fill.r() - 10))
            }
            _ => None,
        })
        .collect();
    output.drop_without_applying_deltas();
    (clicked, painted)
}

#[test]
fn fixed_source_order_agrees_for_real_paint_pointer_modal_and_reappearance() {
    let ctx = Context::default();
    let ids = layers();
    ctx.set_fixed_layer_order(&ids);
    assert!(ctx.compare_layer_order(ids[2], ids[3]).is_lt());
    assert!(
        ctx.compare_layer_order(LayerId::new(Order::Middle, Id::unique("unregistered")), ids[0])
            .is_lt()
    );
    assert!(
        ctx.compare_layer_order(ids[4], LayerId::new(Order::Foreground, Id::unique("popup")))
            .is_lt()
    );
    let scrambled = [3, 2, 0];
    ordered_pass(&ctx, &scrambled, &[], Vec::new());
    let (_, painted) = ordered_pass(&ctx, &scrambled, &[], Vec::new());
    assert_eq!(painted, [0, 2, 3], "Source Social Middle60 paints above menu Foreground50");
    assert!(
        ctx.memory(|memory| memory.area_rect(ids[4].id)).is_none(),
        "registering a hidden layer never creates an Area"
    );
    assert_eq!(ctx.layer_id_at(pos2(30.0, 30.0)), Some(ids[3]));
    assert_eq!(ctx.pointer_widget_at(pos2(30.0, 30.0)), Some(ids[3].id.with("control")));
    ordered_pass(&ctx, &scrambled, &[], pointer(true, pos2(30.0, 30.0)));
    let (clicked, _) = ordered_pass(&ctx, &scrambled, &[], pointer(false, pos2(30.0, 30.0)));
    assert_eq!(clicked, [3]);
    ordered_pass(&ctx, &[3, 2], &[], Vec::new());
    ordered_pass(&ctx, &[3, 2], &[], Vec::new());
    ctx.move_to_top(ids[2]);
    ordered_pass(&ctx, &scrambled, &[2], Vec::new());
    let (_, painted) = ordered_pass(&ctx, &scrambled, &[2], Vec::new());
    assert_eq!(painted, [0, 2, 3]);
    assert!(!ctx.memory(|memory| memory.allows_interaction(ids[0])));
    assert!(ctx.memory(|memory| memory.allows_interaction(ids[3])));
    ordered_pass(&ctx, &[3, 2, 0], &[3, 2], Vec::new());
    assert_eq!(
        ctx.memory(|memory| memory.top_modal_layer()),
        Some(ids[3]),
        "highest Source modal wins regardless of callback order"
    );
    let (_, painted) = ordered_pass(&ctx, &[4, 3, 2, 0], &[], Vec::new());
    assert_eq!(painted, [0, 2, 3], "a newly created Area retains its native sizing pass");
    let (_, painted) = ordered_pass(&ctx, &[4, 3, 2, 0], &[], Vec::new());
    assert_eq!(painted, [0, 2, 3, 4]);
}

#[test]
fn unregistered_context_retains_default_order_and_fixed_policy_can_be_removed() {
    let ctx = Context::default();
    let ids = layers();
    ordered_pass(&ctx, &[2, 3], &[], Vec::new());
    let (_, painted) = ordered_pass(&ctx, &[2, 3], &[], Vec::new());
    assert_eq!(painted, [3, 2]);
    ctx.set_fixed_layer_order(&ids);
    let (_, painted) = ordered_pass(&ctx, &[2, 3], &[], Vec::new());
    assert_eq!(painted, [2, 3]);
    ctx.set_fixed_layer_order(&[]);
    let (_, painted) = ordered_pass(&ctx, &[2, 3], &[], Vec::new());
    assert_eq!(painted, [3, 2]);
    assert_eq!(ctx.layer_id_at(pos2(30.0, 30.0)), Some(ids[2]));
}

fn passive_pass(ctx: &Context, events: Vec<Event>, passive: bool) -> Vec<&'static str> {
    let [_, book, _, map, _] = layers();
    let mut clicked = Vec::new();
    ctx.run_pass(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(400.0, 300.0))),
            events,
            ..Default::default()
        },
        |ctx| {
            for (layer, upper) in [(book, false), (map, true)] {
                Area::new(layer.id)
                    .order(layer.order)
                    .fixed_pos(Pos2::ZERO)
                    .fade_in(false)
                    .background_interactable(!upper || !passive)
                    .show(ctx, |ui| {
                        ui.set_min_size(vec2(400.0, 300.0));
                        if upper {
                            ui.add(
                                egui::Label::new(
                                    "Passive text keeps its real font/layout rectangle",
                                )
                                .selectable(false),
                            );
                            let body = ui.interact(
                                Rect::from_center_size(pos2(100.0, 100.0), vec2(100.0, 100.0)),
                                Id::unique("map-circle"),
                                Sense::click(),
                            );
                            ctx.set_widget_hit_test(
                                body.id,
                                std::sync::Arc::new(|pos| pos.distance(pos2(100.0, 100.0)) <= 50.0),
                            );
                            if body.clicked() {
                                clicked.push("map");
                            }
                            ui.add_enabled_ui(false, |ui| {
                                let _ = ui.interact(
                                    Rect::from_min_size(pos2(250.0, 90.0), vec2(70.0, 40.0)),
                                    Id::unique("disabled-map-action"),
                                    Sense::click(),
                                );
                            });
                        } else {
                            let response = ui.interact(
                                Rect::from_min_size(Pos2::ZERO, vec2(350.0, 220.0)),
                                Id::unique("equipment"),
                                Sense::click(),
                            );
                            if response.clicked() {
                                clicked.push("equipment");
                            }
                        }
                    });
            }
        },
    )
    .drop_without_applying_deltas();
    clicked
}

#[test]
fn passive_container_keeps_children_and_shapes_without_stealing_lower_equipment() {
    let ctx = Context::default();
    let ids = layers();
    ctx.set_fixed_layer_order(&ids);
    passive_pass(&ctx, Vec::new(), true);
    passive_pass(&ctx, Vec::new(), true);
    assert_eq!(
        ctx.memory(|memory| memory.area_rect(ids[3].id)).unwrap().size(),
        vec2(400.0, 300.0),
        "pass-through does not fake zero layout geometry"
    );
    for (point, expected, owner) in [
        (pos2(20.0, 8.0), "equipment", ids[1]),
        (pos2(55.0, 55.0), "equipment", ids[1]),
        (pos2(100.0, 100.0), "map", ids[3]),
    ] {
        assert_eq!(ctx.layer_id_at(point), Some(owner), "actual clipped owner at {point:?}");
        passive_pass(&ctx, pointer(true, point), true);
        assert_eq!(passive_pass(&ctx, pointer(false, point), true), [expected]);
    }
    let disabled = pos2(270.0, 110.0);
    assert_eq!(ctx.layer_id_at(disabled), Some(ids[3]));
    assert_eq!(ctx.pointer_widget_at(disabled), None);
    passive_pass(&ctx, pointer(true, disabled), true);
    assert!(passive_pass(&ctx, pointer(false, disabled), true).is_empty());
    passive_pass(&ctx, Vec::new(), false);
    passive_pass(&ctx, Vec::new(), false);
    passive_pass(&ctx, pointer(true, pos2(20.0, 20.0)), false);
    assert!(
        passive_pass(&ctx, pointer(false, pos2(20.0, 20.0)), false).is_empty(),
        "ordinary opaque Area still blocks its whole background"
    );
}
