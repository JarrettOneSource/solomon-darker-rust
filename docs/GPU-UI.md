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

The frame owner calls `begin_frame`, processes ordered `paint_next` input passes
with model refresh between them, and uses `paint_layout` for the final visual
pass. `finish_frame` completes managed texture uploads and retirements even when
a controller returns an error. Returning to the editor stops later queued input
and gameplay work in that frame while still completing resource cleanup.

Painting and input share the same transformed viewport. Inverse-transform the
browser viewport before clipping a design-space Area; clipping against unscaled
CSS coordinates can discard valid controls before the layer transform. Compact
player panels reserve a 48-pixel navigation bar above their scrolling body.
Rendering, hit regions, and raw pointer/wheel admission use the same body clip,
and an active drag retains its owner through release or cancellation outside it.

Owned charm controls identify their displayed slot. A wizard may own two tonics,
so the charm selector is not a unique UI identity. Hover and activation resolve
the slot against accepted economy state; removal still emits the canonical charm
selector and never removes a tonic.

## Materials and resource ownership

The health meter adds a pulsing health highlight while the accepted player has Regenerate
active. The pulse clips to the current health fill and follows the meter's scale and
rotation. The Skills book's Boasts view shows the selected stock or authored Boast,
its statement, and its accepted progress: gold for active, green for achieved, red
for failed. Escape returns to Skills. These are shared native/browser UI enhancements;
they do not change retail gameplay, mana reservation, or Boast success rules.

The shared material applies saturation and brightness in sRGB on straight color,
clamping each operation before blend premultiplication and preserving alpha.
Brightness may exceed one, which a byte tint alone cannot express. Saturation uses
the .213/.715/.072 luminance matrix. `UiFrame::filtered_color` gives vector text
and outlines the same operation as stock images and bitmap text.

Sprite shadows sample the selected atlas record's alpha and treat pixels outside
its bounds as transparent. Neighboring records must not bleed into the blur.
Shadow offsets and Gaussian sigma use design coordinates independently of the
sprite's own scale. Group opacity applies after source/shadow composition.

`UiFrame::paint_group` captures a complete group's GPU geometry, including shaped
text, into reusable targets. Child effects compose before parent effects. Clipping
happens after filtering so off-clip pixels can contribute to a visible shadow.
The renderer restores framebuffer, projection, blend, and sampling state after
the group. Targets are reused by nesting depth and destroyed with their owner.

Outer and inner box shadows use opaque geometry as their caster, rather than the
visible image's alpha. An outer shadow removes the original interior; an inner
shadow remains inside it. Spread expands the caster while retaining the original
interior for removal. Text/box-shadow blur radius is twice Gaussian sigma; the
drop-shadow operation takes sigma directly. Large shadows use dense separable
passes to avoid undersampled edges.

An authored icon can request nearest-neighbor sampling without changing its
borrowed world texture. The renderer selects a per-draw sampler and restores it
afterward; the texture keeps its owner, upload lifetime, and filtering parameters.
Material batch keys include sampler choice.

These contracts are exercised by `gpu-color-filter.cjs`, `gpu-composition.cjs`,
and the graphics crate's `composition_probe` example. Composed feature checks
still need to verify the controller's chosen material and actual appearance.

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

The canvas declares its touch-action policy before a gesture starts; cancelling
pointer callbacks alone cannot prevent browser pan/zoom from taking the stream.
Blur and genuine touch cancellation clear held movement and the painted joystick.
Controller Back and physical Escape reach the same typed panel action once.
After that action changes a surface, the next dispatch recomputes input ownership.

Coarse portrait devices show rotate-to-landscape guidance for native game stages.
Dark Cloud remains portrait-admitted. Responsive checks must exercise that rule,
then rotate for gameplay, rather than bypassing admission to test hidden controls.

## Text and accessibility

Stock bitmap fonts retain their authored metrics and glyph order. Licensed/system
fonts use complete shaped runs, including kerning, combining marks, Arabic joins,
and emoji clusters. Painting, caret movement, selection, wrapping, composition,
and clipping use the same layout.

Platform adapters provide native composition, clipboard access, and file dialogs.
Native accessibility uses AccessKit and the operating system's accessibility tree.
Loading exposes a status node for the transition's lifetime, including its artwork
delay, so assistive clients can distinguish pending admission from ready gameplay.
Text ranges, bidirectional character bounds and UTF-16 selection come from the
same editor layout. OS edits and selection requests enter the normal editing
model, including read-only policy and undo history.
Settings sliders expose their numeric value, bounds and step. Native adjustment
requests update the same bounded setting used by pointer and keyboard input.
Browser announcements use `ariaNotify` when available or one marked, nonvisual
text-only live region. Native text shaping and composition use Parley and winit;
browser text uses its native font and composition APIs.

GPU textures for shaped text belong to the current egui context. Retired runs
release through that context's managed resource lifetime.
Overlapping native windows share one retained font system; discarding a failed
game load cannot retire the launcher's font resources.

