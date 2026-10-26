# Client presentation

Accepted `GameSession` state owns commands, UI decisions, sound, saves, and
checkpoints. `web::presentation::NativePresentationTimeline` owns display copies
sampled from received snapshots.

## Lifecycle

Initialize with the validated Welcome tick and snapshot rates. After applying an
accepted snapshot, call `push(&session, received_at_ms)` with the callback's original
`performance.now()` receipt time, including snapshots held during world loading.

Reset on admission or disconnect. A world-kind or Boneyard run-ID change also clears
history. Duplicate ticks replace data without restarting elapsed time. History holds
at most eight snapshots.

## Sampling

Sample once per frame after selecting raw movement and before projecting aim:

```rust
let display = timeline.sample(now_ms, local_player_id, input, gameplay_held);
```

The returned `PresentationFrame` contains a display-only session, fractional tick,
`PresentationClocks`, and optional `EnemyPresentationMetadata`. Camera, aim,
players, geometry, painter order, and lights borrow that same frame.

Local UI blocking supplies zero movement. `gameplay_held` represents shared
pause/resume grace or a loading hold; the timeline reads level-up barriers itself.
Display state cannot be stepped as a session, acknowledged, restored, or saved.

## Interpolation and prediction

Boneyards interpolate without extrapolation. New and removed actors follow endpoint
identity rules. Fixed-lifetime primary effects use reconstructed birth times.
The College predicts only local movement for one snapshot interval, using the
canonical movement/collision kernels on a private character.

Simulation ticks remain integers. Fractional effect ages, damage timers, and hit
alpha travel in presentation clocks. Enemy animation is sampled at endpoints
before interpolation so bones, wrapped phases, and appearance metadata retain
their correct lifetime.

`BattleFrame` supplies `presentation_tick`, `clocks`, and
`enemy_presentation`. Spell and secondary views consume those fields.
Direct snapshots and isolated fixtures use whole actor clocks through the same
draw-plan code.

Cosmetic per-draw noise retains its own frame counter. Skill-picker barriers use
the frozen world clock. Tree occlusion advances to whole presentation ticks using
the displayed local player; weather samples whole simulation epochs.

## Checks

The tracked fixtures include `presentation-timeline-goldens.json`,
`enemy-samples-goldens.json`, and
`secondary-fractional-presentation-goldens.json`.
GPU observations must also establish draw cadence between snapshots; simulation
state alone cannot demonstrate smooth rendering.
