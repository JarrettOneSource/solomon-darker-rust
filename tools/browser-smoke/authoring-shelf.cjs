// Actual Website editor shelf inside the Rust Cloud tab. No state setters,
// synthetic documents, API writes or socket sends are used by this driver.
const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const { chromium } = require(path.resolve(process.env.PW));

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
const checks = [], errors = [], requests = [], cancelled = [], sockets = [], lifecycle = [];
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
  await page.locator('iframe.boneyard-library-frame').waitFor({ state: 'visible' });
  const frame = page.frameLocator('iframe.boneyard-library-frame');
  await frame.getByRole('button', { name: 'NEW BONEYARD', exact: true }).waitFor();
  return frame;
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

function row(frame, shelf, name) {
  return frame.locator(`section[aria-label="${shelf}"] li`).filter({
    has: frame.getByRole('button', { name: `Edit ${name}`, exact: true }),
  });
}

async function editor() {
  await page.waitForURL(url => url.pathname === `${mount}/boneyard` && url.searchParams.has('workshop'));
  assert.match(new URL(page.url()).searchParams.get('workshop'), /^[0-9a-f-]{36}$/);
  lastWorkshopUrl = page.url();
  await page.getByRole('button', { name: 'Test Boneyard' }).waitFor();
}

async function back() {
  const started = Date.now();
  await page.getByRole('button', { name: /The Dark Cloud/ }).click();
  await page.waitForURL(url => url.origin === game.origin && url.pathname === game.pathname
    && url.searchParams.get('dark-cloud') === 'boneyards');
  // Back creates a fresh game document. Game::load clears this existing status
  // only after resident stock assets and audio finish loading.
  await page.waitForFunction(() => document.getElementById('status')?.textContent === '',
    null, { timeout: 240000 });
  await recordAssetStartup(page, `library-return-${assetStartups.length + 1}`, started);
  return library();
}

async function saveName(name) {
  await page.getByLabel('Name', { exact: true }).fill(name);
  await page.getByText('Saved locally', { exact: true }).waitFor();
}

