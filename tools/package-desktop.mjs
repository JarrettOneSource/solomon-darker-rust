// Package the native Rust application and its data. Node is a build tool only.
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { createReadStream } from 'node:fs';
import { chmod, cp, mkdir, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { desktopArtifacts, desktopLayout, repositoryRoot, run } from './desktop-build.mjs';

export async function packageDesktop({platform, arch, executable, assets, build, version,
  output = join(repositoryRoot, 'dist-desktop')}) {
  const layout = desktopLayout(platform, arch);
  if (platform === 'darwin' && process.platform !== 'darwin') {
    throw new Error('macOS packages must be signed on macOS.');
  }
  const applicationPath = join(output, layout.directory);
  const resources = join(applicationPath, layout.resources);
  const destination = join(applicationPath, layout.executable);
  await rm(applicationPath, {recursive: true, force: true});
  await mkdir(dirname(destination), {recursive: true});
  await mkdir(join(resources, 'game'), {recursive: true});
  await cp(executable, destination);
  if (platform !== 'win32') await chmod(destination, 0o755);
  await cp(join(assets, 'assets'), join(resources, 'game', 'assets'), {recursive: true});
  await cp(join(repositoryRoot, 'docs', 'third-party'), join(resources, 'licenses'), {recursive: true});
  await writeJson(join(resources, 'desktop-build.json'), build);
  if (platform === 'darwin') {
    if (!/^\d+\.\d+\.\d+$/.test(version)) throw new Error('The application version must have three numeric components.');
    const bundle = join(applicationPath, 'Solomon Darker.app');
    await writeFile(join(bundle, 'Contents', 'Info.plist'), `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>solomon-darker</string>
<key>CFBundleIdentifier</key><string>com.jarrett.solomondarker</string>
<key>CFBundleName</key><string>Solomon Darker</string>
<key>CFBundleDisplayName</key><string>Solomon Darker</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>${version}</string>
<key>CFBundleVersion</key><string>${version}</string>
<key>NSHighResolutionCapable</key><true/>
<key>NSPrincipalClass</key><string>NSApplication</string>
</dict></plist>
`);
    await run('codesign', ['--force', '--sign', '-', bundle]);
    await run('codesign', ['--verify', '--strict', bundle]);
  }
  // Signing changes the Mach-O and creates CodeResources. Hash only final bytes.
  const manifest = {...build, schemaVersion: 2, runtime: 'native', platform, arch,
    executable: layout.executable, resources: layout.resources, files: await hashes(applicationPath)};
  await writeJson(join(applicationPath, 'desktop-package-manifest.json'), manifest);
  await writeJson(join(output, 'desktop-release.json'), build);
  return {...manifest, applicationPath};
}

async function hashes(directory, prefix = '') {
  const result = {};
  for (const entry of (await readdir(directory, {withFileTypes: true})).sort((a, b) => a.name.localeCompare(b.name))) {
    const name = prefix + entry.name;
    const path = join(directory, entry.name);
    if (entry.isDirectory()) Object.assign(result, await hashes(path, name + '/'));
    else if (entry.isFile()) {
      const hash = createHash('sha256');
      for await (const chunk of createReadStream(path)) hash.update(chunk);
      result[name] = hash.digest('hex');
    } else throw new Error(`Native packages require regular files: ${name}`);
  }
  return result;
}

async function writeJson(path, value) {
  await writeFile(path, JSON.stringify(value, null, 2) + '\n');
}

function argument(name) {
  const index = process.argv.indexOf(name);
  if (index < 0) return undefined;
  const value = process.argv[index + 1];
  if (!value || value.startsWith('--')) throw new Error(`${name} requires a value`);
  return value;
}

async function main() {
  const platform = argument('--platform') || process.platform;
  const arch = argument('--arch') || process.arch;
  const releaseRepository = argument('--release-repository') || process.env.SDR_DESKTOP_RELEASE_REPOSITORY;
  if (!releaseRepository || !/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(releaseRepository)) {
    throw new Error('Set --release-repository OWNER/REPOSITORY for the Rust desktop update owner.');
  }
  const artifacts = await desktopArtifacts({platform, arch, release: true,
    skipBuild: process.argv.includes('--skip-build'), nativeApp: argument('--native-app'),
    assetsRoot: argument('--assets-root')});
  const build = {
    schemaVersion: 1,
    revision: execFileSync('git', ['rev-parse', 'HEAD'], {cwd: repositoryRoot, encoding: 'utf8'}).trim(),
    sourceTimestamp: Number(execFileSync('git', ['show', '-s', '--format=%ct', 'HEAD'],
      {cwd: repositoryRoot, encoding: 'utf8'}).trim()),
    releaseRepository
  };
  const cargo = await readFile(join(repositoryRoot, 'Cargo.toml'), 'utf8');
  const version = /^version = "(\d+\.\d+\.\d+)"$/m.exec(cargo)?.[1];
  if (!version) throw new Error('Cargo.toml has no application version.');
  const result = await packageDesktop({platform, arch, ...artifacts, build, version});
  process.stdout.write(JSON.stringify({applicationPath: result.applicationPath, executable: result.executable,
    revision: result.revision, files: Object.keys(result.files).length}) + '\n');
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) await main();
