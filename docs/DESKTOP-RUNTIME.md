# Desktop runtime

The released desktop app uses an Electron shell around the Rust authority and
WebAssembly GPU client. The shell owns its sandboxed window, preload, native menus,
process lifetimes, peer transport, and optional updates. Rust owns gameplay and
visible launcher/game UI.

[Desktop commands](../desktop/README.md) cover development and packaging.
Native-window work is tracked in [Open work](WORK-QUEUE.md).

## Native child

The shell starts the packaged executable in `authority/`:

```text
solomon-darker --desktop --assets <absolute packaged client directory>
```

| Environment variable | Value |
| --- | --- |
| `SDR_GAME_HOST` | `127.0.0.1` |
| `SDR_GAME_PORT` | `0`, selecting an ephemeral port |
| `SDR_GAME_ALLOWED_ORIGINS` | `solomon-darker://app` |
| `SDR_GAME_BOOTSTRAP_CREDENTIAL` | Random 32-byte base64url capability |
| `SDR_GAME_REVISION` | Full package Git revision |

The credential stays out of arguments and logs. Readiness is one JSON line after
the WebSocket route is installed:

```json
{"type":"ready","url":"ws://127.0.0.1:<port>/game","protocol":"solomon-game-v1"}
```

The parent enforces a twelve-second readiness deadline and bounded output.
Ordinary shutdown sends SIGTERM, then SIGKILL after three seconds if needed.
An open stdin pipe lets the child detect an abruptly lost parent.

## Preload and saves

The preload supplies `window.solomonDarkRuntime.gameEndpoint` with endpoint kind,
session kind, URL, and credential. The client uses that explicit endpoint.

The desktop API exposes `getState`, `startSession`, `returnToLauncher`,
`checkUpdates`, `downloadUpdate`, `openWebsite`, `onBeforeClose`, and `onState`.

Before close or return, the client requests SaveBeforeLeave, persists the browser
save document, and disposes the session only after success. The desktop document
uses schema 51 with supported migrations. A failed or unfinished write rejects
close; the shell's twenty-second deadline offers **Keep playing** or
**Close without saving**.

Optional update failure leaves offline play available. Updates validate revision,
timestamp, platform, and artifact identity; they do not restart an active game.

## Authority and peer transport

`HostedSupervisor::new_standalone` creates a `HostedGameHost` with
`GameSessionKind::Standalone`. It owns accepted commands, inventory, mods,
pause/readiness, audio, and saves.

Desktop admission uses a timing-safe shared capability. Direct standalone play
has no website party directory or Global chat. When the last human leaves, a fresh
owner resets participant and run state while retaining pending sockets and catalog
identity.

The peer process uses revision-checked invites and an HKDF-SHA256/TLS 1.3 PSK tunnel.
It forwards WebSocket bytes to loopback without interpreting gameplay.

## Packaging contract

The client is root-mounted with an external generated initializer. The content
policy permits WebAssembly compilation and supplies correct WASM/audio MIME types.
Prebuilt clients are checked against the desktop root contract.

Release packaging records the native executable hash, runtime identity, revision,
and release repository. Archive inspection checks CRCs, asset membership/hashes,
shell files, the native manifest, and build residue before writing checksums.

Run shell tests and the actual packaged journey on each affected platform.
The journey checks GPU launcher controls, offline play, save/Continue, failed saves,
updates, return to launcher, and two encrypted peers in one Boneyard.
[Release](RELEASE.md) defines qualification and publication.
