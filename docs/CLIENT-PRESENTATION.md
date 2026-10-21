# Native client presentation

The accepted `GameSession` remains the source for commands, UI, sound, saves and checkpoints.
`web::presentation::NativePresentationTimeline` owns only received display copies. It ports
`Website/frontend/src/game/client/{hub,boneyard}-presentation-timeline.ts` and the bounded
local College movement in `client/game-client-session.ts` / `client/hub-prediction.ts`.

Initialize the timeline with the validated Welcome tick and snapshot rates. After applying
an accepted snapshot, call `push(&session, received_at_ms)` with the browser callback's
original `performance.now()` receipt time, including snapshots retained during world loading.
Call `reset()` on admission/disconnect. A world-kind or Boneyard run-ID change also clears it.

Sample once per animation frame, after selecting raw movement and before converting aim
through the camera:

```rust
let display = timeline.sample(now_ms, local_player_id, input, gameplay_held);
```

The returned `PresentationFrame` owns a display-only `Box<GameSession>`, its fractional `tick`,
`PresentationClocks`, and optional `EnemyPresentationMetadata`. Camera, input-to-world aim,
players, world geometry, painter order and lights must borrow this same display frame. Never
step, restore, acknowledge or save that copy. Local UI blocking supplies zero movement;
`gameplay_held` means the actual shared pause/resume grace or loading hold. Level-up barriers
are already read by the timeline.

The history holds at most eight snapshots and never extrapolates a Boneyard. Duplicate ticks
replace data without restarting elapsed time. New/removed actors and discrete IDs follow the
source endpoint rules; fixed-lifetime primary effects use their reconstructed birth times.
The College additionally predicts only its local movement for one snapshot interval and
reconciles with the source correction fade. This calls existing movement/collision kernels on
one private character; it never runs a second session or advances combat/gameplay RNG.

Simulation ticks remain integers. Fractional primary/secondary ages, player damage-four timers
and Puppet hit alpha are carried in `PresentationClocks`. Enemy endpoint animation is sampled
before interpolation (including Spider target/light appearance), so articulated bones and
wrapped phases blend exactly as the source protocol does. Draw plans receive those sampled
fields rather than resampling an interpolated brain.

`BattleFrame` supplies `presentation_tick`, `clocks` and `enemy_presentation`. `SpellView::prepare`
accepts the clocks; `SecondaryView::prepare` accepts its secondary-age map. Direct snapshot
and isolated fixture callers pass `None`. This selects their actual whole actor clocks and
uses the same draw-plan implementation. Player orb, Enchant Staff and status effects use the
fractional display tick. Primary/secondary cosmetic per-draw noise keeps the source retained
frame counter, separate from display time; a skill-picker barrier selects the frozen world
clock instead. Tree occlusion advances at draw time to `floor(presentation_tick)` using the
displayed local position. Weather also explicitly samples whole native epochs.

The focused source fixtures are `presentation-timeline-goldens.json`,
`enemy-samples-goldens.json` and `secondary-fractional-presentation-goldens.json`. The actual renderer cadence
receipt lives with the fleet evidence, not in gameplay state.
