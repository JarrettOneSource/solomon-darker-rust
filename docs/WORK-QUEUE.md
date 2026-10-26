# Open work

Use current Rust main as the base. The game builds from tracked inputs; contributor
instructions are in [AGENTS.md](../AGENTS.md).

## Native desktop

A separate native-window implementation is in progress on
`feat/native-desktop-20261026`. It replaces the desktop shell while sharing Rust
gameplay and GPU UI with the browser. It has not replaced the released desktop app.

Before release it needs complete platform packaging, save/import compatibility,
peer-play, audio, lifecycle failure, and update checks. Its branch owns the detailed
implementation plan. Preserve that task's checkout and evidence.

## Performance

Profile title and idle College browser cost, small-scene display copies, and College
snapshot volume. Measure four-player, constrained-network, mobile, high-refresh,
and long-session workloads. [Performance](PERFORMANCE.md) defines the measured scope.

Preserve actor/RNG order and require a measured improvement before changing entity
storage. Current storage uses typed contiguous collections.
