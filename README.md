# Solomon Darker

Solomon Darker is a wizard survival game written in Rust, playable in the browser
and on Windows, Linux, and macOS.

[Play in your browser](https://solomondarker.com/game-rust/) ·
[Download the desktop app](https://github.com/JarrettOneSource/solomon-darker-rust/releases/latest)

Explore the College, build your wizard's loadout, and fight through the Boneyard.
The game includes five elements, ten welded spell combinations, equipment, loot,
a tutorial, and multiplayer. Keyboard, mouse, gamepad, and touch input are supported.

## Desktop app

Download the ZIP for your platform, extract it, and run the app. Keep the extracted
folder together. Builds are available for Windows x64, Linux x64, Intel Macs, and
Apple silicon Macs.

The desktop app supports offline solo play, local saves, and encrypted direct
multiplayer. Saves live in the app's user-data folder, separate from the download.
To update, save and close the game, then replace the app folder with the new version.

To play with friends:

1. Choose **Host game** and start a game.
2. Press Escape and choose **Copy multiplayer invite**.
3. Have your friends choose **Join a friend** and paste the invite.

Everyone needs the same build. The host must stay running and have a reachable IP
or VPN address with TCP port 27843 allowed or forwarded.

The builds are unsigned and not notarized, so Windows or macOS may ask for
confirmation on first launch. Cloud accounts and community content are available
in the browser version.

## Build from source

### Requirements

- Rust 1.98.1 with Clippy, rustfmt, and the `wasm32-unknown-unknown` target.
  The repository's `rust-toolchain.toml` selects these.
- [Trunk](https://trunkrs.dev), version 0.22.0, for the browser build.
- Python 3.11+ for asset checks and packaging.
- Node.js 22.6+ and npm for development tools and desktop packaging.
- Native builds also need a platform linker, CMake, libclang and Perl. Linux
  needs ALSA, udev and Fontconfig development packages; see [Desktop runtime](docs/DESKTOP-RUNTIME.md).

```sh
npm ci
cargo install trunk --locked --version 0.22.0
python3 scripts/verify-client-assets.py
```

Game assets and test fixtures are included in the repository.

### Browser development

```sh
cd web
trunk serve --port 4181
```

Open `http://127.0.0.1:4181/`. This runs a local browser simulation.
Use WASD or the arrow keys to move.

To build the browser client for distribution:

```sh
cd web
trunk build --release
```

The output is in `web/dist/`. Its URLs are relative, so it can be served from the
site root or below any path.

### Desktop packaging

From the repository root:

```sh
npm run package:desktop -- --release-repository JarrettOneSource/solomon-darker-rust
npm run archive:desktop
```

See [Desktop runtime](docs/DESKTOP-RUNTIME.md) for platform options, prebuilt
inputs and package contracts.

### Native authoritative host

To serve a built client with the Rust host:

```sh
cargo run -p solomon-launcher --bin solomon-darker -- --assets web/dist
```

The host opens `http://127.0.0.1:4188/?host=1`, runs the shared simulation, and
stores saves on disk. Keep it running while playing; Ctrl+C saves and stops it.
On macOS, the default save is
`~/Library/Application Support/Solomon Darker/host-4188.json`.

Options include `--listen ADDRESS` for direct connections, `--port`, `--save FILE`,
and `--no-open`. The default listener is loopback.

To package the host and browser client together:

```sh
python3 scripts/package-game.py --output artifacts/solomon-darker
```

Run the packaged executable with its sibling `game/` directory intact; it needs
no development toolchain. The browser profile retains preferences and the resume
credential for that host origin. Packaging accepts `--profile debug`,
`--web-dist PATH` to reuse verified client output, and `--no-archive` for a directory.

### Website integration

This repository owns the game, Rust host, WebAssembly client, save tools, and
desktop app. The website provides accounts, cloud storage, community content, and editing.

Build an artifact for the website with:

```sh
python3 scripts/package-hosted-game.py --output artifacts/hosted-game \
  --mount /game --website-mount ''
```

The directory and its `.tar.gz` include the host, client, editor entry, save-tools
module, the [cloud save checker](#checking-stored-cloud-saves), and a manifest of file
hashes.

The consumer verifies complete payload membership and hashes before installation.
`--profile debug` selects an unoptimized development build.

Every URL in the client is relative to its page, so the same files serve at any
mount, such as `/game`, `/game-rust` or `/game-stg`. Serve each page at its
directory URL with the trailing slash (`/game/`, `/game/editor-test/`). The mounts
are deployment settings in the pages' metadata: `--mount` records where the game is
served (`solomon-game-mount`), and `--website-mount` sets the prefix of the Website
API, game socket and browser storage (`solomon-website-mount`). A staging artifact
uses `/game-stg` for both. Serving an artifact at another mount changes only that
metadata.

The player's own browser data (saves, settings, Hall of Fame, playtime, layout) is
stored under keys suffixed with the Website mount, so `/game-rust` and `/game-stg`
keep separate data on one origin. A deployment can pin that scope with a
`<meta name="solomon-storage-namespace" content="/game-rust">` tag, which
`prepare-hosted-client.py --storage-namespace` writes. The game then keeps the same
data when it moves to another path. The sign-in token and Boneyard workshop keys
stay on the Website mount, because the Website's own pages there share them.

### Checking stored cloud saves

`solomon-save-check` measures how many of the Website's stored cloud saves the Rust game
loads. For each save it parses the document, restores it as the host admits Continue
(a profile-only save is hydrated into a New Game wizard), loads a restored Boneyard's
navigation, writes the document as the host's next checkpoint would, and loads that
rewrite again. `solomon-save-rows.py`, a standard-library Python 3 script, opens the
Website's SQLite database read-only and streams each `WebGameSaves` row's slot, revision
and document, without user IDs. From an unpacked hosted archive:

```sh
python3 bin/solomon-save-rows.py /path/to/sdr.db | bin/solomon-save-check --assets client > summary.json
```

From source, build the checker with `cargo build -p solomon-launcher --bin solomon-save-check`,
then use `python3 scripts/solomon-save-rows.py` and `--assets web`. `--assets` names the
client directory the host serves, which holds `assets/navigation`.

The summary counts rows by slot (0 is PixiJS, 1 is Rust), schema version and outcome:
`ok`, `parseError`, `restoreError`, `roundTripError`, `oversize` or `panic`. It lists
each distinct error with the slot, revision and SHA-256 of its rows;
`solomon-save-rows.py DATABASE --find-sha256 HASH` finds such a row again. The checker
exits 0 whenever it has read all of its input.

The summary contains no document text. Messages that can repeat a document's keys,
values or names are redacted (the list is in
`crates/launcher/src/bin/solomon-save-check/redact.rs`), numbers become `#`, and a
message that still contains a name, identity or long string from its document is
withheld. Session policies are not applied: each save loads as the first wizard of a
private College whose mods match the save, so no mod packages are needed. The exporter
keeps one read transaction open while it streams, so give it a copy of a live database,
such as one made with `sqlite3 sdr.db ".backup copy.db"`.

## Repository layout

| Path | Contents |
| --- | --- |
| `crates/sim` | Gameplay, movement, spells, enemies, inventory, and session state |
| `crates/assets` | Sprite bundles, map data, and save-document codec |
| `crates/graphics` | Shared GPU renderer and asset decoding |
| `crates/client` | Shared Rust Game, input, audio, menus, HUD, and native/browser adapters |
| `crates/loader` | WebAssembly loading screen |
| `crates/protocol` | Messages between the client and host |
| `crates/launcher` | Native host, asset server, and durable saves |
| `crates/modding` | Mod validation and authoring tools |
| `crates/content-sweep` | Checks that published mods and Boneyards load |
| `crates/save-tools` | Save import, export, and validation |
| `web` | Browser entry point and game assets |
| `crates/desktop` | Native window, session lifetime, encrypted peers, and updates |
| `crates/storage` | Locked atomic native documents |
| `tools` | Authoring tools, fixture generators, and browser checks |

## Testing

Run from the repository root:

```sh
python3 scripts/check-port-references.py
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo clippy -p solomon-client --target wasm32-unknown-unknown -- -D warnings
cargo clippy -p solomon-loader --target wasm32-unknown-unknown -- -D warnings
cargo test --workspace --all-features
npm run test:browser-support
npm run test:desktop
```

The simulation tests replay recorded fixtures. Browser checks exercise the running
client, and desktop checks cover native platform services and packaging. The
packaged journey additionally launches the actual executable through ordinary OS input.
The [full game gate](AGENTS.md#checks) adds strict all-feature linting, the separate
loader target, asset verification, and feature-specific checks.

### Browser smoke

With your development server running on port 4181:

```sh
PW="$PWD/node_modules/playwright-core" URL=http://127.0.0.1:4181/ \
  node tools/browser-smoke/smoke.cjs
```

This check requires Google Chrome. It creates a wizard, checks movement and
rendering, and captures screenshots.

Use `CHROME` to select the browser and `OUT` to retain the journey's screenshots
and receipt. [The UI surface map](docs/GPU-UI.md#surface-map) lists focused journeys
for menus, panels, input, social behavior, and saved continuation.

The shared test-browser launcher disables Chromium's Windows TCP port
randomization to avoid the observed Windows socket error 10055. It preserves the
other disabled features from pinned Playwright 1.61.1. Windows CI compares the
effective launch arguments against a real default launch before running the
journeys; review this list when upgrading Playwright. This changes the acceptance
browser only. Network and asset failures still fail the journey.

For the native multiplayer journey, supply either a complete `PACKAGE` directory
or the executable and client assets separately:

```sh
PW="$PWD/node_modules/playwright-core" \
BINARY=/absolute/path/to/solomon-darker ASSETS=/absolute/path/to/wasm-client \
OUT=/absolute/path/to/native-session-report \
  node tools/browser-smoke/native-session.cjs
```

This drives two browser contexts through shared play, saving, rejoining, and host
restart. It uses an isolated save and runs the host outside the source directory
to check that the package supplies its own runtime inputs.

Add `GAMEOVER=1` to exercise the entrance sequence, combat death, and post-run
return. These checks use a read-only Rust helper to plan keyboard movement:

```sh
cargo build -p solomon-assets --example browser_navigation
npm run test:browser-navigation
```

The helper reads this repository's tracked scenes and navigation meshes. Set
`SOLOMON_NAVIGATION_BINARY` when using a build outside `target/debug/examples`.
It supplies route predictions to the test driver; only ordinary keyboard events
reach the game.

### Published community content

`solomon-content-sweep` checks that the mods and Boneyards published on the Website load
in the Rust game. For each mod package it builds the session content the Website provisions
for a player who enabled that mod, then runs it through the hosted authority: content
admission, Lua definition compilation, the private College's asset, content catalog and Lua
runtime preparation, and ten seconds of play in every Boneyard the mod offers. A Lua mod
plays the stock Random Boneyard, and each Boneyard a package provides is played too. Each
tick runs the mods' controls, actors and rules in the private College's order.

```sh
cargo run -p solomon-content-sweep -- --assets web --packages PACKAGES
cargo run -p solomon-content-sweep -- --assets web --packages PACKAGES \
  --api https://solomondarker.com/game-rust/
```

`--packages` is searched recursively for `.zip` and `.sdmod` files; the Website stores each
published version as `uploads/mods/SLUG/VERSION.zip`. With `--api`, the sweep reads the
Website's public mod listing with anonymous `GET` requests, one at a time and `--delay-ms`
apart (500 by default). It sweeps the version each listed mod loads, its newest version with
a package, matched to a file by the published package SHA-256 and checked against the
published content digest. The Website offers no anonymous package download, so a listed mod
without a local package is reported as unavailable, and local packages that no listed mod
loads are reported as ignored. Unlisted and private mods are not in the listing. `--assets`
names the client directory the host serves, which holds `assets/navigation` for the stock
Boneyards, and `--ticks` sets the ticks played in each Boneyard (100 a second, 1,000 by
default).

The JSON summary on stdout counts `ok`, `failed` and `unavailable` items by kind: `boneyard`
for what the Boneyard editor publishes and `mod` for everything else. Each item lists its
package id, slug, version, package SHA-256 and content digest. A failure adds the stage that
failed (`package`, `identity`, `admission`, `definitions`, `preparation`, `boneyard` or
`session`) and the game's error; a mod rule that fails while playing is a failure. The
summary contains no package contents. The exit status is 0 when everything loaded, 1 when an
item failed or was unavailable, and 2 when the sweep could not run.
`crates/content-sweep/tests/fixture_packages.rs` builds packages from this repository's mod
fixtures and runs both modes, the listing against a local server. The sweep does not
exercise the client's presentation of a mod: decoding its images and audio, or compiling a
Boneyard's scenery view.

## Documentation

- [Architecture](docs/ARCHITECTURE.md)
- [Coding standards](docs/CODING-STANDARDS.md) and [agent instructions](AGENTS.md)
- [Game glossary](CONTEXT.md)
- [Design decisions](docs/DECISIONS.md)
- [Implementation and test coverage](docs/IMPLEMENTATION.md)
- [GPU interface](docs/GPU-UI.md)
- [Performance measurements](docs/PERFORMANCE.md)
- [Release and deployment checks](docs/RELEASE.md)
- [Reference data](docs/REFERENCE-DATA.md)
- [Mod authoring CLI](tools/sdmod/README.md)
- [Work queue](docs/WORK-QUEUE.md)
