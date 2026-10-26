# Open work

## Native desktop

The current source builds a native Rust window, platform services, durable saves
and encrypted peers while sharing Game and GPU UI with the browser. Electron and
its Node peer runtime have been removed from desktop packaging. The published
release has not yet changed.

Remaining work is final four-platform package execution, update and Console/mod
journeys, browser regression acceptance, review and publication/cleanup. The
[qualification checklist](NATIVE-DESKTOP.md) distinguishes established behavior
from unfinished release checks.

## Performance

Profile title and idle College browser cost, small-scene display copies, and College
snapshot volume. Measure four-player, constrained-network, mobile, high-refresh,
and long-session workloads. [Performance](PERFORMANCE.md) defines the measured scope.
