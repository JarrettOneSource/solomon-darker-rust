use super::*;

fn frame(
    ctx: &Context,
    events: Vec<egui::Event>,
    paint: impl FnMut(&mut UiFrame<'_>),
) -> BTreeMap<String, WidgetGeometry> {
    let images = painter::Images::default();
    let widgets = RefCell::new(BTreeMap::new());
    let announcements = RefCell::new(Vec::new());
    let previous_widgets = BTreeMap::new();
    let mut paint = paint;
    let raw = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(400.0, 300.0))),
        events,
        ..Default::default()
    };
    ctx.run_pass(raw, |ctx| {
        paint(&mut UiFrame {
            ctx,
            images: &images,
            widgets: &widgets,
            previous_widgets: &previous_widgets,
            key_codes: &[],
            blend: BlendMode::Normal,
            announcements: &announcements,
        })
    })
    .drop_without_applying_deltas();
    widgets.into_inner()
}

fn pointer(pressed: bool, pos: Pos2) -> Vec<egui::Event> {
    vec![
        egui::Event::PointerMoved(pos),
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        },
    ]
}

#[test]
fn actual_pointer_response_activates_once_and_reports_the_same_visible_bounds() {
    let ctx = Context::default();
    let mut count = 0;
    let mut paint = |frame: &mut UiFrame<'_>| {
        frame.ctx.root_ui(|ui| {
            let response = ui.interact(
                Rect::from_min_size(pos2(20.0, 30.0), vec2(100.0, 45.0)),
                egui::Id::unique("continue"),
                egui::Sense::click(),
            );
            frame.record("continue", "Continue", "button", &response);
            if response.clicked() {
                count += 1;
            }
        });
    };
    frame(&ctx, vec![], &mut paint);
    frame(&ctx, pointer(true, pos2(50.0, 50.0)), &mut paint);
    let widgets = frame(&ctx, pointer(false, pos2(50.0, 50.0)), &mut paint);
    frame(&ctx, vec![], &mut paint);
    assert_eq!(count, 1);
    let bounds = widgets["continue"].bounds;
    assert_eq!([bounds.x, bounds.y, bounds.width, bounds.height], [20.0, 30.0, 100.0, 45.0]);
}

#[test]
fn text_edit_consumes_unicode_composition_and_keyboard_deletion() {
    let ctx = Context::default();
    let mut text = String::new();
    let mut paint = |frame: &mut UiFrame<'_>| {
        frame.ctx.root_ui(|ui| {
            let response = ui.put(
                Rect::from_min_size(pos2(20.0, 30.0), vec2(250.0, 45.0)),
                egui::TextEdit::singleline(&mut text).id_salt("name"),
            );
            frame.record("name", "Wizard name", "textbox", &response);
        });
    };
    frame(&ctx, vec![], &mut paint);
    frame(&ctx, pointer(true, pos2(50.0, 50.0)), &mut paint);
    frame(&ctx, pointer(false, pos2(50.0, 50.0)), &mut paint);
    frame(
        &ctx,
        vec![
            egui::Event::Text("Felix ".to_owned()),
            egui::Event::Ime(egui::ImeEvent::Preedit {
                text: "魔".to_owned(),
                active_range_chars: None,
            }),
        ],
        &mut paint,
    );
    frame(&ctx, vec![egui::Event::Ime(egui::ImeEvent::Commit("魔法".to_owned()))], &mut paint);
    let widgets = frame(
        &ctx,
        vec![egui::Event::Key {
            key: egui::Key::Backspace,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
        &mut paint,
    );
    assert!(ctx.egui_wants_keyboard_input());
    assert!(widgets["name"].focused);
    assert_eq!(text, "Felix 魔");
}

#[test]
fn geometry_excludes_widgets_clipped_out_of_the_scrolling_panel() {
    let ctx = Context::default();
    let paint = |frame: &mut UiFrame<'_>| {
        frame.ctx.root_ui(|ui| {
            ui.set_clip_rect(Rect::from_min_size(pos2(0.0, 0.0), vec2(200.0, 100.0)));
            let response = ui.interact(
                Rect::from_min_size(pos2(10.0, 150.0), vec2(100.0, 20.0)),
                egui::Id::unique("hidden"),
                egui::Sense::click(),
            );
            frame.record("hidden", "Clipped row", "button", &response);
            let response = ui.interact(
                Rect::from_min_size(pos2(10.0, 90.0), vec2(100.0, 20.0)),
                egui::Id::unique("partial"),
                egui::Sense::click(),
            );
            frame.record("partial", "Partial row", "button", &response);
        });
    };
    let widgets = frame(&ctx, vec![], paint);
    assert!(!widgets.contains_key("hidden"));
    assert_eq!(widgets["partial"].bounds.height, 10.0);
}

#[test]
fn a_real_text_pointer_focus_preserves_characters_queued_in_the_same_frame() {
    let ctx = Context::default();
    let mut text = String::new();
    let mut paint = |frame: &mut UiFrame<'_>| {
        frame.ctx.root_ui(|ui| {
            let response = ui.put(
                Rect::from_min_size(pos2(20.0, 30.0), vec2(250.0, 45.0)),
                egui::TextEdit::singleline(&mut text).id_salt("name"),
            );
            frame.record("name", "Wizard name", "textbox", &response);
        });
    };
    let widgets = frame(&ctx, vec![], &mut paint);
    let mut events = pointer(true, pos2(50.0, 50.0));
    events.push(egui::Event::Text("Felix".to_owned()));
    focus_pointer_text(&ctx, &widgets, &events);
    frame(&ctx, events, &mut paint);
    assert_eq!(text, "Felix");
}

#[test]
fn duplicate_visible_semantic_ids_are_rejected_instead_of_overwriting_a_control() {
    let ctx = Context::default();
    let widgets = frame(&ctx, vec![], |frame| {
        frame.record_rect(
            "continue",
            "Continue",
            "button",
            Rect::from_min_size(pos2(10.0, 10.0), vec2(100.0, 40.0)),
            true,
        );
        frame.record_rect(
            "continue",
            "Other control",
            "button",
            Rect::from_min_size(pos2(10.0, 80.0), vec2(100.0, 40.0)),
            true,
        );
    });
    assert_eq!(widgets["continue"].label, "Continue");
    assert!(widgets["continue"].duplicate);
}
