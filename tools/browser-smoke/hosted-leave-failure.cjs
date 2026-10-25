// Recoverable hosted leave failure through the real guest Global College and menu.
// Source: client/game-client-session.ts:473 and MainMenuScene.tsx:1607 in pinned Website.
// The only injected failure changes the FIRST actual SaveBeforeLeave response's
// checkpoint_sequence to zero. All requests, saves, state and other replies remain real.
const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const {chromium} = require(path.resolve(process.env.PW));
const {createNativeMessageReader} = require('./native-wire.cjs');

const url = process.env.URL || 'https://127.0.0.1:9449/game-stg/';
const output = path.resolve(process.env.OUT);
const expectedError = 'The game server could not create a final save.';
const checks = [];
const errors = [];
const cancelledMedia = [];
const connections = [];
const welcomes = [];
const leaveRequests = [];
const leaveReplies = [];
const apiRequests = [];
const wasmFiles = new Set();
let latestSnapshot;
let snapshotCount = 0;
let injection;
let step = 'browser startup';

async function waitUntil(predicate, label, timeout = 30000) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    if (await predicate()) return;
    await new Promise(resolve => setTimeout(resolve, 50));
  }
  throw new Error(`Timed out waiting for ${label}`);
}

async function bounded(promise, label, milliseconds) {
  let timer;
  try {
    return await Promise.race([promise, new Promise((resolve, reject) => {
      timer = setTimeout(() => reject(new Error(`${label} timed out`)), milliseconds);
    })]);
  } finally {
    clearTimeout(timer);
  }
}

function checked(label) {
  checks.push(label);
  console.log(label);
}

async function readUi(page) {
  return page.evaluate(() => ({
    status: document.querySelector('#status')?.textContent,
    menu: document.querySelector('.game-menus')?.dataset.menuScreen,
    // This diagnostic reads the accepted Session, not the display interpolation.
    acceptedTick: window.wasmBindings?.wizard_state()[5],
    text: document.body.innerText.slice(-1800),
  }));
}

async function installFailureRoute(page) {
  await page.routeWebSocket(() => true, route => {
    const server = route.connectToServer();
    const read=createNativeMessageReader();
    const connection = {path: new URL(route.url()).pathname};
    connections.push(connection);
    route.onMessage(payload => {
      const message = JSON.parse(payload.toString());
      if (message.type === 'save_before_leave') {
        leaveRequests.push({requestId: message.request_id, connection: connection.path});
      }
      server.send(payload);
    });
    server.onMessage(payload => {
      const message = read(JSON.parse(payload.toString()));
      if(!message){route.send(payload);return;}
      if (message.type === 'welcome') {
        welcomes.push({playerId: message.credentials.player_id,
          sessionKind: message.hosted?.session_kind});
        latestSnapshot = {sequence: message.frame.sequence, tick: message.frame.snapshot.ticks};
      } else if (message.type === 'snapshot') {
        snapshotCount += 1;
        latestSnapshot = {sequence: message.frame.sequence, tick: message.frame.snapshot.ticks};
      } else if (message.type === 'error') {
        errors.push(`host ${message.code}: ${message.reason}`);
      }
      if (message.type === 'hosted' && message.message.kind === 'save_before_leave') {
        const reply = {requestId: message.message.request_id,
          originalCheckpointSequence: message.message.checkpoint_sequence,
          deliveredCheckpointSequence: message.message.checkpoint_sequence};
        if (!injection && leaveRequests[0]?.requestId === reply.requestId) {
          reply.deliveredCheckpointSequence = 0;
          injection = {...reply, snapshotTick: latestSnapshot.tick};
          leaveReplies.push(reply);
          // Explicit server-failure injection at the untrusted transport boundary.
          // The real host's checkpoint document and every other field are forwarded intact.
          route.send(JSON.stringify({...message,
            message: {...message.message, checkpoint_sequence: 0}}));
          return;
        }
        leaveReplies.push(reply);
      }
      route.send(payload);
    });
    // Keep Playwright's default close forwarding in both directions.
  });
}

