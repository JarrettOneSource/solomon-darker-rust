import assert from 'node:assert/strict';
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';
import { desktopArtifacts } from './desktop-build.mjs';

test('native desktop reuses its executable and source assets without a browser build', async context => {
  const directory = await mkdtemp(join(tmpdir(), 'solomon-native-artifacts-'));
  context.after(() => rm(directory, {recursive: true, force: true}));
  await mkdir(join(directory, 'assets', 'stock'), {recursive: true});
  await writeFile(join(directory, 'assets', 'stock', 'College.bundle'), 'native asset');
  await writeFile(join(directory, 'assets', 'stock-files.json'), '{}');
  await writeFile(join(directory, 'assets', 'boneyards.json'), '[]');
  const artifacts = await desktopArtifacts({skipBuild: true, nativeApp: process.execPath, assetsRoot: directory});
  assert.deepEqual(artifacts, {executable: process.execPath, assets: directory});
  assert.equal(await readFile(join(directory, 'assets', 'stock', 'College.bundle'), 'utf8'), 'native asset');
});

test('native desktop rejects absent assets and unsupported package targets', async context => {
  const directory = await mkdtemp(join(tmpdir(), 'solomon-native-missing-'));
  context.after(() => rm(directory, {recursive: true, force: true}));
  await assert.rejects(desktopArtifacts({skipBuild: true, nativeApp: process.execPath, assetsRoot: directory}), /ENOENT/);
  await assert.rejects(desktopArtifacts({platform: 'win32', arch: 'arm64', skipBuild: true}), /Supported desktop targets/);
});

test('packaging stages native assets and replaces only its own platform output', async context => {
  const {packageDesktop} = await import('./package-desktop.mjs');
  const {createHash} = await import('node:crypto');
  const directory = await mkdtemp(join(tmpdir(), 'solomon-native-package-'));
  context.after(() => rm(directory, {recursive: true, force: true}));
  const assets = join(directory, 'source');
  await mkdir(join(assets, 'assets'), {recursive: true});
  await writeFile(join(assets, 'assets', 'example.json'), '{"asset":true}');
  await writeFile(join(assets, 'index.html'), 'browser entry stays outside native packaging');
  const executable = join(directory, 'application');
  const bytes = 'test-only executable bytes';
  await writeFile(executable, bytes);
  const output = join(directory, 'output');
  const stale = join(output, 'Solomon Darker-linux-x64');
  await mkdir(stale, {recursive: true});
  await writeFile(join(stale, 'main.mjs'), 'retired shell');
  await writeFile(join(output, 'unrelated.txt'), 'keep');
  const build = {schemaVersion: 1, revision: 'a'.repeat(40), sourceTimestamp: 123, releaseRepository: 'owner/repository'};
  const result = await packageDesktop({platform: 'linux', arch: 'x64', executable, assets, output, build, version: '0.1.0'});
  assert.equal(result.runtime, 'native');
  assert.equal(result.schemaVersion, 2);
  assert.equal(result.files['solomon-darker'], createHash('sha256').update(bytes).digest('hex'));
  assert.equal(await readFile(join(result.applicationPath, 'game/assets/example.json'), 'utf8'), '{"asset":true}');
  assert.equal(await readFile(join(output, 'unrelated.txt'), 'utf8'), 'keep');
  assert.deepEqual(JSON.parse(await readFile(join(result.applicationPath, 'desktop-build.json'), 'utf8')), build);
  assert.deepEqual(JSON.parse(await readFile(join(output, 'desktop-release.json'), 'utf8')), build);
  for (const file of ['main.mjs', 'game/index.html']) {
    await assert.rejects(readFile(join(result.applicationPath, file)), {code: 'ENOENT'});
  }
});
