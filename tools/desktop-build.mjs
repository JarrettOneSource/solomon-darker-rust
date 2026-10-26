// Native application and tracked assets used by desktop development and packaging.
import { spawn } from 'node:child_process';
import { access } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
export const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const targets = new Map([['linux-x64', 'x86_64-unknown-linux-gnu'], ['darwin-x64', 'x86_64-apple-darwin'], ['darwin-arm64', 'aarch64-apple-darwin'], ['win32-x64', 'x86_64-pc-windows-msvc']]);
export function desktopLayout(platform, arch) {
  if (!targets.has(`${platform}-${arch}`)) throw new Error('Supported desktop targets: Windows x64, macOS x64/arm64 and Linux x64.');
  return {
    directory: `Solomon Darker-${platform}-${arch}`,
    executable: platform === 'darwin' ? 'Solomon Darker.app/Contents/MacOS/solomon-darker'
      : platform === 'win32' ? 'solomon-darker.exe' : 'solomon-darker',
    resources: platform === 'darwin' ? 'Solomon Darker.app/Contents/Resources' : '.'
  };
}
export async function desktopArtifacts({
  platform = process.platform,
  arch = process.arch,
  release = false,
  skipBuild = false,
  nativeApp,
  assetsRoot
} = {}) {
  const target = targets.get(`${platform}-${arch}`);
  if (!target) throw new Error('Supported desktop targets: Windows x64, macOS x64/arm64 and Linux x64.');
  const crossTarget = platform !== process.platform || arch !== process.arch;
  const cargoTarget = resolve(process.env.CARGO_TARGET_DIR || join(repositoryRoot, 'target'));
  const executable = resolve(nativeApp || join(cargoTarget, ...(crossTarget ? [target] : []), release ? 'release' : 'debug', platform === 'win32' ? 'solomon-darker-app.exe' : 'solomon-darker-app'));
  const assets = resolve(assetsRoot || join(repositoryRoot, 'web'));
  if (!skipBuild && !nativeApp) {
    await run('cargo', ['build', '--locked', '-p', 'solomon-desktop', '--bin', 'solomon-darker-app', ...(release ? ['--release'] : []), ...(crossTarget ? ['--target', target] : [])]);
  }
  await Promise.all([access(executable), ...['stock/College.bundle', 'stock-files.json', 'boneyards.json']
    .map(file => access(join(assets, 'assets', file)))]);
  return { executable, assets };
}
export function run(command, args, cwd = repositoryRoot) {
  return new Promise((resolveRun, reject) => {
    const child = spawn(command, args, {
      cwd,
      stdio: 'inherit',
      env: {
        ...process.env,
        CARGO_INCREMENTAL: process.env.CARGO_INCREMENTAL || '0',
        CARGO_PROFILE_DEV_DEBUG: process.env.CARGO_PROFILE_DEV_DEBUG || '0',
        CARGO_PROFILE_TEST_DEBUG: process.env.CARGO_PROFILE_TEST_DEBUG || '0'
      }
    });
    child.once('error', reject);
    child.once('exit', (code, signal) => {
      if (code === 0) resolveRun();else reject(new Error(`${command} failed (${code ?? signal ?? 'unknown'})`));
    });
  });
}
