// Native/WASM artifacts used by both desktop development and packaging. The shell
// and peer service remain Node/Electron; gameplay always comes from this Rust tree.
import { spawn } from 'node:child_process';
import { access } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
export const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const targets = new Map([['linux-x64', 'x86_64-unknown-linux-gnu'], ['darwin-x64', 'x86_64-apple-darwin'], ['darwin-arm64', 'aarch64-apple-darwin'], ['win32-x64', 'x86_64-pc-windows-msvc']]);
export async function desktopArtifacts({
  platform = process.platform,
  arch = process.arch,
  release = false,
  skipBuild = false,
  nativeHost,
  clientRoot
} = {}) {
  const target = targets.get(`${platform}-${arch}`);
  if (!target) throw new Error('Supported desktop targets: Windows x64, macOS x64/arm64 and Linux x64.');
  const crossTarget = platform !== process.platform || arch !== process.arch;
  const cargoTarget = resolve(process.env.CARGO_TARGET_DIR || join(repositoryRoot, 'target'));
  const executable = resolve(nativeHost || join(cargoTarget, ...(crossTarget ? [target] : []), release ? 'release' : 'debug', platform === 'win32' ? 'solomon-darker.exe' : 'solomon-darker'));
  const client = resolve(clientRoot || join(repositoryRoot, 'web', 'dist'));
  if (!skipBuild) {
    if (!nativeHost) {
      await run('cargo', ['build', '-p', 'solomon-launcher', '--bin', 'solomon-darker', ...(release ? ['--release'] : []), ...(crossTarget ? ['--target', target] : [])]);
    }
    if (!clientRoot) await run('trunk', ['build', '--public-url', '/', ...(release ? ['--release'] : [])], join(repositoryRoot, 'web'));
  }
  await Promise.all([access(executable), access(join(client, 'index.html'))]);
  await run(process.env.PYTHON || (process.platform === 'win32' ? 'python' : 'python3'),
    [join(repositoryRoot, 'scripts', 'prepare-desktop-client.py'), client]);
  return {
    executable,
    client
  };
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
