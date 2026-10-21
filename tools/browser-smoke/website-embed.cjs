// The extracted Website's actual Account controls and disposable editor document.
const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const { chromium } = require(path.resolve(process.env.PW));
const base = new URL(process.env.URL || 'https://127.0.0.1:9449/game-stg/');
const out = path.resolve(process.env.OUT);
const fixture = JSON.parse(fs.readFileSync(path.resolve(__dirname,
  '../../crates/sim/tests/fixtures/website/current-save-tools-goldens.json'), 'utf8'));
const mount = base.pathname.replace(/\/$/, '');
const checks = [];
const errors = [];
const sockets = [];
const requests = [];
const account = {
  username: `Embed${Date.now().toString(36)}`,
  email: `embed-${crypto.randomBytes(8).toString('hex')}@example.invalid`,
  password: `Qa!${crypto.randomBytes(18).toString('base64url')}`,
};
const redact = value => String(value).replaceAll(account.password, '[password]')
  .replaceAll(account.email, '[email]')
  .replace(/eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+/g, '[token]');
const checked = name => { checks.push(name); console.log(name); };
let browser;
let page;
let step = 'registration';

async function savedState() {
  return page.evaluate(async mount => {
    const token = localStorage.getItem(`sdr.token:${mount}`);
    const response = await fetch(`${mount}/api/game/saves/0`, {
      headers: { Authorization: `Bearer ${token}` },
    });
    if (!response.ok) throw new Error(`save read ${response.status}`);
    const body = await response.json();
    return { save: body.save, resume: localStorage.getItem(`solomon-darker-rust-native-resume-v1:${mount}`) };
  }, mount);
}

async function draftState() {
  return page.evaluate(() => Object.fromEntries(Object.keys(localStorage)
    .filter(key => key.startsWith('sdr:boneyard:draft:'))
    .map(key => [key, localStorage.getItem(key)])));
}

async function waitForDisposed(count) {
  await page.waitForFunction(count => window.editorTestMessages
    .filter(message => message.type === 'solomon-editor-test-disposed').length >= count, count);
  await page.locator('iframe.editor-test-frame').waitFor({ state: 'detached' });
  assert.equal(await page.locator('.boneyard-workshop').getAttribute('data-editor-active'), 'true');
}

