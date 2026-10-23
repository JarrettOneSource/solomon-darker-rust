// Valid rejected Inventory receipt through the real guest Global College and UI.
// Source: hub-economy.ts:2010 and host/game-host.ts:2434 in pinned Website.
// Dragging the initial health potion onto Amulet asks the real host to Equip it,
// which rejects without changing inventory. Only that actual ACK loses inventory_result:
// the same absent-result contract produced by a rejected authored consumption.
// This checks the browser receiver, not the separate authored rollback transaction.
const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const {chromium} = require(path.resolve(process.env.PW));
const gpu = require('./gpu-ui.cjs');
const {createNativeMessageReader} = require('./native-wire.cjs');
const {startNewGame} = require('./menu-journey.cjs');

const url = process.env.URL || 'https://127.0.0.1:9449/game-stg/';
const output = path.resolve(process.env.OUT);
const checks = [];
const errors = [];
let observed;
const connections = [];
const welcomes = [];
const inventoryRequests = [];
const inventoryReplies = [];
const clientMessageTypes = {};
const serverMessageTypes = {};
const wasmFiles = new Set();
let latestSnapshot;
let snapshotCount = 0;
let injection;
let step = 'browser startup';

async function waitUntil(predicate, label, timeout = 10000) {
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
  const snapshot=await gpu.readUi(page);
  return {status:gpu.findWidget(snapshot,'runtime:status')?.value || '',menu:snapshot.surface,
    inventoryVisible:Boolean(gpu.findWidget(snapshot,'panel:equipment:amulet')),
    acceptedTick:await page.evaluate(()=>window.wasmBindings.wizard_state()[5]),
    text:snapshot.widgets.map(widget=>widget.label).join('\n').slice(-1200)};
}

async function equipPotion(page,potion,amulet) {
  await gpu.drag(page,potion,amulet);
}

function observeFrame(frame) {
  const playerId = welcomes[0]?.playerId;
  const economy = frame.snapshot.participants[playerId]?.player.economy;
  const inventory = economy ? {backpack: economy.backpack, equipment: economy.equipment} : null;
  latestSnapshot = {sequence: frame.sequence, tick: frame.snapshot.ticks,
    amuletEmpty: economy?.equipment.amulet === null,
    firstBackpackItem: economy?.backpack[0] ? {
      id: economy.backpack[0].id, kind: economy.backpack[0].kind,
      equipmentType: economy.backpack[0].equipment_type,
    } : null,
    // Keep credentials and complete checkpoint documents out of the receipt.
    inventorySha256: crypto.createHash('sha256').update(JSON.stringify(inventory)).digest('hex')};
}

