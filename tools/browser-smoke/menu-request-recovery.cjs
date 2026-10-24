// Real menu requests and controls, with exact owned HTTP failures at the transport boundary.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const {chromium} = require(path.resolve(process.env.PW));
const {launchOwnedBrowser} = require('./browser-lifecycle.cjs');
const gpu = require('./gpu-ui.cjs');

const output = path.resolve(process.env.OUT);
const url = process.env.URL;
const receipt = {checks: [], requests: [], faults: [], routeFailures: [], errors: []};
const plans = [];
const ownedResponseErrors = new Set();
let context;
let page;

function plan(label, accepts, failure) {
  let release;
  const gate = new Promise(resolve => {release = resolve;});
  let finish;
  const done = new Promise(resolve => {finish = resolve;});
  const item = {label, accepts, failure, release, gate, finish, done, request: null};
  plans.push(item);
  return item;
}

async function seen(item) {
  await gpu.waitForUi(page, () => item.request !== null, `the actual ${item.label} request`);
  assert.deepEqual(receipt.routeFailures, []);
}

async function release(item) {
  const response = page.waitForResponse(response => response.request() === item.request);
  item.release();
  await item.done;
  await (await response).finished();
  await gpu.nextFrame(page);
  assert.deepEqual(receipt.routeFailures, []);
}

function endpoint(pathname) {
  return target => target.pathname === new URL(pathname, url).pathname;
}

async function nextResponse(accepts, action) {
  const response = page.waitForResponse(response => response.request().method() === 'GET'
    && accepts(new URL(response.url())) && response.status() === 200);
  await action();
  const accepted = await response;
  await accepted.finished();
  await gpu.nextFrame(page);
  return accepted;
}

async function closeCloud() {
  await gpu.click(page, 'cloud-menu');
  await gpu.surface(page, 'pause');
  await gpu.click(page, 'leave');
  await gpu.surface(page, 'title');
}

async function catalog() {
  const accepts = endpoint('api/mods');
  const initial = plan('catalog loading', accepts, 'Owned catalog availability failure');
  await gpu.click(page, 'explore');
  await gpu.surface(page, 'dark-cloud');
  await seen(initial);
  await gpu.waitForUi(page, snapshot => gpu.findWidget(snapshot, 'cloud-list-status')?.value
    .includes('CONSULTING'), 'the real catalog loading state');
  await page.screenshot({path: path.join(output, 'catalog-loading.png')});
  await release(initial);
  assert.equal((await gpu.widget(page, 'cloud-refresh')).label, 'RETRY');
  await page.screenshot({path: path.join(output, 'catalog-failure.png')});
  const accepted = await nextResponse(accepts, () => gpu.click(page, 'cloud-refresh'));
  const data = await accepted.json();
  assert.ok(data.items.length > 0, 'the ordinary backend owns the recovered catalog');
  await gpu.waitForUi(page, snapshot => snapshot.widgets.some(widget =>
    widget.id.startsWith('cloud-select:mod:')), 'real catalog rows after Retry');
  receipt.checks.push('the held catalog paints loading; its exact failure paints Retry; Retry loads actual backend rows');

  const first = (await gpu.readUi(page)).widgets.find(widget => widget.id.startsWith('cloud-select:mod:'));
  const slug = first.id.slice('cloud-select:mod:'.length);
  const detailPath = endpoint(`api/mods/${encodeURIComponent(slug)}`);
  const detail = plan('detail loading', detailPath, 'Owned detail availability failure');
  await gpu.click(page, first.id, {clickCount: 2});
  await gpu.widget(page, 'cloud-detail-title');
  await seen(detail);
  await gpu.noWidget(page, 'cloud-detail-retry');
  await release(detail);
  await gpu.widget(page, 'cloud-detail-retry');
  await page.screenshot({path: path.join(output, 'detail-failure.png')});
  await nextResponse(detailPath, () => gpu.click(page, 'cloud-detail-retry'));
  await gpu.noWidget(page, 'cloud-detail-retry');
  await gpu.widget(page, 'cloud-gallery-image');
  await gpu.click(page, 'cloud-dialog-close');
  await gpu.noWidget(page, 'cloud-detail-title');
  await gpu.focused(page, first.id);
  receipt.checks.push('the actual detail failure preserves its modal; Retry recovers real detail and Back restores its row focus');

  const lateDetail = plan('retired detail', detailPath, 'Owned retired detail failure');
  await gpu.click(page, first.id, {clickCount: 2});
  await seen(lateDetail);
  await gpu.click(page, 'cloud-dialog-close');
  await gpu.noWidget(page, 'cloud-detail-title');
  await release(lateDetail);
  await nextResponse(detailPath, () => gpu.click(page, first.id, {clickCount: 2}));
  await gpu.noWidget(page, 'cloud-detail-retry');
  await gpu.click(page, 'cloud-dialog-close');
  await closeCloud();

  const lateCatalog = plan('retired catalog', accepts, 'Owned retired catalog failure');
  await gpu.click(page, 'explore');
  await seen(lateCatalog);
  await closeCloud();
  await release(lateCatalog);
  await nextResponse(accepts, () => gpu.click(page, 'explore'));
  await gpu.waitForUi(page, snapshot => snapshot.widgets.some(widget =>
    widget.id.startsWith('cloud-select:mod:')), 'the newly opened actual catalog');
  await gpu.noWidget(page, 'cloud-refresh');
  await closeCloud();
  receipt.checks.push('late real detail/catalog failures after ordinary close cannot replace the next mounted view');
}

