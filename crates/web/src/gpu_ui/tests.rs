use super::*;

fn frame(
    ctx: &Context,
    events: Vec<egui::Event>,
    paint: impl FnMut(&mut UiFrame<'_>),
) -> BTreeMap<String, WidgetGeometry> {
    frame_codes(ctx, events, &[], paint)
}

fn frame_codes(
    ctx: &Context,
    events: Vec<egui::Event>,
    codes: &[BrowserKey],
    paint: impl FnMut(&mut UiFrame<'_>),
) -> BTreeMap<String, WidgetGeometry> {
    frame_native(ctx, events, codes, &[], paint)
}

fn frame_native(
    ctx: &Context,
    events: Vec<egui::Event>,
    codes: &[BrowserKey],
    receipts: &[edit::model::Receipt],
    paint: impl FnMut(&mut UiFrame<'_>),
) -> BTreeMap<String, WidgetGeometry> {
    let images = painter::Images::default();
    let widgets = RefCell::new(BTreeMap::new());
    let announcements = RefCell::new(Vec::new());
    let input_scope = RefCell::new(None);
    let retired_images = RefCell::new(Vec::new());
    let previous_widgets = BTreeMap::new();
    let editors = RefCell::new(Vec::new());
    let editor_errors = RefCell::new(Vec::new());
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
            key_codes: codes,
            blend: BlendMode::Normal,
            color_filter: ColorFilter::default(),
            announcements: &announcements,
            rotation: egui::emath::Rot2::IDENTITY,
            translation: Vec2::ZERO,
            input_scope: &input_scope,
            retired_images: &retired_images,
            focus_from_keyboard: true,
            editor_receipts: receipts,
            editors: &editors,
            editor_errors: &editor_errors,
        })
    })
    .drop_without_applying_deltas();
    edit::set_focus(ctx, &editors.into_inner());
    assert!(editor_errors.into_inner().is_empty());
    widgets.into_inner()
}

#[test]
fn focus_ring_modality_tracks_actual_pointer_and_keyboard_dispatches() {
    let mouse = pointer(true, pos2(50.0, 50.0));
    assert!(!focus_modality(true, &mouse));
    assert!(!focus_modality(false, &[egui::Event::ModifiersChanged(egui::Modifiers::SHIFT)]));
    let navigation = egui::Event::Key {
        key: egui::Key::Tab,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    };
    assert!(focus_modality(false, &[navigation]));
    assert!(focus_modality(true, &[egui::Event::PointerMoved(pos2(60.0, 60.0))]));
    assert!(focus_modality(true, &[]));
}

#[test]
fn native_color_filter_uses_css_luminance_and_preserves_outline_alpha() {
    let ctx = Context::default();
    frame(&ctx, Vec::new(), |frame| {
        let source = Color32::from_rgba_unmultiplied(255, 0, 0, 128);
        assert_eq!(frame.filtered_color(source), source);
        let filtered = frame.with_saturation(0.0).filtered_color(source);
        assert_eq!(filtered.a(), source.a());
        for channel in &filtered.to_srgba_unmultiplied()[..3] {
            assert!(channel.abs_diff(54) <= 1);
        }
    });
    let disabled = ColorFilter { brightness: 0.62, saturation: 0.35 }.rgb([1.0, 0.0, 0.0]);
    for (actual, expected) in disabled.into_iter().zip([0.302839, 0.085839, 0.085839]) {
        assert!((actual - expected).abs() < 0.000001);
    }
}

