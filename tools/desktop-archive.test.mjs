import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import test from 'node:test';
import { desktopLayout, repositoryRoot } from './desktop-build.mjs';

const python = process.env.PYTHON || (process.platform === 'win32' ? 'python' : 'python3');
const revision = 'a'.repeat(40);
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const zipWriter = `import json,sys,zipfile,base64
with zipfile.ZipFile(sys.argv[1], 'w', compression=zipfile.ZIP_STORED) as archive:
    for entry in json.load(sys.stdin):
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
  context.after(() => rm(directory, {recursive: true, force: true}));
  const source = join(directory, 'source');
  const zip = join(directory, 'desktop.zip');
  const layout = desktopLayout(platform, arch);
  const root = layout.directory + '/';
  const resources = root + (layout.resources === '.' ? '' : layout.resources + '/');
  const entries = new Map();
  const add = (name, bytes, mode = name.endsWith('/') ? 0o40755 : 0o100644) =>
    entries.set(name, {name, bytes: Buffer.from(bytes).toString('base64'), mode});
  const copy = async (path, destination, bytes) => {
    await mkdir(dirname(join(source, path)), {recursive: true});
    await writeFile(join(source, path), bytes);
    add(resources + destination, bytes);
  };
  add(root, '');
  const stock = '{"title":"College"}\n';
  await copy('web/assets/stock/tile.json', 'game/assets/stock/tile.json', stock);
  await copy('web/assets/stock-files.json', 'game/assets/stock-files.json', JSON.stringify({'tile.json': digest(stock)}));
  await copy('web/assets/boneyards.json', 'game/assets/boneyards.json', '[]');
  await copy('docs/third-party/codec-LICENSE', 'licenses/codec-LICENSE', 'licensed-code');
  add(root + layout.executable, 'native-application', 0o100755);
  const build = {schemaVersion: 1, revision, sourceTimestamp: 123, releaseRepository: 'owner/repository'};
  add(resources + 'desktop-build.json', JSON.stringify(build));
  if (platform === 'darwin') {
    add(root + 'Solomon Darker.app/Contents/Info.plist', '<plist version="1.0"><dict><key>CFBundleExecutable</key><string>solomon-darker</string><key>CFBundleIdentifier</key><string>com.jarrett.solomondarker</string><key>CFBundlePackageType</key><string>APPL</string></dict></plist>');
    add(root + 'Solomon Darker.app/Contents/_CodeSignature/CodeResources', 'signature-crc-payload');
  }
  const manifest = {...build, schemaVersion: 2, runtime: 'native', platform, arch,
    executable: layout.executable, resources: layout.resources, files: {}};
  for (const [name, entry] of entries) {
    if (!name.endsWith('/')) manifest.files[name.slice(root.length)] = digest(Buffer.from(entry.bytes, 'base64'));
  }
  add(root + 'desktop-package-manifest.json', JSON.stringify(manifest));
  return {root, resources, zip, source, add, entries, manifest,
    declare(name, bytes) {
      add(root + name, bytes);
      manifest.files[name] = digest(bytes);
      add(root + 'desktop-package-manifest.json', JSON.stringify(manifest));
    },
    write() {
      execFileSync(python, ['-c', zipWriter, zip], {input: JSON.stringify([...entries.values()])});
    },
    verify() {
      return spawnSync(python, ['-c', verifyCommand, join(repositoryRoot, 'scripts/verify-desktop-archive.py'),
        zip, source, revision], {encoding: 'utf8', env: {...process.env, PYTHONDONTWRITEBYTECODE: '1'}});
    }
  };
}
test('archive creation rejects the actual final ZIP when it contains a Trunk lock', async context => {
  const root = await mkdtemp(join(tmpdir(), 'solomon-desktop-archive-'));
  context.after(() => rm(root, { recursive: true, force: true }));
  const target = `${process.platform}-${process.arch}`;
  const client = join(root, 'dist-desktop', `Solomon Darker-${target}`, 'game');
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

test('the native archive contract accepts all four platform layouts', async context => {
  for (const [platform, arch] of [['linux', 'x64'], ['win32', 'x64'], ['darwin', 'x64'], ['darwin', 'arm64']]) {
    const fixture = await archiveFixture(context, platform, arch);
    fixture.write();
    const result = fixture.verify();
    assert.equal(result.status, 0, result.stderr);
    assert.equal(JSON.parse(result.stdout).runtime, 'native');
    assert.equal(JSON.parse(result.stdout).stockFiles, 1);
  }
});

test('CRC validation reads signed bundle metadata as well as native and asset files', async context => {
  const fixture = await archiveFixture(context, 'darwin', 'arm64');
  fixture.write();
  const bytes = await readFile(fixture.zip);
  const offset = bytes.indexOf(Buffer.from('signature-crc-payload'));
  assert.ok(offset >= 0);
  bytes[offset] ^= 1;
  await writeFile(fixture.zip, bytes);
  assert.match(fixture.verify().stderr, /CRC/);
});

test('changed native bytes fail the final file manifest', async context => {
  const fixture = await archiveFixture(context);
  fixture.add(fixture.root + fixture.manifest.executable, 'changed executable');
  fixture.write();
  assert.match(fixture.verify().stderr, /Package hash differs.*solomon-darker/);
});

test('a self-consistent manifest cannot hide changed source assets or licenses', async context => {
  for (const name of ['game/assets/stock/tile.json', 'game/assets/boneyards.json', 'licenses/codec-LICENSE']) {
    const fixture = await archiveFixture(context);
    fixture.declare(name, 'changed bytes');
    fixture.write();
    assert.match(fixture.verify().stderr, /Source asset differs|License differs/, name);
  }
});

test('undeclared payload and legacy browser or Node runtime files are rejected', async context => {
  for (const name of ['main.mjs', 'game/index.html', 'game/solomon-web.wasm', 'runtime/node',
    'resources.pak', 'Electron Framework.framework/runtime']) {
    const fixture = await archiveFixture(context);
    fixture.declare(name, 'retired-runtime');
    fixture.write();
    assert.match(fixture.verify().stderr, /Unexpected native package file/, name);
  }
  const fixture = await archiveFixture(context);
  fixture.add(fixture.root + 'unexpected.bin', 'unreviewed');
  fixture.write();
  assert.match(fixture.verify().stderr, /Package membership differs/);
});

test('stock additions, build residue and stale revision cannot qualify', async context => {
  for (const [name, error] of [['game/assets/stock/extra.png', /Source asset membership differs/],
    ['session.log', /Build residue/], ['game/mods/default.sdmod', /Build residue/]]) {
    const fixture = await archiveFixture(context);
    fixture.declare(name, 'unreviewed');
    fixture.write();
    assert.match(fixture.verify().stderr, error);
  }
  const fixture = await archiveFixture(context);
  fixture.add(fixture.root + 'desktop-package-manifest.json', JSON.stringify({...fixture.manifest, revision: 'b'.repeat(40)}));
  fixture.write();
  assert.match(fixture.verify().stderr, /Package revision differs/);
});

test('native bundles reject symlinks and keep only valid macOS resource forks', async context => {
  const fixture = await archiveFixture(context, 'darwin', 'arm64');
  fixture.add('__MACOSX/' + fixture.resources + '._desktop-build.json', Buffer.from([0, 5, 22, 7, 0, 0, 0, 0]));
  fixture.write();
  assert.equal(fixture.verify().status, 0);
  fixture.add(fixture.root + fixture.manifest.executable, '/outside', 0o120777);
  fixture.write();
  assert.match(fixture.verify().stderr, /Native package contains a symlink/);
});
