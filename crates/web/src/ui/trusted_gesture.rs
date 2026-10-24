//! Synchronous browser capabilities use the same real widget owner as native input.

use egui::{Pos2, Response};

pub(crate) fn owns_point(response: &Response, point: Pos2) -> bool {
    let transform = response.ctx.layer_transform_to_global(response.layer_id).unwrap_or_default();
    response.enabled()
        && (transform * response.interact_rect).contains(point)
        && response.ctx.pointer_widget_at(point) == Some(response.id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{
        Color32, Context, Event, Id, LayerId, Modifiers, Order, PointerButton, RawInput, Rect,
        Sense, pos2, vec2,
    };

    #[derive(Clone, Copy, PartialEq)]
    enum Cover {
        None,
        Opaque,
        Modal,
        Disabled,
    }
    fn layers() -> [LayerId; 2] {
        [
            LayerId::new(Order::Foreground, Id::unique("trusted-capability-control")),
            LayerId::new(Order::Middle, Id::unique("trusted-capability-cover")),
        ]
    }
    fn layout(
        ctx: &Context,
        cached: &mut Option<Response>,
        replace: bool,
        cover: Cover,
        events: Vec<Event>,
    ) -> usize {
        let layers = layers();
        ctx.set_fixed_layer_order(&layers);
        let mut clicks = 0;
        let now = ctx.input(|input| input.time) + 1.0 / 60.0;
        ctx.run_pass(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(400.0, 300.0))),
                time: Some(now),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::Area::new(layers[0].id)
                    .order(layers[0].order)
                    .fade_in(false)
                    .fixed_pos(Pos2::ZERO)
                    .show(ctx, |ui| {
                        ui.set_min_size(vec2(160.0, 100.0));
                        let rect = Rect::from_min_size(pos2(20.0, 20.0), vec2(80.0, 40.0));
                        ui.painter().rect_filled(rect, 0.0, Color32::WHITE);
                        let response =
                            ui.interact(rect, Id::unique("real-capability-button"), Sense::click());
                        clicks += usize::from(response.clicked());
                        if replace {
                            *cached = Some(response);
                        }
                    });
                if cover != Cover::None {
                    egui::Area::new(layers[1].id)
                        .background_interactable(false)
                        .order(layers[1].order)
                        .fade_in(false)
                        .fixed_pos(Pos2::ZERO)
                        .show(ctx, |ui| {
                            let rect = if cover == Cover::Modal {
                                Rect::from_min_size(pos2(200.0, 160.0), vec2(80.0, 40.0))
                            } else {
                                Rect::from_min_size(Pos2::ZERO, vec2(160.0, 100.0))
                            };
                            ui.set_min_size(rect.max.to_vec2());
                            if cover == Cover::Modal {
                                ui.memory_mut(|memory| memory.set_modal_layer(ui.layer_id()));
                            }
                            ui.painter().rect_filled(rect, 0.0, Color32::BLACK);
                            ui.add_enabled_ui(cover != Cover::Disabled, |ui| {
                                ui.interact(rect, Id::unique("real-cover-control"), Sense::click());
                            });
                        });
                }
            },
        )
        .drop_without_applying_deltas();
        clicks
    }
    #[test]
    fn genuine_pointer_activation_remains_admitted_without_a_cover() {
        let ctx = Context::default();
        let mut target = None;
        for _ in 0..3 {
            layout(&ctx, &mut target, true, Cover::None, Vec::new());
        }
        let point = pos2(60.0, 40.0);
        assert!(owns_point(target.as_ref().unwrap(), point));
        let event = |pressed| {
            vec![
                Event::PointerMoved(point),
                Event::PointerButton {
                    pos: point,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                },
            ]
        };
        assert_eq!(layout(&ctx, &mut target, true, Cover::None, event(true)), 0);
        assert_eq!(layout(&ctx, &mut target, true, Cover::None, event(false)), 1);
        assert!(owns_point(target.as_ref().unwrap(), point));
    }
    #[test]
    fn stale_bounds_cannot_admit_a_covered_disabled_or_modal_capability() {
        for cover in [Cover::Opaque, Cover::Modal, Cover::Disabled] {
            let ctx = Context::default();
            let mut target = None;
            for _ in 0..3 {
                layout(&ctx, &mut target, true, Cover::None, Vec::new());
            }
            let point = pos2(60.0, 40.0);
            assert!(owns_point(target.as_ref().unwrap(), point));
            // Keep the genuine formerly exposed Response, just like a native click adapter.
            for _ in 0..3 {
                layout(&ctx, &mut target, false, cover, Vec::new());
            }
            let cached = target.as_ref().unwrap();
            assert!(
                cached.enabled() && cached.interact_rect.contains(point),
                "the previous bounds-only adapter would admit this click"
            );
            assert!(!owns_point(cached, point));
        }
    }
    #[test]
    fn actual_clipping_and_layer_transform_define_the_capability_hit() {
        let ctx = Context::default();
        let layer = layers()[0];
        let transform = egui::emath::TSTransform::new(vec2(40.0, 30.0), 2.0);
        ctx.set_fixed_layer_order(&[layer]);
        let mut target = None;
        for _ in 0..3 {
            ctx.set_transform_layer(layer, transform);
            ctx.run_pass(
                RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(400.0, 300.0))),
                    ..Default::default()
                },
                |ctx| {
                    egui::Area::new(layer.id)
                        .order(layer.order)
                        .fade_in(false)
                        .fixed_pos(Pos2::ZERO)
                        .constrain(false)
                        .show(ctx, |ui| {
                            ui.set_min_size(vec2(100.0, 60.0));
                            ui.set_clip_rect(Rect::from_min_size(Pos2::ZERO, vec2(40.0, 50.0)));
                            let rect = Rect::from_min_size(pos2(20.0, 20.0), vec2(80.0, 40.0));
                            ui.painter().rect_filled(rect, 0.0, Color32::WHITE);
                            target = Some(ui.interact(
                                rect,
                                Id::unique("real-clipped-capability"),
                                Sense::click(),
                            ));
                        });
                },
            )
            .drop_without_applying_deltas();
        }
        let target = target.unwrap();
        assert!(owns_point(&target, transform * pos2(30.0, 30.0)));
        assert!(!owns_point(&target, transform * pos2(60.0, 30.0)));
        assert!(!owns_point(&target, transform * pos2(30.0, 55.0)));
        assert!(!owns_point(&target, pos2(-1.0, -1.0)));
    }
}
