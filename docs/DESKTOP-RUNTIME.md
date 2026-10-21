# Desktop runtime

The desktop shell is the current Website's Electron shell at
`fa08ae249ff5563570bdb2a653e2ef9358ce7b83`. It retains the secure
`solomon-darker://app` origin, sandboxed preload, launcher, optional updates and
Node TLS 1.3 PSK peer tunnel. Only the authoritative child changes from the
TypeScript game host to `solomon-darker --desktop`.

## Native child and browser contract

The shell starts the packaged executable in `authority/` with:

```
solomon-darker --desktop --assets <absolute packaged client directory>
```

It supplies these environment variables, keeping the credential out of arguments
and logs:

| Variable | Value |
| --- | --- |
| `SDR_GAME_HOST` | `127.0.0.1` |
| `SDR_GAME_PORT` | `0`, selecting an ephemeral port |
| `SDR_GAME_ALLOWED_ORIGINS` | `solomon-darker://app` |
| `SDR_GAME_BOOTSTRAP_CREDENTIAL` | Random 32-byte base64url capability |
| `SDR_GAME_REVISION` | Full package Git revision |

Readiness is one JSON line, emitted after the actual WebSocket route is installed:
`{"type":"ready","url":"ws://127.0.0.1:<port>/game","protocol":"solomon-game-v1"}`.
The parent retains the existing 12-second deadline and bounded output. It sends
SIGTERM on ordinary shutdown, then SIGKILL after three seconds. An open stdin
pipe also makes an abruptly lost Electron parent stop the Rust child, matching
the former Node IPC-disconnect lifetime.

The preload remains the Source contract:

```js
window.solomonDarkRuntime.gameEndpoint = {
  kind: 'localhost', sessionKind: 'standalone', url, credential,
};
window.solomonDarkRuntime.desktop.onBeforeClose(async () => {
  // Request the canonical SaveBeforeLeave checkpoint, persist its source document
  // in the actual client save store, then dispose the session before resolving.
});
```

The client uses its existing hosted Hello/protocol with that explicit endpoint.
It does not call a Website provisioning API, inspect the legacy direct native
host, or create another simulation. The source document is the current schema51
save, including supported historic migrations; there is no native-schema2 server
file. A failed or unfinished save rejects the callback. Electron retains its
20-second deadline and offers Keep playing / Close without saving.

The rest of `desktop` exposes `getState`, `startSession`, `returnToLauncher`,
`checkUpdates`, `downloadUpdate`, `openWebsite`, `onBeforeClose` and `onState`.
Explore opens the Website and Join returns to the desktop launcher. Optional
update failures must leave offline play available. Browser integration of these
callbacks belongs to the existing Game owner.

## Authority ownership

`HostedSupervisor::new_standalone` constructs one `HostedGameHost` with
`GameSessionKind::Standalone` and `editor_test=false`. The existing supervisor
loop handles real Console, input, inventory/dye, mods, pause/readiness, audio and
source-save transactions. Ordinary Website construction and ticket routing stay
unchanged.

Desktop admission uses one reusable, timing-safe shared capability. It has no
HTTP provisioning/admin surface and no external social broker registration.
Private College cheat policy remains shared; standalone cheat visibility belongs
to the requesting client. Source standalone has no Party/directory/global-chat
surface. After the last human leaves, a fresh owner resets players, clocks,
Console and run state while retaining pending sockets and catalog identity.

The Node peer child retains the Source invite format, revision checks,
HKDF-SHA256 and TLS1.3 PSK cipher. It forwards the existing WebSocket bytes to
loopback and never interprets gameplay. No custom cryptography or second game
store is introduced.

## Evidence and acceptance

The unchanged Source desktop tests passed all thirteen cases, including actual
PSK echo/wrong-key/close and streaming-file cleanup. A Chrome probe against the
unchanged Source static server proved its `script-src 'self'` policy blocks a
valid eight-byte WASM module. The extracted server permits only
`'wasm-unsafe-eval'` and supplies WASM/audio MIME types; JavaScript `unsafe-eval`
remains disabled. Streaming WASM compilation then passed in the same Chrome
probe against the extracted server.

Desktop builds pass Trunk `--public-url /` and externalize its real generated
initializer through the same implementation as hosted builds. Prebuilt clients
are checked against that root contract; Website/editor distributions are rejected.
No Website or deployment metadata is injected. `test:desktop` covers preparation,
prebuilt reuse and the unchanged hosted/editor outputs. The packaged Electron
journey additionally requires the external initializer, successful root JS/WASM
responses, the live game title, accepted input and source-save continuation. The
small WASM policy probe alone is not full client startup acceptance.

`desktop-authority-goldens.mjs` records ten actual Source WebSocket transactions:
wrong bootstrap, same-capability peer admission, guest Console refusal,
client-local Cheats, actual Lua mutation, source SaveBeforeLeave, authority
transfer, last-human reset and saved continuation. The launcher integration test
replays those records over the native public `/game` route. Fixture comparison
and the packaged browser journey are required; recording the Source fixture is
not a Rust or desktop acceptance claim.

The extracted Node suite passes fourteen cases including explicit Rust release-owner
selection. Public desktop commands and packaging retain the Source launcher/peer/update
lifecycle. The Rust repository supplies its own native and WASM artifacts. A
package records its release repository; development can retain the Source update
owner until the new Rust remote is named. Publication must set the actual Rust
release owner rather than ship a Website update target.
