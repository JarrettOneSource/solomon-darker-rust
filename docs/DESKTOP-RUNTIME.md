# Native desktop runtime

The desktop app runs the shared Rust Game and GPU interface in an OS window.
`solomon-desktop` owns the event loop, graphics context, sessions, encrypted peers,
profile lock and optional updates. It embeds `solomon-launcher`'s standalone
supervisor on an owned thread. The browser separately compiles the same game
and interface to WASM. Desktop packages contain no Electron, Chromium or Node runtime.

## Build and package

Use the pinned Rust toolchain, platform linker, CMake, libclang and Perl. Linux
also needs ALSA, udev and Fontconfig development packages. On Ubuntu:

```sh
sudo apt-get install libasound2-dev libudev-dev libfontconfig1-dev libclang-dev cmake
```

Windows uses the MSVC toolchain and native Strawberry Perl for vendored OpenSSL.
When a shell selects another Perl, set `OPENSSL_SRC_PERL` to the native interpreter.
Node and Python run repository tooling; they are not application dependencies.

From the repository root:

```sh
npm ci
npm run dev:desktop
npm run test:desktop
npm run package:desktop -- --release-repository JarrettOneSource/solomon-darker-rust
npm run archive:desktop
python -m pip install -r tools/desktop-smoke-requirements.txt
npm run smoke:desktop
```

Packaging supports `--platform win32 --arch x64`, `--platform linux --arch x64`,
and `--platform darwin --arch arm64` or `x64`. Build on the target platform with
its linker and system libraries. macOS packaging signs the final bundle ad hoc;
these builds are not publisher-signed or notarized.

To stage an already built native executable:

```sh
npm run package:desktop -- --platform darwin --arch arm64 --skip-build \
  --native-app /absolute/solomon-darker-app --assets-root /absolute/source/web \
  --release-repository JarrettOneSource/solomon-darker-rust
```

Development accepts `SDR_DESKTOP_APP_BINARY` and `SDR_DESKTOP_CLIENT_ROOT`.
The executable accepts `--assets`, `--data-dir` and `--revision` for explicit
development paths. A packaged app discovers its assets and build record beside
the executable, or under `Contents/Resources` in a Mac bundle.

## Profiles and save-before-close

The default profile is `Solomon Darker` under `%APPDATA%` on Windows,
`~/Library/Application Support` on macOS, or `$XDG_CONFIG_HOME` (otherwise
`~/.config`) on Linux. Native documents live in its `native-v1` directory.
An OS file lock gives one process ownership; a second launch requests focus from
that owner and exits without opening another game.

`solomon-storage` holds a separate OS lock across each revision check and atomic,
synced document replacement. Client saves preserve the complete supported schema 51
document, separate from the direct launcher's schema 2 server file. Browser builds
use IndexedDB through their own storage adapter.

Close, macOS Quit and return to launcher request the shared SaveBeforeLeave flow.
The app waits for persistence before retiring Game, tasks, peer/authority workers
and the GPU context. A failed save or twenty-second deadline keeps the game alive
and offers **Keep playing** or **Close without saving**. The latter preserves the
preceding durable document. Keep playing also cancels pending Cocoa termination.

A fatal client error replaces gameplay with a recovery screen. It receives native
input and accessibility actions directly, so the inactive game UI cannot consume
them. The shared Game clears local input and sends a neutral input to the authority.
If the connection rejects that release, its error is logged and the original
failure stays visible. Save and return uses the same persistence coordinator.

The previous Electron profile stays intact. To migrate, use the previous app's
Settings → Online and account → Stock / browser save → Download save archive.
In the native app choose Play offline, then Settings → Online and account →
Import / export save and select that ZIP. Inspect the preview before replacing
the native slot. Keep the previous app until the wizard is restored.

## Authority and peers

The local authority owns accepted commands, inventory, mods, readiness, audio and
world state. The client owns its durable player document. A generated capability
authenticates loopback admission; credentials stay out of arguments and logs.

`HostedSupervisor::new_standalone` creates a `HostedGameHost` with
`GameSessionKind::Standalone`. The client sends its normal hosted Hello to that
local endpoint; startup does not provision a website party. When the last human
leaves, a fresh owner resets participant and run state while retaining pending
sockets and catalog identity.

Host game opens a revision-checked v1 invitation. Press Escape and choose
**Copy multiplayer invite**; friends paste it into **Join a friend**. The host
must remain reachable on the advertised TCP port. Tokio forwards WebSocket bytes
through an HKDF-SHA256/TLS 1.3 PSK tunnel implemented by bundled OpenSSL. Each
service cancels sockets and joins its worker before session retirement.

The party leader starts a shared Boneyard. Direct standalone play has no website
party directory or Global chat. Saved worlds may load only on a fresh host owner;
a joining saved world cannot overwrite an already active host.

## Updates and release validation

The app checks its configured GitHub release owner in the background. It validates
the revision, timestamp, platform and archive identity before offering a download.
Download opens the qualified ZIP in the system browser; the app never replaces or
restarts an active game. Network failure leaves offline play available.

The schema2 package manifest hashes every final payload file, including the signed
executable and Mac bundle metadata. Archive inspection checks CRCs, complete file
membership, source assets/licenses, revision and absence of retired runtime files
before producing the archive checksum.

The packaged journey drives OS accessibility, keyboard, clipboard and window APIs
with xa11y. It reads real saves, exercises a concurrent revision conflict, and
connects two real encrypted peers. The driver and its Python dependency stay outside
the package. Reports default to `reports/desktop-smoke`; `SDR_DESKTOP_APP` selects
a package and `SDR_DESKTOP_EVIDENCE` selects the report directory. CI provides a
separate desktop session, accessibility permissions and software graphics as needed.

Run the journey on Windows x64, Linux x64 and both Mac architectures. Screenshot
review and process cleanup are part of acceptance. [Release](RELEASE.md) separates
source checks, qualified archives, installation and live browser outcomes.