#[test]
fn gpu_group_tessellation_keeps_offscreen_source_and_applies_parent_material_once() {
    let ctx = Context::default();
    let images = painter::Images::default();
    let widgets = RefCell::new(BTreeMap::new());
    let announcements = RefCell::new(Vec::new());
    let input_scope = RefCell::new(None);
    let retired_images = RefCell::new(Vec::new());
    let previous_widgets = BTreeMap::new();
    let editors = RefCell::new(Vec::new());
    let editor_errors = RefCell::new(Vec::new());
    let output = ctx.run_pass(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(100.0, 100.0))),
            ..Default::default()
        },
        |ctx| {
            let frame = UiFrame {
                ctx,
                images: &images,
                widgets: &widgets,
                previous_widgets: &previous_widgets,
                key_codes: &[],
                blend: BlendMode::Normal,
                color_filter: ColorFilter::default(),
                announcements: &announcements,
                rotation: egui::emath::Rot2::IDENTITY,
                translation: Vec2::ZERO,
                input_scope: &input_scope,
                retired_images: &retired_images,
                focus_from_keyboard: true,
                editor_receipts: &[],
                editors: &editors,
                editor_errors: &editor_errors,
            };
            let mut painter = ctx.layer_painter(egui::LayerId::new(
                egui::Order::Middle,
                egui::Id::unique("gpu-group"),
            ));
            painter.set_clip_rect(ctx.content_rect());
            painter.set_opacity(0.4);
            frame
                .with_brightness(1.2)
                .paint_group(
                    &painter,
                    GroupEffect { opacity: 0.5, shadows: Vec::new() },
                    |frame, painter| {
                        assert_eq!(frame.color_filter, ColorFilter::default());
                        assert_eq!(painter.opacity(), 1.0);
                        painter.rect_filled(
                            Rect::from_min_size(pos2(-5.0, 20.0), vec2(20.0, 10.0)),
                            0.0,
                            Color32::WHITE,
                        );
                        Ok(())
                    },
                )
                .unwrap();
        },
    );
    let nodes =
        effects::parse(ctx.tessellate(output.shapes.clone(), output.pixels_per_point)).unwrap();
    output.drop_without_applying_deltas();
    assert_eq!(nodes.len(), 1);
    let effects::Node::Group { style, children, .. } = &nodes[0] else { panic!("GPU group") };
    assert!((style.effect.opacity - 0.2).abs() < 0.00001);
    assert_eq!(style.filter.brightness, 1.2);
    assert!(effects::content_bounds(children).left() < 0.0);
}

fn browser_key(code: &str, key: &str) -> BrowserKey {
    BrowserKey {
        code: code.to_owned(),
        key: key.to_owned(),
        pressed: true,
        repeat: false,
        alt: false,
        ctrl: false,
        shift: false,
        meta: false,
    }
}

#[test]
fn a_clicked_binding_row_commits_before_the_following_escape_dispatch() {
    let ctx = Context::default();
    let mut controls_open = true;
    let mut binding = false;
    let mut cancelled = 0;
    let mut clicked = 0;
    let mut paint = |frame: &mut UiFrame<'_>| {
        for key in frame.key_codes() {
            if key.code == "Escape" {
                if binding {
                    binding = false;
                    cancelled += 1;
                } else {
                    controls_open = false;
                }
            }
        }
        if controls_open {
            frame.ctx.root_ui(|ui| {
                let response = ui.interact(
                    Rect::from_min_size(pos2(20.0, 30.0), vec2(100.0, 45.0)),
                    egui::Id::unique("bind:move-up"),
                    egui::Sense::click(),
                );
                frame.record("bind:move-up", "Move up", "button", &response);
                if response.clicked() {
                    binding = true;
                    clicked += 1;
                }
            });
        }
    };
    frame(&ctx, Vec::new(), &mut paint);
    let mut events = pointer(true, pos2(50.0, 50.0));
    events.extend(pointer(false, pos2(50.0, 50.0)));
    events.push(egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    });
    let passes = batches::into_passes(
        egui::RawInput { events, ..Default::default() },
        vec![browser_key("Escape", "Escape")],
        Vec::new(),
        vec![
            batches::DispatchEnd { events: 2, codes: 0, edits: 0 },
            batches::DispatchEnd { events: 4, codes: 0, edits: 0 },
            batches::DispatchEnd { events: 5, codes: 1, edits: 0 },
        ],
    );
    for pass in passes {
        frame_codes(&ctx, pass.raw.events, &pass.codes, &mut paint);
    }
    assert!(controls_open, "Escape cancels the newly selected binding instead of closing Controls");
    assert!(!binding);
    assert_eq!((clicked, cancelled), (1, 1));
}

#[test]
fn a_new_field_has_committed_geometry_before_an_immediate_click_and_first_character() {
    let ctx = Context::default();
    let mut opened = false;
    let mut text = String::new();
    let mut paint = |frame: &mut UiFrame<'_>| {
        frame.ctx.root_ui(|ui| {
            if opened {
                // A stock space exercises actual editing/focus without requiring
                // a browser image upload in this native interaction regression.
                NativeTextEdit::singleline(&mut text, "new-name", "Name")
                    .native_name(Pos2::ZERO, 1.0)
                    .margin(Vec2::ZERO)
                    .show(frame, ui, Rect::from_min_size(pos2(20.0, 100.0), vec2(250.0, 45.0)));
            } else {
                let response = ui.interact(
                    Rect::from_min_size(pos2(20.0, 30.0), vec2(100.0, 45.0)),
                    egui::Id::unique("open-name"),
                    egui::Sense::click(),
                );
                frame.record("open-name", "Open name", "button", &response);
                if response.clicked() {
                    opened = true;
                }
            }
        });
    };
    let mut widgets = frame(&ctx, Vec::new(), &mut paint);
    let mut events = pointer(true, pos2(50.0, 50.0));
    events.extend(pointer(false, pos2(50.0, 50.0)));
    events.extend(pointer(true, pos2(50.0, 120.0)));
    events.push(enter_key(egui::Key::Space));
    let receipt = edit::model::Receipt::unbound(
        1,
        1.0,
        edit::model::Intent::Replace { range: TextSelection::caret(0), text: " ".into() },
    );
    let passes = batches::into_passes(
        egui::RawInput { events, ..Default::default() },
        vec![browser_key("Space", " ")],
        vec![receipt],
        vec![
            batches::DispatchEnd { events: 2, codes: 0, edits: 0 },
            batches::DispatchEnd { events: 4, codes: 0, edits: 0 },
            batches::DispatchEnd { events: 6, codes: 0, edits: 0 },
            batches::DispatchEnd { events: 7, codes: 1, edits: 1 },
        ],
    );
    for pass in passes {
        focus_pointer_text(&ctx, &widgets, &pass.raw.events);
        widgets = frame_native(&ctx, pass.raw.events, &pass.codes, &pass.edits, &mut paint);
    }
    assert!(opened);
    assert_eq!(text, " ");
    assert!(widgets["new-name"].focused);
    assert!(text_edit_focused(&ctx));
}

