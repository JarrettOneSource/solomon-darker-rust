# Account save tools

`solomon-save-tools` is an independent WebAssembly module for Account save
import/export. Validation and conversion live in `solomon-assets`.
Importing this module does not start a game, renderer, asset load, or socket.

## Browser API

After the generated module's `init()` resolves:

- `readGameSaveFileSelection(files)` accepts a browser `FileList` or `File[]`.
  It resolves a preview with `discipline`, `displayName`, `document`, `element`,
  `gold`, `hagathaPerks`, `learnedRows`, `level`, `source`, and `warnings`.
- `exportWebGameSaveToNativeArchive(document)` resolves
  `{archive, portable, warnings}`; `archive` is a `Uint8Array`.
  The Account owns warning confirmation and downloading the ZIP.

Canonical save failures reject with JavaScript `Error` objects. Browser file-read
failures retain their rejection. Directory selection preserves
`webkitRelativePath` for native run names.

Build this entry with the workspace WASM toolchain and
`wasm-bindgen --target web`. The Account may initialize it lazily.

## Checks

```sh
cargo test -p solomon-save-tools --test current_portability
```

The portability tests cover discipline roots, later-hotbar warnings, and
equipment-only learned rows.

Serve the module and point the browser ABI check at a same-origin Account or
neutral page that does not start a game:

```sh
PW=/path/to/playwright-core \
SAVE_TOOLS_PAGE_URL=https://example.test/account \
SAVE_TOOLS_MODULE_URL=https://example.test/save-tools/solomon_save_tools.js \
node tools/browser-smoke/save-tools.cjs
```

This checks real `File[]` and `FileList` inputs. The website's mounted Account
buttons require their own integration journey.
