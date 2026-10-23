// Actual Rust GPU Shelf and Website editor journey. No state setters,
// synthetic documents, API writes or socket sends are used by this driver.
const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const { chromium } = require(path.resolve(process.env.PW));
const gpu = require('./gpu-ui.cjs');

const game = new URL(process.env.URL || 'https://127.0.0.1:9449/game-stg/');
const website = new URL(process.env.WEBSITE_URL || game.href);
const mount = website.pathname.replace(/\/$/, '');
const out = path.resolve(process.env.OUT);
const localTls = process.env.LOCAL_TLS === '1';
assert.ok(!localTls || ['127.0.0.1', 'localhost', '[::1]'].includes(game.hostname),
  'LOCAL_TLS is only supported for loopback acceptance');
assert.equal(website.origin, game.origin, 'the library contract is same-origin');
const account = {
  username: `Shelf${Date.now().toString(36)}`,
  email: `shelf-${crypto.randomBytes(8).toString('hex')}@example.invalid`,
  password: `Qa!${crypto.randomBytes(18).toString('base64url')}`,
};
const localName = `Library ${Date.now().toString(36)}`;
const importName = `${localName} import`;
const nativeName = `${localName} native`;
const checks = [], errors = [], requests = [], failedResources = [], sockets = [], lifecycle = [];
const assetStartups = [];
const errorCounts = {};
const redact = value => String(value).replaceAll(account.password, '[password]')
  .replaceAll(account.email, '[email]')
  .replace(/eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+/g, '[token]');
const checked = label => { checks.push(label); console.log(label); };
function recordError(message) {
  const text = redact(message);
  const key = text.split('\n\n')[0];
  if (!errorCounts[key]) errors.push(text);
  errorCounts[key] = (errorCounts[key] || 0) + 1;
}
const pause = milliseconds => new Promise(resolve => setTimeout(resolve, milliseconds));
let browser, page, step = 'registration', lastWorkshopUrl;

async function library() {
  await gpu.ready(page);
  await gpu.widget(page, 'boneyards/new', {enabled:true});
  await gpu.assertGpuOnly(page);
  assert.equal(page.frames().length, 1, 'the game Shelf is painted by Rust with no visual iframe');
}

function rowPrefix(shelf, action) {
  return `boneyards/${action}:${shelf === 'In the cloud' ? 'cloud' : 'local'}:`;
}

async function shelfWidget(shelf, name, action = 'edit') {
  for (let attempt = 0; attempt < 18; attempt += 1) {
    const snapshot = await gpu.readUi(page);
    const found = snapshot.widgets.filter(widget => widget.id.startsWith(rowPrefix(shelf, action))
      && widget.value === name);
    assert.ok(found.length <= 1, `${shelf} contains one matching ${action} action for ${name}`);
    if (found[0]) return found[0];
    await gpu.wheel(page, 'cloud-boneyards-content', 0, attempt === 0 ? -2500 : 200);
    await pause(120);
  }
  throw new Error(`The actual visible Shelf has no ${action} action for ${shelf}: ${name}`);
}

async function focusWithTab(id) {
  for (let attempt = 0; attempt < 40; attempt += 1) {
    if (gpu.findWidget(await gpu.readUi(page), id)?.focused) return;
    await page.keyboard.press('Tab');
    await pause(30);
  }
  assert.fail(`Actual keyboard Tab did not reach ${id}`);
}

async function recordAssetStartup(target, label, started) {
  const resources = await target.evaluate(() => performance.getEntriesByType('resource')
    .map(entry => ({ path: new URL(entry.name).pathname, initiatorType: entry.initiatorType,
      duration: entry.duration, transferSize: entry.transferSize,
      encodedBodySize: entry.encodedBodySize, decodedBodySize: entry.decodedBodySize })));
  const startup = { label, elapsed_ms: Date.now() - started, resources };
  assetStartups.push(startup);
  console.log(JSON.stringify({ asset_ready: label, elapsed_ms: startup.elapsed_ms,
    resources: resources.length }));
}

async function editor() {
  await page.waitForURL(url => url.pathname === `${mount}/boneyard` && url.searchParams.has('workshop'));
  assert.match(new URL(page.url()).searchParams.get('workshop'), /^[0-9a-f-]{36}$/);
  lastWorkshopUrl = page.url();
  await page.getByRole('button', { name: 'Test Boneyard' }).waitFor();
}

async function back() {
  const started = Date.now();
  await gpu.settleErrors(page);
  await gpu.observeDocumentReplacement(page, () => page.getByRole('button', { name: /The Dark Cloud/ }).click());
  await page.waitForURL(url => url.origin === game.origin && url.pathname === game.pathname
    && url.searchParams.get('dark-cloud') === 'boneyards');
  await gpu.ready(page);
  await recordAssetStartup(page, `library-return-${assetStartups.length + 1}`, started);
  await library();
}