fn enter_key(key: egui::Key) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}

#[test]
fn native_editor_enter_is_per_dispatch_and_keeps_the_real_singleline_focus() {
    let ctx = Context::default();
    let mut text = " ".to_owned();
    let descriptor = RefCell::new(None);
    let submitted = std::cell::Cell::new(false);
    let mut paint = |frame: &mut UiFrame<'_>| {
        frame.ctx.root_ui(|ui| {
            let response = NativeTextEdit::singleline(&mut text, "native-form", "Query")
                .native_name(Pos2::ZERO, 1.0)
                .request_focus(true)
                .show(frame, ui, Rect::from_min_size(pos2(20.0, 30.0), vec2(250.0, 45.0)));
            submitted.set(response.submitted);
            *descriptor.borrow_mut() = frame.editors.borrow().last().cloned();
        });
    };
    frame(&ctx, Vec::new(), &mut paint);
    let mut session = edit::session::Session::new(descriptor.borrow().clone().unwrap());
    let first = session.receipt(1, 1.0, edit::model::Intent::Submit, None, None);
    let widgets = frame_native(&ctx, vec![enter_key(egui::Key::Enter)], &[], &[first], &mut paint);
    assert!(submitted.get());
    assert!(widgets["native-form"].focused);
    frame(&ctx, Vec::new(), &mut paint);
    assert!(!submitted.get());
    let next = session.receipt(2, 1.0, edit::model::Intent::Submit, None, None);
    let widgets = frame_native(&ctx, vec![enter_key(egui::Key::Enter)], &[], &[next], &mut paint);
    assert!(submitted.get());
    assert!(widgets["native-form"].focused);
    assert_eq!(text, " ");
}

#[test]
fn a_remounted_field_does_not_accept_a_native_receipt_from_its_retired_instance() {
    let ctx = Context::default();
    let mut text = " ".to_owned();
    let mut descriptor = None;
    frame(&ctx, Vec::new(), |frame| {
        frame.ctx.root_ui(|ui| {
            NativeTextEdit::singleline(&mut text, "rename", "Name")
                .native_name(Pos2::ZERO, 1.0)
                .request_focus(true)
                .show(frame, ui, Rect::from_min_size(pos2(20.0, 30.0), vec2(250.0, 45.0)));
            descriptor = frame.editors.borrow().last().cloned();
        })
    });
    let mut old_session = edit::session::Session::new(descriptor.unwrap());
    let old_input = old_session.receipt(
        1,
        1.0,
        edit::model::Intent::Replace { range: TextSelection::caret(1), text: " ".into() },
        None,
        None,
    );
    frame(&ctx, Vec::new(), |_| {});
    text.clear();
    frame_native(&ctx, Vec::new(), &[], &[old_input], |frame| {
        frame.ctx.root_ui(|ui| {
            NativeTextEdit::singleline(&mut text, "rename", "Name")
                .native_name(Pos2::ZERO, 1.0)
                .request_focus(true)
                .show(frame, ui, Rect::from_min_size(pos2(20.0, 30.0), vec2(250.0, 45.0)));
        })
    });
    assert_eq!(
        text, "",
        "the same public field name does not revive a retired native editing owner"
    );
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
    assert_eq!(widgets["partial"].paint_bounds.height, 20.0);
}

