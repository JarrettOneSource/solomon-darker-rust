import assert from 'node:assert/strict';
import { copyFile, mkdir, mkdtemp, readFile, realpath, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
import vm from 'node:vm';
import test from 'node:test';

test('early editor cancellation survives loader-only bindings until main cleanup completes', async () => {
  const source = await readFile(new URL('../web/editor-test-boot.mjs', import.meta.url), 'utf8');
  const listeners = new Map();
  let poll;
  let cleared = false;
  let finished = false;
  const parent = { postMessage() {} };
  const window = {
    location: { origin: 'https://example.test' }, parent,
    wasmBindings: { gpu_ui_state: () => '{}' },
    solomonLoader: { finish() { finished = true; } },
    addEventListener(name, callback) { listeners.set(name, callback); },
    setInterval(callback) { poll = callback; return 1; },
    clearInterval() { cleared = true; },
  };
  vm.runInNewContext(source, { window });
  listeners.get('message')({ origin: window.location.origin, source: parent, data: { type: 'solomon-editor-test-cancel' } });
  assert.doesNotThrow(() => poll());
  assert.equal(cleared, false, 'the cancellation request survives before main owns cleanup');
  assert.equal(finished, false);
  window.wasmBindings.editor_test_cleanup = () => true;
  poll();
  assert.equal(cleared, true);
  assert.equal(finished, true, 'completed disposal retires a loader whose first main frame never ran');
});

test('the test start imports the game entry relative to the document base, like its script src', async context => {
  // Module URLs name real paths; macOS reaches its temporary directory through a link.
  const root = await realpath(await mkdtemp(join(tmpdir(), 'solomon-editor-entry-')));
  context.after(() => rm(root, { recursive: true, force: true }));
  // The base names the client root. The entry script sits elsewhere here, so resolving
  // against the script's own URL would miss the game initializer.
  const client = join(root, 'client');
  const scripts = join(root, 'scripts');
  await mkdir(client);
  await mkdir(scripts);
  const entry = join(client, 'solomon-boot-0123456789abcdef.mjs');
  await writeFile(entry, 'globalThis.solomonEditorEntry = import.meta.url;\n');
  const boot = join(scripts, 'editor-test-boot.mjs');
  await copyFile(new URL('../web/editor-test-boot.mjs', import.meta.url), boot);
  const listeners = new Map();
  const posted = [];
  const parent = { postMessage(message) { posted.push(message.type); } };
  const previous = { window: globalThis.window, document: globalThis.document };
  globalThis.window = {
    location: { origin: 'https://example.test' }, parent,
    addEventListener(name, callback) { listeners.set(name, callback); },
  };
  globalThis.document = {
    baseURI: new URL('client/', pathToFileURL(join(root, '/'))).href,
    querySelector: selector => selector === 'script[data-editor-test-boot]'
      ? { dataset: { gameBoot: './solomon-boot-0123456789abcdef.mjs' } } : null,
  };
  try {
    await import(pathToFileURL(boot).href);
    await listeners.get('message')({ origin: 'https://example.test', source: parent,
      data: { type: 'solomon-editor-test-start', name: 'Draft', bytesBase64: '' } });
    assert.equal(globalThis.solomonEditorEntry, pathToFileURL(entry).href);
    assert.deepEqual(posted, ['solomon-editor-test-ready'], 'the import reported no failure');
  } finally {
    globalThis.window = previous.window;
    globalThis.document = previous.document;
    delete globalThis.solomonEditorEntry;
  }
});
