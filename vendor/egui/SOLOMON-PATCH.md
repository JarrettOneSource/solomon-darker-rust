# Egui integration patch

The game vendors egui 0.37.0 from the crates.io package with SHA256
`bec9144f49b7a28b5f395d3147e325b4fe3e77ba686eac11d1bdf0a7da3f375f`.
Its upstream revision is `7c0b40402271e8d40cda69f87cf3c2617c2e4382`.

The package retains its `MIT OR Apache-2.0` license and upstream notices.
The included MIT and Apache license texts have SHA256 values
`95ca92f5f8ea5231f1580b3a2a799e8260af3114b900e1def5355a7f44bcf60c` and
`8173d5c29b4f956d532781d2b86e4e30f83e6b7878dce18c919451d6ba707c90`.

## Integration behavior

| Files under `src/` | Behavior |
| --- | --- |
| `context.rs`, `memory/mod.rs`, `layers.rs` | Opt-in fixed ordering shared by paint draining, pointer ownership, and modal comparisons |
| `containers/area.rs`, `ui.rs`, `widget_rect.rs`, `hit_test.rs` | Passive backgrounds with real child widgets, clipped hit regions, and optional authored hit predicates |
| `data/input/event.rs`, `input_state/mod.rs`, `interaction.rs`, `context.rs` | Pointer cancellation that retires button, click, drag, and motion ownership without release activation |
| `input_state/mod.rs`, `input_state/wheel_state.rs` | Opt-in `raw_point_scroll` for one-time consumption of platform-paced pixel deltas |
| `context.rs`, `memory/mod.rs` | Focusable registrations, deferred focus, and scoped controller activation |

Registering a fixed layer does not create an Area, widget, geometry, visibility,
or focus. The registry is runtime configuration and is not persisted.
Unregistered contexts retain upstream ordering.

Passive hover content does not occlude lower controls; disabled interactive
children still block. Hit predicates use the application's ellipse/rotation
geometry and cannot call Context while it is locked.

Cancellation preserves raw Touch cancellation and ordinary release behavior.
Raw point scrolling preserves phase, source, modifiers, and line/page handling;
upstream smoothing remains the default.

The application resolves sampled controller navigation under the highest active
input scope. Confirmation may queue one activation for the next commit. Both
queueing and delivery require the same enabled, visible click widget, layer, and
modal admission. A missing target expires after that pass.

## Application API

Layer and hit integration uses `Context::set_fixed_layer_order`,
`fixed_layer_order_index`, `compare_layer_order`, `set_widget_hit_test`,
`pointer_widget_at`, `Area::background_interactable`, and
`Event::PointerCancelled`.

Navigation uses `Context::focusable_widgets_in_layer`,
`request_activation_next_frame`, `Memory::take_focus_direction`, and
`request_focus_next_frame`.

The application supplies the registry before input/layout. The native text adapter
resolves its target through the same widget hit test.

## Upgrade checks

The workspace selects this package with `[patch.crates-io]` and excludes it from
application lint targets.

Application regressions in `crates/web/src/gpu_ui/layer_tests.rs` cover paint,
input, modal order, reappearance, passive controls, disabled children, and cutouts.
`crates/web/src/gpu_ui/input/cancel_tests.rs` covers cancellation and later taps.
`crates/web/src/menu/widgets.rs` covers wheel pacing and focus reveal.

Paths in this section are relative to the repository root.
An upgrade must retain these contracts or replace the patch with supported APIs.
