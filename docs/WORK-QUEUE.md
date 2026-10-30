# Open work

## Native desktop

The current source builds a native Rust window, platform services, durable saves
and encrypted peers while sharing Game and GPU UI with the browser. Electron and
its Node peer runtime have been removed from the published native packages.

Qualification remains specific to each revision. Changes require the affected
four-platform package and browser journeys, review of any failures, and the
publication/installation/cleanup checks in [Release](RELEASE.md). The broader
native-completion review remains open; a successful release does not establish
unmeasured performance, hardware coverage or long-session reliability.

## Performance

Profile title and idle College browser cost, small-scene display copies, and College
snapshot volume. Measure four-player, constrained-network, mobile, high-refresh,
and long-session workloads. [Performance](PERFORMANCE.md) defines the measured scope.
