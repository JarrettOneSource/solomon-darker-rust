# Desktop shell

This is the current Website desktop shell extracted into the Rust repository.
Electron43.4.0 still owns its sandboxed window and preload, and the separately
pinned Node22.17.0 still owns the authenticated TLS1.3 PSK tunnel. The gameplay
child is the canonical Rust `solomon-darker --desktop` process. No TypeScript
game or Wasmoon bundle is staged.

See [the runtime contract](../docs/DESKTOP-RUNTIME.md) for the exact startup,
preload, save-before-close, shared credential and lifetime interfaces.

Install this repository's locked development tools once with `npm ci`. The public
commands are:

```sh
npm run dev:desktop
npm run test:desktop
npm run package:desktop:mac -- --release-repository OWNER/RUST_REPOSITORY
npm run package:desktop:windows -- --release-repository OWNER/RUST_REPOSITORY
npm run package:desktop:linux -- --release-repository OWNER/RUST_REPOSITORY
node tools/package-desktop.mjs --platform darwin --arch x64 --release-repository OWNER/RUST_REPOSITORY
npm run archive:desktop
npm run smoke:desktop
```

Development builds the native executable and WASM client sequentially. Packaging
builds release artifacts for the selected platform, retains the Source ad-hoc
macOS signing, and records the native executable SHA256 beside the pinned Node
runtime and revision. A cross-platform build still needs the corresponding Rust
linker/toolchain; per-platform execution is a separate acceptance gate.

For a shared or prebuilt artifact, avoid a second compiler:

```sh
npm run package:desktop -- --platform darwin --arch arm64 --skip-build \
  --native-host /absolute/solomon-darker --client-root /absolute/wasm-client \
  --release-repository OWNER/RUST_REPOSITORY
```

Development supports `--skip-build`, `SDR_DESKTOP_GAME_HOST` and
`SDR_DESKTOP_CLIENT_ROOT` for the same purpose. `SDR_DESKTOP_NODE` may select an
installed peer runtime during development; release packages always stage the
Source checksum-pinned Node runtime. Release packaging requires an explicit
repository (`--release-repository` or `SDR_DESKTOP_RELEASE_REPOSITORY`), preventing
a final Rust package from silently targeting Website releases. Updates retain
Source revision/timestamp/platform/artifact validation and never restart a game.

The game repository's validation workflow builds and runs this packaged journey on
Windows x64, macOS ARM64/x64 and Linux x64. Publication waits for all four desktop
jobs and the native/WASM replay gate. Its release includes the four desktop archives,
their checksums, the update manifest and the separately hashed Linux Website artifact.
The update owner is the game repository supplied by the workflow; Website Downloads
links to that same repository. Workflow configuration is not a completed platform run.

`smoke:desktop` launches the actual packaged app with external networking blocked.
It drives ordinary Solo/Create/movement, app-close save, restart/Continue, the
update notice, then two real apps through the encrypted invite and Boneyard entry.
It observes native frames and the actual schema51 client document; it sends no
synthetic gameplay packets or save contents. Evidence defaults to
`reports/desktop-smoke`; set `SDR_DESKTOP_APP` and `SDR_DESKTOP_EVIDENCE` to use
an assembled candidate and an external report directory. The full package smoke
requires the root-owned desktop Game/preload/save binding. A passing shell unit
suite alone is not a packaged desktop acceptance claim.

The extracted peer/update/static/runtime-stage tests passed14cases on the M2.
A real Chrome check first reproduced the Source CSP's WASM failure, then passed
streaming compilation with the extracted server's narrow `wasm-unsafe-eval`
permission and `application/wasm` MIME. The retained native10-transaction Source
replay and packaged Electron journey are separate gates.