(async () => {
  fs.mkdirSync(out, { recursive: true });
  fs.writeFileSync(path.join(out, 'private-account.json'), JSON.stringify(account), { mode: 0o600, flag: 'wx' });
  let failure;
  try {
    browser = await chromium.launch({ headless: true,
      executablePath: process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
      args: ['--use-angle=metal', '--ignore-gpu-blocklist'] });
    const context = await browser.newContext({ ignoreHTTPSErrors: true, acceptDownloads: true,
      viewport: { width: 1600, height: 900 } });
    await context.addInitScript(() => {
      window.editorTestMessages = [];
      window.addEventListener('message', event => {
        if (event.origin !== location.origin || !event.data?.type?.startsWith('solomon-editor-test-')) return;
        window.editorTestMessages.push({ type: event.data.type, state: event.data.state, message: event.data.message });
      });
    });
    page = await context.newPage();
    page.setDefaultTimeout(45000);
    page.on('pageerror', error => errors.push(redact(error.message)));
    page.on('console', message => { if (message.type() === 'error') errors.push(redact(message.text())); });
    page.on('response', response => {
      const target = new URL(response.url());
      if (target.pathname.includes('/api/')) requests.push({ path: target.pathname, method: response.request().method(), status: response.status() });
      if (response.status() >= 400) errors.push(`${response.status()} ${target.pathname}`);
    });
    page.on('websocket', socket => {
      const row = { path: new URL(socket.url()).pathname, closed: false, received: 0 };
      sockets.push(row);
      socket.on('framereceived', () => row.received++);
      socket.on('close', () => { row.closed = true; });
    });
    await page.goto(new URL(`${mount}/register`, base).href);
    await page.locator('input[autocomplete="username"]').fill(account.username);
    await page.locator('input[autocomplete="email"]').fill(account.email);
    await page.locator('input[autocomplete="new-password"]').fill(account.password);
    await page.getByRole('button', { name: 'Create account', exact: true }).click();
    await page.waitForURL(url => url.pathname === `${mount}/account`);
    await page.getByRole('button', { name: 'import save', exact: true }).waitFor();
    assert.equal(await page.evaluate(() => localStorage.getItem('sdr.token')), null);
    checked('real registration opens Account and keeps credentials in the staging namespace');

    step = 'Account import and export';
    const selection = fixture.selections.find(row => row.name === 'browser-json');
    const source = selection.input[0];
    await page.locator('input[type=file]').setInputFiles({ name: source.name, mimeType: 'application/json', buffer: Buffer.from(source.bytes, 'base64') });
    await page.getByRole('button', { name: 'save to slot I', exact: true }).waitFor();
    await page.getByText(selection.expected.displayName, { exact: true }).waitFor();
    await page.getByRole('button', { name: 'save to slot I', exact: true }).click();
    await page.getByRole('button', { name: 'erase', exact: true }).waitFor();
    const saved = await savedState();
    assert.ok(saved.save?.document);
    page.on('dialog', dialog => dialog.accept());
    const downloadPromise = page.waitForEvent('download');
    await page.getByRole('button', { name: 'export save', exact: true }).click();
    const download = await downloadPromise;
    const archive = path.join(out, 'exported-save.zip');
    await download.saveAs(archive);
    assert.deepEqual([...fs.readFileSync(archive).subarray(0, 2)], [80, 75]);
    assert.equal(sockets.length, 0);
    assert.equal(await page.evaluate(() => Object.hasOwn(window, 'wasmBindings')), false);
    await page.screenshot({ path: path.join(out, 'account-save.png'), fullPage: true });
    checked('Account previews, imports and exports a real save through independent WASM without starting the game');

    step = 'editor draft';
    await page.goto(new URL(`${mount}/boneyard`, base).href);
    await page.getByRole('button', { name: 'Test Boneyard' }).waitFor();
    const draftName = `Preserved ${Date.now().toString(36)}`;
    await page.getByLabel('Name', { exact: true }).fill(draftName);
    await page.getByText('Saved locally', { exact: true }).waitFor();
    const draft = await draftState();
    assert.ok(Object.values(draft).some(value => value.includes(draftName)));
    const before = await savedState();
    checked('the ordinary editor writes the named draft before Test');

    step = 'cancel admitted test before hello';
    let release;
    let admitted;
    const admission = new Promise(resolve => { admitted = resolve; });
    await page.route(`**${mount}/api/game/editor-test`, async route => {
      const response = await route.fetch();
      assert.equal(response.status(), 200);
      admitted();
      await new Promise(resolve => { release = resolve; });
      await route.fulfill({ response });
    });
    await page.getByRole('button', { name: 'Test Boneyard' }).click();
    let admissionTimer;
    try {
      await Promise.race([admission, new Promise((_, reject) => {
        admissionTimer = setTimeout(() => reject(new Error('editor admission did not arrive')), 45000);
      })]);
    } finally { clearTimeout(admissionTimer); }
    await page.getByRole('button', { name: 'Return to editing' }).click();
    assert.equal(await page.locator('iframe.editor-test-frame').getAttribute('hidden'), '');
    release();
    await waitForDisposed(1);
    await page.unroute(`**${mount}/api/game/editor-test`);
    assert.equal(sockets.length, 1);
    assert.ok(sockets[0].received > 0 && sockets[0].closed);
    assert.deepEqual(await savedState(), before);
    assert.deepEqual(await draftState(), draft);
    checked('cancellation after admission claims and closes the authority while preserving draft, cloud save and resume');

    step = 'play editor test and return';
    await page.getByRole('button', { name: 'Test Boneyard' }).click();
    await page.locator('[data-editor-test-state="playing"]').waitFor();
    const frame = page.frames().find(frame => new URL(frame.url()).pathname === `${mount}/editor-test/`);
    assert.ok(frame);
    const firstTick = await frame.evaluate(() => window.wasmBindings.wizard_state()[5]);
    await frame.waitForFunction(tick => window.wasmBindings.wizard_state()[5] > tick + 20, firstTick);
    await page.screenshot({ path: path.join(out, 'editor-playing.png') });
    await page.getByRole('button', { name: 'Return to editing' }).click();
    await waitForDisposed(2);
    assert.equal(sockets.length, 2);
    assert.ok(sockets.every(socket => socket.closed));
    assert.deepEqual(await savedState(), before);
    assert.deepEqual(await draftState(), draft);
    await page.screenshot({ path: path.join(out, 'editor-returned.png') });
    checked('the actual disposable Boneyard advances native ticks and returns to the unchanged draft');
    const messages = await page.evaluate(() => window.editorTestMessages);
    assert.deepEqual(messages.filter(message => message.state === 'error'), []);
    assert.deepEqual(errors, []);
    checked('both editor lifecycles complete without browser, API or game errors');
  } catch (error) {
    failure = { step, error: redact(error.stack || error.message) };
    if (page) {
      failure.messages = await page.evaluate(() => window.editorTestMessages).catch(() => []);
      failure.frames = await Promise.all(page.frames().map(async frame => ({
        path: new URL(frame.url()).pathname,
        status: await frame.locator('#status').textContent({ timeout: 1000 }).catch(() => null),
      })));
      await page.screenshot({ path: path.join(out, 'failure.png') }).catch(() => {});
    }
  } finally {
    if (browser) await browser.close();
    fs.writeFileSync(path.join(out, 'receipt.json'), JSON.stringify({ checks, failure, errors, requests, sockets }, null, 2));
    console.log(JSON.stringify({ checks: checks.length, failure, output: out }));
    if (failure) process.exitCode = 1;
  }
})();
