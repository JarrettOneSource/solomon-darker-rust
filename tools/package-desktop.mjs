// Source tools/package-desktop.mjs, staging the Rust host and WASM client instead
// of Website wwwroot and the former TypeScript authority bundle.
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { createReadStream } from 'node:fs';
import { chmod, cp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { packager } from '@electron/packager';
import { stagePinnedNodeRuntime } from './stage-node-runtime.mjs';
import { desktopArtifacts, repositoryRoot } from './desktop-build.mjs';
const platform = argument('--platform');
const arch = argument('--arch');
const [toolPackage, desktopPackage] = await Promise.all([readJson(join(repositoryRoot, 'package.json')), readJson(join(repositoryRoot, 'desktop/package.json'))]);
const releaseRepository = argument('--release-repository', false) || process.env.SDR_DESKTOP_RELEASE_REPOSITORY;
if (!releaseRepository || !/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(releaseRepository)) {
  throw new Error('Set --release-repository OWNER/REPOSITORY for the Rust desktop update owner.');
}
const artifacts = await desktopArtifacts({
  platform,
  arch,
  release: true,
  skipBuild: process.argv.includes('--skip-build'),
  nativeHost: argument('--native-host', false),
  clientRoot: argument('--client-root', false)
});
const build = {
  schemaVersion: 1,
  revision: execFileSync('git', ['rev-parse', 'HEAD'], {
    cwd: repositoryRoot,
    encoding: 'utf8'
  }).trim(),
  sourceTimestamp: Number(execFileSync('git', ['show', '-s', '--format=%ct', 'HEAD'], {
    cwd: repositoryRoot,
    encoding: 'utf8'
  }).trim()),
  releaseRepository
};
const electronVersion = toolPackage.devDependencies.electron;
const stage = join(repositoryRoot, '.desktop-stage', `${platform}-${arch}`);
const output = join(repositoryRoot, 'dist-desktop');
await rm(stage, {
  force: true,
  recursive: true
});
await mkdir(join(stage, 'client', '__desktop'), {
  recursive: true
});
await mkdir(join(stage, 'authority'), {
  recursive: true
});
const authorityFile = platform === 'win32' ? 'solomon-darker.exe' : 'solomon-darker';
await Promise.all([...['main.mjs', 'preload.cjs', 'static-client-server.mjs', 'local-game-host.mjs', 'runtime-service.mjs', 'local-peer.mjs', 'run-peer-service.mjs', 'peer-network.mjs', 'updates.mjs', 'package.json'].map(file => cp(join(repositoryRoot, 'desktop', file), join(stage, file))), cp(artifacts.client, join(stage, 'client'), {
  recursive: true
}), cp(artifacts.executable, join(stage, 'authority', authorityFile)), writeFile(join(stage, 'desktop-build.json'), `${JSON.stringify(build, null, 2)}\n`)]);
if (platform !== 'win32') await chmod(join(stage, 'authority', authorityFile), 0o755);
await Promise.all(['launcher.html', 'launcher.css', 'launcher.mjs'].map(file => cp(join(repositoryRoot, 'desktop', file), join(stage, 'client', '__desktop', file))));
const runtime = await stagePinnedNodeRuntime({
  platform,
  arch,
  destination: join(stage, 'runtime')
});
const applicationPaths = await packager({
  appVersion: desktopPackage.version,
  appBundleId: 'com.jarrett.solomondarker',
  arch,
  asar: false,
  dir: stage,
  electronVersion,
  executableName: 'solomon-darker',
  name: 'Solomon Darker',
  out: output,
  overwrite: true,
  platform,
  ...(platform === 'darwin' ? {
    osxSign: {
      identity: '-',
      identityValidation: false,
      preAutoEntitlements: false,
      preEmbedProvisioningProfile: false,
      optionsForFile: () => ({
        hardenedRuntime: false,
        timestamp: 'none'
      })
    }
  } : {}),
  prune: false
});
if (applicationPaths.length !== 1) throw new Error('Desktop packager produced an unexpected output set');
const applicationPath = applicationPaths[0];
const applicationResources = join(applicationPath, platform === 'darwin'
  ? 'Solomon Darker.app/Contents/Resources/app' : 'resources/app');
const executable = platform === 'darwin' ? 'Solomon Darker.app/Contents/MacOS/solomon-darker' : platform === 'win32' ? 'solomon-darker.exe' : 'solomon-darker';
const manifest = {
  ...build,
  arch,
  electronVersion,
  executable,
  platform,
  nativeAuthority: {
    // macOS signing changes the copied Mach-O after staging.
    sha256: await fileHash(join(applicationResources, 'authority', authorityFile)),
    executable: `authority/${authorityFile}`
  },
  nodeRuntime: {
    sha256: runtime.sha256,
    version: runtime.version
  }
};
await writeFile(join(applicationPath, 'desktop-package-manifest.json'), `${JSON.stringify(manifest, null, 2)}\n`);
await writeFile(join(output, 'desktop-release.json'), `${JSON.stringify(build, null, 2)}\n`);
process.stdout.write(`${JSON.stringify({
  ...manifest,
  applicationPath
})}\n`);
async function readJson(path) {
  return JSON.parse(await readFile(path, 'utf8'));
}
function argument(name, required = true) {
  const index = process.argv.indexOf(name);
  const value = index < 0 ? undefined : process.argv[index + 1];
  if (!value || value.startsWith('--')) {
    if (required || index >= 0) throw new Error(`${name} requires a value`);
    return undefined;
  }
  return name === '--native-host' || name === '--client-root' ? resolve(value) : value;
}
async function fileHash(path) {
  const hash = createHash('sha256');
  for await (const chunk of createReadStream(path)) hash.update(chunk);
  return hash.digest('hex');
}
