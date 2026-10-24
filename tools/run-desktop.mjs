import { execFileSync, spawn } from 'node:child_process';
import { join } from 'node:path';
import electron from 'electron';
import { desktopArtifacts, repositoryRoot } from './desktop-build.mjs';
const artifacts = await desktopArtifacts({
  skipBuild: process.argv.includes('--skip-build'),
  nativeHost: process.env.SDR_DESKTOP_GAME_HOST,
  clientRoot: process.env.SDR_DESKTOP_CLIENT_ROOT
});
const child = spawn(electron, [join(repositoryRoot, 'desktop')], {
  env: {
    ...process.env,
    SDR_DESKTOP_BUILD_JSON: JSON.stringify({
      schemaVersion: 1,
      revision: execFileSync('git', ['rev-parse', 'HEAD'], {
        cwd: repositoryRoot,
        encoding: 'utf8'
      }).trim(),
      sourceTimestamp: 0,
      ...(process.env.SDR_DESKTOP_RELEASE_REPOSITORY ? {
        releaseRepository: process.env.SDR_DESKTOP_RELEASE_REPOSITORY
      } : {})
    }),
    SDR_DESKTOP_CLIENT_ROOT: artifacts.client,
    SDR_DESKTOP_GAME_HOST: artifacts.executable,
    SDR_DESKTOP_NODE: process.execPath
  },
  stdio: 'inherit'
});
process.once('SIGINT', () => child.kill('SIGTERM'));
process.once('SIGTERM', () => child.kill('SIGTERM'));
const exit = await new Promise((resolveExit, reject) => {
  child.once('error', reject);
  child.once('exit', (code, signal) => resolveExit({
    code,
    signal
  }));
});
process.exitCode = exit.code ?? (exit.signal ? 1 : 0);
