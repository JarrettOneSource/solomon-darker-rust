// Temporary diagnostic only. The package stays exact 5c; each full journey
// selects one capture API before launch and gets one 30 s font + capture budget.
import assert from 'node:assert/strict';
import { appendFileSync, statSync } from 'node:fs';
import { readFile } from 'node:fs/promises';
import { join } from 'node:path';

const now = () => Number(process.hrtime.bigint()) / 1e6;

export function createWindowsCaptureControl(evidence) {
  const mode = process.env.SDR_CAPTURE_MODE;
  assert.ok(['playwright', 'electron'].includes(mode), 'choose one diagnostic capture path before launch');
  const journal = join(evidence, 'capture-control.jsonl');
  const log = row => {
    const line = `${JSON.stringify({wallTime: new Date().toISOString(), monotonicMs: now(), mode, ...row})}\n`;
    let bytes = 0;
    try { bytes = statSync(journal).size; }
    catch (error) { if (error.code !== 'ENOENT') throw error; }
    if (bytes + Buffer.byteLength(line) <= 65536) appendFileSync(journal, line);
  };
  log({stage: 'control-created', viewport: 'unchanged', captureBudgetMs: 30000});

  return {
    async observePair(stage, instances) {
      // These identical probes can affect scheduling. A passing probed PW
      // baseline does not by itself prove that the native API fixes the failure.
      await Promise.all(instances.map(async instance => {
        const started = now();
        const observations = [
          ['electron-window', () => instance.app.evaluate(({BrowserWindow}) => {
            const window = BrowserWindow.getAllWindows()[0];
            return {visible: window.isVisible(), minimized: window.isMinimized(), focused: window.isFocused(),
              bounds: window.getBounds()};
          })],
          ['document', () => instance.page.evaluate(() => ({visibilityState: document.visibilityState,
            fontStatus: document.fonts.status, width: innerWidth, height: innerHeight}))],
        ];
        await Promise.all(observations.map(async ([owner, operation]) => {
          try {
            const value = await limited(operation(), 5000, `${stage} ${owner} observation timed out`);
            log({stage, label: instance.label, owner, elapsedMs: now() - started, value,
              viewport: instance.page.viewportSize()});
          } catch (error) {
            // Observations cannot replace or relax a gameplay/resource gate.
            log({stage, label: instance.label, owner, elapsedMs: now() - started, error: error.name});
          }
        }));
      }));
    },

    async capture(instance, filename) {
      const started = now();
      const deadline = started + 30000;
      const path = join(evidence, filename);
      const remaining = () => {
        const value = deadline - now();
        if (value <= 0) throw new Error(`${instance.label} ${mode} font and capture budget expired`);
        return value;
      };
      const stage = async (name, operation) => {
        const began = now();
        log({stage: name, event: 'begin', label: instance.label, elapsedMs: began - started});
        try {
          const timeout = remaining();
          const value = await limited(operation(), timeout, `${instance.label} ${name} exceeded the shared 30000 ms capture budget`);
          remaining();
          log({stage: name, event: 'end', label: instance.label, elapsedMs: now() - started, durationMs: now() - began});
          return value;
        } catch (error) {
          log({stage: name, event: 'failed', label: instance.label, elapsedMs: now() - started, durationMs: now() - began,
            error: error.name});
          throw error;
        }
      };
      log({stage: 'capture', event: 'begin', label: instance.label, budgetMs: 30000});
      const fontStatus = await stage('document.fonts.ready', () => instance.page.evaluate(async () => {
        await document.fonts.ready;
        return document.fonts.status;
      }));
      assert.equal(fontStatus, 'loaded', 'the selected capture waits for the actual document fonts');
      if (mode === 'playwright') {
        await stage('playwright.screenshot', () => instance.page.screenshot({path, timeout: remaining()}));
      } else {
        const size = await stage('electron.capturePage', () => instance.app.evaluate(async ({BrowserWindow}, path) => {
          const window = BrowserWindow.getAllWindows()[0];
          const image = await window.webContents.capturePage({x: 0, y: 0, width: 0, height: 0},
            {stayHidden: true, stayAwake: false});
          const size = image.getSize();
          if (image.isEmpty() || size.width <= 0 || size.height <= 0) throw new Error('Electron returned an empty capture');
          process.getBuiltinModule('node:fs').writeFileSync(path, image.toPNG());
          return size;
        }, path));
        assert.ok(size.width > 0 && size.height > 0);
      }
      const png = await stage('verify-png', () => readFile(path));
      assert.equal(png.subarray(0, 8).toString('hex'), '89504e470d0a1a0a');
      const dimensions = {width: png.readUInt32BE(16), height: png.readUInt32BE(20)};
      assert.ok(dimensions.width > 0 && dimensions.height > 0, 'the real captured PNG has positive dimensions');
      remaining();
      const receipt = {mode, label: instance.label, fontStatus, ...dimensions, elapsedMs: now() - started};
      log({stage: 'capture', event: 'end', ...receipt});
      return receipt;
    },
  };
}

async function limited(operation, timeout, message) {
  let timer;
  try {
    return await Promise.race([operation, new Promise((_, reject) => {
      timer = setTimeout(() => reject(new Error(message)), timeout);
    })]);
  } finally { clearTimeout(timer); }
}
