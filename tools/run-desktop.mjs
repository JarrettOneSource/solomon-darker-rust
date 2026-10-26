import { execFileSync, spawn } from 'node:child_process';
import { desktopArtifacts, repositoryRoot } from './desktop-build.mjs';
const artifacts = await desktopArtifacts({
  skipBuild: process.argv.includes('--skip-build'),
  nativeApp: process.env.SDR_DESKTOP_APP_BINARY,
  assetsRoot: process.env.SDR_DESKTOP_CLIENT_ROOT
});
const child = spawn(artifacts.executable, ['--assets', artifacts.assets], {
  env: {
    ...process.env,
    SDR_DESKTOP_BUILD_JSON: JSON.stringify({
      schemaVersion: 1,
      revision: execFileSync('git', ['rev-parse', 'HEAD'], {
        cwd: repositoryRoot,
        encoding: 'utf8'
      }).trim(),
      sourceTimestamp: 0,
      releaseRepository: process.env.SDR_DESKTOP_RELEASE_REPOSITORY || 'JarrettOneSource/solomon-darker-rust'
    })
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
