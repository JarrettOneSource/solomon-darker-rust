import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';

import { repositoryRoot } from './desktop-build.mjs';

const python = process.env.PYTHON || (process.platform === 'win32' ? 'python' : 'python3');
const revision = 'a'.repeat(40);
const shellFiles = ['main.mjs', 'preload.cjs', 'static-client-server.mjs', 'local-game-host.mjs',
  'runtime-service.mjs', 'local-peer.mjs', 'run-peer-service.mjs', 'peer-network.mjs', 'updates.mjs', 'package.json'];
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const zipWriter = `import json,sys,zipfile
with zipfile.ZipFile(sys.argv[1], 'w', compression=zipfile.ZIP_STORED) as archive:
    for entry in json.load(sys.stdin):
        import base64
        info = zipfile.ZipInfo(entry['name'])
        info.create_system = 3
        info.external_attr = entry['mode'] << 16
        archive.writestr(info, base64.b64decode(entry['bytes']))
`;
const verifyCommand = `import json,runpy,sys
from pathlib import Path
verify = runpy.run_path(sys.argv[1])['verify_archive']
print(json.dumps(verify(Path(sys.argv[2]), Path(sys.argv[3]), sys.argv[4])))
`;

async function archiveFixture(context, platform = 'linux', arch = 'x64') {
  const directory = await mkdtemp(join(tmpdir(), 'solomon-archive-fixture-'));
  context.after(() => rm(directory, { recursive: true, force: true }));
  const source = join(directory, 'source');
  const zip = join(directory, 'desktop.zip');
  const root = `Solomon Darker-${platform}-${arch}/`;
  const app = root + (platform === 'darwin' ? 'Solomon Darker.app/Contents/Resources/app/' : 'resources/app/');
  const entries = new Map();
  const add = (name, bytes, mode = name.endsWith('/') ? 0o40755 : 0o100644) =>
    entries.set(name, { name, bytes: Buffer.from(bytes).toString('base64'), mode });
  await mkdir(join(source, 'desktop'), { recursive: true });
  await mkdir(join(source, 'web', 'assets'), { recursive: true });
  add(root, '');
  for (const name of shellFiles) {
    const bytes = name === 'package.json'
      ? JSON.stringify({ name: 'fixture', private: true, main: 'main.mjs' }) + '\n'
      : `// reviewed ${name}\n`;
    await writeFile(join(source, 'desktop', name), bytes);
    const destination = app + name;
    add(destination, name === 'package.json' ? JSON.stringify({ name: 'fixture', main: 'main.mjs' }) : bytes);
  }
  const stock = '{"title":"College"}\n';
  await writeFile(join(source, 'web', 'assets', 'stock-files.json'), JSON.stringify({ 'tile.json': digest(stock) }));
  add(app + 'client/assets/stock/tile.json', stock);
  add(root + 'resources.pak', 'runtime-crc-payload');
  const native = 'authority/solomon-darker' + (platform === 'win32' ? '.exe' : '');
  add(app + native, 'native-authority');
  const build = { schemaVersion: 1, revision, sourceTimestamp: 123, releaseRepository: 'owner/repository' };
  const manifest = { ...build, platform, arch, nativeAuthority: { executable: native, sha256: digest('native-authority') } };
  add(app + 'desktop-build.json', JSON.stringify(build));
  add(root + 'desktop-package-manifest.json', JSON.stringify(manifest));
  return { root, app, zip, source, add, entries, manifest,
    write() {
      execFileSync(python, ['-c', zipWriter, zip], { input: JSON.stringify([...entries.values()]) });
    },
    verify() {
      return spawnSync(python, ['-c', verifyCommand, join(repositoryRoot, 'scripts/verify-desktop-archive.py'),
        zip, source, revision], { encoding: 'utf8', env: { ...process.env, PYTHONDONTWRITEBYTECODE: '1' } });
    }
  };
}

test('archive creation rejects the actual final ZIP when it contains a Trunk lock', async context => {
  const root = await mkdtemp(join(tmpdir(), 'solomon-desktop-archive-'));
  context.after(() => rm(root, { recursive: true, force: true }));
  const target = `${process.platform}-${process.arch}`;
  const client = join(root, 'dist-desktop', `Solomon Darker-${target}`, 'resources', 'app', 'client');
  await mkdir(client, { recursive: true });
  await writeFile(join(client, '.trunk-lock'), '');
  const result = spawnSync(process.execPath, [join(repositoryRoot, 'tools/archive-desktop.mjs')], {
    cwd: root, encoding: 'utf8', env: { ...process.env, PYTHONDONTWRITEBYTECODE: '1' }
  });
  assert.notEqual(result.status, 0, 'an invalid final ZIP must stop the archive/upload step');
  assert.match(result.stdout + result.stderr, /[Bb]uild residue:.*\.trunk-lock/);
  assert.ok((await readFile(join(root, 'dist-desktop', `Solomon-Darker-${target}.zip`))).length > 0,
    'the gate examines the final archive, leaving the rejected candidate available for diagnosis');
});