async function saveName(name) {
  await page.getByLabel('Name', { exact: true }).fill(name);
  await page.getByText('Saved locally', { exact: true }).waitFor();
}

async function testRow(shelf, name, label) {
  const before = sockets.length;
  const started = Date.now();
  await gpu.settleErrors(page);
  await gpu.observeDocumentReplacement(page, async () => gpu.click(page, (await shelfWidget(shelf, name, 'test')).id));
  await page.waitForURL(url => url.pathname === `${mount}/boneyard`);
  await page.waitForFunction(() => window.shelfPresentationTrace.some(event =>
    event.kind === 'solomon-editor-test-state' && (event.state === 'error'
      || (event.state === 'loading' && event.message === 'Starting a private test'))),
    null, { timeout: 240000 });
  const messages = await page.evaluate(() => window.shelfPresentationTrace);
  assert.deepEqual(messages.filter(message => message.state === 'error'), []);
  const test = page.frames().find(frame => new URL(frame.url()).pathname === `${mount}/editor-test/`);
  assert.ok(test, 'the shelf Test opens the real disposable Rust runtime');
  await recordAssetStartup(test, label, started);
  await page.locator('[data-editor-test-state="playing"]').waitFor();
  await gpu.ready(test);
  await gpu.assertGpuOnly(test);
  const tick = await test.evaluate(() => window.wasmBindings.wizard_state()[5]);
  await test.waitForFunction(tick => window.wasmBindings.wizard_state()[5] > tick + 20, tick);
  await page.screenshot({ path: path.join(out, `${label}-playing.png`) });
  await gpu.settleErrors(page);
  await page.getByRole('button', { name: /Return to editing/ }).click();
  await page.locator('iframe.editor-test-frame').waitFor({ state: 'detached' });
  assert.equal(await page.getByLabel('Name', { exact: true }).inputValue(), name);
  assert.equal(await page.getByRole('button', { name: 'Test Boneyard' })
    .evaluate(node => document.activeElement === node), true);
  assert.ok(sockets.length > before, 'an actual host connection was admitted');
  for (let attempt = 0; attempt < 100 && sockets.slice(before).some(socket => !socket.closed); attempt++) {
    await pause(50);
  }
  assert.ok(sockets.slice(before).every(socket => socket.closed), 'Test disposed its host connection');
  lifecycle.push({ label, events: await page.evaluate(() => window.shelfPresentationTrace) });
  checked(`${label} advances actual Rust ticks and returns to the unchanged editor with Test focused`);
  return back();
}

async function confirmDelete(shelf, name) {
  const safe = (await shelfWidget(shelf, name, 'safe')).id;
  await gpu.click(page, safe);
  await gpu.waitForUi(page, snapshot => {
    const keep = gpu.findWidget(snapshot, safe);
    return keep?.label === `Keep ${name}` && keep.selected === true;
  }, 'the same last row button becomes safe Keep');
  await gpu.focused(page, safe);
  await gpu.click(page, safe);
  await shelfWidget(shelf, name);
  assert.equal((await gpu.widget(page, safe)).label, `Delete ${name}`,
    'a repeated click on the stable safe button keeps the draft');
  await gpu.click(page, safe);
  await gpu.click(page, safe.replace('boneyards/safe:', 'boneyards/confirm:'));
  const location = shelf === 'In the cloud' ? 'the cloud' : 'this device';
  await gpu.wheel(page, 'cloud-boneyards-content', 0, -2500);
  await gpu.value(page, 'boneyards/status', `Deleted ${name} from ${location}.`);
  await gpu.focused(page, 'boneyards/new');
}

