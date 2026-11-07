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
// Trunk writes the configured public URL, "./", as the document base.
function trunkDocument(base = './') {
  return `<!doctype html><html><head><title>Solomon Darker</title>
<meta name="solomon-bootstrap" content="gpu-loader-v1"><base href="${base}">
<link rel="icon" type="image/png" href="assets/stock/favicon.png">
</head><body><canvas id="game"></canvas></body></html>`;
}
const generatedResources = [
  'solomon-loader-1111111111111111.js', 'solomon-loader-2222222222222222_bg.wasm',
  'solomon-client-d693347574b3e849.js', 'solomon-client-852180e6d53c052b_bg.wasm',
];
const digest = bytes => createHash('sha256').update(bytes).digest('hex').slice(0, 16);
const python = process.env.PYTHON || (process.platform === 'win32' ? 'python' : 'python3');

async function distribution(context, document) {
  const root = await mkdtemp(join(tmpdir(), 'solomon-desktop-build-'));
  context.after(() => rm(root, {recursive: true, force: true}));
  await writeFile(join(root, 'index.html'), document);
  for (const name of generatedResources) await writeFile(join(root, name), 'fixture generated resource');
  await mkdir(join(root, 'assets/stock'), {recursive: true});
  await writeFile(join(root, 'assets/stock/favicon.png'), 'fixture icon');
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
  execFileSync(python, ['-c', `
import os, subprocess, sys, tomllib
from pathlib import Path
hook = tomllib.loads(Path('Trunk.toml').read_text())['hooks'][0]
platform = {'win32': 'windows', 'darwin': 'macos', 'linux': 'linux'}[sys.platform]
command = hook.get(platform, hook)
subprocess.run([command['command'], *command['command_arguments']], check=True)
`], {
    cwd: web,
    env: {...process.env, PYTHONDONTWRITEBYTECODE: '1',
      TRUNK_SOURCE_DIR: toNamespacedPath(web), TRUNK_STAGING_DIR: toNamespacedPath(root)},
  });
  const document = await readFile(join(root, 'index.html'), 'utf8');
  const initializer = /src="\.\/(solomon-boot-[a-f0-9]{16}\.mjs)"/.exec(document);
  assert.ok(initializer, 'the configured hook emits the external GPU loader');
  const bootstrap = await readFile(join(root, initializer[1]), 'utf8');
  assert.match(bootstrap, /"loaderModule": "solomon-loader-[a-f0-9]+\.js"/);
  assert.match(bootstrap, /"mainWasm": "solomon-client-[a-f0-9]+_bg\.wasm"/);
});

test('worker outputs hash WASM before rewriting and hashing the JavaScript binding', async context => {
  const root = await mkdtemp(join(tmpdir(), 'solomon-worker-build-'));
  context.after(() => rm(root, {recursive: true, force: true}));
  await writeFile(join(root, 'index.html'), trunkDocument());
  const expected = [];
  for (const name of ['solomon-loader', 'solomon-client']) {
    const wasm = Buffer.from(`${name} WASM fixture`);
    const wasmName = `${name}-${digest(wasm)}_bg.wasm`;
    const binding = `export default () => new URL('${name}_bg.wasm', import.meta.url);${name === 'solomon-client' ? '\r\n' : '\n'}`;
    const rewritten = binding.replace(`${name}_bg.wasm`, wasmName);
    const moduleName = `${name}-${digest(rewritten)}.js`;
    await writeFile(join(root, `${name}_bg.wasm`), wasm);
    await writeFile(join(root, `${name}.js`), binding);
    expected.push(wasmName, moduleName);
  }
  const prepare = () => execFileSync(python, [join(repositoryRoot, 'scripts/prepare-trunk-client.py')],
    {env: {...process.env, PYTHONDONTWRITEBYTECODE: '1', TRUNK_STAGING_DIR: root}});
  prepare();
  const files = await readdir(root);
  for (const name of expected) assert.ok(files.includes(name), `missing content-hashed ${name}`);
  assert.equal(files.some(name => /solomon-(?:loader|client)(?:_bg\.wasm|\.js)$/.test(name)), false);
  for (const name of expected.filter(name => name.endsWith('.js'))) {
    const binding = await readFile(join(root, name), 'utf8');
    assert.equal(name.match(/-([a-f0-9]{16})\.js$/)[1], digest(binding));
    assert.match(binding, /new URL\('solomon-(?:loader|client)-[a-f0-9]{16}_bg\.wasm', import.meta.url\)/);
  }
  prepare();
  assert.deepEqual((await readdir(root)).sort(), files.sort(), 'preparation is idempotent');
});

