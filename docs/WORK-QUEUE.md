# Open work

## Standalone cutover

Retiring the PixiJS client is tracked in [Standalone cutover](CUTOVER.md), which
holds the decisions, the hitlist and the checks that must pass before Rust moves
to `/game`.

## Native desktop

The current source builds a native Rust window, platform services, durable saves
and encrypted peers while sharing Game and GPU UI with the browser. Electron and
its Node peer runtime have been removed from the published native packages.

Qualification remains specific to each revision. Changes require the affected
four-platform package and browser journeys, review of any failures, and the
publication/installation/cleanup checks in [Release](RELEASE.md). The broader
performance, hardware coverage and long-session workloads below remain separate
from those release checks.

## Performance

The title/College baseline and sprite-uniform savings are measured in
[Performance](PERFORMANCE.md). Continue profiling the remaining browser hot paths,
small-scene display copies and College snapshot volume. Measure four-player,
constrained-network, mobile, high-refresh and long-session workloads.

## Player action presentation

The no-item HandCast1 owner now latches the recovered native float stores. Staff
and Wand retain their existing cast cadence. Audit their broader construction
rate precision separately, using fresh retail evidence and independent expected
values rather than replacing historical replay fixtures.
