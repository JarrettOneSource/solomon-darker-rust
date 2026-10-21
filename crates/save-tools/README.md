# Account save tools

`solomon-save-tools` is an independent WebAssembly entry for the Website's Account
save controls. It depends on the existing `solomon-assets` native/Website save readers.
It has no game startup function or dependency on `solomon-web`.

After the generated module's explicit `init()` resolves, it exports:

- `readGameSaveFileSelection(files)`: accepts a browser `FileList` or `File[]` and
  resolves the Source `GameSaveImportPreview`: `discipline`, `displayName`, `document`,
  `element`, `gold`, `hagathaPerks`, `learnedRows`, `level`, `source`, and `warnings`.
- `exportWebGameSaveToNativeArchive(document)`: resolves `{archive, portable, warnings}`.
  `archive` is a `Uint8Array`; the Account keeps warning confirmation and ZIP download.

Both operations reject with real JavaScript `Error` objects for canonical save errors.
Browser File read failures retain their original rejection. Directory selections preserve
`webkitRelativePath`, which supplies the native save's run name.

Build this entry separately with the workspace's existing WASM toolchain and
`wasm-bindgen --target web`; the Website loader can initialize it lazily on first use.
No game assets, renderer, WebSocket, or automatic game initialization are imported.

The current Account fixture replays all eight portable discipline roots, later-hotbar
export warnings and equipment-only learned rows through the existing asset APIs:

```sh
cargo test -p solomon-save-tools --test current_portability
```

After serving the generated module, the browser ABI check uses real `File[]` and
`FileList` selections. Point it at a same-origin Account or neutral page which does
not start the game:

```sh
PW=/path/to/playwright \
SAVE_TOOLS_PAGE_URL=https://example.test/account \
SAVE_TOOLS_MODULE_URL=https://example.test/save-tools/solomon_save_tools.js \
node tools/browser-smoke/save-tools.cjs
```

This explicit-module check is separate from acceptance of the Website's mounted
Account import/export buttons.