#[test]
fn authored_clip_reports_visible_notification_ink_and_preserves_original_paint_bounds() {
    let ctx = Context::default();
    let widgets = frame(&ctx, vec![], |frame| {
        let clip = Rect::from_min_max(pos2(0.0, 50.0), pos2(400.0, 300.0));
        frame.record_clipped_rect(
            "notification:first",
            "Level up",
            "status",
            Rect::from_min_size(pos2(10.0, 40.0), vec2(100.0, 25.0)),
            clip,
            true,
        );
        frame.record_clipped_rect(
            "notification:retired",
            "Old message",
            "status",
            Rect::from_min_size(pos2(10.0, 20.0), vec2(100.0, 25.0)),
            clip,
            true,
        );
    });
    let visible = &widgets["notification:first"];
    assert_eq!((visible.bounds.y, visible.bounds.height), (50.0, 15.0));
    assert_eq!((visible.paint_bounds.y, visible.paint_bounds.height), (40.0, 25.0));
    assert!(!visible.is_interactive());
    assert!(!widgets.contains_key("notification:retired"));
}

#[test]
fn rotated_control_hits_follow_the_painted_layer_scale_and_translation() {
    for radians in [0.0, std::f32::consts::FRAC_PI_4] {
        let ctx = Context::default();
        let transform = egui::emath::TSTransform::new(vec2(17.0, 11.0), 0.5);
        let center = pos2(200.0, 180.0);
        let size = vec2(120.0, 70.0);
        let rotation = egui::emath::Rot2::from_angle(radians);
        let corners = Rect::from_center_size(Pos2::ZERO, size);
        let bounds = Rect::from_points(&[
            center + rotation * corners.left_top().to_vec2(),
            center + rotation * corners.right_top().to_vec2(),
            center + rotation * corners.right_bottom().to_vec2(),
            center + rotation * corners.left_bottom().to_vec2(),
        ]);
        let paint = |frame: &mut UiFrame<'_>| {
            frame.ctx.root_ui(|ui| {
                ui.ctx().set_transform_layer(ui.layer_id(), transform);
                let response =
                    ui.interact(bounds, egui::Id::unique("rotated"), egui::Sense::click());
                frame.record_rotated_rect(
                    "rotated",
                    "Inventory",
                    "button",
                    &response,
                    center,
                    size,
                    radians,
                );
            });
        };
        frame(&ctx, vec![], paint);
        let widgets = frame(&ctx, vec![], paint);
        let control = &widgets["rotated"];
        assert!(control.contains(transform * center), "the visible center owns touch input");
        assert!(control.contains(transform * (center + rotation * vec2(55.0, 0.0))));
        assert!(!control.contains(transform * (center + rotation * vec2(0.0, 40.0))));
    }
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
fn a_noninteractive_text_edit_never_receives_forced_pointer_focus() {
    let ctx = Context::default();
    let mut text = "Finalizing wizard".to_owned();
    let mut paint = |frame: &mut UiFrame<'_>| {
        frame.ctx.root_ui(|ui| {
            let response = ui.put(
                Rect::from_min_size(pos2(20.0, 30.0), vec2(250.0, 45.0)),
                egui::TextEdit::singleline(&mut text).id_salt("name").interactive(false),
            );
            frame.record("name", "Wizard name", "textbox", &response);
        });
    };
    let widgets = frame(&ctx, vec![], &mut paint);
    // TextEdit::interactive(false) preserves the containing UI's enabled state.
    assert!(widgets["name"].enabled);
    assert!(!widgets["name"].is_interactive());
    let events = pointer(true, pos2(50.0, 50.0));
    focus_pointer_text(&ctx, &widgets, &events);
    assert!(ctx.memory(|memory| memory.focused().is_none()));
    frame(&ctx, events, &mut paint);
    assert!(!ctx.text_edit_focused());
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

#[test]
fn inherited_rotation_keeps_stock_and_glyph_vertices_in_one_paint_transform() {
    let ctx = Context::default();
    frame(&ctx, vec![], |frame| {
        let rotated = frame.with_rotation(pos2(100.0, 100.0), std::f32::consts::FRAC_PI_2);
        let point = rotated.paint_position(pos2(110.0, 100.0));
        assert!((point - pos2(100.0, 110.0)).length() < 0.0001);
        let restored = rotated.with_rotation(pos2(100.0, 100.0), -std::f32::consts::FRAC_PI_2);
        assert!(
            (restored.paint_position(pos2(110.0, 100.0)) - pos2(110.0, 100.0)).length() < 0.0001
        );
        assert!(
            (rotated.with_blend(BlendMode::Add).paint_position(pos2(110.0, 100.0)) - point)
                .length()
                < 0.0001
        );
        let scaled = frame.with_scale(pos2(100.0, 100.0), 0.5);
        assert!((scaled.paint_position(pos2(110.0, 100.0)) - pos2(105.0, 100.0)).length() < 0.0001);
    });
}
