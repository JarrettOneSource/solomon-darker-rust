# Mod authoring CLI

This is the current Website `tools/sdmod` frontend, using the canonical Rust
definition compiler and Lua runtime. It imports no TypeScript game modules or
Wasmoon. The existing ZIP, asset-download, Blender and reference-generation code
is retained; the native owner supplies checked packages and compiler metadata.

Build the native tool once, then put `solomon-sdmod` on `PATH` or set its explicit
location. The frontend never invokes Cargo or starts a game host:

```sh
cargo build -p solomon-modding --bin solomon-sdmod
export SOLOMON_SDMOD_BINARY=/path/to/solomon-sdmod
node tools/sdmod/cli.mjs new /path/to/my-mod
node tools/sdmod/cli.mjs check /path/to/my-mod
```

The eight public commands remain `new`, `check`, `test`, `pack`, `migrate`,
`generate`, `assets fetch` and `render-sprite`. Package tests run real Lua against
the canonical compiled graph with the Source closed globals. Packed includes
are recompiled without loose scripts and must retain their graph digest. The
retained ZIP writer preserves Source file order, timestamps and bytes.

`generate <directory> --check` compares all five reference files. Their inventory
comes directly from the Rust compiler's content schemas, constructor names,
art slots, events, scopes and diagnostics. This preserves the Website lint
consumer without keeping another definition compiler.

Focused checks:

```sh
cargo test -p solomon-modding --test authoring_goldens
SOLOMON_SDMOD_BINARY=/path/to/solomon-sdmod node --test tools/sdmod/cli.test.mjs
```

The native fixture records 29 unchanged current-Source cases. The frontend tests
exercise creation/check/test/migration, independent packed includes, exact five-file
reference output, deterministic archives and the current Source starter ZIP hash.
Website cutover must also run its existing `generate public/web-lua --check` and
the four `examples/web-lua` checks through this entry. Those packages remain
Website control-plane content; no example asset bank is copied here.

`starter-icon.png` is the existing Source Kenney Game Icons CC0 image; `new` writes
the same attribution. `asset-sources.json` and the art helpers retain the Source
download hashes/licenses. Asset fetching and Blender execution occur only when
their corresponding commands are requested.
