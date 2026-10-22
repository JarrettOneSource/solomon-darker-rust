import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdtemp, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';

import { desktopArtifacts, repositoryRoot } from './desktop-build.mjs';

// Trunk 0.22's real initializer shape, including its two generated resource
// paths. These checks exercise build preparation without compiling another game.
function trunkDocument(mount = '') {
  return `<!doctype html><html><head><title>Solomon Darker</title>
<script type="module">
import init, * as bindings from '${mount}/solomon-web-d693347574b3e849.js';
const wasm = await init({ module_or_path: '${mount}/solomon-web-852180e6d53c052b_bg.wasm' });
window.wasmBindings = bindings;
dispatchEvent(new CustomEvent("TrunkApplicationStarted", {detail: {wasm}}));
</script>
<link rel="stylesheet" href="${mount}/hud-b771245a093ca7fb.css">
</head><body><canvas id="game"></canvas></body></html>`;
}

async function distribution(context, document) {
  const root = await mkdtemp(join(tmpdir(), 'solomon-desktop-build-'));
  context.after(() => rm(root, {recursive: true, force: true}));
  await writeFile(join(root, 'index.html'), document);
  return root;
}

test('desktop artifacts externalize the root initializer and retain prepared builds', async context => {
  const root = await distribution(context, trunkDocument());
  await writeFile(join(root, '.trunk-lock'), '');
  const options = {skipBuild: true, nativeHost: process.execPath, clientRoot: root};
  const artifacts = await desktopArtifacts(options);
  assert.equal(artifacts.client, root);
  assert.equal((await readdir(root)).includes('.trunk-lock'), false,
    'a fresh desktop distribution excludes the Trunk build lock');
  const document = await readFile(join(root, 'index.html'), 'utf8');
  assert.equal(document.includes('<script type="module">'), false);
  assert.equal(document.includes('solomon-game-mount'), false);
  const initializer = /<script type="module" src="\/(solomon-boot-[a-f0-9]{16}\.mjs)"><\/script>/.exec(document);
  assert.ok(initializer, 'the ordinary script policy can load the generated external initializer');
  const bootstrap = await readFile(join(root, initializer[1]), 'utf8');
  assert.match(bootstrap, /from '\/solomon-web-[a-f0-9]+\.js'/);
  assert.match(bootstrap, /module_or_path: '\/solomon-web-[a-f0-9]+_bg\.wasm'/);
  await writeFile(join(root, '.trunk-lock'), '');
  await desktopArtifacts(options);
  assert.equal((await readdir(root)).includes('.trunk-lock'), false,
    'a prebuilt desktop distribution also excludes the Trunk build lock');
  assert.equal(await readFile(join(root, 'index.html'), 'utf8'), document);
  assert.equal((await readdir(root)).filter(name => name.endsWith('.mjs')).length, 1);
});

test('desktop preparation rejects a hosted distribution without changing it', async context => {
  const hosted = trunkDocument('/game-stg');
  const root = await distribution(context, hosted);
  await assert.rejects(desktopArtifacts({skipBuild: true, nativeHost: process.execPath, clientRoot: root}),
    /python.*failed/);
  assert.equal(await readFile(join(root, 'index.html'), 'utf8'), hosted);
  assert.deepEqual(await readdir(root), ['index.html']);
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
  await assert.rejects(desktopArtifacts({skipBuild: true, nativeHost: process.execPath, clientRoot: root}),
    /python.*failed/);
  assert.equal(await readFile(join(root, 'index.html'), 'utf8'), document);
});