The browser font adapter measures complete joined/bidirectional runs with a hidden,
noninteractive SVG text probe, attached only for synchronous measurement and removed
on success or failure. An unattached canvas rasterizes text-only GPU tiles. The
editor uses those measurements for weak carets, disjoint selections, composition
underlines, and wrapping. Grapheme and line-break rules prevent splitting an emoji
cluster or joining sequence. Native clipboard operations observe the current
accepted text and UTF-16 selection immediately, including between draw frames.

Announcements carry the identity of their status owner or accepted event. Compare
that owner's text with the previous foreground frame: unchanged text stays quiet,
while a new event with identical words still announces. Preserve producer order
and publish a changed batch together so the fallback live region does not overwrite
earlier messages. A message that leaves and later returns can announce again.
Browser announcement delivery does not by itself verify hardware screen-reader audio.

## Accepted feedback and world notices

`WorldNotifications` consumes accepted local loot, rank-book, overload, and
Cheat Death events. Each lane retains its own cursor and source key; equal numeric
IDs from different lanes are distinct events. Restore seeds cursors after retained
receipts. Repeated display frames cannot replay them. Player/world changes, tick
rewind, Game Over, and return to loadout reset the presentation and its baselines.

One integer-tick clock owns rise, fade, insertion pressure, and active Gold merging.
Visibility can hide pixels while cursors and clocks continue to follow accepted
state. Text is centered in the gameplay viewport, clipped below native Y 50, with
baseline `50 + 17 + round(offset)` and a two-unit downward shadow. Glyph scale and
viewport/UI scale retain their separate roles. Boneyard notices sit below its HUD;
College book notices have their own application layer.

The rank-book modal and floating text independently consume the accepted economy
receipt. A rank award closes Inventory and holds input/pause until OKAY or Escape.
A choice book opens its offer; a rank outcome without an eligible skill does not
open a confirmation. Stale, rejected, duplicate, and restored receipts do not reopen
the result. Session/world changes and terminal transitions retire its sound and UI.

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
| Splash and resource failure | `runtime_ui.rs`, separate loader crate | `startup.cjs`, `image-load-failure.cjs` |
| Title, Create, Pause, settings | `menu/` | `menus.cjs`, `menu-input.cjs`, `match-loading.cjs`, `settings-parity.cjs` |
| Desktop launch settings | `desktop/` | Packaged desktop journey |
| HUD, belt, skill selection | `hud/`, input and picker owners | `hud-input.cjs`, `gameplay-input.cjs`, belt/input unit tests |
| Inventory, Skillbook, services | `player_panels/` | `player-panels.cjs`, `hub-services.cjs` |
| Dialogue, questions, selectors | Dialogue and panel controllers | Interaction fixtures and GPU navigation tests |
| Party, chat, player cards | `social/` | `hosted-social.cjs` |
| Cloud content and sharing | `menu/cloud/` | `dark-cloud.cjs`, `mobile-sharing.cjs`, `routing-content.cjs` |
| World notices and book feedback | Notification and feedback owners | Notification and feedback lifecycle tests |
| Save/Continue and conflicts | Storage, menu, and session owners | `save-resume.cjs`, `saved-game.cjs`, packaged desktop journey |
| Viewport and touch | Shared layout and input owners | `responsive.cjs`, `hud-input.cjs` |

Additional composed journeys cover complete Tutorial progression and callouts,
controller-only dialogue and selector traversal, skill-book receipt dismissal and
reload, mobile-layout editing, authored shops/skills/minimap actions, observer
capabilities, shared pause, spectator behavior, and deployment recovery. A passing
foundation probe or title journey does not establish those feature boundaries.

The original C++ game supplies retail artwork, layout evidence, and recovered
interaction rules. Regression recordings retain their actual provenance;
[Reference data](REFERENCE-DATA.md) explains that boundary.

## Validation

Source checks verify GPU ownership and resource paths. Browser journeys use
ordinary input and read-only geometry/state observations. Review screenshots and
collect page errors, console errors, failed requests, and cleanup results.

Use full painted glyph/card bounds for layout assertions and clipped hit bounds
for pointer targeting. Clipping must not make an overflowing title appear to fit.
Wait for the actual accepted model and released loading scope before interacting;
a background widget record alone does not prove that a control is exposed.

Expected resource cancellation needs evidence of the owning navigation, disposal,
or media suspension. A suspended resident song can resume fetching after trusted
playback; the observer requires the same loaded owner and source, later native
playback, retained future data and no media error. A truncated response or
content-length mismatch remains a failure. Mark controlled prerequisites, such as
Console-granted funds or imported inventory, separately from naturally earned
outcomes. Browser IME emulation and read-only announcement observation also retain
their actual scope.

Run cold/warm startup, delayed and failed required resources, resize, input-device
changes, modal replacement, save/Continue, and the affected feature's failure paths.
For desktop changes, inspect and exercise each affected final package.
[Release](RELEASE.md) defines publication and live acceptance separately.
