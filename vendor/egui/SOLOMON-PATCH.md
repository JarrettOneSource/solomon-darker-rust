# Pinned egui integration

This is egui0.37.0, extracted from the unchanged crates.io registry package. Its SHA256
is `bec9144f49b7a28b5f395d3147e325b4fe3e77ba686eac11d1bdf0a7da3f375f`, matching
the original workspace Cargo.lock. The package's `.cargo_vcs_info.json` records
upstream revision `7c0b40402271e8d40cda69f87cf3c2617c2e4382`.

The package manifest retains its original `MIT OR Apache-2.0` license. The release's
unchanged license texts are included from
[LICENSE-MIT](https://raw.githubusercontent.com/emilk/egui/0.37.0/LICENSE-MIT) and
[LICENSE-APACHE](https://raw.githubusercontent.com/emilk/egui/0.37.0/LICENSE-APACHE).
Their SHA256 values are respectively
`95ca92f5f8ea5231f1580b3a2a799e8260af3114b900e1def5355a7f44bcf60c` and
`8173d5c29b4f956d532781d2b86e4e30f83e6b7878dce18c919451d6ba707c90`.

Local changes are confined to seven implementation files:

- `context.rs`, `memory/mod.rs`, `layers.rs`: one opt-in fixed layer order shared by
  paint draining, pointer ownership and modal comparisons. Registering an ID creates
  no Area, widget, visibility, geometry or focus. Unregistered/default contexts keep
  normal egui behavior. The fixed registry is runtime configuration, not persisted memory.
- `containers/area.rs`, `ui.rs`, `widget_rect.rs`, `hit_test.rs`: passive container
  backgrounds retain layout and genuine child widgets. Hover-only passive content does
  not occlude lower controls; disabled interactive children still block. Native pointer
  queries use actual clipped child regions and optional pure authored hit predicates.

Public integration methods are `Context::set_fixed_layer_order`,
`Context::fixed_layer_order_index`, `Context::compare_layer_order`, `Context::set_widget_hit_test`,
`Context::pointer_widget_at`, and `Area::background_interactable`.
The application supplies its recovered fixed registry before input/layout. Hit predicates
reuse its canonical ellipse/rotation geometry and cannot call Context while it is locked.
The native textarea adapter resolves its target through the same real widget hit test.

No animation, typography, Area lifetime, widget parentage, clip geometry or default
window policy is replaced. No shared Cargo registry files are modified. This package
is excluded from the application workspace's lint targets and selected with
`[patch.crates-io]`; its upstream manifest/dependencies remain unchanged.

Focused application regressions live in `crates/web/src/gpu_ui/layer_tests.rs`, covering
real paint/input/modal order, hidden registration, reappearance, default restoration,
passive controls, disabled children and circular cutout ownership. Further upstream
upgrades must retain these contracts or replace the patch with equivalent supported APIs.
