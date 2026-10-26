# Mod authoring CLI

The CLI uses the Rust definition compiler and Lua runtime. It creates projects,
validates and tests packages, writes archives, and generates editor references.

## Setup

Build the native tool and put it on `PATH` or set its location:

```sh
cargo build -p solomon-modding --bin solomon-sdmod
export SOLOMON_SDMOD_BINARY=/absolute/path/to/solomon-sdmod
node tools/sdmod/cli.mjs new /path/to/my-mod
node tools/sdmod/cli.mjs check /path/to/my-mod
```

The frontend does not invoke Cargo or start a game host.

Commands are `new`, `check`, `test`, `pack`, `migrate`, `generate`,
`assets fetch`, and `render-sprite`. Package tests execute Lua against the
compiled definition graph and supported sandbox globals. Packed includes are
recompiled and must retain their graph digest. Archives use deterministic file
order, timestamps, and bytes.

`generate <directory> --check` compares all five reference files. Their schemas,
constructors, art slots, events, scopes, and diagnostics come from the Rust compiler.
The website editor consumes these references through its integration.

## Checks and asset sources

```sh
cargo test -p solomon-modding --test authoring_goldens
SOLOMON_SDMOD_BINARY=/absolute/path/to/solomon-sdmod node --test tools/sdmod/cli.test.mjs
```

Tests cover project creation, validation, Lua execution, migration, packed includes,
reference output, and deterministic archives.

`starter-icon.png` is a Kenney Game Icons CC0 asset; `new` writes its attribution.
`asset-sources.json` records download hashes and licenses.
Asset downloads and Blender run only for the corresponding requested command.
