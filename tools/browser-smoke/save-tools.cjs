// Explicit Account module ABI over real browser Files and canonical save bytes.
// This checks the independent module; mounted Account button wiring is separate.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

const { chromium } = require(path.resolve(process.env.PW));
const moduleUrl = process.env.SAVE_TOOLS_MODULE_URL;
const pageUrl = process.env.SAVE_TOOLS_PAGE_URL;
assert.ok(moduleUrl, 'SAVE_TOOLS_MODULE_URL names the built independent WASM module');
assert.ok(pageUrl, 'SAVE_TOOLS_PAGE_URL names a same-origin page without game startup');

const fixture = JSON.parse(fs.readFileSync(path.resolve(__dirname,
  '../../crates/sim/tests/fixtures/website/current-save-tools-goldens.json'), 'utf8'));

async function startupState(page) {
  return page.evaluate(() => ({
    gameBindings: Object.hasOwn(globalThis, 'wasmBindings'),
    canvasCount: document.querySelectorAll('canvas').length,
  }));
}

async function readSelection(page, module, input, asFileList) {
  return page.evaluate(async ({ module, input, asFileList }) => {
    const files = input.map(row => {
      const bytes = Uint8Array.from(atob(row.bytes), character => character.charCodeAt(0));
      const file = new File([bytes], row.name);
      Object.defineProperty(file, 'webkitRelativePath', { value: row.relativePath });
      return file;
    });
    let selection = files;
    if (asFileList) {
      const transfer = new DataTransfer();
      for (const file of files) transfer.items.add(file);
      selection = transfer.files;
    }
    try {
      const preview = await module.readGameSaveFileSelection(selection);
      const { document, ...summary } = preview;
      return {
        accepted: true,
        summary,
        frozen: Object.isFrozen(preview),
        hasDocument: typeof document === 'string' && typeof JSON.parse(document) === 'object',
      };
    } catch (error) {
      return { accepted: false, error: error.message, realError: error instanceof Error };
    }
  }, { module, input, asFileList });
}

async function run() {
  const browser = await chromium.launch({
    headless: true,
    executablePath: process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
  });
  const errors = [];
  const sockets = [];
  const requests = [];
  try {
    const context = await browser.newContext({
      ignoreHTTPSErrors: true,
      viewport: { width: 1280, height: 900 },
    });
    const page = await context.newPage();
    page.on('pageerror', error => errors.push(error.message));
    await page.goto(pageUrl);
    await page.waitForLoadState('networkidle');
    const before = await startupState(page);
    assert.equal(before.gameBindings, false, 'the selected page has no game bindings');
    page.on('websocket', socket => sockets.push(socket.url()));
    page.on('request', request => requests.push(request.url()));

    const module = await page.evaluateHandle(async url => {
      const loaded = await import(url);
      await loaded.default();
      return loaded;
    }, moduleUrl);
    const checks = [];
    for (const row of fixture.selections) {
      const result = await readSelection(page, module, row.input, false);
      if (row.error !== undefined) {
        assert.equal(result.accepted, false, row.name);
        assert.equal(result.realError, true, `${row.name}: a real JavaScript Error`);
        assert.equal(result.error, row.error, row.name);
      } else {
        assert.equal(result.accepted, true, `${row.name}: ${result.error || ''}`);
        assert.deepEqual(result.summary, row.expected, row.name);
        assert.equal(result.frozen, true, `${row.name}: Source preview freeze`);
        assert.equal(result.hasDocument, true, `${row.name}: validated save document`);
      }
      checks.push(row.name);
    }
    const jsonSelection = fixture.selections.find(row => row.name === 'browser-json');
    const listResult = await readSelection(page, module, jsonSelection.input, true);
    assert.equal(listResult.accepted, true, 'the Account FileList is admitted');
    assert.deepEqual(listResult.summary, jsonSelection.expected, 'FileList preview');
    checks.push('browser-file-list');

    for (const row of fixture.exports) {
      const result = await page.evaluate(async ({ module, document }) => {
        const exported = await module.exportWebGameSaveToNativeArchive(document);
        return {
          archiveIsBytes: exported.archive instanceof Uint8Array,
          archiveLength: exported.archive.length,
          archiveSignature: Array.from(exported.archive.slice(0, 2)),
          frozen: Object.isFrozen(exported),
          summary: {
            warnings: exported.warnings,
            learnedOrder: exported.portable.wizard.learnedOrder,
            skillQuickbar: exported.portable.wizard.skillQuickbar,
            disciplineRoot: exported.portable.wizard.disciplineRoot,
          },
        };
      }, { module, document: row.input });
      assert.equal(result.archiveIsBytes, true, `${row.mode}: downloadable bytes`);
      assert.ok(result.archiveLength > 0, `${row.mode}: archive exists`);
      assert.deepEqual(result.archiveSignature, [80, 75], `${row.mode}: ZIP signature`);
      assert.equal(result.frozen, true, `${row.mode}: Source export freeze`);
      assert.deepEqual(result.summary, row.expected, row.mode);
      checks.push(`export-${row.mode}`);
    }
    await module.dispose();
    assert.deepEqual(await startupState(page), before, 'the explicit save operations do not start the game');
    assert.deepEqual(sockets, [], 'the module opens no game connection');
    assert.deepEqual(errors, []);
    const receipt = { passed: true, sourceRevision: fixture.sourceRevision,
      moduleUrl, pageUrl, checks, requests, sockets, errors };
    if (process.env.OUT) {
      fs.mkdirSync(process.env.OUT, { recursive: true });
      fs.writeFileSync(path.join(process.env.OUT, 'save-tools.json'), JSON.stringify(receipt, null, 2));
    }
    console.log(JSON.stringify(receipt));
  } finally {
    await browser.close();
  }
}

run().catch(error => {
  console.error(error);
  process.exitCode = 1;
});