test('a document base that names a mount fails the build', async context => {
  const root = await distribution(context, trunkDocument('/game/'));
  assert.throws(() => execFileSync(python, [join(repositoryRoot, 'scripts/prepare-trunk-client.py')],
    {env: {...process.env, PYTHONDONTWRITEBYTECODE: '1', TRUNK_STAGING_DIR: root}, stdio: 'pipe'}),
  error => /<base href="\.\/">/.test(error.stderr));
});

// Resolve a document's references as a browser does: against its base, which is itself
// relative to the page. Returns the distribution files they name under the serving mount.
function resolvedFiles(document, page, mount) {
  const [tag, base] = /<base href="([^"]+)">/.exec(document);
  const references = [...document.replace(tag, '').matchAll(/ (?:src|href|data-game-boot)="([^"]+)"/g)]
    .map(match => match[1]);
  return references.map(reference => {
    for (const url of [base, reference]) assert.doesNotMatch(url, /^(?:\/|[a-z]+:)/, `${url} names no mount`);
    const url = new URL(reference, new URL(base, page));
    assert.ok(url.origin === page.origin && url.pathname.startsWith(mount), `${url} stays under ${mount}`);
    return decodeURIComponent(url.pathname.slice(mount.length));
  });
}

test('hosted preparation keeps every reference relative and adds the deployment metadata and editor entry', async context => {
  const root = await distribution(context, trunkDocument());
  const revision = '1'.repeat(40);
  execFileSync(python, [join(repositoryRoot, 'scripts/prepare-hosted-client.py'), root,
    '--mount', '/game-stg', '--website-mount', '', '--revision', revision],
  {env: {...process.env, PYTHONDONTWRITEBYTECODE: '1'}});
  const document = await readFile(join(root, 'index.html'), 'utf8');
  assert.equal(document.includes('solomon-storage-namespace'), false, 'no namespace unless one is asked for');
  const editor = await readFile(join(root, 'editor-test/index.html'), 'utf8');
  const boot = /<script type="module" src="\.\/(solomon-boot-[a-f0-9]{16}\.mjs)"><\/script>/.exec(document)[1];
  assert.match(document, /<base href="\.\/">/);
  assert.match(editor, /<base href="\.\.\/">/);
  assert.ok(editor.includes('<script type="module" src="./editor-test-boot.mjs" data-editor-test-boot '
    + `data-game-boot="./${boot}"></script>`), 'the editor entry wraps the same game initializer');
  assert.match(editor, /name="solomon-editor-test"/);
  for (const page of [document, editor]) {
    assert.equal(page.split('<script').length, 2, 'the one external entry is the only script');
    assert.match(page, /name="solomon-build-revision" content="1{40}"/);
    assert.match(page, /name="solomon-website-mount" content=""/);
    // The mount is deployment metadata only; no URL names it.
    assert.match(page, /name="solomon-game-mount" content="\/game-stg"/);
    assert.equal(page.split('/game-stg').length, 2);
  }
  // One distribution serves at the site root or below any mount.
  for (const mount of ['/', '/game/', '/game-rust/', '/game-stg/']) {
    const origin = 'https://example.test';
    assert.deepEqual(resolvedFiles(document, new URL(mount, origin), mount),
      ['assets/stock/favicon.png', boot]);
    assert.deepEqual(resolvedFiles(editor, new URL(`${mount}editor-test/`, origin), mount),
      ['assets/stock/favicon.png', 'editor-test-boot.mjs', boot]);
  }
  for (const name of ['assets/stock/favicon.png', 'editor-test-boot.mjs', boot]) await readFile(join(root, name));
  assert.equal(await readFile(join(root, 'editor-test-boot.mjs'), 'utf8'),
    await readFile(join(repositoryRoot, 'web/editor-test-boot.mjs'), 'utf8'));
  assert.deepEqual(JSON.parse(await readFile(join(root, 'deployment.json'), 'utf8')), {revision});
});

test('a storage namespace is written only when given, and must name a path', async context => {
  const python = process.env.PYTHON || (process.platform === 'win32' ? 'python' : 'python3');
  const prepare = (root, namespace) => execFileSync(python,
    [join(repositoryRoot, 'scripts/prepare-hosted-client.py'), root, '--mount', '/game',
      '--website-mount', '/game', '--storage-namespace', namespace, '--revision', '1'.repeat(40)],
    {env: {...process.env, PYTHONDONTWRITEBYTECODE: '1'}, stdio: 'pipe'});
  const root = await distribution(context, trunkDocument());
  prepare(root, '/game-rust');
  assert.match(await readFile(join(root, 'index.html'), 'utf8'),
    /name="solomon-storage-namespace" content="\/game-rust"/);
  for (const invalid of ['', '/', 'game-rust', '/game-rust/']) {
    const fresh = await distribution(context, trunkDocument());
    assert.throws(() => prepare(fresh, invalid), undefined, `accepted ${JSON.stringify(invalid)}`);
  }
});