async function main() {
  fs.mkdirSync(output, {recursive: true});
  const receipt = {passed: false, url,
    driverSha256: crypto.createHash('sha256').update(fs.readFileSync(__filename)).digest('hex'),
    source: {revision: 'ed0a2d598f1df5ac51f9c335e453c7d7041fcb5a',
      failureMessage: expectedError,
      client: 'frontend/src/game/client/game-client-session.ts:473',
      menu: 'frontend/src/game/MainMenuScene.tsx:1607'},
    failureInjection: 'First real hosted SaveBeforeLeave response: checkpoint_sequence only -> 0',
    checks, errors, cancelledMedia, connections, welcomes, leaveRequests, leaveReplies, apiRequests};
  let browserServer;
  let page;
  try {
    browserServer = await chromium.launchServer({headless: true, host: '127.0.0.1',
      executablePath: process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
      args: ['--use-angle=metal', '--ignore-gpu-blocklist']});
    const browser = await chromium.connect(browserServer.wsEndpoint());
    page = await browser.newPage({ignoreHTTPSErrors: process.env.LOCAL_TLS === '1',
      viewport: {width: 1280, height: 720}});
    page.setDefaultTimeout(30000);
    page.setDefaultNavigationTimeout(30000);
    page.on('pageerror', error => errors.push(error.message));
    page.on('console', message => {
      if (message.type() === 'error') errors.push(message.text());
    });
    page.on('response', response => {
      const pathname = new URL(response.url()).pathname;
      if (pathname.endsWith('.wasm')) wasmFiles.add(path.basename(pathname));
      if (pathname.includes('/api/')) {
        apiRequests.push({path: pathname, method: response.request().method(), status: response.status()});
      }
      if (response.status() >= 400) errors.push(`${response.status()} ${pathname}`);
    });
    page.on('requestfailed', request => {
      const failure = `${new URL(request.url()).pathname}: ${request.failure()?.errorText}`;
      if (request.resourceType() === 'media' && request.failure()?.errorText === 'net::ERR_ABORTED') {
        cancelledMedia.push(failure);
      } else errors.push(failure);
    });
    await installFailureRoute(page);

    step = 'real guest Global admission';
    await page.goto(url, {waitUntil: 'load'});
    await page.waitForFunction(() => window.wasmBindings?.wizard_state().length > 0
      && document.querySelector('#status')?.textContent === '');
    receipt.renderer = await page.evaluate(() => {
      const gl = document.querySelector('canvas').getContext('webgl2');
      const info = gl.getExtension('WEBGL_debug_renderer_info');
      return info ? gl.getParameter(info.UNMASKED_RENDERER_WEBGL) : gl.getParameter(gl.RENDERER);
    });
    await page.locator('.title-prompt:not([hidden]) [data-menu-action="prompt-secondary"]').click();
    await page.locator('[data-menu-action="play"]').click();
    await page.locator('[data-menu-action="new-game"]').click();
    await page.getByRole('textbox', {name: 'Wizard name', exact: true}).fill('Leave Failure');
    await page.locator('[data-menu-action="element:fire"]').click();
    await page.locator('[data-menu-action="discipline:arcane"]').click();
    await waitUntil(() => welcomes.length === 1, 'real host Welcome');
    assert.equal(welcomes[0].sessionKind, 'GlobalHub');
    await page.locator('.game-menus[data-menu-screen="hidden"]').waitFor({state: 'attached'});
    await page.waitForFunction(() => document.querySelector('#status')?.textContent === '');
    assert.deepEqual(errors, []);
    checked('ordinary Create admits a real guest to the Global College');

    step = 'first real Leave with explicit zero-sequence failure';
    await page.keyboard.press('Escape');
    const leave = page.locator('.game-menus[data-menu-screen="pause"] [data-menu-action="leave"]');
    await leave.click();
    await waitUntil(() => injection !== undefined, 'actual SaveBeforeLeave reply to inject');
    assert.ok(injection.originalCheckpointSequence > 0, 'real host produced a positive checkpoint');
    assert.equal(leaveReplies.length, 1);
    assert.equal(leaveReplies[0].deliveredCheckpointSequence, 0);
    await page.waitForFunction(() => {
      const status = document.querySelector('#status')?.textContent;
      return status && status !== 'Saving game…';
    });
    receipt.failureUi = await readUi(page);
    assert.equal(receipt.failureUi.status, expectedError);
    await leave.waitFor({state: 'visible'});
    await page.screenshot({path: path.join(output, 'failed-leave-restored-pause.png')});
    checked('zero checkpoint shows the exact Source error and restores the ordinary pause menu');

    step = 'continued accepted snapshots after failed Leave';
    const before = receipt.failureUi.acceptedTick;
    assert.ok(Number.isFinite(before));
    await page.waitForFunction(tick => window.wasmBindings.wizard_state()[5] > tick + 10, before);
    const after = await readUi(page);
    receipt.continuedSession = {acceptedTickBefore: before, acceptedTickAfter: after.acceptedTick,
      receivedTick: latestSnapshot.tick, snapshotCount};
    assert.equal(after.menu, 'pause');
    assert.equal(connections.length, 1, 'failed Leave did not replace the authority connection');
    assert.equal(welcomes.length, 1, 'same admitted wizard continues without a new Welcome');
    assert.deepEqual(errors, []);
    checked('the same connection applies more than ten additional authoritative ticks after failure');

    step = 'normal Leave retry';
    await leave.click();
    await page.locator('.game-menus[data-menu-screen="title"]').waitFor();
    assert.equal(leaveRequests.length, 2);
    assert.equal(leaveReplies.length, 2);
    const retry = leaveReplies[1];
    assert.equal(retry.requestId, leaveRequests[1].requestId);
    assert.notEqual(retry.requestId, leaveRequests[0].requestId);
    assert.ok(retry.originalCheckpointSequence > 0);
    assert.equal(retry.deliveredCheckpointSequence, retry.originalCheckpointSequence);
    assert.deepEqual(errors, []);
    receipt.finalUi = await readUi(page);
    assert.equal(receipt.finalUi.status, '');
    await page.screenshot({path: path.join(output, 'retry-leave-title.png')});
    checked('normal retry persists a real positive checkpoint and reaches Title');
    receipt.passed = true;
  } catch (error) {
    receipt.failure = {step, message: error.message, stack: error.stack};
    if (page) {
      // Let the real server advance before recording a stopped client for the RED receipt.
      if (injection) {
        await waitUntil(() => latestSnapshot.tick > injection.snapshotTick + 10,
          'server ticks after failure', 2000).catch(() => {});
      }
      receipt.failureUi = await readUi(page).catch(failure => ({readError: failure.message}));
      await page.screenshot({path: path.join(output, 'failure.png')}).catch(() => {});
    }
    throw error;
  } finally {
    receipt.injection = injection;
    receipt.lastReceivedSnapshot = latestSnapshot;
    receipt.receivedSnapshotCount = snapshotCount;
    receipt.wasmFiles = [...wasmFiles];
    if (browserServer) {
      try {
        await bounded(browserServer.close(), 'Chrome close', 15000);
        receipt.cleanup = 'graceful';
      } catch {
        await bounded(browserServer.kill(), 'owned Chrome kill', 5000);
        receipt.cleanup = 'forced';
      }
      receipt.browserClosed = browserServer.process().exitCode !== null
        || browserServer.process().signalCode !== null;
    }
    fs.writeFileSync(path.join(output, 'receipt.json'), JSON.stringify(receipt, null, 2) + '\n');
    console.log(JSON.stringify({passed: receipt.passed, checks, failure: receipt.failure,
      wasmFiles: receipt.wasmFiles, browserClosed: receipt.browserClosed}));
  }
}

main().catch(error => {console.error(error); process.exitCode = 1;});