async function hall() {
  const leaderboard = endpoint('api/game/leaderboards');
  const initial = plan('global Hall loading', leaderboard, 'Owned Hall availability failure');
  await gpu.click(page, 'hall');
  await gpu.click(page, 'hall-scope:global');
  await seen(initial);
  await gpu.value(page, 'hall-status', 'Loading global records...');
  await release(initial);
  await gpu.value(page, 'hall-status', initial.failure);
  await gpu.widget(page, 'hall-retry');
  await page.screenshot({path: path.join(output, 'hall-failure.png')});
  await nextResponse(leaderboard, () => gpu.click(page, 'hall-retry'));
  await gpu.noWidget(page, 'hall-retry');
  await gpu.waitForUi(page, snapshot => gpu.findWidget(snapshot, 'hall-status')?.value
    !== 'Loading global records...', 'the accepted real Hall result');
  receipt.checks.push('Global Hall paints its real loading/failure message and Retry accepts the ordinary API result');

  const wave = target => leaderboard(target) && target.searchParams.get('board') === 'wave';
  const kills = target => leaderboard(target) && target.searchParams.get('board') === 'kills';
  const late = plan('superseded Hall board', wave, 'Owned superseded Hall failure');
  await gpu.click(page, 'hall-board:wave');
  await seen(late);
  await nextResponse(kills, () => gpu.click(page, 'hall-board:kills'));
  await release(late);
  await gpu.nextFrame(page);
  assert.equal((await gpu.widget(page, 'hall-board:kills')).selected, true);
  await gpu.noWidget(page, 'hall-retry');
  assert.notEqual(gpu.findWidget(await gpu.readUi(page), 'hall-status')?.value, late.failure);
  await gpu.click(page, 'hall-back');
  await gpu.surface(page, 'title');
  receipt.checks.push('a late error from the old Hall board cannot overwrite the actual selected board or expose its Retry');
}

async function directory() {
  const accepts = endpoint('api/game/parties');
  const initial = plan('party directory loading', accepts, 'Owned directory availability failure');
  await gpu.click(page, 'play');
  await gpu.click(page, 'join-party');
  await gpu.surface(page, 'join-party');
  await seen(initial);
  assert.ok((await gpu.widget(page, 'join-status')).label.includes('CONSULTING'));
  await release(initial);
  assert.equal((await gpu.widget(page, 'join-error')).label, initial.failure);
  await page.screenshot({path: path.join(output, 'directory-failure.png')});
  await nextResponse(accepts, () => gpu.click(page, 'join-refresh'));
  await gpu.noWidget(page, 'join-error');
  receipt.checks.push('the real directory loading/error state recovers through its ordinary Refresh control');

  const late = plan('retired directory', accepts, 'Owned retired directory failure');
  await gpu.click(page, 'join-refresh');
  await seen(late);
  await gpu.click(page, 'join-back');
  await gpu.surface(page, 'play');
  await release(late);
  await nextResponse(accepts, () => gpu.click(page, 'join-party'));
  await gpu.noWidget(page, 'join-error');
  await gpu.click(page, 'join-back');
  await gpu.click(page, 'title');
  await gpu.surface(page, 'title');
  receipt.checks.push('a late directory error after Back is excluded from the next actual directory session');
}