(async () => {
  fs.mkdirSync(out, { recursive: true });
  fs.writeFileSync(path.join(out, 'private-account.json'), JSON.stringify(account), { mode: 0o600, flag: 'wx' });
  let failure;
  try {
    browser = await chromium.launch({ headless: true,
      executablePath: process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
      args: ['--use-angle=metal', '--ignore-gpu-blocklist'] });
    const context = await browser.newContext({ ignoreHTTPSErrors: localTls, acceptDownloads: true,
      viewport: { width: 1600, height: 900 } });
    await context.addInitScript(() => {
      performance.setResourceTimingBufferSize(2000);
      window.shelfPresentationTrace = [];
      const trace = (kind, dimensions = {}) => {
        const records = window.top.shelfPresentationTrace;
        if (Array.isArray(records) && records.length < 1000) {
          records.push({ kind, time: performance.timeOrigin + performance.now(), ...dimensions });
        }
      };
      const dimensions = () => {
        const canvas = document.querySelector('canvas');
        return { width: canvas?.clientWidth, height: canvas?.clientHeight };
      };
      const request = window.requestAnimationFrame.bind(window);
      window.requestAnimationFrame = callback => request(time => {
        if (document.querySelector('meta[name="solomon-editor-test"]')) {
          const size = dimensions();
          if (size.width === 0 || size.height === 0) trace('zero-size-frame', size);
        }
        callback(time);
      });
      window.addEventListener('resize', () => trace('resize', dimensions()));
      window.addEventListener('message', event => {
        if (event.origin === location.origin && typeof event.data?.type === 'string'
          && event.data.type.startsWith('solomon-editor-test-')) {
          trace(event.data.type, { ...dimensions(), state: event.data.state, message: event.data.message });
        }
      });
      new MutationObserver(records => {
        for (const record of records) {
          const frame = record.target;
          if (frame instanceof HTMLIFrameElement && frame.classList.contains('editor-test-frame')) {
            trace('iframe-hidden', { hidden: frame.hidden, width: frame.clientWidth, height: frame.clientHeight });
          }
        }
      }).observe(document, { subtree: true, attributes: true, attributeFilter: ['hidden'] });
    });
    page = await context.newPage();
    page.setDefaultTimeout(45000);
    await gpu.watchRequestFailures(page, {errors}).ready;
    page.on('pageerror', error => recordError(error.message));
    page.on('console', message => {
      if (message.type() !== 'error') return;
      const source = message.location().url;
      if (message.text().includes('404')
        && (/\/api\/game\/saves\//.test(source) || source.endsWith('/api/game/rejoin'))) return;
      recordError(message.text());
    });
    page.on('requestfailed', request => {
      const receipt = { path: new URL(request.url()).pathname, reason: request.failure()?.errorText };
      failedResources.push(receipt);
    });
    page.on('response', response => {
      const pathname = new URL(response.url()).pathname;
      if (pathname.includes('/api/')) requests.push({ path: pathname,
        method: response.request().method(), status: response.status() });
      if (response.status() >= 400 && !(response.status() === 404
        && (/\/api\/game\/saves\//.test(pathname) || pathname.endsWith('/api/game/rejoin')))) {
        recordError(`${response.status()} ${pathname}`);
      }
    });
    page.on('websocket', socket => {
      const receipt = { path: new URL(socket.url()).pathname, closed: false };
      sockets.push(receipt);
      socket.on('close', () => { receipt.closed = true; });
    });

    await page.goto(new URL(`${mount}/register`, website).href);
    await page.locator('input[autocomplete="username"]').fill(account.username);
    await page.locator('input[autocomplete="email"]').fill(account.email);
    await page.locator('input[autocomplete="new-password"]').fill(account.password);
    await page.getByRole('button', { name: 'Create account', exact: true }).click();
    await page.waitForURL(url => url.pathname === `${mount}/account`);
    await page.getByRole('button', { name: 'import save', exact: true }).waitFor();
    checked('ordinary registration creates an isolated authoring account');

    step = 'Explore Boneyards';
    await gpu.settleErrors(page);
    await gpu.observeDocumentReplacement(page, () => page.goto(game.href, { timeout: 240000 }));
    await gpu.ready(page);
    if (gpu.findWidget(await gpu.readUi(page), 'prompt-secondary')) {
      await gpu.click(page, 'prompt-secondary');
      await gpu.noWidget(page, 'prompt-secondary');
    }
    await gpu.click(page, 'explore');
    await gpu.click(page, 'cloud-tab:boneyards');
    await library();
    await page.screenshot({ path: path.join(out, 'shelf-desktop.png') });
    await page.setViewportSize({ width: 390, height: 844 });
    await gpu.waitForUi(page, snapshot => {
      const button = gpu.findWidget(snapshot, 'boneyards/new');
      return button && button.rect[0] >= 0 && button.rect[0] + button.rect[2] <= 390;
    }, 'the real Shelf header fits the phone viewport');
    const phoneNew = gpu.bounds(await gpu.widget(page, 'boneyards/new'));
    const phoneImport = gpu.bounds(await gpu.widget(page, 'boneyards/import'));
    assert.ok(phoneNew.y + phoneNew.height <= phoneImport.y
      || phoneNew.x + phoneNew.width <= phoneImport.x, 'responsive Shelf tools do not overlap');
    await gpu.assertGpuOnly(page);
    await page.screenshot({ path: path.join(out, 'shelf-phone.png') });
    await page.setViewportSize({ width: 1600, height: 900 });
    await focusWithTab('boneyards/new');
    await page.keyboard.press('Escape');
    await gpu.surface(page, 'pause');
    await gpu.click(page, 'resume');
    await library();
    checked('Explore paints the Rust GPU Shelf, responds to the parent viewport and preserves the focused menu key');

    step = 'New, export and cloud save';
    // Clicking the parent scene must keep its embedded Shelf above that scene.
    await gpu.click(page, 'cloud-tab:boneyards');
    await gpu.settleErrors(page);
    await gpu.observeDocumentReplacement(page, () => gpu.click(page, 'boneyards/new'));
    await editor();
    await saveName(importName);
    await page.getByRole('button', { name: 'File ▾', exact: true }).click();
    const downloading = page.waitForEvent('download');
    await page.getByRole('button', { name: 'Export draft', exact: true }).click();
    const download = await downloading;
    const exported = path.join(out, 'actual-export.sdr-boneyard.json');
    await download.saveAs(exported);
    await saveName(nativeName);
    await page.getByRole('button', { name: 'File ▾', exact: true }).click();
    const compiling = page.waitForEvent('download');
    await page.getByRole('button', { name: 'Download .boneyard', exact: true }).click();
    const nativeExported = path.join(out, 'actual-export.boneyard');
    await (await compiling).saveAs(nativeExported);
    await saveName(localName);
    const cloudWrite = page.waitForResponse(response => new URL(response.url()).pathname
      .startsWith(`${mount}/api/boneyards/`) && response.request().method() === 'PUT' && response.ok());
    await page.getByRole('button', { name: 'Save to cloud', exact: true }).click();
    await cloudWrite;
    await page.getByText('Draft saved to the cloud.', { exact: true }).waitFor();
    await back();
    await shelfWidget('On this device', localName);
    await shelfWidget('In the cloud', localName);
    checked('New reaches the top-level editor and ordinary autosave/export/cloud save populate both shelves');

    step = 'Import and local Edit';
    const choosing = page.waitForEvent('filechooser');
    await gpu.click(page, 'boneyards/import');
    await (await choosing).setFiles(exported);
    await shelfWidget('On this device', importName);
    const nativeChoosing = page.waitForEvent('filechooser');
    await gpu.click(page, 'boneyards/import');
    await (await nativeChoosing).setFiles(nativeExported);
    await shelfWidget('On this device', nativeName);
    await confirmDelete('On this device', nativeName);
    checked('the ordinary editor .boneyard export imports through the full Rust native-document reader');
    await gpu.settleErrors(page);
    await gpu.observeDocumentReplacement(page, async () => gpu.click(page, (await shelfWidget('On this device', localName)).id));
    await editor();
    assert.equal(await page.getByLabel('Name', { exact: true }).inputValue(), localName);
    const consumedUrl = lastWorkshopUrl;
    await back();
    await gpu.settleErrors(page);
    await gpu.observeDocumentReplacement(page, () => page.goto(consumedUrl));
    await page.getByRole('alert').filter({ hasText: 'no longer available' }).waitFor();
    await back();
    await confirmDelete('On this device', importName);
    checked('real export imports locally; Edit consumes once; repeated Delete becomes Keep and confirmed deletion restores New focus');

    step = 'local and cloud Test';
    await testRow('On this device', localName, 'local-test');
    await testRow('In the cloud', localName, 'cloud-test');

    step = 'cloud Edit and Delete';
    await gpu.settleErrors(page);
    await gpu.observeDocumentReplacement(page, async () => gpu.click(page, (await shelfWidget('In the cloud', localName)).id));
    await editor();
    assert.equal(await page.getByLabel('Name', { exact: true }).inputValue(), localName);
    await back();
    await confirmDelete('In the cloud', localName);
    await shelfWidget('On this device', localName);
    checked('cloud deletion preserves the actual local copy');
    await page.screenshot({ path: path.join(out, 'shelf-complete.png') });
    checked('cloud Edit/Test/Delete use real API rows and cloud deletion preserves the local draft');
    await gpu.settleErrors(page);
    assert.deepEqual(errors, []);
    checked('the complete GPU Shelf/editor journey has no browser, API or runtime errors');
  } catch (error) {
    failure = { step, error: redact(error.stack || error.message), url: page?.url() };
    if (page) {
      failure.gpu = await gpu.readUi(page).catch(() => null);
      failure.frames = page.frames().map(frame => new URL(frame.url()).pathname);
      lifecycle.push({ label: 'failure', events: await page.evaluate(() => window.shelfPresentationTrace) });
      await page.screenshot({ path: path.join(out, 'failure.png') }).catch(() => {});
    }
  } finally {
    // Retain the app result before browser-process shutdown, which can hang
    // independently of the accepted UI journey on a busy GPU machine.
    try {
      if (page && !page.isClosed()) await gpu.settleErrors(page);
      fs.writeFileSync(path.join(out, 'receipt.json'), JSON.stringify({ target: game.href, tls_verified: !localTls, checks, failure, errors,
        errorCounts, requests, failedResources, sockets, lifecycle, assetStartups }, null, 2));
      console.log(JSON.stringify({ checks: checks.length, failure, output: out }));
    } finally {if (browser) await browser.close();}
    if (failure) process.exitCode = 1;
  }
})();