async function testRow(frame, shelf, name, label) {
  const before = sockets.length;
  const started = Date.now();
  await row(frame, shelf, name).getByRole('button', { name: `Test ${name}`, exact: true }).click();
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
  const tick = await test.evaluate(() => window.wasmBindings.wizard_state()[5]);
  await test.waitForFunction(tick => window.wasmBindings.wizard_state()[5] > tick + 20, tick);
  await page.screenshot({ path: path.join(out, `${label}-playing.png`) });
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

async function confirmDelete(frame, shelf, name) {
  const section = frame.locator(`section[aria-label="${shelf}"]`);
  const target = row(frame, shelf, name);
  await target.getByRole('button', { name: `Delete ${name}`, exact: true }).click();
  const keep = section.getByRole('button', { name: `Keep ${name}`, exact: true });
  await keep.waitFor();
  assert.equal(await keep.evaluate(node => document.activeElement === node), true);
  await keep.click();
  assert.equal(await row(frame, shelf, name).count(), 1, 'a repeated click on the safe button keeps the draft');
  await row(frame, shelf, name).getByRole('button', { name: `Delete ${name}`, exact: true }).click();
  await section.getByRole('button', { name: `Delete ${name}`, exact: true }).click();
  // Edit is already absent during confirmation. Await the actual deletion
  // receipt instead, particularly while cloud removal is still pending.
  const location = shelf === 'In the cloud' ? 'the cloud' : 'this device';
  await frame.getByRole('status').filter({ hasText: `Deleted ${name} from ${location}.` }).waitFor();
  const newButton = frame.getByRole('button', { name: 'NEW BONEYARD', exact: true });
  for (let attempt = 0; attempt < 50; attempt++) {
    if (await newButton.evaluate(node => document.activeElement === node)) return;
    await pause(20);
  }
  assert.fail('confirmed deletion did not return focus to New');
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
      if (receipt.reason === 'net::ERR_ABORTED') cancelled.push(receipt);
      else recordError(`${receipt.path}: ${receipt.reason}`);
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
    await page.goto(game.href, { timeout: 240000 });
    await page.locator('.title-prompt[aria-label="PLAY THE TUTORIAL?"]').waitFor({ timeout: 240000 });
    await page.locator('[data-menu-action="prompt-secondary"]').click();
    await page.locator('.title-prompt').waitFor({ state: 'hidden' });
    await page.locator('[data-menu-action="explore"]').click();
    await page.locator('[data-menu-action="cloud-tab:boneyards"]').click();
    let frame = await library();
    assert.equal(await frame.locator('.boneyard-library-document').getAttribute('data-parent-narrow'), 'false');
    await page.screenshot({ path: path.join(out, 'shelf-desktop.png') });
    await page.setViewportSize({ width: 390, height: 844 });
    await frame.locator('.boneyard-library-document[data-parent-narrow="true"]').waitFor();
    await page.screenshot({ path: path.join(out, 'shelf-phone.png') });
    await page.setViewportSize({ width: 1600, height: 900 });
    await frame.getByRole('button', { name: 'NEW BONEYARD', exact: true }).focus();
    await page.keyboard.press('Escape');
    await page.locator('.game-menus[data-menu-screen="pause"]').waitFor();
    await page.locator('[data-menu-action="resume"]').click();
    frame = await library();
    checked('Explore mounts the same-origin shelf, parent-responsive layout and focused iframe menu key');

    step = 'New, export and cloud save';
    await frame.getByRole('button', { name: 'NEW BONEYARD', exact: true }).click();
    await editor();
    await saveName(importName);
    await page.getByRole('button', { name: 'File ▾', exact: true }).click();
    const downloading = page.waitForEvent('download');
    await page.getByRole('button', { name: 'Export draft', exact: true }).click();
    const download = await downloading;
    const exported = path.join(out, 'actual-export.sdr-boneyard.json');
    await download.saveAs(exported);
    await saveName(localName);
    const cloudWrite = page.waitForResponse(response => new URL(response.url()).pathname
      .startsWith(`${mount}/api/boneyards/`) && response.request().method() === 'PUT' && response.ok());
    await page.getByRole('button', { name: 'Save to cloud', exact: true }).click();
    await cloudWrite;
    await page.getByText('Draft saved to the cloud.', { exact: true }).waitFor();
    frame = await back();
    await row(frame, 'On this device', localName).waitFor();
    await row(frame, 'In the cloud', localName).waitFor();
    checked('New reaches the top-level editor and ordinary autosave/export/cloud save populate both shelves');

    step = 'Import and local Edit';
    await frame.locator('input[type="file"]').setInputFiles(exported);
    await row(frame, 'On this device', importName).waitFor();
    await row(frame, 'On this device', localName).getByRole('button', { name: `Edit ${localName}`, exact: true }).click();
    await editor();
    assert.equal(await page.getByLabel('Name', { exact: true }).inputValue(), localName);
    const consumedUrl = lastWorkshopUrl;
    frame = await back();
    await page.goto(consumedUrl);
    await page.getByRole('alert').filter({ hasText: 'no longer available' }).waitFor();
    frame = await back();
    await confirmDelete(frame, 'On this device', importName);
    checked('real export imports locally; Edit consumes once; repeated Delete becomes Keep and confirmed deletion restores New focus');

    step = 'local and cloud Test';
    frame = await testRow(frame, 'On this device', localName, 'local-test');
    frame = await testRow(frame, 'In the cloud', localName, 'cloud-test');

    step = 'cloud Edit and Delete';
    await row(frame, 'In the cloud', localName).getByRole('button', { name: `Edit ${localName}`, exact: true }).click();
    await editor();
    assert.equal(await page.getByLabel('Name', { exact: true }).inputValue(), localName);
    frame = await back();
    await confirmDelete(frame, 'In the cloud', localName);
    assert.equal(await row(frame, 'On this device', localName).count(), 1, 'cloud deletion preserves the actual local copy');
    await page.screenshot({ path: path.join(out, 'shelf-complete.png') });
    checked('cloud Edit/Test/Delete use real API rows and cloud deletion preserves the local draft');
    assert.deepEqual(errors, []);
    checked('the complete mounted journey has no browser, API or runtime errors');
  } catch (error) {
    failure = { step, error: redact(error.stack || error.message), url: page?.url() };
    if (page) {
      failure.status = await page.locator('#status').textContent({ timeout: 1000 }).catch(() => null);
      failure.frames = page.frames().map(frame => new URL(frame.url()).pathname);
      lifecycle.push({ label: 'failure', events: await page.evaluate(() => window.shelfPresentationTrace) });
      await page.screenshot({ path: path.join(out, 'failure.png') }).catch(() => {});
    }
  } finally {
    // Retain the app result before browser-process shutdown, which can hang
    // independently of the accepted UI journey on a busy GPU machine.
    fs.writeFileSync(path.join(out, 'receipt.json'), JSON.stringify({ target: game.href, tls_verified: !localTls, checks, failure, errors,
      errorCounts, requests, cancelled, sockets, lifecycle, assetStartups }, null, 2));
    console.log(JSON.stringify({ checks: checks.length, failure, output: out }));
    if (browser) await browser.close();
    if (failure) process.exitCode = 1;
  }
})();