test('a repeated archive is a fresh snapshot without removed entries', async context => {
  const root = await mkdtemp(join(tmpdir(), 'solomon-archive-replacement-'));
  context.after(() => rm(root, { recursive: true, force: true }));
  const target = `${process.platform}-${process.arch}`;
  const directory = `Solomon Darker-${target}`;
  const output = join(root, 'dist-desktop', `Solomon-Darker-${target}.zip`);
  await mkdir(join(root, 'dist-desktop', directory), { recursive: true });
  await writeFile(join(root, 'dist-desktop', directory, 'current.txt'), 'current');
  await writeFile(join(root, 'dist-desktop', 'unrelated.zip'), 'keep');
  execFileSync(python, ['-c', zipWriter, output], { input: JSON.stringify([
    { name: directory + '/removed.txt', bytes: Buffer.from('old').toString('base64'), mode: 0o100644 }
  ]) });
  // This deliberately incomplete candidate is rejected by the later gate; its
  // actual ZIP must still be an exact snapshot of the current package directory.
  spawnSync(process.execPath, [join(repositoryRoot, 'tools/archive-desktop.mjs')], {
    cwd: root, encoding: 'utf8', env: { ...process.env, PYTHONDONTWRITEBYTECODE: '1' }
  });
  const names = JSON.parse(execFileSync(python, ['-c',
    'import json,sys,zipfile; print(json.dumps(zipfile.ZipFile(sys.argv[1]).namelist()))', output], { encoding: 'utf8' }));
  assert.ok(names.includes(directory + '/current.txt'));
  assert.equal(names.includes(directory + '/removed.txt'), false);
  assert.equal(await readFile(join(root, 'dist-desktop', 'unrelated.zip'), 'utf8'), 'keep');
});

test('the final archive contract accepts every desktop layout and npm package metadata', async context => {
  for (const [platform, arch] of [['linux', 'x64'], ['win32', 'x64'], ['darwin', 'x64'], ['darwin', 'arm64']]) {
    const fixture = await archiveFixture(context, platform, arch);
    fixture.write();
    const result = fixture.verify();
    assert.equal(result.status, 0, result.stderr);
    assert.equal(JSON.parse(result.stdout).sourceFiles, 10);
    assert.equal(JSON.parse(result.stdout).stockFiles, 1);
  }
});

test('CRC validation also reads runtime files without a declared source hash', async context => {
  const fixture = await archiveFixture(context);
  fixture.write();
  const bytes = await readFile(fixture.zip);
  const offset = bytes.indexOf(Buffer.from('runtime-crc-payload'));
  assert.ok(offset >= 0);
  bytes[offset] ^= 1;
  await writeFile(fixture.zip, bytes);
  const result = fixture.verify();
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /CRC/);
});

test('valid ZIP CRCs cannot hide changed stock, source-owned shell or native bytes', async context => {
  for (const [path, error] of [['client/assets/stock/tile.json', /Stock hash differs/],
    ['main.mjs', /Source-owned shell differs/], ['authority/solomon-darker', /Native authority hash differs/]]) {
    const fixture = await archiveFixture(context);
    fixture.add(fixture.app + path, 'changed bytes');
    fixture.write();
    const result = fixture.verify();
    assert.notEqual(result.status, 0, path);
    assert.match(result.stderr, error);
  }
});

test('Windows text checkout normalization never changes the exact stock hash contract', async context => {
  const fixture = await archiveFixture(context, 'win32');
  for (const [name, entry] of fixture.entries) {
    if (name.endsWith('.mjs') || name.endsWith('.cjs') || name.endsWith('.html') || name.endsWith('.css')) {
      fixture.add(name, Buffer.from(entry.bytes, 'base64').toString().replaceAll('\n', '\r\n'));
    }
  }
  fixture.write();
  assert.equal(fixture.verify().status, 0);
  fixture.add(fixture.app + 'client/assets/stock/tile.json', '{"title":"College"}\r\n');
  fixture.write();
  const result = fixture.verify();
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /Stock hash differs/);
});

test('macOS resource forks and framework symlink chains remain valid package members', async context => {
  const fixture = await archiveFixture(context, 'darwin', 'arm64');
  const framework = fixture.root + 'Solomon Darker.app/Contents/Frameworks/Runtime.framework/';
  fixture.add(framework + 'Versions/A/Runtime', 'framework bytes');
  fixture.add(framework + 'Versions/Current', 'A', 0o120777);
  fixture.add(framework + 'Runtime', 'Versions/Current/Runtime', 0o120777);
  fixture.add('__MACOSX/' + fixture.app + '._main.mjs', Buffer.from([0, 5, 22, 7, 0, 0, 0, 0]));
  fixture.write();
  assert.equal(fixture.verify().status, 0);
  fixture.add(framework + 'Versions/Current', '../../../../../../outside', 0o120777);
  fixture.write();
  const result = fixture.verify();
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /Symlink escapes/);
});

test('extra stock, generated logs, default mods and stale revisions are rejected', async context => {
  for (const [path, error] of [['client/assets/stock/extra.png', /Stock membership differs/],
    ['session.log', /Build residue/], ['client/mods/default.sdmod', /Build residue/]]) {
    const fixture = await archiveFixture(context);
    fixture.add(fixture.app + path, 'unreviewed');
    fixture.write();
    assert.match(fixture.verify().stderr, error);
  }
  const fixture = await archiveFixture(context);
  fixture.add(fixture.root + 'desktop-package-manifest.json', JSON.stringify({ ...fixture.manifest, revision: 'b'.repeat(40) }));
  fixture.write();
  assert.match(fixture.verify().stderr, /Package revision differs/);
});

test('a final package cannot retain the removed HTML launcher', async context => {
  const fixture = await archiveFixture(context);
  fixture.add(fixture.app + 'client/__desktop/launcher.html', '<button>Play offline</button>');
  fixture.write();
  const result = fixture.verify();
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /Retired visual desktop launcher/);
});
