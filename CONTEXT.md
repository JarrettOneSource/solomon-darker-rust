# Glossary

The words this repository uses, in the sense it uses them. Code, tests, fixtures and tickets
use these terms exactly; when a new concept earns a name, add it here.

## Sources of truth

- **Native** (or **retail**): Solomon Dark 0.72.5, the Windows game. The ground truth for every
  behaviour. Its decompilation lives with the workspace.
- **Website**: the web port whose `fa08ae249ff5563570bdb2a653e2ef9358ce7b83` revision
  defines the rebuild's accepted behavior. Historical recordings retain their actual source
  revision. Accounts, cloud content and editing remain Website responsibilities.
- **Native goldens**: language-neutral JSON captured from the instrumented retail
  executable, committed under `crates/sim/tests/fixtures/native/`; each file carries its
  native contract in its header. The old Mod Loader that first held them is retired.
- **Kernel**: a pure function or state type in `solomon-sim` that reproduces one game behaviour
  from a cited reference.
- **Oracle**: a Node script under `tools/website-oracle/` that imports unmodified Website
  kernels and records what they produce.
- **Fixture** (or **golden**): recorded reference output as JSON. Website fixtures live in
  `crates/sim/tests/fixtures/website/` and native goldens in `crates/sim/tests/fixtures/native/`.
- **Differential test**: a test in `crates/sim/tests/website_goldens.rs` that replays a fixture
  through the port and requires bit-for-bit equality.
- **Smoke**: the headless-Chrome check of the served client (`tools/browser-smoke/smoke.cjs`).

## Time and numbers

- **Tick**: one 10 ms simulation step; the game runs at 100 Hz. Tick counters are `u64`.
- **Frame**: one browser draw, independent of the authoritative simulation tick.
- **Accepted session**: the completed game state admitted from the native host, including
  participants, world, inventories and random streams.
- **Display frame**: the presentation of accepted sessions at the current draw time; its
  positions and animation phases can lie between authoritative ticks.
- **Presentation tick**: the fractional tick of a display frame, bounded by the newest
  received state. It is not another gameplay step.
- **Hub prediction**: the local wizard's displayed movement for at most one snapshot
  interval, reconciled to the next accepted session. Boneyard displays interpolate instead.
- **fround** / **single-precision point**: narrowing a value to `f32` and back, where the
  reference (`Math.fround` or the native 32-bit store) does the same. The port rounds at those
  points and nowhere else.
- **Lane cap**: 118.75 world units per second, the largest per-axis speed. A **scripted speed**
  is a multiplier on it.
- **Native RNG**: the 55-word lagged-Fibonacci generator (`NativeRng`), 30-bit words under
  `MASK`, float divisor 100000.
- **Semantic word**: one native RNG word used as an opaque input to a **random domain**
  reduction: an integer below a bound or a float up to a maximum.
- **Collision fallback sampler**: the xorshift `u32` the courtyard uses when a sweep cannot
  resolve; it advances only on fallback, which the fixtures count.

## Space and movement

- **World units**: the courtyard is 2000 × 1024; positions are `f64`.
- **Heading index** / **facing**: one of 24 sprite facings 15° apart, index 0 pointing north,
  increasing clockwise.
- **Gait** and **walk cycle**: the wizard's pose phases, advanced by distance travelled.
- **Segment**: a line obstacle; collision is a circle swept against segments. The courtyard
  has 129.
- **Movement plan** (`HubWizardPlan`): what one tick wants to do (delta, requested velocity,
  retained velocity, whether movement is active) before collision resolves it.

## The Hub

- **Hub**: the College. Its **regions** are the **courtyard** and four **private rooms**:
  mortuary, library, storeroom, office.
- **Portal** (or **doorway**): a trigger segment in a source region with a destination, a
  scripted target and speed, and whether the contact x is preserved.
- **Transition**: the walk through a doorway in phases: outgoing fade, **placement** (where
  the wizard appears and the walk in), incoming fade. The College intro has its own fade rate.
- **Scripted movement**: a walk toward a target with no input and nothing retained between
  ticks.

## Painting

