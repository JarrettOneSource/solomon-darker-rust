import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
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
