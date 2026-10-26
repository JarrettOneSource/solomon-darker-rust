# Desktop app development

The released app combines a sandboxed Electron window, a Rust game authority,
a WebAssembly GPU client, and a pinned Node peer runtime.
[Desktop runtime](../docs/DESKTOP-RUNTIME.md) defines their interfaces and lifetimes.

## Commands

Install the locked development tools with `npm ci`, then run commands from the
repository root:

```sh
npm run dev:desktop
npm run test:desktop
npm run package:desktop:mac -- --release-repository JarrettOneSource/solomon-darker-rust
npm run package:desktop:windows -- --release-repository JarrettOneSource/solomon-darker-rust
npm run package:desktop:linux -- --release-repository JarrettOneSource/solomon-darker-rust
node tools/package-desktop.mjs --platform darwin --arch x64 --release-repository JarrettOneSource/solomon-darker-rust
npm run archive:desktop
npm run smoke:desktop
```

Packaging needs the selected platform's Rust linker/toolchain. Development builds
the native executable and client sequentially; release packaging records their
revision, hashes, and runtime identity.

To reuse verified builds:

```sh
npm run package:desktop -- --platform darwin --arch arm64 --skip-build \
  --native-host /absolute/solomon-darker --client-root /absolute/wasm-client \
  --release-repository JarrettOneSource/solomon-darker-rust
```

Development also accepts `SDR_DESKTOP_GAME_HOST`, `SDR_DESKTOP_CLIENT_ROOT`,
and `SDR_DESKTOP_NODE`. Release packages stage the checksum-pinned peer runtime.
Release ownership must be explicit through `--release-repository` or
`SDR_DESKTOP_RELEASE_REPOSITORY`.

## Package validation

Archive inspection checks final contents before checksums are written.
The packaged journey runs with external networking blocked and drives ordinary
launcher, Solo/Create, movement, save/Continue, failed-save choices, updates,
return, and two-app encrypted peer play.

Reports default to `reports/desktop-smoke`. Set `SDR_DESKTOP_APP` to select an
assembled candidate and `SDR_DESKTOP_EVIDENCE` for an external report directory.

CI qualifies Windows x64, Linux x64, and both Mac architectures separately.
Publication waits for all package jobs and the game gate. A passing shell unit
suite does not establish packaged gameplay. See [Release](../docs/RELEASE.md).