(async () => {
  fs.mkdirSync(output, {recursive: true});
  const owned = await launchOwnedBrowser(chromium, {headless: true,
    executablePath: process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    args: ['--use-angle=metal', '--ignore-gpu-blocklist']});
  try {
    context = await owned.browser.newContext({viewport: {width: 1600, height: 900},
      ignoreHTTPSErrors: process.env.LOCAL_TLS === '1'});
    page = await context.newPage();
    const observed = gpu.watchErrors(page, {errors: receipt.errors});
    await observed.ready;
    page.on('response', response => {
      const request = response.request();
      const item = plans.find(item => item.request === request);
      if (item) {
        item.response = {url: response.url(), status: response.status()};
        // watchErrors registered first: bind its response row to this exact
        // captured Request object, rather than exempt a URL or later response.
        const error = receipt.errors.at(-1);
        if (error?.type === 'response' && error.url === response.url()
          && error.status === response.status() && response.status() === 503)
          ownedResponseErrors.add(error);
      }
      if (new URL(response.url()).pathname.includes('/api/'))
        receipt.requests.push({url: response.url(), status: response.status(), owned: item?.label});
    });
    await context.route('**/api/**', async route => {
      const request = route.request();
      const item = plans.find(item => item.request === null && request.method() === 'GET'
        && item.accepts(new URL(request.url())));
      if (!item) {await route.continue(); return;}
      item.request = request;
      try {
        await item.gate;
        await route.fulfill({status: 503, contentType: 'application/json',
          body: JSON.stringify({error: item.failure})});
      } catch (error) {receipt.routeFailures.push({label: item.label, error: String(error.stack || error)});}
      finally {item.finish();}
    });
    await page.goto(url);
    await gpu.ready(page);
    if (gpu.findWidget(await gpu.readUi(page), 'prompt-secondary')) await gpu.click(page, 'prompt-secondary');
    await catalog();
    await hall();
    await directory();
    await gpu.assertGpuOnly(page);
    receipt.behaviorPassed = true;
  } catch (error) {
    receipt.failure = String(error.stack || error);
    process.exitCode = 1;
    if (page && !page.isClosed()) {
      try {
        receipt.actual = await gpu.readUi(page);
        await page.screenshot({path: path.join(output, 'recovery-failure.png')});
      } catch (error) {receipt.observationFailure = String(error.stack || error);}
    }
  } finally {
    try {
      for (const item of plans) item.release();
      if (context) await context.unrouteAll({behavior: 'wait'});
      if (page && !page.isClosed()) await gpu.settleErrors(page);
      assert.deepEqual(receipt.routeFailures, []);
      receipt.faults = plans.filter(item => item.request).map(item => ({label: item.label,
        url: item.request.url(), method: item.request.method(), failure: item.failure,
        response: item.response}));
      for (const fault of receipt.faults) assert.equal(fault.response?.status, 503);
      assert.equal(ownedResponseErrors.size, receipt.faults.length,
        'every deliberately failed native Request retains its own observed response error');
      const consoleCounts = new Map();
      receipt.unexpectedErrors = receipt.errors.filter(error => {
        if (ownedResponseErrors.has(error)) return false;
        if (error.type === 'console' && error.message.includes('503')) {
          const faults = receipt.faults.filter(fault => fault.response?.url === error.location?.url);
          const count = consoleCounts.get(error.location?.url) || 0;
          if (count < faults.length) {
            consoleCounts.set(error.location.url, count + 1);
            return false;
          }
        }
        return true;
      });
      assert.deepEqual(receipt.unexpectedErrors, [], 'only the exact deliberately failed responses may fail');
    } catch (error) {
      receipt.validationFailure = String(error.stack || error);
      process.exitCode = 1;
    } finally {
      try {receipt.browserCleanup = await owned.close();}
      catch (error) {receipt.browserCleanup = error.cleanup || {failure: String(error.stack || error)}; process.exitCode = 1;}
      receipt.passed = receipt.behaviorPassed === true && process.exitCode !== 1;
      fs.writeFileSync(path.join(output, 'receipt.json'), `${JSON.stringify(receipt, null, 2)}\n`);
    }
  }
  console.log(JSON.stringify(receipt));
})().catch(error => {console.error(error); process.exitCode = 1;});