- **Painter**: the stock draw-order queue for world actors.
- **Registration ordinal**: the index at which an actor registered with the world manager.
  The 48 **fixed actors** in `HUB_FIXED_PAINTER_IDS` (the traders, the teacher, the
  obstacles, the statue, the room paintings and props, and the named College staff) reserve
  ordinals 0..47; the first player takes 48.
- **Painter row**: two-unit rows measured from the reference actor's feet; ties break by
  ordinal.
- **Sort bias**: a nudge to an actor's row; the player gets ±20 inside the north arch.
- **Transient lane**: painters an effect registers for a while, such as the teacher's burst
  column and frame strip.
- **Depth band**: the fixed drawing band of a non-actor layer (seal glyphs, seal core,
  fountain spray, foreground) relative to the actors at 1000 + row.
- **Obstacle**: one of eight fixed courtyard presences with a position and radius that sort
  among the actors.

## Ambient life

- **Ambient state**: the courtyard's self-running life, advanced once per tick: fountain
  spray, seal pulse, statue bob and the teacher clock.
- **Drop**: one fountain spray particle with an id, remaining lifetime and scale.
- **Seal**: the courtyard's additive floor emblem: thirteen **glyph** records under one
  **core** record, each tinted along a three-colour **track** by its **phase**.
- **Statue bob**: the College statue's vertical offset and aura drift from its phase in
  degrees.
- **Teacher cycle**: 847 ticks of cast (0..267), release (268..346) and idle. The **burst** is
  the release's effect with four members (core, flare, column, frames) that fade by native
  decay; the **release index** counts cycles.

## Art and rendering

- **Bundle** / **record**: a stock `.bundle` sprite record: atlas rectangle, logical frame,
  registration centre, collision points.
- **Registered origin**: where a record draws inside the room's logical frame; the seal draws
  at scale 2 from a rounded registered origin.
- **Atlas**: the stock PNG a bundle indexes (`College.bundle` for the courtyard).
- **Camera origin** / **logical view**: the 1600 × 900 logical view at scale 1.2, clamped to
  the world and letterboxed into the canvas.
- **Blend mode**: Normal, Multiply (the statue aura) or Add (seals and spray), on
  premultiplied output.
- **Tint**: a colour multiplier quantised to 8 bits per channel, with alpha separate, the way
  Pixi applies it.
- **Starter tints**: the hat and robe colours rolled from the owner's base colour with the
  native RNG.

## Boneyard loot

- **Ground loot actor**: an Orb (2011), Gold (2012), Sack (2013) or Bonus (2038) on the
  Boneyard floor, with its own pickup timing and world-manager registration.
- **Sack**: a ground carrier for an inventory item. An inventory Sack (7008) is a container
  item with addressed child items; picking up a ground Sack can therefore carry a nested tree.
- **Goodie**: a locked scenery chest (2061). An explicit nearest-facing interaction consumes
  a Wizard Key; opening advances through phases at 100/200 ticks and yields contents at 250.
- **Loot stream**: the ground store's shared native RNG, used by constructors, enemy loot
  seed writes, pickups and Goodie fragments. An enemy's separate private stream chooses its
  candidate category; constructing that candidate advances the shared stream.

## Tutorial

- **Stage**: one of twenty stock Tutorial progression states. The numeric IDs preserve the
  authored script; named variants describe the instruction or condition at that stage.
- **Narration**: ordered Sirmin/Solomon cues with PCM-derived durations at 100 Hz. Playback
  consumes cue events; it does not advance the Tutorial clock.
- **Camera lock**: the Tutorial's checked transition from the full scene to its upper combat
  rectangle. Enemy bodies and ground Sacks must remain safe before its cleanup starts.

## Operations

- **Social snapshot**: an immutable host projection of actual Party membership, durable
  roster rows, accepted chat messages and player cards. It does not own gameplay state.
- **Directory capability**: whether an actual broker can resolve party codes, invitations
  and listings. A direct native host has no directory; its roster and chat still work.

- **Trunk**: the WebAssembly bundler and dev server; `web/` is its root.
- **Launchd agent**: `com.jarrett.solomon-rust-game`, the M2 service that serves the release
  build on loopback port 4180, published to the tailnet by `tailscale serve`.
