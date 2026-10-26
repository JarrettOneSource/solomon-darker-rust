# GPU interface

Rust owns the visible game UI and desktop launcher. Menus, HUD, panels, social
controls, loading screens, notifications, and overlays use GPU geometry, textures,
and text. Browser/platform adapters provide input, accessibility, clipboard, files,
audio, networking, and storage.

## Composition

`crates/web/src/gpu_ui/` owns egui interaction, layout, text editing, and resource
lifetimes. Feature controllers borrow accepted models and emit typed actions.
The shared graphics renderer submits their final shapes.

One application layer order governs painting, pointer hits, modal comparison, and
controller scope. Passive backgrounds allow child controls to receive input.
World notices stay below the HUD; modal and transition layers keep their explicit
scope. The local egui integration is documented in
[its patch contract](../vendor/egui/SOLOMON-PATCH.md).

For each input dispatch, resolve the active control, commit its action, and
refresh the model before processing the next event. The final layout pass paints
without replaying actions. Hidden sizing passes cannot consume focus requests.

## Input contract

- Hit testing uses the same translation, scale, clipping, and authored shape as
  painting, including rotated touch controls.
- Cancellation clears pointer/click/drag ownership without activation.
- A held stationary press activates on release; an outside release does not.
- Browser click detail supplies multi-click provenance where the control needs it.
- Keyboard/controller focus tracks control identity and content kind across
  speech, choices, selectors, and modal replacement.
- Tab can enter GPU focus after touch without escaping to browser chrome.
- Fullscreen entry runs synchronously inside trusted input, with one activation
  per pointer, keyboard, or controller action.
- Binding capture consumes its terminating event before another surface can use it.
- All three eight-slot belt banks share the simulation's slot bound.
- Modal UI seals local gameplay input while preserving the owning world's pause rules.

## Text and accessibility

Stock bitmap fonts retain their authored metrics and glyph order. Licensed/system
fonts use complete shaped runs, including kerning, combining marks, Arabic joins,
and emoji clusters. Painting, caret movement, selection, wrapping, composition,
and clipping use the same layout.

Platform adapters provide native composition, clipboard access, and file dialogs.
Native accessibility uses AccessKit and the operating system's accessibility tree.
Browser announcements use `ariaNotify` when available or one marked, nonvisual
text-only live region. Native text shaping and composition use Parley and winit;
browser text uses its native font and composition APIs.

GPU textures for shaped text belong to the current egui context. Retired runs
release through that context's managed resource lifetime.

## Startup and transitions

The browser's separate Rust loader renders the splash while resources load. Progress counts
completed images, fonts, and audio. It hands off only after account/save admission
and the first complete game frame; retired loader draws stop.

Resource construction and saved-state admission are sequential phases. Large
session records are heap-owned at parsing boundaries to avoid overlapping
WebAssembly stack frames.

Match loading has its own delay, progress, text, and input owner. Failed Create
admission releases its pending commit while retaining the draft. Cancelling a
saved-content mismatch restores the previous menu and preserves the save.
Saved Leave reports persistence before permitting acknowledgement or reload.

## Surface map

Paths below are relative to `crates/web/src/`; journeys live in
`tools/browser-smoke/`.

| Surface | Main owner | Checks |
| --- | --- | --- |
| Splash and resource failure | `runtime_ui.rs`, separate loader crate | `startup.cjs` |
| Title, Create, Pause, settings | `menu/` | `menus.cjs`, `menu-input.cjs`, `match-loading.cjs` |
| Desktop launch settings | `desktop/` | Packaged desktop journey |
| HUD, belt, skill selection | `hud/`, input and picker owners | `hud-input.cjs`, `gameplay-input.cjs`, belt/input unit tests |
| Inventory, Skillbook, services | `player_panels/` | `player-panels.cjs`, `hub-services.cjs` |
| Dialogue, questions, selectors | Dialogue and panel controllers | Interaction fixtures and GPU navigation tests |
| Party, chat, player cards | `social/` | `hosted-social.cjs` |
| Cloud content and sharing | `menu/cloud/` | `dark-cloud.cjs`, `mobile-sharing.cjs`, `routing-content.cjs` |
| World notices and book feedback | Notification and feedback owners | Notification and feedback lifecycle tests |
| Save/Continue and conflicts | Storage, menu, and session owners | `save-resume.cjs`, `saved-game.cjs`, packaged desktop journey |
| Viewport and touch | Shared layout and input owners | `responsive.cjs`, `hud-input.cjs` |

The original C++ game supplies retail artwork, layout evidence, and recovered
interaction rules. Regression recordings retain their actual provenance;
[Reference data](REFERENCE-DATA.md) explains that boundary.

## Validation

Source checks verify GPU ownership and resource paths. Browser journeys use
ordinary input and read-only geometry/state observations. Review screenshots and
collect page errors, console errors, failed requests, and cleanup results.

Run cold/warm startup, delayed and failed required resources, resize, input-device
changes, modal replacement, save/Continue, and the affected feature's failure paths.
For desktop changes, inspect and exercise each affected final package.
[Release](RELEASE.md) defines publication and live acceptance separately.
