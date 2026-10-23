import assert from 'node:assert/strict';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
import test from 'node:test';

test('a main WASM failure rejects bootstrap import while retaining the Rust loader error', async () => {
  const root = await mkdtemp(join(tmpdir(), 'solomon-bootstrap-failure-'));
  const resources = {
    loaderModule: 'solomon-loader-1111111111111111.js',
    loaderWasm: 'solomon-loader-2222222222222222_bg.wasm',
    mainModule: 'solomon-web-3333333333333333.js',
    mainWasm: 'solomon-web-4444444444444444_bg.wasm',
  };
  const previousWindow = globalThis.window;
  const previousFetch = globalThis.fetch;
  const previousError = console.error;
  const report = [];
  globalThis.window = { report };
  console.error = () => {};
  globalThis.fetch = async url => new Response('', { status: url.pathname.endsWith(resources.mainWasm) ? 503 : 200 });
  try {
    await writeFile(join(root, 'package.json'), '{"type":"module"}');
    await writeFile(join(root, resources.loaderModule), `
      export default async function init() {}
      export async function boot_loader() { window.report.push('loader-drawn'); }
      export function progress() {}
      export function error(message) { window.report.push(message); }
      export function finish() {}
      export function active() { return true; }
      export function gpu_ui_state() { return '{}'; }
    `);
    await writeFile(join(root, resources.mainModule), 'export default async function init() { throw new Error("must not initialize a failed response"); }');
    const template = await readFile(new URL('../web/bootstrap.mjs', import.meta.url), 'utf8');
    await writeFile(join(root, 'boot.mjs'), template.replace('__SOLOMON_RESOURCES__', JSON.stringify(resources)));
    await assert.rejects(import(pathToFileURL(join(root, 'boot.mjs'))), new RegExp(`${resources.mainWasm}: HTTP 503`));
    assert.equal(report[0], 'loader-drawn');
    assert.match(report[1], new RegExp(`${resources.mainWasm}: HTTP 503`));
    assert.equal(typeof window.wasmBindings.gpu_ui_state, 'function');
    assert.equal(window.wasmBindings.editor_test_cleanup, undefined);
  } finally {
    globalThis.window = previousWindow;
    globalThis.fetch = previousFetch;
    console.error = previousError;
    await rm(root, { recursive: true, force: true });
  }
});
