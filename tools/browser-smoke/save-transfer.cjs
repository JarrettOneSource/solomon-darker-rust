// Browser export/import through the real File chooser, Blob download and durable save slot.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const {chromium} = require(path.resolve(process.env.PW));
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
    const opening = indexedDB.open('solomon-darker-rust-saves:/game-stg');
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
    await page.locator('[data-save-transfer-action="choose"]').click();
    await (await opening).setFiles(files);
  };
  try {
    await page.goto(process.env.URL, {waitUntil: 'load'});
    await page.waitForFunction(() => window.wasmBindings?.wizard_state().length > 0
      && document.querySelector('#status')?.textContent === '', null, {timeout: 60000});
    await page.locator('.title-prompt:not([hidden]) [data-menu-action="prompt-secondary"]').click();
    await page.locator('[data-menu-action="settings"]').click();
    await page.locator('[data-menu-action="settings-page:cloud"]').click();
    await page.locator('[data-menu-action="settings-page:save-transfer"]').click();
    assert.equal(await page.locator('[data-save-transfer-action="export"]').isDisabled(), true);
    assert.equal(await storedSave(), null);
    await choose({name: 'invalid.json', mimeType: 'application/json', buffer: Buffer.from('{}')});
    await page.locator('.native-save-transfer-error:not([hidden])').waitFor();
    assert.equal(await storedSave(), null);
    checks.push('invalid file reports an import error and leaves the empty slot unchanged');
    await choose(process.env.ARCHIVE);
    const preview = page.locator('.native-save-transfer-preview:not([hidden])');
    await preview.waitFor();
    assert.ok((await preview.innerText()).includes(expected.displayName));
    assert.ok((await preview.innerText()).includes('Browser save: inventory, equipment, and the saved run will be restored.'));
    assert.equal(await storedSave(), null);
    await page.locator('[data-save-transfer-action="cancel"]').click();
    await preview.waitFor({state: 'hidden'});
    assert.equal(await storedSave(), null);
    checks.push('actual downloaded ZIP preview and Cancel preserve the existing slot');
    await choose(process.env.ARCHIVE);
    await preview.waitFor();
    await page.screenshot({path: path.join(output, 'native-save-import-preview.png')});
    await page.locator('[data-save-transfer-action="replace"]').click();
    await page.waitForFunction(() => document.querySelector('.native-save-transfer-status')?.textContent.includes('Save imported.'));
    const record = JSON.parse(await storedSave());
    assert.equal(record.revision, 1);
    assert.ok(record.document.length > 1024);
    assert.equal(await page.locator('[data-save-transfer-action="export"]').isEnabled(), true);
    assert.equal(await preview.isVisible(), false);
    checks.push('Replace waits for revision-one persistence and updates resume/export availability');
    await page.screenshot({path: path.join(output, 'native-save-imported.png')});
    await page.reload({waitUntil: 'load'});
    await page.waitForFunction(() => document.querySelector('#status')?.textContent === '', null, {timeout: 60000});
    await page.locator('[data-menu-action="play"]').click();
    await page.locator('[data-menu-action="continue"]').click();
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
      ui: await page.evaluate(() => ({status: document.querySelector('#status')?.textContent, menu: document.querySelector('.game-menus')?.dataset.menuScreen, text: document.body.innerText.slice(-2500)}))}, null, 2));
    throw error;
  } finally {await browser.close();}
})().catch(error => {console.error(error); process.exitCode = 1;});
