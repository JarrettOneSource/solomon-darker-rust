// One selected full journey. A hard diagnostic process cap only tears down this
// recorded child tree; the original gameplay/capture/close deadlines stay intact.
import assert from 'node:assert/strict';
import { execFile, spawn } from 'node:child_process';
import { appendFileSync } from 'node:fs';
import { mkdir, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';

const [name, mode] = process.argv.slice(2);
assert.ok(['playwright-1', 'electron', 'playwright-2', 'original-5c'].includes(name));
assert.ok(['playwright', 'electron', 'original'].includes(mode));
const evidence = resolve('reports/windows-capture-control', name);
await mkdir(evidence, {recursive: true});
const started = new Date().toISOString();
const env = {...process.env, SDR_DESKTOP_EVIDENCE: evidence};
if (mode === 'original') {
  delete env.SDR_CAPTURE_MODE;
  delete env.SDR_CAPTURE_PROTOCOL_LOG;
} else {
  env.SDR_CAPTURE_MODE = mode;
  env.SDR_CAPTURE_PROTOCOL_LOG = join(evidence, 'playwright-stages.jsonl');
}
const child = spawn(process.execPath, ['tools/smoke-desktop-runtime.mjs'], {
  env,
  stdio: ['ignore', 'pipe', 'pipe'],
});
const receipt = {name, mode, started, driverPid: child.pid, timedOut: false};
let cleanup = Promise.resolve();
let logBytes = 0;
for (const stream of [child.stdout, child.stderr]) stream.on('data', bytes => {
  const retained = bytes.subarray(0, Math.max(0, 262144 - logBytes));
  if (retained.length) {
    appendFileSync(join(evidence, 'driver.log'), retained);
    process.stdout.write(retained);
    logBytes += retained.length;
  }
});
const timer = setTimeout(() => {
  if (child.exitCode !== null || child.signalCode !== null) return;
  receipt.timedOut = true;
  cleanup = new Promise(resolveCleanup => execFile('taskkill.exe', ['/PID', String(child.pid), '/T', '/F'],
    {timeout: 10000, windowsHide: true}, error => {
      receipt.timeoutCleanup = error ? {status: 'failed', code: error.code} : {status: 'recorded-driver-tree-terminated'};
      if (error && child.exitCode === null && child.signalCode === null) child.kill('SIGKILL');
      resolveCleanup();
    }));
}, 12 * 60 * 1000);
const result = await new Promise(resolveExit => {
  child.once('error', error => resolveExit({spawnError: error.code}));
  child.once('close', (code, signal) => resolveExit({code, signal}));
});
clearTimeout(timer);
await cleanup;
Object.assign(receipt, result, {finished: new Date().toISOString(), retainedLogBytes: logBytes});
await writeFile(join(evidence, 'journey-process.json'), `${JSON.stringify(receipt, null, 2)}\n`);
console.log(JSON.stringify(receipt));
process.exitCode = !receipt.timedOut && result.code === 0 ? 0 : 1;
