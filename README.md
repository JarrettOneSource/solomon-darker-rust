# Solomon Darker

Solomon Darker is a wizard survival game written in Rust, playable in the browser
and on Windows, Linux, and macOS.

[Play in your browser](https://solomondarker.com/game/) ·
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
2. Select **Game → Copy multiplayer invite**.
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
- [Trunk](https://trunkrs.dev), version 0.22.0.
- Python 3.11+ for asset checks and packaging.
- Node.js 22.6+ and npm for development tools and desktop packaging.

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

The output is in `web/dist/`.

### Desktop packaging

From the repository root:

```sh
npm run package:desktop -- --release-repository JarrettOneSource/solomon-darker-rust
npm run archive:desktop
```

See [desktop development](desktop/README.md) for platform options and prebuilt
inputs, and [Desktop runtime](docs/DESKTOP-RUNTIME.md) for package contracts.

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
module, and a manifest of file hashes.

The consumer verifies complete payload membership and hashes before installation.
Mounts are build inputs: rebuilding with `/game-stg` for both prefixes creates a
staging artifact. `--profile debug` selects an unoptimized development build.

## Repository layout

| Path | Contents |
| --- | --- |
| `crates/sim` | Gameplay, movement, spells, enemies, inventory, and session state |
| `crates/assets` | Sprite bundles, map data, and save-document codec |
| `crates/graphics` | Shared GPU renderer and asset decoding |
| `crates/web` | WebAssembly client, input, audio, menus, and HUD |
| `crates/loader` | WebAssembly loading screen |
| `crates/protocol` | Messages between the client and host |
| `crates/launcher` | Native host, asset server, and durable saves |
| `crates/modding` | Mod validation and authoring tools |
| `crates/save-tools` | Save import, export, and validation |
| `web` | Browser entry point and game assets |
| `desktop` | Desktop shell and packaging |
| `tools` | Authoring tools, fixture generators, and browser checks |

## Testing

Run from the repository root:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets
cargo clippy -p solomon-web --target wasm32-unknown-unknown
cargo test --workspace
npm run test:browser-support
npm run test:desktop
```

The simulation tests replay recorded fixtures. Browser checks exercise the running
client, and desktop checks cover the shell and packaging.
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
