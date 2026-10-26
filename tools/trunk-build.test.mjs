import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { copyFile, mkdir, mkdtemp, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, toNamespacedPath } from 'node:path';
import test from 'node:test';

import { repositoryRoot } from './desktop-build.mjs';

// Both wasm-bindgen modules are built without auto-start; the shared bootstrap
// is produced from their content-hashed resources by the ordinary Trunk hook.
function trunkDocument(mount = '') {
  return `<!doctype html><html><head><title>Solomon Darker</title>
<meta name="solomon-bootstrap" content="gpu-loader-v1"><base href="${mount}/">
</head><body><canvas id="game"></canvas></body></html>`;
}
const generatedResources = [
  'solomon-loader-1111111111111111.js', 'solomon-loader-2222222222222222_bg.wasm',
  'solomon-web-d693347574b3e849.js', 'solomon-web-852180e6d53c052b_bg.wasm',
];
const digest = bytes => createHash('sha256').update(bytes).digest('hex').slice(0, 16);

async function distribution(context, document) {
  const root = await mkdtemp(join(tmpdir(), 'solomon-desktop-build-'));
  context.after(() => rm(root, {recursive: true, force: true}));
  await writeFile(join(root, 'index.html'), document);
  for (const name of generatedResources) await writeFile(join(root, name), 'fixture generated resource');
  return root;
}

test('the configured Trunk hook prepares a checkout whose path contains spaces', async context => {
  const root = await distribution(context, trunkDocument());
  const checkout = join(root, 'checkout with spaces');
  const web = join(checkout, 'web');
  await mkdir(web, {recursive: true});
  await mkdir(join(checkout, 'scripts'));
  for (const name of ['web/Trunk.toml', 'web/bootstrap.mjs', 'scripts/trunk_bootstrap.py',
    'scripts/prepare-trunk-client.py']) {
    await copyFile(join(repositoryRoot, name), join(checkout, name));
  }
  execFileSync(process.env.PYTHON || (process.platform === 'win32' ? 'python' : 'python3'), ['-c', `
import os, subprocess, sys, tomllib
from pathlib import Path
hook = tomllib.loads(Path('Trunk.toml').read_text())['hooks'][0]
platform = {'win32': 'windows', 'darwin': 'macos', 'linux': 'linux'}[sys.platform]
command = hook.get(platform, hook)
subprocess.run([command['command'], *command['command_arguments']], check=True)
`], {
    cwd: web,
    env: {...process.env, PYTHONDONTWRITEBYTECODE: '1', TRUNK_PUBLIC_URL: '/',
      TRUNK_SOURCE_DIR: toNamespacedPath(web), TRUNK_STAGING_DIR: toNamespacedPath(root)},
  });
  const document = await readFile(join(root, 'index.html'), 'utf8');
  const initializer = /src="\/(solomon-boot-[a-f0-9]{16}\.mjs)"/.exec(document);
  assert.ok(initializer, 'the configured hook emits the external GPU loader');
  const bootstrap = await readFile(join(root, initializer[1]), 'utf8');
  assert.match(bootstrap, /"loaderModule": "solomon-loader-[a-f0-9]+\.js"/);
  assert.match(bootstrap, /"mainWasm": "solomon-web-[a-f0-9]+_bg\.wasm"/);
});

test('worker outputs hash WASM before rewriting and hashing the JavaScript binding', async context => {
  const root = await mkdtemp(join(tmpdir(), 'solomon-worker-build-'));
  context.after(() => rm(root, {recursive: true, force: true}));
  await writeFile(join(root, 'index.html'), trunkDocument());
  const expected = [];
  for (const name of ['solomon-loader', 'solomon-web']) {
    const wasm = Buffer.from(`${name} WASM fixture`);
    const wasmName = `${name}-${digest(wasm)}_bg.wasm`;
    const binding = `export default () => new URL('${name}_bg.wasm', import.meta.url);${name === 'solomon-web' ? '\r\n' : '\n'}`;
    const rewritten = binding.replace(`${name}_bg.wasm`, wasmName);
    const moduleName = `${name}-${digest(rewritten)}.js`;
    await writeFile(join(root, `${name}_bg.wasm`), wasm);
    await writeFile(join(root, `${name}.js`), binding);
    expected.push(wasmName, moduleName);
  }
  const prepare = () => execFileSync(process.env.PYTHON || (process.platform === 'win32' ? 'python' : 'python3'),
    [join(repositoryRoot, 'scripts/prepare-trunk-client.py')],
    {env: {...process.env, PYTHONDONTWRITEBYTECODE: '1', TRUNK_PUBLIC_URL: '/', TRUNK_STAGING_DIR: root}});
  prepare();
  const files = await readdir(root);
  for (const name of expected) assert.ok(files.includes(name), `missing content-hashed ${name}`);
  assert.equal(files.some(name => /solomon-(?:loader|web)(?:_bg\.wasm|\.js)$/.test(name)), false);
  for (const name of expected.filter(name => name.endsWith('.js'))) {
    const binding = await readFile(join(root, name), 'utf8');
    assert.equal(name.match(/-([a-f0-9]{16})\.js$/)[1], digest(binding));
    assert.match(binding, /new URL\('solomon-(?:loader|web)-[a-f0-9]{16}_bg\.wasm', import.meta.url\)/);
  }
  prepare();
  assert.deepEqual((await readdir(root)).sort(), files.sort(), 'preparation is idempotent');
});

test('shared externalization preserves the separate Website mount and editor entry', async context => {
  const root = await distribution(context, trunkDocument('/game-stg'));
  const revision = '1'.repeat(40);
  execFileSync(process.env.PYTHON || (process.platform === 'win32' ? 'python' : 'python3'),
    [join(repositoryRoot, 'scripts/prepare-hosted-client.py'), root,
      '--mount', '/game-stg', '--website-mount', '', '--revision', revision],
    {env: {...process.env, PYTHONDONTWRITEBYTECODE: '1'}});
  const document = await readFile(join(root, 'index.html'), 'utf8');
  assert.match(document, /<base href="\/game-stg\/">/);
  assert.match(document, /name="solomon-website-mount" content=""/);
  assert.match(document, /name="solomon-game-mount" content="\/game-stg"/);
  assert.equal(document.includes('<script type="module">'), false);
  const editor = await readFile(join(root, 'editor-test/index.html'), 'utf8');
  assert.match(editor, /name="solomon-editor-test"/);
  assert.match(editor, /src="\/game-stg\/editor-test-boot\.mjs" data-editor-test-boot data-game-boot="\/game-stg\/solomon-boot-/);
  assert.equal(await readFile(join(root, 'editor-test-boot.mjs'), 'utf8'),
    await readFile(join(repositoryRoot, 'web/editor-test-boot.mjs'), 'utf8'));
  assert.deepEqual(JSON.parse(await readFile(join(root, 'deployment.json'), 'utf8')), {revision});
  assert.equal(await readFile(join(root, 'index.html'), 'utf8'), document);
});
