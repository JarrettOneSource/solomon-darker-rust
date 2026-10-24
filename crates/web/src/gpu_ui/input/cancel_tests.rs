//! Real Context responses from the production native pointer projection.

use super::*;
use egui::{Context, Id, Sense};

#[derive(Default)]
struct Responses {
    clicked: bool,
    double_clicked: bool,
    dragging: bool,
    drag_stopped: bool,
}

fn pass(ctx: &Context, events: Vec<UiEvent>) -> Responses {
    let mut response = Responses::default();
    ctx.run_pass(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(400.0, 300.0))),
            events,
            ..Default::default()
        },
        |ctx| {
            egui::Area::new(Id::unique("cancel-controls"))
                .fixed_pos(Pos2::ZERO)
                .fade_in(false)
                .show(ctx, |ui| {
                    ui.set_min_size(vec2(400.0, 300.0));
                    let button = ui.interact(
                        Rect::from_min_size(pos2(10.0, 10.0), vec2(80.0, 40.0)),
                        Id::unique("cancel-button"),
                        Sense::click(),
                    );
                    response.clicked = button.clicked();
                    response.double_clicked = button.double_clicked();
                    let drag = ui.interact(
                        Rect::from_min_size(pos2(10.0, 100.0), vec2(120.0, 40.0)),
                        Id::unique("cancel-drag"),
                        Sense::drag(),
                    );
                    response.dragging = drag.dragged();
                    response.drag_stopped = drag.drag_stopped();
                });
        },
    )
    .drop_without_applying_deltas();
    response
}

fn native(ctx: &Context, phase: TouchPhase, pos: Pos2) -> Responses {
    let mut queue = InputQueue { primary_touch: Some(1), ..Default::default() };
    queue.events.push(UiEvent::Touch {
        device_id: egui::TouchDeviceId(0),
        id: egui::TouchId(1),
        phase,
        pos,
        force: Some(0.5),
    });
    queue.events.push(UiEvent::PointerMoved(pos));
    if !matches!(phase, TouchPhase::Move) {
        queue.finish_pointer(phase, pos, egui::PointerButton::Primary, Modifiers::NONE, true);
    }
    if matches!(phase, TouchPhase::End | TouchPhase::Cancel) {
        assert!(queue.primary_touch.is_none());
    }
    if matches!(phase, TouchPhase::Cancel) {
        assert!(
            queue
                .events
                .iter()
                .any(|event| matches!(event, UiEvent::Touch { phase: TouchPhase::Cancel, .. })),
            "the actual raw touch cancellation remains in the transport"
        );
        assert!(
            !queue
                .events
                .iter()
                .any(|event| matches!(event, UiEvent::PointerButton { pressed: false, .. })),
            "cancellation is never a synthetic normal release"
        );
    }
    pass(ctx, queue.events)
}

fn controls() -> Context {
    let ctx = Context::default();
    pass(&ctx, Vec::new());
    pass(&ctx, Vec::new());
    ctx
}

#[test]
fn cancelled_native_touch_never_activates_a_real_button() {
    let ctx = controls();
    let point = pos2(30.0, 30.0);
    assert!(!native(&ctx, TouchPhase::Start, point).clicked);
    assert!(
        !native(&ctx, TouchPhase::Cancel, point).clicked,
        "pointercancel must not become a normal click release"
    );
    assert!(!ctx.input(|input| input.pointer.primary_down()));
    assert!(!ctx.input(|input| input.pointer.primary_released()));
    assert!(ctx.input(|input| input.pointer.interact_pos()).is_none());
    assert!(ctx.dragged_id().is_none());
    native(&ctx, TouchPhase::Start, point);
    assert!(
        native(&ctx, TouchPhase::End, point).clicked,
        "the next ordinary native tap still activates once"
    );
    native(&ctx, TouchPhase::Start, point);
    assert!(
        native(&ctx, TouchPhase::End, point).double_clicked,
        "ordinary consecutive native taps retain the original double-click behavior"
    );
}

#[test]
fn cancelled_native_drag_retires_its_real_context_owner() {
    let ctx = controls();
    native(&ctx, TouchPhase::Start, pos2(30.0, 115.0));
    for x in [45.0, 60.0, 75.0] {
        assert!(native(&ctx, TouchPhase::Move, pos2(x, 115.0)).dragging);
    }
    assert!(
        ctx.input(|input| input.pointer.velocity().length()) > 0.0,
        "the actual drag has motion to retire, rather than an already-zero velocity"
    );
    assert_eq!(ctx.dragged_id(), Some(Id::unique("cancel-drag")));
    let ended = native(&ctx, TouchPhase::Cancel, pos2(75.0, 115.0));
    assert!(!ended.clicked && !ended.dragging);
    assert!(ended.drag_stopped);
    assert!(!ctx.input(|input| input.pointer.primary_down()));
    assert!(ctx.dragged_id().is_none());
    assert_eq!(ctx.input(|input| input.pointer.velocity()), egui::Vec2::ZERO);
}

#[test]
fn native_mouse_cancel_clears_all_buttons_without_a_release_or_click() {
    let ctx = controls();
    let pos = pos2(30.0, 30.0);
    let mut queue = InputQueue::default();
    for button in [egui::PointerButton::Primary, egui::PointerButton::Secondary] {
        queue.finish_pointer(TouchPhase::Start, pos, button, Modifiers::NONE, false);
    }
    pass(&ctx, queue.events);
    assert!(ctx.input(|input| input.pointer.primary_down() && input.pointer.secondary_down()));
    let mut queue = InputQueue::default();
    queue.finish_pointer(
        TouchPhase::Cancel,
        pos,
        egui::PointerButton::Primary,
        Modifiers::NONE,
        false,
    );
    assert!(!pass(&ctx, queue.events).clicked);
    assert!(!ctx.input(|input| input.pointer.any_down() || input.pointer.any_released()));
}

#[test]
fn secondary_raw_touch_cancellation_does_not_cancel_the_primary_pointer() {
    let ctx = controls();
    native(&ctx, TouchPhase::Start, pos2(30.0, 115.0));
    for phase in [TouchPhase::Start, TouchPhase::Cancel] {
        pass(
            &ctx,
            vec![UiEvent::Touch {
                device_id: egui::TouchDeviceId(0),
                id: egui::TouchId(2),
                phase,
                pos: pos2(100.0, 115.0),
                force: Some(0.5),
            }],
        );
    }
    assert!(ctx.input(|input| input.pointer.primary_down()));
    assert_eq!(ctx.dragged_id(), Some(Id::unique("cancel-drag")));
    native(&ctx, TouchPhase::End, pos2(30.0, 115.0));
    assert!(!ctx.input(|input| input.pointer.any_down()));
}
