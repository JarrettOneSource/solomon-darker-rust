// Browser export/import through the real File chooser, Blob download and durable save slot.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const {chromium} = require(path.resolve(process.env.PW));
const gpu = require('./gpu-ui.cjs');
const output = path.resolve(process.env.OUT);
const expected = JSON.parse(fs.readFileSync(path.join(output, 'archive-receipt.json')));
const checks = [], errors = [], requests = [];
let accepted;
let playerId;

const waitUntil = async (predicate, label, timeout = 60000) => {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    if (predicate()) return;
    await new Promise(resolve => setTimeout(resolve, 50));
  }
  throw new Error(`Timed out waiting for ${label}`);
};

async function reveal(page, id) {
  if (gpu.findWidget(await gpu.readUi(page), id)) return;
  await gpu.wheel(page, 'settings-content', 0, -4000);
  for (let attempt = 0; attempt < 16; attempt += 1) {
    if (gpu.findWidget(await gpu.readUi(page), id)) return;
    await gpu.wheel(page, 'settings-content', 0, 240);
    await gpu.nextFrame(page);
  }
  throw new Error(`The real Save Transfer content did not reveal ${id}`);
}

(async () => {
  const browser = await chromium.launch({headless: true,
    executablePath: process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    args: ['--use-angle=metal', '--ignore-gpu-blocklist']});
  const page = await browser.newPage({ignoreHTTPSErrors: process.env.LOCAL_TLS === '1',
    viewport: {width: 1280, height: 720}});
  page.setDefaultTimeout(30000);
  page.on('pageerror', error => errors.push(error.message));
  page.on('console', message => {if (message.type() === 'error') errors.push(message.text());});
  page.on('response', response => {
    const pathname = new URL(response.url()).pathname;
    if (pathname.includes('/api/')) requests.push({path: pathname, status: response.status()});
    if (response.status() >= 400) errors.push(`${response.status()} ${pathname}`);
  });
  page.on('requestfailed', request => {
    if (!(request.resourceType() === 'media' && request.failure()?.errorText === 'net::ERR_ABORTED'))
      errors.push(`${new URL(request.url()).pathname}: ${request.failure()?.errorText}`);
  });
  page.on('websocket', socket => socket.on('framereceived', ({payload}) => {
    const message = JSON.parse(payload.toString());
    if (message.type === 'welcome') {accepted = message.frame; playerId = message.credentials.player_id;}
    else if (message.type === 'snapshot') accepted = message.frame;
    else if (message.type === 'error') errors.push(`host ${message.code}: ${message.reason}`);
  }));
  const storedSave = () => page.evaluate(() => new Promise((resolve, reject) => {
    const mount = document.querySelector('meta[name="solomon-website-mount"]')?.content.replace(/\/$/, '') || '';
    const opening = indexedDB.open(`solomon-darker-rust-saves${mount ? `:${mount}` : ''}`);
    opening.onerror = () => reject(opening.error);
    opening.onsuccess = () => {
      const db = opening.result;
      const reading = db.transaction('slots', 'readonly').objectStore('slots').get(0);
      reading.onerror = () => {db.close(); reject(reading.error);};
      reading.onsuccess = () => {db.close(); resolve(reading.result ?? null);};
    };
  }));
  const choose = async files => {
    const opening = page.waitForEvent('filechooser');
    await reveal(page, 'save-import');
    await gpu.click(page, 'save-import');
    await (await opening).setFiles(files);
  };
  try {
    await page.goto(process.env.URL, {waitUntil: 'load'});
    await gpu.ready(page);
    if (gpu.findWidget(await gpu.readUi(page), 'prompt-secondary')) {
      await gpu.click(page, 'prompt-secondary');
      await gpu.noWidget(page, 'prompt-secondary');
    }
    await gpu.click(page, 'settings');
    await gpu.click(page, 'settings-page:cloud');
    await gpu.click(page, 'settings-page:save-transfer');
    assert.equal((await gpu.widget(page, 'save-export')).enabled, false);
    assert.equal(await storedSave(), null);
    await choose({name: 'invalid.json', mimeType: 'application/json', buffer: Buffer.from('{}')});
    await reveal(page, 'save-error');
    await gpu.widget(page, 'save-error');
    assert.equal(await storedSave(), null);
    checks.push('invalid file reports an import error and leaves the empty slot unchanged');
    await choose(process.env.ARCHIVE);
    await reveal(page, 'save-preview');
    const preview = await gpu.widget(page, 'save-preview');
    assert.equal(preview.label, expected.displayName);
    assert.ok(preview.value.includes('Browser save: inventory, equipment, and the saved run will be restored.'));
    assert.equal(await storedSave(), null);
    await reveal(page, 'save-cancel');
    await gpu.click(page, 'save-cancel');
    await gpu.noWidget(page, 'save-preview');
    assert.equal(await storedSave(), null);
    checks.push('actual downloaded ZIP preview and Cancel preserve the existing slot');
    await choose(process.env.ARCHIVE);
    await reveal(page, 'save-preview');
    await gpu.widget(page, 'save-preview');
    await page.screenshot({path: path.join(output, 'native-save-import-preview.png')});
    await reveal(page, 'save-replace');
    await gpu.click(page, 'save-replace');
    await reveal(page, 'save-status');
    await gpu.waitForUi(page, snapshot => gpu.findWidget(snapshot, 'save-status')?.value.includes('Save imported.'), 'actual imported persistence receipt');
    const record = JSON.parse(await storedSave());
    assert.equal(record.revision, 1);
    assert.ok(record.document.length > 1024);
    await reveal(page, 'save-export');
    assert.equal((await gpu.widget(page, 'save-export')).enabled, true);
    await gpu.noWidget(page, 'save-preview');
    await gpu.assertGpuOnly(page);
    checks.push('Replace waits for revision-one persistence and updates resume/export availability');
    await page.screenshot({path: path.join(output, 'native-save-imported.png')});
    const downloading = page.waitForEvent('download');
    await gpu.click(page, 'save-export');
    const exportedState = await gpu.waitForUi(page, snapshot =>
      gpu.findWidget(snapshot, 'save-export-limitations')
      || gpu.findWidget(snapshot, 'save-status')?.value.includes('Save archive downloaded'),
    'actual export or its stock-limitations confirmation');
    if (gpu.findWidget(exportedState, 'save-export-limitations')) {
      await reveal(page, 'save-export-confirm');
      await gpu.click(page, 'save-export-confirm');
    }
    const exportedArchive = path.join(output, 'menu-roundtrip-export.zip');
    await (await downloading).saveAs(exportedArchive);
    const bytes = fs.readFileSync(exportedArchive);
    assert.deepEqual([...bytes.subarray(0, 2)], [80, 75]);
    assert.ok(bytes.length > 1024);
    await choose(exportedArchive);
    await reveal(page, 'save-preview');
    assert.equal((await gpu.widget(page, 'save-preview')).label, expected.displayName);
    assert.ok((await gpu.widget(page, 'save-preview')).value.includes('Browser save:'));
    await reveal(page, 'save-cancel');
    await gpu.click(page, 'save-cancel');
    assert.equal(JSON.parse(await storedSave()).revision, 1);
    checks.push('the real GPU export/limitations/download produces a readable browser ZIP and Cancel preserves revision one');
    await page.reload({waitUntil: 'load'});
    await gpu.ready(page);
    await gpu.click(page, 'play');
    await gpu.click(page, 'continue');
    await waitUntil(() => accepted?.snapshot.boneyard?.run_id === expected.runId
      && !accepted.host.loading_run_id && !accepted.host.resume_grace, 'actual imported Boneyard continuation', 120000);
    assert.equal(accepted.snapshot.participants[playerId].player.selection.name, expected.displayName);
    const restoredTick = accepted.snapshot.ticks;
    await waitUntil(() => accepted?.snapshot.ticks > restoredTick + 100,
      'restored Boneyard advances through a full second of authoritative ticks');
    checks.push('fresh browser reload restores the exported wizard and the same Boneyard run through actual host admission');
    assert.deepEqual(errors, []);
    const receipt = {passed: true, checks, requests, errors, archiveSha256: expected.sha256, importedRevision: record.revision};
    fs.writeFileSync(path.join(output, 'native-save-transfer.json'), JSON.stringify(receipt, null, 2));
    console.log(JSON.stringify(receipt));
  } catch (error) {
    await page.screenshot({path: path.join(output, 'native-save-transfer-failure.png')});
    fs.writeFileSync(path.join(output, 'native-save-transfer-failure.json'), JSON.stringify({error: error.stack, checks, requests, errors,
      ui: await gpu.readUi(page)}, null, 2));
    throw error;
  } finally {await browser.close();}
})().catch(error => {console.error(error); process.exitCode = 1;});
