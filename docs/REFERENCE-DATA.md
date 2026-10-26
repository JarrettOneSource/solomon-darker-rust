# Reference data

The original C++ Solomon Dark 0.72.5 game is the retail reference. Its recovered
rules, assets, native recordings, and binary evidence support continued work on
the Rust implementation.

## Tracked inputs

Builds and regression tests use the assets, catalogs, fixtures, lockfiles, and
toolchain configuration in this repository. A normal build does not need a sibling
game checkout or a reference server.

- `web/assets/stock/`: sprites, fonts, audio, and other game resources.
- `web/assets/stock-files.json`: reviewed file membership and SHA256 values.
- `web/assets/boneyards.json` and `web/assets/navigation/`: retained scene geometry
  and prepared navigation data.
- `crates/sim/tests/fixtures/native/`: recordings from the original executable.
- Fixture directories named `website/`: frozen regression recordings.
- `docs/third-party/` and asset license files: notices that remain part of distribution.

Run `python3 scripts/verify-client-assets.py` before packaging. Asset updates are
reviewed source changes; builds must not download or silently regenerate them.

## Recording provenance

Some regression fixtures were captured through the retired PixiJS implementation.
Their source revisions, generator fields, directory names, and expected values
remain intact so the evidence is attributable. They are frozen data, not a
development target or a requirement to maintain that implementation.

Historical capture and comparison tools are unmaintained. Their source requirements
do not apply to the Rust build or required test workflow.

For new retail behavior, capture evidence from the original C++ game and test the
Rust implementation against it. For Rust features and bug fixes, add focused
regressions against the supported game contract. Do not relabel an intermediary
recording as a direct C++ capture or regenerate expectations from the Rust code
being tested.

Compressed fixtures retain their JSON checksum where supplied. Preserve the complete
recording and verify its decoded bytes; selecting a smaller sample changes coverage.