async function installRejectionRoute(page) {
  await page.routeWebSocket(() => true, route => {
    const server = route.connectToServer();
    const readServer = createNativeMessageReader();
    const connection = {path: new URL(route.url()).pathname};
    connections.push(connection);
    route.onMessage(payload => {
      const message = JSON.parse(payload.toString());
      clientMessageTypes[message.type] = (clientMessageTypes[message.type] || 0) + 1;
      if (message.type === 'command' && message.command.kind === 'inventory') {
        inventoryRequests.push({sequence: message.sequence, action: message.command.action});
      }
      server.send(payload);
    });
    server.onMessage(payload => {
      const raw = JSON.parse(payload.toString());
      serverMessageTypes[raw.type] = (serverMessageTypes[raw.type] || 0) + 1;
      const message = readServer(raw);
      if (!message) {
        route.send(payload);
        return;
      }
      if (message.type === 'welcome') {
        welcomes.push({playerId: message.credentials.player_id,
          sessionKind: message.hosted?.session_kind});
        observeFrame(message.frame);
      } else if (message.type === 'snapshot') {
        snapshotCount += 1;
        observeFrame(message.frame);
      } else if (message.type === 'error') {
        errors.push(`host ${message.code}: ${message.reason}`);
      }
      if (message.type === 'command_result') {
        const request = inventoryRequests.find(request => request.sequence === message.sequence);
        if (request) {
          const reply = {sequence: message.sequence, snapshotSequence: message.snapshot_sequence,
            accepted: message.accepted, reason: message.reason,
            originalInventoryResult: message.inventory_result,
            deliveredInventoryResult: message.inventory_result !== undefined
              && message.inventory_result !== null};
          inventoryReplies.push(reply);
          if (!injection && message.accepted === false
            && request.action.Equip?.slot === 'Amulet') {
            reply.deliveredInventoryResult = false;
            injection = {...reply, snapshot: {...latestSnapshot}};
            // Explicit ACK-boundary injection: requests and authoritative state are real.
            // accepted/reason/sequence/snapshot_sequence remain exactly as the host sent them.
            const delivered = {...message};
            delete delivered.inventory_result;
            route.send(JSON.stringify(delivered));
            return;
          }
        }
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
      ordinaryRejection: 'frontend/src/game/core-kernels/hub-economy.ts:2010',
      authoredRejection: 'frontend/src/game/host/game-host.ts:2434'},
    injectionScope: 'Omit inventory_result from the first real rejected health-potion Equip ACK only',
    claimScope: 'Browser receiver liveness; actual authored-consumption rollback is a separate native fixture',
    checks, errors, connections, welcomes, inventoryRequests, inventoryReplies,
    clientMessageTypes, serverMessageTypes};
  let browserServer;
  let browser;
  let page;
  try {
    browserServer = await chromium.launchServer({headless: true, host: '127.0.0.1',
      executablePath: process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
      args: ['--use-angle=metal', '--ignore-gpu-blocklist']});
    browser = await chromium.connect(browserServer.wsEndpoint());
    page = await browser.newPage({ignoreHTTPSErrors: process.env.LOCAL_TLS === '1',
      viewport: {width: 1280, height: 720}});
    page.setDefaultTimeout(10000);
    page.setDefaultNavigationTimeout(30000);
    observed = gpu.watchErrors(page, {errors});
    await observed.ready;
    page.on('response', response => {
      const pathname = new URL(response.url()).pathname;
      if (pathname.endsWith('.wasm')) wasmFiles.add(path.basename(pathname));
    });
    await installRejectionRoute(page);

    step = 'real guest Global admission';
    await page.goto(url, {waitUntil: 'load'});
    await gpu.ready(page);
    receipt.renderer = await page.evaluate(() => {
      const gl = document.querySelector('canvas').getContext('webgl2');
      const info = gl.getExtension('WEBGL_debug_renderer_info');
      return info ? gl.getParameter(info.UNMASKED_RENDERER_WEBGL) : gl.getParameter(gl.RENDERER);
    });
    await startNewGame(page,{name:'Inventory Reject',element:'fire',discipline:'arcane'});
    await waitUntil(() => welcomes.length === 1, 'real host Welcome');
    assert.equal(welcomes[0].sessionKind, 'GlobalHub');
    await gpu.surface(page,'hidden');
    await gpu.settleErrors(page);
    assert.deepEqual(errors, []);
    checked('ordinary Create admits a real guest to the Global College');

    step = 'real Inventory ineligible Equip rejection';
    await page.keyboard.press('KeyI');
    const amulet='panel:equipment:amulet';await gpu.widget(page,amulet);
    const potion=(await gpu.byLabel(page,'Health Potion, quantity 1',{prefix:'panel:item:',role:'button'})).id;
    assert.equal(latestSnapshot.amuletEmpty, true, 'fresh wizard has no Amulet');
    receipt.beforeAction = {...latestSnapshot};
    assert.equal(receipt.beforeAction.firstBackpackItem.equipmentType, null);
    await equipPotion(page, potion, amulet);
    await waitUntil(() => injection !== undefined, 'real rejected Equip ACK');
    assert.equal(inventoryRequests.length, 1);
    assert.deepEqual(inventoryRequests[0].action,
      {Equip: {item_id: receipt.beforeAction.firstBackpackItem.id, slot: 'Amulet'}});
    assert.equal(injection.accepted, false);
    assert.equal(injection.reason, 'ineligible-item');
    assert.equal(injection.originalInventoryResult.accepted, false);
    assert.equal(injection.originalInventoryResult.reason, 'IneligibleItem');
    assert.equal(injection.deliveredInventoryResult, false);
    checked('the host naturally rejects equipping a potion; only inventory_result is omitted');

    step = 'rejected receipt UI and receiver liveness';
    await gpu.waitForUi(page,snapshot=>gpu.findWidget(snapshot,'runtime:status')?.value==='ineligible-item','actual rejected receipt status');
    receipt.rejectionUi = await readUi(page);
    assert.equal(receipt.rejectionUi.status, 'ineligible-item');
    assert.equal(receipt.rejectionUi.inventoryVisible, true);
    const before = receipt.rejectionUi.acceptedTick;
    await page.waitForFunction(tick => window.wasmBindings.wizard_state()[5] > tick + 10,
      before, {timeout: 5000});
    const after = await readUi(page);
    receipt.continuedSession = {acceptedTickBefore: before, acceptedTickAfter: after.acceptedTick,
      snapshot: {...latestSnapshot}, snapshotCount};
    assert.equal(after.inventoryVisible, true);
    assert.equal(latestSnapshot.inventorySha256, receipt.beforeAction.inventorySha256);
    assert.equal(connections.length, 1);
    assert.equal(welcomes.length, 1);
    await gpu.settleErrors(page);
    assert.deepEqual(errors, []);
    await page.screenshot({path: path.join(output, 'rejection-inventory-live.png')});
    checked('Inventory remains usable and more than ten additional authoritative ticks are accepted');

    step = 'unchanged ordinary Inventory retry';
    await equipPotion(page, potion, amulet);
    await waitUntil(() => inventoryReplies.length === 2, 'real unmodified retry ACK');
    const retry = inventoryReplies[1];
    assert.equal(inventoryRequests.length, 2);
    assert.notEqual(retry.sequence, injection.sequence);
    assert.equal(retry.accepted, false);
    assert.equal(retry.reason, 'ineligible-item');
    assert.equal(retry.deliveredInventoryResult, true);
    const beforeRetry = await readUi(page);
    await page.waitForFunction(tick => window.wasmBindings.wizard_state()[5] > tick + 10,
      beforeRetry.acceptedTick, {timeout: 5000});
    receipt.finalUi = await readUi(page);
    receipt.afterRetry = {...latestSnapshot};
    assert.equal(receipt.finalUi.inventoryVisible, true);
    assert.equal(receipt.afterRetry.inventorySha256, receipt.beforeAction.inventorySha256);
    await gpu.settleErrors(page);
    assert.deepEqual(errors, []);
    await gpu.assertGpuOnly(page);
    await page.screenshot({path: path.join(output, 'normal-retry-inventory-live.png')});
    checked('the same ordinary action retries through the real host and leaves inventory unchanged');
    receipt.passed = true;
  } catch (error) {
    receipt.failure = {step, message: error.message, stack: error.stack};
    if (page) {
      receipt.failureUi = await readUi(page).catch(failure => ({readError: failure.message}));
      if (injection) {
        const receivedBefore = latestSnapshot.tick;
        await waitUntil(() => latestSnapshot.tick > receivedBefore + 20,
          'server ticks after receiver failure', 3000).catch(() => {});
        receipt.failureAfterServerAdvance = {ui: await readUi(page).catch(() => null),
          receivedBefore, receivedAfter: latestSnapshot.tick};
      }
      await page.screenshot({path: path.join(output, 'failure.png')}).catch(() => {});
    }
    throw error;
  } finally {
    if (page) await gpu.settleErrors(page);
    receipt.expectedMediaSuspensions = observed?.expectedSuspensions || [];
    receipt.injection = injection;
    receipt.lastReceivedSnapshot = latestSnapshot;
    receipt.receivedSnapshotCount = snapshotCount;
    receipt.wasmFiles = [...wasmFiles];
    // Preserve the original failure even if the owned browser is slow to shut down.
    fs.writeFileSync(path.join(output, 'receipt.json'), JSON.stringify(receipt, null, 2) + '\n');
    if (browser) {
      await bounded(browser.close(), 'browser connection close', 5000).catch(error => {
        receipt.connectionCloseError = error.message;
      });
    }
    if (browserServer) {
      try {
        await bounded(browserServer.close(), 'Chrome close', 15000);
        receipt.cleanup = 'graceful';
      } catch (error) {
        receipt.cleanupError = error.message;
        await bounded(browserServer.kill(), 'owned Chrome kill', 5000).catch(error => {
          receipt.cleanupError += `; ${error.message}`;
        });
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
