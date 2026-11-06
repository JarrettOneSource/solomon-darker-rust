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

## Website save reward identifiers

Website-format saves represent Bonus rewards with numeric IDs: 0 is a skill
choice, 1 is a skill rank and 2 is quadruple damage. The asset codec reads and
writes those IDs for every `BonusKind` field. Earlier Rust variant and kebab
names remain accepted on input and export as the same numeric IDs. Negative,
out-of-range and fractional IDs are rejected. Native checkpoint enum encoding
and the reward rules remain unchanged. Public codec regressions use synthetic
loot stores and cover all three rewards across source schemas 50, 51 and 52;
private player saves remain outside the repository.

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

The original `weld-actor-goldens.json` and `weld-step-goldens.json` bytes remain
unchanged after the report133 retail correction. Their weak Steam contact
permission and subsequent contact clock came from the earlier incorrect
implementation. Only those fields are superseded in the named weak cases;
`native_steam_low_mana` and `spell_contact_rules` independently assert the recovered
normal/weak/Over contact behavior. Every other recorded field, random word and
presentation sample remains checked.

The schema-51 `current-save-continuation-goldens` recording also remains intact.
Its comparison expects the current export schema and the seven schema-52 Boulder
guidance fields. The four recorded rocks retain all motion and ownership values:
held actors use the recovered release speed, while existing flight keeps the
recorded velocity magnitude and heading. Missing target and turn history receive
the documented migration defaults. Current checkpoint and Website round-trip
regressions independently verify non-default guidance state and required fields.

The profile, native-save conversion and desktop-authority recordings likewise
retain their schema-51 bytes. Comparisons qualify only newly exported game
documents as the current schema. Source-version admission, native disk bytes,
portable-profile bytes and all other recorded fields retain their exact checks.

The two player-contact recordings retain the earlier omission of defensive-effect
painter registrations. Their comparison supersedes only those fields in the named
Flash, broken Magic Shield and physical Stoneskin cases: each recorded child owns
one transient registration in birth order, and the allocator advances accordingly.
The complete child membership and order are asserted before this expected-only
projection. Existing actors, light ownership, gameplay, presentation, random state
and all other recorded fields remain exact. The contact, poison and Shield
regressions in `current_ether_drain_session_goldens`, plus the physical contact case
in `current_stoneskin_goldens`, independently cover the producer contract, including
callbacks after the secondary actor pass.
